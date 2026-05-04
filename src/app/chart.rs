use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::platform;
use super::state::AppState;
use super::types::{ChartDoc, Note, NoteType, RecordingDoc};

pub(crate) async fn load_generated_chart() -> ChartDoc {
    match platform::load_asset_bytes("generated_chart.json").await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            serde_json::from_str::<ChartDoc>(&text).unwrap_or_else(|_| fallback_chart())
        }
        Err(_) => fallback_chart(),
    }
}

fn fallback_chart() -> ChartDoc {
    ChartDoc {
        version: "0.1.0".to_string(),
        title: "Fallback Demo Chart".to_string(),
        bpm: 180.0,
        notes: vec![
            Note {
                time: 0.60,
                lane: 1,
                note_type: NoteType::Tap,
                hold_duration: 0.0,
            },
            Note {
                time: 0.90,
                lane: 3,
                note_type: NoteType::Tap,
                hold_duration: 0.0,
            },
            Note {
                time: 1.20,
                lane: 5,
                note_type: NoteType::Tap,
                hold_duration: 0.0,
            },
            Note {
                time: 1.50,
                lane: 8,
                note_type: NoteType::Tap,
                hold_duration: 0.0,
            },
            Note {
                time: 1.80,
                lane: 9,
                note_type: NoteType::Touch,
                hold_duration: 0.0,
            },
            Note {
                time: 2.20,
                lane: 6,
                note_type: NoteType::Hold,
                hold_duration: 0.80,
            },
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

    let content = serde_json::to_string_pretty(&doc).map_err(|e| format!("serialize: {e}"))?;
    let record_name = format!("recording_{now}.json");
    let path = platform::write_output_text(&record_name, &content)?;

    let latest_chart =
        serde_json::to_string_pretty(&app.chart).map_err(|e| format!("serialize chart: {e}"))?;
    platform::write_output_text("latest_chart.json", &latest_chart)?;

    Ok(path)
}

pub(crate) fn load_latest_saved_chart() -> Result<ChartDoc, String> {
    let s = platform::read_output_text("latest_chart.json")?;
    serde_json::from_str::<ChartDoc>(&s).map_err(|e| format!("parse latest chart: {e}"))
}
