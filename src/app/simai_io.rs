//! Bridge between the in-memory `ChartDoc` model and Simai maidata text.
//!
//! Provides:
//!   * `simai_file_to_chart_doc` — pick one chart from a cached Simai file
//!     and convert to our flat `ChartDoc` (lane + seconds based).
//!   * `chart_doc_to_simai_file` — best-effort reverse conversion.
//!
//! Simai import is compiled through `lnmai-core`'s Lean FFI. The local Rust
//! code in this module only keeps source metadata and formats ChartDoc values
//! back to text for save/export workflows.

use std::path::PathBuf;

use lnmai_core::types as core;

use super::platform;
use super::types::zone::PadZone;
use super::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
    measure_to_secs, sdur_to_mdur, secs_to_measure, snap_measure,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Bpm {
    pub measure: f32,
    pub bpm: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlidePattern {
    Line,
    Caret,
    Left,
    Right,
    LowerV,
    BigV,
    S,
    Z,
    P,
    Q,
    PP,
    QQ,
    Wifi,
}

impl SlidePattern {
    fn as_str(&self) -> &'static str {
        match self {
            SlidePattern::Line => "-",
            SlidePattern::Caret => "^",
            SlidePattern::Left => "<",
            SlidePattern::Right => ">",
            SlidePattern::LowerV => "v",
            SlidePattern::BigV => "V",
            SlidePattern::S => "s",
            SlidePattern::Z => "z",
            SlidePattern::P => "p",
            SlidePattern::Q => "q",
            SlidePattern::PP => "pp",
            SlidePattern::QQ => "qq",
            SlidePattern::Wifi => "w",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimaiNote {
    Tap {
        measure: f32,
        button: u8,
        is_break: bool,
        is_ex: bool,
        is_star: bool,
        hi_speed: f32,
    },
    Hold {
        measure: f32,
        button: u8,
        duration: f32,
        is_ex: bool,
        hi_speed: f32,
    },
    Slide {
        measure: f32,
        start: u8,
        end: u8,
        pattern: SlidePattern,
        reflect: Option<u8>,
        duration: f32,
        delay: f32,
        is_break: bool,
        is_ex: bool,
        is_tapless: bool,
        chain: Vec<(SlidePattern, u8, Option<u8>, bool)>,
        hi_speed: f32,
    },
    TouchTap {
        measure: f32,
        region: char,
        position: u8,
        is_firework: bool,
        hi_speed: f32,
    },
    TouchHold {
        measure: f32,
        region: char,
        position: u8,
        duration: f32,
        is_firework: bool,
        hi_speed: f32,
    },
}

impl SimaiNote {
    fn measure(&self) -> f32 {
        match self {
            SimaiNote::Tap { measure, .. }
            | SimaiNote::Hold { measure, .. }
            | SimaiNote::Slide { measure, .. }
            | SimaiNote::TouchTap { measure, .. }
            | SimaiNote::TouchHold { measure, .. } => *measure,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimaiChart {
    pub notes: Vec<SimaiNote>,
    pub bpms: Vec<Bpm>,
}

#[derive(Debug, Clone, Default)]
pub struct SimaiFile {
    pub title: String,
    pub artist: String,
    pub first: f32,
    pub levels: Vec<(u32, String)>,
    pub charts: Vec<(u32, SimaiChart)>,
    pub wholebpm: Option<f32>,
    source_text: String,
}

impl SimaiFile {
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    pub fn chart_count(&self) -> usize {
        if self.charts.is_empty() {
            self.levels.len()
        } else {
            self.charts.len()
        }
    }
}

/// Pick a chart from a Simai file (highest difficulty by default) and convert
/// it to a `ChartDoc`. Returns `Err` if the file has no charts.
pub fn simai_file_to_chart_doc(file: &SimaiFile, prefer: Option<u32>) -> Result<ChartDoc, String> {
    let level = select_chart_level(file, prefer)?;
    ensure_lnmai_runtime()?;
    let source_text = if file.source_text.is_empty() {
        export_simai_file(file)
    } else {
        file.source_text.clone()
    };
    let frontend = lnmai_core::api::parse_frontend_chart(&source_text, level)
        .map_err(|e| format!("lnmai-core parse: {}", ffi_error_message(&e.json)))?;
    frontend_chart_to_chart_doc(file, level, &frontend)
}

fn ensure_lnmai_runtime() -> Result<(), String> {
    lnmai_core::session::ensure_runtime()
        .map_err(|_| "lnmai-core runtime initialization failed".to_string())
}

fn ffi_error_message(json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return json.to_string();
    };
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|message| message.as_str())
        .or_else(|| value.get("message").and_then(|message| message.as_str()))
        .unwrap_or(json)
        .to_string()
}

fn select_chart_level(file: &SimaiFile, prefer: Option<u32>) -> Result<u32, String> {
    let mut levels: Vec<u32> = file.levels.iter().map(|(level, _)| *level).collect();
    for (level, _) in &file.charts {
        if !levels.contains(level) {
            levels.push(*level);
        }
    }
    levels.sort_unstable();
    if levels.is_empty() {
        return Err("Simai file contains no &inote_N= charts".to_string());
    }
    if let Some(preferred) = prefer {
        if levels.contains(&preferred) {
            return Ok(preferred);
        }
        return levels
            .last()
            .copied()
            .ok_or_else(|| "Simai file contains no &inote_N= charts".to_string());
    }
    levels
        .last()
        .copied()
        .ok_or_else(|| "Simai file contains no &inote_N= charts".to_string())
}

fn frontend_chart_to_chart_doc(
    file: &SimaiFile,
    level: u32,
    frontend: &core::FrontendChartResult,
) -> Result<ChartDoc, String> {
    let bpms = bpm_changes_from_source(file, &frontend.inspection.source.events);
    let bpm0 = bpms.first().map(|b| b.bpm).unwrap_or(120.0);
    let mut notes = Vec::new();

    for event in &frontend.inspection.source.events {
        for note in &event.notes {
            if let Some(converted) = token_to_note(&note.token, file.first, &bpms) {
                notes.push(converted);
            }
        }
    }

    notes.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tag_each_notes(&mut notes);

    let mut doc = ChartDoc {
        version: "0.3.0-measure".to_string(),
        title: file.title.clone(),
        artist: file.artist.clone(),
        simai_level: level,
        bpm: bpm0,
        bpms,
        audio_offset: file.first,
        notes,
        templates: Vec::new(),
        template_instances: Vec::new(),
    };
    if doc.title.is_empty() {
        doc.title = "Imported Simai".to_string();
    }
    Ok(doc)
}

fn bpm_changes_from_source(file: &SimaiFile, events: &[core::SourceEvent]) -> Vec<BpmChange> {
    let initial = events
        .first()
        .map(|event| rational_to_f32(&event.bpm))
        .filter(|bpm| *bpm > 0.0)
        .or(file.wholebpm)
        .unwrap_or(120.0);
    let mut bpms = vec![BpmChange {
        measure: 1.0,
        bpm: initial,
    }];
    let mut last_bpm = initial;
    for event in events {
        let bpm = rational_to_f32(&event.bpm);
        if bpm <= 0.0 || (bpm - last_bpm).abs() < 0.001 {
            continue;
        }
        let seconds = chart_seconds(event.timing, file.first);
        let measure = snap_measure(secs_to_measure(seconds, &bpms));
        if let Some(first) = bpms.first_mut().filter(|_| (measure - 1.0).abs() < 0.0001) {
            first.bpm = bpm;
        } else {
            bpms.push(BpmChange { measure, bpm });
        }
        last_bpm = bpm;
    }
    bpms
}

fn token_to_note(token: &core::RawNoteToken, first: f32, bpms: &[BpmChange]) -> Option<Note> {
    let measure = snap_measure(secs_to_measure(chart_seconds(token.timing, first), bpms));
    let hi_speed = rational_to_f32(&token.h_speed).max(0.0);
    match token.kind {
        core::RawNoteKind::Tap => {
            let lane = token.slot.map(slot_to_lane)?;
            Some(Note {
                time: measure,
                lane,
                is_break: token.is_break,
                is_ex: token.is_ex,
                is_star: token.is_force_star,
                hi_speed,
                ..Default::default()
            })
        }
        core::RawNoteKind::Hold => {
            let lane = token.slot.map(slot_to_lane)?;
            Some(Note {
                time: measure,
                lane,
                note_type: NoteType::Hold,
                hold_duration: token_duration_measures(token, measure, bpms),
                is_break: token.is_break,
                is_ex: token.is_ex,
                hi_speed,
                ..Default::default()
            })
        }
        core::RawNoteKind::Touch => {
            let lane = token.sensor_pos.map(sensor_to_lane)?;
            Some(Note {
                time: measure,
                lane,
                note_type: NoteType::Touch,
                is_break: token.is_break,
                hi_speed,
                ..Default::default()
            })
        }
        core::RawNoteKind::TouchHold => {
            let lane = token.sensor_pos.map(sensor_to_lane)?;
            Some(Note {
                time: measure,
                lane,
                note_type: NoteType::Hold,
                hold_duration: token_duration_measures(token, measure, bpms),
                is_break: token.is_break,
                is_ex: token.is_ex,
                hi_speed,
                ..Default::default()
            })
        }
        core::RawNoteKind::Slide => {
            let lane = token.slot.map(slot_to_lane)?;
            let body = token.slide_body.as_ref()?;
            let end = body.end_area.map(sensor_to_outer_index)?;
            let pattern = slide_body_to_pattern(body);
            let reflect = body.turn_area.map(sensor_to_outer_index);
            let delay = duration_measures(token.star_wait.unwrap_or(0), measure, bpms);
            let travel_start = measure + delay;
            let travel = token
                .length
                .map(|length| duration_measures(length, travel_start, bpms))
                .unwrap_or_else(|| 1.0 / token.divisor.max(1) as f32);
            let shape = simai_pattern_to_shape(pattern);
            let slide = Slide {
                segments: vec![SlideSegment {
                    points: simai_pattern_to_points(lane.saturating_sub(1), end, pattern, reflect),
                    shape,
                }],
                slide_duration: (delay + travel).max(0.0),
                slide_start_delay: delay.max(0.0),
                slide_is_break: token.is_slide_break || token.is_break,
            };
            Some(Note {
                time: measure,
                lane,
                note_type: NoteType::Slide,
                is_break: token.is_break,
                is_ex: token.is_ex,
                is_star: !token.is_slide_no_head || token.is_force_star,
                is_tapless: token.is_slide_no_head,
                hi_speed,
                slide: vec![slide],
                ..Default::default()
            })
        }
        core::RawNoteKind::Rest | core::RawNoteKind::Unknown => None,
    }
}

fn token_duration_measures(token: &core::RawNoteToken, start_measure: f32, bpms: &[BpmChange]) -> f32 {
    token
        .length
        .map(|length| duration_measures(length, start_measure, bpms))
        .unwrap_or_else(|| 1.0 / token.divisor.max(1) as f32)
        .max(0.0)
}

fn duration_measures(duration: core::Duration, start_measure: f32, bpms: &[BpmChange]) -> f32 {
    let seconds = duration as f32 / 1_000_000.0;
    let start_secs = measure_to_secs(start_measure, bpms);
    sdur_to_mdur(seconds, start_secs, bpms)
}

fn chart_seconds(time: core::TimePoint, first: f32) -> f32 {
    ((time as f32 / 1_000_000.0) - first).max(0.0)
}

fn rational_to_f32(value: &core::Rational) -> f32 {
    value
        .decimal
        .parse::<f32>()
        .unwrap_or_else(|_| value.num as f32 / value.den.max(1) as f32)
}

fn slot_to_lane(slot: core::OuterSlot) -> u8 {
    match slot {
        core::OuterSlot::S1 => 1,
        core::OuterSlot::S2 => 2,
        core::OuterSlot::S3 => 3,
        core::OuterSlot::S4 => 4,
        core::OuterSlot::S5 => 5,
        core::OuterSlot::S6 => 6,
        core::OuterSlot::S7 => 7,
        core::OuterSlot::S8 => 8,
    }
}

fn sensor_to_lane(area: core::SensorArea) -> u8 {
    match area {
        core::SensorArea::A1 => 1,
        core::SensorArea::A2 => 2,
        core::SensorArea::A3 => 3,
        core::SensorArea::A4 => 4,
        core::SensorArea::A5 => 5,
        core::SensorArea::A6 => 6,
        core::SensorArea::A7 => 7,
        core::SensorArea::A8 => 8,
        core::SensorArea::B1 => 9,
        core::SensorArea::B2 => 10,
        core::SensorArea::B3 => 11,
        core::SensorArea::B4 => 12,
        core::SensorArea::B5 => 13,
        core::SensorArea::B6 => 14,
        core::SensorArea::B7 => 15,
        core::SensorArea::B8 => 16,
        core::SensorArea::C => 17,
        core::SensorArea::D1 => 18,
        core::SensorArea::D2 => 19,
        core::SensorArea::D3 => 20,
        core::SensorArea::D4 => 21,
        core::SensorArea::D5 => 22,
        core::SensorArea::D6 => 23,
        core::SensorArea::D7 => 24,
        core::SensorArea::D8 => 25,
        core::SensorArea::E1 => 26,
        core::SensorArea::E2 => 27,
        core::SensorArea::E3 => 28,
        core::SensorArea::E4 => 29,
        core::SensorArea::E5 => 30,
        core::SensorArea::E6 => 31,
        core::SensorArea::E7 => 32,
        core::SensorArea::E8 => 33,
    }
}

fn sensor_to_outer_index(area: core::SensorArea) -> u8 {
    sensor_to_lane(area).saturating_sub(1) % 8
}

fn slide_body_to_pattern(body: &core::ParsedSlideBody) -> SlidePattern {
    match body.kind {
        core::SlideBodyKind::Line => SlidePattern::Line,
        core::SlideBodyKind::CircleRight => SlidePattern::Right,
        core::SlideBodyKind::CircleLeft => SlidePattern::Left,
        core::SlideBodyKind::CircleUp => SlidePattern::Caret,
        core::SlideBodyKind::V => SlidePattern::LowerV,
        core::SlideBodyKind::Pp => SlidePattern::PP,
        core::SlideBodyKind::Qq => SlidePattern::QQ,
        core::SlideBodyKind::P => SlidePattern::P,
        core::SlideBodyKind::Q => SlidePattern::Q,
        core::SlideBodyKind::S => SlidePattern::S,
        core::SlideBodyKind::Z => SlidePattern::Z,
        core::SlideBodyKind::Turn => SlidePattern::BigV,
        core::SlideBodyKind::Wifi => SlidePattern::Wifi,
    }
}

fn tag_each_notes(notes: &mut [Note]) {
    let mut measure_counts = std::collections::HashMap::<i64, u32>::new();
    for note in notes.iter() {
        *measure_counts.entry(quantize(note.time)).or_insert(0) += 1;
    }
    for note in notes {
        note.is_each = measure_counts
            .get(&quantize(note.time))
            .map(|count| *count > 1)
            .unwrap_or(false);
    }
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
            SimaiNote::Tap {
                measure,
                button,
                is_star,
                is_break,
                is_ex,
                hi_speed,
                ..
            } => {
                // Skip star-Taps emitted by the parser as the head of a
                // Slide on the same (button, measure): the Slide note
                // already renders its own star head.
                if *is_star {
                    let q = quantize(*measure);
                    let has_slide = chart.notes.iter().any(|m| {
                        matches!(m,
                        SimaiNote::Slide { measure: ms, start, .. }
                            if quantize(*ms) == q && start == button)
                    });
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
                    hi_speed: *hi_speed,
                    ..Default::default()
                });
            }
            SimaiNote::Hold {
                measure,
                button,
                duration,
                is_ex,
                hi_speed,
                ..
            } => {
                notes.push(Note {
                    time: *measure,
                    lane: button + 1,
                    note_type: NoteType::Hold,
                    hold_duration: duration.max(0.0),
                    is_each,
                    is_ex: *is_ex,
                    hi_speed: *hi_speed,
                    ..Default::default()
                });
            }
            SimaiNote::Slide {
                measure,
                start,
                end,
                pattern,
                reflect,
                duration,
                delay,
                is_break,
                is_ex: _,
                is_tapless,
                chain,
                hi_speed,
                ..
            } => {
                let mut slides: Vec<Slide> = Vec::new();
                let mut cur_segments: Vec<SlideSegment> = Vec::new();
                let first_pts = simai_pattern_to_points(*start, *end, *pattern, *reflect);
                cur_segments.push(SlideSegment {
                    points: first_pts,
                    shape: simai_pattern_to_shape(*pattern),
                });
                let mut prev_end = *end;
                for (cp, ce, cr, is_star) in chain {
                    let chain_pts = simai_pattern_to_points(prev_end, *ce, *cp, *cr);
                    if *is_star {
                        // Flush current segments as a Slide, start new one
                        // `slide_duration` is the total span from the head
                        // (tail = head + duration), so it includes the delay.
                        slides.push(Slide {
                            segments: std::mem::take(&mut cur_segments),
                            slide_duration: (*duration + *delay).max(0.0),
                            slide_start_delay: delay.max(0.0),
                            slide_is_break: *is_break,
                        });
                    }
                    cur_segments.push(SlideSegment {
                        points: chain_pts,
                        shape: simai_pattern_to_shape(*cp),
                    });
                    prev_end = *ce;
                }
                // Flush remaining
                if !cur_segments.is_empty() {
                    slides.push(Slide {
                        segments: cur_segments,
                        slide_duration: (*duration + *delay).max(0.0),
                        slide_start_delay: delay.max(0.0),
                        slide_is_break: *is_break,
                    });
                }
                // Look for a star Tap at the same measure+button to get
                // the star head's break/ex flags.
                let q = quantize(*measure);
                let (sb, se) = chart
                    .notes
                    .iter()
                    .find_map(|m| match m {
                        SimaiNote::Tap {
                            measure: tm,
                            button: tb,
                            is_star: true,
                            is_break: tb_brk,
                            is_ex: tb_ex,
                            ..
                        } if quantize(*tm) == q && *tb == *start => Some((*tb_brk, *tb_ex)),
                        _ => None,
                    })
                    .unwrap_or((false, false));
                notes.push(Note {
                    time: *measure,
                    lane: start + 1,
                    note_type: NoteType::Slide,
                    is_each,
                    is_break: sb,
                    is_ex: se,
                    is_tapless: *is_tapless,
                    hi_speed: *hi_speed,
                    slide: slides,
                    ..Default::default()
                });
            }
            SimaiNote::TouchTap {
                measure,
                region,
                position,
                hi_speed,
                ..
            } => {
                if let Some(lane) = touch_to_lane(*region, *position) {
                    notes.push(Note {
                        time: *measure,
                        lane,
                        note_type: NoteType::Touch,
                        is_each,
                        hi_speed: *hi_speed,
                        ..Default::default()
                    });
                }
            }
            SimaiNote::TouchHold {
                measure,
                region,
                position,
                duration,
                hi_speed,
                ..
            } => {
                if let Some(lane) = touch_to_lane(*region, *position) {
                    notes.push(Note {
                        time: *measure,
                        lane,
                        note_type: NoteType::Hold,
                        hold_duration: duration.max(0.0),
                        is_each,
                        hi_speed: *hi_speed,
                        ..Default::default()
                    });
                }
            }
        }
    }

    notes.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let bpm_changes: Vec<BpmChange> = bpms
        .iter()
        .map(|b| BpmChange {
            measure: b.measure,
            bpm: b.bpm,
        })
        .collect();

    ChartDoc {
        version: "0.3.0-measure".to_string(),
        title: String::new(),
        artist: String::new(),
        simai_level: 0,
        bpm: bpm0,
        bpms: bpm_changes,
        audio_offset: 0.0,
        notes,
        templates: Vec::new(),
        template_instances: Vec::new(),
    }
}

fn ensure_initial_bpm(bpms: &[Bpm], fallback: f32) -> Vec<Bpm> {
    let mut out: Vec<Bpm> = bpms.to_vec();
    out.sort_by(|a, b| {
        a.measure
            .partial_cmp(&b.measure)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if out.first().map(|b| b.measure > 1.0001).unwrap_or(true) {
        out.insert(
            0,
            Bpm {
                measure: 1.0,
                bpm: fallback,
            },
        );
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
pub fn simai_pattern_to_points(
    start: u8,
    end: u8,
    pattern: SlidePattern,
    reflect: Option<u8>,
) -> Vec<SlidePoint> {
    let _s = start + 1; // 1-indexed zone
    let e = end + 1;
    let sp = |z: u8| SlidePoint {
        zone: PadZone::from(z),
        beat_offset: 0.0,
    };
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
            // let cw = ring_cw(s, e);
            // let ccw = ring_ccw(s, e);
            // let route = if cw.len() <= ccw.len() { cw } else { ccw };
            // route.into_iter().map(sp).collect()
            return vec![sp(e)];
        }
        SlidePattern::Right => {
            // > = CCW arc around the outer ring.
            // ring_ccw(s, e).into_iter().map(sp).collect()
            return vec![sp(e)];
        }
        SlidePattern::Left => {
            // < = CW arc around the outer ring.
            // ring_cw(s, e).into_iter().map(sp).collect()
            return vec![sp(e)];
        }
        SlidePattern::LowerV => {
            // V-shape through center.
            vec![sp(17), sp(e)]
        }
        SlidePattern::P => {
            // CW half-circle through inner ring.
            // let mid = ring_cw(s, e);
            // let mut pts: Vec<SlidePoint> = mid.iter()
            //     .take(mid.len().saturating_sub(1))
            //     .map(|&z| sp(a_to_b(z)))
            //     .collect();
            // pts.push(sp(e));
            // pts
            return vec![sp(e)];
        }
        SlidePattern::Q => {
            // CCW half-circle through inner ring.
            // let mid = ring_ccw(s, e);
            // let mut pts: Vec<SlidePoint> = mid.iter()
            //     .take(mid.len().saturating_sub(1))
            //     .map(|&z| sp(a_to_b(z)))
            //     .collect();
            // pts.push(sp(e));
            // pts
            return vec![sp(e)];
        }
        SlidePattern::PP => {
            // Full CW circle (inner ring) then to end.
            // let full = ring_cw_full(s);
            // let mut pts: Vec<SlidePoint> = full.iter().map(|&z| sp(a_to_b(z))).collect();
            // // After the full loop, go from last inner zone outward to end.
            // pts.push(sp(e));
            // pts
            return vec![sp(e)];
        }
        SlidePattern::QQ => {
            // Full CCW circle (inner ring) then to end.
            // let full = ring_ccw_full(s);
            // let mut pts: Vec<SlidePoint> = full.iter().map(|&z| sp(a_to_b(z))).collect();
            // pts.push(sp(e));
            // pts
            return vec![sp(e)];
        }
        SlidePattern::S => {
            // S-curve: CW first half → center → CCW second half.
            // s_curve_waypoints(s, e, false).into_iter().map(sp).collect()
            return vec![sp(e)];
        }
        SlidePattern::Z => {
            // Z-curve: CCW first half → center → CW second half.
            // s_curve_waypoints(s, e, true).into_iter().map(sp).collect()
            return vec![sp(e)];
        }
        SlidePattern::Wifi => {
            // Fan shape — just the endpoint (wifi is visually 3 lanes but
            // stored as individual slides).
            vec![sp(e)]
        }
    }
}

/// A-zone (1-8) → corresponding B-zone (9-16).
fn a_to_b(a: u8) -> u8 {
    a + 8
}

/// CW traversal around the outer ring from `start` to `end` (both 1-indexed),
/// excluding start, including end.
fn ring_cw(start: u8, end: u8) -> Vec<u8> {
    let mut pts = Vec::new();
    let mut cur = start;
    for _ in 0..8 {
        cur = if cur == 8 { 1 } else { cur + 1 };
        pts.push(cur);
        if cur == end {
            break;
        }
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
        if cur == end {
            break;
        }
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
    let lv = if doc.simai_level > 0 {
        doc.simai_level
    } else {
        6
    };
    let mut file = SimaiFile {
        title: doc.title.clone(),
        artist: doc.artist.clone(),
        first: doc.audio_offset,
        levels: vec![(lv, "?".to_string())],
        charts: vec![(lv, chart)],
        wholebpm: Some(doc.bpm),
        source_text: String::new(),
    };
    file.source_text = export_simai_file(&file);
    file
}

pub fn chart_doc_to_simai_chart(doc: &ChartDoc) -> SimaiChart {
    let bpms: Vec<Bpm> = if doc.bpms.is_empty() {
        let bpm0 = if doc.bpm > 0.0 { doc.bpm } else { 120.0 };
        vec![Bpm {
            measure: 1.0,
            bpm: bpm0,
        }]
    } else {
        doc.bpms
            .iter()
            .map(|b| Bpm {
                measure: b.measure,
                bpm: b.bpm,
            })
            .collect()
    };

    let mut notes: Vec<SimaiNote> = Vec::with_capacity(doc.notes.len());
    for n in &doc.notes {
        let measure = snap_measure(n.time);
        // Editor-created notes leave `hi_speed` at its Default (0); export as 1x.
        let hs = if n.hi_speed > 0.0 { n.hi_speed } else { 1.0 };
        match n.note_type {
            NoteType::Tap => {
                if n.lane >= 1 && n.lane <= 8 {
                    notes.push(SimaiNote::Tap {
                        measure,
                        button: n.lane - 1,
                        is_break: n.is_break,
                        is_ex: n.is_ex,
                        is_star: n.is_star,
                        hi_speed: hs,
                    });
                } else {
                    let (region, position) = lane_to_touch(n.lane);
                    notes.push(SimaiNote::TouchTap {
                        measure,
                        region,
                        position,
                        is_firework: false,
                        hi_speed: hs,
                    });
                }
            }
            NoteType::Touch => {
                let (region, position) = lane_to_touch(n.lane);
                notes.push(SimaiNote::TouchTap {
                    measure,
                    region,
                    position,
                    is_firework: false,
                    hi_speed: hs,
                });
            }
            NoteType::Hold => {
                let dur_meas = snap_measure(n.hold_duration.max(0.0));
                if n.lane >= 1 && n.lane <= 8 {
                    notes.push(SimaiNote::Hold {
                        measure,
                        button: n.lane - 1,
                        duration: dur_meas,
                        is_ex: n.is_ex,
                        hi_speed: hs,
                    });
                } else {
                    let (region, position) = lane_to_touch(n.lane);
                    notes.push(SimaiNote::TouchHold {
                        measure,
                        region,
                        position,
                        duration: dur_meas,
                        is_firework: false,
                        hi_speed: hs,
                    });
                }
            }
            NoteType::Slide => {
                if !(n.lane >= 1 && n.lane <= 8) {
                    continue;
                }
                // Emit a star Tap for the slide head (carries star break/ex).
                // Only emit once per (measure, lane) group.
                let already_emitted = notes.iter().any(|sn| {
                    matches!(sn,
                    SimaiNote::Tap { measure: m, button: b, is_star: true, .. }
                        if (*m - measure).abs() < 0.0001 && *b == n.lane - 1)
                });
                if !already_emitted {
                    notes.push(SimaiNote::Tap {
                        measure,
                        button: n.lane - 1,
                        is_break: n.is_break,
                        is_ex: n.is_ex,
                        is_star: true,
                        hi_speed: hs,
                    });
                }
                // Emit one SimaiNote::Slide per Slide in note.slide.
                for sl in &n.slide {
                    // First segment determines the primary pattern and end point.
                    let first_seg = sl.segments.first();
                    let first_shape = first_seg.map(|seg| seg.shape).unwrap_or(SlideShape::Line);
                    let first_pts: Vec<&SlidePoint> = first_seg
                        .map(|seg| seg.points.iter().collect())
                        .unwrap_or_default();
                    let pattern = shape_to_simai_pattern(Some(first_shape));
                    let (reflect, end) = match (pattern, first_pts.as_slice()) {
                        (SlidePattern::BigV, [r, e, ..]) => (
                            Some(r.zone.to_id().saturating_sub(1)),
                            e.zone.to_id().saturating_sub(1),
                        ),
                        (_, [.., last]) => (None, last.zone.to_id().saturating_sub(1)),
                        _ => (None, 0),
                    };
                    // Build chain from additional segments.
                    let chain: Vec<(SlidePattern, u8, Option<u8>, bool)> = sl
                        .segments
                        .iter()
                        .skip(1)
                        .map(|seg| {
                            let cp = shape_to_simai_pattern(Some(seg.shape));
                            let ce = seg
                                .points
                                .last()
                                .map(|p| p.zone.to_id().saturating_sub(1))
                                .unwrap_or(0);
                            let cr = if matches!(cp, SlidePattern::BigV) {
                                seg.points.first().map(|p| p.zone.to_id().saturating_sub(1))
                            } else {
                                None
                            };
                            (cp, ce, cr, false)
                        })
                        .collect();
                    let delay_meas = snap_measure(sl.slide_start_delay.max(0.0));
                    // `slide_duration` is the total span from the head, so the
                    // Simai travel `duration` is total minus the delay.
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
                        hi_speed: hs,
                    });
                }
            }
        }
    }

    SimaiChart { notes, bpms }
}

pub fn parse_simai_source(text: &str) -> Result<SimaiFile, String> {
    if text.contains("&inote_") || text.contains("&title=") {
        parse_maidata_source(text)
    } else {
        let source_text = format!("&first=0\n&inote_0={}\n", text.trim());
        Ok(SimaiFile {
            levels: vec![(0, "Lv.0".to_string())],
            source_text,
            ..Default::default()
        })
    }
}

fn parse_maidata_source(text: &str) -> Result<SimaiFile, String> {
    let mut title = String::new();
    let mut artist = String::new();
    let mut first = 0.0;
    let mut wholebpm = None;
    let mut level_labels = std::collections::HashMap::<u32, String>::new();
    let mut chart_levels = Vec::<u32>::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index].trim();
        if line.is_empty() || !line.starts_with('&') {
            index += 1;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            index += 1;
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "&title" {
            title = value.to_string();
        } else if key == "&artist" {
            artist = value.to_string();
        } else if key == "&first" {
            first = value.parse::<f32>().unwrap_or(0.0);
        } else if key == "&wholebpm" {
            wholebpm = value.parse::<f32>().ok();
        } else if let Some(level_text) = key.strip_prefix("&lv_") {
            if let Ok(level) = level_text.parse::<u32>() {
                level_labels.insert(level, value.to_string());
            }
        } else if let Some(level_text) = key.strip_prefix("&inote_") {
            if let Ok(level) = level_text.parse::<u32>() {
                chart_levels.push(level);
            }
        }
        index += 1;
    }

    chart_levels.sort_unstable();
    chart_levels.dedup();
    if chart_levels.is_empty() {
        return Err("Simai file contains no &inote_N= charts".to_string());
    }

    let levels = chart_levels
        .into_iter()
        .map(|level| {
            let label = level_labels
                .remove(&level)
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| format!("Lv.{level}"));
            (level, label)
        })
        .collect();

    Ok(SimaiFile {
        title,
        artist,
        first,
        levels,
        charts: Vec::new(),
        wholebpm,
        source_text: text.to_string(),
    })
}

// ─── Simai export ────────────────────────────────────────────────────────

pub fn export_simai_file(file: &SimaiFile) -> String {
    if file.charts.is_empty() && !file.source_text.is_empty() {
        return file.source_text.clone();
    }

    let mut s = String::new();
    s.push_str(&format!("&title={}\n", file.title));
    s.push_str(&format!("&artist={}\n", file.artist));
    s.push_str(&format!("&first={}\n", trim_float(file.first)));
    if let Some(b) = file.wholebpm {
        s.push_str(&format!("&wholebpm={}\n", trim_float(b)));
    }
    for (n, lv) in &file.levels {
        s.push_str(&format!("&lv_{n}={lv}\n"));
    }
    for (n, chart) in &file.charts {
        s.push_str(&format!("&inote_{n}={}\n", export_chart(chart)));
    }
    s
}

fn export_chart(chart: &SimaiChart) -> String {
    export_chart_with(chart, 1000)
}

fn export_chart_with(chart: &SimaiChart, max_den: u32) -> String {
    let mut measures: Vec<f32> = Vec::new();
    for n in &chart.notes {
        measures.push(n.measure());
    }
    for b in &chart.bpms {
        measures.push(b.measure);
    }
    let mut whole_set: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    for m in &measures {
        whole_set.insert(m.floor() as i64);
    }
    for w in whole_set {
        measures.push(w as f32);
    }
    measures.push(1.0);
    measures.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    measures.dedup_by(|a, b| (*a - *b).abs() < 1e-5);

    let last_whole = measures.iter().fold(1, |acc, m| acc.max(m.floor() as i64));
    let mut whole_divisors: Vec<Option<u32>> = Vec::with_capacity((last_whole + 1) as usize);
    for w in 0..=last_whole {
        let in_w: Vec<f32> = measures
            .iter()
            .copied()
            .filter(|m| m.floor() as i64 == w)
            .collect();
        whole_divisors.push(measure_divisor(&in_w, max_den));
    }

    let mut last_measure = 1.0_f32;
    let mut measure_tick = 1.0_f32;
    let mut prev_div: Option<u32> = None;
    let mut prev_measure_int: i64 = 0;
    let mut out = String::new();

    let n_measures = measures.len();
    for (i, &cur) in measures.iter().enumerate() {
        let bpm_here: Vec<&Bpm> = chart
            .bpms
            .iter()
            .filter(|b| (b.measure - cur).abs() < 1e-4)
            .collect();
        let notes_here: Vec<&SimaiNote> = chart
            .notes
            .iter()
            .filter(|n| (n.measure() - cur).abs() < 1e-4)
            .collect();

        for n in &notes_here {
            match n {
                SimaiNote::Hold { duration, .. } | SimaiNote::TouchHold { duration, .. } => {
                    last_measure = last_measure.max(cur + *duration);
                }
                SimaiNote::Slide {
                    duration, delay, ..
                } => {
                    last_measure = last_measure.max(cur + *delay + *duration);
                }
                _ => {}
            }
        }

        let whole_div = whole_divisors.get(cur.floor() as usize).copied().flatten();

        let (whole, cur_div, rest_amount);
        if i == n_measures - 1 {
            if last_measure > cur {
                let r = compute_rest(cur, last_measure, None, prev_div.or(whole_div), max_den);
                whole = r.0;
                cur_div = r.1;
                rest_amount = r.2;
            } else {
                whole = 0;
                cur_div = prev_div.or(whole_div).unwrap_or(4);
                rest_amount = 0;
            }
        } else {
            let next_m = measures[i + 1];
            let after = if i + 2 < n_measures {
                Some(measures[i + 2])
            } else {
                None
            };
            let r = compute_rest(cur, next_m, after, prev_div.or(whole_div), max_den);
            whole = r.0;
            cur_div = r.1;
            rest_amount = r.2;
        }

        let bpm_at_next = bpm_value_at(chart, cur + 1.0);
        let frag = render_fragment(&notes_here, bpm_at_next, max_den);
        let bpm_prefix = if let Some(b) = bpm_here.first() {
            format!("({})", trim_float(b.bpm))
        } else {
            String::new()
        };

        if prev_div != Some(cur_div) || (measure_tick.floor() as i64) > prev_measure_int {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&bpm_prefix);
            out.push_str(&format!("{{{cur_div}}}"));
            out.push_str(&frag);
            prev_div = Some(cur_div);
            prev_measure_int = measure_tick.floor() as i64;
        } else {
            out.push_str(&bpm_prefix);
            out.push_str(&frag);
        }
        measure_tick = cur;

        for _ in 0..rest_amount {
            out.push(',');
            measure_tick += 1.0 / cur_div as f32;
        }
        if whole > 0 {
            if cur_div != 1 {
                out.push_str("{1}");
                prev_div = Some(1);
            }
            for _ in 0..whole {
                out.push(',');
                measure_tick += 1.0;
            }
        }
        measure_tick = (measure_tick * 10000.0).round() / 10000.0;
    }

    out.push_str(",\nE\n");
    out
}

fn render_fragment(notes: &[&SimaiNote], bpm_for_slides: f32, max_den: u32) -> String {
    let mut out = String::new();
    let mut counter = 0u32;
    let mut star_positions: Vec<u8> = Vec::new();

    let taps: Vec<&SimaiNote> = notes
        .iter()
        .copied()
        .filter(|n| matches!(n, SimaiNote::Tap { .. }))
        .collect();
    let holds: Vec<&SimaiNote> = notes
        .iter()
        .copied()
        .filter(|n| matches!(n, SimaiNote::Hold { .. }))
        .collect();
    let touch_taps: Vec<&SimaiNote> = notes
        .iter()
        .copied()
        .filter(|n| matches!(n, SimaiNote::TouchTap { .. }))
        .collect();
    let touch_holds: Vec<&SimaiNote> = notes
        .iter()
        .copied()
        .filter(|n| matches!(n, SimaiNote::TouchHold { .. }))
        .collect();
    let slides: Vec<&SimaiNote> = notes
        .iter()
        .copied()
        .filter(|n| matches!(n, SimaiNote::Slide { .. }))
        .collect();

    for n in &taps {
        if let SimaiNote::Tap {
            button,
            is_break,
            is_ex,
            is_star,
            ..
        } = n
        {
            if *is_star
                && slides
                    .iter()
                    .any(|s| matches!(s, SimaiNote::Slide { start, .. } if start == button))
            {
                star_positions.push(*button);
                continue;
            }
            if counter > 0 {
                out.push('/');
            }
            let mut mods = String::new();
            if *is_break {
                mods.push('b');
            }
            if *is_ex {
                mods.push('x');
            }
            if *is_star {
                mods.push('$');
            }
            out.push_str(&format!("{}{}", button + 1, mods));
            counter += 1;
        }
    }

    for n in &holds {
        if let SimaiNote::Hold {
            button,
            duration,
            is_ex,
            ..
        } = n
        {
            if counter > 0 {
                out.push('/');
            }
            let mods = if *is_ex { "hx" } else { "h" };
            let (den, num) = float_to_fraction(*duration, max_den * 2);
            out.push_str(&format!("{}{}[{}:{}]", button + 1, mods, den, num));
            counter += 1;
        }
    }

    for n in &touch_taps {
        if let SimaiNote::TouchTap {
            region,
            position,
            is_firework,
            ..
        } = n
        {
            if counter > 0 {
                out.push('/');
            }
            let modf = if *is_firework { "f" } else { "" };
            if *region == 'C' {
                out.push_str(&format!("C{modf}"));
            } else {
                out.push_str(&format!("{}{}{}", region, position + 1, modf));
            }
            counter += 1;
        }
    }

    for n in &touch_holds {
        if let SimaiNote::TouchHold {
            region,
            position,
            duration,
            is_firework,
            ..
        } = n
        {
            if counter > 0 {
                out.push('/');
            }
            let mods = if *is_firework { "hf" } else { "h" };
            let (den, num) = float_to_fraction(*duration, max_den * 2);
            if *region == 'C' {
                out.push_str(&format!("C{mods}[{den}:{num}]"));
            } else {
                out.push_str(&format!("{}{}{}[{}:{}]", region, position + 1, mods, den, num));
            }
            counter += 1;
        }
    }

    let mut written_starts: Vec<u8> = Vec::new();
    for n in &slides {
        if let SimaiNote::Slide {
            start,
            end,
            pattern,
            reflect,
            duration,
            delay,
            is_break,
            is_ex,
            is_tapless,
            chain,
            ..
        } = n
        {
            if counter > 0 && !written_starts.contains(start) {
                out.push('/');
            }
            let head = if written_starts.contains(start) {
                "*".to_string()
            } else {
                format!("{}", start + 1)
            };
            let mut mods = String::new();
            if !written_starts.contains(start) {
                if *is_tapless && !star_positions.contains(start) {
                    mods.push('?');
                } else if *is_break {
                    mods.push('b');
                } else if *is_ex {
                    mods.push('x');
                }
            }
            let pat_str = match pattern {
                SlidePattern::BigV => reflect
                    .map(|r| format!("V{}", r + 1))
                    .unwrap_or_else(|| "V".to_string()),
                _ => pattern.as_str().to_string(),
            };
            let chain_str: String = chain
                .iter()
                .map(|(cp, ce, cr, _is_star)| {
                    let cp_str = match cp {
                        SlidePattern::BigV => cr
                            .map(|r| format!("V{}", r + 1))
                            .unwrap_or_else(|| "V".to_string()),
                        _ => cp.as_str().to_string(),
                    };
                    format!("{cp_str}{}", ce + 1)
                })
                .collect();
            let suffix = if (delay - 0.25).abs() > 0.0025 {
                let scale = if *delay > 0.0025 {
                    0.25 / *delay
                } else {
                    100.0
                };
                let eq_bpm = ((bpm_for_slides * scale) * 10000.0).round() / 10000.0;
                let (den, num) = float_to_fraction(*duration * scale, max_den * 10);
                format!("[{}#{}:{}]", trim_float(eq_bpm), den, num)
            } else {
                let (den, num) = float_to_fraction(*duration, max_den * 10);
                format!("[{den}:{num}]")
            };
            out.push_str(&format!("{head}{mods}{pat_str}{}{chain_str}{suffix}", end + 1));
            counter += 1;
            if !written_starts.contains(start) {
                written_starts.push(*start);
            }
        }
    }

    out
}

fn bpm_value_at(chart: &SimaiChart, measure: f32) -> f32 {
    let mut last = if let Some(b) = chart.bpms.first() {
        b.bpm
    } else {
        120.0
    };
    let mut sorted: Vec<&Bpm> = chart.bpms.iter().collect();
    sorted.sort_by(|a, b| {
        a.measure
            .partial_cmp(&b.measure)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for b in sorted {
        if b.measure > measure + 1e-4 {
            break;
        }
        last = b.bpm;
    }
    last
}

fn trim_float(v: f32) -> String {
    let mut s = format!("{v:.4}");
    while s.contains('.') && (s.ends_with('0') || s.ends_with('.')) {
        s.pop();
    }
    s
}

fn gcd_u32(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd_u32(b, a % b) }
}

fn lcm_u32(a: u32, b: u32) -> u32 {
    if a == 0 || b == 0 {
        0
    } else {
        a / gcd_u32(a, b) * b
    }
}

fn float_to_fraction(value: f32, max_den: u32) -> (u32, u32) {
    if !value.is_finite() || value < 0.0 {
        return (1, 0);
    }
    if value == 0.0 {
        return (1, 0);
    }
    let mut h0: i64 = 0;
    let mut h1: i64 = 1;
    let mut k0: i64 = 1;
    let mut k1: i64 = 0;
    let mut x = value as f64;
    for _ in 0..32 {
        let a = x.floor() as i64;
        let h2 = a * h1 + h0;
        let k2 = a * k1 + k0;
        if k2 > max_den as i64 {
            break;
        }
        h0 = h1;
        h1 = h2;
        k0 = k1;
        k1 = k2;
        let frac = x - a as f64;
        if frac.abs() < 1e-9 {
            break;
        }
        x = 1.0 / frac;
    }
    let den = k1.max(1) as u32;
    let num = h1.max(0) as u32;
    let g = gcd_u32(den, num.max(1));
    (den / g.max(1), num / g.max(1))
}

fn measure_divisor(measures: &[f32], max_den: u32) -> Option<u32> {
    if measures.is_empty() {
        return None;
    }
    let base = measures[0].floor();
    let mut prev = base;
    let mut sorted = measures.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut current_lcm: u32 = 1;
    for m in sorted {
        let frac = (m - prev).fract().abs();
        let (d, _n) = float_to_fraction(frac, max_den);
        current_lcm = lcm_u32(current_lcm, d);
        if current_lcm > 64 {
            return None;
        }
        prev = m;
    }
    Some(current_lcm)
}

fn compute_rest(
    cur: f32,
    next: f32,
    after_next: Option<f32>,
    cur_divisor: Option<u32>,
    max_den: u32,
) -> (i32, u32, i32) {
    if next < cur {
        return (0, cur_divisor.unwrap_or(4), 0);
    }
    let diff = next - cur;
    if diff < 1e-5 {
        return (0, cur_divisor.unwrap_or(4), 0);
    }
    let whole = diff.floor() as i32;
    let frac = diff - whole as f32;
    let (frac_den, frac_num) = float_to_fraction(frac, max_den);

    if let Some(cd) = cur_divisor {
        let l = lcm_u32(cd, frac_den);
        if l == cd && diff < 1.0 {
            let amount = (frac * cd as f32).round() as i32;
            return (0, cd, amount);
        }
    }

    if let Some(an) = after_next {
        if an >= next {
            let diff2 = an - next;
            let frac2 = diff2 - diff2.floor();
            let (d2, _) = float_to_fraction(frac2, max_den);
            let l = lcm_u32(frac_den, d2);
            if l <= 64 && diff < 1.0 {
                let amount = (frac * l as f32).round() as i32;
                return (0, l, amount);
            }
        }
    }

    (whole, frac_den.max(1), frac_num as i32)
}

// ──────────────────────────────── I/O helpers ─────────────────────────────

/// Read a Simai file from `<output>/<name>` and convert it to a ChartDoc.
pub fn import_from_simai_path(name: &str) -> Result<ChartDoc, String> {
    let text = platform::read_output_text(name)?;
    let parsed = parse_simai_source(&text)?;
    simai_file_to_chart_doc(&parsed, None)
}

/// Export a ChartDoc as a Simai maidata.txt-style file under `<output>/<name>`.
pub fn export_to_simai_path(doc: &ChartDoc, name: &str) -> Result<PathBuf, String> {
    let file = chart_doc_to_simai_file(doc);
    let text = export_simai_file(&file);
    platform::write_output_text(name, &text)
}

// ─────────────────────── Dialog-based import ──────────────────────────

/// Result of a dialog-based chart import.
#[derive(Clone)]
pub struct DialogImport {
    pub chart: ChartDoc,
    pub title: String,
    pub audio_bytes: Option<Vec<u8>>,
    pub audio_ext: Option<String>,
    /// Available difficulty levels: (level_number, display_text).
    pub levels: Vec<(u32, String)>,
    /// Parsed file for level switching without re-reading disk.
    pub simai_file: SimaiFile,
    /// Directory containing the source `maidata.txt` (for copying into the
    /// song library), when the import came from a file path.
    pub source_dir: Option<PathBuf>,
}

/// Open native file dialog to import a chart and its background music.
pub fn dialog_import() -> Result<DialogImport, String> {
    let path_str = native_open_file_dialog().ok_or_else(|| "cancelled".to_string())?;
    import_from_file_path(&path_str)
}

/// Import chart from a file path. Audio is auto-detected in the same directory.
pub fn import_from_file_path(path_str: &str) -> Result<DialogImport, String> {
    let file = PathBuf::from(path_str);
    let base_dir = file.parent().map(|p| p.to_path_buf());
    let chart_text = std::fs::read_to_string(&file).map_err(|e| format!("read file: {e}"))?;

    let parsed = parse_simai_source(&chart_text)?;

    let chart =
        simai_file_to_chart_doc(&parsed, None).map_err(|e| format!("chart convert: {e}"))?;
    let title = chart.title.clone();

    // Build level list from charts + level labels
    let mut levels: Vec<(u32, String)> = parsed
        .charts
        .iter()
        .map(|(lv, _)| {
            let label = parsed
                .levels
                .iter()
                .find(|(n, _)| n == lv)
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| format!("Lv.{lv}"));
            (*lv, label)
        })
        .collect();
    levels.sort_by_key(|(lv, _)| *lv);

    let (audio_bytes, audio_ext) = if let Some(dir) = base_dir.clone() {
        let result = ["track.mp3", "track.wav", "music.mp3", "music.wav"]
            .iter()
            .find_map(|name| {
                let path = dir.join(name);
                match std::fs::read(&path) {
                    Ok(b) => {
                        println!(
                            "[dialog_import] found audio: {} ({} bytes)",
                            path.display(),
                            b.len()
                        );
                        Some((b, name.rsplit('.').next().unwrap_or("").to_string()))
                    }
                    Err(_) => None,
                }
            });
        if result.is_none() {
            println!("[dialog_import] no audio found in {:?}", dir);
        }
        result
    } else {
        None
    }
    .unzip();

    Ok(DialogImport {
        chart,
        title,
        audio_bytes,
        audio_ext,
        levels,
        simai_file: parsed,
        source_dir: base_dir,
    })
}

/// Re-convert a specific level from an already-parsed Simai file.
pub fn convert_simai_level(simai_file: &SimaiFile, level: u32) -> Result<ChartDoc, String> {
    simai_file_to_chart_doc(simai_file, Some(level))
}

#[cfg(target_os = "macos")]
fn native_open_file_dialog() -> Option<String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"POSIX path of (choose file with prompt "Open Chart" default location (path to desktop))"#)
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    println!(
        "[dialog] osascript stdout={stdout:?} stderr={stderr:?} status={}",
        output.status
    );
    if output.status.success() && !stdout.is_empty() {
        Some(stdout)
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn native_open_file_dialog() -> Option<String> {
    tinyfiledialogs::open_file_dialog("Open Chart", "", Some((&["*.txt", "*.map"], "Chart files")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide_timing(text: &str) -> (f32, f32) {
        let parsed = parse_simai_source(text).expect("parse source");
        let doc = simai_file_to_chart_doc(&parsed, Some(0)).expect("parse with lnmai-core");
        let sl = &doc
            .notes
            .iter()
            .find(|n| matches!(n.note_type, super::super::types::NoteType::Slide))
            .expect("slide note")
            .slide[0];
        (sl.slide_duration, sl.slide_start_delay)
    }

    #[test]
    fn slide_import_preserves_travel_duration_with_no_delay() {
        // `[8:3]` is 3/8 of a measure of travel. Lean applies the standard
        // one-beat slide wait when no explicit wait is provided.
        let (dur, delay) = slide_timing("(120){4}1-5[8:3],E");
        assert!((dur - 0.625).abs() < 0.001);
        assert!((delay - 0.25).abs() < 0.001);
    }

    #[test]
    fn slide_import_keeps_explicit_delay_from_seconds_form() {
        // `[0.2##0.8]` → 0.2s delay, 0.8s travel at 120 BPM → 0.1 / 0.4
        // measures. `slide_duration` is the total span from the head, so it
        // includes the delay (0.1 + 0.4 = 0.5).
        let (dur, delay) = slide_timing("(120){4}1-5[0.2##0.8],E");
        assert!((dur - 0.5).abs() < 0.001, "total: {}", dur);
        assert!((delay - 0.1).abs() < 0.001, "delay: {}", delay);
    }

    #[test]
    fn slide_timing_round_trips_through_export() {
        let parsed = parse_simai_source("(120){4}1-5[8:3],E").expect("parse source");
        let doc = simai_file_to_chart_doc(&parsed, Some(0)).expect("parse with lnmai-core");
        let exported = export_simai_file(&chart_doc_to_simai_file(&doc));
        let reparsed = parse_simai_source(&exported).expect("parse exported source");
        let round_tripped =
            simai_file_to_chart_doc(&reparsed, Some(doc.simai_level)).expect("reparse exported");
        let sl = &round_tripped
            .notes
            .iter()
            .find(|n| matches!(n.note_type, super::super::types::NoteType::Slide))
            .expect("slide note")
            .slide[0];
        assert!(
            (sl.slide_duration - 0.625).abs() < 0.001,
            "duration: {}",
            sl.slide_duration
        );
        assert!(
            (sl.slide_start_delay - 0.25).abs() < 0.001,
            "delay: {}",
            sl.slide_start_delay
        );
    }
}
