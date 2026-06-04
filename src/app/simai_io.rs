//! Bridge between the in-memory `ChartDoc` model and the `lnmai-core-rs` crate.
//!
//! Provides:
//!   * `import_from_simai_text` — top-level helper used by the UI to parse Simai text.
//!   * `export_to_simai_text` — best-effort conversion from ChartDoc to Simai text.
//!
//! Uses lnmai-core-rs session API for parsing.

use std::path::PathBuf;

use lnmai_core_rs::session::{Session, Empty};

use super::platform;
use super::types::{ChartDoc, Note, NoteType, Slide, SlideSegment, SlidePoint, SlideShape, BpmChange};
use super::types::zone::PadZone;

/// Read a Simai file from `<output>/<name>` and convert it to a ChartDoc
/// using lnmai-core-rs session API.
pub fn import_from_simai_path(name: &str) -> Result<ChartDoc, String> {
    let text = platform::read_output_text(name)?;
    import_from_simai_text(&text)
}

/// Parse Simai text and convert it to a ChartDoc using lnmai-core-rs session API.
pub fn import_from_simai_text(text: &str) -> Result<ChartDoc, String> {
    // Create a new lnmai session
    let empty = Session::<Empty>::create()
        .map_err(|e| format!("lnmai create session failed: {}", e.json))?;

    // Load the simai text into the session
    let (loaded, _info) = empty.load_chart_text(text, 6)
        .map_err(|e| format!("lnmai load chart failed: {}", e.json))?;

    // Get the lowered chart JSON
    let envelope = loaded.get_lowered_chart_json()
        .map_err(|e| format!("lnmai get lowered chart failed: {}", e.json))?;

    // Parse the JSON to extract note information
    let json_val: serde_json::Value = serde_json::from_str(&envelope.json)
        .map_err(|e| format!("Failed to parse lnmai JSON: {e}"))?;

    let data = json_val.get("result").unwrap_or(&json_val);

    // Create a basic ChartDoc
    let mut doc = ChartDoc {
        version: "0.3.0-measure".to_string(),
        title: "Imported Simai".to_string(),
        bpm: 120.0,
        bpms: Vec::new(),
        audio_offset: 0.0,
        notes: Vec::new(),
        templates: Vec::new(),
        template_instances: Vec::new(),
    };

    // Extract taps
    if let Some(taps) = data.get("taps").and_then(|v| v.as_array()) {
        for tap in taps {
            if let Some(slot) = tap.get("slot").and_then(|v| v.as_str()) {
                let lane = parse_slot_to_lane(slot);
                let timing = tap.get("timing").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let is_break = tap.get("isBreak").and_then(|v| v.as_bool()).unwrap_or(false);
                let is_ex = tap.get("isEX").and_then(|v| v.as_bool()).unwrap_or(false);

                doc.notes.push(Note {
                    time: timing_to_measure(timing),
                    lane,
                    is_break,
                    is_ex,
                    ..Default::default()
                });
            }
        }
    }

    // Extract holds
    if let Some(holds) = data.get("holds").and_then(|v| v.as_array()) {
        for hold in holds {
            if let Some(slot) = hold.get("slot").and_then(|v| v.as_str()) {
                let lane = parse_slot_to_lane(slot);
                let timing = hold.get("timing").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let length = hold.get("length").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

                doc.notes.push(Note {
                    time: timing_to_measure(timing),
                    lane,
                    note_type: NoteType::Hold,
                    hold_duration: timing_to_measure(length),
                    ..Default::default()
                });
            }
        }
    }

    // Extract slides
    if let Some(slides) = data.get("slides").and_then(|v| v.as_array()) {
        for slide in slides {
            if let Some(slot) = slide.get("slot").and_then(|v| v.as_str()) {
                let lane = parse_slot_to_lane(slot);
                let timing = slide.get("timing").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let length = slide.get("length").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let start_timing = slide.get("startTiming").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

                // Create a basic slide segment
                let segment = SlideSegment {
                    points: vec![SlidePoint { zone: PadZone::A1, beat_offset: 0.0 }],
                    shape: SlideShape::Line,
                };

                doc.notes.push(Note {
                    time: timing_to_measure(timing),
                    lane,
                    note_type: NoteType::Slide,
                    slide: vec![Slide {
                        segments: vec![segment],
                        slide_duration: timing_to_measure(length),
                        slide_start_delay: timing_to_measure(start_timing - timing),
                        slide_is_break: false,
                    }],
                    ..Default::default()
                });
            }
        }
    }

    // Extract BPM from the first note's timing
    if let Some(first_tap) = data.get("taps").and_then(|v| v.as_array()).and_then(|a| a.first()) {
        if let Some(bpm) = first_tap.get("bpm").and_then(|v| v.as_f64()) {
            doc.bpm = bpm as f32;
        }
    }

    // Sort notes by time
    doc.notes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));

    // Free the session
    let _ = loaded.free();

    Ok(doc)
}

/// Parse a slot string (e.g., "S1", "S2") to a lane number (1-8)
fn parse_slot_to_lane(slot: &str) -> u8 {
    if let Some(num) = slot.strip_prefix('S').and_then(|s| s.parse::<u8>().ok()) {
        num.clamp(1, 8)
    } else {
        1
    }
}

/// Convert timing in microseconds to measure (assuming 120 BPM, 4/4 time)
fn timing_to_measure(timing: f32) -> f32 {
    // timing is in microseconds, convert to measures
    // Assuming 120 BPM: 1 beat = 0.5 seconds = 500000 microseconds
    // 1 measure = 4 beats = 2000000 microseconds
    timing / 2000000.0
}

/// Export a ChartDoc as a Simai maidata.txt-style file under `<output>/<name>`.
/// Note: This is a simplified export that creates a basic Simai format.
pub fn export_to_simai_path(doc: &ChartDoc, name: &str) -> Result<PathBuf, String> {
    let text = chart_doc_to_simai_text(doc);
    platform::write_output_text(name, &text)
}

/// Convert a ChartDoc to Simai text format (simplified).
pub fn chart_doc_to_simai_text(doc: &ChartDoc) -> String {
    let mut output = String::new();
    
    // Header
    output.push_str(&format!("&title={}\n", doc.title));
    output.push_str(&format!("&first={}\n", doc.audio_offset));
    
    // BPM
    if !doc.bpms.is_empty() {
        output.push_str(&format!("&wholebpm={}\n", doc.bpm));
    }
    
    // Chart level
    output.push_str("&lv_6=13+\n");
    
    // Notes
    output.push_str("&inote_6=\n");
    
    let mut last_measure = 0.0;
    for note in &doc.notes {
        let measure = note.time;
        
        // Add comma separators for empty measures
        while last_measure < measure - 0.001 {
            output.push(',');
            last_measure += 0.25; // Assuming 4/4 time
        }
        
        match note.note_type {
            NoteType::Tap | NoteType::Touch => {
                output.push_str(&format!("{}", note.lane));
            }
            NoteType::Hold => {
                output.push_str(&format!("{}h", note.lane));
                // Add duration in brackets
                let beats = (note.hold_duration * 4.0).round() as u32;
                if beats > 0 {
                    output.push_str(&format!("[{}:{}]", 4, beats));
                }
            }
            NoteType::Slide => {
                if let Some(slide) = note.slide.first() {
                    // Simplified slide representation
                    output.push_str(&format!("{}-", note.lane));
                    if let Some(seg) = slide.segments.first() {
                        if let Some(last_point) = seg.points.last() {
                            let end_lane = last_point.zone.to_id().clamp(1, 8);
                            output.push_str(&format!("{}", end_lane));
                        }
                    }
                    // Add duration
                    let beats = (slide.slide_duration * 4.0).round() as u32;
                    if beats > 0 {
                        output.push_str(&format!("[{}:{}]", 4, beats));
                    }
                }
            }
        }
        
        last_measure = measure;
    }
    
    output.push_str(",\nE\n");
    
    output
}

/// Convert a SlideShape to a string representation for Simai export.
pub fn slide_shape_to_string(shape: SlideShape) -> &'static str {
    match shape {
        SlideShape::Line => "-",
        SlideShape::Caret => "^",
        SlideShape::Left => "<",
        SlideShape::Right => ">",
        SlideShape::VShape => "v",
        SlideShape::P => "p",
        SlideShape::Q => "q",
        SlideShape::S => "s",
        SlideShape::Z => "z",
        SlideShape::PP => "pp",
        SlideShape::QQ => "qq",
        SlideShape::BigV => "V",
        SlideShape::Wifi => "w",
    }
}

/// Validate a Simai text format (basic validation).
pub fn validate_simai_text(text: &str) -> Result<(), String> {
    // Basic validation: check for required fields
    if !text.contains("&inote_") && !text.contains("1") && !text.contains("2") {
        return Err("Simai文本格式无效：缺少音符数据".to_string());
    }
    Ok(())
}

/// Slide pattern type for generating waypoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlidePatternType {
    Line,
    Caret,
    Left,
    Right,
    VShape,
    BigV,
    S,
    Z,
    P,
    Q,
    PP,
    QQ,
    Wifi,
}

/// Convert a SlideShape to a SlidePatternType.
pub fn shape_to_slide_pattern(shape: SlideShape) -> SlidePatternType {
    match shape {
        SlideShape::Line => SlidePatternType::Line,
        SlideShape::Caret => SlidePatternType::Caret,
        SlideShape::Left => SlidePatternType::Left,
        SlideShape::Right => SlidePatternType::Right,
        SlideShape::VShape => SlidePatternType::VShape,
        SlideShape::BigV => SlidePatternType::BigV,
        SlideShape::S => SlidePatternType::S,
        SlideShape::Z => SlidePatternType::Z,
        SlideShape::P => SlidePatternType::P,
        SlideShape::Q => SlidePatternType::Q,
        SlideShape::PP => SlidePatternType::PP,
        SlideShape::QQ => SlidePatternType::QQ,
        SlideShape::Wifi => SlidePatternType::Wifi,
    }
}

/// Generate slide waypoints for a given pattern.
/// start and end are 0-indexed A-zone buttons (0..=7).
/// Returns a list of waypoint zones (as PadZone).
pub fn generate_slide_points(start: u8, end: u8, pattern: SlidePatternType, reflect: Option<u8>,) -> Vec<SlidePoint> {
    let s = start + 1; // Convert to 1-indexed
    let e = end + 1;
    let sp = |z: u8| SlidePoint { zone: PadZone::from(z), beat_offset: 0.0 };

    match pattern {
        SlidePatternType::Line => {
            // Straight line through center; endpoint only.
            vec![sp(e)]
        }
        SlidePatternType::BigV => {
            let mut pts = Vec::new();
            if let Some(r) = reflect {
                pts.push(sp(r + 1));
            }
            pts.push(sp(e));
            pts
        }
        SlidePatternType::Caret => {
            // Shorter arc around the outer ring.
            let cw = ring_cw(s, e);
            let ccw = ring_ccw(s, e);
            let route = if cw.len() <= ccw.len() { cw } else { ccw };
            route.into_iter().map(sp).collect()
        }
        SlidePatternType::Right => {
            // > = CCW arc around the outer ring.
            ring_ccw(s, e).into_iter().map(sp).collect()
        }
        SlidePatternType::Left => {
            // < = CW arc around the outer ring.
            ring_cw(s, e).into_iter().map(sp).collect()
        }
        SlidePatternType::VShape => {
            // V-shape through center.
            vec![sp(17), sp(e)]
        }
        SlidePatternType::P => {
            // CW half-circle through inner ring.
            let mid = ring_cw(s, e);
            let mut pts: Vec<SlidePoint> = mid.iter()
                .take(mid.len().saturating_sub(1))
                .map(|&z| sp(a_to_b(z)))
                .collect();
            pts.push(sp(e));
            pts
        }
        SlidePatternType::Q => {
            // CCW half-circle through inner ring.
            let mid = ring_ccw(s, e);
            let mut pts: Vec<SlidePoint> = mid.iter()
                .take(mid.len().saturating_sub(1))
                .map(|&z| sp(a_to_b(z)))
                .collect();
            pts.push(sp(e));
            pts
        }
        SlidePatternType::PP => {
            // Full CW circle (inner ring) then to end.
            let full = ring_cw_full(s);
            let mut pts: Vec<SlidePoint> = full.iter().map(|&z| sp(a_to_b(z))).collect();
            // After the full loop, go from last inner zone outward to end.
            pts.push(sp(e));
            pts
        }
        SlidePatternType::QQ => {
            // Full CCW circle (inner ring) then to end.
            let full = ring_ccw_full(s);
            let mut pts: Vec<SlidePoint> = full.iter().map(|&z| sp(a_to_b(z))).collect();
            pts.push(sp(e));
            pts
        }
        SlidePatternType::S => {
            // S-curve: CW first half → center → CCW second half.
            s_curve_waypoints(s, e, false).into_iter().map(sp).collect()
        }
        SlidePatternType::Z => {
            // Z-curve: CCW first half → center → CW second half.
            s_curve_waypoints(s, e, true).into_iter().map(sp).collect()
        }
        SlidePatternType::Wifi => {
            // Fan shape — just the endpoint (wifi is visually 3 lanes but
            // stored as individual slides).
            vec![sp(e)]
        }
    }
}

/// Generate slide points and return as SlidePoint vector.
pub fn simai_pattern_to_points(start: u8, end: u8, pattern: SlidePatternType, reflect: Option<u8>) -> Vec<SlidePoint> {
    let zones = generate_slide_points(start, end, pattern,reflect);
    zones
}

/// Helper: A-zone (1-8) → corresponding B-zone (9-16).
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
