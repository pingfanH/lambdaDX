//! Format C serialization: measure + beat + division + offset.
//!
//! Internal representation stays as float measures; this module handles
//! conversion to/from the beat-based JSON format for save/load.

use super::types::{BpmChange, ChartDoc, HitEvent, Note, NoteType, RecordingDoc, Slide};
use serde::{Deserialize, Serialize};

const TICKS_PER_MEASURE: i32 = 384;
const TICKS_PER_BEAT: i32 = 96; // 4/4 time: 384/4

// ─── Serialization structs ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerChartDoc {
    pub version: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub simai_level: u32,
    pub bpm: f32,
    #[serde(default)]
    pub bpms: Vec<BpmChange>,
    #[serde(default)]
    pub audio_offset: f32,
    pub notes: Vec<SerNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerNote {
    pub measure: i32,
    pub beat: i32,
    #[serde(default = "default_division")]
    pub division: i32,
    #[serde(default)]
    pub offset: i32,
    pub lane: u8,
    pub note_type: NoteType,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_each: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_break: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_ex: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_star: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_tapless: bool,
    /// Hold duration as [numerator, denominator] in beats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_duration: Option<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slide: Vec<Slide>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerRecordingDoc {
    pub created_at_epoch_ms: u128,
    pub source: String,
    pub chart: SerChartDoc,
    pub hits: Vec<HitEvent>,
    pub record_speed: f32,
    pub play_speed: f32,
}

fn default_division() -> i32 {
    1
}

// ─── Conversion: Internal → Serialized ──────────────────────────────

fn gcd(mut a: i32, mut b: i32) -> i32 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Convert internal measure float → (measure, beat, division, offset).
/// measure 1-indexed, beat 1-indexed (1..=4 for 4/4).
fn measure_to_beat_pos(time: f32) -> (i32, i32, i32, i32) {
    let ticks = ((time - 1.0) * TICKS_PER_MEASURE as f32).round() as i32;
    // Handle negative ticks (before measure 1)
    let measure = if ticks >= 0 {
        ticks / TICKS_PER_MEASURE + 1
    } else {
        (ticks - TICKS_PER_MEASURE + 1) / TICKS_PER_MEASURE + 1
    };
    let tick_in_measure = ticks - (measure - 1) * TICKS_PER_MEASURE;
    let beat = tick_in_measure / TICKS_PER_BEAT + 1;
    let tick_in_beat = tick_in_measure % TICKS_PER_BEAT;

    let (division, offset) = if tick_in_beat == 0 {
        (1, 0)
    } else {
        let g = gcd(tick_in_beat, TICKS_PER_BEAT);
        (TICKS_PER_BEAT / g, tick_in_beat / g)
    };

    (measure, beat, division, offset)
}

/// Convert duration in measures → [numerator, denominator] in beats.
fn duration_to_fraction(dur: f32) -> [i32; 2] {
    let ticks = (dur * TICKS_PER_MEASURE as f32).round() as i32;
    if ticks == 0 {
        return [0, 1];
    }
    let g = gcd(ticks, TICKS_PER_BEAT);
    [ticks / g, TICKS_PER_BEAT / g]
}

/// Convert [numerator, denominator] beats fraction → duration in measures.
fn fraction_to_duration(frac: [i32; 2]) -> f32 {
    if frac[1] == 0 {
        return 0.0;
    }
    let ticks = frac[0] * TICKS_PER_BEAT / frac[1];
    ticks as f32 / TICKS_PER_MEASURE as f32
}

/// Convert (measure, beat, division, offset) → internal measure float.
fn beat_pos_to_measure(measure: i32, beat: i32, division: i32, offset: i32) -> f32 {
    let tick = (measure - 1) * TICKS_PER_MEASURE
        + (beat - 1) * TICKS_PER_BEAT
        + if division > 0 {
            offset * TICKS_PER_BEAT / division
        } else {
            0
        };
    1.0 + tick as f32 / TICKS_PER_MEASURE as f32
}

// ─── Public conversion API ──────────────────────────────────────────

pub fn note_to_ser(note: &Note) -> SerNote {
    let (measure, beat, division, offset) = measure_to_beat_pos(note.time);

    let hold_duration = if note.hold_duration != 0.0 {
        Some(duration_to_fraction(note.hold_duration))
    } else {
        None
    };

    SerNote {
        measure,
        beat,
        division,
        offset,
        lane: note.lane,
        note_type: note.note_type,
        is_each: note.is_each,
        is_break: note.is_break,
        is_ex: note.is_ex,
        is_star: note.is_star,
        is_tapless: note.is_tapless,
        hold_duration,
        slide: note.slide.clone(),
    }
}

pub fn ser_to_note(s: &SerNote) -> Note {
    let time = beat_pos_to_measure(s.measure, s.beat, s.division, s.offset);
    let hold_duration = s.hold_duration.map_or(0.0, fraction_to_duration);

    Note {
        id: 0,
        time,
        lane: s.lane,
        note_type: s.note_type,
        hold_duration,
        is_each: s.is_each,
        is_break: s.is_break,
        is_ex: s.is_ex,
        is_star: s.is_star,
        is_tapless: s.is_tapless,
        slide: s.slide.clone(),
        template_source: None,
    }
}

pub fn chart_to_ser(chart: &ChartDoc) -> SerChartDoc {
    SerChartDoc {
        version: "0.4.0-beat".to_string(),
        title: chart.title.clone(),
        artist: chart.artist.clone(),
        simai_level: chart.simai_level,
        bpm: chart.bpm,
        bpms: chart.bpms.clone(),
        audio_offset: chart.audio_offset,
        notes: chart.notes.iter().map(note_to_ser).collect(),
    }
}

pub fn ser_to_chart(s: &SerChartDoc) -> ChartDoc {
    let bpms = if s.bpms.is_empty() && s.bpm > 0.0 {
        vec![BpmChange {
            measure: 1.0,
            bpm: s.bpm,
        }]
    } else {
        s.bpms.clone()
    };
    ChartDoc {
        version: s.version.clone(),
        title: s.title.clone(),
        artist: s.artist.clone(),
        simai_level: s.simai_level,
        bpm: s.bpm,
        bpms,
        audio_offset: s.audio_offset,
        notes: s.notes.iter().map(ser_to_note).collect(),
        templates: Vec::new(),
        template_instances: Vec::new(),
    }
}

pub fn recording_to_ser(doc: &RecordingDoc) -> SerRecordingDoc {
    SerRecordingDoc {
        created_at_epoch_ms: doc.created_at_epoch_ms,
        source: doc.source.clone(),
        chart: chart_to_ser(&doc.chart),
        hits: doc.hits.clone(),
        record_speed: doc.record_speed,
        play_speed: doc.play_speed,
    }
}

pub fn ser_to_recording(s: &SerRecordingDoc) -> RecordingDoc {
    RecordingDoc {
        created_at_epoch_ms: s.created_at_epoch_ms,
        source: s.source.clone(),
        chart: ser_to_chart(&s.chart),
        hits: s.hits.clone(),
        record_speed: s.record_speed,
        play_speed: s.play_speed,
    }
}

/// Serialize a ChartDoc to pretty JSON in format C.
pub fn chart_to_json(chart: &ChartDoc) -> Result<String, String> {
    let ser = chart_to_ser(chart);
    serde_json::to_string_pretty(&ser).map_err(|e| format!("serialize chart: {e}"))
}

/// Deserialize a ChartDoc from JSON (supports both format C and legacy).
pub fn chart_from_json(json: &str) -> Result<ChartDoc, String> {
    // Try format C first
    if let Ok(ser) = serde_json::from_str::<SerChartDoc>(json) {
        if ser.version.contains("beat") {
            return Ok(ser_to_chart(&ser));
        }
    }
    // Fall back to legacy format
    serde_json::from_str::<ChartDoc>(json).map_err(|e| format!("parse chart: {e}"))
}

/// Serialize a RecordingDoc to pretty JSON in format C.
pub fn recording_to_json(doc: &RecordingDoc) -> Result<String, String> {
    let ser = recording_to_ser(doc);
    serde_json::to_string_pretty(&ser).map_err(|e| format!("serialize recording: {e}"))
}

/// Deserialize a RecordingDoc from JSON (supports both format C and legacy).
pub fn recording_from_json(json: &str) -> Result<RecordingDoc, String> {
    if let Ok(ser) = serde_json::from_str::<SerRecordingDoc>(json) {
        if ser.chart.version.contains("beat") {
            return Ok(ser_to_recording(&ser));
        }
    }
    serde_json::from_str::<RecordingDoc>(json).map_err(|e| format!("parse recording: {e}"))
}
