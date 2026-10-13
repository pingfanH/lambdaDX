//! Bottom overview / progress bar (egui).
//!
//! A compact minimap of every note's time with a draggable red playhead. It is
//! the only place a drag seeks to an absolute time; note editing happens in the
//! timeline itself.

use egui_macroquad::egui;

use crate::app::types::{ChartDoc, NoteType, note_secs};
use crate::state::EditorState;

use crate::theme::{
    ACCENT_RED, BG_OVERVIEW, BORDER_LIGHT, FONT_SMALL, TEXT, TEXT_DIM, TICK_HOLD, TICK_SLIDE,
    TICK_TAP, TICK_TOUCH,
};

/// Draw the overview into `rect` (already reserved by the layout).
pub fn draw_in(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &ChartDoc,
    current_time: f32,
    duration: f32,
) {
    let painter = ui.painter();
    // Band background blends with the workspace; the design's 10px scrubber
    // sits near the top of it (`#2B2B2D`) with a 1px red start tick.
    painter.rect_filled(rect, 0.0, crate::theme::BG_WINDOW);
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + 3.0),
        egui::vec2(rect.width(), 10.0),
    );
    painter.rect_filled(bar, 0.0, BG_OVERVIEW);
    painter.vline(
        bar.left() + 0.5,
        bar.y_range(),
        egui::Stroke::new(1.0_f32, ACCENT_RED),
    );

    let plot_left = bar.left();
    let width = bar.width().max(1.0);
    painter.line_segment(
        [
            egui::pos2(plot_left, bar.top()),
            egui::pos2(plot_left, bar.bottom()),
        ],
        egui::Stroke::new(1.0_f32, BORDER_LIGHT),
    );

    let duration = duration.max(1.0);
    let x_of = |t: f32| plot_left + t.clamp(0.0, duration) / duration * width;

    for note in &chart.notes {
        let x = x_of(note_secs(note, &chart.bpms));
        painter.line_segment(
            [
                egui::pos2(x, bar.top() + 2.0),
                egui::pos2(x, bar.bottom() - 2.0),
            ],
            egui::Stroke::new(1.0_f32, tick_color(note.note_type)),
        );
    }

    let play_x = x_of(current_time);
    painter.vline(
        play_x,
        bar.y_range(),
        egui::Stroke::new(2.0_f32, ACCENT_RED),
    );

    // Time read-out inside the bar (design shows "0.00" there).
    painter.text(
        egui::pos2(bar.left() + 6.0, bar.center().y),
        egui::Align2::LEFT_CENTER,
        &format!("{:.2}s / {:.2}s", current_time.max(0.0), duration),
        egui::FontId::proportional(FONT_SMALL),
        TEXT,
    );
    painter.text(
        egui::pos2(bar.right() - 6.0, bar.center().y),
        egui::Align2::RIGHT_CENTER,
        &right_readout(editor, chart),
        egui::FontId::proportional(FONT_SMALL),
        TEXT_DIM,
    );

    let response = ui.interact(
        bar,
        ui.id().with("editor_overview"),
        egui::Sense::click_and_drag(),
    );
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let frac = ((pos.x - plot_left) / width).clamp(0.0, 1.0);
            editor.pending_seek = Some(frac * duration);
            // A click seeks on release; only an ongoing drag pauses the clock.
            editor.scrubbing = response.dragged();
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

/// Right-hand read-out: details of the hovered note when there is one, else the
/// zoom / note-count summary.
fn right_readout(editor: &EditorState, chart: &ChartDoc) -> String {
    if let Some(note) = editor
        .hovered
        .and_then(|id| chart.notes.iter().find(|n| n.id == id))
    {
        return format!("L{} · m{:.2} · {:?}", note.lane, note.time, note.note_type);
    }
    format!(
        "{:.0} px/s · {} notes",
        editor.pixels_per_second,
        chart.notes.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, editor: &mut EditorState, events: Vec<egui::Event>) {
        let chart = ChartDoc {
            version: String::new(),
            title: String::new(),
            artist: String::new(),
            simai_level: 0,
            bpm: 120.0,
            bpms: Vec::new(),
            audio_offset: 0.0,
            notes: Vec::new(),
            templates: Vec::new(),
            template_instances: Vec::new(),
        };
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw_in(
                    ui,
                    egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 30.0)),
                    editor,
                    &chart,
                    0.0,
                    8.0,
                );
            });
        });
    }

    #[test]
    fn overview_visual_midpoint_matches_seek_midpoint() {
        let ctx = egui::Context::default();
        let mut editor = EditorState::default();
        let pos = egui::pos2(300.0, 108.0);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(pos)]);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        frame(&ctx, &mut editor, vec![button(pos, false)]);
        assert_eq!(editor.pending_seek, Some(4.0));
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    #[test]
    fn overview_click_seeks_without_leaving_playback_scrubbing() {
        let ctx = egui::Context::default();
        let mut editor = EditorState::default();
        let pos = egui::pos2(300.0, 108.0);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(pos)]);
        frame(&ctx, &mut editor, vec![button(pos, true)]);
        frame(&ctx, &mut editor, vec![button(pos, false)]);
        assert_eq!(editor.pending_seek.take(), Some(4.0));
        assert!(!editor.scrubbing, "click release must not freeze playback");
        frame(&ctx, &mut editor, vec![]);
        assert!(!editor.scrubbing);
        assert_eq!(editor.pending_seek, None);
    }

    #[test]
    fn overview_drag_scrubs_until_release_outside_the_bar() {
        let ctx = egui::Context::default();
        let mut editor = EditorState::default();
        let start = egui::pos2(300.0, 108.0);
        let end = egui::pos2(400.0, 108.0);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(start)]);
        frame(&ctx, &mut editor, vec![button(start, true)]);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(end)]);
        assert!(editor.scrubbing);
        assert_eq!(editor.pending_seek.take(), Some(6.0));
        let outside = egui::pos2(600.0, 160.0);
        frame(
            &ctx,
            &mut editor,
            vec![egui::Event::PointerMoved(outside), button(outside, false)],
        );
        assert!(!editor.scrubbing, "release outside must resume playback");
    }
}
