use macroquad::prelude::*;
use macroquad::prelude::get_time;

use super::chart;
use super::state::AppState;
use super::types::{Note, NoteType, PadGeom, PointerEvent, RecordInputId, RectF, UiButton, DragPart, MOUSE_POINTER_ID, SPEED_MAX, SPEED_MIN, SPEED_STEP, SCROLL_SPEED, LANE_COUNT, SCROLL_SPEED_FACTOR, SCROLL_INVERT, PAD_ZONE_MAX, is_touch_zone, sanitize_note_zone, hold_tail_time, note_secs, secs_to_measure, mdur_to_secs, snap_measure};
use super::ui::{rect_contains, trigger_ui_action};

/// Collect indices of selected notes (multi-select or single).
fn gather_selected(app: &AppState) -> Vec<usize> {
    if !app.selected_notes.is_empty() {
        app.selected_notes.clone()
    } else if let Some(i) = app.selected_note {
        vec![i]
    } else {
        vec![]
    }
}

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
        app.set_status("Cleared recording hits".to_string());
    }
    if is_key_pressed(KeyCode::P) {
        app.show_pad_only = !app.show_pad_only;
        app.set_status(format!("Pad only: {}", app.show_pad_only));
    }
    if is_key_pressed(KeyCode::M) {
        app.mobile_ui = !app.mobile_ui;
        app.set_status(format!("Mobile UI mode: {}", app.mobile_ui));
    }
    if is_key_pressed(KeyCode::A) {
        app.audio_enabled = !app.audio_enabled;
        app.set_status(format!("Audio enabled: {}", app.audio_enabled));
        if !app.audio_enabled {
            app.stop_audio_if_any();
        }
    }

    // Record speed: [ and ]
    if is_key_pressed(KeyCode::LeftBracket) {
        app.set_record_speed((app.record_speed - SPEED_STEP).max(SPEED_MIN));
        app.set_status(format!("Record speed: {:.1}x", app.record_speed));
    }
    if is_key_pressed(KeyCode::RightBracket) {
        app.set_record_speed((app.record_speed + SPEED_STEP).min(SPEED_MAX));
        app.set_status(format!("Record speed: {:.1}x", app.record_speed));
    }

    // Playback speed: - and =
    if is_key_pressed(KeyCode::Minus) {
        app.set_play_speed((app.play_speed - SPEED_STEP).max(SPEED_MIN));
        app.set_status(format!("Playback speed: {:.1}x", app.play_speed));
    }
    if is_key_pressed(KeyCode::Equal) {
        app.set_play_speed((app.play_speed + SPEED_STEP).min(SPEED_MAX));
        app.set_status(format!("Playback speed: {:.1}x", app.play_speed));
    }

    if is_key_pressed(KeyCode::L) {
        match chart::load_latest_saved_chart() {
            Ok(chart) => {
                let n = chart.notes.len();
                app.set_chart(chart);
                app.set_status(format!("Loaded {n} notes"));
            }
            Err(err) => {
                app.set_status(format!("Load latest failed: {err}"));
            }
        }
    }

    if is_key_pressed(KeyCode::S) {
        match chart::save_recording_doc(app) {
            Ok(path) => app.set_status(format!("Saved recording: {}", path.display())),
            Err(err) => app.set_status(format!("Save failed: {err}")),
        }
    }

    // Delete selected note (skipped while editing a slide trajectory: there
    // Backspace pops the last slide point instead).
    if (is_key_pressed(KeyCode::Delete)
        || (is_key_pressed(KeyCode::Backspace) && app.editing_slide_path.is_none()))
    {
        if let Some(i) = app.selected_note {
            if i < app.chart.notes.len() {
                app.chart.notes.remove(i);
                app.set_selected_note(None);
                app.set_status(format!("Deleted note #{i}"));
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
        if !app.clipboard.is_empty() { app.set_status(format!("Copied {} notes", app.clipboard.len())); }
    }
    // Ctrl/Cmd+V paste
    if is_key_pressed(KeyCode::V) && mod_down {
        eprintln!("[hotkey] paste clipboard_len={}", app.clipboard.len());
        if !app.clipboard.is_empty() {
            app.pasting = true;
            app.set_status(format!("Pasting {} notes — click to place", app.clipboard.len()));
        }
    }
    if app.pasting && is_key_pressed(KeyCode::Escape) { eprintln!("[hotkey] cancel paste"); app.pasting = false; }

    // E: toggle slide-trajectory edit mode for the selected slide note.
    // While active, clicking pad zones appends to the note's slide_points.
    if is_key_pressed(KeyCode::E) && !mod_down {
        if app.editing_slide_path.is_some() {
            app.set_editing_slide_path(None);
            app.set_status("Trajectory edit: off".to_string());
        } else if let Some(i) = app.selected_note {
            if let Some(n) = app.chart.notes.get(i) {
                if matches!(n.note_type, super::types::NoteType::Slide) {
                    app.set_editing_slide_path(Some(i));
                    app.set_status(format!(
                        "Trajectory edit: click pad zones to append (Backspace=undo, Esc=exit). #{i}"
                    ));
                } else {
                    app.set_status("Trajectory edit: select a Slide note first".to_string());
                }
            }
        } else {
            app.set_status("Trajectory edit: no note selected".to_string());
        }
    }
    // Esc exits trajectory edit mode.
    if is_key_pressed(KeyCode::Escape) && app.editing_slide_path.is_some() {
        app.set_editing_slide_path(None);
        app.set_status("Trajectory edit: off".to_string());
    }
    // Backspace removes last point of the slide being edited (when not deleting note).
    if app.editing_slide_path.is_some() && is_key_pressed(KeyCode::Backspace) {
        if let Some(i) = app.editing_slide_path {
            if let Some(n) = app.chart.notes.get_mut(i) {
                if let Some(removed) = n.slide_points.pop() {
                    n.slide_shape = super::slide_match::match_slide_shape(n.lane, &n.slide_points);
                    app.set_status(format!("Removed zone {}", removed.zone));
                }
            }
        }
    }
    // B: toggle Break on selected note(s)
    // In star-edit mode, toggles star_is_break on Slide notes only.
    if is_key_pressed(KeyCode::B) && !mod_down {
        let indices = gather_selected(app);
        if !indices.is_empty() {
            app.push_undo();
            let star_mode = app.editing_star;
            let mut count = 0;
            for &i in &indices {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    match n.note_type {
                        NoteType::Slide if star_mode => {
                            n.star_is_break = !n.star_is_break;
                            count += 1;
                        }
                        NoteType::Tap | NoteType::Hold | NoteType::Slide => {
                            n.is_break = !n.is_break;
                            count += 1;
                        }
                        _ => {}
                    }
                }
            }
            let label = if star_mode { "Star Break" } else { "Break" };
            let on = app.chart.notes.get(*indices.first().unwrap()).map(|n| {
                if star_mode { n.star_is_break } else { n.is_break }
            }).unwrap_or(false);
            app.set_status(format!("{} {}: {} notes", label, if on { "ON" } else { "OFF" }, count));
        }
    }
    // X: toggle Ex on selected note(s)
    // In star-edit mode, toggles star_is_ex on Slide notes only.
    if is_key_pressed(KeyCode::X) && !mod_down {
        let indices = gather_selected(app);
        if !indices.is_empty() {
            app.push_undo();
            let star_mode = app.editing_star;
            let mut count = 0;
            for &i in &indices {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    match n.note_type {
                        NoteType::Slide if star_mode => {
                            n.star_is_ex = !n.star_is_ex;
                            count += 1;
                        }
                        NoteType::Tap | NoteType::Hold | NoteType::Slide => {
                            n.is_ex = !n.is_ex;
                            count += 1;
                        }
                        _ => {}
                    }
                }
            }
            let label = if star_mode { "Star Ex" } else { "Ex" };
            let on = app.chart.notes.get(*indices.first().unwrap()).map(|n| {
                if star_mode { n.star_is_ex } else { n.is_ex }
            }).unwrap_or(false);
            app.set_status(format!("{} {}: {} notes", label, if on { "ON" } else { "OFF" }, count));
        }
    }
    // N: toggle ExBreak on selected note(s) (both break + ex)
    // In star-edit mode, toggles star_is_break + star_is_ex.
    if is_key_pressed(KeyCode::N) && !mod_down {
        let indices = gather_selected(app);
        if !indices.is_empty() {
            app.push_undo();
            let star_mode = app.editing_star;
            let mut count = 0;
            for &i in &indices {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    match n.note_type {
                        NoteType::Slide if star_mode => {
                            let target = !(n.star_is_break && n.star_is_ex);
                            n.star_is_break = target;
                            n.star_is_ex = target;
                            count += 1;
                        }
                        NoteType::Tap | NoteType::Hold | NoteType::Slide => {
                            let target = !(n.is_break && n.is_ex);
                            n.is_break = target;
                            n.is_ex = target;
                            count += 1;
                        }
                        _ => {}
                    }
                }
            }
            let label = if star_mode { "Star ExBreak" } else { "ExBreak" };
            let on = app.chart.notes.get(*indices.first().unwrap()).map(|n| {
                if star_mode { n.star_is_break && n.star_is_ex } else { n.is_break && n.is_ex }
            }).unwrap_or(false);
            app.set_status(format!("{} {}: {} notes", label, if on { "ON" } else { "OFF" }, count));
        }
    }

    // Toggle grid snap for recording
    if is_key_pressed(KeyCode::G) {
        app.record_snap_grid = !app.record_snap_grid;
        app.set_status(format!("Record snap to grid: {}", app.record_snap_grid));
    }
    // Waveform threshold
    if is_key_pressed(KeyCode::LeftBracket) {
        app.waveform_threshold = (app.waveform_threshold - 0.05).max(0.0);
        app.set_status(format!("Wave threshold: {:.2}", app.waveform_threshold));
    }
    if is_key_pressed(KeyCode::RightBracket) {
        app.waveform_threshold = (app.waveform_threshold + 0.05).min(1.0);
        app.set_status(format!("Wave threshold: {:.2}", app.waveform_threshold));
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

                // Slide-trajectory edit mode: clicks append to slide_points
                // of the selected slide note (Idle mode only; recording/playing
                // keep their normal behaviour).
                if matches!(app.mode, super::types::Mode::Idle) {
                    if let (Some(i), Some(z)) = (app.editing_slide_path, zone) {
                        let mut handled = false;
                        let mut new_count = 0usize;
                        if let Some(n) = app.chart.notes.get_mut(i) {
                            if matches!(n.note_type, super::types::NoteType::Slide) {
                                let beat_offset = (n.slide_points.len() as f32 + 1.0)
                                    * (n.slide_duration.max(0.3)
                                        / (n.slide_points.len() as f32 + 2.0));
                                let last_zone = n.slide_points.last().map(|p| p.zone)
                                    .unwrap_or(n.lane);
                                if z != last_zone {
                                    n.slide_points.push(super::types::SlidePoint {
                                        zone: z, beat_offset,
                                    });
                                    n.slide_shape = super::slide_match::match_slide_shape(
                                        n.lane, &n.slide_points,
                                    );
                                    new_count = n.slide_points.len();
                                }
                                handled = true;
                            }
                        }
                        if handled {
                            if new_count > 0 {
                                app.push_feedback(z, 0.18);
                                app.set_status(format!("Added zone {} (#{} points)", z, new_count));
                            }
                            continue;
                        }
                    }
                }

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

/// Snap a screen-seconds value to the nearest measure grid position.
/// Returns the snapped value in **measures**.
fn snap_secs_to_measure(secs: f32, bpm: f32) -> f32 {
    snap_measure(secs_to_measure(secs, bpm))
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
    // Edge auto-scroll while box-selecting: cursor above/below the timeline
    // pans the view so the user can extend the selection beyond the viewport.
    if app.box_anchor_t.is_some() && is_mouse_button_down(MouseButton::Left) {
        let above = (tl.y - pos.y).max(0.0);
        let below = (pos.y - (tl.y + tl.h)).max(0.0);
        // Speed scales with how far past the edge the cursor is (in chart time).
        // Frame-time-multiplied so it's roughly framerate-independent.
        let dt_frame = macroquad::prelude::get_frame_time().min(0.05);
        let edge_speed = (above - below) / SCROLL_SPEED * 4.0; // chart-seconds per second
        let edge_shift = edge_speed * dt_frame;
        if edge_shift != 0.0 {
            shift += edge_shift;
        }
    }
    if shift != 0.0 {
        if matches!(app.mode, super::types::Mode::Playing) {
            app.mode_song_offset = app.song_time();
            app.mode_wall_anchor = macroquad::prelude::get_time();
        }
        app.mode_song_offset = (app.mode_song_offset + shift).max(0.0);
        app.timeline_view_time = app.mode_song_offset;
        app.seek_audio_to(app.mode_song_offset);
    }
    let inside_tl = pos.x >= tl.x && pos.x <= tl.x + tl.w && pos.y >= tl.y && pos.y <= tl.y + tl.h;
    // Allow processing while any drag (note move or box-select) is active so
    // release events fire even if the cursor leaves the timeline rect.
    let drag_active = app.dragging_note.is_some() || app.box_start.is_some()
        || (app.drag_start_pos.is_some() && is_mouse_button_down(MouseButton::Left));
    if !inside_tl && !drag_active { return; }

    let now = match app.mode {
        super::types::Mode::Playing | super::types::Mode::Recording => app.song_time(),
        _ => app.timeline_view_time,
    };

    // Update view time when not playing
    if !matches!(app.mode, super::types::Mode::Playing | super::types::Mode::Recording) {
        app.timeline_view_time = now;
    }

    let sidebar_w = super::types::TIMELINE_SIDEBAR_W;
    let track_x = tl.x + 14.0 + sidebar_w;
    let track_y = tl.y + 66.0;
    let track_w = tl.w - 28.0 - sidebar_w;
    let track_h = tl.h - 80.0;
    let ruler_w = 64.0;
    let lanes_w = track_w - ruler_w;
    let lane_w = lanes_w / LANE_COUNT as f32;
    let judge_y = track_y + track_h - 38.0;
    let lanes_x = track_x + ruler_w;

    // Re-derive the box-select start screen y from its chart-time anchor each
    // frame, so scrolling pans the entire selection rectangle along with the
    // notes (the start sticks to its original chart time, not its screen y).
    if let (Some(anchor_t), Some(start)) = (app.box_anchor_t, app.box_start.as_mut()) {
        start.y = judge_y - (anchor_t - now) * SCROLL_SPEED;
    }

    // ── Sidebar tool buttons (Tap / Hold / Star) ──
    if is_mouse_button_pressed(MouseButton::Left) {
        for (btn, tool, _label) in super::types::timeline_sidebar_buttons(&tl) {
            if pos.x >= btn.x && pos.x <= btn.x + btn.w
                && pos.y >= btn.y && pos.y <= btn.y + btn.h
            {
                if app.place_tool != tool {
                    app.place_tool = tool;
                    app.placement = super::types::PlacementState::Idle;
                    app.set_status(format!("Tool: {:?}", tool));
                }
                return;
            }
        }
    }
    // Escape cancels any in-progress multi-step placement.
    if is_key_pressed(KeyCode::Escape) {
        if !matches!(app.placement, super::types::PlacementState::Idle) {
            app.placement = super::types::PlacementState::Idle;
            app.set_status("Placement cancelled".to_string());
        }
    }

    // ── Ruler scrub ──
    if pos.x >= track_x && pos.x <= lanes_x && is_mouse_button_down(MouseButton::Left) {
        let dt = (judge_y - pos.y) / SCROLL_SPEED;
        let new_t = (now + dt).max(0.0);
        if matches!(app.mode, super::types::Mode::Playing) {
            app.mode_song_offset = new_t; app.mode_wall_anchor = get_time(); app.seek_audio_to(new_t);
        } else { app.timeline_view_time = new_t; }
        app.set_selected_note(None); app.dragging_note = None; app.drag_part = None;
        return;
    }

    // ── Right-click delete ──
    if is_mouse_button_pressed(MouseButton::Right) && pos.x >= lanes_x {
        let mut best: Option<usize> = None; let mut best_d = 30.0;
        for (i, note) in app.chart.notes.iter().enumerate() {
            let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, app.chart.bpm);
            let d = pos.distance(vec2(cx, ny));
            if d < best_d { best = Some(i); best_d = d; }
        }
        if let Some(i) = best {
            app.push_undo();
            app.chart.notes.remove(i);
            app.recompute_each();
            app.set_selected_note(None); app.dragging_note = None;
            app.set_status(format!("Deleted note #{i}"));
        }
        return;
    }

    // ── Middle-click: enter star-edit mode (selects the slide, toggles editing_star) ──
    if is_mouse_button_pressed(MouseButton::Middle) && pos.x >= lanes_x {
        let mut best: Option<usize> = None; let mut best_d = 30.0_f32;
        for (i, note) in app.chart.notes.iter().enumerate() {
            if !matches!(note.note_type, NoteType::Slide) { continue; }
            let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, app.chart.bpm);
            let d = pos.distance(vec2(cx, ny));
            if d < best_d { best = Some(i); best_d = d; }
        }
        if let Some(i) = best {
            app.set_selected_note(Some(i));
            app.editing_star = true;
            app.set_status(format!("Star edit mode: #{i} (B=star break, X=star ex)"));
        } else {
            app.editing_star = false;
        }
    }

    // ── Mouse press: record candidate, do NOT select yet ──
    let drag_threshold = 8.0;
    if is_mouse_button_pressed(MouseButton::Left) && pos.x >= lanes_x {
        // Exit star-edit mode on left-click
        app.editing_star = false;
        // Hit-test: find nearest note (potential drag target)
        let mut best: Option<usize> = None; let mut best_d = 30.0;
        let mut best_part = DragPart::Body;
        for (i, note) in app.chart.notes.iter().enumerate() {
            let (cx, ny, tail_ny, has_tail) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, app.chart.bpm);
            let d = pos.distance(vec2(cx, ny));
            if matches!(note.note_type, super::types::NoteType::Slide) && note.slide_duration > 0.0 {
                let ns = note_secs(note, app.chart.bpm);
                let delay_secs = ns + mdur_to_secs(note.slide_start_delay, app.chart.bpm);
                let delay_y = judge_y - (delay_secs - now) * SCROLL_SPEED;
                let delay_d = pos.distance(vec2(cx, delay_y));
                let tail_d = pos.distance(vec2(cx, tail_ny));
                if delay_d < best_d && delay_d < d && delay_d < tail_d {
                    best = Some(i); best_d = delay_d; best_part = DragPart::SlideDelayEnd;
                    continue;
                }
            }
            if has_tail {
                let tail_d = pos.distance(vec2(cx, tail_ny));
                let mid_y = (ny + tail_ny) * 0.5;
                let mid_d = pos.distance(vec2(cx, mid_y));
                if tail_d < best_d && tail_d < d && tail_d < mid_d { best = Some(i); best_d = tail_d; best_part = DragPart::Tail; }
                else if d < best_d && d < mid_d && d < tail_d { best = Some(i); best_d = d; best_part = DragPart::Head; }
                else if mid_d < best_d && mid_d < d && mid_d < tail_d { best = Some(i); best_d = mid_d; best_part = DragPart::Body; }
            } else if d < best_d { best = Some(i); best_d = d; best_part = DragPart::Body; }
        }
        let cursor_t_at_click = (now + (judge_y - pos.y) / SCROLL_SPEED).max(0.0);
        app.drag_cursor_anchor_t = cursor_t_at_click;

        if let Some(i) = best {
            // Double-click on a Slide head → fork a new slide from same star
            let click_now = get_time();
            let is_dbl = app.last_click_note == Some(i)
                && (click_now - app.last_click_time) < 0.4
                && matches!(best_part, DragPart::Body | DragPart::Head);
            app.last_click_time = click_now;
            app.last_click_note = Some(i);
            if is_dbl && matches!(app.chart.notes.get(i).map(|n| n.note_type), Some(NoteType::Slide)) {
                let src = app.chart.notes[i].clone();
                app.push_undo();
                let new_note = Note {
                    time: src.time, lane: src.lane, note_type: NoteType::Slide,
                    hold_duration: 0.0, is_each: src.is_each,
                    is_break: false, is_ex: false, is_star: false, is_tapless: false,
                    star_is_break: src.star_is_break, star_is_ex: src.star_is_ex,
                    slide_points: vec![], slide_duration: src.slide_duration,
                    slide_start_delay: src.slide_start_delay, slide_shape: None,
                };
                app.chart.notes.push(new_note);
                app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
                let new_i = app.chart.notes.iter().position(|n| {
                    n.time == src.time && n.lane == src.lane
                        && matches!(n.note_type, NoteType::Slide)
                        && n.slide_points.is_empty()
                }).unwrap_or(app.chart.notes.len() - 1);
                app.set_selected_note(Some(new_i));
                app.set_editing_slide_path(Some(new_i));
                app.set_status(format!("Forked new slide from star #{i} — click pad zones to build path"));
                return;
            }
            // Record candidate for potential drag — do NOT select yet
            app.press_note_candidate = Some(i);
            app.drag_start_pos = Some(pos);
            let n = &app.chart.notes[i];
            let bpm = app.chart.bpm;
            app.drag_start_time = match best_part {
                DragPart::Tail => match n.note_type {
                    super::types::NoteType::Hold => hold_tail_time(n, bpm),
                    super::types::NoteType::Slide => note_secs(n, bpm) + mdur_to_secs(n.slide_duration, bpm),
                    _ => note_secs(n, bpm),
                },
                DragPart::SlideDelayEnd => note_secs(n, bpm) + mdur_to_secs(n.slide_start_delay, bpm),
                _ => note_secs(n, bpm),
            };
            app.drag_part = Some(best_part);
            app.drag_orig_note = Some(app.chart.notes[i].clone());
        } else {
            // No note under cursor — prepare for potential box-select
            app.press_note_candidate = None;
            app.drag_start_pos = Some(pos);
            app.box_anchor_t = Some(cursor_t_at_click);
            app.drag_part = None;
            app.drag_orig_note = None;
        }
    }

    // ── Dragging: note move or box selection ──
    if is_mouse_button_down(MouseButton::Left) {
        let moved = app.drag_start_pos.map(|s| pos.distance(s) >= drag_threshold).unwrap_or(false);
        if !moved { /* waiting for threshold */ }
        // Promote candidate → dragging (on first frame beyond threshold)
        else if app.press_note_candidate.is_some() && app.dragging_note.is_none() {
            let i = app.press_note_candidate.take().unwrap();
            if i < app.chart.notes.len() {
                app.push_undo();
                app.set_selected_note(Some(i));
                app.dragging_note = Some(i);
                app.drag_orig_note = app.chart.notes.get(i).cloned();
                app.drag_multi_orig.clear();
                for &si in &app.selected_notes {
                    if let Some(n) = app.chart.notes.get(si) {
                        app.drag_multi_orig.push((si, n.time, n.lane));
                    }
                }
            }
        }
        // No candidate → box selection
        else if app.drag_part.is_none() && app.dragging_note.is_none() {
            if app.box_start.is_none() {
                app.box_start = app.drag_start_pos;
            }
            app.box_end = Some(pos);
        }
        // Active note drag
        else if let Some(i) = app.dragging_note.filter(|&i| i < app.chart.notes.len()) {
            // Absolute cursor tracking: compute chart time under the current
            // cursor each frame, then add the offset captured at click time.
            // This lets drags follow the mouse even if the user scrolls the
            // timeline while dragging.
            let cursor_t_now = (now + (judge_y - pos.y) / SCROLL_SPEED).max(0.0);
            let new_t = snap_secs_to_measure(
                (cursor_t_now + (app.drag_start_time - app.drag_cursor_anchor_t)).max(0.0),
                app.chart.bpm,
            );
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
                        note.time = snap_measure((orig_t + t_delta).max(1.0));
                        note.lane = (orig_l as i32 + l_delta).clamp(1, PAD_ZONE_MAX as i32) as u8;
                    }
                }
                app.set_status(format!("Moving {} notes", app.selected_notes.len()));
            } else {
                // Single note drag
                let Some(note) = app.chart.notes.get_mut(i) else { app.dragging_note = None; return; };
                let orig = app.drag_orig_note.as_ref();
                let part = app.drag_part.unwrap_or(DragPart::Body);
                let is_slide = matches!(note.note_type, super::types::NoteType::Slide);
                match part {
                    DragPart::Head => {
                        if let Some(o) = orig {
                            if is_slide {
                                let tail_m = o.time + o.slide_duration;
                                note.time = new_t;
                                note.slide_duration = (tail_m - new_t).max(0.0);
                            } else {
                                let tail_m = o.time + o.hold_duration;
                                note.time = new_t;
                                note.hold_duration = (tail_m - new_t).max(0.0);
                            }
                        }
                    }
                    DragPart::Tail => {
                        if let Some(o) = orig {
                            if is_slide {
                                note.slide_duration = (new_t - o.time).max(0.0);
                            } else {
                                note.hold_duration = (new_t - o.time).max(0.0);
                            }
                        }
                    }
                    DragPart::SlideDelayEnd => {
                        if let Some(o) = orig {
                            // Adjust the start-delay; clamp to [0, slide_duration].
                            let delay = (new_t - o.time).max(0.0).min(o.slide_duration);
                            note.slide_start_delay = delay;
                        }
                    }
                    DragPart::Body => {
                        if let Some(o) = orig {
                            note.time = new_t;
                            if is_slide {
                                note.slide_duration = o.slide_duration;
                            } else {
                                note.hold_duration = o.hold_duration;
                            }
                        } else {
                            note.time = new_t;
                        }
                    }
                }
                note.lane = new_lane;
                let dur = if is_slide { note.slide_duration } else { note.hold_duration };
                let t_val = note.time;
                app.set_status(format!("#{i}: t={:.2} dur={:.2}", t_val, dur));
            }
        }
    }
    if is_mouse_button_released(MouseButton::Left) {
        let was_dragging_note = app.dragging_note.is_some();
        let click_start = app.drag_start_pos;
        let moved = click_start.map(|s| pos.distance(s) >= drag_threshold).unwrap_or(false);

        if was_dragging_note {
            // Note drag finished — nothing extra to do
        } else if moved && app.box_start.is_some() {
            // Box selection: select all notes within drag rectangle
            let start = app.box_start.unwrap_or(pos);
            let x1 = start.x.min(pos.x); let x2 = start.x.max(pos.x);
            let y1 = start.y.min(pos.y); let y2 = start.y.max(pos.y);
            app.selected_notes.clear();
            for (i, note) in app.chart.notes.iter().enumerate() {
                let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, app.chart.bpm);
                if cx >= x1 && cx <= x2 && ny >= y1 && ny <= y2 { app.selected_notes.push(i); }
            }
            if !app.selected_notes.is_empty() { app.set_selected_note(Some(app.selected_notes[0])); }
            app.set_status(format!("Selected {} notes", app.selected_notes.len()));
        } else if !moved {
            // Simple click (no drag) → always place note via tool
            let dt = (judge_y - pos.y) / SCROLL_SPEED;
            let t = snap_secs_to_measure((now + dt).max(0.0), app.chart.bpm);
            let lx = pos.x - lanes_x;
            let lane = if lx >= 0.0 {
                let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
                if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
            } else { 1 };
            handle_tool_click(app, t, lane);
        }
        // Clean up
        app.press_note_candidate = None;
        app.dragging_note = None; app.drag_start_pos = None; app.drag_orig_note = None;
        app.box_start = None; app.box_end = None; app.box_anchor_t = None;
        app.recompute_each();
    }
    // ── Click while pasting: place notes ──
    if app.pasting && is_mouse_button_pressed(MouseButton::Left) && pos.x >= lanes_x {
        app.push_undo();
        let min_t = app.clipboard.iter().map(|n| n.time).fold(f32::MAX, f32::min);
        let dt = (judge_y - pos.y) / SCROLL_SPEED;
        let target = snap_secs_to_measure((now + dt).max(0.0), app.chart.bpm);
        let offset = target - min_t;
        let lx = pos.x - lanes_x;
        let tgt_lane = if lx >= 0.0 {
            let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
            if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
        } else { 1 };
        let anchor_lane = app.clipboard.first().map(|n| n.lane).unwrap_or(1);
        let lane_off = tgt_lane as i32 - anchor_lane as i32;
        for mut n in app.clipboard.clone() {
            n.time = snap_measure(n.time + offset);
            n.lane = (n.lane as i32 + lane_off).clamp(1, super::types::PAD_ZONE_MAX as i32) as u8;
            app.chart.notes.push(n);
        }
        app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        app.recompute_each();
        app.pasting = false;
        app.set_status(format!("Placed {} notes", app.clipboard.len()));
    }
}

/// Sidebar-tool placement dispatcher. Called once per "click empty space" on
/// the timeline. Implements the multi-step state machines for Hold and Star.
fn handle_tool_click(app: &mut AppState, t: f32, lane: u8) {
    use super::types::{NoteType, PlaceTool, PlacementState, Note};
    let zone = sanitize_note_zone(NoteType::Tap, lane);
    let touch = is_touch_zone(zone);
    match app.place_tool {
        PlaceTool::Tap => {
            app.push_undo();
            let nt = if touch { NoteType::Touch } else { NoteType::Tap };
            app.chart.notes.push(Note {
                time: t, lane, note_type: nt, hold_duration: 0.0, is_each: false,
                is_break: false, is_ex: false, is_star: false, is_tapless: false,
                star_is_break: false, star_is_ex: false,
                slide_points: vec![], slide_duration: 0.0, slide_start_delay: 0.0,
                slide_shape: None,
            });
            app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
            app.recompute_each();
            app.set_status(format!("Placed {} at m{:.3}",
                if matches!(nt, NoteType::Tap) { "Tap" } else { "Touch" }, t));
        }
        PlaceTool::Hold => {
            match app.placement {
                PlacementState::Idle => {
                    app.placement = PlacementState::HoldPending { anchor_t: t, lane };
                    app.set_status(format!("Hold #1 set at m{:.3}; click again to set the other end", t));
                }
                PlacementState::HoldPending { anchor_t, lane: lane0 } => {
                    // Head = earlier time, tail = later time (regardless of click order).
                    // Lane is locked to the first click.
                    let (head_t, tail_t) = if t >= anchor_t { (anchor_t, t) } else { (t, anchor_t) };
                    let dur = (tail_t - head_t).max(0.05);
                    app.push_undo();
                    app.chart.notes.push(Note {
                        time: head_t, lane: lane0, note_type: NoteType::Hold,
                        hold_duration: dur, is_each: false,
                        is_break: false, is_ex: false, is_star: false, is_tapless: false,
                        star_is_break: false, star_is_ex: false,
                        slide_points: vec![], slide_duration: 0.0, slide_start_delay: 0.0,
                        slide_shape: None,
                    });
                    app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
                    app.recompute_each();
                    app.placement = PlacementState::Idle;
                    app.set_status(format!("Placed Hold m{:.3} + {:.3}", head_t, dur));
                }
                _ => {
                    // Tool changed mid-flow; reset and start over.
                    app.placement = PlacementState::HoldPending { anchor_t: t, lane };
                }
            }
        }
        PlaceTool::Star => {
            match app.placement {
                PlacementState::Idle => {
                    app.placement = PlacementState::StarHead { head_t: t, lane };
                    app.set_status(format!("Star head at m{:.3}; click later to set delay end", t));
                }
                PlacementState::StarHead { head_t, lane: lane0 } => {
                    // Second click must be later in time than the head.
                    if t <= head_t {
                        app.set_status("Star: 第二次点击必须在星星头上方（更晚）".to_string());
                        return;
                    }
                    app.placement = PlacementState::StarDelay {
                        head_t, lane: lane0, delay_end_t: t,
                    };
                    app.set_status(format!("Star delay end at m{:.3}; click later to set tail", t));
                }
                PlacementState::StarDelay { head_t, lane: lane0, delay_end_t } => {
                    if t <= delay_end_t {
                        app.set_status("Star: 第三次点击必须在 delay handle 上方（更晚）".to_string());
                        return;
                    }
                    let slide_duration = t - head_t;
                    let slide_start_delay = (delay_end_t - head_t).max(0.0).min(slide_duration);
                    app.push_undo();
                    app.chart.notes.push(Note {
                        time: head_t, lane: lane0, note_type: NoteType::Slide,
                        hold_duration: 0.0, is_each: false,
                        is_break: false, is_ex: false, is_star: false, is_tapless: false,
                        star_is_break: false, star_is_ex: false,
                        slide_points: vec![], slide_duration, slide_start_delay,
                        slide_shape: None,
                    });
                    app.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
                    app.recompute_each();
                    app.placement = PlacementState::Idle;
                    app.set_status(format!("Placed Star m{:.3}, delay {:.3}, dur {:.3}",
                        head_t, slide_start_delay, slide_duration));
                }
                _ => {
                    app.placement = PlacementState::StarHead { head_t: t, lane };
                }
            }
        }
    }
}

/// Get screen position of a note. Returns (cx, head_y, tail_y, has_tail).
/// `has_tail` is true for Hold (hold_duration) and Slide (slide_duration) notes.
pub(crate) fn note_screen_pos(note: &super::types::Note, now: f32, track_x: f32, ruler_w: f32, lane_w: f32, judge_y: f32, bpm: f32) -> (f32, f32, f32, bool) {
    let zone = sanitize_note_zone(note.note_type, note.lane);
    let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
    let cx = track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
    let ns = note_secs(note, bpm);
    let dt = ns - now;
    let ny = judge_y - dt * SCROLL_SPEED;
    let (tail_t, has_tail) = match note.note_type {
        super::types::NoteType::Hold => (hold_tail_time(note, bpm), true),
        super::types::NoteType::Slide if note.slide_duration > 0.0 => {
            (ns + mdur_to_secs(note.slide_duration, bpm), true)
        }
        _ => (ns, false),
    };
    let tail_ny = if has_tail { judge_y - (tail_t - now) * SCROLL_SPEED } else { ny };
    (cx, ny, tail_ny, has_tail)
}