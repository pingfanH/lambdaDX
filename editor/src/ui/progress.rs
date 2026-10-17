//! Absolute song navigation on both time-axis edges of the chart pane.
use egui_macroquad::egui::{self, Color32, Pos2, Rect, Sense, Stroke};
use crate::state::{EditorLayout, EditorState};
use crate::timeline::geometry::{self, JUDGE_MARGIN, LANE_GUTTER};

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorState, now: f32, duration: f32, enabled: bool) -> (Rect, bool) {
    let rect = ui.max_rect();
    let vertical = editor.layout == EditorLayout::Vertical;
    let width = 10.0;
    let plot = if vertical { rect.shrink2(egui::vec2(width, 0.0)) }
        else { rect.shrink2(egui::vec2(0.0, width)) };
    let rails = if vertical {
        [Rect::from_min_max(rect.min, Pos2::new(plot.left(), rect.bottom())),
         Rect::from_min_max(Pos2::new(plot.right(), rect.top()), rect.max)]
    } else {
        [Rect::from_min_max(rect.min, Pos2::new(rect.right(), plot.top())),
         Rect::from_min_max(Pos2::new(rect.left(), plot.bottom()), rect.max)]
    };
    let duration = if duration.is_finite() { duration.max(0.01) } else { 1.0 };
    let view = geometry::view_reference(editor, now);
    let pps = editor.pixels_per_second.max(1.0);
    let (start, end) = if vertical {
        (view - JUDGE_MARGIN / pps, view + (plot.height() - LANE_GUTTER - JUDGE_MARGIN).max(0.0) / pps)
    } else {
        (view, view + (plot.width() - LANE_GUTTER).max(0.0) / pps)
    };
    let drag_id = ui.id().with("song_progress_drag");
    let was_dragging = ui.ctx().data(|data| data.get_temp::<bool>(drag_id)).unwrap_or(false);
    let mut owns_pointer = was_dragging;
    let mut dragging = false;
    for (index, rail) in rails.into_iter().enumerate() {
        let r = rail.shrink(2.0);
        let axis = |t: f32| {
            let f = (t / duration).clamp(0.0, 1.0);
            if vertical { r.bottom() - f * r.height() } else { r.left() + f * r.width() }
        };
        let span = |a: f32, b: f32| {
            let (a, b) = (axis(a), axis(b));
            if vertical { Rect::from_min_max(Pos2::new(r.left(), a.min(b)), Pos2::new(r.right(), a.max(b))) }
            else { Rect::from_min_max(Pos2::new(a.min(b), r.top()), Pos2::new(a.max(b), r.bottom())) }
        };
        let p = ui.painter();
        p.rect_filled(r, 2.0, Color32::from_white_alpha(12));
        p.rect_filled(span(start, end), 2.0, Color32::from_white_alpha(55));
        if let Some((a, b)) = editor.loop_range {
            p.rect_filled(span(a, b), 0.0, crate::theme::loop_fill(editor.loop_enabled));
        }
        let play = axis(editor.pending_seek.unwrap_or(now));
        let stroke = Stroke::new(2.0_f32, crate::theme::ACCENT_RED);
        if vertical { p.hline(r.x_range(), play, stroke); }
        else { p.vline(play, r.y_range(), stroke); }
        let response = ui.interact(rail, ui.id().with(("song_progress", index)),
            if enabled { Sense::click_and_drag() } else { Sense::hover() });
        owns_pointer |= enabled && (response.is_pointer_button_down_on() || response.clicked()
            || response.dragged() || response.drag_stopped());
        dragging |= enabled && response.dragged() && ui.input(|i| i.focused && i.pointer.primary_down());
        if enabled && (response.clicked() || response.dragged() || response.drag_stopped()) {
            if let Some(pos) = response.interact_pointer_pos() {
                let fraction = if vertical { (r.bottom() - pos.y) / r.height().max(1.0) }
                    else { (pos.x - r.left()) / r.width().max(1.0) };
                let time = fraction.clamp(0.0, 1.0) * duration;
                editor.pending_seek = Some(time);
                editor.scrubbing = dragging;
                let reference = if vertical { time } else { time - (end - start) * 0.5 };
                geometry::set_view_reference(editor, reference, time);
            }
        }
        response.on_hover_text(format!("{:.2} / {:.2} s", editor.pending_seek.unwrap_or(now), duration));
    }
    if was_dragging && !dragging { editor.scrubbing = false; }
    ui.ctx().data_mut(|data| data.insert_temp(drag_id, dragging));
    (plot, owns_pointer)
}
