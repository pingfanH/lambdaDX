use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::beat_format;
use super::platform;
use super::state::AppState;
use super::types::{ChartDoc, Note, NoteType, RecordingDoc, secs_to_measure, sdur_to_mdur};

pub(crate) async fn load_generated_chart() -> ChartDoc {
    // Try latest saved chart first, then generated_chart, then fallback
    if let Ok(s) = platform::read_output_text("latest_chart.json") {
        if let Ok(chart) = load_chart_from_json(&s) {
            return chart;
        }
    }
    match platform::load_asset_bytes("generated_chart.json").await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            match load_chart_from_json(&text) {
                Ok(chart) => chart,
                Err(_) => fallback_chart(),
            }
        }
        Err(_) => fallback_chart(),
    }
}

/// Load a chart from JSON, supporting format C (beat), legacy measure, and old seconds formats.
fn load_chart_from_json(json: &str) -> Result<ChartDoc, String> {
    // Try format C (beat) first
    if let Ok(chart) = beat_format::chart_from_json(json) {
        if chart.version.contains("beat") {
            return Ok(chart);
        }
    }
    // Legacy: parse as ChartDoc directly and migrate if needed
    match serde_json::from_str::<ChartDoc>(json) {
        Ok(mut chart) => {
            migrate_to_measures(&mut chart);
            Ok(chart)
        }
        Err(e) => Err(format!("parse chart: {e}")),
    }
}

/// Migrate a chart loaded from JSON: old charts store time in seconds,
/// new ones in measures.  Detection is based on the version string.
fn migrate_to_measures(chart: &mut ChartDoc) {
    if chart.version.contains("measure") {
        return;
    }
    let bpm = if chart.bpm > 0.0 { chart.bpm } else { 120.0 };
    for note in &mut chart.notes {
        note.time = secs_to_measure(note.time, bpm);
        note.hold_duration = sdur_to_mdur(note.hold_duration, bpm);
        note.slide_duration = sdur_to_mdur(note.slide_duration, bpm);
        note.slide_start_delay = sdur_to_mdur(note.slide_start_delay, bpm);
    }
    if !chart.version.is_empty() {
        chart.version = format!("{}-measure", chart.version);
    } else {
        chart.version = "0.3.0-measure".to_string();
    }
}

fn fallback_chart() -> ChartDoc {
    // All times/durations in measures (1.0 = first beat, 0.25 = one beat at 4/4).
    ChartDoc {
        version: "0.3.0-measure".to_string(),
        title: "Fallback Demo Chart".to_string(),
        bpm: 180.0,
        audio_offset: 0.0,
        notes: vec![
            Note { time: 2.0,  lane: 1, note_type: NoteType::Tap,   hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false, slide_points: Vec::new(), slide_duration: 0.0, slide_start_delay: 0.0, slide_shape: None },
            Note { time: 2.25, lane: 3, note_type: NoteType::Tap,   hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false, slide_points: Vec::new(), slide_duration: 0.0, slide_start_delay: 0.0, slide_shape: None },
            Note { time: 2.5,  lane: 5, note_type: NoteType::Tap,   hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false, slide_points: Vec::new(), slide_duration: 0.0, slide_start_delay: 0.0, slide_shape: None },
            Note { time: 2.75, lane: 8, note_type: NoteType::Tap,   hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false, slide_points: Vec::new(), slide_duration: 0.0, slide_start_delay: 0.0, slide_shape: None },
            Note { time: 3.0,  lane: 9, note_type: NoteType::Touch, hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false, slide_points: Vec::new(), slide_duration: 0.0, slide_start_delay: 0.0, slide_shape: None },
            // Hold: 2 beats = 0.5 measures
            Note { time: 3.5,  lane: 6, note_type: NoteType::Hold,  hold_duration: 0.5, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false, slide_points: Vec::new(), slide_duration: 0.0, slide_start_delay: 0.0, slide_shape: None },
            // Slide 1: A1 -> A5
            Note { time: 5.0,  lane: 1, note_type: NoteType::Slide, hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false,
                slide_points: vec![super::types::SlidePoint { zone: 5, beat_offset: 0.0 }],
                slide_duration: 0.5, slide_start_delay: 0.0625, slide_shape: None },
            // Slide 2: A3 -> A7
            Note { time: 6.0,  lane: 3, note_type: NoteType::Slide, hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false,
                slide_points: vec![super::types::SlidePoint { zone: 7, beat_offset: 0.0 }],
                slide_duration: 0.5, slide_start_delay: 0.125, slide_shape: None },
            // Slide 3: A1 -> A3 -> A5
            Note { time: 7.0,  lane: 1, note_type: NoteType::Slide, hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false,
                slide_points: vec![
                    super::types::SlidePoint { zone: 3, beat_offset: 0.0 },
                    super::types::SlidePoint { zone: 5, beat_offset: 0.0 },
                ],
                slide_duration: 0.75, slide_start_delay: 0.0625, slide_shape: None },
            // Slide 4: A2 -> A4 -> A6 -> A8
            Note { time: 8.5,  lane: 2, note_type: NoteType::Slide, hold_duration: 0.0, is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false,
                slide_points: vec![
                    super::types::SlidePoint { zone: 4, beat_offset: 0.0 },
                    super::types::SlidePoint { zone: 6, beat_offset: 0.0 },
                    super::types::SlidePoint { zone: 8, beat_offset: 0.0 },
                ],
                slide_duration: 1.0, slide_start_delay: 0.25, slide_shape: None },
            // Each pair: two simultaneous slides
            Note { time: 10.0, lane: 1, note_type: NoteType::Slide, hold_duration: 0.0, is_each: true, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false,
                slide_points: vec![super::types::SlidePoint { zone: 5, beat_offset: 0.0 }],
                slide_duration: 0.5, slide_start_delay: 0.125, slide_shape: None },
            Note { time: 10.0, lane: 5, note_type: NoteType::Slide, hold_duration: 0.0, is_each: true, is_break: false, is_ex: false, is_star: false, is_tapless: false, star_is_break: false, star_is_ex: false,
                slide_points: vec![super::types::SlidePoint { zone: 1, beat_offset: 0.0 }],
                slide_duration: 0.5, slide_start_delay: 0.125, slide_shape: None },
        ],
    }
}

/// Save full recording and latest chart to a platform-compatible writable folder.
pub(crate) fn save_recording_doc(app: &AppState) -> Result<PathBuf, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_millis();

    let doc = RecordingDoc {
        created_at_epoch_ms: now,
        source: "macroquad_sim".to_string(),
        chart: app.chart.clone(),
        hits: app.recording_hits.clone(),
        record_speed: app.record_speed,
        play_speed: app.play_speed,
    };

    let content = beat_format::recording_to_json(&doc)?;
    let record_name = format!("recording_{now}.json");
    let path = platform::write_output_text(&record_name, &content)?;

    let latest_chart = beat_format::chart_to_json(&app.chart)?;
    platform::write_output_text("latest_chart.json", &latest_chart)?;

    Ok(path)
}

pub(crate) fn load_latest_saved_chart() -> Result<ChartDoc, String> {
    let s = platform::read_output_text("latest_chart.json")?;
    load_chart_from_json(&s)
}
