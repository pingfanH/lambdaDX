//! Pointer + keyboard interaction for the timeline.
//!
//! Input math works entirely in egui points (the macroquad renderers convert the
//! same rect with a scale factor), so a screen position maps to a (time, lane)
//! pair identically regardless of DPI.
//!
//! The two orientations differ only in how (time, lane) map to (x, y). Each
//! entry point precomputes a `markers` table of on-screen note positions, which
//! then drives both hit-testing and marquee selection; the shared state machine
//! in [`interact`] stays orientation-agnostic.

use egui_macroquad::egui;

use crate::app::types::zone::PadZone;
use crate::app::types::{
    ChartDoc, NoteType, mdur_to_secs, measure_to_secs, note_secs, secs_to_measure,
};
use crate::io::save_document;
use crate::state::{
    DragGrab, EditorLayout, EditorState, SelectionTransformKind, TimelinePress, Tool,
};

use super::geometry::{
    JUDGE_MARGIN, LANE_COUNT, LANE_GUTTER, MAX_PPS, MIN_PPS, RULER_H, lane_at, lane_index_of,
    move_note, paint_marquee, select_note, snap,
};

// ── Shared shortcuts ───────────────────────────────────────────────────────

/// Clipboard / undo / redo / flag shortcuts, called once per frame. Uses
/// `command` so Ctrl (Win/Linux) and Cmd (macOS) both work.
pub fn handle_edit_shortcuts(
    ui: &egui::Ui,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
) -> bool {
    if editor.timeline_pan.is_some() { return true; }
    if let Some(owned) = crate::keyboard::handle(ui, editor, chart, current_time) {
        return owned;
    }
    if ui.ctx().wants_keyboard_input() {
        return true;
    }
    if editor.transform_pointer_down {
        return true;
    }
    let cmd = ui.input(|i| i.modifiers.command);
    let shift = ui.input(|i| i.modifiers.shift);
    let plain = ui.input(|i| !i.modifiers.any());
    let pressed = |k| ui.input(|i| i.key_pressed(k));

    if let Some(kind) = editor.selection_transform.as_ref().map(|t| t.kind) {
        if pressed(egui::Key::Escape) || pressed(egui::Key::Enter) {
            super::transform::finish(editor, chart, !pressed(egui::Key::Escape));
        } else if kind == SelectionTransformKind::Grab && plain {
            let (dt, dl) = arrow_delta(ui, editor);
            if dt != 0.0 || dl != 0 {
                super::transform::shift(editor, chart, dt, dl);
            }
        }
        return true;
    }

    // A pointer gesture already owns its before-snapshot. Do not let undo,
    // nudging or a second modal operation mutate the same notes mid-drag.
    if (editor.dragging.is_some()
        || editor.handle_grab.is_some()
        || editor.marquee_start.is_some()
        || editor.scrubbing)
        && !pressed(egui::Key::Escape)
    {
        return true;
    }

    if cmd && !shift && pressed(egui::Key::Z) {
        editor.star_placement = None;
        editor.hold_placement = None;
        editor.undo(chart);
    }
    if (cmd && shift && pressed(egui::Key::Z)) || (cmd && pressed(egui::Key::Y)) {
        editor.redo(chart);
    }
    if cmd && pressed(egui::Key::C) {
        editor.copy_selection(chart);
    }
    if cmd && pressed(egui::Key::X) {
        editor.copy_selection(chart);
        editor.delete_selection(chart);
    }
    if cmd && pressed(egui::Key::V) {
        let measure = snap(
            secs_to_measure(current_time, &chart.bpms),
            editor.snap_enabled,
            editor.snap_division,
        )
        .max(1.0);
        editor.paste_at(chart, measure);
    }
    if cmd && pressed(egui::Key::S) {
        save_document(editor, chart);
    }
    if cmd && pressed(egui::Key::A) {
        if shift {
            editor.selected.clear();
        } else {
            editor.selected = chart.notes.iter().map(|n| n.id).collect();
        }
        editor.editing_slide = None;
        editor.pending_shape = None;
    }
    if cmd {
        return true;
    }
    if (editor.star_placement.is_some() || editor.hold_placement.is_some()) && !pressed(egui::Key::Escape) {
        return true;
    }
    let shift_only = ui.input(|i| i.modifiers.shift && !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.mac_cmd);
    if shift_only && pressed(egui::Key::D) {
        let anchor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(egui::Pos2::ZERO);
        super::transform::duplicate(editor, chart, anchor);
        return true;
    }
    if shift_only && pressed(egui::Key::R) {
        super::transform::repeat(editor, chart);
        return true;
    }
    if plain && pressed(egui::Key::E) {
        if editor.is_slide_edit() {
            editor.editing_slide = None;
            editor.pending_shape = None;
            return true;
        }
        if let Some(id) = editor.single_selected() {
            if let Some(note) = chart
                .notes
                .iter()
                .find(|n| n.id == id && n.note_type == NoteType::Slide)
            {
                editor.editing_slide = Some(id);
                editor.editing_slide_idx = editor
                    .editing_slide_idx
                    .min(note.slide.len().saturating_sub(1));
                editor.pending_shape = None;
                return true;
            }
        }
    }
    if (pressed(egui::Key::Delete)
        || (plain && pressed(egui::Key::D))
        || (pressed(egui::Key::Backspace) && !editor.is_slide_edit()))
        && !editor.selected.is_empty()
    {
        editor.delete_selection(chart);
        return true;
    }

    // `?` (Shift+/) or F1 toggles the shortcut-help overlay.
    if pressed(egui::Key::Questionmark) || pressed(egui::Key::F1) {
        editor.show_help = !editor.show_help;
    }

    // Slide ("star") track-edit mode: entered by middle-clicking a slide's trail
    // (see `interact`). While active, shape keys pick a shape and `1-8` applies
    // it, lane keys append waypoints, `⌫` undoes the last waypoint and `Esc`
    // exits. Editing the path takes over the plain keys, so it returns early.
    if editor.is_slide_edit() {
        use crate::app::types::SlideShape;
        // Shape keys (with their shifted / punctuation variants), then `1-8`
        // picks the end lane and applies the shape.
        let shape_key = if pressed(egui::Key::Q) && shift {
            Some(SlideShape::QQ)
        } else if pressed(egui::Key::Q) {
            Some(SlideShape::Q)
        } else if pressed(egui::Key::P) && shift {
            Some(SlideShape::PP)
        } else if pressed(egui::Key::P) {
            Some(SlideShape::P)
        } else if pressed(egui::Key::V) {
            Some(SlideShape::VShape)
        } else if pressed(egui::Key::W) {
            Some(SlideShape::Wifi)
        } else if pressed(egui::Key::Minus) {
            Some(SlideShape::Line)
        } else if pressed(egui::Key::Comma) {
            Some(SlideShape::Left)
        } else if pressed(egui::Key::Period) {
            Some(SlideShape::Right)
        } else if pressed(egui::Key::Semicolon) {
            Some(SlideShape::Caret)
        } else if pressed(egui::Key::S) {
            Some(SlideShape::S)
        } else if pressed(egui::Key::Z) {
            Some(SlideShape::Z)
        } else {
            None
        };
        if let Some(shape) = shape_key {
            editor.pending_shape = Some(shape);
            editor.status = Some(format!("Shape {shape:?} — press 1-8 for the end lane"));
        }

        let lane_key = if pressed(egui::Key::Num1) {
            Some(1u8)
        } else if pressed(egui::Key::Num2) {
            Some(2)
        } else if pressed(egui::Key::Num3) {
            Some(3)
        } else if pressed(egui::Key::Num4) {
            Some(4)
        } else if pressed(egui::Key::Num5) {
            Some(5)
        } else if pressed(egui::Key::Num6) {
            Some(6)
        } else if pressed(egui::Key::Num7) {
            Some(7)
        } else if pressed(egui::Key::Num8) {
            Some(8)
        } else {
            None
        };
        if let Some(lane) = lane_key {
            if let Some(shape) = editor.pending_shape.take() {
                if editor.apply_slide_shape(chart, shape, lane) {
                    editor.status = Some(format!("Set shape {shape:?} → lane {lane}"));
                } else {
                    editor.status = Some(format!("Shape {shape:?} can't end at lane {lane}"));
                }
            } else if editor.append_slide_waypoint(chart, PadZone::from(lane)) {
                editor.status = Some(format!("Added zone {lane}"));
            }
        }

        if pressed(egui::Key::Backspace) && editor.pop_slide_waypoint(chart) {
            editor.status = Some("Removed last waypoint".to_string());
        }
        if pressed(egui::Key::Escape) {
            editor.editing_slide = None;
            editor.pending_shape = None;
            editor.status = Some("Slide edit: off".to_string());
        }
        return true;
    }
    if plain && pressed(egui::Key::Escape) {
        editor.selected.clear();
        editor.tool = Tool::Select;
        editor.star_placement = None;
        editor.hold_placement = None;
        editor.dragging = None;
        editor.drag_grab = None;
        editor.handle_grab = None;
        editor.marquee_start = None;
        editor.pointer_press = None;
        editor.scrubbing = false;
        editor.drag_history_pushed = false;
        editor.drag_multi_orig.clear();
        editor.drag_orig_notes.clear();
        editor.marquee_base.clear();
        return true;
    }
    if plain && !editor.selected.is_empty() {
        let kind = if pressed(egui::Key::G) {
            Some(SelectionTransformKind::Grab)
        } else if pressed(egui::Key::S) {
            Some(SelectionTransformKind::Scale)
        } else if pressed(egui::Key::R) {
            Some(SelectionTransformKind::Rotate)
        } else {
            None
        };
        if let Some(kind) = kind {
            let anchor = ui
                .input(|i| i.pointer.hover_pos())
                .unwrap_or(ui.max_rect().center());
            super::transform::begin(editor, chart, kind, anchor);
            return true;
        }
    }
    if plain && pressed(egui::Key::P) {
        editor.autoplay_mode = (editor.autoplay_mode + 1) % 4;
        editor.mode_dirty = true;
    }

    if plain && pressed(egui::Key::O) {
        editor.autoplay_mode = if editor.autoplay_mode == 1 { 2 } else { 1 };
        editor.mode_dirty = true;
    }

    // Arrow nudges follow the visible axes in either layout.
    if plain && !editor.selected.is_empty() {
        let (d_time, d_lane) = arrow_delta(ui, editor);
        if d_time != 0.0 || d_lane != 0 {
            editor.nudge_selection(chart, d_time, d_lane);
        }
    }

    if !editor.selected.is_empty() {
        if plain && pressed(egui::Key::B) {
            editor.edit_selected(chart, |n| n.is_break = !n.is_break);
        }
        if plain && pressed(egui::Key::X) {
            editor.edit_selected(chart, |n| n.is_ex = !n.is_ex);
        }
        if plain && pressed(egui::Key::N) {
            editor.edit_selected(chart, |n| {
                let on = !(n.is_break && n.is_ex);
                n.is_break = on;
                n.is_ex = on;
            });
        }
    }
    false
}

fn arrow_delta(ui: &egui::Ui, editor: &EditorState) -> (f32, i32) {
    let step = 1.0 / editor.snap_division.max(1) as f32;
    let pressed = |key| ui.input(|i| i.key_pressed(key));
    let (earlier, later, prev_lane, next_lane) = match editor.layout {
        EditorLayout::Vertical => (
            egui::Key::ArrowDown,
            egui::Key::ArrowUp,
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
        ),
        EditorLayout::Horizontal => (
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
        ),
    };
    if pressed(earlier) {
        (-step, 0)
    } else if pressed(later) {
        (step, 0)
    } else if pressed(prev_lane) {
        (0.0, -1)
    } else if pressed(next_lane) {
        (0.0, 1)
    } else {
        (0.0, 0)
    }
}

// ── Shared drag / click state machine ───────────────────────────────────────

/// Cursor resolved to the timeline's (time, lane) space.
struct Cursor {
    time: f32,
    lane: u8,
    /// Whether the cursor is on the grabbable playhead / judgment line.
    on_playhead: bool,
}

/// One on-screen note marker: id → position, used for hit-test and marquee.
type Markers = Vec<(u64, egui::Rect)>;

pub(crate) fn head_rect(
    editor: &EditorState,
    note: &crate::app::types::Note,
    center: egui::Pos2,
) -> egui::Rect {
    let size = super::notes::head_size(note)
        * editor.glyph_to_points
        * crate::app::params::get().editor_note_scale.clamp(0.05, 8.0);
    egui::Rect::from_center_size(center, egui::vec2(size, size))
}

fn interact(
    response: &egui::Response,
    ui: &egui::Ui,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    cursor: Cursor,
    markers: &Markers,
) {
    // Note under the pointer, for the hover ring + read-out. Skipped mid-drag so
    // the read-out tracks the dragged note rather than the raw cursor.
    if !response.dragged_by(egui::PointerButton::Primary) {
        editor.hovered = response
            .hover_pos()
            .and_then(|p| nearest_marker(markers, p));
    }
    if response.contains_pointer() && ui.input(|i| i.pointer.primary_pressed()) {
        let hit = response.interact_pointer_pos().and_then(|p| nearest_marker(markers, p));
        crate::keyboard::navigation::pointer_anchor(editor, chart, cursor.time, cursor.lane, hit);
    }
    if editor.star_placement.is_some() || editor.hold_placement.is_some() {
        if response.clicked() {
            let measure = snap(
                secs_to_measure(cursor.time, &chart.bpms),
                editor.snap_enabled,
                editor.snap_division,
            ).max(1.0);
            if editor.hold_placement.is_some() {
                editor.place_hold_step(chart, measure, cursor.lane);
            } else {
                editor.place_star_step(chart, measure, cursor.lane);
            }
        }
        return;
    }
    if response.contains_pointer() && ui.input(|i| i.pointer.primary_pressed()) {
        if let Some(pos) = response.interact_pointer_pos() {
            let note = nearest_marker(markers, pos);
            editor.pointer_press = Some(TimelinePress {
                note,
                position: (pos.x, pos.y),
                measure: secs_to_measure(cursor.time, &chart.bpms),
                lane: cursor.lane as i32,
                on_playhead: cursor.on_playhead && note.is_none() && editor.tool == Tool::Select,
            });
        }
    }

    if response.drag_started_by(egui::PointerButton::Primary) {
        let additive = ui.input(|i| i.modifiers.shift);
        let press = editor.pointer_press;
        editor.scrubbing = press.map(|p| p.on_playhead).unwrap_or(cursor.on_playhead);
        editor.dragging = press.and_then(|p| p.note);
        editor.drag_history_pushed = false;
        editor.marquee_start = None;
        editor.drag_grab = None;
        editor.drag_multi_orig.clear();
        editor.drag_orig_notes.clear();
        if let Some(id) = editor.dragging {
            // Grabbing a note that is already part of a multi-selection keeps
            // that selection and moves the whole group (matching the original
            // editor). Otherwise the drag selects just the grabbed note, adding
            // it when Shift is held.
            if additive {
                editor.selected.insert(id);
            } else if !editor.selected.contains(&id) {
                editor.select_only(id);
            }
            editor.drag_grab = chart.notes.iter().find(|n| n.id == id).map(|n| DragGrab {
                note_time: n.time,
                note_lane: n.lane,
                grab_measure: press
                    .map(|p| p.measure)
                    .unwrap_or_else(|| secs_to_measure(cursor.time, &chart.bpms)),
                grab_lane: press.map(|p| p.lane).unwrap_or(cursor.lane as i32),
            });
            editor.drag_multi_orig = chart
                .notes
                .iter()
                .filter(|n| editor.selected.contains(&n.id))
                .map(|n| (n.id, n.time, n.lane))
                .collect();
            editor.drag_orig_notes = chart
                .notes
                .iter()
                .filter(|n| editor.selected.contains(&n.id))
                .cloned()
                .collect();
            reconcile_selection(editor);
        } else if !editor.scrubbing {
            editor.marquee_start = press.map(|p| p.position);
            editor.marquee_subtract = ui.input(|i| i.modifiers.command);
            editor.marquee_base = if additive || editor.marquee_subtract {
                editor.selected.clone()
            } else {
                Default::default()
            };
            editor.selected = editor.marquee_base.clone();
        }
    }

    if editor.scrubbing && (response.dragged_by(egui::PointerButton::Primary)
        || response.drag_stopped_by(egui::PointerButton::Primary)) {
        editor.pending_seek = Some(cursor.time.max(0.0));
        if response.dragged_by(egui::PointerButton::Primary) { return; }
    }

    if response.dragged_by(egui::PointerButton::Primary) || response.drag_stopped_by(egui::PointerButton::Primary) {
        if let Some((sx, sy)) = editor.marquee_start {
            if let Some(p) = response.interact_pointer_pos() {
                let sel_rect = egui::Rect::from_two_pos(egui::pos2(sx, sy), p);
                paint_marquee(ui, sel_rect);
                editor.selected = editor.marquee_base.clone();
                for (id, bounds) in markers {
                    if sel_rect.intersects(*bounds) {
                        if editor.marquee_subtract {
                            editor.selected.remove(id);
                        } else {
                            editor.selected.insert(*id);
                        }
                    }
                }
                reconcile_selection(editor);
            }
            if response.dragged_by(egui::PointerButton::Primary) {
                return;
            }
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        if let Some(id) = editor.dragging {
            let cur_measure = secs_to_measure(cursor.time, &chart.bpms);
            match editor.drag_grab {
                Some(g) => {
                    let t = snap(
                        g.note_time + cur_measure - g.grab_measure,
                        editor.snap_enabled,
                        editor.snap_division,
                    );
                    let earliest = editor
                        .drag_orig_notes
                        .iter()
                        .map(|n| n.time)
                        .fold(f32::INFINITY, f32::min);
                    let dt = (t - g.note_time).max(1.0 - earliest);
                    let dl = cursor.lane as i32 - g.grab_lane;
                    if dt == 0.0 && dl == 0 && !editor.drag_history_pushed {
                        return;
                    }
                    if !editor.drag_history_pushed {
                        editor.push_history(chart);
                        editor.drag_history_pushed = true;
                    }
                    for original in &editor.drag_orig_notes {
                        if let Some(note) = chart.notes.iter_mut().find(|n| n.id == original.id) {
                            *note = original.clone();
                            move_note(note, original.time + dt, dl);
                        }
                    }
                }
                None => {
                    if !editor.drag_history_pushed {
                        editor.push_history(chart);
                        editor.drag_history_pushed = true;
                    }
                    if let Some(note) = chart.notes.iter_mut().find(|n| n.id == id) {
                        note.time =
                            snap(cur_measure, editor.snap_enabled, editor.snap_division).max(1.0);
                        if !crate::app::types::note_is_touch(note) {
                            note.lane = cursor.lane.clamp(1, 8);
                        }
                    }
                }
            }
        }
    }

    if response.drag_stopped_by(egui::PointerButton::Primary) {
        editor.dragging = None;
        editor.drag_history_pushed = false;
        editor.scrubbing = false;
        editor.marquee_start = None;
        editor.drag_grab = None;
        editor.drag_multi_orig.clear();
        editor.drag_orig_notes.clear();
        editor.pointer_press = None;
        editor.marquee_base.clear();
    }
    if ui.input(|i| i.pointer.primary_released()) {
        editor.pointer_press = None;
    }

    // Right-click picks the note under the pointer and opens its menu; the
    // marker count is small, so a straight linear scan is fine.
    let context_pos = response.interact_pointer_pos();
    if response.secondary_clicked() {
        if let Some(id) = context_pos.and_then(|pos| nearest_marker(markers, pos)) {
            if !editor.selected.contains(&id) {
                editor.select_only(id);
                reconcile_selection(editor);
            }
        }
    }
    response.context_menu(|ui| {
        let hit = context_pos.and_then(|pos| nearest_marker(markers, pos));
        if let Some(id) = hit {
            ui.label(format!("Note {id}"));
        }
        ui.add_enabled_ui(!editor.selected.is_empty(), |ui| {
            if ui.button("删除").clicked() {
                editor.delete_selection(chart);
                reconcile_selection(editor);
                ui.close_menu();
            }
        });
    });

    // A double-click on a star head pulls out a new same-head trajectory.
    // Single head clicks select; middle/double trail clicks edit that trail.
    if response.double_clicked() && editor.tool != Tool::Eraser {
        if let Some(id) =
            nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default())
        {
            if editor.begin_same_head_placement(chart, id) {
                return;
            }
        }
    }

    if response.middle_clicked() || response.double_clicked() {
        if nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default()).is_none() {
            if let Some((id, sub)) = slide_trail_at_pointer(
                chart,
                editor,
                response.rect,
                response.interact_pointer_pos().unwrap_or_default(),
                cursor.time,
            ) {
                editor.select_only(id);
                editor.editing_slide = Some(id);
                editor.editing_slide_idx = sub;
                editor.pending_shape = None;
                editor.status = Some(
                "Slide edit: click pad/lane zones to add · shape key + 1-8 · Backspace=undo · Esc=exit"
                    .to_string(),
            );
            }
        }
    }

    if response.clicked() {
        let hit = nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default());
        if cursor.on_playhead && hit.is_none() && editor.tool == Tool::Select {
            editor.pending_seek = Some(cursor.time.max(0.0));
            return;
        }
        if let Some(id) = hit {
            if ui.input(|i| i.modifiers.command) {
                if !editor.selected.remove(&id) {
                    editor.selected.insert(id);
                }
            } else {
                select_note(editor, id, ui.input(|i| i.modifiers.shift));
            }
            if editor.editing_slide.is_some() && editor.single_selected() != editor.editing_slide {
                editor.editing_slide = None;
                editor.pending_shape = None;
            }
            if editor.tool == Tool::Eraser {
                editor.delete_note(chart, id);
            }
            return;
        }
        if editor.tool == Tool::Select
            && !editor.is_slide_edit()
            && !ui.input(|i| i.modifiers.shift || i.modifiers.command)
        {
            editor.selected.clear();
        }
        // A placement tool drops a note. A blank click with the Select tool is
        // deliberately a no-op: clicking empty timeline space no longer seeks
        // (seeking is the playhead/overview's job). Slide-edit mode never places.
        if !editor.is_slide_edit()
            && matches!(
                editor.tool,
                Tool::Tap | Tool::Hold | Tool::Touch | Tool::TouchHold | Tool::Slide | Tool::Break
            )
        {
            let measure = snap(
                secs_to_measure(cursor.time, &chart.bpms),
                editor.snap_enabled,
                editor.snap_division,
            )
            .max(1.0);
            if editor.tool == Tool::Slide {
                editor.place_star_step(chart, measure, cursor.lane);
            } else if matches!(editor.tool, Tool::Hold | Tool::TouchHold) {
                editor.place_hold_step(chart, measure, cursor.lane);
            } else {
                editor.place_note(chart, measure, cursor.lane);
            }
        }
    }
}

fn reconcile_selection(editor: &mut EditorState) {
    if editor.editing_slide.is_some() && editor.single_selected() != editor.editing_slide {
        editor.editing_slide = None;
        editor.pending_shape = None;
    }
}

/// The slide note (and which sub-slide) whose trail is under `(time, lane)`,
/// i.e. a slide in the same lane whose head..tail span contains the cursor time.
/// Used to enter slide-edit mode from a middle-click on the star's trajectory.
#[cfg(test)]
fn slide_trail_at(chart: &ChartDoc, time: f32, lane: u8) -> Option<(u64, usize)> {
    const MARGIN: f32 = 0.05;
    let mut best: Option<(u64, usize, f32)> = None;
    for n in chart
        .notes
        .iter()
        .filter(|n| n.note_type == NoteType::Slide && n.lane == lane)
    {
        let head = note_secs(n, &chart.bpms);
        // A slide with no sub-slides still has a grabbable head.
        if n.slide.is_empty() && (time - head).abs() <= MARGIN {
            if best.map(|(_, _, bd)| MARGIN < bd).unwrap_or(true) {
                best = Some((n.id, 0, MARGIN));
            }
            continue;
        }
        for (si, sl) in n.slide.iter().enumerate() {
            let dur_s = mdur_to_secs(sl.slide_duration, n.time, &chart.bpms).max(0.0);
            let delay_s = mdur_to_secs(sl.slide_start_delay, n.time, &chart.bpms)
                .max(0.0)
                .min(dur_s);
            let start = head + delay_s;
            if time >= start - MARGIN && time <= head + dur_s + MARGIN {
                let d = (time - start).abs();
                if best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                    best = Some((n.id, si, d));
                }
            }
        }
    }
    best.map(|(id, si, _)| (id, si))
}

fn slide_trail_at_pointer(
    chart: &ChartDoc,
    editor: &EditorState,
    rect: egui::Rect,
    pointer: egui::Pos2,
    cursor_time: f32,
) -> Option<(u64, usize)> {
    let position = |time: f32, lane: usize| match editor.layout {
        EditorLayout::Vertical => egui::pos2(
            rect.left()
                + LANE_GUTTER
                + (lane as f32 + 0.5) * (rect.width() - LANE_GUTTER) / LANE_COUNT as f32,
            pointer.y - (time - cursor_time) * editor.pixels_per_second,
        ),
        EditorLayout::Horizontal => egui::pos2(
            pointer.x + (time - cursor_time) * editor.pixels_per_second,
            rect.top()
                + RULER_H
                + (lane as f32 + 0.5) * (rect.height() - RULER_H) / LANE_COUNT as f32,
        ),
    };
    let radius =
        7.0 * editor.glyph_to_points * crate::app::params::get().editor_note_scale.clamp(0.05, 8.0)
            + 3.0;
    let mut best = None;
    let mut distance = radius * radius;
    for note in chart
        .notes
        .iter()
        .filter(|n| n.note_type == NoteType::Slide)
    {
        let head = note_secs(note, &chart.bpms);
        for (index, slide) in note.slide.iter().enumerate() {
            let duration = mdur_to_secs(slide.slide_duration, note.time, &chart.bpms).max(0.0);
            let delay =
                mdur_to_secs(slide.slide_start_delay, note.time, &chart.bpms).clamp(0.0, duration);
            let mut start = position(head + delay, lane_index_of(note));
            let zones: Vec<_> = slide
                .segments
                .iter()
                .flat_map(|s| &s.points)
                .map(|p| p.zone.to_id())
                .filter(|id| (1..=8).contains(id))
                .collect();
            let count = zones.len().max(1);
            for i in 0..count {
                let lane = zones
                    .get(i)
                    .map(|id| *id as usize - 1)
                    .unwrap_or_else(|| lane_index_of(note));
                let end = position(
                    head + delay + (duration - delay) * (i + 1) as f32 / count as f32,
                    lane,
                );
                let segment = end - start;
                let fraction = ((pointer - start).dot(segment)
                    / segment.length_sq().max(f32::EPSILON))
                .clamp(0.0, 1.0);
                let d = (pointer - (start + segment * fraction)).length_sq();
                if d <= distance {
                    distance = d;
                    best = Some((note.id, index));
                }
                start = end;
            }
        }
    }
    best
}

/// Hit the full rendered head, resolving overlapping heads by centre distance.
fn nearest_marker(markers: &Markers, at: egui::Pos2) -> Option<u64> {
    markers
        .iter()
        .filter(|(_, bounds)| bounds.contains(at))
        .min_by(|a, b| {
            let da = (a.1.center() - at).length_sq();
            let db = (b.1.center() - at).length_sq();
            da.total_cmp(&db).then_with(|| a.0.cmp(&b.0))
        })
        .map(|(id, _)| *id)
}

/// Marquee edge-autoscroll pan factor for a pointer at `p` on an axis spanning
/// `lo..hi`: `+1` at the `hi` edge, `-1` at the `lo` edge, `0` in the middle
/// band, interpolated within the edge bands.
fn edge_pan(p: f32, lo: f32, hi: f32) -> f32 {
    const EDGE: f32 = 28.0;
    if p < lo + EDGE {
        -((lo + EDGE - p) / EDGE).min(1.0)
    } else if p > hi - EDGE {
        ((p - (hi - EDGE)) / EDGE).min(1.0)
    } else {
        0.0
    }
}

/// Seconds the view advances per frame at full marquee-autoscroll tilt.
const AUTOSCROLL_SECS: f32 = 0.08;

pub(crate) fn scroll_step(offset: f32, dt: f32) -> f32 {
    (offset * 6.0).clamp(-1200.0, 1200.0) * dt.clamp(0.0, 0.05)
}

pub(crate) fn rate_pan_step(offset: f32, dt: f32) -> f32 {
    // A small neutral zone prevents pointer jitter from starting a scroll.
    scroll_step(offset.signum() * (offset.abs() - 4.0).max(0.0), dt)
}

fn view_time(editor: &EditorState, now: f32) -> f32 {
    super::geometry::view_reference(editor, now)
}

fn pan_view(editor: &mut EditorState, delta: f32, now: f32) {
    if delta == 0.0 { return; }
    let view = view_time(editor, now);
    if editor.follow_playhead && !editor.keyboard.detached {
        let now = editor.pending_seek.unwrap_or(now);
        let target = (now + delta).max(0.0);
        editor.follow_view_offset = Some(now - view);
        editor.view_start = view + target - now;
        if target != now { editor.pending_seek = Some(target); }
    } else {
        editor.view_start = view + delta;
    }
}

fn scrub_position(
    response: &egui::Response, ui: &egui::Ui, rect: egui::Rect,
    editor: &mut EditorState, now: f32, mut pos: egui::Pos2,
) -> egui::Pos2 {
    let dragging = response.dragged_by(egui::PointerButton::Primary);
    if (dragging || response.drag_stopped_by(egui::PointerButton::Primary))
        && (editor.scrubbing || editor.pointer_press.is_some_and(|press| press.on_playhead)) {
        let (axis, lo, hi, sign) = match editor.layout {
            EditorLayout::Horizontal => (&mut pos.x, rect.left() + LANE_GUTTER, rect.right(), 1.0),
            EditorLayout::Vertical => (&mut pos.y, rect.top() + RULER_H, rect.bottom(), -1.0),
        };
        let edge = (*axis).clamp(lo, hi.max(lo));
        let step = if dragging { scroll_step((*axis - edge) * sign, ui.input(|i| i.stable_dt)) } else { 0.0 };
        editor.view_start = view_time(editor, now) + step / editor.pixels_per_second;
        editor.scrubbing = true;
        *axis = edge;
        if step != 0.0 { ui.ctx().request_repaint(); }
    }
    pos
}

// ── Horizontal timeline (time → right) ──────────────────────────────────────

/// Pointer handling for the horizontal timeline. `rect` is in egui points.
pub fn horizontal(
    response: &egui::Response,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
    ui: &egui::Ui,
) {
    let plot_left = rect.left() + LANE_GUTTER;
    let lanes_top = rect.top() + RULER_H;
    let lane_h = (rect.bottom() - lanes_top) / LANE_COUNT as f32;

    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let pos = scrub_position(response, ui, rect, editor, current_time, pos);
    let time = editor.view_start + (pos.x - plot_left) / editor.pixels_per_second;
    let lane = lane_at(pos.y - lanes_top, lane_h);
    let play_x = plot_left + (current_time - editor.view_start) * editor.pixels_per_second;
    let on_playhead = (pos.x - play_x).abs() <= 6.0;
    if let (Some(start), Some(press)) = (&mut editor.marquee_start, editor.pointer_press) {
        start.0 = plot_left
            + (measure_to_secs(press.measure, &chart.bpms) - editor.view_start)
                * editor.pixels_per_second;
    }

    let markers: Markers = chart
        .notes
        .iter()
        .map(|n| {
            let x = plot_left
                + (note_secs(n, &chart.bpms) - editor.view_start) * editor.pixels_per_second;
            let y = lanes_top + (lane_index_of(n) as f32 + 0.5) * lane_h;
            (n.id, head_rect(editor, n, egui::pos2(x, y)))
        })
        .collect();

    interact(
        response,
        ui,
        editor,
        chart,
        Cursor {
            time,
            lane,
            on_playhead,
        },
        &markers,
    );

    // Marquee edge-autoscroll: while box-selecting, pan toward the edge the
    // pointer is pushed against (later time = right).
    if editor.marquee_start.is_some() {
        if let Some(p) = response.interact_pointer_pos() {
            let f = edge_pan(p.x, rect.left(), rect.right());
            if f != 0.0 {
                pan_view(editor, f * AUTOSCROLL_SECS, current_time);
            }
        }
    }
}

// ── Vertical timeline (time flows up from the judgment line) ─────────────────

/// Pointer handling for the vertical timeline. `rect` is in egui points.
pub fn vertical(
    response: &egui::Response,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
    ui: &egui::Ui,
) {
    let plot_left = rect.left() + LANE_GUTTER;
    let plot_right = rect.right();
    let judge_y = rect.bottom() - JUDGE_MARGIN;
    let lane_w = (plot_right - plot_left).max(1.0) / LANE_COUNT as f32;
    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let pos = scrub_position(response, ui, rect, editor, current_time, pos);
    let pps = editor.pixels_per_second;
    let now = crate::keyboard::navigation::reference_time(editor, current_time);
    let time = now + (judge_y - pos.y) / pps;
    let lane = lane_at(pos.x - plot_left, lane_w);
    let play_y = judge_y - (current_time - now) * pps;
    let on_playhead = (pos.y - play_y).abs() <= 6.0;
    if let (Some(start), Some(press)) = (&mut editor.marquee_start, editor.pointer_press) {
        start.1 = judge_y - (measure_to_secs(press.measure, &chart.bpms) - now) * pps;
    }

    let markers: Markers = chart
        .notes
        .iter()
        .map(|n| {
            let y = judge_y - (note_secs(n, &chart.bpms) - now) * pps;
            let x = plot_left + (lane_index_of(n) as f32 + 0.5) * lane_w;
            (n.id, head_rect(editor, n, egui::pos2(x, y)))
        })
        .collect();

    interact(
        response,
        ui,
        editor,
        chart,
        Cursor {
            time,
            lane,
            on_playhead,
        },
        &markers,
    );

    // Marquee edge-autoscroll. Time flows up, so the top edge is "later".
    if editor.marquee_start.is_some() {
        if let Some(p) = response.interact_pointer_pos() {
            let f = edge_pan(p.y, rect.top(), rect.bottom());
            if f != 0.0 {
                pan_view(editor, -f * AUTOSCROLL_SECS, current_time);
            }
        }
    }
}

/// New pixels-per-second after a zoom of `delta` wheel units at `step`.
///
/// The result is clamped into [`MIN_PPS`, `MAX_PPS`]; without the clamp a large
/// wheel/flick makes `1 + delta * step` negative (or `0`), which flips the time
/// axis (elements reverse) or collapses the view to nothing.
pub(crate) fn zoomed_pps(old: f32, delta: f32, step: f32) -> f32 {
    (old * (1.0 + delta * step)).clamp(MIN_PPS, MAX_PPS)
}

/// Navigation runs before note handles so middle-button presses cannot edit notes.
pub fn navigate(
    ui: &egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    current_time: f32,
    raw_wheel: f32,
) -> bool {
    let middle = egui::PointerButton::Middle;
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let vertical = editor.layout == EditorLayout::Vertical;
    let axis = |pos: egui::Pos2| if vertical { -pos.y } else { pos.x };
    if editor.timeline_pan.is_some() {
        if ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape)) {
            editor.timeline_pan = None;
            editor.scrubbing = false;
            return true;
        }
        if !ui.input(|i| i.pointer.button_down(middle)) {
            // A stationary middle click still opens a slide; scrolling never does.
            editor.scrubbing = false;
            return editor.timeline_pan.take().is_some_and(|(_, moved)| moved);
        }
    }
    if editor.dragging.is_some() || editor.handle_grab.is_some()
        || editor.scrubbing && editor.timeline_pan.is_none()
        || editor.marquee_start.is_some()
        || editor.pointer_press.is_some() && ui.input(|i| i.pointer.primary_down()) {
        return false;
    }
    let hovered = ui.rect_contains_pointer(rect);
    if hovered && ui.input(|i| i.focused && i.pointer.button_pressed(middle) && !i.pointer.primary_down()) {
        editor.timeline_pan = pointer.map(|pos| (axis(pos), false));
        editor.view_start = view_time(editor, current_time);
    }
    if let Some((origin, moved)) = editor.timeline_pan {
        let step = pointer.map_or(0.0, |pos| rate_pan_step(axis(pos) - origin, ui.input(|i| i.stable_dt)));
        editor.timeline_pan = Some((origin, moved || step != 0.0));
        pan_view(editor, step / editor.pixels_per_second, current_time);
        if editor.pending_seek.is_some() { editor.scrubbing = true; }
        ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);
        if step != 0.0 { ui.ctx().request_repaint(); }
        return true;
    }
    if !hovered { return false; }
    let command = ui.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.mac_cmd
        || i.events.iter().any(|event| matches!(event, egui::Event::MouseWheel { modifiers, .. }
            if modifiers.command || modifiers.ctrl || modifiers.mac_cmd)));
    let shift = ui.input(|i| i.modifiers.shift || i.events.iter().any(|event|
        matches!(event, egui::Event::MouseWheel { modifiers, .. } if modifiers.shift)));
    let delta = ui.input(|i| i.raw_scroll_delta);
    let scroll = if delta.y.abs() > delta.x.abs() { delta.y } else { delta.x };
    let wheel = if raw_wheel.abs() > f32::EPSILON { raw_wheel } else { scroll / 30.0 };
    let pinch = crate::gesture::take();
    if pinch.abs() > 1e-6 || command && wheel.abs() > f32::EPSILON {
        let change = if pinch.abs() > 1e-6 { pinch } else { wheel * 0.05 };
        let old = editor.pixels_per_second;
        let new = zoomed_pps(old, change, 1.0);
        let pos = pointer.unwrap_or(rect.center());
        let offset = if vertical { rect.bottom() - JUDGE_MARGIN - pos.y }
            else { pos.x - rect.left() - LANE_GUTTER };
        let time = view_time(editor, current_time) + offset / old - offset / new;
        super::geometry::set_view_reference(editor, time, current_time);
        editor.pixels_per_second = new;
    } else if !command && (scroll != 0.0 || wheel != 0.0) {
        let direction = if editor.scroll_invert { -1.0 } else { 1.0 };
        let seconds = if shift { (if scroll != 0.0 { scroll } else { wheel * 30.0 }) / editor.pixels_per_second }
            else { wheel * 0.002 };
        pan_view(editor, -seconds * direction, current_time);
    } else { return false; }
    ui.input_mut(|i| {
        i.smooth_scroll_delta = egui::Vec2::ZERO;
        i.raw_scroll_delta = egui::Vec2::ZERO;
    });
    false
}

#[cfg(test)]
mod tests {
    use super::{
        Cursor, MAX_PPS, MIN_PPS, edge_pan, interact, nearest_marker, slide_trail_at, zoomed_pps,
    };
    use crate::app::types::{ChartDoc, Note, NoteType, Slide};
    use crate::state::{EditorState, Tool};
    use egui_macroquad::egui;

    fn chart_with(notes: Vec<Note>) -> ChartDoc {
        ChartDoc {
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
        }
    }

    struct PointerTest {
        ctx: egui::Context,
        editor: EditorState,
        chart: ChartDoc,
        clock: f64,
    }

    impl PointerTest {
        fn new() -> Self {
            Self {
                ctx: egui::Context::default(),
                editor: EditorState::default(),
                clock: 0.0,
                chart: chart_with(vec![
                    Note {
                        id: 1,
                        time: 2.0,
                        lane: 3,
                        ..Default::default()
                    },
                    Note {
                        id: 2,
                        time: 3.1,
                        lane: 4,
                        ..Default::default()
                    },
                ]),
            }
        }

        fn frame(&mut self, at: egui::Pos2, button: Option<bool>, modifiers: egui::Modifiers) {
            self.clock += 0.5;
            let mut events = vec![egui::Event::PointerMoved(at)];
            if let Some(pressed) = button {
                events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                });
            }
            let _ = self.ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    time: Some(self.clock),
                    modifiers,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = ui.interact(
                            ui.max_rect(),
                            ui.id().with("timeline"),
                            egui::Sense::click_and_drag(),
                        );
                        if crate::timeline::transform::handle_pointer(
                            ui, ui.max_rect(), &mut self.editor, &mut self.chart, 0.0, 0.0,
                        ) {
                            return;
                        }
                        interact(
                            &response,
                            ui,
                            &mut self.editor,
                            &mut self.chart,
                            Cursor {
                                time: 2.0 + (at.x - 120.0) / 100.0,
                                lane: 3,
                                on_playhead: false,
                            },
                            &vec![
                                (
                                    1,
                                    egui::Rect::from_center_size(
                                        egui::pos2(120.0, 100.0),
                                        egui::vec2(40.0, 40.0),
                                    ),
                                ),
                                (
                                    2,
                                    egui::Rect::from_center_size(
                                        egui::pos2(180.0, 150.0),
                                        egui::vec2(40.0, 40.0),
                                    ),
                                ),
                            ],
                        );
                    });
                },
            );
        }

        fn click(&mut self, at: egui::Pos2, modifiers: egui::Modifiers) {
            self.frame(at, None, modifiers);
            self.frame(at, Some(true), modifiers);
            self.frame(at, Some(false), modifiers);
        }
    }

    #[test]
    fn requested_timeline_mouse_click_refreshes_tab_anchor() {
        let mut test = PointerTest::new();
        test.editor.keyboard.cursor = crate::keyboard::navigation::note_cursor(&test.chart.notes[1]);
        test.editor.keyboard.candidate = Some(2);
        test.click(egui::pos2(120.0, 100.0), egui::Modifiers::NONE);
        assert_eq!(test.editor.keyboard.candidate, Some(1));
        assert!(test.editor.pending_seek.is_none());
        crate::keyboard::navigation::cycle(&mut test.editor, &test.chart, false);
        assert_eq!(test.editor.keyboard.candidate, Some(2));
        test.click(egui::pos2(200.0, 250.0), egui::Modifiers::NONE);
        assert_eq!(test.editor.keyboard.candidate, None);
        crate::keyboard::navigation::cycle(&mut test.editor, &test.chart, true);
        assert_eq!(test.editor.keyboard.candidate, Some(1));
    }

    #[test]
    fn hold_confirmation_click_sets_duration_without_placing_an_extra_note() {
        for tool in [Tool::Hold, Tool::TouchHold] {
            let mut test = PointerTest::new();
            test.editor.tool = tool;
            test.click(egui::pos2(300.0, 250.0), egui::Modifiers::NONE);
            assert_eq!(test.chart.notes.len(), 2);
            assert!(test.editor.hold_placement.is_some());
            assert!(test.editor.undo.is_empty());
            test.frame(egui::pos2(350.0, 250.0), None, egui::Modifiers::NONE);
            assert_eq!(test.chart.notes.len(), 2);
            test.click(egui::pos2(400.0, 250.0), egui::Modifiers::NONE);
            assert_eq!(test.chart.notes.len(), 3);
            assert_eq!(test.chart.notes[2].hold_duration, 0.5);
            assert_eq!(test.editor.undo.len(), 1);
            assert!(test.editor.hold_placement.is_none());
        }
    }

    #[test]
    fn duplicate_confirmation_click_does_not_place_an_extra_note() {
        let mut test = PointerTest::new();
        test.editor.tool = Tool::Tap;
        test.editor.select_only(1);
        let at = egui::pos2(300.0, 250.0);
        test.frame(at, None, egui::Modifiers::NONE);
        crate::timeline::transform::duplicate(&mut test.editor, &mut test.chart, at);
        test.click(at, egui::Modifiers::NONE);
        assert_eq!(test.chart.notes.len(), 3, "confirmation must not place another tap");
        assert_eq!(test.editor.undo.len(), 1);
        test.click(egui::pos2(350.0, 250.0), egui::Modifiers::NONE);
        assert_eq!(test.chart.notes.len(), 4, "next placement click remains usable");
    }

    #[test]
    fn double_click_star_head_adjusts_timing_before_adding_same_head_trajectory() {
        for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
            let mut test = PointerTest::new();
            test.editor.layout = layout;
            test.chart.notes[0].note_type = NoteType::Slide;
            test.chart.notes[0].slide.push(Slide {
                slide_duration: 0.75,
                slide_start_delay: 0.25,
                segments: Vec::new(),
                slide_is_break: false,
                runtime_parts: 1,
            });
            let original = test.chart.notes[0].clone();
            // Keep both clicks inside egui's double-click interval without
            // changing the timing of the existing drag tests.
            test.ctx.options_mut(|options| options.input_options.max_double_click_delay = 2.0);
            let head = egui::pos2(120.0, 100.0);
            test.click(head, egui::Modifiers::NONE);
            assert_eq!(test.chart.notes[0].slide.len(), 1);
            assert!(test.editor.editing_slide.is_none());
            test.click(head, egui::Modifiers::NONE);
            assert_eq!(test.editor.star_placement.unwrap().note_id, Some(1));
            assert_eq!(test.chart.notes[0], original);
            assert!(test.editor.undo.is_empty());
            assert!(test.editor.editing_slide.is_none());
            test.click(egui::pos2(170.0, 200.0), egui::Modifiers::NONE);
            assert_eq!(test.editor.star_placement.unwrap().delay_end, Some(2.25));
            assert_eq!(test.chart.notes[0], original);
            test.click(egui::pos2(270.0, 200.0), egui::Modifiers::NONE);
            assert_eq!(test.chart.notes.len(), 2, "do not duplicate the note head");
            assert_eq!(test.chart.notes[0].slide.len(), 2);
            assert_eq!(test.chart.notes[0].slide[1].slide_start_delay, 0.25);
            assert_eq!(test.chart.notes[0].slide[1].slide_duration, 0.75);
            assert_eq!(test.chart.notes[0].slide[0], original.slide[0]);
            assert_eq!(test.chart.notes[0].time, original.time);
            assert_eq!(test.chart.notes[0].lane, original.lane);
            assert_eq!(test.editor.editing_slide, Some(1));
            assert_eq!(test.editor.editing_slide_idx, 1);
            assert_eq!(test.editor.undo.len(), 1);
            test.editor.undo(&mut test.chart);
            assert_eq!(test.chart.notes[0], original);
            test.editor.redo(&mut test.chart);
            assert_eq!(test.chart.notes[0].slide.len(), 2);
        }
    }

    #[test]
    fn double_click_tap_head_does_not_add_a_slide() {
        let mut test = PointerTest::new();
        test.ctx.options_mut(|options| options.input_options.max_double_click_delay = 2.0);
        let head = egui::pos2(120.0, 100.0);
        test.click(head, egui::Modifiers::NONE);
        test.click(head, egui::Modifiers::NONE);
        assert!(test.chart.notes[0].slide.is_empty());
        assert!(test.editor.editing_slide.is_none());
        assert!(test.editor.undo.is_empty());
    }

    #[test]
    fn first_drag_frame_uses_press_hit_and_preserves_unsnapped_group_spacing() {
        let mut test = PointerTest::new();
        test.editor.snap_enabled = false;
        test.editor.selected = [1, 2].into_iter().collect();
        let start = egui::pos2(120.0, 100.0);
        test.frame(start, None, egui::Modifiers::NONE);
        test.frame(start, Some(true), egui::Modifiers::NONE);
        // The first moved frame is already outside the note hit radius.
        test.frame(egui::pos2(174.0, 100.0), None, egui::Modifiers::NONE);
        assert_eq!(test.editor.dragging, Some(1));
        assert!((test.chart.notes[0].time - 2.27).abs() < 1e-5);
        assert!((test.chart.notes[1].time - 3.37).abs() < 1e-5);
        test.frame(egui::pos2(194.0, 100.0), None, egui::Modifiers::NONE);
        test.frame(egui::pos2(194.0, 100.0), Some(false), egui::Modifiers::NONE);
        assert_eq!(test.editor.undo.len(), 1);
        assert_eq!(test.editor.selected.len(), 2);
        assert!((test.chart.notes[1].time - test.chart.notes[0].time - 1.1).abs() < 1e-5);
        test.editor.undo(&mut test.chart);
        assert_eq!(test.chart.notes[0].time, 2.0);
    }

    #[test]
    fn shrinking_marquee_drops_notes_outside_current_rectangle() {
        let mut test = PointerTest::new();
        let start = egui::pos2(50.0, 50.0);
        test.frame(start, None, egui::Modifiers::NONE);
        test.frame(start, Some(true), egui::Modifiers::NONE);
        test.frame(egui::pos2(200.0, 200.0), None, egui::Modifiers::NONE);
        assert_eq!(test.editor.selected.len(), 2);
        test.frame(egui::pos2(140.0, 130.0), None, egui::Modifiers::NONE);
        assert_eq!(test.editor.selected, [1].into_iter().collect());
        test.frame(egui::pos2(140.0, 130.0), Some(false), egui::Modifiers::NONE);
        assert!(test.editor.undo.is_empty());
    }

    #[test]
    fn marquee_uses_release_position_and_clears_gesture_state() {
        let mut test = PointerTest::new();
        let start = egui::pos2(50.0, 50.0);
        test.frame(start, None, egui::Modifiers::NONE);
        test.frame(start, Some(true), egui::Modifiers::NONE);
        test.frame(egui::pos2(200.0, 200.0), None, egui::Modifiers::NONE);
        test.frame(egui::pos2(140.0, 130.0), Some(false), egui::Modifiers::NONE);
        assert_eq!(test.editor.selected, [1].into_iter().collect());
        assert!(test.editor.marquee_start.is_none());
        assert!(test.editor.pointer_press.is_none());
        assert!(test.editor.marquee_base.is_empty());
    }

    #[test]
    fn shift_click_adds_and_command_click_toggles() {
        let mut test = PointerTest::new();
        let shift = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let cmd = egui::Modifiers {
            command: true,
            ctrl: true,
            ..Default::default()
        };
        test.click(egui::pos2(120.0, 100.0), egui::Modifiers::NONE);
        test.click(egui::pos2(180.0, 150.0), shift);
        test.click(egui::pos2(180.0, 150.0), shift);
        assert_eq!(test.editor.selected.len(), 2);
        test.click(egui::pos2(180.0, 150.0), cmd);
        assert_eq!(test.editor.selected, [1].into_iter().collect());
        test.click(egui::pos2(180.0, 150.0), cmd);
        assert_eq!(test.editor.selected.len(), 2);
    }

    #[test]
    fn additive_and_subtractive_marquee_keep_base_selection() {
        for subtract in [false, true] {
            let mut test = PointerTest::new();
            test.editor.selected = if subtract {
                [1, 2].into_iter().collect()
            } else {
                [1].into_iter().collect()
            };
            let modifiers = if subtract {
                egui::Modifiers {
                    command: true,
                    ctrl: true,
                    ..Default::default()
                }
            } else {
                egui::Modifiers {
                    shift: true,
                    ..Default::default()
                }
            };
            let start = egui::pos2(150.0, 120.0);
            test.frame(start, None, modifiers);
            test.frame(start, Some(true), modifiers);
            test.frame(egui::pos2(200.0, 180.0), None, modifiers);
            assert!(test.editor.selected.contains(&1));
            assert_eq!(test.editor.selected.contains(&2), !subtract);
            // Shrink away from note 2: restore the base, even for subtract.
            test.frame(egui::pos2(155.0, 125.0), None, modifiers);
            assert_eq!(test.editor.selected.contains(&2), subtract);
        }
    }

    #[test]
    fn placement_tools_allow_marquee_without_placing_notes() {
        for tool in [
            Tool::Tap,
            Tool::Hold,
            Tool::Slide,
            Tool::Touch,
            Tool::TouchHold,
        ] {
            let mut test = PointerTest::new();
            test.editor.tool = tool;
            let start = egui::pos2(50.0, 50.0);
            test.frame(start, None, egui::Modifiers::NONE);
            test.frame(start, Some(true), egui::Modifiers::NONE);
            test.frame(egui::pos2(200.0, 200.0), None, egui::Modifiers::NONE);
            test.frame(egui::pos2(200.0, 200.0), Some(false), egui::Modifiers::NONE);
            assert_eq!(test.editor.selected.len(), 2, "{tool:?}");
            assert_eq!(test.editor.tool, tool);
            assert_eq!(test.chart.notes.len(), 2);
            assert!(test.editor.star_placement.is_none());
            assert!(test.editor.undo.is_empty());
        }
    }

    #[test]
    fn trail_hit_testing_follows_rendered_diagonal_in_both_layouts() {
        use crate::app::types::zone::PadZone;
        use crate::app::types::{SlidePoint, SlideSegment, SlideShape};
        let chart = chart_with(vec![Note {
            id: 7,
            time: 1.0,
            lane: 3,
            note_type: NoteType::Slide,
            slide: vec![Slide {
                segments: vec![SlideSegment {
                    points: vec![SlidePoint::from(PadZone::from(7))],
                    shape: SlideShape::Line,
                }],
                slide_duration: 1.0,
                slide_start_delay: 0.25,
                slide_is_break: false,
                runtime_parts: 1,
            }],
            ..Default::default()
        }]);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(930.0, 922.0));
        for layout in [
            crate::state::EditorLayout::Vertical,
            crate::state::EditorLayout::Horizontal,
        ] {
            let mut editor = EditorState::default();
            editor.layout = layout;
            editor.pixels_per_second = 100.0;
            let pointer = match layout {
                crate::state::EditorLayout::Vertical => egui::pos2(480.0, 400.0),
                crate::state::EditorLayout::Horizontal => egui::pos2(400.0, 472.0),
            };
            assert_eq!(
                super::slide_trail_at_pointer(&chart, &editor, rect, pointer, 1.25),
                Some((7, 0))
            );
            let outside = match layout {
                crate::state::EditorLayout::Vertical => pointer + egui::vec2(40.0, 0.0),
                crate::state::EditorLayout::Horizontal => pointer + egui::vec2(0.0, 40.0),
            };
            assert_eq!(
                super::slide_trail_at_pointer(&chart, &editor, rect, outside, 1.25),
                None
            );
        }
    }

    #[test]
    fn hit_testing_includes_head_edges_and_chooses_nearest_center() {
        let bounds =
            |x, size| egui::Rect::from_center_size(egui::pos2(x, 100.0), egui::vec2(size, size));
        let markers = vec![(1, bounds(100.0, 40.0)), (2, bounds(125.0, 45.0))];
        assert_eq!(nearest_marker(&markers, egui::pos2(80.0, 80.0)), Some(1));
        assert_eq!(nearest_marker(&markers, egui::pos2(79.9, 100.0)), None);
        assert_eq!(nearest_marker(&markers, egui::pos2(110.0, 100.0)), Some(1));
        assert_eq!(nearest_marker(&markers, egui::pos2(116.0, 100.0)), Some(2));
        assert_eq!(nearest_marker(&markers, egui::pos2(147.5, 122.5)), Some(2));
    }

    fn key(
        editor: &mut EditorState,
        chart: &mut ChartDoc,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) -> bool {
        let ctx = egui::Context::default();
        let mut owned = false;
        let _ = ctx.run(
            egui::RawInput {
                modifiers,
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(300.0, 200.0)),
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    owned = super::handle_edit_shortcuts(ui, editor, chart, 0.0);
                    if !owned {
                        crate::ui::tools::handle_shortcuts(ui, editor);
                    }
                });
            },
        );
        owned
    }

    #[test]
    fn shift_r_repeats_detected_placement_without_changing_tool() {
        let mut test = PointerTest::new();
        test.editor.tool = Tool::Hold;
        for (time, lane) in [(4.0, 1), (4.25, 2), (4.5, 3)] {
            test.editor.place_note(&mut test.chart, time, lane);
        }
        assert!(key(&mut test.editor, &mut test.chart, egui::Key::R, egui::Modifiers::SHIFT));
        let note = test.chart.notes.last().unwrap();
        assert_eq!((note.time, note.lane), (4.75, 4));
        assert_eq!(note.note_type, NoteType::Hold);
        assert_eq!(test.editor.tool, Tool::Hold);
        assert!(test.editor.selection_transform.is_none());
    }

    #[test]
    fn duplicate_shortcut_moves_copies_and_repeat_uses_current_selection() {
        let mut test = PointerTest::new();
        test.editor.selected = [1, 2].into_iter().collect();
        test.editor.copy_selection(&test.chart);
        let clipboard = test.editor.clipboard.clone();
        let original = test.chart.notes.clone();
        assert!(key(&mut test.editor, &mut test.chart, egui::Key::D, egui::Modifiers::SHIFT));
        assert_eq!(test.chart.notes.len(), original.len() + 2);
        assert!(test.editor.selection_transform.is_some());
        assert_eq!(test.editor.clipboard, clipboard);
        assert!(test.editor.selected.iter().all(|id| !original.iter().any(|n| n.id == *id)));
        super::super::transform::shift(&mut test.editor, &mut test.chart, 0.25, 1);
        key(&mut test.editor, &mut test.chart, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(test.editor.undo.len(), 1);
        assert_eq!(&test.chart.notes[..original.len()], &original);
        let copies: Vec<_> = test.chart.notes.iter().filter(|n| test.editor.selected.contains(&n.id)).cloned().collect();
        assert!(key(&mut test.editor, &mut test.chart, egui::Key::R, egui::Modifiers::SHIFT));
        assert!(test.editor.selection_transform.is_none());
        for (old, new) in copies.iter().zip(test.chart.notes.iter().filter(|n| test.editor.selected.contains(&n.id))) {
            assert_eq!(new.time, old.time + 0.25);
            assert_eq!(new.lane, (old.lane % 8) + 1);
        }
        assert_eq!(test.editor.undo.len(), 2);
        test.editor.undo(&mut test.chart);
        assert_eq!(test.chart.notes.len(), original.len() + 2);
        test.editor.undo(&mut test.chart);
        assert_eq!(test.chart.notes, original);
    }

    #[test]
    fn duplicate_cancel_keeps_in_place_copies_and_is_undoable() {
        let mut test = PointerTest::new();
        test.editor.selected = [1].into_iter().collect();
        let original = test.chart.notes.clone();
        key(&mut test.editor, &mut test.chart, egui::Key::D, egui::Modifiers::SHIFT);
        super::super::transform::shift(&mut test.editor, &mut test.chart, 0.5, 2);
        key(&mut test.editor, &mut test.chart, egui::Key::Escape, egui::Modifiers::NONE);
        let copy = test.chart.notes.last().unwrap();
        assert_eq!((copy.time, copy.lane), (original[0].time, original[0].lane));
        assert!(test.editor.selected.contains(&copy.id));
        assert_eq!(test.editor.undo.len(), 1);
        test.editor.undo(&mut test.chart);
        assert_eq!(test.chart.notes, original);
    }

    #[test]
    fn repeat_shortcut_without_operation_or_selection_does_nothing() {
        let mut test = PointerTest::new();
        let original = test.chart.notes.clone();
        key(&mut test.editor, &mut test.chart, egui::Key::R, egui::Modifiers::SHIFT);
        key(&mut test.editor, &mut test.chart, egui::Key::D, egui::Modifiers::SHIFT);
        assert_eq!(test.chart.notes, original);
        assert!(test.editor.undo.is_empty());
        assert!(test.editor.selection_transform.is_none());
    }

    #[test]
    fn escape_cancels_same_head_placement_without_editing_the_chart() {
        let mut test = PointerTest::new();
        test.chart.notes[0].note_type = NoteType::Slide;
        let original = test.chart.notes.clone();
        assert!(test.editor.begin_same_head_placement(&test.chart, 1));
        test.editor.place_star_step(&mut test.chart, 2.25, 3);
        key(&mut test.editor, &mut test.chart, egui::Key::G, egui::Modifiers::NONE);
        assert!(test.editor.selection_transform.is_none());
        key(&mut test.editor, &mut test.chart, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(test.editor.star_placement.is_none());
        assert_eq!(test.chart.notes, original);
        assert!(test.editor.undo.is_empty());
    }

    #[test]
    fn contextual_transform_keys_do_not_switch_tools_and_escape_restores() {
        let mut test = PointerTest::new();
        test.editor.selected = [1, 2].into_iter().collect();
        assert!(key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::G,
            egui::Modifiers::NONE
        ));
        assert_eq!(test.editor.tool, crate::state::Tool::Select);
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::ArrowUp,
            egui::Modifiers::NONE,
        );
        assert!(test.chart.notes[0].time > 2.0);
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::Escape,
            egui::Modifiers::NONE,
        );
        assert_eq!(test.chart.notes[0].time, 2.0);
        assert!(test.editor.undo.is_empty());
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::S,
            egui::Modifiers::NONE,
        );
        assert!(test.editor.selection_transform.is_some());
        assert_eq!(test.editor.tool, crate::state::Tool::Select);
    }

    #[test]
    fn grab_enter_commits_once_and_arrows_follow_layout() {
        for layout in [
            crate::state::EditorLayout::Vertical,
            crate::state::EditorLayout::Horizontal,
        ] {
            let mut test = PointerTest::new();
            test.editor.layout = layout;
            test.editor.selected = [1, 2].into_iter().collect();
            key(
                &mut test.editor,
                &mut test.chart,
                egui::Key::G,
                egui::Modifiers::NONE,
            );
            let later = match layout {
                crate::state::EditorLayout::Vertical => egui::Key::ArrowUp,
                crate::state::EditorLayout::Horizontal => egui::Key::ArrowRight,
            };
            key(
                &mut test.editor,
                &mut test.chart,
                later,
                egui::Modifiers::NONE,
            );
            key(
                &mut test.editor,
                &mut test.chart,
                later,
                egui::Modifiers::NONE,
            );
            key(
                &mut test.editor,
                &mut test.chart,
                egui::Key::Enter,
                egui::Modifiers::NONE,
            );
            assert!(test.editor.selection_transform.is_none());
            assert_eq!(test.editor.undo.len(), 1);
            assert_eq!(test.chart.notes[0].lane, 3);
            assert_eq!(
                test.chart.notes[0].time,
                2.0 + 2.0 / test.editor.snap_division as f32
            );
            test.editor.undo(&mut test.chart);
            assert_eq!(test.chart.notes[0].time, 2.0);
        }
    }

    #[test]
    fn slide_edit_keys_own_preview_and_command_chords_do_not_pick_shapes() {
        let mut test = PointerTest::new();
        test.editor.mode_dirty = false;
        test.chart.notes[0].note_type = NoteType::Slide;
        test.editor.selected.insert(1);
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::E,
            egui::Modifiers::NONE,
        );
        assert_eq!(test.editor.editing_slide, Some(1));
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::P,
            egui::Modifiers::NONE,
        );
        assert_eq!(
            test.editor.pending_shape,
            Some(crate::app::types::SlideShape::P)
        );
        assert!(!test.editor.mode_dirty);
        let cmd = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        key(&mut test.editor, &mut test.chart, egui::Key::C, cmd);
        assert_eq!(test.editor.clipboard.len(), 1);
        assert_eq!(
            test.editor.pending_shape,
            Some(crate::app::types::SlideShape::P)
        );
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::E,
            egui::Modifiers::NONE,
        );
        assert!(test.editor.editing_slide.is_none());
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::P,
            egui::Modifiers::NONE,
        );
        assert!(test.editor.mode_dirty);
    }

    #[test]
    fn select_all_clear_and_delete_shortcuts_are_undoable() {
        let mut test = PointerTest::new();
        let cmd = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        key(&mut test.editor, &mut test.chart, egui::Key::A, cmd);
        assert_eq!(test.editor.selected.len(), 2);
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::A,
            egui::Modifiers { shift: true, ..cmd },
        );
        assert!(test.editor.selected.is_empty());
        key(&mut test.editor, &mut test.chart, egui::Key::A, cmd);
        key(
            &mut test.editor,
            &mut test.chart,
            egui::Key::D,
            egui::Modifiers::NONE,
        );
        assert!(test.chart.notes.is_empty());
        test.editor.undo(&mut test.chart);
        assert_eq!(test.chart.notes.len(), 2);
    }

    #[test]
    fn edge_pan_is_zero_in_the_middle_and_signed_at_the_edges() {
        // 0..100 axis, middle band = 28..72.
        assert_eq!(edge_pan(50.0, 0.0, 100.0), 0.0);
        assert_eq!(edge_pan(28.0, 0.0, 100.0), 0.0);
        assert_eq!(edge_pan(72.0, 0.0, 100.0), 0.0);
        assert!(edge_pan(0.0, 0.0, 100.0) < 0.0, "lo edge pans earlier");
        assert!(edge_pan(100.0, 0.0, 100.0) > 0.0, "hi edge pans later");
        // Full tilt at the very edge, clamped to 1.
        assert!((edge_pan(0.0, 0.0, 100.0) + 1.0).abs() < 1e-5);
        assert!((edge_pan(100.0, 0.0, 100.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn slide_trail_hit_test_respects_lane_and_span() {
        // 120 BPM fallback: measure 1.0 → 0s, a 1-measure slide spans 2s.
        let slide = Note {
            id: 7,
            time: 1.0,
            lane: 3,
            note_type: NoteType::Slide,
            slide: vec![Slide {
                segments: Vec::new(),
                slide_duration: 1.0,
                slide_start_delay: 0.0,
                slide_is_break: false,
                runtime_parts: 1,
            }],
            ..Default::default()
        };
        let chart = chart_with(vec![slide]);
        assert_eq!(slide_trail_at(&chart, 1.0, 3), Some((7, 0)), "on the trail");
        assert_eq!(slide_trail_at(&chart, 0.0, 3), Some((7, 0)), "at the head");
        assert_eq!(slide_trail_at(&chart, 2.0, 3), Some((7, 0)), "at the tail");
        assert_eq!(slide_trail_at(&chart, 5.0, 3), None, "past the tail");
        assert_eq!(slide_trail_at(&chart, 1.0, 4), None, "wrong lane");
    }

    #[test]
    fn zoom_stays_positive_and_bounded() {
        // A big scroll-out would drive `1 + delta*step` negative or to zero; the
        // clamp must keep the zoom positive so the timeline never flips.
        assert_eq!(zoomed_pps(120.0, -1.0e6, 0.05), MIN_PPS);
        assert_eq!(zoomed_pps(120.0, -20.0, 0.05), MIN_PPS, "factor hits 0");
        assert_eq!(zoomed_pps(120.0, 1.0e6, 0.05), MAX_PPS);
        // Ordinary steps still move in the expected direction.
        assert!(zoomed_pps(120.0, 1.0, 0.05) > 120.0);
        assert!(zoomed_pps(120.0, -1.0, 0.05) < 120.0);
    }

    #[test]
    fn double_click_empty_star_starts_pending_placement_without_scrubbing() {
        let ctx = egui::Context::default();
        let mut editor = EditorState::default();
        let mut chart = chart_with(vec![Note {
            id: 7,
            time: 1.0,
            lane: 3,
            note_type: NoteType::Slide,
            ..Default::default()
        }]);
        let at = egui::pos2(120.0, 100.0);
        let mut clock = 0.0;
        let mut frame = |events| {
            clock += 0.05;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    time: Some(clock),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = ui.interact(
                            ui.max_rect(),
                            ui.id().with("timeline"),
                            egui::Sense::click_and_drag(),
                        );
                        interact(
                            &response,
                            ui,
                            &mut editor,
                            &mut chart,
                            Cursor {
                                time: 0.0,
                                lane: 3,
                                on_playhead: true,
                            },
                            &vec![(7, egui::Rect::from_center_size(at, egui::vec2(45.0, 45.0)))],
                        );
                    });
                },
            );
        };
        frame(vec![egui::Event::PointerMoved(at)]);
        for _ in 0..2 {
            for pressed in [true, false] {
                frame(vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
        }
        assert!(chart.notes[0].slide.is_empty());
        assert_eq!(editor.star_placement.unwrap().note_id, Some(7));
        assert_eq!(editor.editing_slide, None);
        assert!(editor.selected.contains(&7));
        assert!(editor.undo.is_empty());
        assert_eq!(editor.pending_seek, None, "star clicks do not scrub");
    }
}
