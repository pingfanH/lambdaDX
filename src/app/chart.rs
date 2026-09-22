use super::beat_format;
use super::platform;
use super::types::{
    BpmChange, ChartDoc, Note, NoteType, RecordingDoc, sdur_to_mdur, secs_to_measure,
};
use crate::app::types::zone::PadZone;
use std::path::{Path, PathBuf};

/// Load the chart for the preview.
///
/// Order:
/// 1. the bundled `.maichart` song (`assets/charts/jack_ripper/chart.json`),
/// 2. a saved `latest_chart.json` in the writable output dir,
/// 3. `assets/generated_chart.json`,
/// 4. the built-in fallback chart.
pub async fn load_generated_chart(diff: Option<i32>) -> ChartDoc {
    if let Ok(chart) = super::maichart::load_default_with_diff(diff).await {
        return chart;
    }
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

/// Load a chart from a local path given on the command line.
///
/// `path` may be:
/// * a `.maichart`-style **folder** containing `maidata.txt` (simai) and/or
///   `chart.json`, or
/// * any **JSON file** (maichart / internal format) or a **`maidata.txt`**.
pub fn load_chart_from_path(path: &Path, diff: Option<i32>) -> Result<ChartDoc, String> {
    // A folder prefers its simai `maidata.txt`, then `chart.json`.
    let file = if path.is_dir() {
        let maidata = path.join("maidata.txt");
        if maidata.is_file() {
            maidata
        } else {
            path.join("chart.json")
        }
    } else {
        path.to_path_buf()
    };

    let bytes = std::fs::read(&file).map_err(|e| format!("read {}: {e}", file.display()))?;

    // Simai `maidata.txt` (by name or `.txt` extension).
    let is_simai = file
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("maidata.txt"))
        .unwrap_or(false)
        || file
            .extension()
            .map(|e| e.eq_ignore_ascii_case("txt"))
            .unwrap_or(false);
    if is_simai {
        let text = String::from_utf8_lossy(&bytes);
        return super::maidata::from_maidata(&text, diff)
            .map_err(|e| format!("{}: {e}", file.display()));
    }

    // maichart chart.json (songName / notes / bpmList …) first.
    if let Ok(chart) = super::maichart::from_bytes_with_diff(&bytes, diff) {
        return Ok(chart);
    }
    // Fall back to the internal formats.
    let text = String::from_utf8_lossy(&bytes);
    load_chart_from_json(&text).map_err(|e| format!("{}: {e}", file.display()))
}

/// Audio file to use for a chart folder, if one is present.
pub fn find_audio_in_dir(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    for name in ["track.mp3", "track.wav", "demo.mp3", "demo.wav"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Load a chart from JSON, supporting the beat format, the legacy measure
/// format, the old seconds format, and `RecordingDoc` (which wraps a chart).
fn load_chart_from_json(json: &str) -> Result<ChartDoc, String> {
    if let Ok(chart) = beat_format::chart_from_json(json) {
        if chart.version.contains("beat") {
            return Ok(chart);
        }
    }
    if let Ok(ser) = serde_json::from_str::<beat_format::SerRecordingDoc>(json) {
        if ser.chart.version.contains("beat") {
            return Ok(beat_format::ser_to_chart(&ser.chart));
        }
    }
    if let Ok(rec) = serde_json::from_str::<RecordingDoc>(json) {
        return Ok(rec.chart);
    }
    match serde_json::from_str::<ChartDoc>(json) {
        Ok(mut chart) => {
            migrate_to_measures(&mut chart);
            Ok(chart)
        }
        Err(e) => Err(format!("parse chart: {e}")),
    }
}

/// Migrate a chart loaded from JSON: old charts store time in seconds, new
/// ones in measures. Detection is based on the version string.
fn migrate_to_measures(chart: &mut ChartDoc) {
    if chart.version.contains("measure") {
        return;
    }
    if chart.bpms.is_empty() && chart.bpm > 0.0 {
        chart.bpms = vec![BpmChange {
            measure: 1.0,
            bpm: chart.bpm,
        }];
    }
    let bpms = &chart.bpms;
    for note in &mut chart.notes {
        let t = note.time;
        note.time = secs_to_measure(t, bpms);
        note.hold_duration = sdur_to_mdur(note.hold_duration, t, bpms);
        for sl in &mut note.slide {
            sl.slide_duration = sdur_to_mdur(sl.slide_duration, t, bpms);
            sl.slide_start_delay = sdur_to_mdur(sl.slide_start_delay, t, bpms);
        }
    }
    if !chart.version.is_empty() {
        chart.version = format!("{}-measure", chart.version);
    } else {
        chart.version = "0.3.0-measure".to_string();
    }
}

/// Try to load `latest_chart.json` from the writable output dir.
pub fn load_latest_saved_chart() -> Result<ChartDoc, String> {
    let s = platform::read_output_text("latest_chart.json")?;
    load_chart_from_json(&s)
}/// Built-in demo chart, used when no JSON asset parses. Times/durations are in
/// measures (1.0 = first beat, 0.25 = one beat at 4/4).
pub fn fallback_chart() -> ChartDoc {
    use super::types::{Slide, SlidePoint, SlideSegment, SlideShape};
    let mk_slide = |pts: Vec<SlidePoint>, dur: f32, delay: f32| -> Vec<Slide> {
        vec![Slide {
            segments: vec![SlideSegment {
                points: pts,
                shape: SlideShape::Line,
            }],
            slide_duration: dur,
            slide_start_delay: delay,
            slide_is_break: false,
        }]
    };
    let sp = |z: u8| SlidePoint {
        zone: PadZone::from(z),
        beat_offset: 0.0,
    };
    ChartDoc {
        version: "0.3.0-measure".to_string(),
        title: "Fallback Demo Chart".to_string(),
        artist: String::new(),
        simai_level: 0,
        bpm: 180.0,
        bpms: vec![BpmChange {
            measure: 1.0,
            bpm: 180.0,
        }],
        audio_offset: 0.0,
        notes: vec![
            Note {
                time: 2.0,
                lane: 1,
                ..Default::default()
            },
            Note {
                time: 2.25,
                lane: 3,
                ..Default::default()
            },
            Note {
                time: 2.5,
                lane: 5,
                ..Default::default()
            },
            Note {
                time: 2.75,
                lane: 8,
                ..Default::default()
            },
            Note {
                time: 3.0,
                lane: 9,
                note_type: NoteType::Touch,
                ..Default::default()
            },
            Note {
                time: 3.5,
                lane: 6,
                note_type: NoteType::Hold,
                hold_duration: 0.5,
                ..Default::default()
            },
            Note {
                time: 5.0,
                lane: 1,
                note_type: NoteType::Slide,
                slide: mk_slide(vec![sp(5)], 0.5, 0.0625),
                ..Default::default()
            },
            Note {
                time: 6.0,
                lane: 3,
                note_type: NoteType::Slide,
                slide: mk_slide(vec![sp(7)], 0.5, 0.125),
                ..Default::default()
            },
            Note {
                time: 7.0,
                lane: 1,
                note_type: NoteType::Slide,
                slide: mk_slide(vec![sp(3), sp(5)], 0.75, 0.0625),
                ..Default::default()
            },
            Note {
                time: 8.5,
                lane: 2,
                note_type: NoteType::Slide,
                slide: mk_slide(vec![sp(4), sp(6), sp(8)], 1.0, 0.25),
                ..Default::default()
            },
            Note {
                time: 10.0,
                lane: 1,
                note_type: NoteType::Slide,
                is_each: true,
                slide: mk_slide(vec![sp(5)], 0.5, 0.125),
                ..Default::default()
            },
            Note {
                time: 10.0,
                lane: 5,
                note_type: NoteType::Slide,
                is_each: true,
                slide: mk_slide(vec![sp(1)], 0.5, 0.125),
                ..Default::default()
            },
        ],
        templates: Vec::new(),
        template_instances: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{find_audio_in_dir, load_chart_from_path};
    use std::path::Path;

    fn bundled_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/charts/jack_ripper")
    }

    #[test]
    fn loads_bundled_chart_folder_and_file() {
        let dir = bundled_dir();
        let from_dir = load_chart_from_path(&dir, None).expect("folder load");
        assert!(from_dir.title.contains("Jack"), "title = {}", from_dir.title);

        let file = dir.join("chart.json");
        let from_file = load_chart_from_path(&file, Some(1)).expect("file load");
        // Difficulty 1 is the easiest, so it has fewer notes than the default.
        assert!(from_file.notes.len() < from_dir.notes.len());

        assert!(find_audio_in_dir(&dir).is_some(), "track auto-detect");
    }

    #[test]
    fn missing_path_errors() {
        assert!(load_chart_from_path(Path::new("does/not/exist.json"), None).is_err());
    }
}
