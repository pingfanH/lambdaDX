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
use crate::app::types::{mdur_to_secs, note_secs, secs_to_measure, ChartDoc, NoteType};
use crate::state::{DragGrab, EditorState, Tool};
use crate::io::save_chart;

use super::geometry::{
    dragged_position, lane_index_of, paint_marquee, select_note, snap, LANE_COUNT, LANE_GUTTER,
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
    if (pressed(egui::Key::Delete)
        || (pressed(egui::Key::Backspace) && !editor.is_slide_edit()))
        && !editor.selected.is_empty()
    {
        editor.delete_selection(chart);
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
        return;
    }

    // Arrow keys nudge the selection by one grid step: ←/→ move in time
    // (earlier/later), ↑/↓ move a lane. Orientation-independent, matching how
    // the design labels the axes. A plain press only (no modifiers).
    if plain && !editor.selected.is_empty() {
        let step = 1.0 / editor.snap_division.max(1) as f32;
        let (d_time, d_lane) = if pressed(egui::Key::ArrowLeft) {
            (-step, 0)
        } else if pressed(egui::Key::ArrowRight) {
            (step, 0)
        } else if pressed(egui::Key::ArrowUp) {
            (0.0, 1)
        } else if pressed(egui::Key::ArrowDown) {
            (0.0, -1)
        } else {
            (0.0, 0)
        };
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
    // Note under the pointer, for the hover ring + read-out. Skipped mid-drag so
    // the read-out tracks the dragged note rather than the raw cursor.
    if !response.dragged() {
        editor.hovered = response
            .hover_pos()
            .and_then(|p| nearest_marker(markers, p));
    }

    if response.drag_started() {
        let additive = ui.input(|i| i.modifiers.shift);
        editor.scrubbing = cursor.on_playhead;
        editor.dragging = if cursor.on_playhead {
            None
        } else {
            nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default())
        };
        editor.drag_history_pushed = false;
        editor.marquee_start = None;
        editor.drag_grab = None;
        editor.drag_multi_orig.clear();
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
                grab_measure: secs_to_measure(cursor.time, &chart.bpms),
                grab_lane: cursor.lane as i32,
            });
            editor.drag_multi_orig = chart
                .notes
                .iter()
                .filter(|n| editor.selected.contains(&n.id))
                .map(|n| (n.id, n.time, n.lane))
                .collect();
        } else if !cursor.on_playhead && editor.tool == Tool::Select {
            editor.marquee_start = response.interact_pointer_pos().map(|p| (p.x, p.y));
            if !additive {
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
            match editor.drag_grab {
                Some(g) => {
                    let (t, l) =
                        dragged_position(&g, cur_measure, cursor.lane as i32, editor.snap_division);
                    let dt = t - g.note_time;
                    let dl = l as i32 - g.note_lane as i32;
                    // Move the whole drag group (the grabbed note plus any
                    // co-selected notes) by the same delta, mirroring the
                    // original editor's multi-select drag.
                    let group = if editor.drag_multi_orig.is_empty() {
                        vec![(id, g.note_time, g.note_lane)]
                    } else {
                        editor.drag_multi_orig.clone()
                    };
                    for (nid, t0, l0) in group {
                        if let Some(note) = chart.notes.iter_mut().find(|n| n.id == nid) {
                            note.time = snap(t0 + dt, true, editor.snap_division).max(1.0);
                            note.lane = (l0 as i32 + dl).clamp(1, 8) as u8;
                        }
                    }
                }
                None => {
                    if let Some(note) = chart.notes.iter_mut().find(|n| n.id == id) {
                        note.time =
                            snap(cur_measure, editor.snap_enabled, editor.snap_division).max(1.0);
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
        editor.drag_multi_orig.clear();
    }

    // Right-click removes the nearest note (the count of markers is small, so a
    // straight linear scan is fine).
    if response.secondary_clicked() {
        if let Some(id) = nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default()) {
            editor.push_history(chart);
            chart.notes.retain(|n| n.id != id);
            editor.selected.remove(&id);
            if editor.hovered == Some(id) {
                editor.hovered = None;
            }
        }
    }

    // Middle-click (or trackpad three-finger click) a slide's trail to enter the
    // slide ("star") track-edit mode on that note's sub-slide. A double-click on
    // the trail does the same — a reliable entry that needs no middle button.
    if response.middle_clicked() || response.double_clicked() {
        if let Some((id, sub)) = slide_trail_at(chart, cursor.time, cursor.lane) {
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

    if response.clicked() {
        let hit = nearest_marker(markers, response.interact_pointer_pos().unwrap_or_default());
        if cursor.on_playhead {
            editor.pending_seek = Some(cursor.time.max(0.0));
            return;
        }
        if let Some(id) = hit {
            select_note(editor, id, ui.input(|i| i.modifiers.shift));
            if editor.tool == Tool::Eraser {
                editor.push_history(chart);
                chart.notes.retain(|n| n.id != id);
                editor.selected.clear();
            }
            return;
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
            let measure = snap(secs_to_measure(cursor.time, &chart.bpms), editor.snap_enabled, editor.snap_division)
                .max(1.0);
            editor.place_note(chart, measure, cursor.lane);
        }
    }
}

/// The slide note (and which sub-slide) whose trail is under `(time, lane)`,
/// i.e. a slide in the same lane whose head..tail span contains the cursor time.
/// Used to enter slide-edit mode from a middle-click on the star's trajectory.
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

    // Marquee edge-autoscroll: while box-selecting, pan toward the edge the
    // pointer is pushed against (later time = right).
    if editor.marquee_start.is_some() {
        if let Some(p) = response.interact_pointer_pos() {
            let f = edge_pan(p.x, rect.left(), rect.right());
            if f != 0.0 {
                editor.view_start = (editor.view_start + f * AUTOSCROLL_SECS).max(0.0);
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

    // Marquee edge-autoscroll. Time flows up, so the top edge is "later".
    if editor.marquee_start.is_some() {
        if let Some(p) = response.interact_pointer_pos() {
            let f = edge_pan(p.y, rect.top(), rect.bottom());
            if f != 0.0 {
                editor.pending_seek = Some((now - f * AUTOSCROLL_SECS).max(0.0));
            }
        }
    }
}

/// Clamp bounds for the timeline zoom (pixels per second).
const MIN_PPS: f32 = 30.0;
const MAX_PPS: f32 = 1600.0;

/// New pixels-per-second after a zoom of `delta` wheel units at `step`.
///
/// The result is clamped into [`MIN_PPS`, `MAX_PPS`]; without the clamp a large
/// wheel/flick makes `1 + delta * step` negative (or `0`), which flips the time
/// axis (elements reverse) or collapses the view to nothing.
fn zoomed_pps(old: f32, delta: f32, step: f32) -> f32 {
    (old * (1.0 + delta * step)).clamp(MIN_PPS, MAX_PPS)
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
    const ZOOM_STEP: f32 = 0.05;
    /// Seconds the view advances per wheel unit when panning / scrubbing. A fixed
    /// *time* step (rather than a pixel amount) keeps the scroll speed tied to the
    /// time axis, the same at every zoom level.
    const WHEEL_SECS: f32 = 0.002;
    /// Zoom factor per unit of accumulated trackpad magnification (a pinch is a
    /// fraction, e.g. 0.1 ≈ +10%).
    const PINCH_ZOOM: f32 = 1.0;

    // Trackpad pinch (macOS) applies whenever it arrives — a pinch doesn't move
    // the cursor, so requiring the pointer to hover the timeline would silently
    // drop it (and make the user lift to re-pinch). The zoom anchors on the
    // judgment line (vertical, fixed origin) or the pointer (horizontal).
    let pinch = crate::gesture::take();
    if pinch.abs() > 1e-6 {
        let old = editor.pixels_per_second;
        let new = zoomed_pps(old, pinch, PINCH_ZOOM);
        if !vertical {
            if let Some(pos) = response.hover_pos() {
                let anchor = editor.view_start + (pos.x - plot_left) / old;
                editor.view_start = (anchor - (pos.x - plot_left) / new).max(0.0);
            }
        }
        editor.pixels_per_second = new;
    }

    if !response.hovered() {
        return;
    }
    let (_, raw_wheel) = macroquad::input::mouse_wheel();
    if raw_wheel.abs() < f32::EPSILON {
        return;
    }
    let cmd = ui.input(|i| i.modifiers.command);
    // The `Wheel ↕` toggle flips the pan/scrub direction only (not zoom).
    let wheel = raw_wheel * if editor.scroll_invert { -1.0 } else { 1.0 };

    if cmd {
        let old = editor.pixels_per_second;
        let new = zoomed_pps(old, raw_wheel, ZOOM_STEP);
        if !vertical {
            if let Some(pos) = response.hover_pos() {
                let anchor = editor.view_start + (pos.x - plot_left) / old;
                editor.view_start = (anchor - (pos.x - plot_left) / new).max(0.0);
            }
        }
        editor.pixels_per_second = new;
    } else if vertical {
        editor.pending_seek = Some((current_time - wheel * WHEEL_SECS).max(0.0));
    } else {
        editor.view_start = (editor.view_start - wheel * WHEEL_SECS).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{edge_pan, slide_trail_at, zoomed_pps, MAX_PPS, MIN_PPS};
    use crate::app::types::{ChartDoc, Note, NoteType, Slide};

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
        }
    }

    #[test]
    fn edge_pan_is_zero_in_the_middle_and_signed_at_the_edges() {        // 0..100 axis, middle band = 28..72.
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
}


