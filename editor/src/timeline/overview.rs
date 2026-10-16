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
    interactive: bool,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, crate::theme::surface(BG_OVERVIEW));
    let bar = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width(), (rect.height() - 20.0).max(10.0)),
    );
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
    paint_loop(painter, bar, editor, x_of, true);

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

    let readout_y = bar.bottom() + 10.0;
    painter.text(
        egui::pos2(bar.left() + 6.0, readout_y),
        egui::Align2::LEFT_CENTER,
        &format!("{:.2}s / {:.2}s", current_time.max(0.0), duration),
        egui::FontId::proportional(FONT_SMALL),
        TEXT,
    );
    if rect.width() > 320.0 {
        painter.text(
            egui::pos2(bar.right() - 6.0, readout_y),
            egui::Align2::RIGHT_CENTER,
            &right_readout(editor, chart),
            egui::FontId::proportional(FONT_SMALL),
            TEXT_DIM,
        );
    }

    let response = ui.interact(
        bar,
        ui.id().with("editor_overview"),
        if interactive {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::hover()
        },
    );
    let looping = loop_gesture(ui, &response, editor, interactive, |x| {
        ((x - plot_left) / width).clamp(0.0, 1.0) * duration
    }, x_of);
    if interactive && !looping && (response.clicked() || response.dragged()) {
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

pub fn paint_loop(
    painter: &egui::Painter, plot: egui::Rect, editor: &EditorState,
    position: impl Fn(f32) -> f32, handles: bool,
) {
    let Some((start, end)) = editor.loop_range.filter(|(a, b)| a.is_finite() && b.is_finite() && b > a) else { return; };
    let (a, b) = (position(start), position(end));
    let range = egui::Rect::from_min_max(egui::pos2(a, plot.top()), egui::pos2(b, plot.bottom())).intersect(plot);
    if !range.is_positive() { return; }
    let p = painter.with_clip_rect(plot.intersect(painter.clip_rect()));
    p.rect_filled(range, 0.0, crate::theme::loop_fill(editor.loop_enabled));
    let color = crate::theme::ACCENT_YELLOW.gamma_multiply(if editor.loop_enabled { 1.0 } else { 0.4 });
    for x in [a, b] {
        if x < plot.left() || x > plot.right() { continue; }
        p.vline(x, plot.y_range(), egui::Stroke::new(1.0_f32, color));
        if handles {
            p.rect_filled(egui::Rect::from_center_size(egui::pos2(x, plot.center().y),
                egui::vec2(4.0, plot.height().min(12.0))), 1.0, color);
        }
    }
}

/// Capture a range gesture independently of seeking, including release outside.
pub fn loop_gesture(
    ui: &egui::Ui,
    response: &egui::Response,
    editor: &mut EditorState,
    interactive: bool,
    time_at: impl Fn(f32) -> f32,
    position: impl Fn(f32) -> f32,
) -> bool {
    let boundary = response.hover_pos().and_then(|pos| {
        let (a, b) = editor.loop_range?;
        let da = (position(a) - pos.x).abs();
        let db = (position(b) - pos.x).abs();
        if da.min(db) > 6.0 { None } else { Some(if da <= db { b } else { a }) }
    });
    if interactive && (boundary.is_some() || editor.loop_drag.as_ref().is_some_and(|drag| drag.owner == response.id)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if interactive
        && editor.loop_drag.is_none()
        && response.is_pointer_button_down_on()
        && ui.input(|i| i.pointer.primary_pressed() && (i.modifiers.shift || boundary.is_some()))
        && let Some(pos) = response.interact_pointer_pos()
    {
        editor.loop_drag = Some(crate::state::LoopDrag {
            owner: response.id,
            anchor: if ui.input(|i| i.modifiers.shift) { time_at(pos.x) } else { boundary.unwrap_or(time_at(pos.x)) },
            previous: editor.loop_range,
            was_enabled: editor.loop_enabled,
            cancelled: false,
        });
    }
    let Some(drag) = editor
        .loop_drag
        .as_mut()
        .filter(|drag| drag.owner == response.id)
    else {
        return false;
    };
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        || !ui.input(|i| i.focused)
    {
        editor.loop_range = drag.previous;
        editor.loop_enabled = drag.was_enabled;
        drag.cancelled = true;
    }
    if !drag.cancelled
        && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
    {
        let time = time_at(pos.x);
        let range = (drag.anchor.min(time), drag.anchor.max(time));
        if range.1 - range.0 >= 0.01 {
            editor.loop_range = Some(range);
            editor.loop_enabled = true;
        } else {
            editor.loop_range = drag.previous;
            editor.loop_enabled = drag.was_enabled;
        }
    }
    if !ui.input(|i| i.pointer.primary_down()) {
        editor.loop_drag = None;
    }
    true
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
        frame_with_modifiers(ctx, editor, events, egui::Modifiers::NONE);
    }

    fn frame_with_modifiers(
        ctx: &egui::Context,
        editor: &mut EditorState,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) {
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
            autoplay_overrides: Vec::new(),
        };
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            modifiers,
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
                    true,
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
        frame(
            &ctx,
            &mut editor,
            vec![egui::Event::PointerMoved(pos), button(pos, true)],
        );
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

    #[test]
    fn shift_drag_sets_loop_without_seeking_and_captures_release_outside() {
        let ctx = egui::Context::default();
        let mut editor = EditorState::default();
        let start = egui::pos2(200.0, 108.0);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(start)]);
        frame_with_modifiers(
            &ctx,
            &mut editor,
            vec![button(start, true)],
            egui::Modifiers::SHIFT,
        );
        assert!(editor.loop_drag.is_some());
        frame(
            &ctx,
            &mut editor,
            vec![egui::Event::PointerMoved(egui::pos2(400.0, 108.0))],
        );
        assert_eq!(editor.loop_range, Some((2.0, 6.0)));
        let outside = egui::pos2(600.0, 160.0);
        frame(
            &ctx,
            &mut editor,
            vec![egui::Event::PointerMoved(outside), button(outside, false)],
        );
        assert_eq!(editor.loop_range, Some((2.0, 8.0)));
        assert!(editor.loop_enabled);
        assert!(editor.loop_drag.is_none());
        assert_eq!(editor.pending_seek, None);
        assert!(!editor.scrubbing);
    }

    #[test]
    fn loop_boundary_drag_adjusts_existing_range_without_seeking() {
        let ctx = egui::Context::default();
        let mut editor = EditorState { loop_range: Some((2.0, 6.0)), loop_enabled: true, ..Default::default() };
        for (start, end, expected) in [
            (egui::pos2(200.0, 108.0), egui::pos2(250.0, 108.0), (3.0, 6.0)),
            (egui::pos2(400.0, 108.0), egui::pos2(450.0, 108.0), (3.0, 7.0)),
        ] {
            frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(start)]);
            frame(&ctx, &mut editor, vec![button(start, true)]);
            assert!(editor.loop_drag.is_some());
            frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(end)]);
            frame(&ctx, &mut editor, vec![button(end, false)]);
            assert_eq!(editor.loop_range, Some(expected));
            assert_eq!(editor.pending_seek, None);
            assert!(editor.loop_drag.is_none());
        }
    }

    #[test]
    fn cancelling_loop_restores_previous_range_and_does_not_seek_on_release() {
        let ctx = egui::Context::default();
        let mut editor = EditorState {
            loop_range: Some((1.0, 3.0)),
            ..Default::default()
        };
        let start = egui::pos2(200.0, 108.0);
        let end = egui::pos2(400.0, 108.0);
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(start)]);
        frame_with_modifiers(
            &ctx,
            &mut editor,
            vec![button(start, true)],
            egui::Modifiers::SHIFT,
        );
        frame(&ctx, &mut editor, vec![egui::Event::PointerMoved(end)]);
        frame(
            &ctx,
            &mut editor,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(&ctx, &mut editor, vec![button(end, false)]);
        assert_eq!(editor.loop_range, Some((1.0, 3.0)));
        assert!(!editor.loop_enabled);
        assert!(editor.loop_drag.is_none());
        assert_eq!(editor.pending_seek, None);
    }
}
