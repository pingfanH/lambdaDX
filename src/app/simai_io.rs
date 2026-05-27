//! Bridge between the in-memory `ChartDoc` model and the `maisimai` crate.
//!
//! Provides:
//!   * `simai_file_to_chart_doc` — pick one chart from a parsed Simai file
//!     and convert to our flat `ChartDoc` (lane + seconds based).
//!   * `chart_doc_to_simai_file` — best-effort reverse conversion.
//!   * `import_from_simai_text` / `export_to_simai_text` — top-level helpers
//!     used by the UI.
//!
//! Note: the Simai format carries per-note attributes (break/ex/star, slide
//! `is_tapless`, equivalent BPM) that the editor's runtime model does not
//! distinguish. Those flags round-trip approximately — break/ex are dropped,
//! and slide heads are emitted with a `?` (tapless) modifier when no
//! companion Tap exists at the same beat.

use std::path::PathBuf;

use maisimai::{
    self as ms,
    Bpm, SimaiChart, SimaiFile, SimaiNote, SlidePattern,
};
use serde::de;

use super::platform;
use super::types::{ChartDoc, Note, NoteType, Slide, SlideSegment, SlidePoint, SlideShape, BpmChange, snap_measure};
use super::types::zone::PadZone;

/// Pick a chart from a Simai file (highest difficulty by default) and convert
/// it to a `ChartDoc`. Returns `Err` if the file has no charts.
pub fn simai_file_to_chart_doc(
    file: &SimaiFile,
    prefer: Option<u32>,
) -> Result<ChartDoc, String> {
    if file.charts.is_empty() {
        return Err("Simai file contains no &inote_N= charts".to_string());
    }
    let (num, chart) = if let Some(n) = prefer {
        file.charts
            .iter()
            .find(|(k, _)| *k == n)
            .cloned()
            .or_else(|| file.charts.last().cloned())
            .unwrap()
    } else {
        // Highest difficulty number wins.
        let mut best = file.charts[0].clone();
        for (k, c) in &file.charts {
            if *k > best.0 {
                best = (*k, c.clone());
            }
        }
        best
    };
    let _ = num;
    let mut doc = simai_chart_to_chart_doc(&chart);
    // Apply the `&first` offset: it specifies how many seconds into the
    // audio the first beat occurs. We store it in `audio_offset` so that
    // audio playback is shifted accordingly.
    doc.audio_offset = file.first;
    if doc.title.is_empty() {
        doc.title = if file.title.is_empty() {
            "Imported Simai".to_string()
        } else {
            file.title.clone()
        };
    }
    Ok(doc)
}

pub fn simai_chart_to_chart_doc(chart: &SimaiChart) -> ChartDoc {
    let bpm0 = chart.bpms.first().map(|b| b.bpm).unwrap_or(120.0);
    let bpms = ensure_initial_bpm(&chart.bpms, bpm0);

    // Mark each-groups: any note that shares its (rounded) measure with at
    // least one other note becomes `is_each = true`. Mirrors how the editor
    // already treats simultaneous notes.
    let mut measure_counts: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
    for n in &chart.notes {
        *measure_counts.entry(quantize(n.measure())).or_insert(0) += 1;
    }

    let mut notes: Vec<Note> = Vec::with_capacity(chart.notes.len());
    for sn in &chart.notes {
        let is_each = measure_counts
            .get(&quantize(sn.measure()))
            .map(|c| *c > 1)
            .unwrap_or(false);

        match sn {
            SimaiNote::Tap { measure, button, is_star, is_break, is_ex, .. } => {
                // Skip star-Taps emitted by the parser as the head of a
                // Slide on the same (button, measure): the Slide note
                // already renders its own star head.
                if *is_star {
                    let q = quantize(*measure);
                    let has_slide = chart.notes.iter().any(|m| matches!(m,
                        SimaiNote::Slide { measure: ms, start, .. }
                            if quantize(*ms) == q && start == button));
                    if has_slide {
                        // Star Tap's break/ex will be applied to Slide notes below.
                        continue;
                    }
                }
                notes.push(Note {
                    time: *measure,
                    lane: button + 1,
                    is_each,
                    is_break: *is_break,
                    is_ex: *is_ex,
                    is_star: *is_star,
                    ..Default::default()
                });
            }
            SimaiNote::Hold { measure, button, duration, is_ex, .. } => {
                notes.push(Note {
                    time: *measure,
                    lane: button + 1,
                    note_type: NoteType::Hold,
                    hold_duration: duration.max(0.0),
                    is_each,
                    ..Default::default()
                });
            }
            SimaiNote::Slide { measure, start, end, pattern, reflect, duration, delay, is_break, is_ex, is_tapless, chain, .. } => {
                // Build chain segments: first arc + chain arcs, each as a SlideSegment.
                let mut segments = Vec::new();
                let first_pts = simai_pattern_to_points(*start, *end, *pattern, *reflect);
                segments.push(SlideSegment {
                    points: first_pts,
                    shape: simai_pattern_to_shape(*pattern),
                });
                let mut prev_end = *end;
                for (cp, ce, cr) in chain {
                    let chain_pts = simai_pattern_to_points(prev_end, *ce, *cp, *cr);
                    segments.push(SlideSegment {
                        points: chain_pts,
                        shape: simai_pattern_to_shape(*cp),
                    });
                    prev_end = *ce;
                }
                let slide_obj = Slide {
                    segments,
                    slide_duration: (*delay + *duration).max(0.0),
                    slide_start_delay: delay.max(0.0),
                    slide_is_break: *is_break,
                };
                // Look for a star Tap at the same measure+button to get
                // the star head's break/ex flags.
                let q = quantize(*measure);
                let (sb, se) = chart.notes.iter().find_map(|m| match m {
                    SimaiNote::Tap { measure: tm, button: tb, is_star: true, is_break: tb_brk, is_ex: tb_ex, .. }
                        if quantize(*tm) == q && *tb == *start => Some((*tb_brk, *tb_ex)),
                    _ => None,
                }).unwrap_or((false, false));
                notes.push(Note {
                    time: *measure,
                    lane: start + 1,
                    note_type: NoteType::Slide,
                    is_each,
                    is_break: sb,
                    is_ex: se,
                    is_tapless: *is_tapless,
                    slide: vec![slide_obj],
                    ..Default::default()
                });
            }
            SimaiNote::TouchTap { measure, region, position, .. } => {
                if let Some(lane) = touch_to_lane(*region, *position) {
                    notes.push(Note {
                        time: *measure,
                        lane,
                        note_type: NoteType::Touch,
                        is_each,
                        ..Default::default()
                    });
                }
            }
            SimaiNote::TouchHold { measure, region, position, duration, .. } => {
                if let Some(lane) = touch_to_lane(*region, *position) {
                    notes.push(Note {
                        time: *measure,
                        lane,
                        note_type: NoteType::Hold,
                        hold_duration: duration.max(0.0),
                        is_each,
                        ..Default::default()
                    });
                }
            }
        }
    }

    notes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));

    let bpm_changes: Vec<BpmChange> = bpms.iter()
        .map(|b| BpmChange { measure: b.measure, bpm: b.bpm })
        .collect();

    ChartDoc {
        version: "0.3.0-measure".to_string(),
        title: String::new(),
        bpm: bpm0,
        bpms: bpm_changes,
        audio_offset: 0.0,
        notes,
    }
}

fn ensure_initial_bpm(bpms: &[Bpm], fallback: f32) -> Vec<Bpm> {
    let mut out: Vec<Bpm> = bpms.to_vec();
    out.sort_by(|a, b| a.measure.partial_cmp(&b.measure).unwrap_or(std::cmp::Ordering::Equal));
    if out.first().map(|b| b.measure > 1.0001).unwrap_or(true) {
        out.insert(0, Bpm { measure: 1.0, bpm: fallback });
    }
    out
}

fn quantize(measure: f32) -> i64 {
    (measure * 100_000.0).round() as i64
}


/// Map a Simai slide pattern into our discrete `SlideShape` enum.
fn simai_pattern_to_shape(p: SlidePattern) -> SlideShape {
    match p {
        SlidePattern::Line => SlideShape::Line,
        SlidePattern::Caret => SlideShape::Caret,
        SlidePattern::Left => SlideShape::Left,
        SlidePattern::Right => SlideShape::Right,
        SlidePattern::LowerV => SlideShape::VShape,
        SlidePattern::BigV => SlideShape::BigV,
        SlidePattern::S => SlideShape::S,
        SlidePattern::Z => SlideShape::Z,
        SlidePattern::P => SlideShape::P,
        SlidePattern::Q => SlideShape::Q,
        SlidePattern::PP => SlideShape::PP,
        SlidePattern::QQ => SlideShape::QQ,
        SlidePattern::Wifi => SlideShape::Wifi,
    }
}

pub fn shape_to_simai_pattern(shape: Option<SlideShape>) -> SlidePattern {
    match shape {
        Some(SlideShape::Caret) => SlidePattern::Caret,
        Some(SlideShape::Left) => SlidePattern::Left,
        Some(SlideShape::Right) => SlidePattern::Right,
        Some(SlideShape::VShape) => SlidePattern::LowerV,
        Some(SlideShape::BigV) => SlidePattern::BigV,
        Some(SlideShape::P) => SlidePattern::P,
        Some(SlideShape::Q) => SlidePattern::Q,
        Some(SlideShape::S) => SlidePattern::S,
        Some(SlideShape::Z) => SlidePattern::Z,
        Some(SlideShape::PP) => SlidePattern::PP,
        Some(SlideShape::QQ) => SlidePattern::QQ,
        Some(SlideShape::Wifi) => SlidePattern::Wifi,
        Some(SlideShape::Line) | None => SlidePattern::Line,
    }
}

/// Convert a Simai slide arc to a list of waypoint zones (excluding the
/// start). Lane numbers are 1-indexed A-buttons (1..=8). For `BigV`, the
/// reflect button comes first followed by the end.
///
/// Zone numbering: A1-A8 = 1-8 (outer ring), B1-B8 = 9-16 (inner ring),
/// C = 17 (center).
pub fn simai_pattern_to_points(start: u8, end: u8, pattern: SlidePattern, reflect: Option<u8>) -> Vec<SlidePoint> {
    let s = start + 1; // 1-indexed zone
    let e = end + 1;
    let sp = |z: u8| SlidePoint { zone: PadZone::from(z), beat_offset: 0.0 };
    return vec![sp(e)];
    match pattern {
        SlidePattern::Line => {
            // Straight line through center; endpoint only.
            vec![sp(e)]
        }
        SlidePattern::BigV => {
            let mut pts = Vec::new();
            if let Some(r) = reflect {
                pts.push(sp(r + 1));
            }
            pts.push(sp(e));
            pts
        }
        SlidePattern::Caret => {
            // Shorter arc around the outer ring.
            let cw = ring_cw(s, e);
            let ccw = ring_ccw(s, e);
            let route = if cw.len() <= ccw.len() { cw } else { ccw };
            route.into_iter().map(sp).collect()
        }
        SlidePattern::Right => {
            // > = CCW arc around the outer ring.
            ring_ccw(s, e).into_iter().map(sp).collect()
        }
        SlidePattern::Left => {
            // < = CW arc around the outer ring.
            ring_cw(s, e).into_iter().map(sp).collect()
        }
        SlidePattern::LowerV => {
            // V-shape through center.
            vec![sp(17), sp(e)]
        }
        SlidePattern::P => {
            // CW half-circle through inner ring.
            let mid = ring_cw(s, e);
            let mut pts: Vec<SlidePoint> = mid.iter()
                .take(mid.len().saturating_sub(1))
                .map(|&z| sp(a_to_b(z)))
                .collect();
            pts.push(sp(e));
            pts
        }
        SlidePattern::Q => {
            // CCW half-circle through inner ring.
            let mid = ring_ccw(s, e);
            let mut pts: Vec<SlidePoint> = mid.iter()
                .take(mid.len().saturating_sub(1))
                .map(|&z| sp(a_to_b(z)))
                .collect();
            pts.push(sp(e));
            pts
        }
        SlidePattern::PP => {
            // Full CW circle (inner ring) then to end.
            let full = ring_cw_full(s);
            let mut pts: Vec<SlidePoint> = full.iter().map(|&z| sp(a_to_b(z))).collect();
            // After the full loop, go from last inner zone outward to end.
            pts.push(sp(e));
            pts
        }
        SlidePattern::QQ => {
            // Full CCW circle (inner ring) then to end.
            let full = ring_ccw_full(s);
            let mut pts: Vec<SlidePoint> = full.iter().map(|&z| sp(a_to_b(z))).collect();
            pts.push(sp(e));
            pts
        }
        SlidePattern::S => {
            // S-curve: CW first half → center → CCW second half.
            s_curve_waypoints(s, e, false).into_iter().map(sp).collect()
        }
        SlidePattern::Z => {
            // Z-curve: CCW first half → center → CW second half.
            s_curve_waypoints(s, e, true).into_iter().map(sp).collect()
        }
        SlidePattern::Wifi => {
            // Fan shape — just the endpoint (wifi is visually 3 lanes but
            // stored as individual slides).
            vec![sp(e)]
        }
    }
}

/// A-zone (1-8) → corresponding B-zone (9-16).
fn a_to_b(a: u8) -> u8 { a + 8 }

/// CW traversal around the outer ring from `start` to `end` (both 1-indexed),
/// excluding start, including end.
fn ring_cw(start: u8, end: u8) -> Vec<u8> {
    let mut pts = Vec::new();
    let mut cur = start;
    for _ in 0..8 {
        cur = if cur == 8 { 1 } else { cur + 1 };
        pts.push(cur);
        if cur == end { break; }
    }
    pts
}

/// CCW traversal around the outer ring.
fn ring_ccw(start: u8, end: u8) -> Vec<u8> {
    let mut pts = Vec::new();
    let mut cur = start;
    for _ in 0..8 {
        cur = if cur == 1 { 8 } else { cur - 1 };
        pts.push(cur);
        if cur == end { break; }
    }
    pts
}

/// Full CW circle (8 positions starting from start+1, wrapping around).
fn ring_cw_full(start: u8) -> Vec<u8> {
    let mut pts = Vec::new();
    let mut cur = start;
    for _ in 0..8 {
        cur = if cur == 8 { 1 } else { cur + 1 };
        pts.push(cur);
    }
    pts
}

/// Full CCW circle.
fn ring_ccw_full(start: u8) -> Vec<u8> {
    let mut pts = Vec::new();
    let mut cur = start;
    for _ in 0..8 {
        cur = if cur == 1 { 8 } else { cur - 1 };
        pts.push(cur);
    }
    pts
}

/// Generate waypoint zones for an S or Z curve.
/// `reverse` = false → S (CW first half), true → Z (CCW first half).
fn s_curve_waypoints(s: u8, e: u8, reverse: bool) -> Vec<u8> {
    // S-curve: from start, curve one direction on the inner ring,
    // pass through center, then curve the opposite direction to end.
    let cw_dist = ring_cw(s, e).len() as i32;
    let half = (cw_dist + 1) / 2;
    let si = s as i32;
    let ei = e as i32;
    let (mid1, mid2) = if !reverse {
        // S: CW offset from start, CCW offset from end
        (wrap8(si + half), wrap8(ei - half.min(cw_dist - half)))
    } else {
        // Z: CCW offset from start, CW offset from end
        (wrap8(si - half), wrap8(ei + half.min(cw_dist - half)))
    };
    vec![a_to_b(mid1), 17, a_to_b(mid2), e]
}

/// Wrap a signed integer into the 1..=8 range (A-zone ring).
fn wrap8(v: i32) -> u8 {
    (((v - 1).rem_euclid(8)) + 1) as u8
}

fn touch_to_lane(region: char, position: u8) -> Option<u8> {
    match region {
        'A' => Some((position % 8) + 1),
        'B' => Some((position % 8) + 9),
        'C' => Some(17),
        'D' => Some((position % 8) + 18),
        'E' => Some((position % 8) + 26),
        _ => None,
    }
}

fn lane_to_touch(lane: u8) -> (char, u8) {
    match lane {
        1..=8 => ('A', lane - 1),
        9..=16 => ('B', lane - 9),
        17 => ('C', 0),
        18..=25 => ('D', lane - 18),
        26..=33 => ('E', lane - 26),
        _ => ('A', 0),
    }
}

// ──────────────────────────── Reverse conversion ──────────────────────────

pub fn chart_doc_to_simai_file(doc: &ChartDoc) -> SimaiFile {
    let chart = chart_doc_to_simai_chart(doc);
    SimaiFile {
        title: doc.title.clone(),
        artist: String::new(),
        first: doc.audio_offset,
        levels: vec![(6, "?".to_string())],
        charts: vec![(6, chart)],
        wholebpm: Some(doc.bpm),
    }
}

pub fn chart_doc_to_simai_chart(doc: &ChartDoc) -> SimaiChart {
    let bpms: Vec<Bpm> = if doc.bpms.is_empty() {
        let bpm0 = if doc.bpm > 0.0 { doc.bpm } else { 120.0 };
        vec![Bpm { measure: 1.0, bpm: bpm0 }]
    } else {
        doc.bpms.iter().map(|b| Bpm { measure: b.measure, bpm: b.bpm }).collect()
    };

    let mut notes: Vec<SimaiNote> = Vec::with_capacity(doc.notes.len());
    for n in &doc.notes {
        let measure = snap_measure(n.time);
        match n.note_type {
            NoteType::Tap => {
                if n.lane >= 1 && n.lane <= 8 {
                    notes.push(SimaiNote::Tap {
                        measure,
                        button: n.lane - 1,
                        is_break: n.is_break,
                        is_ex: n.is_ex,
                        is_star: n.is_star,
                    });
                } else {
                    let (region, position) = lane_to_touch(n.lane);
                    notes.push(SimaiNote::TouchTap { measure, region, position, is_firework: false });
                }
            }
            NoteType::Touch => {
                let (region, position) = lane_to_touch(n.lane);
                notes.push(SimaiNote::TouchTap { measure, region, position, is_firework: false });
            }
            NoteType::Hold => {
                let dur_meas = snap_measure(n.hold_duration.max(0.0));
                if n.lane >= 1 && n.lane <= 8 {
                    notes.push(SimaiNote::Hold {
                        measure,
                        button: n.lane - 1,
                        duration: dur_meas,
                        is_ex: n.is_ex,
                    });
                } else {
                    let (region, position) = lane_to_touch(n.lane);
                    notes.push(SimaiNote::TouchHold {
                        measure,
                        region,
                        position,
                        duration: dur_meas,
                        is_firework: false,
                    });
                }
            }
            NoteType::Slide => {
                if !(n.lane >= 1 && n.lane <= 8) {
                    continue;
                }
                // Emit a star Tap for the slide head (carries star break/ex).
                // Only emit once per (measure, lane) group.
                let already_emitted = notes.iter().any(|sn| matches!(sn,
                    SimaiNote::Tap { measure: m, button: b, is_star: true, .. }
                        if (*m - measure).abs() < 0.0001 && *b == n.lane - 1));
                if !already_emitted {
                    notes.push(SimaiNote::Tap {
                        measure,
                        button: n.lane - 1,
                        is_break: n.is_break,
                        is_ex: n.is_ex,
                        is_star: true,
                    });
                }
                // Emit one SimaiNote::Slide per Slide in note.slide.
                for sl in &n.slide {
                    // Collect all points across segments for this slide.
                    let all_points: Vec<&SlidePoint> = sl.segments.iter()
                        .flat_map(|seg| seg.points.iter())
                        .collect();
                    // First segment determines the primary pattern.
                    let first_shape = sl.segments.first()
                        .map(|seg| seg.shape)
                        .unwrap_or(SlideShape::Line);
                    let pattern = shape_to_simai_pattern(Some(first_shape));
                    let (reflect, end) = match (pattern, all_points.as_slice()) {
                        (SlidePattern::BigV, [r, e, ..]) => (Some(r.zone.to_id().saturating_sub(1)), e.zone.to_id().saturating_sub(1)),
                        (_, [.., last]) => (None, last.zone.to_id().saturating_sub(1)),
                        _ => (None, 0),
                    };
                    // Build chain from additional segments.
                    let chain: Vec<(SlidePattern, u8, Option<u8>)> = sl.segments.iter().skip(1)
                        .map(|seg| {
                            let cp = shape_to_simai_pattern(Some(seg.shape));
                            let ce = seg.points.last().map(|p| p.zone.to_id().saturating_sub(1)).unwrap_or(0);
                            (cp, ce, None)
                        })
                        .collect();
                    let delay_meas = snap_measure(sl.slide_start_delay.max(0.0));
                    let total_meas = snap_measure(sl.slide_duration.max(0.0));
                    let travel_meas = (total_meas - delay_meas).max(0.05);
                    notes.push(SimaiNote::Slide {
                        measure,
                        start: n.lane - 1,
                        end,
                        pattern,
                        reflect,
                        duration: travel_meas,
                        delay: delay_meas,
                        is_break: sl.slide_is_break,
                        is_ex: false,
                        is_tapless: n.is_tapless,
                        chain,
                    });
                }
            }
        }
    }

    SimaiChart { notes, bpms }
}

// ──────────────────────────────── I/O helpers ─────────────────────────────

/// Read a Simai file from `<output>/<name>` and convert it to a ChartDoc.
pub fn import_from_simai_path(name: &str) -> Result<ChartDoc, String> {
    let text = platform::read_output_text(name)?;
    let parsed = if text.contains("&inote_") || text.contains("&title=") {
        ms::parse_file(&text).map_err(|e| format!("simai parse: {e}"))?
    } else {
        // Treat the whole file as a bare chart body.
        let chart = ms::parse_chart_text(&text).map_err(|e| format!("simai parse: {e}"))?;
        SimaiFile { title: String::new(), artist: String::new(), first: 0.0, levels: Vec::new(), charts: vec![(0, chart)], wholebpm: None }
    };
    simai_file_to_chart_doc(&parsed, None)
}

/// Export a ChartDoc as a Simai maidata.txt-style file under `<output>/<name>`.
pub fn export_to_simai_path(doc: &ChartDoc, name: &str) -> Result<PathBuf, String> {
    let file = chart_doc_to_simai_file(doc);
    let text = ms::export_file(&file);
    platform::write_output_text(name, &text)
}
