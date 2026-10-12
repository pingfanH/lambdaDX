//! Right-hand panel: live pad preview on top, inspector below.
//!
//! The panel background is painted by macroquad (so the pad can render behind a
//! transparent egui layer); this module only reserves the preview region and
//! draws the inspector chrome.

use egui_macroquad::egui;

use crate::app::types::ChartDoc;
use crate::state::EditorState;
use crate::theme::PADDING;

use super::inspector;
use super::layout::Layout;

/// Fill the sidebar `ui`, reserving the pad preview at the top and the
/// inspector below. Records the preview rect on `layout`.
pub fn draw(
    ui: &mut egui::Ui,
    layout: &mut Layout,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    inspector_open: &mut [bool; 2],
) {
    layout.sidebar = ui.max_rect();

    // Pad preview: the design allots the upper ~55 % of the panel to the pad,
    // with the inspector stacked underneath.
    let pad_h = (ui.available_height() * 0.55)
        .min(ui.available_width() * 1.15)
        .max(160.0);
    let (preview_rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), pad_h), egui::Sense::hover());
    layout.preview = preview_rect;

    ui.add_space(PADDING);
    inspector::draw(ui, editor, chart, inspector_open);
}
