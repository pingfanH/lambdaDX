//! Invisible playback navigation on the chart's existing beat ruler.
use egui_macroquad::egui::{self, Pos2, Rect, Sense};

use crate::app::types::{ChartDoc, measure_to_secs, secs_to_measure};
use crate::state::{EditorLayout, EditorState};
use crate::timeline::geometry::{self, JUDGE_MARGIN, LANE_GUTTER, RULER_H};

#[derive(Clone, Copy)]
struct RulerDrag {
    view: f32,
    pps: f32,
}

pub(super) fn ruler_rect(rect: Rect, layout: EditorLayout) -> Rect {
    match layout {
        EditorLayout::Vertical => Rect::from_min_max(
            Pos2::new(rect.left(), (rect.top() + RULER_H).min(rect.bottom())),
            Pos2::new((rect.left() + LANE_GUTTER).min(rect.right()), rect.bottom()),
        ),
        EditorLayout::Horizontal => Rect::from_min_max(
            Pos2::new((rect.left() + LANE_GUTTER).min(rect.right()), rect.top()),
            Pos2::new(rect.right(), (rect.top() + RULER_H).min(rect.bottom())),
        ),
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    chart: &ChartDoc,
    now: f32,
    enabled: bool,
) -> (Rect, bool) {
    let rect = ui.max_rect();
    let vertical = editor.layout == EditorLayout::Vertical;
    let ruler = ruler_rect(rect, editor.layout);
    let id = ui.id().with("chart_beat_ruler");
    let drag_id = id.with("drag");
    let mut drag = ui.ctx().data(|data| data.get_temp::<RulerDrag>(drag_id));
    let tools = super::tools::panel_rect(rect, editor.hide_chart_tools);
    let over_tools = ui.input(|i| {
        i.pointer
            .interact_pos()
            .is_some_and(|pos| tools.contains(pos))
    });
    let owns_loop = editor
        .loop_drag
        .as_ref()
        .is_some_and(|loop_drag| loop_drag.owner == id);
    let interactive = enabled && (!over_tools || drag.is_some() || owns_loop);
    let response = ui.interact(
        ruler,
        id,
        if interactive {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let focused = ui.input(|i| i.focused);
    if drag.is_some() && (!enabled || !focused || ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        ui.ctx().data_mut(|data| data.remove::<RulerDrag>(drag_id));
        editor.scrubbing = false;
        return (rect, true);
    }
    let view = drag.map_or_else(
        || {
            if vertical {
                geometry::view_reference(editor, now)
            } else {
                editor.view_start
            }
        },
        |drag| drag.view,
    );
    let pps = drag.map_or(editor.pixels_per_second.max(1.0), |drag| drag.pps);
    let axis = |pos: Pos2| if vertical { pos.y } else { pos.x };
    let origin = if vertical {
        rect.bottom() - JUDGE_MARGIN
    } else {
        ruler.left()
    };
    let direction = if vertical { -1.0 } else { 1.0 };
    let snap = editor.snap_enabled;
    let division = editor.snap_division;
    let time_at = |position: f32, view: f32| {
        let time = view + (position - origin) * direction / pps;
        measure_to_secs(
            geometry::snap(secs_to_measure(time, &chart.bpms), snap, division),
            &chart.bpms,
        )
        .max(chart.playback_start_time())
    };
    let looping = crate::timeline::overview::loop_gesture_axis(
        ui,
        &response,
        editor,
        interactive && drag.is_none(),
        vertical,
        |position| time_at(position, view),
        |time| origin + (time - view) * pps * direction,
    );
    if looping {
        return (rect, true);
    }
    if interactive
        && focused
        && response.is_pointer_button_down_on()
        && ui.input(|i| i.pointer.primary_pressed())
    {
        drag = Some(RulerDrag { view, pps });
    }
    let dragging = response.dragged_by(egui::PointerButton::Primary);
    let released = response.drag_stopped_by(egui::PointerButton::Primary);
    let owns_pointer = drag.is_some();
    if let Some(active) = &mut drag {
        if (response.clicked() || dragging || released)
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
        {
            let (lo, hi) = if vertical {
                (ruler.top(), ruler.bottom())
            } else {
                (ruler.left(), ruler.right())
            };
            let position = axis(pos);
            let edge = position.clamp(lo, hi.max(lo));
            if dragging && position != edge {
                let step = crate::timeline::input::scroll_step(
                    (position - edge) * direction,
                    ui.input(|i| i.stable_dt),
                );
                active.view += step / active.pps;
                ui.ctx().request_repaint();
            }
            let time = time_at(edge, active.view);
            editor.pending_seek = Some(time);
            editor.scrubbing = dragging;
            geometry::set_view_reference(editor, active.view, time);
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            drag = None;
            editor.scrubbing = false;
        }
    }
    ui.ctx().data_mut(|data| {
        if let Some(drag) = drag {
            data.insert_temp(drag_id, drag);
        } else {
            data.remove::<RulerDrag>(drag_id);
        }
    });
    (rect, owns_pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        ctx: egui::Context,
        editor: EditorState,
        chart: ChartDoc,
        rect: Rect,
        now: f32,
        enabled: bool,
        owned: bool,
        time: f64,
    }

    impl Fixture {
        fn new(layout: EditorLayout) -> Self {
            let mut fixture = Self {
                ctx: egui::Context::default(),
                editor: EditorState {
                    layout,
                    follow_playhead: false,
                    view_start: 1.0,
                    pixels_per_second: 120.0,
                    ..Default::default()
                },
                chart: crate::app::maidata::from_maidata("&inote_1=(120){4}1,,E", None).unwrap(),
                rect: Rect::NOTHING,
                now: 1.0,
                enabled: true,
                owned: false,
                time: 0.0,
            };
            fixture.frame(Vec::new(), egui::Modifiers::NONE);
            fixture
        }

        fn frame(
            &mut self,
            events: Vec<egui::Event>,
            modifiers: egui::Modifiers,
        ) -> egui::FullOutput {
            self.time += 0.1;
            let focused = !events.iter().any(|event| matches!(event, egui::Event::WindowFocused(false)));
            self.ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
                    time: Some(self.time),
                    focused,
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let full = ui.max_rect();
                        let (rect, owned) =
                            draw(ui, &mut self.editor, &self.chart, self.now, self.enabled);
                        assert_eq!(rect, full, "no rail space may be reserved");
                        self.rect = rect;
                        self.owned = owned;
                    });
                },
            )
        }

        fn position(&self, time: f32) -> Pos2 {
            let view = if self.editor.layout == EditorLayout::Vertical {
                geometry::view_reference(&self.editor, self.now)
            } else {
                self.editor.view_start
            };
            let pps = self.editor.pixels_per_second;
            match self.editor.layout {
                EditorLayout::Vertical => Pos2::new(
                    self.rect.left() + 4.0,
                    self.rect.bottom() - JUDGE_MARGIN - (time - view) * pps,
                ),
                EditorLayout::Horizontal => Pos2::new(
                    self.rect.left() + LANE_GUTTER + (time - view) * pps,
                    self.rect.top() + 8.0,
                ),
            }
        }

        fn button(&mut self, pos: Pos2, pressed: bool, modifiers: egui::Modifiers) {
            self.frame(
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers,
                    },
                ],
                modifiers,
            );
        }
    }

    #[test]
    fn ruler_is_invisible_and_preserves_full_canvas_bounds() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut f = Fixture::new(layout);
            let output = f.frame(Vec::new(), egui::Modifiers::NONE);
            assert_eq!(
                output.shapes.len(),
                1,
                "only the central panel background is painted"
            );
            let ruler = ruler_rect(f.rect, layout);
            if layout == EditorLayout::Vertical {
                assert_eq!(ruler.width(), LANE_GUTTER);
                assert_eq!(ruler.top(), f.rect.top() + RULER_H);
            } else {
                assert_eq!(ruler.height(), RULER_H);
                assert_eq!(ruler.left(), f.rect.left() + LANE_GUTTER);
            }
        }
    }

    #[test]
    fn ruler_click_uses_visible_beats_and_honors_snap() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut f = Fixture::new(layout);
            let pos = f.position(1.27);
            f.frame(vec![egui::Event::PointerMoved(pos)], egui::Modifiers::NONE);
            f.button(pos, true, egui::Modifiers::NONE);
            assert!(f.owned);
            f.button(pos, false, egui::Modifiers::NONE);
            assert_eq!(f.editor.pending_seek.take(), Some(1.25));
            assert!(!f.editor.scrubbing);
            f.editor.snap_enabled = false;
            f.button(pos, true, egui::Modifiers::NONE);
            f.button(pos, false, egui::Modifiers::NONE);
            assert!((f.editor.pending_seek.unwrap() - 1.27).abs() < 0.00001);
        }
    }

    #[test]
    fn measure_zero_ruler_seeks_within_preroll_on_both_axes() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut f = Fixture::new(layout);
            f.editor.hide_chart_tools = true;
            f.editor.view_start = f.chart.playback_start_time();
            f.now = f.editor.view_start;
            f.frame(Vec::new(), egui::Modifiers::NONE);
            let pos = f.position(-1.25);
            f.frame(vec![egui::Event::PointerMoved(pos)], egui::Modifiers::NONE);
            f.button(pos, true, egui::Modifiers::NONE);
            f.button(pos, false, egui::Modifiers::NONE);
            assert_eq!(f.editor.pending_seek, Some(-1.25));
            assert!(!f.editor.scrubbing);
        }
    }

    #[test]
    fn ruler_loop_drag_and_boundary_adjustment_work_on_both_axes() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut f = Fixture::new(layout);
            let start = f.position(1.25);
            let end = f.position(2.0);
            f.frame(
                vec![egui::Event::PointerMoved(start)],
                egui::Modifiers::NONE,
            );
            f.button(start, true, egui::Modifiers::SHIFT);
            f.frame(vec![egui::Event::PointerMoved(end)], egui::Modifiers::SHIFT);
            f.button(end, false, egui::Modifiers::NONE);
            assert_eq!(f.editor.loop_range, Some((1.25, 2.0)));
            assert!(f.editor.loop_enabled);
            assert!(f.editor.loop_drag.is_none());
            assert_eq!(f.editor.pending_seek, None);
            f.frame(vec![egui::Event::PointerMoved(end)], egui::Modifiers::NONE);
            f.button(end, true, egui::Modifiers::NONE);
            let new_end = f.position(2.25);
            f.frame(
                vec![egui::Event::PointerMoved(new_end)],
                egui::Modifiers::NONE,
            );
            f.button(new_end, false, egui::Modifiers::NONE);
            assert_eq!(f.editor.loop_range, Some((1.25, 2.25)));
            assert!(!f.editor.scrubbing);
        }
    }

    #[test]
    fn disabled_ruler_and_tool_palette_do_not_seek() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut f = Fixture::new(layout);
            f.enabled = false;
            let pos = f.position(1.25);
            f.button(pos, true, egui::Modifiers::NONE);
            f.button(pos, false, egui::Modifiers::NONE);
            assert_eq!(f.editor.pending_seek, None);
            assert!(!f.owned);
        }
        let mut f = Fixture::new(EditorLayout::Vertical);
        let pos = f.rect.min + egui::vec2(15.0, 50.0);
        f.button(pos, true, egui::Modifiers::NONE);
        f.button(pos, false, egui::Modifiers::NONE);
        assert_eq!(f.editor.pending_seek, None);
    }

    #[test]
    fn hidden_chart_tools_allow_ruler_seek_under_former_palette() {
        let mut f = Fixture::new(EditorLayout::Vertical);
        f.editor.hide_chart_tools = true;
        let pos = f.rect.min + egui::vec2(15.0, 50.0);
        f.frame(vec![egui::Event::PointerMoved(pos)], egui::Modifiers::NONE);
        f.button(pos, true, egui::Modifiers::NONE);
        f.button(pos, false, egui::Modifiers::NONE);
        assert!(f.editor.pending_seek.is_some());
        assert!(!f.editor.scrubbing);
    }

    #[test]
    fn ruler_drag_keeps_its_time_mapping_while_playhead_follows() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut f = Fixture::new(layout);
            f.editor.follow_playhead = true;
            f.editor.follow_view_offset = Some(0.0);
            let start = f.position(1.25);
            let end = f.position(1.75);
            f.frame(
                vec![egui::Event::PointerMoved(start)],
                egui::Modifiers::NONE,
            );
            f.button(start, true, egui::Modifiers::NONE);
            f.frame(vec![egui::Event::PointerMoved(end)], egui::Modifiers::NONE);
            assert_eq!(f.editor.pending_seek, Some(1.75));
            f.now = 1.75;
            f.frame(Vec::new(), egui::Modifiers::NONE);
            assert_eq!(
                f.editor.pending_seek,
                Some(1.75),
                "stationary dragging must not drift"
            );
            f.button(end, false, egui::Modifiers::NONE);
            assert!(!f.editor.scrubbing);
        }
    }

    #[test]
    fn ruler_drag_releases_on_focus_loss_or_disable() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            for lose_focus in [false, true] {
                let mut f = Fixture::new(layout);
                let start = f.position(1.25);
                let end = f.position(1.75);
                f.frame(
                    vec![egui::Event::PointerMoved(start)],
                    egui::Modifiers::NONE,
                );
                f.button(start, true, egui::Modifiers::NONE);
                f.frame(vec![egui::Event::PointerMoved(end)], egui::Modifiers::NONE);
                assert!(f.editor.scrubbing);
                if lose_focus {
                    f.frame(
                        vec![egui::Event::WindowFocused(false)],
                        egui::Modifiers::NONE,
                    );
                } else {
                    f.enabled = false;
                    f.frame(Vec::new(), egui::Modifiers::NONE);
                }
                assert!(!f.editor.scrubbing);
            }
        }
    }
}
