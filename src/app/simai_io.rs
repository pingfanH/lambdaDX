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

use super::platform;
use super::types::{ChartDoc, Note, NoteType, SlidePoint, SlideShape};

/// Pick a chart from a Simai file (highest difficulty by default) and convert
/// it to a `ChartDoc`. Returns `Err` if the file has no charts.
pub(crate) fn simai_file_to_chart_doc(
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
    if doc.title.is_empty() {
        doc.title = if file.title.is_empty() {
            "Imported Simai".to_string()
        } else {
            file.title.clone()
        };
    }
    Ok(doc)
}

pub(crate) fn simai_chart_to_chart_doc(chart: &SimaiChart) -> ChartDoc {
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
            SimaiNote::Tap { measure, button, is_star, .. } => {
                // Skip star-Taps emitted by the parser as the head of a
                // Slide on the same (button, measure): the Slide note
                // already renders its own star head.
                if *is_star {
                    let q = quantize(*measure);
                    let has_slide = chart.notes.iter().any(|m| matches!(m,
                        SimaiNote::Slide { measure: ms, start, .. }
                            if quantize(*ms) == q && start == button));
                    if has_slide {
                        continue;
                    }
                }
                notes.push(Note {
                    time: ms::measure_to_seconds(*measure, &bpms),
                    lane: button + 1,
                    note_type: NoteType::Tap,
                    hold_duration: 0.0,
                    is_each,
                    slide_points: Vec::new(),
                    slide_duration: 0.0,
                    slide_start_delay: 0.0,
                    slide_shape: None,
                });
            }
            SimaiNote::Hold { measure, button, duration, .. } => {
                let t0 = ms::measure_to_seconds(*measure, &bpms);
                let t1 = ms::measure_to_seconds(*measure + *duration, &bpms);
                notes.push(Note {
                    time: t0,
                    lane: button + 1,
                    note_type: NoteType::Hold,
                    hold_duration: (t1 - t0).max(0.0),
                    is_each,
                    slide_points: Vec::new(),
                    slide_duration: 0.0,
                    slide_start_delay: 0.0,
                    slide_shape: None,
                });
            }
            SimaiNote::Slide { measure, start, end, pattern, reflect, duration, delay, .. } => {
                let t_head = ms::measure_to_seconds(*measure, &bpms);
                let t_motion_start = ms::measure_to_seconds(*measure + *delay, &bpms);
                let t_tail = ms::measure_to_seconds(*measure + *delay + *duration, &bpms);
                let slide_points = simai_pattern_to_points(*start, *end, *pattern, *reflect);
                notes.push(Note {
                    time: t_head,
                    lane: start + 1,
                    note_type: NoteType::Slide,
                    hold_duration: 0.0,
                    is_each,
                    slide_points,
                    slide_duration: (t_tail - t_head).max(0.05),
                    slide_start_delay: (t_motion_start - t_head).max(0.0),
                    slide_shape: Some(simai_pattern_to_shape(*pattern)),
                });
            }
            SimaiNote::TouchTap { measure, region, position, .. } => {
                if let Some(lane) = touch_to_lane(*region, *position) {
                    notes.push(Note {
                        time: ms::measure_to_seconds(*measure, &bpms),
                        lane,
                        note_type: NoteType::Touch,
                        hold_duration: 0.0,
                        is_each,
                        slide_points: Vec::new(),
                        slide_duration: 0.0,
                        slide_start_delay: 0.0,
                        slide_shape: None,
                    });
                }
            }
            SimaiNote::TouchHold { measure, region, position, duration, .. } => {
                if let Some(lane) = touch_to_lane(*region, *position) {
                    let t0 = ms::measure_to_seconds(*measure, &bpms);
                    let t1 = ms::measure_to_seconds(*measure + *duration, &bpms);
                    notes.push(Note {
                        time: t0,
                        lane,
                        note_type: NoteType::Hold,
                        hold_duration: (t1 - t0).max(0.0),
                        is_each,
                        slide_points: Vec::new(),
                        slide_duration: 0.0,
                        slide_start_delay: 0.0,
                        slide_shape: None,
                    });
                }
            }
        }
    }

    notes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));

    ChartDoc {
        version: "0.2.0-simai".to_string(),
        title: String::new(),
        bpm: bpm0,
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

fn shape_to_simai_pattern(shape: Option<SlideShape>) -> SlidePattern {
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
fn simai_pattern_to_points(_start: u8, end: u8, pattern: SlidePattern, reflect: Option<u8>) -> Vec<SlidePoint> {
    let mut pts = Vec::new();
    if let (SlidePattern::BigV, Some(r)) = (pattern, reflect) {
        pts.push(SlidePoint { zone: r + 1, beat_offset: 0.0 });
    }
    pts.push(SlidePoint { zone: end + 1, beat_offset: 0.0 });
    pts
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
        17 | 34 => ('C', 0),
        18..=25 => ('D', lane - 18),
        26..=33 => ('E', lane - 26),
        _ => ('A', 0),
    }
}

// ──────────────────────────── Reverse conversion ──────────────────────────

pub(crate) fn chart_doc_to_simai_file(doc: &ChartDoc) -> SimaiFile {
    let chart = chart_doc_to_simai_chart(doc);
    SimaiFile {
        title: doc.title.clone(),
        artist: String::new(),
        first: 0.0,
        levels: vec![(6, "?".to_string())],
        charts: vec![(6, chart)],
        wholebpm: Some(doc.bpm),
    }
}

pub(crate) fn chart_doc_to_simai_chart(doc: &ChartDoc) -> SimaiChart {
    let bpm0 = if doc.bpm > 0.0 { doc.bpm } else { 120.0 };
    let bpms = vec![Bpm { measure: 1.0, bpm: bpm0 }];

    let mut notes: Vec<SimaiNote> = Vec::with_capacity(doc.notes.len());
    for n in &doc.notes {
        let measure = ms::seconds_to_measure(n.time, &bpms);
        match n.note_type {
            NoteType::Tap => {
                if n.lane >= 1 && n.lane <= 8 {
                    notes.push(SimaiNote::Tap {
                        measure,
                        button: n.lane - 1,
                        is_break: false,
                        is_ex: false,
                        is_star: false,
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
                let dur_meas = ms::seconds_to_measure(n.time + n.hold_duration.max(0.0), &bpms) - measure;
                if n.lane >= 1 && n.lane <= 8 {
                    notes.push(SimaiNote::Hold {
                        measure,
                        button: n.lane - 1,
                        duration: dur_meas.max(0.0),
                        is_ex: false,
                    });
                } else {
                    let (region, position) = lane_to_touch(n.lane);
                    notes.push(SimaiNote::TouchHold {
                        measure,
                        region,
                        position,
                        duration: dur_meas.max(0.0),
                        is_firework: false,
                    });
                }
            }
            NoteType::Slide => {
                if !(n.lane >= 1 && n.lane <= 8) {
                    continue;
                }
                let pattern = shape_to_simai_pattern(n.slide_shape);
                let (reflect, end) = match (pattern, n.slide_points.as_slice()) {
                    (SlidePattern::BigV, [r, e, ..]) => (Some(r.zone.saturating_sub(1)), e.zone.saturating_sub(1)),
                    (_, [.., last]) => (None, last.zone.saturating_sub(1)),
                    _ => (None, 0),
                };
                let total_meas = ms::seconds_to_measure(n.time + n.slide_duration.max(0.0), &bpms) - measure;
                let delay_meas = ms::seconds_to_measure(n.time + n.slide_start_delay.max(0.0), &bpms) - measure;
                let travel_meas = (total_meas - delay_meas).max(0.05);
                notes.push(SimaiNote::Slide {
                    measure,
                    start: n.lane - 1,
                    end,
                    pattern,
                    reflect,
                    duration: travel_meas,
                    delay: delay_meas.max(0.0),
                    is_break: false,
                    is_ex: false,
                    is_tapless: !has_companion_tap(doc, n),
                });
            }
        }
    }

    SimaiChart { notes, bpms }
}

fn has_companion_tap(doc: &ChartDoc, slide: &Note) -> bool {
    doc.notes.iter().any(|m| {
        matches!(m.note_type, NoteType::Tap)
            && m.lane == slide.lane
            && (m.time - slide.time).abs() < 0.005
    })
}

// ──────────────────────────────── I/O helpers ─────────────────────────────

/// Read a Simai file from `<output>/<name>` and convert it to a ChartDoc.
pub(crate) fn import_from_simai_path(name: &str) -> Result<ChartDoc, String> {
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
pub(crate) fn export_to_simai_path(doc: &ChartDoc, name: &str) -> Result<PathBuf, String> {
    let file = chart_doc_to_simai_file(doc);
    let text = ms::export_file(&file);
    platform::write_output_text(name, &text)
}
