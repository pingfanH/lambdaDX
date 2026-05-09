use macroquad::prelude::*;
use macroquad::prelude::get_time;

use super::chart;
use super::state::AppState;
use super::types::{PadGeom, PointerEvent, RecordInputId, RectF, UiButton, DragPart, MOUSE_POINTER_ID, SPEED_MAX, SPEED_MIN, SPEED_STEP, TOUCH_SPEED_MIN, TOUCH_SPEED_MAX, TOUCH_SPEED_STEP, SCROLL_SPEED, LANE_COUNT, PREVIEW_LEAD_TIME, GRID_DIVISION, SCROLL_SPEED_FACTOR, SCROLL_INVERT, PAD_ZONE_MAX, is_touch_zone, sanitize_note_zone, hold_tail_time};
use super::ui::{rect_contains, trigger_ui_action};

pub(crate) fn handle_global_hotkeys(app: &mut AppState) {
    if is_key_pressed(KeyCode::Space) {
        app.toggle_play();
    }
    if is_key_pressed(KeyCode::R) {
        app.toggle_record();
    }
    if is_key_pressed(KeyCode::C) {
        app.recording_hits.clear();
        app.recording_notes.clear();
        app.active_record_holds.clear();
        app.active_pointer_zones.clear();
        app.prev_pointer_pos.clear();
        app.status = "Cleared recording hits".to_string();
    }
    if is_key_pressed(KeyCode::P) {
        app.show_pad_only = !app.show_pad_only;
        app.status = format!("Pad only: {}", app.show_pad_only);
    }
    if is_key_pressed(KeyCode::M) {
        app.mobile_ui = !app.mobile_ui;
        app.status = format!("Mobile UI mode: {}", app.mobile_ui);
    }
    if is_key_pressed(KeyCode::A) {
        app.audio_enabled = !app.audio_enabled;
        app.status = format!("Audio enabled: {}", app.audio_enabled);
        if !app.audio_enabled {
            app.stop_audio_if_any();
        }
    }

    // Record speed: [ and ]
    if is_key_pressed(KeyCode::LeftBracket) {
        app.set_record_speed((app.record_speed - SPEED_STEP).max(SPEED_MIN));
        app.status = format!("Record speed: {:.1}x", app.record_speed);
    }
    if is_key_pressed(KeyCode::RightBracket) {
        app.set_record_speed((app.record_speed + SPEED_STEP).min(SPEED_MAX));
        app.status = format!("Record speed: {:.1}x", app.record_speed);
    }

    // Playback speed: - and =
    if is_key_pressed(KeyCode::Minus) {
        app.set_play_speed((app.play_speed - SPEED_STEP).max(SPEED_MIN));
        app.status = format!("Playback speed: {:.1}x", app.play_speed);
    }
    if is_key_pressed(KeyCode::Equal) {
        app.set_play_speed((app.play_speed + SPEED_STEP).min(SPEED_MAX));
        app.status = format!("Playback speed: {:.1}x", app.play_speed);
    }

    if is_key_pressed(KeyCode::L) {
        match chart::load_latest_saved_chart() {
            Ok(chart) => {
                let n = chart.notes.len();
                app.chart = chart;
                app.status = format!("Loaded {n} notes");
            }
            Err(err) => {
                app.status = format!("Load latest failed: {err}");
            }
        }
    }

    if is_key_pressed(KeyCode::S) {
        match chart::save_recording_doc(app) {
            Ok(path) => app.status = format!("Saved recording: {}", path.display()),
            Err(err) => app.status = format!("Save failed: {err}"),
        }
    }

    // Delete selected note
    if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
        if let Some(i) = app.selected_note {
            if i < app.chart.notes.len() {
                app.chart.notes.remove(i);
                app.selected_note = None;
                app.status = format!("Deleted note #{i}");
            }
        }
    }
    let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
    let cmd = is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper);
    let mod_down = ctrl || cmd;

    // Ctrl/Cmd+Z undo
    if is_key_pressed(KeyCode::Z) && mod_down {
        eprintln!("[hotkey] undo");
        app.undo();
    }
    // Ctrl/Cmd+C copy
    if is_key_pressed(KeyCode::C) && mod_down {
        eprintln!("[hotkey] copy sel={:?} multi={}", app.selected_note, app.selected_notes.len());
        app.clipboard.clear();
        if app.selected_notes.is_empty() {
            if let Some(i) = app.selected_note {
                if let Some(n) = app.chart.notes.get(i) {
                    app.clipboard.push(n.clone());
                }
            }
        } else {
            for &i in &app.selected_notes {
                if let Some(n) = app.chart.notes.get(i) {
                    app.clipboard.push(n.clone());
                }
            }
        }
        eprintln!("[hotkey] copied {} notes", app.clipboard.len());
        if !app.clipboard.is_empty() { app.status = format!("Copied {} notes", app.clipboard.len()); }
    }
    // Ctrl/Cmd+V paste
    if is_key_pressed(KeyCode::V) && mod_down {
        eprintln!("[hotkey] paste clipboard_len={}", app.clipboard.len());
        if !app.clipboard.is_empty() {
            app.pasting = true;
            app.status = format!("Pasting {} notes — click to place", app.clipboard.len());
        }
    }
    if app.pasting && is_key_pressed(KeyCode::Escape) { eprintln!("[hotkey] cancel paste"); app.pasting = false; }
    // Toggle grid snap for recording
    if is_key_pressed(KeyCode::G) {
        app.record_snap_grid = !app.record_snap_grid;
        app.status = format!("Record snap to grid: {}", app.record_snap_grid);
    }
    // Waveform threshold
    if is_key_pressed(KeyCode::LeftBracket) {
        app.waveform_threshold = (app.waveform_threshold - 0.05).max(0.0);
        app.status = format!("Wave threshold: {:.2}", app.waveform_threshold);
    }
    if is_key_pressed(KeyCode::RightBracket) {
        app.waveform_threshold = (app.waveform_threshold + 0.05).min(1.0);
        app.status = format!("Wave threshold: {:.2}", app.waveform_threshold);
    }
}

pub(crate) fn handle_lane_input(app: &mut AppState) {
    let bindings = [
        (KeyCode::Key1, 1_u8),
        (KeyCode::Key2, 2_u8),
        (KeyCode::Key3, 3_u8),
        (KeyCode::Key4, 4_u8),
        (KeyCode::Key5, 5_u8),
        (KeyCode::Key6, 6_u8),
        (KeyCode::Key7, 7_u8),
        (KeyCode::Key8, 8_u8),
        (KeyCode::T, super::types::PAD_C_ZONE),
    ];

    for (key, lane) in bindings {
        if is_key_pressed(key) {
            let input_id = RecordInputId::Key(lane);
            app.start_record_hold_input(input_id, lane);
        }
        if is_key_released(key) {
            let input_id = RecordInputId::Key(lane);
            app.finish_record_hold_input(input_id);
        }
    }
}

pub(crate) fn collect_pointer_events() -> Vec<PointerEvent> {
    let touch_events = touches();
    let mut events = Vec::with_capacity(touch_events.len() + 2);
    let has_touch = !touch_events.is_empty();

    for t in touch_events {
        events.push(PointerEvent {
            id: t.id,
            phase: t.phase,
            position: t.position,
        });
    }

    if !has_touch {
        let (mx, my) = mouse_position();
        let pos = vec2(mx, my);
        if is_mouse_button_pressed(MouseButton::Left) {
            events.push(PointerEvent {
                id: MOUSE_POINTER_ID,
                phase: TouchPhase::Started,
                position: pos,
            });
        } else if is_mouse_button_down(MouseButton::Left) {
            events.push(PointerEvent {
                id: MOUSE_POINTER_ID,
                phase: TouchPhase::Stationary,
                position: pos,
            });
        }
        if is_mouse_button_released(MouseButton::Left) {
            events.push(PointerEvent {
                id: MOUSE_POINTER_ID,
                phase: TouchPhase::Ended,
                position: pos,
            });
        }
    }

    events
}

pub(crate) fn handle_touch_controls(
    app: &mut AppState,
    pad: PadGeom,
    buttons: &[UiButton],
    pointer_events: &[PointerEvent],
) {
    for ev in pointer_events {
        match ev.phase {
            TouchPhase::Started => {
                app.prev_pointer_pos.insert(ev.id, ev.position);

                if let Some(btn) = buttons.iter().find(|b| rect_contains(b.rect, ev.position)) {
                    trigger_ui_action(app, btn.action);
                    continue;
                }

                let zone = app.pad_svg.as_ref()
                    .and_then(|svg| svg.hit_test(ev.position, &pad));
                if let Some(zone) = zone {
                    app.active_pointer_zones.insert(ev.id, zone);
                    app.push_feedback(zone, 0.12);
                    app.start_record_hold_input(RecordInputId::Pointer(ev.id), zone);
                }
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                let prev = app.prev_pointer_pos.get(&ev.id).copied();
                app.prev_pointer_pos.insert(ev.id, ev.position);

                let samples = if let Some(p) = prev {
                    let dist = ev.position.distance(p);
                    let steps = (dist / 4.0).ceil() as usize;
                    if steps > 0 {
                        let mut pts = Vec::with_capacity(steps + 1);
                        for i in 0..=steps {
                            let t = i as f32 / (steps + 1) as f32;
                            pts.push(p + (ev.position - p) * t);
                        }
                        pts
                    } else {
                        vec![ev.position]
                    }
                } else {
                    vec![ev.position]
                };

                for sample in &samples {
                    let old_zone = app.active_pointer_zones.get(&ev.id).copied();
                    let new_zone = app.pad_svg.as_ref()
                        .and_then(|svg| svg.hit_test(*sample, &pad));

                    if old_zone != new_zone {
                        if let Some(zone) = new_zone {
                            app.record_slide_zone(RecordInputId::Pointer(ev.id), zone);
                            app.active_pointer_zones.insert(ev.id, zone);
                            app.push_feedback(zone, 0.10);
                        } else {
                            app.active_pointer_zones.remove(&ev.id);
                        }
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                app.prev_pointer_pos.remove(&ev.id);
                app.active_pointer_zones.remove(&ev.id);
                app.finish_record_hold_input(RecordInputId::Pointer(ev.id));
            }
            _ => {}
        }
    }
}

fn snap_to_grid(time: f32, bpm: f32) -> f32 {
    let beat = 60.0 / bpm;
    let grid = beat / (GRID_DIVISION as f32 / 4.0);
    (time / grid).round() * grid
}

pub(crate) fn handle_timeline_editing(app: &mut AppState, timeline_rect: Option<RectF>) {
    let Some(tl) = timeline_rect else { return };
    let (mx, my) = mouse_position();
    let pos = vec2(mx, my);

    // Scroll / middle-drag: move actual playback progress
    let (_, wy) = mouse_wheel();
    let mut shift = 0.0_f32;
    let dir = if SCROLL_INVERT { 1.0 } else { -1.0 };
    if wy != 0.0 { shift = dir * wy * SCROLL_SPEED_FACTOR; }
    if is_mouse_button_down(MouseButton::Middle) { shift = -mouse_delta_position().y * 0.02; }
    if shift != 0.0 {
        if matches!(app.mode, super::types::Mode::Playing) {
            app.mode_song_offset = app.song_time();
            app.mode_wall_anchor = macroquad::prelude::get_time();
        }
        app.mode_song_offset = (app.mode_song_offset + shift).max(0.0);
        app.timeline_view_time = app.mode_song_offset;
        app.seek_audio_to(app.mode_song_offset);
    }
    if !(pos.x >= tl.x && pos.x <= tl.x + tl.w && pos.y >= tl.y && pos.y <= tl.y + tl.h) { return; }

    let now = match app.mode {
        super::types::Mode::Playing | super::types::Mode::Recording => app.song_time(),
        _ => app.timeline_view_time,
    };

    // Update view time when not playing
    if !matches!(app.mode, super::types::Mode::Playing | super::types::Mode::Recording) {
        app.timeline_view_time = now;
    }

    let track_x = tl.x + 14.0;
    let track_y = tl.y + 66.0;
    let track_w = tl.w - 28.0;
    let track_h = tl.h - 80.0;
    let ruler_w = 64.0;
    let lanes_w = track_w - ruler_w;
    let lane_w = lanes_w / LANE_COUNT as f32;
    let judge_y = track_y + track_h - 38.0;
    let lanes_x = track_x + ruler_w;

    // ── Ruler scrub ──
    if pos.x >= track_x && pos.x <= lanes_x && is_mouse_button_down(MouseButton::Left) {
        let dt = (judge_y - pos.y) / SCROLL_SPEED;
        let new_t = (now + dt).max(0.0);
        if matches!(app.mode, super::types::Mode::Playing) {
            app.mode_song_offset = new_t; app.mode_wall_anchor = get_time(); app.seek_audio_to(new_t);
        } else { app.timeline_view_time = new_t; }
        app.selected_note = None; app.dragging_note = None; app.drag_part = None;
        return;
    }

    // ── Right-click delete ──
    if is_mouse_button_pressed(MouseButton::Right) && pos.x >= lanes_x {
        let mut best: Option<usize> = None; let mut best_d = 30.0;
        for (i, note) in app.chart.notes.iter().enumerate() {
            let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y);
            let d = pos.distance(vec2(cx, ny));
            if d < best_d { best = Some(i); best_d = d; }
        }
        if let Some(i) = best {
            app.push_undo();
            app.chart.notes.remove(i);
            app.recompute_each();
            app.selected_note = None; app.dragging_note = None;
            app.status = format!("Deleted note #{i}");
        }
        return;
    }

    // ── Mouse press: select or start box-drag ──
    let drag_threshold = 8.0;
    if is_mouse_button_pressed(MouseButton::Left) && pos.x >= lanes_x {
        // Try to find a note near the click
        let mut best: Option<usize> = None; let mut best_d = 30.0;
        let mut best_part = DragPart::Body;
        for (i, note) in app.chart.notes.iter().enumerate() {
            let (cx, ny, tail_ny, is_hold) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y);
            let d = pos.distance(vec2(cx, ny));
            if is_hold {
                let tail_d = pos.distance(vec2(cx, tail_ny));
                let mid_y = (ny + tail_ny) * 0.5;
                let mid_d = pos.distance(vec2(cx, mid_y));
                if tail_d < best_d && tail_d < d && tail_d < mid_d { best = Some(i); best_d = tail_d; best_part = DragPart::Tail; }
                else if d < best_d && d < mid_d && d < tail_d { best = Some(i); best_d = d; best_part = DragPart::Head; }
                else if mid_d < best_d && mid_d < d && mid_d < tail_d { best = Some(i); best_d = mid_d; best_part = DragPart::Body; }
            } else if d < best_d { best = Some(i); best_d = d; best_part = DragPart::Body; }
        }
        if let Some(i) = best {
            app.selected_note = Some(i);
            app.drag_start_pos = Some(pos);
            app.drag_start_time = app.chart.notes[i].time;
            app.drag_part = Some(best_part);
            app.drag_orig_note = Some(app.chart.notes[i].clone());
        } else {
            // Start box selection
            app.selected_note = None;
            app.box_start = Some(pos);
            app.box_end = Some(pos);
            app.drag_start_pos = Some(pos);
            app.drag_part = None;
            app.drag_orig_note = None;
        }
    }

    // ── Dragging: note move or box selection ──
    if is_mouse_button_down(MouseButton::Left) {
        let moved = app.drag_start_pos.map(|s| pos.distance(s) >= drag_threshold).unwrap_or(false);
        if !moved { } // just selecting
        // Update box end during box drag
        else if app.drag_part.is_none() { app.box_end = Some(pos); }
        else if let Some(i) = app.dragging_note.or(app.selected_note).filter(|&i| i < app.chart.notes.len()) {
            // Note dragging (only after threshold)
            if app.dragging_note.is_none() && app.selected_note.is_some() {
                app.push_undo();
                app.dragging_note = Some(i);
                app.drag_orig_note = app.chart.notes.get(i).cloned();
                // Save all selected notes' original state for multi-move
                app.drag_multi_orig.clear();
                for &si in &app.selected_notes {
                    if let Some(n) = app.chart.notes.get(si) {
                        app.drag_multi_orig.push((si, n.time, n.lane));
                    }
                }
            }
            let dt = (app.drag_start_pos.unwrap_or(pos).y - pos.y) / SCROLL_SPEED;
            let new_t = snap_to_grid((app.drag_start_time + dt).max(0.0), app.chart.bpm);
            let lx = pos.x - lanes_x;
            let new_lane = if lx >= 0.0 {
                let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
                if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
            } else { 1 };
            // Multi-select: move all selected notes by delta
            if app.selected_notes.len() > 1 && app.selected_notes.contains(&i) {
                let orig = app.drag_orig_note.as_ref();
                let t_delta = new_t - orig.map(|o| o.time).unwrap_or(new_t);
                let l_delta = new_lane as i32 - orig.map(|o| o.lane as i32).unwrap_or(new_lane as i32);
                for &(si, orig_t, orig_l) in &app.drag_multi_orig {
                    if let Some(note) = app.chart.notes.get_mut(si) {
                        note.time = snap_to_grid((orig_t + t_delta).max(0.0), app.chart.bpm);
                        note.lane = (orig_l as i32 + l_delta).clamp(1, PAD_ZONE_MAX as i32) as u8;
                    }
                }
                app.status = format!("Moving {} notes", app.selected_notes.len());
            } else {
                // Single note drag
                let Some(note) = app.chart.notes.get_mut(i) else { app.dragging_note = None; return; };
                let orig = app.drag_orig_note.as_ref();
                let part = app.drag_part.unwrap_or(DragPart::Body);
                match part {
                    DragPart::Head => { if let Some(o) = orig { let tail = o.time + o.hold_duration.max(0.15); note.time = new_t; note.hold_duration = (tail - new_t).max(0.0); } }
                    DragPart::Tail => { if let Some(o) = orig { note.hold_duration = (new_t - o.time).max(0.0); } }
                    DragPart::Body => { if let Some(o) = orig { let len = o.hold_duration; note.time = new_t; note.hold_duration = len; } else { note.time = new_t; } }
                }
                note.lane = new_lane;
                app.status = format!("#{i}: t={:.2} dur={:.2}", note.time, note.hold_duration);
            }
        }
    }
    if is_mouse_button_released(MouseButton::Left) {
        // Box selection: select all notes within drag rectangle
        if let (Some(start), None) = (app.drag_start_pos, app.drag_part) {
            let moved = pos.distance(start) >= drag_threshold;
            if moved {
                let x1 = start.x.min(pos.x); let x2 = start.x.max(pos.x);
                let y1 = start.y.min(pos.y); let y2 = start.y.max(pos.y);
                app.selected_notes.clear();
                for (i, note) in app.chart.notes.iter().enumerate() {
                    let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y);
                    if cx >= x1 && cx <= x2 && ny >= y1 && ny <= y2 { app.selected_notes.push(i); }
                }
                if !app.selected_notes.is_empty() { app.selected_note = Some(app.selected_notes[0]); }
                app.status = format!("Selected {} notes", app.selected_notes.len());
            } else {
                // Click empty → place note
                app.push_undo();
                let dt = (judge_y - pos.y) / SCROLL_SPEED;
                let t = snap_to_grid((now + dt).max(0.0), app.chart.bpm);
                let lx = pos.x - lanes_x;
                let lane = if lx >= 0.0 {
                    let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
                    if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
                } else { 1 };
                let nt = if is_touch_zone(sanitize_note_zone(super::types::NoteType::Tap, lane)) { super::types::NoteType::Touch } else { super::types::NoteType::Tap };
                app.chart.notes.push(super::types::Note { time: t, lane, note_type: nt, hold_duration: 0.0, is_each: false, slide_points: vec![], slide_duration: 0.0, slide_shape: None });
                app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
                app.recompute_each();
                app.status = format!("Placed {} at {:.2}s", if nt == super::types::NoteType::Tap {"Tap"} else {"Touch"}, t);
            }
        }
        app.dragging_note = None; app.drag_start_pos = None; app.drag_orig_note = None;
        app.box_start = None; app.box_end = None;
        app.recompute_each();
    }
    // ── Click while pasting: place notes ──
    if app.pasting && is_mouse_button_pressed(MouseButton::Left) && pos.x >= lanes_x {
        app.push_undo();
        let min_t = app.clipboard.iter().map(|n| n.time).fold(f32::MAX, f32::min);
        let dt = (judge_y - pos.y) / SCROLL_SPEED;
        let target = snap_to_grid((now + dt).max(0.0), app.chart.bpm);
        let offset = target - min_t;
        let lx = pos.x - lanes_x;
        let tgt_lane = if lx >= 0.0 {
            let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
            if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
        } else { 1 };
        let anchor_lane = app.clipboard.first().map(|n| n.lane).unwrap_or(1);
        let lane_off = tgt_lane as i32 - anchor_lane as i32;
        for mut n in app.clipboard.clone() {
            n.time = (n.time + offset).max(0.0);
            n.lane = (n.lane as i32 + lane_off).clamp(1, super::types::PAD_ZONE_MAX as i32) as u8;
            app.chart.notes.push(n);
        }
        app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        app.recompute_each();
        app.pasting = false;
        app.status = format!("Placed {} notes", app.clipboard.len());
    }
}

/// Get screen position of a note. Returns (cx, head_y, tail_y, is_hold)
fn note_screen_pos(note: &super::types::Note, now: f32, track_x: f32, ruler_w: f32, lane_w: f32, judge_y: f32) -> (f32, f32, f32, bool) {
    let zone = sanitize_note_zone(note.note_type, note.lane);
    let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
    let cx = track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
    let dt = note.time - now;
    let ny = judge_y - dt * SCROLL_SPEED;
    let is_hold = matches!(note.note_type, super::types::NoteType::Hold);
    let tail_ny = if is_hold { judge_y - (hold_tail_time(note) - now) * SCROLL_SPEED } else { ny };
    (cx, ny, tail_ny, is_hold)
}