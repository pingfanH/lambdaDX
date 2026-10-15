//! Reversible selection previews; history is committed only on confirmation.

use egui_macroquad::egui;

use crate::app::types::{ChartDoc, secs_to_measure};
use crate::state::{
    EditorLayout, EditorState, RepeatEdit, SelectionTransform, SelectionTransformKind,
};

use super::geometry::{JUDGE_MARGIN, LANE_COUNT, LANE_GUTTER, RULER_H, move_note, snap};

pub fn begin(
    editor: &mut EditorState,
    chart: &ChartDoc,
    kind: SelectionTransformKind,
    anchor: egui::Pos2,
) {
    editor.star_placement = None;
    editor.hold_placement = None;
    editor.placement_sequence.clear();
    editor.selection_transform = Some(SelectionTransform {
        keyboard_driven: false,
        kind,
        before: chart.clone(),
        was_dirty: editor.dirty,
        anchor: (anchor.x, anchor.y),
        wheel: 0.0,
        keyboard_time: 0.0,
        keyboard_lane: 0,
        duplicate_before: None,
        operation: RepeatEdit {
            kind,
            duplicate: false,
            time: 0.0,
            lanes: 0,
            factor: 1.0,
        },
    });
    editor.status = Some(format!("{kind:?}"));
}

pub fn duplicate(editor: &mut EditorState, chart: &mut ChartDoc, anchor: egui::Pos2) {
    let before = chart.clone();
    if !editor.duplicate_selection(chart) {
        return;
    }
    begin(editor, chart, SelectionTransformKind::Grab, anchor);
    let transform = editor.selection_transform.as_mut().unwrap();
    transform.duplicate_before = Some(before);
    transform.operation.duplicate = true;
    editor.dirty = true;
}

fn changed(editor: &EditorState, chart: &ChartDoc) -> bool {
    let Some(transform) = &editor.selection_transform else {
        return false;
    };
    transform
        .before
        .notes
        .iter()
        .filter(|n| editor.selected.contains(&n.id))
        .any(|old| {
            chart
                .notes
                .iter()
                .find(|n| n.id == old.id)
                .map(|n| {
                    n.time != old.time
                        || n.lane != old.lane
                        || n.hold_duration != old.hold_duration
                        || n.slide.iter().zip(&old.slide).any(|(new, old)| {
                            new.slide_duration != old.slide_duration
                                || new.slide_start_delay != old.slide_start_delay
                        })
                })
                .unwrap_or(true)
        })
}

pub fn finish(editor: &mut EditorState, chart: &mut ChartDoc, apply: bool) {
    let edited = changed(editor, chart);
    let Some(transform) = editor.selection_transform.take() else {
        return;
    };
    if let Some(before) = &transform.duplicate_before {
        if !apply {
            *chart = transform.before.clone();
        }
        editor.push_history(before);
        editor.last_edit = Some(if apply {
            transform.operation
        } else {
            RepeatEdit {
                time: 0.0,
                lanes: 0,
                ..transform.operation
            }
        });
    } else if apply && edited {
        editor.push_history(&transform.before);
        editor.last_edit = Some(transform.operation);
    } else {
        *chart = transform.before;
        editor.dirty = transform.was_dirty;
    }
    editor.pointer_press = None;
    editor.status = Some(format!(
        "{:?} {}",
        transform.kind,
        if apply { "applied" } else { "cancelled" }
    ));
}

pub fn shift(editor: &mut EditorState, chart: &mut ChartDoc, dt: f32, dl: i32) {
    let earliest = chart
        .notes
        .iter()
        .filter(|n| editor.selected.contains(&n.id))
        .map(|n| n.time)
        .fold(f32::INFINITY, f32::min);
    let dt = dt.max(1.0 - earliest);
    if let Some(transform) = &mut editor.selection_transform {
        transform.keyboard_time += dt;
        transform.keyboard_lane += dl;
        transform.operation.time += dt;
        transform.operation.lanes += dl;
    }
    for note in chart
        .notes
        .iter_mut()
        .filter(|n| editor.selected.contains(&n.id))
    {
        move_note(note, note.time + dt, dl);
    }
    if let Some(transform) = &editor.selection_transform {
        editor.dirty = transform.was_dirty || changed(editor, chart);
    }
}

pub fn scale(editor: &mut EditorState, chart: &mut ChartDoc, pointer: egui::Pos2) {
    let Some(transform) = &editor.selection_transform else {
        return;
    };
    let displacement = match editor.layout {
        EditorLayout::Vertical => transform.anchor.1 - pointer.y,
        EditorLayout::Horizontal => pointer.x - transform.anchor.0,
    };
    let factor = (1.0 + displacement / 200.0 + transform.wheel * 0.1).max(0.1);
    apply_scale(editor, chart, factor);
}

pub fn apply_scale(editor: &mut EditorState, chart: &mut ChartDoc, factor: f32) {
    let Some(transform) = &mut editor.selection_transform else {
        return;
    };
    transform.operation.factor = factor;
    let notes: Vec<_> = transform
        .before
        .notes
        .iter()
        .filter(|n| editor.selected.contains(&n.id))
        .collect();
    let earliest = notes.iter().map(|n| n.time).fold(f32::INFINITY, f32::min);
    let latest = notes
        .iter()
        .map(|n| n.time)
        .fold(f32::NEG_INFINITY, f32::max);
    let center = (earliest + latest) * 0.5;
    for original in notes {
        if let Some(note) = chart.notes.iter_mut().find(|n| n.id == original.id) {
            *note = original.clone();
            if factor != 1.0 {
                let time = snap(
                    center + (original.time - center) * factor,
                    editor.snap_enabled,
                    editor.snap_division,
                )
                .max(1.0);
                move_note(note, time, 0);
                note.hold_duration = original.hold_duration * factor;
                for slide in &mut note.slide {
                    slide.slide_start_delay *= factor;
                    slide.slide_duration *= factor;
                }
            }
        }
    }
    editor.dirty = transform.was_dirty || changed(editor, chart);
}

/// Derive every frame from the original selection, including accumulated wheel
/// steps. Pointer motion never compounds rounding or overwrites wheel input.
pub fn preview(
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    rect: egui::Rect,
    now: f32,
    pointer: egui::Pos2,
    wheel: f32,
) {
    let Some(transform) = &mut editor.selection_transform else {
        return;
    };
    transform.wheel += wheel;
    if transform.kind == SelectionTransformKind::Scale {
        scale(editor, chart, pointer);
        return;
    }
    let anchor = egui::pos2(transform.anchor.0, transform.anchor.1);
    let (time_axis, lane_axis, lane_size) = match editor.layout {
        EditorLayout::Vertical => (
            anchor.y - pointer.y,
            pointer.x - anchor.x,
            (rect.width() - LANE_GUTTER) / LANE_COUNT as f32,
        ),
        EditorLayout::Horizontal => (
            pointer.x - anchor.x,
            pointer.y - anchor.y,
            (rect.height() - RULER_H) / LANE_COUNT as f32,
        ),
    };
    let time_at = |pos: egui::Pos2| match editor.layout {
        EditorLayout::Vertical => {
            now + (rect.bottom() - JUDGE_MARGIN - pos.y) / editor.pixels_per_second
        }
        EditorLayout::Horizontal => {
            editor.view_start + (pos.x - rect.left() - LANE_GUTTER) / editor.pixels_per_second
        }
    };
    let step = 1.0 / editor.snap_division.max(1) as f32;
    let (mut dt, dl) = match transform.kind {
        SelectionTransformKind::Grab => (
            snap(
                secs_to_measure(time_at(pointer), &chart.bpms)
                    - secs_to_measure(time_at(anchor), &chart.bpms)
                    + transform.wheel * step
                    + transform.keyboard_time,
                editor.snap_enabled,
                editor.snap_division,
            ),
            (lane_axis / lane_size.max(1.0)).round() as i32 + transform.keyboard_lane,
        ),
        SelectionTransformKind::Rotate => (
            0.0,
            (lane_axis / lane_size.max(1.0) + time_axis / 80.0 + transform.wheel).round() as i32,
        ),
        SelectionTransformKind::Scale => unreachable!(),
    };
    let earliest = transform
        .before
        .notes
        .iter()
        .filter(|n| editor.selected.contains(&n.id))
        .map(|n| n.time)
        .fold(f32::INFINITY, f32::min);
    dt = dt.max(1.0 - earliest);
    transform.operation.time = dt;
    transform.operation.lanes = dl;
    for original in transform
        .before
        .notes
        .iter()
        .filter(|n| editor.selected.contains(&n.id))
    {
        if let Some(note) = chart.notes.iter_mut().find(|n| n.id == original.id) {
            *note = original.clone();
            move_note(note, original.time + dt, dl);
        }
    }
    editor.dirty = transform.was_dirty || changed(editor, chart);
}

pub fn repeat(editor: &mut EditorState, chart: &mut ChartDoc) {
    let Some(operation) = editor.last_edit else {
        editor.repeat_placement(chart);
        return;
    };
    if !chart.notes.iter().any(|n| editor.selected.contains(&n.id)) {
        return;
    }
    let before = chart.clone();
    if operation.duplicate && !editor.duplicate_selection(chart) {
        return;
    }
    begin(editor, chart, operation.kind, egui::Pos2::ZERO);
    if operation.duplicate {
        editor
            .selection_transform
            .as_mut()
            .unwrap()
            .duplicate_before = Some(before);
    }
    if operation.kind == SelectionTransformKind::Scale {
        apply_scale(editor, chart, operation.factor);
    } else {
        shift(editor, chart, operation.time, operation.lanes);
    }
    editor
        .selection_transform
        .as_mut()
        .unwrap()
        .operation
        .duplicate = operation.duplicate;
    finish(editor, chart, true);
}

/// Own timeline clicks while a transform is active, so confirmation cannot
/// also place a note or start a drag.
pub fn handle_pointer(
    ui: &egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
    wheel: f32,
) -> bool {
    if editor.transform_pointer_down {
        if !ui.input(|i| i.pointer.any_down()) {
            editor.transform_pointer_down = false;
        }
        return true;
    }
    if editor.selection_transform.is_none() {
        return false;
    }
    if editor.selection_transform.as_ref().is_some_and(|t| t.keyboard_driven) {
        if ui.input(|i| i.pointer.primary_pressed()) {
            finish(editor, chart, true);
            editor.transform_pointer_down = true;
        } else if ui.input(|i| i.pointer.secondary_pressed()) {
            finish(editor, chart, false);
            editor.transform_pointer_down = true;
        }
        return true;
    }
    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
        let wheel = if rect.contains(pos) { wheel } else { 0.0 };
        preview(editor, chart, rect, current_time, pos, wheel);
        if rect.contains(pos) && ui.input(|i| i.pointer.primary_pressed()) {
            finish(editor, chart, true);
            editor.transform_pointer_down = true;
        } else if ui.input(|i| i.pointer.secondary_pressed()) {
            finish(editor, chart, false);
            editor.transform_pointer_down = true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::zone::PadZone;
    use crate::app::types::{Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape};

    fn fixture() -> (EditorState, ChartDoc) {
        let notes = vec![
            Note {
                id: 1,
                time: 2.0,
                lane: 2,
                ..Default::default()
            },
            Note {
                id: 2,
                time: 4.0,
                lane: 6,
                ..Default::default()
            },
            Note {
                id: 3,
                time: 5.0,
                lane: 17,
                is_touch: true,
                note_type: NoteType::Touch,
                ..Default::default()
            },
        ];
        let mut editor = EditorState::default();
        editor.selected = [1, 2].into_iter().collect();
        let chart = ChartDoc {
            version: String::new(),
            title: String::new(),
            artist: String::new(),
            simai_level: 0,
            bpm: 120.0,
            bpms: Vec::new(),
            audio_offset: 0.0,
            notes,
            templates: Vec::new(),
            template_instances: Vec::new(),
            autoplay_overrides: Vec::new(),
        };
        (editor, chart)
    }

    #[test]
    fn repeat_scale_uses_same_factor_and_cancel_preserves_last_operation() {
        let (mut editor, mut chart) = fixture();
        editor.snap_enabled = false;
        chart.notes[0].hold_duration = 1.0;
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Scale,
            egui::Pos2::ZERO,
        );
        apply_scale(&mut editor, &mut chart, 1.5);
        finish(&mut editor, &mut chart, true);
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Grab,
            egui::Pos2::ZERO,
        );
        shift(&mut editor, &mut chart, 0.5, 1);
        finish(&mut editor, &mut chart, false);
        repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes[0].time, 1.0);
        assert_eq!(chart.notes[1].time, 5.25);
        assert_eq!(chart.notes[0].hold_duration, 2.25);
        assert_eq!(editor.undo.len(), 2);
        editor.undo(&mut chart);
        assert_eq!(chart.notes[0].hold_duration, 1.5);
    }

    #[test]
    fn repeat_move_targets_new_selection_and_respects_chart_start() {
        let (mut editor, mut chart) = fixture();
        editor.nudge_selection(&mut chart, -0.75, 1);
        editor.selected = [3].into_iter().collect();
        repeat(&mut editor, &mut chart);
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (4.25, 17));
        editor.selected = [1].into_iter().collect();
        repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes[0].time, 1.0);
        assert_eq!(chart.notes[0].lane, 4);
    }

    #[test]
    fn grab_follows_pointer_axes_and_preserves_wheel_offset_in_both_layouts() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let (mut editor, mut chart) = fixture();
            editor.layout = layout;
            editor.pixels_per_second = 100.0;
            editor.snap_division = 16;
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(930.0, 922.0));
            let anchor = egui::pos2(200.0, 200.0);
            begin(&mut editor, &chart, SelectionTransformKind::Grab, anchor);
            let pointer = match layout {
                EditorLayout::Vertical => anchor + egui::vec2(100.0, -100.0),
                EditorLayout::Horizontal => anchor + egui::vec2(100.0, 100.0),
            };
            preview(&mut editor, &mut chart, rect, 0.0, pointer, 1.0);
            assert_eq!((chart.notes[0].time, chart.notes[0].lane), (2.5625, 3));
            assert_eq!((chart.notes[1].time, chart.notes[1].lane), (4.5625, 7));
            preview(&mut editor, &mut chart, rect, 0.0, pointer, 0.0);
            assert_eq!(
                chart.notes[0].time, 2.5625,
                "wheel offset survives next frame"
            );
            preview(&mut editor, &mut chart, rect, 0.0, anchor, -1.0);
            finish(&mut editor, &mut chart, true);
            assert!(editor.undo.is_empty(), "return to original is a no-op");
        }
    }

    #[test]
    fn scale_is_time_only_including_hold_and_slide_spans() {
        let (mut editor, mut chart) = fixture();
        editor.snap_enabled = false;
        chart.notes[0].hold_duration = 1.0;
        chart.notes[1].slide = vec![Slide {
            segments: Vec::new(),
            slide_duration: 2.0,
            slide_start_delay: 0.25,
            slide_is_break: false,
            runtime_parts: 1,
        }];
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Scale,
            egui::Pos2::ZERO,
        );
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 900.0));
        preview(
            &mut editor,
            &mut chart,
            rect,
            0.0,
            egui::pos2(400.0, -100.0),
            1.0,
        );
        assert_eq!((chart.notes[0].lane, chart.notes[1].lane), (2, 6));
        assert!((chart.notes[0].hold_duration - 1.6).abs() < 1e-5);
        assert!((chart.notes[1].slide[0].slide_duration - 3.2).abs() < 1e-5);
        assert!((chart.notes[1].slide[0].slide_start_delay - 0.4).abs() < 1e-5);
        finish(&mut editor, &mut chart, false);
        assert_eq!(chart.notes[0].hold_duration, 1.0);
        assert_eq!(chart.notes[1].slide[0].slide_duration, 2.0);
    }

    #[test]
    fn rotate_moves_slide_path_and_touch_ring_without_changing_time() {
        let (mut editor, mut chart) = fixture();
        chart.notes[0].note_type = NoteType::Slide;
        chart.notes[0].slide = vec![Slide {
            segments: vec![SlideSegment {
                points: vec![SlidePoint::from(PadZone::from(8))],
                shape: SlideShape::Line,
            }],
            slide_duration: 1.0,
            slide_start_delay: 0.25,
            slide_is_break: false,
            runtime_parts: 1,
        }];
        chart.notes[2].lane = 16;
        editor.selected.insert(3);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(930.0, 922.0));
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Rotate,
            egui::Pos2::ZERO,
        );
        preview(
            &mut editor,
            &mut chart,
            rect,
            0.0,
            egui::pos2(100.0, 0.0),
            1.0,
        );
        assert_eq!((chart.notes[0].time, chart.notes[0].lane), (2.0, 4));
        assert_eq!(
            chart.notes[0].slide[0].segments[0].points[0].zone.to_id(),
            2
        );
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (5.0, 10));
        preview(
            &mut editor,
            &mut chart,
            rect,
            0.0,
            egui::pos2(100.0, 0.0),
            0.0,
        );
        assert_eq!(chart.notes[0].lane, 4);
        finish(&mut editor, &mut chart, true);
        assert_eq!(editor.undo.len(), 1);
        repeat(&mut editor, &mut chart);
        assert_eq!((chart.notes[0].time, chart.notes[0].lane), (2.0, 6));
        assert_eq!(
            chart.notes[0].slide[0].segments[0].points[0].zone.to_id(),
            4
        );
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (5.0, 12));
        assert_eq!(editor.undo.len(), 2);
        editor.undo(&mut chart);
        assert_eq!(chart.notes[0].lane, 4);
        editor.undo(&mut chart);
        assert_eq!(
            chart.notes[0].slide[0].segments[0].points[0].zone.to_id(),
            8
        );
    }

    #[test]
    fn grab_clamps_group_at_start_and_commits_one_undo() {
        let (mut editor, mut chart) = fixture();
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Grab,
            egui::Pos2::ZERO,
        );
        shift(&mut editor, &mut chart, -10.0, -3);
        assert_eq!((chart.notes[0].time, chart.notes[1].time), (1.0, 3.0));
        assert_eq!((chart.notes[0].lane, chart.notes[1].lane), (7, 3));
        shift(&mut editor, &mut chart, 0.25, 1);
        assert!(editor.undo.is_empty(), "preview is not an undo step");
        finish(&mut editor, &mut chart, true);
        assert_eq!(editor.undo.len(), 1);
        editor.undo(&mut chart);
        assert_eq!((chart.notes[0].time, chart.notes[1].time), (2.0, 4.0));
        editor.redo(&mut chart);
        assert_eq!((chart.notes[0].time, chart.notes[1].time), (1.25, 3.25));
        assert_eq!(chart.notes[2].time, 5.0);
    }

    #[test]
    fn cancel_preserves_dirty_and_redo_and_noop_has_no_history() {
        for dirty in [false, true] {
            let (mut editor, mut chart) = fixture();
            editor.dirty = dirty;
            editor.redo.push(chart.clone().into());
            begin(
                &mut editor,
                &chart,
                SelectionTransformKind::Grab,
                egui::Pos2::ZERO,
            );
            shift(&mut editor, &mut chart, 1.0, 2);
            finish(&mut editor, &mut chart, false);
            assert_eq!(chart.notes[0].time, 2.0);
            assert_eq!(chart.notes[0].lane, 2);
            assert_eq!(editor.dirty, dirty);
            assert_eq!(editor.redo.len(), 1);
            begin(
                &mut editor,
                &chart,
                SelectionTransformKind::Grab,
                egui::Pos2::ZERO,
            );
            shift(&mut editor, &mut chart, 0.25, 1);
            shift(&mut editor, &mut chart, -0.25, -1);
            finish(&mut editor, &mut chart, true);
            assert!(editor.undo.is_empty());
            assert_eq!(editor.redo.len(), 1);
            assert_eq!(editor.dirty, dirty);
        }
    }

    #[test]
    fn scale_uses_originals_and_visible_time_axis_in_both_layouts() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let (mut editor, mut chart) = fixture();
            editor.layout = layout;
            editor.snap_enabled = false;
            let anchor = egui::pos2(200.0, 200.0);
            begin(&mut editor, &chart, SelectionTransformKind::Scale, anchor);
            let pointer = match layout {
                EditorLayout::Vertical => egui::pos2(200.0, 123.0),
                EditorLayout::Horizontal => egui::pos2(277.0, 200.0),
            };
            scale(&mut editor, &mut chart, pointer);
            assert!((chart.notes[0].time - 1.615).abs() < 1e-5);
            assert!((chart.notes[1].time - 4.385).abs() < 1e-5);
            scale(&mut editor, &mut chart, pointer);
            assert!(
                (chart.notes[0].time - 1.615).abs() < 1e-5,
                "no accumulation"
            );
            scale(&mut editor, &mut chart, anchor);
            assert_eq!(chart.notes[0].time, 2.0);
            assert_eq!(chart.notes[0].lane, 2);
            finish(&mut editor, &mut chart, true);
            assert!(editor.undo.is_empty());
        }
    }

    #[test]
    fn scale_respects_snap_and_preserves_touch_sensor() {
        let (mut editor, mut chart) = fixture();
        editor.selected.insert(3);
        editor.snap_enabled = true;
        editor.snap_division = 4;
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Scale,
            egui::Pos2::ZERO,
        );
        scale(&mut editor, &mut chart, egui::pos2(0.0, -77.0));
        assert_eq!(chart.notes[2].lane, 17);
        assert_eq!(
            chart.notes[0].time * 4.0,
            (chart.notes[0].time * 4.0).round()
        );
    }

    #[test]
    fn grab_rotates_each_slides_own_path_and_touch_ring() {
        let (mut editor, mut chart) = fixture();
        for (note, end) in chart.notes.iter_mut().take(2).zip([8u8, 10]) {
            note.note_type = NoteType::Slide;
            note.slide = vec![Slide {
                segments: vec![SlideSegment {
                    points: vec![SlidePoint::from(PadZone::from(end))],
                    shape: SlideShape::Line,
                }],
                slide_duration: 1.0,
                slide_start_delay: 0.25,
                slide_is_break: false,
                runtime_parts: 1,
            }];
        }
        chart.notes[2].lane = 16;
        editor.selected.insert(3);
        begin(
            &mut editor,
            &chart,
            SelectionTransformKind::Grab,
            egui::Pos2::ZERO,
        );
        shift(&mut editor, &mut chart, 0.0, 1);
        assert_eq!(
            chart.notes[0].slide[0].segments[0].points[0].zone.to_id(),
            1
        );
        assert_eq!(
            chart.notes[1].slide[0].segments[0].points[0].zone.to_id(),
            11
        );
        assert_eq!(chart.notes[2].lane, 9);
        finish(&mut editor, &mut chart, false);
        assert_eq!(
            chart.notes[0].slide[0].segments[0].points[0].zone.to_id(),
            8
        );
        assert_eq!(
            chart.notes[1].slide[0].segments[0].points[0].zone.to_id(),
            10
        );
        assert_eq!(chart.notes[2].lane, 16);
    }
}
