//! Note pass: iterate the chart, derive timing, cull, and dispatch each note to
//! the appropriate kind-specific renderer (`ring`, `touch`, `slide`).

use macroquad::math::Vec2;

use crate::app::params;
use crate::app::types::{Note, NoteType, PadGeom};
use crate::player::render::timing::{self, NoteTiming};
use crate::player::render::{ring, slide, touch};
use crate::player::state::PadPreviewState;

/// Draw all visible notes for `current_t` (seconds).
///
/// Slides are drawn first (trail under the head), then fall through to the zone
/// branch exactly like the player: a slide on a ring lane draws its head star
/// inside `slide`, and the ring branch only adds the on-hit ring.
///
/// The note pass runs forward (later notes on top) or in reverse (earlier notes
/// on top) depending on `note_earlier_on_top`, so overlapping note art can be
/// ordered either way.
pub fn draw_notes(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    current_t: f32,
    speed_scale: f32,
) {
    let bpms = &app.chart.bpms;
    let notes: Box<dyn Iterator<Item = &Note>> = if params::note_earlier_on_top() {
        Box::new(app.chart.notes.iter().rev())
    } else {
        Box::new(app.chart.notes.iter())
    };

    for note in notes {
        if app.hidden_notes.contains(&note.id) {
            continue;
        }

        let t: NoteTiming = timing::compute(note, app, bpms, current_t, speed_scale);
        if !t.visible() {
            continue;
        }

        if matches!(note.note_type, NoteType::Slide) {
            slide::draw(app, note, pad, scale, spawn_cx, pad.outer_r, current_t, &t);
        }

        if t.zone <= 8 {
            ring::draw(app, note, &t, scale, spawn_cx, pad.outer_r);
        } else {
            touch::draw(app, note, &t, pad, scale, current_t);
        }
    }
}
