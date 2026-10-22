//! Note timing handles shared by both timeline orientations.

use egui_macroquad::egui;

use crate::app::types::{ChartDoc, NoteType, measure_to_secs, secs_to_measure};
use crate::state::{DragHandle, EditorLayout, EditorState, HandleGrab};
use crate::theme::{ACCENT_BLUE, ACCENT_YELLOW, TEXT};

use super::geometry::{JUDGE_MARGIN, LANE_COUNT, LANE_GUTTER, RULER_H, lane_index_of, snap};

#[derive(Clone, Copy)]
struct View {
    rect: egui::Rect,
    layout: EditorLayout,
    origin: f32,
    pps: f32,
}

impl View {
    fn handle_position(self, time: f32, lane: usize, head_offset: f32) -> egui::Pos2 {
        let mut pos = self.position(time, lane);
        let offset = if lane == LANE_COUNT - 1 {
            -head_offset
        } else {
            head_offset
        };
        match self.layout {
            EditorLayout::Vertical => pos.x += offset,
            EditorLayout::Horizontal => pos.y += offset,
        }
        pos
    }

    fn position(self, time: f32, lane: usize) -> egui::Pos2 {
        let left = self.rect.left() + LANE_GUTTER;
        match self.layout {
            EditorLayout::Vertical => egui::pos2(
                left + (lane as f32 + 0.5) * (self.rect.right() - left) / LANE_COUNT as f32,
                self.rect.bottom() - JUDGE_MARGIN - (time - self.origin) * self.pps,
            ),
            EditorLayout::Horizontal => egui::pos2(
                left + (time - self.origin) * self.pps,
                self.rect.top()
                    + RULER_H
                    + (lane as f32 + 0.5) * (self.rect.height() - RULER_H) / LANE_COUNT as f32,
            ),
        }
    }

    fn time(self, pos: egui::Pos2) -> f32 {
        self.origin
            + match self.layout {
                EditorLayout::Vertical => (self.rect.bottom() - JUDGE_MARGIN - pos.y) / self.pps,
                EditorLayout::Horizontal => (pos.x - self.rect.left() - LANE_GUTTER) / self.pps,
            }
    }
}

/// Returns whether a handle owns the pointer, so note/playhead drags stay idle.
pub fn draw(
    ui: &egui::Ui,
    rect: egui::Rect,
    tools_rect: egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
) -> bool {
    if editor.star_placement.is_some()
        || editor.hold_placement.is_some()
        || editor.slide_draft.is_some()
    {
        return false;
    }
    let view = View {
        rect,
        layout: editor.layout,
        origin: match editor.layout {
            EditorLayout::Vertical => crate::keyboard::navigation::reference_time(editor, current_time),
            EditorLayout::Horizontal => editor.view_start,
        },
        pps: editor.pixels_per_second,
    };
    let mut handles = Vec::new();
    for note in &chart.notes {
        let lane = lane_index_of(note);
        match note.note_type {
            NoteType::Hold | NoteType::TouchHold => {
                handles.push((
                    note.id,
                    None,
                    DragHandle::Tail,
                    lane,
                    note.time,
                    0.0,
                    note.hold_duration,
                ));
            }
            NoteType::Slide => {
                for (idx, slide) in note.slide.iter().enumerate() {
                    if editor.editing_slide == Some(note.id) && editor.editing_slide_idx != idx {
                        continue;
                    }
                    for kind in [DragHandle::Delay, DragHandle::Tail] {
                        handles.push((
                            note.id,
                            Some(idx),
                            kind,
                            if kind == DragHandle::Tail {
                                slide
                                    .segments
                                    .iter()
                                    .flat_map(|s| &s.points)
                                    .map(|p| p.zone.to_id())
                                    .filter(|id| (1..=8).contains(id))
                                    .last()
                                    .map(super::geometry::zone_lane_index)
                                    .unwrap_or(lane)
                            } else {
                                lane
                            },
                            note.time,
                            slide.slide_start_delay,
                            slide.slide_duration,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    let mut owns_pointer = editor.handle_grab.is_some();
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    for (id, idx, kind, lane, head, delay, duration) in handles {
        let measure = head
            + if kind == DragHandle::Delay {
                delay
            } else {
                duration
            };
        let time = measure_to_secs(measure, &chart.bpms);
        // Slide delay handles stay beside their head; hold tails use judgment centres.
        let head_offset = if measure == head && idx.is_some() {
            let note = chart.notes.iter().find(|n| n.id == id).unwrap();
            super::notes::head_size(note)
                * editor.glyph_to_points
                * editor.note_scale()
                * 0.5
                + 8.0
        } else {
            0.0
        };
        let pos = view.handle_position(time, lane, head_offset);
        // Keep the floating palette above handle interaction and drawing.
        let active = editor
            .handle_grab
            .is_some_and(|g| g.id == id && g.slide_idx == idx && g.kind == kind);
        if tools_rect.contains(pos) && !active {
            continue;
        }
        let handle_rect = egui::Rect::from_center_size(pos, egui::vec2(16.0, 16.0));
        if !handle_rect.intersects(rect.intersect(ui.clip_rect())) && !active {
            continue;
        }
        let response = ui.interact(
            handle_rect,
            ui.id()
                .with(("timing_handle", id, idx, kind == DragHandle::Delay)),
            egui::Sense::click_and_drag(),
        );
        let color = if kind == DragHandle::Delay {
            ACCENT_BLUE
        } else {
            ACCENT_YELLOW
        };
        if head_offset != 0.0 {
            painter.line_segment(
                [view.position(time, lane), pos],
                egui::Stroke::new(1.0_f32, color),
            );
        }
        painter.circle_filled(pos, if response.hovered() { 6.0 } else { 4.5 }, color);
        painter.circle_stroke(pos, 6.0, egui::Stroke::new(1.0_f32, TEXT));
        let pressed = response.is_pointer_button_down_on();
        if response.hovered() || pressed || response.dragged() {
            owns_pointer = true;
        }
        if pressed && editor.handle_grab.is_none() {
            if !editor.selected.contains(&id) {
                editor.select_only(id);
                editor.editing_slide = None;
                editor.pending_shape = None;
            }
            let press = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
            editor.handle_grab = Some(HandleGrab {
                id,
                kind,
                slide_idx: idx,
                grab_measure: secs_to_measure(view.time(press), &chart.bpms),
                note_time: head,
                start_delay: delay,
                duration,
            });
            editor.drag_history_pushed = false;
        }
        if response.dragged() {
            if let Some(grab) = editor
                .handle_grab
                .filter(|g| g.id == id && g.slide_idx == idx && g.kind == kind)
            {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let cursor_measure = secs_to_measure(view.time(pointer), &chart.bpms);
                    resize(editor, chart, grab, cursor_measure);
                }
            }
        }
        let label = if kind == DragHandle::Delay {
            "Slide delay"
        } else {
            "Duration"
        };
        response.on_hover_text(match idx {
            Some(idx) => format!("{label} (slide {})", idx + 1),
            None => label.to_owned(),
        });
    }
    // Releasing outside the timeline must end the gesture too.
    if editor.handle_grab.is_some() && !ui.input(|i| i.pointer.primary_down()) {
        editor.handle_grab = None;
        editor.drag_history_pushed = false;
    }
    owns_pointer
}

fn resize(editor: &mut EditorState, chart: &mut ChartDoc, grab: HandleGrab, cursor_measure: f32) {
    let initial = if grab.kind == DragHandle::Delay {
        grab.start_delay
    } else {
        grab.duration
    };
    let end = snap(
        grab.note_time + initial + cursor_measure - grab.grab_measure,
        editor.snap_enabled,
        editor.snap_division,
    );
    let min_span = if grab.slide_idx.is_none() {
        0.0
    } else if editor.snap_enabled {
        1.0 / editor.snap_division.max(1) as f32
    } else {
        0.0001
    };
    let value = if grab.kind == DragHandle::Delay {
        (end - grab.note_time).clamp(0.0, (grab.duration - min_span).max(0.0))
    } else {
        (end - grab.note_time).max(grab.start_delay + min_span)
    };
    let Some(note) = chart.notes.iter().find(|n| n.id == grab.id) else {
        return;
    };
    let old = match grab.slide_idx {
        Some(idx) => {
            let Some(slide) = note.slide.get(idx) else {
                return;
            };
            if grab.kind == DragHandle::Delay {
                slide.slide_start_delay
            } else {
                slide.slide_duration
            }
        }
        None => note.hold_duration,
    };
    if (old - value).abs() < 1e-6 {
        return;
    }
    if !editor.drag_history_pushed {
        editor.push_history(chart);
        editor.drag_history_pushed = true;
    }
    let note = chart.notes.iter_mut().find(|n| n.id == grab.id).unwrap();
    if let Some(idx) = grab.slide_idx {
        let slide = &mut note.slide[idx];
        if grab.kind == DragHandle::Delay {
            slide.slide_start_delay = value;
        } else {
            slide.slide_duration = value;
        }
    } else {
        note.hold_duration = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        editor: &mut EditorState,
        chart: &mut ChartDoc,
        events: Vec<egui::Event>,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.interact(
                        rect(),
                        ui.id().with("timeline_host"),
                        egui::Sense::click_and_drag(),
                    );
                    draw(ui, rect(), egui::Rect::NOTHING, editor, chart, 0.0);
                });
            },
        );
    }

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(50.0, 60.0), egui::vec2(500.0, 500.0))
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn tail_drag_in_both_layouts_is_one_undoable_edit_and_releases_outside() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let ctx = egui::Context::default();
            let mut chart =
                crate::app::maidata::from_maidata("&inote_1=(120){4}1h[2:1],E", None).unwrap();
            let id = chart.notes[0].id;
            let mut editor = EditorState {
                layout,
                ..Default::default()
            };
            let view = View {
                rect: rect(),
                layout,
                origin: 0.0,
                pps: 120.0,
            };
            let start = view.position(1.0, 0);
            let end = view.position(1.5, 0);
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(start)],
            );
            frame(&ctx, &mut editor, &mut chart, vec![button(start, true)]);
            assert_eq!(editor.single_selected(), Some(id));
            assert!(editor.undo.is_empty(), "press does not edit");
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(end)],
            );
            assert_eq!(chart.notes[0].hold_duration, 0.75);
            frame(&ctx, &mut editor, &mut chart, vec![]);
            assert_eq!(editor.undo.len(), 1, "stationary drag does not add history");
            let outside = egui::pos2(750.0, 590.0);
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(outside), button(outside, false)],
            );
            assert!(editor.handle_grab.is_none());
            assert!(!editor.scrubbing);
            assert_eq!(chart.notes[0].time, 1.0);
            assert_eq!(chart.notes[0].lane, 1);
            editor.undo(&mut chart);
            assert_eq!(chart.notes[0].hold_duration, 0.5);
            editor.redo(&mut chart);
            assert_eq!(chart.notes[0].hold_duration, 0.75);
        }
    }

    #[test]
    fn slide_delay_drag_and_unsnapped_touchhold_tail_keep_grab_offset() {
        for (source, handle_time, target_time, expected, delay) in [
            ("&inote_1=(120){4}1-3[4:1],E", 0.5, 0.75, 0.375, true),
            ("&inote_1=(120){4}Ch[2:1],E", 1.0, 1.13, 0.565, false),
        ] {
            for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
                let ctx = egui::Context::default();
                let mut chart = crate::app::maidata::from_maidata(source, None).unwrap();
                let note = &chart.notes[0];
                let mut editor = EditorState {
                    layout,
                    pixels_per_second: 120.0,
                    snap_enabled: delay,
                    ..Default::default()
                };
                let view = View {
                    rect: rect(),
                    layout,
                    origin: 0.0,
                    pps: 120.0,
                };
                let offset = match layout {
                    EditorLayout::Vertical => egui::vec2(0.0, 3.0),
                    EditorLayout::Horizontal => egui::vec2(3.0, 0.0),
                };
                let start = view.position(handle_time, lane_index_of(note)) + offset;
                let end = view.position(target_time, lane_index_of(note)) + offset;
                frame(
                    &ctx,
                    &mut editor,
                    &mut chart,
                    vec![egui::Event::PointerMoved(start)],
                );
                frame(&ctx, &mut editor, &mut chart, vec![button(start, true)]);
                frame(
                    &ctx,
                    &mut editor,
                    &mut chart,
                    vec![egui::Event::PointerMoved(end)],
                );
                frame(&ctx, &mut editor, &mut chart, vec![button(end, false)]);
                let actual = if delay {
                    chart.notes[0].slide[0].slide_start_delay
                } else {
                    chart.notes[0].hold_duration
                };
                assert!(
                    (actual - expected).abs() < 1e-5,
                    "grab offset must not change duration: {actual}"
                );
                assert_eq!(editor.undo.len(), 1);
                assert!(editor.handle_grab.is_none());
            }
        }
    }

    #[test]
    fn zero_length_hold_handles_can_be_dragged_without_selecting_the_head() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            for tool in [crate::state::Tool::Hold, crate::state::Tool::TouchHold] {
                let ctx = egui::Context::default();
                let mut chart =
                    crate::app::maidata::from_maidata("&inote_1=(120){4}E", None).unwrap();
                let mut editor = EditorState {
                    layout,
                    tool,
                    ..Default::default()
                };
                let id = editor.place_note(&mut chart, 1.0, 3);
                editor.selected.clear();
                editor.undo.clear();
                let note = &chart.notes[0];
                let view = View {
                    rect: rect(),
                    layout,
                    origin: 0.0,
                    pps: editor.pixels_per_second,
                };
                let offset = 0.0;
                let start = view.handle_position(0.0, lane_index_of(note), offset);
                let end = view.handle_position(0.5, lane_index_of(note), offset);
                frame(
                    &ctx,
                    &mut editor,
                    &mut chart,
                    vec![egui::Event::PointerMoved(start)],
                );
                frame(&ctx, &mut editor, &mut chart, vec![button(start, true)]);
                assert!(editor.handle_grab.is_some());
                frame(
                    &ctx,
                    &mut editor,
                    &mut chart,
                    vec![egui::Event::PointerMoved(end)],
                );
                frame(&ctx, &mut editor, &mut chart, vec![button(end, false)]);
                assert_eq!(chart.notes[0].hold_duration, 0.25);
                assert_eq!(editor.single_selected(), Some(id));
                assert_eq!(editor.undo.len(), 1);
                editor.undo(&mut chart);
                assert_eq!(chart.notes[0].hold_duration, 0.0);
            }
        }
    }

    #[test]
    fn slide_tail_handle_uses_final_waypoint_lane_without_selection() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let ctx = egui::Context::default();
            let mut chart =
                crate::app::maidata::from_maidata("&inote_1=(120){4}1-3-5[4:2],E", None).unwrap();
            let mut editor = EditorState {
                layout,
                pixels_per_second: 120.0,
                ..Default::default()
            };
            let note = &chart.notes[0];
            let id = note.id;
            let duration = note.slide[0].slide_duration;
            let tail = measure_to_secs(note.time + duration, &chart.bpms);
            let view = View {
                rect: rect(),
                layout,
                origin: 0.0,
                pps: editor.pixels_per_second,
            };
            let head_lane = view.position(tail, lane_index_of(note));
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(head_lane)],
            );
            frame(&ctx, &mut editor, &mut chart, vec![button(head_lane, true)]);
            assert!(
                editor.handle_grab.is_none(),
                "no tail handle on the head lane"
            );
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![button(head_lane, false)],
            );
            let start = view.position(tail, 4);
            let end = view.position(tail + 0.5, 4);
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(start)],
            );
            frame(&ctx, &mut editor, &mut chart, vec![button(start, true)]);
            assert_eq!(editor.handle_grab.unwrap().slide_idx, Some(0));
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(end)],
            );
            frame(&ctx, &mut editor, &mut chart, vec![button(end, false)]);
            assert_eq!(chart.notes[0].slide[0].slide_duration, duration + 0.25);
            assert_eq!(chart.notes[0].lane, 1);
            assert_eq!(editor.single_selected(), Some(id));
            assert_eq!(editor.undo.len(), 1);
        }
    }

    #[test]
    fn pending_star_placement_does_not_capture_existing_timing_handles() {
        let ctx = egui::Context::default();
        let mut chart =
            crate::app::maidata::from_maidata("&inote_1=(120){4}1-3[4:1],E", None).unwrap();
        let mut editor = EditorState::default();
        assert!(editor.begin_same_head_placement(&chart, chart.notes[0].id));
        let view = View {
            rect: rect(),
            layout: editor.layout,
            origin: 0.0,
            pps: editor.pixels_per_second,
        };
        let start = view.position(0.5, 0);
        frame(
            &ctx,
            &mut editor,
            &mut chart,
            vec![egui::Event::PointerMoved(start)],
        );
        frame(&ctx, &mut editor, &mut chart, vec![button(start, true)]);
        assert!(editor.handle_grab.is_none());
        assert!(editor.undo.is_empty());
    }

    #[test]
    fn pending_slide_draft_cannot_drag_timing_handles() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let ctx = egui::Context::default();
            let mut chart =
                crate::app::maidata::from_maidata("&inote_1=(120){4}1,E", None).unwrap();
            let mut editor = EditorState {
                layout,
                tool: crate::state::Tool::Slide,
                pixels_per_second: 120.0,
                ..Default::default()
            };
            for time in [2.0, 2.25, 3.0] {
                editor.place_star_step(&mut chart, time, 3);
            }
            assert!(editor.slide_draft.is_some());
            let before = chart.clone();
            let note = chart.notes.last().unwrap();
            let tail_time = measure_to_secs(
                note.time + note.slide[0].slide_duration,
                &chart.bpms,
            );
            let origin = tail_time - 0.5;
            let view = View {
                rect: rect(),
                layout,
                origin,
                pps: editor.pixels_per_second,
            };
            let tail = view.position(tail_time, lane_index_of(note));
            let farther = view.position(tail_time + 0.5, lane_index_of(note));
            assert!(rect().contains(tail) && rect().contains(farther));
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(tail)],
            );
            frame(&ctx, &mut editor, &mut chart, vec![button(tail, true)]);
            frame(
                &ctx,
                &mut editor,
                &mut chart,
                vec![egui::Event::PointerMoved(farther)],
            );
            frame(&ctx, &mut editor, &mut chart, vec![button(farther, false)]);
            assert_eq!(chart, before);
            assert!(editor.handle_grab.is_none());
            assert!(editor.undo.is_empty());
            assert!(!editor.dirty);
        }
    }

    #[test]
    fn slide_resize_targets_one_subslide_and_keeps_positive_motion_span() {
        let mut chart =
            crate::app::maidata::from_maidata("&inote_1=(120){4}1-3[4:1],E", None).unwrap();
        let mut second = chart.notes[0].slide[0].clone();
        second.slide_duration += 0.25;
        chart.notes[0].slide.push(second);
        let note = &chart.notes[0];
        assert_eq!(note.slide.len(), 2);
        let untouched = (
            note.slide[0].slide_start_delay,
            note.slide[0].slide_duration,
        );
        let grab = HandleGrab {
            id: note.id,
            kind: DragHandle::Tail,
            slide_idx: Some(1),
            grab_measure: note.time + note.slide[1].slide_duration,
            note_time: note.time,
            start_delay: note.slide[1].slide_start_delay,
            duration: note.slide[1].slide_duration,
        };
        let mut editor = EditorState {
            snap_division: 16,
            ..Default::default()
        };
        resize(&mut editor, &mut chart, grab, 0.0);
        assert_eq!(
            chart.notes[0].slide[1].slide_duration,
            grab.start_delay + 1.0 / 16.0
        );
        let delay = HandleGrab {
            kind: DragHandle::Delay,
            duration: chart.notes[0].slide[1].slide_duration,
            grab_measure: grab.note_time + grab.start_delay,
            ..grab
        };
        resize(&mut editor, &mut chart, delay, 100.0);
        let slide = &chart.notes[0].slide[1];
        assert_eq!(slide.slide_start_delay, slide.slide_duration - 1.0 / 16.0);
        assert_eq!(
            (
                chart.notes[0].slide[0].slide_start_delay,
                chart.notes[0].slide[0].slide_duration
            ),
            untouched
        );
    }

    #[test]
    fn custom_subdivision_controls_tail_snap_and_can_be_disabled() {
        let mut chart =
            crate::app::maidata::from_maidata("&inote_1=(120){4}1h[4:1],E", None).unwrap();
        let note = &chart.notes[0];
        let grab = HandleGrab {
            id: note.id,
            kind: DragHandle::Tail,
            slide_idx: None,
            grab_measure: note.time + note.hold_duration,
            note_time: note.time,
            start_delay: 0.0,
            duration: note.hold_duration,
        };
        let mut editor = EditorState {
            snap_division: 7,
            ..Default::default()
        };
        resize(&mut editor, &mut chart, grab, 1.44);
        assert!((chart.notes[0].hold_duration - 3.0 / 7.0).abs() < 1e-6);
        editor.snap_enabled = false;
        resize(&mut editor, &mut chart, grab, 1.44);
        assert!((chart.notes[0].hold_duration - 0.44).abs() < 1e-6);
    }

    #[test]
    fn sensor_edit_hold_tail_can_return_to_zero_without_changing_zone() {
        for source in ["1h[4:1]", "B3h[4:1]"] {
            let mut chart = crate::app::maidata::from_maidata(
                &format!("&inote_1=(120){{4}}{source},E"), None).unwrap();
            let note = &chart.notes[0];
            let zone = note.lane;
            let grab = HandleGrab {
                id: note.id, kind: DragHandle::Tail, slide_idx: None,
                grab_measure: note.time + note.hold_duration, note_time: note.time,
                start_delay: 0.0, duration: note.hold_duration,
            };
            let mut editor = EditorState::default();
            resize(&mut editor, &mut chart, grab, grab.note_time);
            assert_eq!(chart.notes[0].hold_duration, 0.0);
            assert_eq!(chart.notes[0].lane, zone);
        }
    }

    #[test]
    fn pointer_mapping_roundtrips_across_bpm_changes_and_zoom() {
        let chart = crate::app::maidata::from_maidata("&inote_1=(120){4}1,,,,(240)2h[2:1],E", None)
            .unwrap();
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            for pps in [30.0, 120.0, 1600.0] {
                let view = View {
                    rect: rect(),
                    layout,
                    origin: 0.25,
                    pps,
                };
                for measure in [1.25, 2.0, 2.5] {
                    let pos = view.position(measure_to_secs(measure, &chart.bpms), 0);
                    assert!((secs_to_measure(view.time(pos), &chart.bpms) - measure).abs() < 1e-5);
                }
            }
        }
    }
}
