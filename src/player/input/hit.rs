//! Stand-in judgment: when a zone is pressed, find the nearest note in that
//! zone and return a label so the pad shows hit text.

use crate::app::types::zone::PadZone;
use crate::app::types::{Mode, note_secs, sanitize_note_zone};
use crate::player::state::PadPreviewState;

/// Label a zone press based on the nearest same-zone note, or `None` if there
/// is no note close enough. Thresholds are loose — this only drives visuals.
pub fn judge_label_for_zone(app: &PadPreviewState, zone: PadZone) -> Option<&'static str> {
    let now = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    let bpms = &app.chart.bpms;
    let zid = zone.to_id();

    let mut best = f32::MAX;
    for note in &app.chart.notes {
        if sanitize_note_zone(note.note_type, note.lane) != zid {
            continue;
        }
        let dt = (note_secs(note, bpms) - now).abs();
        if dt < best {
            best = dt;
        }
    }

    if best <= 0.12 {
        Some("Perfect")
    } else if best <= 0.25 {
        Some("Good")
    } else {
        None
    }
}
