//! Bottom overview / progress bar (egui).
//!
//! A compact minimap of every note's time with a draggable red playhead. It is
//! the only place a drag seeks to an absolute time; note editing happens in the
//! timeline itself.

use egui_macroquad::egui;

use crate::app::types::{hold_tail_time, note_secs, ChartDoc, NoteType};
use crate::state::EditorState;

use crate::theme::{
    ACCENT_RED, BG_PANEL, BORDER_LIGHT, FONT_SMALL, TEXT, TEXT_DIM, TICK_HOLD, TICK_SLIDE,
    TICK_TAP, TICK_TOUCH,
};

/// Draw the overview into `rect` (already reserved by the layout).
pub fn draw_in(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &ChartDoc,
    current_time: f32,
    plot_left: f32,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, BG_PANEL);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0_f32, BORDER_LIGHT),
    );

    let width = (rect.right() - plot_left).max(1.0);
    painter.line_segment(
        [
            egui::pos2(plot_left, rect.top()),
            egui::pos2(plot_left, rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, BORDER_LIGHT),
    );

    let duration = chart
        .notes
        .iter()
        .map(|n| hold_tail_time(n, &chart.bpms).max(note_secs(n, &chart.bpms)))
        .fold(8.0_f32, f32::max);
    let x_of = |t: f32| plot_left + t.clamp(0.0, duration) / duration * width;

    for note in &chart.notes {
        let x = x_of(note_secs(note, &chart.bpms));
        painter.line_segment(
            [
                egui::pos2(x, rect.top() + 5.0),
                egui::pos2(x, rect.bottom() - 5.0),
            ],
            egui::Stroke::new(1.0_f32, tick_color(note.note_type)),
        );
    }

    let play_x = x_of(current_time);
    painter.vline(play_x, rect.y_range(), egui::Stroke::new(2.0_f32, ACCENT_RED));

    // Read-outs on the bar (design shows a time / zoom readout under the grid).
    painter.text(
        egui::pos2(plot_left + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &format!("{:.2}s / {:.2}s", current_time.max(0.0), duration),
        egui::FontId::proportional(FONT_SMALL),
        TEXT,
    );
    painter.text(
        egui::pos2(rect.right() - 6.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        &format!("{:.0} px/s · {} notes", editor.pixels_per_second, chart.notes.len()),
        egui::FontId::proportional(FONT_SMALL),
        TEXT_DIM,
    );

    let response = ui.interact(
        rect,
        ui.id().with("editor_overview"),
        egui::Sense::click_and_drag(),
    );
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let frac = ((pos.x - plot_left) / width).clamp(0.0, 1.0);
            editor.pending_seek = Some(frac * duration);
            editor.scrubbing = true;
        }
    }
    if response.drag_stopped() {
        editor.scrubbing = false;
    }
}

/// Colour for a note's tick in the minimap.
fn tick_color(kind: NoteType) -> egui::Color32 {
    match kind {
        NoteType::Hold => TICK_HOLD,
        NoteType::Touch | NoteType::TouchHold => TICK_TOUCH,
        NoteType::Slide => TICK_SLIDE,
        NoteType::Tap => TICK_TAP,
    }
}
