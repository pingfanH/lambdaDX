//! Timeline: geometry, input, and the two macroquad renderers.
//!
//! Layout of this module tree:
//!
//! * [`geometry`] — orientation-independent lane/snap/selection maths + tokens.
//! * [`input`]    — pointer + keyboard interaction (per orientation).
//! * [`notes`]    — named-import-as-`draw` note recipes shared by both renderers.
//! * [`vertical`] — the design/default renderer (time flows up).
//! * [`horizontal`] — the legacy renderer (time flows right).
//! * [`waveform`] — audio backdrop, per orientation.
//! * [`overview`] — the bottom progress minimap (egui).

pub mod geometry;
pub mod handles;
pub mod horizontal;
pub mod input;
pub mod judgment;
pub mod notes;
pub mod placement;
pub mod transform;
pub mod overview;
pub mod vertical;
pub mod waveform;

use macroquad::prelude::Rect as MqRect;

use crate::app::types::ChartDoc;
use crate::audio::WaveformPyramid;
use crate::state::{EditorLayout, EditorState};
use crate::player::state::PadPreviewState;

pub use geometry::LANE_GUTTER;
pub use input::handle_edit_shortcuts;
pub use overview::draw_in as draw_overview;

/// Draw the timeline for the active layout into `rect` (macroquad logical units,
/// i.e. egui points × `scale`).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    layout: EditorLayout,
    app: &PadPreviewState,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: MqRect,
    current_time: f32,
    waveform: &WaveformPyramid,
    scale: f32,
) {
    match layout {
        EditorLayout::Vertical => {
            vertical::draw(app, editor, chart, rect, current_time, waveform, scale)
        }
        EditorLayout::Horizontal => {
            horizontal::draw(app, editor, chart, rect, current_time, waveform, scale)
        }
    }
}

/// Route pointer interaction to the active layout's handler.
pub fn handle_input(
    layout: EditorLayout,
    response: &egui_macroquad::egui::Response,
    rect: egui_macroquad::egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
    ui: &egui_macroquad::egui::Ui,
    raw_wheel: f32,
) {
    match layout {
        EditorLayout::Vertical => input::vertical(response, rect, editor, chart, current_time, ui, raw_wheel),
        EditorLayout::Horizontal => {
            input::horizontal(response, rect, editor, chart, current_time, ui, raw_wheel)
        }
    }
}
