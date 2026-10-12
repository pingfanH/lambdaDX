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

use crate::app::types::{note_secs, secs_to_measure, ChartDoc};
use crate::state::{DragGrab, EditorState, Tool};
use crate::io::save_chart;

use super::geometry::{
    dragged_position, lane_index_of, paint_marquee, snap, toggle_select, LANE_COUNT, LANE_GUTTER,
    RULER_H, JUDGE_MARGIN,
};

// ── Shared shortcuts ───────────────────────────────────────────────────────

/// Clipboard / undo / redo / flag shortcuts, called once per frame. Uses
/// `command` so Ctrl (Win/Linux) and Cmd (macOS) both work.
pub fn handle_edit_shortcuts(
    ui: &egui::Ui,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
) {
    if ui.ctx().wants_keyboard_input() {
        return;
    }
    let cmd = ui.input(|i| i.modifiers.command);
    let shift = ui.input(|i| i.modifiers.shift);
    let plain = !cmd && !shift;
    let pressed = |k| ui.input(|i| i.key_pressed(k));

    if cmd && !shift && pressed(egui::Key::Z) {
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
        let measure = snap(secs_to_measure(current_time, &chart.bpms), editor.snap_enabled, editor.snap_division).max(1.0);
        editor.paste_at(chart, measure);
    }
    if cmd && pressed(egui::Key::S) {
        save_chart(editor, chart);
    }
    if (pressed(egui::Key::Delete) || pressed(egui::Key::Backspace)) && !editor.selected.is_empty() {
        editor.delete_selection(chart);
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
type Markers = Vec<(u64, egui::Pos2)>;

fn interact(
    response: &egui::Response,
    ui: &egui::Ui,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    cursor: Cursor,
    markers: &Markers,
) {
    if response.drag_started() {
        editor.scrubbing = cursor.on_playhead;
        editor.dragging = if cursor.on_playhead {
            None
        } else {
            nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default())
        };
        editor.drag_history_pushed = false;
        editor.marquee_start = None;
        editor.drag_grab = None;
        if let Some(id) = editor.dragging {
            editor.select_only(id);
            editor.drag_grab = chart.notes.iter().find(|n| n.id == id).map(|n| DragGrab {
                note_time: n.time,
                note_lane: n.lane,
                grab_measure: secs_to_measure(cursor.time, &chart.bpms),
                grab_lane: cursor.lane as i32,
            });
        } else if !cursor.on_playhead && editor.tool == Tool::Select {
            editor.marquee_start = response.interact_pointer_pos().map(|p| (p.x, p.y));
            if !ui.input(|i| i.modifiers.shift) {
                editor.selected.clear();
            }
        }
    }

    if response.dragged() && editor.scrubbing {
        editor.pending_seek = Some(cursor.time.max(0.0));
        return;
    }

    if response.dragged() {
        if let Some((sx, sy)) = editor.marquee_start {
            if let Some(p) = response.interact_pointer_pos() {
                let sel_rect = egui::Rect::from_two_pos(egui::pos2(sx, sy), p);
                paint_marquee(ui, sel_rect);
                for (id, pos) in markers {
                    if sel_rect.contains(*pos) {
                        editor.selected.insert(*id);
                    }
                }
            }
            return;
        }
        if let Some(id) = editor.dragging {
            if !editor.drag_history_pushed {
                editor.push_history(chart);
                editor.drag_history_pushed = true;
            }
            let cur_measure = secs_to_measure(cursor.time, &chart.bpms);
            if let Some(note) = chart.notes.iter_mut().find(|n| n.id == id) {
                match editor.drag_grab {
                    Some(g) => {
                        let (t, l) = dragged_position(&g, cur_measure, cursor.lane as i32, editor.snap_division);
                        note.time = t;
                        note.lane = l;
                    }
                    None => {
                        note.time = snap(cur_measure, editor.snap_enabled, editor.snap_division).max(1.0);
                        note.lane = cursor.lane.clamp(1, 8);
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        editor.dragging = None;
        editor.drag_history_pushed = false;
        editor.scrubbing = false;
        editor.marquee_start = None;
        editor.drag_grab = None;
    }

    if response.clicked() {
        let hit = nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default());
        if cursor.on_playhead {
            editor.pending_seek = Some(cursor.time.max(0.0));
            return;
        }
        if let Some(id) = hit {
            toggle_select(editor, id, ui.input(|i| i.modifiers.shift));
            if editor.tool == Tool::Eraser {
                editor.push_history(chart);
                chart.notes.retain(|n| n.id != id);
                editor.selected.clear();
            }
            return;
        }
        if matches!(editor.tool, Tool::Tap | Tool::Hold | Tool::Touch | Tool::Slide) {
            let measure = snap(secs_to_measure(cursor.time, &chart.bpms), editor.snap_enabled, editor.snap_division)
                .max(1.0);
            editor.place_note(chart, measure, cursor.lane);
        } else {
            editor.pending_seek = Some(cursor.time.max(0.0));
        }
    }
}

/// Nearest note marker within the grab radius (12 px in either axis).
fn nearest_marker(markers: &Markers, at: egui::Pos2) -> Option<u64> {
    const GRAB: f32 = 12.0;
    markers
        .iter()
        .filter(|(_, p)| (p.x - at.x).abs() < GRAB && (p.y - at.y).abs() <= GRAB)
        .min_by(|a, b| {
            let da = (a.1 - at).length_sq();
            let db = (b.1 - at).length_sq();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| *id)
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

    // Wheel works on hover (no button held), so handle it before the
    // press-only pointer path below.
    handle_wheel(response, ui, editor, plot_left, current_time, false);

    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let time = editor.view_start + (pos.x - plot_left) / editor.pixels_per_second;
    let lane = (((pos.y - lanes_top) / lane_h).floor() as i64).clamp(1, LANE_COUNT as i64) as u8;
    let play_x = plot_left + (current_time - editor.view_start) * editor.pixels_per_second;
    let on_playhead = (pos.x - play_x).abs() <= 6.0;

    let markers: Markers = chart
        .notes
        .iter()
        .map(|n| {
            let x = plot_left
                + (note_secs(n, &chart.bpms) - editor.view_start) * editor.pixels_per_second;
            let y = lanes_top + (lane_index_of(n) as f32 + 0.5) * lane_h;
            (n.id, egui::pos2(x, y))
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
    let pps = editor.pixels_per_second;
    let now = current_time;

    // Wheel works on hover (no button held), so handle it before the
    // press-only pointer path below.
    handle_wheel(response, ui, editor, plot_left, current_time, true);

    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let time = now + (judge_y - pos.y) / pps;
    let lane = (((pos.x - plot_left) / lane_w).floor() as i64).clamp(1, LANE_COUNT as i64) as u8;
    let on_playhead = (pos.y - judge_y).abs() <= 6.0;

    let markers: Markers = chart
        .notes
        .iter()
        .map(|n| {
            let y = judge_y - (note_secs(n, &chart.bpms) - now) * pps;
            let x = plot_left + (lane_index_of(n) as f32 + 0.5) * lane_w;
            (n.id, egui::pos2(x, y))
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
}

/// Cmd/Ctrl + wheel zooms; plain wheel pans (horizontal) or scrubs (vertical).
///
/// Reads macroquad's raw wheel delta (the egui bridge divides it down, which
/// made the egui scroll far too small, and egui also consumes Cmd+scroll for its
/// own zoom). Needs only hover, not a held button.
fn handle_wheel(
    response: &egui::Response,
    ui: &egui::Ui,
    editor: &mut EditorState,
    plot_left: f32,
    current_time: f32,
    vertical: bool,
) {
    /// Zoom fraction per wheel unit.
    const ZOOM_STEP: f32 = 0.12;
    /// Pixels the view scrolls per wheel unit when panning / scrubbing.
    const PAN_PX: f32 = 48.0;

    if !response.hovered() {
        return;
    }
    let (_, wheel) = macroquad::input::mouse_wheel();
    if wheel.abs() < f32::EPSILON {
        return;
    }
    let cmd = ui.input(|i| i.modifiers.command);

    if cmd {
        let old = editor.pixels_per_second;
        let new = (old * (1.0 + wheel * ZOOM_STEP)).clamp(30.0, 1600.0);
        if !vertical {
            if let Some(pos) = response.hover_pos() {
                let anchor = editor.view_start + (pos.x - plot_left) / old;
                editor.view_start = (anchor - (pos.x - plot_left) / new).max(0.0);
            }
        }
        editor.pixels_per_second = new;
    } else if vertical {
        editor.pending_seek =
            Some((current_time - wheel * PAN_PX / editor.pixels_per_second).max(0.0));
    } else {
        editor.view_start =
            (editor.view_start - wheel * PAN_PX / editor.pixels_per_second).max(0.0);
    }
}


