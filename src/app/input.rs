use std::cmp::{Ordering, PartialEq, PartialOrd};
use macroquad::prelude::*;
use macroquad::prelude::get_time;
use crate::app::types::NoteType::Slide;
use crate::app::types::zone::PadZone;
use super::chart;
use super::state::AppState;
use super::template;
use super::types::{Note, NoteType, PadGeom, PointerEvent, RecordInputId, RectF, UiButton, DragPart, SlideShape, BpmChange, MOUSE_POINTER_ID, SPEED_MAX, SPEED_MIN, SPEED_STEP, SCROLL_SPEED, LANE_COUNT, SCROLL_SPEED_FACTOR, SCROLL_INVERT, PAD_ZONE_MAX, is_touch_zone, sanitize_note_zone, hold_tail_time, note_secs, secs_to_measure, mdur_to_secs, SlidePoint};
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



pub fn handle_global_hotkeys(app: &mut AppState) {
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
    if is_key_pressed(KeyCode::P) && app.editing_slide_path.is_none() {
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
    // H 隐藏选中 note（支持多选），Shift+H 取消全部隐藏
    if is_key_pressed(KeyCode::H) {
        if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
            app.unhide_all_notes();
        } else {
            let targets: Vec<u64> = if !app.selected_note_ids.is_empty() {
                app.selected_note_ids.iter().copied().collect()
            } else if app.selected_note.is_some() {
                app.selected_note.and_then(|i| app.chart.notes.get(i)).map(|n| n.id).into_iter().collect()
            } else {
                vec![]
            };
            if !targets.is_empty() {
                for id in &targets { app.hidden_notes.insert(*id); }
                app.selected_note = None;
                app.selected_notes.clear();
                app.set_status(format!("Hidden {} note(s)", targets.len()));
            }
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
    if is_key_pressed(KeyCode::Minus) && app.editing_slide_path.is_none() {
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

    if is_key_pressed(KeyCode::S) && app.editing_slide_path.is_none() {
        if app.selected_notes.len() > 1 && !app.scaling_notes {
            // Enter scale mode
            app.push_undo();
            app.scaling_notes = true;
            let (_, my) = mouse_position();
            app.scale_anchor_y = my;
            app.scale_orig_notes = app.selected_notes.iter()
                .filter_map(|&i| app.chart.notes.get(i).map(|n| (i, n.time, n.lane)))
                .collect();
            app.set_status(format!("Scaling {} notes — mouse up=enlarge, down=shrink, Esc=cancel", app.selected_notes.len()));
        } else if !app.scaling_notes {
            match chart::save_recording_doc(app) {
                Ok(path) => app.set_status(format!("Saved recording: {}", path.display())),
                Err(err) => app.set_status(format!("Save failed: {err}")),
            }
        }
    }

    // Delete selected note(s) (skipped while editing a slide trajectory: there
    // Backspace pops the last slide point instead).
    // Block deletion of template instance notes (must edit in isolation mode).
    if (is_key_pressed(KeyCode::Delete)
        || is_key_pressed(KeyCode::D)
        || (is_key_pressed(KeyCode::Backspace) && app.editing_slide_path.is_none()))
    {
        if app.selected_notes.len() > 1 {
            // Multi-select delete
            app.push_undo();
            let mut to_remove: Vec<usize> = app.selected_notes.iter()
                .filter(|&&i| i < app.chart.notes.len() && !app.is_note_in_template_instance(i))
                .copied()
                .collect();
            to_remove.sort_unstable();
            to_remove.dedup();
            for &i in to_remove.iter().rev() {
                if i < app.chart.notes.len() {
                    app.chart.notes.remove(i);
                }
            }
            app.set_selected_note(None);
            app.selected_notes.clear();
            app.selected_note_ids.clear();
            app.set_status(format!("Deleted {} notes", to_remove.len()));
        } else if let Some(i) = app.selected_note {
            if i < app.chart.notes.len() && !app.is_note_in_template_instance(i) {
                app.push_undo();
                app.chart.notes.remove(i);
                app.set_selected_note(None);
                app.set_status(format!("Deleted note #{i}"));
            } else if app.is_note_in_template_instance(i) {
                app.set_status("Cannot delete template note here; enter isolation mode (Edit)".to_string());
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
                    app.editing_slide_idx = Some(0);
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
                let edit_idx = app.editing_slide_idx.unwrap_or(0);
                if let Some(sl) = n.slide.get_mut(edit_idx) {
                    if let Some(seg) = sl.segments.last_mut() {
                        if let Some(removed) = seg.points.pop() {
                            if seg.points.is_empty() {
                                sl.segments.pop(); // 删除整个空 segment
                                app.set_status(format!("Removed segment → zone {}", removed.zone));
                            } else {
                                seg.shape = super::slide_match::match_slide_shape(n.lane, &seg.points)
                                    .unwrap_or(super::types::SlideShape::Line);
                                app.set_status(format!("Removed zone {}", removed.zone));
                            }
                        }
                    }
                }
            }
        }
    }
    // ── Shape-key slide editing (only while editing_slide_path is active) ──
    // Press a shape key (Q, P, S, Z, V, W, etc.) then a lane number (1-8) to
    // replace the slide's trajectory with the predefined shape ending at that lane.
    if app.editing_slide_path.is_some() && !mod_down {
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let shape_key =
            if is_key_pressed(KeyCode::Q) && shift { Some(SlideShape::QQ) }
            else if is_key_pressed(KeyCode::Q) { Some(SlideShape::Q) }
            else if is_key_pressed(KeyCode::P) && shift { Some(SlideShape::PP) }
            else if is_key_pressed(KeyCode::P) { Some(SlideShape::P) }
            else if is_key_pressed(KeyCode::V) { Some(SlideShape::VShape) }
            else if is_key_pressed(KeyCode::W) { Some(SlideShape::Wifi) }
            else if is_key_pressed(KeyCode::Minus) { Some(SlideShape::Line) }
            else if is_key_pressed(KeyCode::Comma) { Some(SlideShape::Left) }
            else if is_key_pressed(KeyCode::Period) { Some(SlideShape::Right) }
            else if is_key_pressed(KeyCode::Semicolon) { Some(SlideShape::Caret) }
            else if is_key_pressed(KeyCode::S) { Some(SlideShape::S) }
            else if is_key_pressed(KeyCode::Z) { Some(SlideShape::Z) }
            else { None };
        if let Some(shape) = shape_key {
            app.pending_slide_shape = Some(shape);
            app.set_status(format!("Shape {:?} — press 1-8 for end lane", shape));
        }
        // Lane key completes the shape.
        let lane_key = if is_key_pressed(KeyCode::Key1) { Some(1u8) }
            else if is_key_pressed(KeyCode::Key2) { Some(2) }
            else if is_key_pressed(KeyCode::Key3) { Some(3) }
            else if is_key_pressed(KeyCode::Key4) { Some(4) }
            else if is_key_pressed(KeyCode::Key5) { Some(5) }
            else if is_key_pressed(KeyCode::Key6) { Some(6) }
            else if is_key_pressed(KeyCode::Key7) { Some(7) }
            else if is_key_pressed(KeyCode::Key8) { Some(8) }
            else { None };
        if let (Some(end_lane), Some(shape)) = (lane_key, app.pending_slide_shape) {
            // Shape key + lane: apply predefined shape to first slide's first segment
            if let Some(i) = app.editing_slide_path {
                // if let Some(n) = app.chart.notes.get(i) {
                //     if matches!(n.note_type, NoteType::Slide) && n.lane >= 1 && n.lane <= 8 {
                //         // Validate the slide shape before applying
                //         if let Err(err_msg) = super::types::validate_slide_shape(shape, n.lane, end_lane) {
                //             app.toasts.error(format!("非法的slide形状: {}", err_msg));
                //             app.set_status(format!("Slide shape error: {}", err_msg));
                //             app.pending_slide_shape = None;
                //             return;
                //         }
                //     }
                // }
                // Apply the shape if validation passed
                if let Some(n) = app.chart.notes.get_mut(i) {
                    if matches!(n.note_type, NoteType::Slide) && n.lane >= 1 && n.lane <= 8 {
                        let pattern = super::simai_io::shape_to_slide_pattern(shape);
                        let points = super::simai_io::simai_pattern_to_points(
                            n.lane - 1, end_lane - 1, pattern, None,
                        );
                        // Ensure at least one Slide with one segment exists
                        if n.slide.is_empty() {
                            n.slide.push(super::types::Slide {
                                segments: vec![super::types::SlideSegment { points, shape }],
                                slide_duration: 0.5, slide_start_delay: 0.0625,
                                slide_is_break: false,
                            });
                            app.editing_slide_idx = Some(0);
                        } else {
                            let edit_idx = app.editing_slide_idx.unwrap_or(0).min(n.slide.len().saturating_sub(1));
                            let sl = &mut n.slide[edit_idx];
                            sl.segments.push(super::types::SlideSegment { points, shape });
                        }
                        app.set_status(format!("Set shape {:?} → lane {}", shape, end_lane));
                    }
                }
            }
            app.pending_slide_shape = None;
        } else if let (Some(z), None) = (lane_key, app.pending_slide_shape) {
            // No shape pending: lane key appends a waypoint to first slide's first segment
            let mut msg: Option<String> = None;
            if let Some(i) = app.editing_slide_path {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    if matches!(n.note_type, NoteType::Slide) {
                        // Ensure at least one Slide with one segment exists
                        if n.slide.is_empty() {
                            n.slide.push(super::types::Slide {
                                segments: vec![super::types::SlideSegment {
                                    points: vec![], shape: super::types::SlideShape::Line,
                                }],
                                slide_duration: 0.5, slide_start_delay: 0.0625,
                                slide_is_break: false,
                            });
                            app.editing_slide_idx = Some(0);
                        }
                        let edit_idx = app.editing_slide_idx.unwrap_or(0).min(n.slide.len().saturating_sub(1));
                        let sl = &mut n.slide[edit_idx];
                        if sl.segments.is_empty() {
                            sl.segments.push(super::types::SlideSegment {
                                points: vec![], shape: super::types::SlideShape::Line,
                            });
                        }
                        let seg = &mut sl.segments[0];
                        let dur = sl.slide_duration;
                        let beat_offset = (seg.points.len() as f32 + 1.0)
                            * (dur.max(0.3) / (seg.points.len() as f32 + 2.0));
                        let last_zone = seg.points.last().copied()
                            .unwrap_or(SlidePoint::from(PadZone::from(n.lane)));
                        if z != last_zone.zone {
                            seg.points.push(SlidePoint::from(PadZone::from(z)));
                            seg.shape = super::slide_match::match_slide_shape(
                                n.lane, &seg.points,
                            ).unwrap_or(SlideShape::Line);
                            msg = Some(format!("Added zone {} (#{} points)", z, seg.points.len()));
                        }
                    }
                }
            }
            if let Some(m) = msg { app.set_status(m); }
        }
    } else {
        app.pending_slide_shape = None;
    }

    // B: toggle Break on selected note(s)
    // For Slide notes, toggles is_break (star head break).
    // Skip template instance notes (must edit in isolation mode).
    if is_key_pressed(KeyCode::B) && !mod_down {
        let indices = gather_selected(app);
        let filtered: Vec<usize> = indices.iter()
            .copied()
            .filter(|&i| !app.is_note_in_template_instance(i))
            .collect();
        if !filtered.is_empty() {
            app.push_undo();
            let mut count = 0;
            for &i in &filtered {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    n.is_break = !n.is_break;
                    count += 1;
                }
            }
            let on = app.chart.notes.get(*filtered.first().unwrap()).map(|n| n.is_break).unwrap_or(false);
            app.set_status(format!("Break {}: {} notes", if on { "ON" } else { "OFF" }, count));
        }
    }
    // X: toggle Ex on selected note(s)
    // Skip template instance notes.
    if is_key_pressed(KeyCode::X) && !mod_down {
        let indices = gather_selected(app);
        let filtered: Vec<usize> = indices.iter()
            .copied()
            .filter(|&i| !app.is_note_in_template_instance(i))
            .collect();
        if !filtered.is_empty() {
            app.push_undo();
            let mut count = 0;
            for &i in &filtered {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    n.is_ex = !n.is_ex;
                    count += 1;
                }
            }
            let on = app.chart.notes.get(*filtered.first().unwrap()).map(|n| n.is_ex).unwrap_or(false);
            app.set_status(format!("Ex {}: {} notes", if on { "ON" } else { "OFF" }, count));
        }
    }
    // N: toggle ExBreak on selected note(s) (both break + ex)
    // Skip template instance notes.
    if is_key_pressed(KeyCode::N) && !mod_down {
        let indices = gather_selected(app);
        let filtered: Vec<usize> = indices.iter()
            .copied()
            .filter(|&i| !app.is_note_in_template_instance(i))
            .collect();
        if !filtered.is_empty() {
            app.push_undo();
            let mut count = 0;
            for &i in &filtered {
                if let Some(n) = app.chart.notes.get_mut(i) {
                    let target = !(n.is_break && n.is_ex);
                    n.is_break = target;
                    n.is_ex = target;
                    count += 1;
                }
            }
            let on = app.chart.notes.get(*filtered.first().unwrap()).map(|n| n.is_break && n.is_ex).unwrap_or(false);
            app.set_status(format!("ExBreak {}: {} notes", if on { "ON" } else { "OFF" }, count));
        }
    }

    // Toggle grid snap for recording (only when not in grab mode and no multi-select)
    if is_key_pressed(KeyCode::G) && !app.grabbing_notes && app.selected_notes.len() <= 1 {
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

    // ── Template hotkeys ───────────────────────────────────────────
    // Escape priority: slide edit > isolation mode > placement reset.
    if is_key_pressed(KeyCode::Escape) {
        // If editing a slide path inside isolation, exit slide edit first.
        if app.editing_slide_path.is_some() {
            app.set_editing_slide_path(None);
            app.pending_slide_shape = None;
            app.set_status("Exited slide edit".to_string());
        }
        // If in isolation mode (and not editing slide anymore), exit isolation.
        else if template::is_in_isolation(app) {
            match template::exit_isolation(app) {
                Ok(()) => {}
                Err(e) => app.set_status(format!("Exit: {}", e)),
            }
        }
        // Otherwise, reset placement state.
        else {
            app.placement = super::types::PlacementState::Idle;
        }
    }
}

pub fn handle_lane_input(app: &mut AppState) {
    // Don't record taps while editing slide trajectory
    if app.editing_slide_path.is_some() { return; }

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
            app.start_record_hold_input(input_id, PadZone::from(lane));
        }
        if is_key_released(key) {
            let input_id = RecordInputId::Key(lane);
            app.finish_record_hold_input(input_id);
        }
    }
}

pub fn collect_pointer_events() -> Vec<PointerEvent> {
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



pub fn handle_touch_controls(
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
                        // If a shape key is pending, clicking an A-zone (1-8)
                        // completes the shape instead of appending a waypoint.
                        if let Some(shape) = app.pending_slide_shape {
                            if z >= 1 && z <= 8 {
                                if let Some(n) = app.chart.notes.get_mut(i) {
                                    if matches!(n.note_type, super::types::NoteType::Slide) && n.lane >= 1 && n.lane <= 8 {
                                        let pattern = super::simai_io::shape_to_slide_pattern(shape);
                                        let points = super::simai_io::simai_pattern_to_points(
                                            n.lane.saturating_sub(1), z.to_id().saturating_sub(1), pattern, None,
                                        );
                                        if n.slide.is_empty() {
                                            n.slide.push(super::types::Slide {
                                                segments: vec![super::types::SlideSegment { points, shape }],
                                                slide_duration: 0.5, slide_start_delay: 0.0625,
                                                slide_is_break: false,
                                            });
                                            app.editing_slide_idx = Some(0);
                                        } else {
                                            let edit_idx = app.editing_slide_idx.unwrap_or(0).min(n.slide.len().saturating_sub(1));
                                            let sl = &mut n.slide[edit_idx];
                                            sl.segments.push(super::types::SlideSegment { points, shape });
                                        }
                                        app.set_status(format!("Set shape {:?} → lane {}", shape, z));
                                    }
                                }
                                app.pending_slide_shape = None;
                                app.push_feedback(z, 0.18);
                                continue;
                            }
                        }

                        let mut handled = false;
                        let mut new_count = 0usize;
                        if let Some(n) = app.chart.notes.get_mut(i) {
                            if matches!(n.note_type, super::types::NoteType::Slide) {
                                // Ensure at least one Slide with one segment
                                if n.slide.is_empty() {
                                    n.slide.push(super::types::Slide {
                                        segments: vec![super::types::SlideSegment {
                                            points: vec![], shape: super::types::SlideShape::Line,
                                        }],
                                        slide_duration: 0.5, slide_start_delay: 0.0625,
                                        slide_is_break: false,
                                    });
                                    app.editing_slide_idx = Some(0);
                                }
                                let edit_idx = app.editing_slide_idx.unwrap_or(0).min(n.slide.len().saturating_sub(1));
                                let sl = &mut n.slide[edit_idx];
                                if sl.segments.is_empty() {
                                    sl.segments.push(super::types::SlideSegment {
                                        points: vec![], shape: super::types::SlideShape::Line,
                                    });
                                }
                                let seg = &mut sl.segments[0];
                                let dur = sl.slide_duration;
                                let beat_offset = (seg.points.len() as f32 + 1.0)
                                    * (dur.max(0.3) / (seg.points.len() as f32 + 2.0));
                                let last_zone = seg.points.last().copied()
                                    .unwrap_or(SlidePoint::from(PadZone::from(n.lane)));
                                if z != last_zone.zone {
                                    seg.points.push(SlidePoint::from(PadZone::from(z)));
                                    seg.shape = super::slide_match::match_slide_shape(
                                        n.lane, &seg.points,
                                    ).unwrap_or(super::types::SlideShape::Line);
                                    new_count = seg.points.len();
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

/// Snap a measure value to the nearest visible grid position.
/// One measure = one bar; GRID_DIVISION subdivisions per bar → step = 1/GRID_DIVISION.
fn snap_grid(m: f32) -> f32 {
    use super::types::GRID_DIVISION;
    let step = 1.0 / GRID_DIVISION as f32;
    (m / step).round() * step
}

/// Distance from point (px, py) to line segment (x0,y0)-(x1,y1).
fn point_to_segment_dist(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.001 {
        return ((px - x0) * (px - x0) + (py - y0) * (py - y0)).sqrt();
    }
    let t = ((px - x0) * dx + (py - y0) * dy).clamp(0.0, len_sq) / len_sq;
    let proj_x = x0 + t * dx;
    let proj_y = y0 + t * dy;
    ((px - proj_x) * (px - proj_x) + (py - proj_y) * (py - proj_y)).sqrt()
}

/// Wrap lane within its zone group (A1-A8, D1-D8, E1-E8, B1-B8, C).
fn wrap_lane(lane: u8, delta: i32) -> u8 {
    if lane >= 1 && lane <= 8 {
        // A-zone: wrap 1-8
        ((lane as i32 - 1 + delta).rem_euclid(8) + 1) as u8
    } else if lane >= 10 && lane <= 17 {
        // D-zone: wrap 10-17
        ((lane as i32 - 10 + delta).rem_euclid(8) + 10) as u8
    } else if lane >= 18 && lane <= 25 {
        // E-zone: wrap 18-25
        ((lane as i32 - 18 + delta).rem_euclid(8) + 18) as u8
    } else if lane >= 26 && lane <= 33 {
        // B-zone: wrap 26-33
        ((lane as i32 - 26 + delta).rem_euclid(8) + 26) as u8
    } else {
        lane // C or unknown: don't wrap
    }
}

/// Snap a screen-seconds value to the nearest visible grid position.
/// Returns the snapped value in **measures**.
fn snap_secs_to_measure(secs: f32, bpms: &[BpmChange]) -> f32 {
    snap_grid(secs_to_measure(secs, bpms))
}

pub fn handle_timeline_editing(app: &mut AppState, timeline_rect: Option<RectF>) {
    let Some(tl) = timeline_rect else { return };
    let (mx, my) = mouse_position();
    let pos = vec2(mx, my);

    // Block clicks on the egui toolbar area.
    if my < app.egui_toolbar_bottom && is_mouse_button_pressed(MouseButton::Left) {
        return;
    }

    let scroll_speed = SCROLL_SPEED * app.timeline_zoom;

    // Cmd+scroll: zoom
    let cmd_down = is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper)
        || is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
    let (_, wy) = mouse_wheel();
    if cmd_down && wy != 0.0 {
        let factor = if wy > 0.0 { 1.15 } else { 1.0 / 1.15 };
        app.timeline_zoom = (app.timeline_zoom * factor).clamp(0.1, 10.0);
    }

    // Scroll / middle-drag: move actual playback progress
    let mut shift = 0.0_f32;
    let dir = if SCROLL_INVERT { 1.0 } else { -1.0 };
    if wy != 0.0 && !cmd_down { shift = dir * wy * SCROLL_SPEED_FACTOR; }
    if is_mouse_button_down(MouseButton::Middle) { shift = -mouse_delta_position().y * 0.02; }
    // Edge auto-scroll while box-selecting: cursor above/below the timeline
    // pans the view so the user can extend the selection beyond the viewport.
    if app.box_anchor_t.is_some() && is_mouse_button_down(MouseButton::Left) {
        let above = (tl.y - pos.y).max(0.0);
        let below = (pos.y - (tl.y + tl.h)).max(0.0);
        // Speed scales with how far past the edge the cursor is (in chart time).
        // Frame-time-multiplied so it's roughly framerate-independent.
        let dt_frame = macroquad::prelude::get_frame_time().min(0.05);
        let edge_speed = (above - below) / scroll_speed * 4.0; // chart-seconds per second
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
    let progress_bar_h = 20.0_f32;
    let track_h = tl.h - 80.0 - progress_bar_h;
    let ruler_w = 64.0;
    let lanes_w = track_w - ruler_w;
    let lane_w = lanes_w / LANE_COUNT as f32;
    let judge_y = track_y + track_h - 38.0;
    let lanes_x = track_x + ruler_w;

    // ── Progress bar click/drag: seek to position ──
    {
        let bar_x = track_x;
        let bar_y = track_y + track_h + 4.0;
        let bar_w = track_w;
        let bar_h = progress_bar_h - 4.0;
        // Start drag when clicking inside the bar
        if is_mouse_button_pressed(MouseButton::Left)
            && pos.x >= bar_x && pos.x <= bar_x + bar_w
            && pos.y >= bar_y && pos.y <= bar_y + bar_h
        {
            app.dragging_progress_bar = true;
        }
        // Release drag
        if !is_mouse_button_down(MouseButton::Left) {
            app.dragging_progress_bar = false;
        }
        // While dragging, seek based on mouse x (even if cursor leaves the bar)
        if app.dragging_progress_bar && is_mouse_button_down(MouseButton::Left) {
            let bpms = &app.chart.bpms;
            let total_dur = if let Some(ref wav) = app.audio_wav_pcm {
                let audio_dur = wav.samples.len() as f32 / (wav.sample_rate as f32 * wav.channels as f32).max(1.0);
                audio_dur.max(1.0)
            } else {
                let last_note_end = app.chart.notes.iter().map(|n| {
                    let ns = note_secs(n, bpms);
                    match n.note_type {
                        super::types::NoteType::Hold => hold_tail_time(n, bpms),
                        super::types::NoteType::Slide => {
                            let max_dur = n.slide.iter().map(|s| s.slide_duration).fold(0.0_f32, f32::max);
                            ns + mdur_to_secs(max_dur, n.time, bpms)
                        }
                        _ => ns,
                    }
                }).fold(0.0_f32, f32::max);
                last_note_end.max(1.0)
            };
            let frac = ((pos.x - bar_x) / bar_w).clamp(0.0, 1.0);
            let new_t = frac * total_dur;
            if matches!(app.mode, super::types::Mode::Playing) {
                app.mode_song_offset = new_t;
                app.mode_wall_anchor = macroquad::prelude::get_time();
                app.seek_audio_to(new_t);
            } else {
                app.mode_song_offset = new_t;
                app.timeline_view_time = new_t;
            }
            return;
        }
    }

    // Re-derive the box-select start screen y from its chart-time anchor each
    // frame, so scrolling pans the entire selection rectangle along with the
    // notes (the start sticks to its original chart time, not its screen y).
    if let (Some(anchor_t), Some(start)) = (app.box_anchor_t, app.box_start.as_mut()) {
        start.y = judge_y - (anchor_t - now) * scroll_speed;
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
    // if pos.x >= track_x && pos.x <= lanes_x && is_mouse_button_down(MouseButton::Left) {
    //     let dt = (judge_y - pos.y) / scroll_speed;
    //     let new_t = (now + dt).max(0.0);
    //     if matches!(app.mode, super::types::Mode::Playing) {
    //         app.mode_song_offset = new_t; app.mode_wall_anchor = get_time(); app.seek_audio_to(new_t);
    //     } else { app.timeline_view_time = new_t; }
    //     app.set_selected_note(None); app.dragging_note = None; app.drag_part = None;
    //     return;
    // }

    // ── Shift+Right-click delete template instance ──
    let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
    if is_mouse_button_pressed(MouseButton::Right) && pos.x >= lanes_x && shift_down && !template::is_in_isolation(app) {
        for (inst_idx, inst) in app.chart.template_instances.iter().enumerate() {
            let (i_start, i_end) = template::instance_time_range(app, inst);
            let start_secs = super::types::measure_to_secs(i_start, &app.chart.bpms);
            let end_secs = super::types::measure_to_secs(i_end, &app.chart.bpms);
            let y_top = judge_y - (start_secs - now) * scroll_speed;
            let y_bot = judge_y - (end_secs - now) * scroll_speed;
            let block_x = track_x + ruler_w;
            let block_w = lanes_w;
            let block_y = y_top.min(y_bot);
            let block_h = (y_top - y_bot).abs();
            if pos.x >= block_x && pos.x <= block_x + block_w
                && pos.y >= block_y && pos.y <= block_y + block_h
            {
                let name = app.chart.templates.iter()
                    .find(|t| t.id == inst.template_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                app.push_undo();
                app.chart.template_instances.remove(inst_idx);
                app.set_status(format!("Deleted instance of '{}'", name));
                return;
            }
        }
    }

    // ── Right-click delete ──
    if is_mouse_button_pressed(MouseButton::Right) && pos.x >= lanes_x {
        let mut best: Option<usize> = None; let mut best_d = 30.0;
        for (i, note) in app.chart.notes.iter().enumerate() {
            if app.hidden_notes.contains(&note.id) { continue; }
            let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, &app.chart.bpms, scroll_speed);
            let d = pos.distance(vec2(cx, ny));
            if d < best_d { best = Some(i); best_d = d; }
        }
        if let Some(i) = best {
            app.push_undo();
            app.chart.notes.remove(i);
            app.recompute_each();
            app.set_selected_note(None); app.dragging_note = None; app.drag_slide_idx = None;
            app.set_status(format!("Deleted note #{i}"));
        }
        return;
    }

    // ── Middle-click on slide path on timeline: enter trajectory editing mode ──
    if is_mouse_button_pressed(MouseButton::Middle) && pos.x >= lanes_x {
        let mut best: Option<usize> = None; let mut best_d = 20.0_f32; // max click distance
        for (i, note) in app.chart.notes.iter().enumerate() {
            if app.hidden_notes.contains(&note.id) { continue; }
            if !matches!(note.note_type, NoteType::Slide) { continue; }
            let zone = sanitize_note_zone(note.note_type, note.lane);
            let ns = note_secs(note, &app.chart.bpms);
            let dt = ns - now;
            let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
            let cx = track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
            let ny = judge_y - dt * scroll_speed;

            for (si, sl) in note.slide.iter().enumerate() {
                if sl.slide_duration <= 0.0 { continue; }
                let slide_dur_s = mdur_to_secs(sl.slide_duration, note.time, &app.chart.bpms).max(0.3);
                let delay_s = mdur_to_secs(sl.slide_start_delay, note.time, &app.chart.bpms);
                let delay_y = judge_y - (ns + delay_s - now) * scroll_speed;
                let tail_t = ns + slide_dur_s;
                let tail_y = judge_y - (tail_t - now) * scroll_speed;
                let tail_zone = sl.segments.last()
                    .and_then(|seg| seg.points.last())
                    .map(|sp| sp.zone)
                    .unwrap_or(PadZone::from(note.lane));
                let tail_li = (tail_zone.to_id().saturating_sub(1) as usize).min(LANE_COUNT - 2);
                let tail_cx = track_x + ruler_w + lane_w * tail_li as f32 + lane_w * 0.5;

                // Build waypoints: head → delay → segments → tail
                let mut waypoints: Vec<(f32, f32)> = Vec::new();
                waypoints.push((cx, ny));
                if delay_s > 0.0 {
                    waypoints.push((cx, delay_y));
                }
                let zone_to_cx = |z: PadZone| -> f32 {
                    let li = (z.to_id().saturating_sub(1) as usize).min(LANE_COUNT - 2);
                    track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5
                };
                let mut a_points: Vec<&super::types::SlidePoint> = Vec::new();
                for seg in &sl.segments {
                    for sp in &seg.points {
                        if sp.zone >= 1 && sp.zone <= 8 {
                            a_points.push(sp);
                        }
                    }
                }
                let n_pts = a_points.len();
                if n_pts > 0 {
                    for (pi, sp) in a_points.iter().enumerate() {
                        let frac = (pi + 1) as f32 / n_pts as f32;
                        let wy = delay_y + (tail_y - delay_y) * frac;
                        let wx = zone_to_cx(sp.zone);
                        waypoints.push((wx, wy));
                    }
                }
                waypoints.push((tail_cx, tail_y));

                // Check distance to each line segment
                for w in 0..waypoints.len() - 1 {
                    let (x0, y0) = waypoints[w];
                    let (x1, y1) = waypoints[w + 1];
                    let d = point_to_segment_dist(pos.x, pos.y, x0, y0, x1, y1);
                    if d < best_d {
                        best = Some(i);
                        best_d = d;
                        app.drag_slide_idx = Some(si);
                    }
                }
            }
        }
        if let Some(i) = best {
            app.set_selected_note(Some(i));
            app.set_editing_slide_path(Some(i));
            app.set_status(format!("Trajectory edit: click pad zones to modify (Backspace=undo, Esc=exit). #{i}"));
        }
    }

    // ── Mouse press: record candidate, do NOT select yet ──
    let drag_threshold = 8.0;

    if is_mouse_button_pressed(MouseButton::Left) && pos.x >= lanes_x {
        // ── Template block interaction (Shift+click/drag only, main chart) ──
        if !template::is_in_isolation(app) && shift_down {
            let mut hit_inst: Option<(usize, f32, String)> = None;
            for (inst_idx, inst) in app.chart.template_instances.iter().enumerate() {
                let (i_start, i_end) = template::instance_time_range(app, inst);
                let start_secs = super::types::measure_to_secs(i_start, &app.chart.bpms);
                let end_secs = super::types::measure_to_secs(i_end, &app.chart.bpms);
                let y_top = judge_y - (start_secs - now) * scroll_speed;
                let y_bot = judge_y - (end_secs - now) * scroll_speed;
                let block_x = track_x + ruler_w;
                let block_w = lanes_w;
                let block_y = y_top.min(y_bot);
                let block_h = (y_top - y_bot).abs();

                if pos.x >= block_x && pos.x <= block_x + block_w
                    && pos.y >= block_y && pos.y <= block_y + block_h
                {
                    let name = app.chart.templates.iter()
                        .find(|t| t.id == inst.template_id)
                        .map(|t| t.name.clone())
                        .unwrap_or_default();
                    hit_inst = Some((inst_idx, inst.anchor_time, name));
                    break;
                }
            }

            if let Some((inst_idx, anchor_time, tpl_name)) = hit_inst {
                app.set_selected_note(None);
                app.selected_notes.clear();
                app.drag_start_pos = Some(pos);
                app.press_note_candidate = Some(usize::MAX);
                app.drag_start_time = anchor_time;
                app.drag_cursor_anchor_t = (now + (judge_y - pos.y) / scroll_speed).max(0.0);
                app.set_status(format!("Template '{}': drag to move, release to edit", tpl_name));
                return;
            }
        }

        // ── Template drag handling (no Shift) — clear sentinel if set ──
        if app.press_note_candidate == Some(usize::MAX) && !shift_down {
            app.press_note_candidate = None;
            app.drag_start_pos = None;
        }

        // Hit-test: find nearest note (potential drag target)
        let mut best: Option<usize> = None; let mut best_d = 30.0;
        let mut best_part = DragPart::Body;
        let mut best_slide_idx: Option<usize> = None;
        for (i, note) in app.chart.notes.iter().enumerate() {
            if app.hidden_notes.contains(&note.id) { continue; }
            let (cx, ny, tail_ny, has_tail) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, &app.chart.bpms, scroll_speed);
            let d = pos.distance(vec2(cx, ny));
            if matches!(note.note_type, super::types::NoteType::Slide) && !note.slide.is_empty() {
                let ns = note_secs(note, &app.chart.bpms);
                for (si, sl) in note.slide.iter().enumerate() {
                    if sl.slide_duration <= 0.0 { continue; }
                    let delay_secs = ns + mdur_to_secs(sl.slide_start_delay, note.time, &app.chart.bpms);
                    let delay_y = judge_y - (delay_secs - now) * scroll_speed;
                    let tail_secs = ns + mdur_to_secs(sl.slide_duration, note.time, &app.chart.bpms);
                    let tail_y = judge_y - (tail_secs - now) * scroll_speed;
                    let tail_x = slide_tail_cx_for(note, si, track_x, ruler_w, lane_w);
                    let delay_d = pos.distance(vec2(cx, delay_y));
                    let tail_d = pos.distance(vec2(tail_x, tail_y));
                    if delay_d < best_d && delay_d < d && delay_d < tail_d {
                        best = Some(i); best_d = delay_d; best_part = DragPart::SlideDelayEnd; best_slide_idx = Some(si);
                        continue;
                    }
                    if tail_d < best_d && tail_d < d {
                        best = Some(i); best_d = tail_d; best_part = DragPart::Tail; best_slide_idx = Some(si);
                        continue;
                    }
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
        let cursor_t_at_click = (now + (judge_y - pos.y) / scroll_speed).max(0.0);
        app.drag_cursor_anchor_t = cursor_t_at_click;

        if let Some(i) = best {
            // Double-click on a Slide head → append a new sub-slide to same star note
            let click_now = get_time();
            let is_dbl = app.last_click_note == Some(i)
                && (click_now - app.last_click_time) < 0.4
                && matches!(best_part, DragPart::Body | DragPart::Head);
            app.last_click_time = click_now;
            app.last_click_note = Some(i);
            if is_dbl && matches!(app.chart.notes.get(i).map(|n| n.note_type), Some(NoteType::Slide)) {
                let src = app.chart.notes[i].clone();
                app.push_undo();
                // Mark the original star as double-star
                app.chart.notes[i].is_star = true;
                // 继承上一个 slide 的终点 lane
                let last_end_lane = src.slide.last()
                    .and_then(|s| s.segments.last())
                    .and_then(|seg| seg.points.last())
                    .map(|sp| sp.zone)
                    .unwrap_or(PadZone::from(src.lane));
                let default_dur = src.slide.first().map(|s| s.slide_duration).unwrap_or(0.5);
                let default_delay = src.slide.first().map(|s| s.slide_start_delay).unwrap_or(0.0625);
                let new_slide = super::types::Slide {
                        segments: vec![super::types::SlideSegment {
                            points: vec![SlidePoint { zone: last_end_lane, beat_offset: 0.0 }],
                            shape: SlideShape::Line,
                        }],
                        slide_duration: default_dur,
                        slide_start_delay: default_delay,
                        slide_is_break: false,
                };
                app.chart.notes[i].slide.push(new_slide);
                let new_si = app.chart.notes[i].slide.len().saturating_sub(1);
                app.chart.notes[i].is_tapless = false;
                app.set_selected_note(Some(i));
                app.set_editing_slide_path(Some(i));
                app.editing_slide_idx = Some(new_si);
                app.set_status(format!("Appended slide #{new_si} on star #{i} — click pad zones to build path"));
                return;
            }
            // Record candidate for potential drag — do NOT select yet
            app.press_note_candidate = Some(i);
            app.drag_start_pos = Some(pos);
            let n = &app.chart.notes[i];
            let bpms = &app.chart.bpms;
            app.drag_start_time = match best_part {
                DragPart::Tail => match n.note_type {
                    super::types::NoteType::Hold => hold_tail_time(n, bpms),
                    super::types::NoteType::Slide => {
                        let si = app.drag_slide_idx.unwrap_or(0);
                        let d = n.slide.get(si).map(|s| s.slide_duration).unwrap_or(0.0);
                        note_secs(n, bpms) + mdur_to_secs(d, n.time, bpms)
                    }
                    _ => note_secs(n, bpms),
                },
                DragPart::SlideDelayEnd => {
                    let si = app.drag_slide_idx.unwrap_or(0);
                    let d = n.slide.get(si).map(|s| s.slide_start_delay).unwrap_or(0.0);
                    note_secs(n, bpms) + mdur_to_secs(d, n.time, bpms)
                }
                _ => note_secs(n, bpms),
            };
            app.drag_part = Some(best_part);
            app.drag_shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            app.drag_slide_idx = best_slide_idx;
            app.drag_orig_note = Some(app.chart.notes[i].clone());
        } else {
            // No note under cursor — prepare for potential box-select
            app.press_note_candidate = None;
            app.drag_start_pos = Some(pos);
            app.box_anchor_t = Some(cursor_t_at_click);
            app.drag_part = None;
            app.drag_slide_idx = None;
            app.drag_orig_note = None;
        }
    }

    // ── Template drag/release handling (runs every frame while active) ──
    if app.press_note_candidate == Some(usize::MAX) && shift_down {
        if is_mouse_button_down(MouseButton::Left) {
            let moved = app.drag_start_pos.map(|s| pos.distance(s) >= drag_threshold).unwrap_or(false);
            if moved {
                let cursor_secs = (now + (judge_y - pos.y) / scroll_speed).max(0.0);
                let cursor_m = secs_to_measure(cursor_secs, &app.chart.bpms);
                let anchor_m = secs_to_measure(app.drag_cursor_anchor_t, &app.chart.bpms);
                let start_m = secs_to_measure(app.drag_start_time, &app.chart.bpms);
                let new_time = snap_grid((cursor_m + (start_m - anchor_m)).max(1.0));
                let anchor = app.drag_start_time;
                if let Some(inst_idx) = app.chart.template_instances.iter()
                    .position(|i| (i.anchor_time - anchor).abs() < 0.01)
                {
                    template::move_instance(app, inst_idx, new_time);
                    app.drag_start_time = new_time;
                    app.drag_cursor_anchor_t = cursor_secs;
                }
            }
        }
        if is_mouse_button_released(MouseButton::Left) {
            let barely_moved = app.drag_start_pos.map(|s| pos.distance(s) < drag_threshold).unwrap_or(true);
            if barely_moved {
                let inst_idx = app.drag_cursor_anchor_t as usize;
                if inst_idx < app.chart.template_instances.len() {
                    match template::enter_instance_isolation(app, inst_idx) {
                        Ok(()) => {}
                        Err(e) => app.set_status(format!("Enter: {}", e)),
                    }
                }
            }
            app.press_note_candidate = None;
            app.drag_start_pos = None;
            return;
        }
        return;
    }
    // Clear template sentinel if shift was released.
    if app.press_note_candidate == Some(usize::MAX) {
        app.press_note_candidate = None;
        app.drag_start_pos = None;
    }

    // ── Enter grab mode (G key with multi-select) ──
    if is_key_pressed(KeyCode::G) && !app.grabbing_notes && !app.scaling_notes
        && app.selected_notes.len() > 1 && app.editing_slide_path.is_none()
    {
        app.push_undo();
        app.grabbing_notes = true;
        let cursor_m = snap_secs_to_measure(now.max(0.0), &app.chart.bpms);
        app.grab_anchor_time = cursor_m;
        app.grab_orig_notes = app.selected_notes.iter()
            .filter_map(|&i| app.chart.notes.get(i).map(|n| (i, n.time, n.lane)))
            .collect();
        app.set_status(format!("Grabbing {} notes — click to place, Esc=cancel", app.selected_notes.len()));
    }

    // ── Note scaling mode (S key with multi-select) ──
    if app.scaling_notes {
        if is_key_pressed(KeyCode::Escape) {
            // Cancel: restore original positions
            for &(i, orig_t, orig_l) in &app.scale_orig_notes {
                if let Some(note) = app.chart.notes.get_mut(i) {
                    note.time = orig_t;
                    note.lane = orig_l;
                }
            }
            app.scaling_notes = false;
            app.scale_orig_notes.clear();
            app.drag_start_pos = None;
            app.press_note_candidate = None;
            app.set_status("Scale cancelled".to_string());
            return;
        }
        if is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Enter) {
            // Confirm scaling
            app.scaling_notes = false;
            app.scale_orig_notes.clear();
            app.drag_start_pos = None;
            app.press_note_candidate = None;
            app.recompute_each();
            app.set_status("Scale applied".to_string());
            return;
        }
        // Compute scale factor from mouse Y displacement
        let (_, my) = mouse_position();
        let dy = app.scale_anchor_y - my; // positive = mouse moved up = enlarge
        let scale_factor = (1.0 + dy / 200.0).max(0.1);
        let bpms = &app.chart.bpms;
        // Compute center of selected notes group
        let times: Vec<f32> = app.scale_orig_notes.iter().map(|&(_, t, _)| t).collect();
        let center = if times.is_empty() { 1.0 } else {
            (times.iter().cloned().fold(f32::INFINITY, f32::min)
                + times.iter().cloned().fold(f32::NEG_INFINITY, f32::max)) / 2.0
        };
        // Apply scale: center stays fixed, other notes move proportionally
        for &(i, orig_t, orig_l) in &app.scale_orig_notes {
            if let Some(note) = app.chart.notes.get_mut(i) {
                let delta = orig_t - center;
                note.time = snap_grid((center + delta * scale_factor).max(1.0));
                // Lane scaling: scale lane distance from center lane
                let lanes: Vec<u8> = app.scale_orig_notes.iter().map(|&(_, _, l)| l).collect();
                let lane_center = lanes.iter().map(|&l| l as f32).sum::<f32>() / lanes.len().max(1) as f32;
                let lane_delta = orig_l as f32 - lane_center;
                let new_lane = (lane_center + lane_delta * scale_factor).round().clamp(1.0, 8.0) as u8;
                note.lane = new_lane;
            }
        }
        app.recompute_each();
        app.set_status(format!("Scaling ×{:.2} (mouse up=enlarge, down=shrink, click=confirm, Esc=cancel)", scale_factor));
        return;
    }

    // ── Note grab mode (G key with multi-select) ──
    if app.grabbing_notes {
        if is_key_pressed(KeyCode::Escape) {
            // Cancel: restore original positions
            for &(i, orig_t, orig_l) in &app.grab_orig_notes {
                if let Some(note) = app.chart.notes.get_mut(i) {
                    note.time = orig_t;
                    note.lane = orig_l;
                }
            }
            app.grabbing_notes = false;
            app.grab_orig_notes.clear();
            app.drag_start_pos = None;
            app.press_note_candidate = None;
            app.set_status("Grab cancelled".to_string());
            return;
        }
        if is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Enter) {
            // Confirm grab
            app.grabbing_notes = false;
            app.grab_orig_notes.clear();
            app.drag_start_pos = None;
            app.press_note_candidate = None;
            app.recompute_each();
            app.set_status("Grab applied".to_string());
            return;
        }
        // Arrow keys: Left/Right = lane (with wrapping), Up/Down = time
        let lane_step = 1;
        let time_step = snap_grid(1.0 / 4.0); // 1/4 measure per keypress
        if is_key_pressed(KeyCode::Left) {
            for &(i, _, _) in &app.grab_orig_notes {
                if let Some(note) = app.chart.notes.get_mut(i) {
                    note.lane = wrap_lane(note.lane, -lane_step);
                }
            }
        }
        if is_key_pressed(KeyCode::Right) {
            for &(i, _, _) in &app.grab_orig_notes {
                if let Some(note) = app.chart.notes.get_mut(i) {
                    note.lane = wrap_lane(note.lane, lane_step);
                }
            }
        }
        if is_key_pressed(KeyCode::Up) {
            for &(i, _, _) in &app.grab_orig_notes {
                if let Some(note) = app.chart.notes.get_mut(i) {
                    note.time = snap_grid((note.time + time_step).max(1.0));
                }
            }
        }
        if is_key_pressed(KeyCode::Down) {
            for &(i, _, _) in &app.grab_orig_notes {
                if let Some(note) = app.chart.notes.get_mut(i) {
                    note.time = snap_grid((note.time - time_step).max(1.0));
                }
            }
        }
        app.recompute_each();
        app.set_status(format!("Grabbing {} notes (arrows=move, click/Enter=confirm, Esc=cancel)", app.selected_notes.len()));
        return;
    }

    // ── Dragging: note move or box selection ──
    if is_mouse_button_down(MouseButton::Left) {
        let moved = app.drag_start_pos.map(|s| pos.distance(s) >= drag_threshold).unwrap_or(false);
        if !moved { /* waiting for threshold */ }
        // Promote candidate -> dragging (on first frame beyond threshold)
        // Block dragging of template instance notes.
        else if app.press_note_candidate.is_some() && app.dragging_note.is_none() {
            let i = app.press_note_candidate.take().unwrap();
            if i < app.chart.notes.len() && !app.is_note_in_template_instance(i) {
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
            } else if app.is_note_in_template_instance(i) {
                // Select the note but don't allow dragging.
                app.set_selected_note(Some(i));
                app.set_status("Template note: enter isolation mode to edit".to_string());
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
            // Compute cursor position directly in measures, apply the
            // offset (also in measures), then snap to the visible grid.
            let cursor_secs = (now + (judge_y - pos.y) / scroll_speed).max(0.0);
            let cursor_m = secs_to_measure(cursor_secs, &app.chart.bpms);
            let anchor_m = secs_to_measure(app.drag_cursor_anchor_t, &app.chart.bpms);
            let start_m = secs_to_measure(app.drag_start_time, &app.chart.bpms);
            let raw_m = (cursor_m + (start_m - anchor_m)).max(1.0);
            let new_t = snap_grid(raw_m);
            let lx = pos.x - lanes_x;
            // Raw lane index (0-based A-zone), NOT clamped — used for delta in multi-select
            let raw_lane = (lx / lane_w) as i32;
            // Clamped lane for single-note and display
            let new_lane = if lx >= 0.0 {
                let l = raw_lane.clamp(0, LANE_COUNT as i32 - 1) as u8;
                if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
            } else { 1 };
            // Multi-select: move all selected notes by delta
            if app.selected_notes.len() > 1 && app.selected_notes.contains(&i) {
                let orig = app.drag_orig_note.as_ref();
                let t_delta = new_t - orig.map(|o| o.time).unwrap_or(new_t);
                // Use raw (unclamped) lane for delta so wrapping works outside bounds
                let orig_raw = orig.map(|o| (o.lane as i32 - 1)).unwrap_or(raw_lane);
                let l_delta = raw_lane - orig_raw;
                for &(si, orig_t, orig_l) in &app.drag_multi_orig {
                    if let Some(note) = app.chart.notes.get_mut(si) {
                        note.time = snap_grid((orig_t + t_delta).max(1.0));
                        note.lane = wrap_lane(orig_l, l_delta);
                        // Rotate slide points for slide heads
                        if matches!(note.note_type, super::types::NoteType::Slide) && l_delta != 0 {
                            if let Some(orig_note) = app.drag_orig_note.as_ref() {
                                for (sli, sl) in note.slide.iter_mut().enumerate() {
                                    if let Some(old_sl) = orig_note.slide.get(sli) {
                                        sl.segments = old_sl.segments.clone();
                                        for seg in &mut sl.segments {
                                            for pt in &mut seg.points {
                                                let id = pt.zone.to_id() as i32;
                                                let new_id = match id {
                                                    1..=8  => (id - 1 + l_delta).rem_euclid(8) + 1,
                                                    9..=16 => (id - 9 + l_delta).rem_euclid(8) + 9,
                                                    17 => 17,
                                                    18..=25 => (id - 18 + l_delta).rem_euclid(8) + 18,
                                                    26..=33 => (id - 26 + l_delta).rem_euclid(8) + 26,
                                                    _ => id,
                                                } as u8;
                                                pt.zone = PadZone::from(new_id);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                app.set_status(format!("Moving {} notes", app.selected_notes.len()));
            } else {
                // Single note drag
                let Some(note) = app.chart.notes.get_mut(i) else { app.dragging_note = None; return; };
                let prev_time = note.time;
                let prev_lane = note.lane;
                let orig = app.drag_orig_note.as_ref();
                let part = app.drag_part.unwrap_or(DragPart::Body);
                let is_slide = matches!(note.note_type, super::types::NoteType::Slide);
                match part {
                    DragPart::Head => {
                        if let Some(o) = orig {
                            if is_slide {
                                // Restore original slide points before applying rotation
                                // to prevent cumulative rotation each frame
                                for (si, sl) in note.slide.iter_mut().enumerate() {
                                    if let Some(old_sl) = o.slide.get(si) {
                                        sl.segments = old_sl.segments.clone();
                                    }
                                }
                                // Use raw (unclamped) lane for delta so wrapping works outside bounds
                                let lane_delta = raw_lane - (o.lane as i32 - 1);
                                if lane_delta != 0 {
                                    for sl in &mut note.slide {
                                        for seg in &mut sl.segments {
                                            for pt in &mut seg.points {
                                                let id = pt.zone.to_id() as i32;
                                                let new_id = match id {
                                                    1..=8  => (id - 1 + lane_delta).rem_euclid(8) + 1,
                                                    9..=16 => (id - 9 + lane_delta).rem_euclid(8) + 9,
                                                    17 => 17,
                                                    18..=25 => (id - 18 + lane_delta).rem_euclid(8) + 18,
                                                    26..=33 => (id - 26 + lane_delta).rem_euclid(8) + 26,
                                                    _ => id,
                                                } as u8;
                                                pt.zone = PadZone::from(new_id);
                                            }
                                        }
                                    }
                                }
                            }
                            if !app.drag_shift {
                                // Shift+Head: only move head time, keep all durations/delays
                                note.time = new_t;
                            } else {
                                note.time = new_t;
                                // Keep duration fixed (head+tail move together)
                                if is_slide {
                                     for (si, sl) in note.slide.iter_mut().enumerate() {
                                    let old_tail_m = o.time + o.slide.get(si).map(|x| x.slide_duration).unwrap_or(sl.slide_duration);
                                    let old_start_m = o.time + o.slide.get(si).map(|x| x.slide_start_delay).unwrap_or(sl.slide_start_delay);
                                    sl.slide_duration = (old_tail_m - new_t).max(0.0);
                                    sl.slide_start_delay= (old_start_m - new_t).max(0.0);
                                     }
                                } else {
                                   for (si, sl) in note.slide.iter_mut().enumerate() {
                                        if let Some(old) = o.slide.get(si) {
                                            sl.slide_duration = old.slide_duration;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    DragPart::Tail => {
                        if let Some(o) = orig {
                            if is_slide {
                                let si = app.drag_slide_idx.unwrap_or(0);
                                if let Some(sl) = note.slide.get_mut(si) {
                                    sl.slide_duration = (new_t - o.time).max(0.0);
                                }
                            } else {
                                note.hold_duration = (new_t - o.time).max(0.0);
                            }
                        }
                    }
                    DragPart::SlideDelayEnd => {
                        if let Some(o) = orig {
                            let si = app.drag_slide_idx.unwrap_or(0);
                            if let Some(sl) = note.slide.get_mut(si) {
                                let max_dur = o.slide.get(si).map(|x| x.slide_duration).unwrap_or(sl.slide_duration);
                                let delay = (new_t - o.time).max(0.0).min(max_dur);
                                sl.slide_start_delay = delay;
                            }
                        }
                    }
                    DragPart::Body => {
                        if let Some(o) = orig {
                            note.time = new_t;
                            if is_slide {
                                for (si, sl) in note.slide.iter_mut().enumerate() {
                                    if let Some(old) = o.slide.get(si) {
                                        sl.slide_duration = old.slide_duration;
                                        sl.slide_start_delay = old.slide_start_delay;
                                    }
                                }
                            } else {
                                note.hold_duration = o.hold_duration;
                            }
                        } else {
                            note.time = new_t;
                        }
                    }
                }
                if !matches!(part, DragPart::Tail | DragPart::SlideDelayEnd) {
                    if is_slide && matches!(part, DragPart::Head) {
                        // Slide head: use wrap_lane for wrapping
                        if let Some(o) = orig {
                            let lane_delta = raw_lane - (o.lane as i32 - 1);
                            note.lane = wrap_lane(o.lane, lane_delta);
                        } else {
                            note.lane = new_lane;
                        }
                    } else {
                        note.lane = new_lane;
                    }
                }
                let dur = if is_slide {
                    note.slide.iter().map(|s| s.slide_duration).fold(0.0_f32, f32::max)
                } else {
                    note.hold_duration
                };
                let t_val = note.time;
                app.set_status(format!("#{i}: t={:.2} dur={:.2}", t_val, dur));
                // Move sibling slides (same star group) together
                if is_slide && matches!(part, DragPart::Body) {
                    for j in 0..app.chart.notes.len() {
                        if j == i { continue; }
                        let sib = &app.chart.notes[j];
                        if matches!(sib.note_type, super::types::NoteType::Slide)
                            && sib.time == prev_time && sib.lane == prev_lane
                        {
                            app.chart.notes[j].time = new_t;
                            app.chart.notes[j].lane = new_lane;
                        }
                    }
                }
            }
        }
    }
    if is_mouse_button_released(MouseButton::Left) && app.drag_start_pos.is_some() {
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
            app.selected_note_ids.clear();
            for (i, note) in app.chart.notes.iter().enumerate() {
                if app.hidden_notes.contains(&note.id) { continue; }
                let (cx, ny, _, _) = note_screen_pos(note, now, track_x, ruler_w, lane_w, judge_y, &app.chart.bpms, scroll_speed);
                if cx >= x1 && cx <= x2 && ny >= y1 && ny <= y2 {
                    app.selected_notes.push(i);
                    app.selected_note_ids.insert(note.id);
                }
            }
            if !app.selected_notes.is_empty() { app.set_selected_note(Some(app.selected_notes[0])); }
            app.set_status(format!("Selected {} notes", app.selected_notes.len()));
        } else if !moved {
            // Simple click (no drag)
            let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            if let Some(candidate) = app.press_note_candidate {
                let cid = app.chart.notes.get(candidate).map(|n| n.id);
                if shift && !app.selected_notes.contains(&candidate) {
                    // Shift+click: 追加到多选
                    app.selected_notes.push(candidate);
                    if let Some(id) = cid { app.selected_note_ids.insert(id); }
                    app.set_status(format!("Selected {} notes", app.selected_notes.len()));
                } else if !shift {
                    // 单击：替换选择
                    app.set_selected_note(Some(candidate));
                    app.selected_notes = vec![candidate];
                    app.selected_note_ids.clear();
                    if let Some(id) = cid { app.selected_note_ids.insert(id); }
                    app.set_status(format!("Selected 1 notes"));
                }
            } else {
                // Clicked on empty space → place note via tool
                let dt = (judge_y - pos.y) / scroll_speed;
                let t = snap_secs_to_measure((now + dt).max(0.0), &app.chart.bpms);
                let lx = pos.x - lanes_x;
                let lane = if lx >= 0.0 {
                    let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
                    if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
                } else { 1 };
                handle_tool_click(app, t, lane);
            }
        }
        // Clean up
        app.press_note_candidate = None;
        app.dragging_note = None; app.drag_start_pos = None; app.drag_orig_note = None; app.drag_slide_idx = None;
        app.box_start = None; app.box_end = None; app.box_anchor_t = None;
        app.recompute_each();
    }
    // ── Click while pasting: place notes ──
    if app.pasting && is_mouse_button_pressed(MouseButton::Left) && pos.x >= lanes_x {
        app.push_undo();
        let min_t = app.clipboard.iter().map(|n| n.time).fold(f32::MAX, f32::min);
        let dt = (judge_y - pos.y) / scroll_speed;
        let target = snap_secs_to_measure((now + dt).max(0.0), &app.chart.bpms);
        let offset = target - min_t;
        let lx = pos.x - lanes_x;
        let tgt_lane = if lx >= 0.0 {
            let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
            if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
        } else { 1 };
        let anchor_lane = app.clipboard.first().map(|n| n.lane).unwrap_or(1);
        let lane_off = tgt_lane as i32 - anchor_lane as i32;
        for mut n in app.clipboard.clone() {
            n.id = app.next_id();
            n.time = snap_grid(n.time + offset);
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
            let nid = app.next_id();
            app.chart.notes.push(Note {
                id: nid, time: t, lane, note_type: nt, hold_duration: 0.0,
                is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false,
                slide: vec![],
                template_source: None,
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
                    let nid = app.next_id();
                    app.chart.notes.push(Note {
                        id: nid, time: head_t, lane: lane0, note_type: NoteType::Hold,
                        hold_duration: dur, is_each: false,
                        is_break: false, is_ex: false, is_star: false, is_tapless: false,
                        slide: vec![],
                        template_source: None,
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
                    let nid = app.next_id();
                    app.chart.notes.push(Note {
                        id: nid, time: head_t, lane: lane0, note_type: NoteType::Slide,
                        hold_duration: 0.0, is_each: false,
                        is_break: false, is_ex: false, is_star: false, is_tapless: false,
                        slide: vec![super::types::Slide {
                            segments: vec![],
                            slide_duration,
                            slide_start_delay,
                            slide_is_break: false,
                        }],
                        template_source: None,
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
/// For slides with waypoints, tail_cx is at the last slide_point's lane.
pub fn note_screen_pos(note: &super::types::Note, now: f32, track_x: f32, ruler_w: f32, lane_w: f32, judge_y: f32, bpms: &[BpmChange], scroll_speed: f32) -> (f32, f32, f32, bool) {
    let zone = sanitize_note_zone(note.note_type, note.lane);
    let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
    let cx = track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
    let ns = note_secs(note, bpms);
    let dt = ns - now;
    let ny = judge_y - dt * scroll_speed;
    let (tail_t, has_tail) = match note.note_type {
        super::types::NoteType::Hold => (hold_tail_time(note, bpms), true),
        super::types::NoteType::Slide if !note.slide.is_empty() => {
            let max_d = note.slide.iter().map(|s| s.slide_duration).fold(0.0_f32, f32::max);
            (ns + mdur_to_secs(max_d, note.time, bpms), max_d > 0.0)
        }
        _ => (ns, false),
    };
    let tail_ny = if has_tail { judge_y - (tail_t - now) * scroll_speed } else { ny };
    (cx, ny, tail_ny, has_tail)
}

/// Get the x position of a slide note's tail (last A-zone waypoint lane).
pub fn slide_tail_cx(note: &super::types::Note, track_x: f32, ruler_w: f32, lane_w: f32) -> f32 {
    let idx = note.slide.iter().enumerate()
        .max_by(|a, b| a.1.slide_duration.partial_cmp(&b.1.slide_duration).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    slide_tail_cx_for(note, idx, track_x, ruler_w, lane_w)
}

pub fn slide_tail_cx_for(note: &super::types::Note, slide_idx: usize, track_x: f32, ruler_w: f32, lane_w: f32) -> f32 {
    if let Some(sl) = note.slide.get(slide_idx) {
        for seg in sl.segments.iter().rev() {
            if let Some(last) = seg.points.iter().rev().find(|sp| sp.zone >= 1 && sp.zone <= 8) {
                let li = (last.zone.to_id().saturating_sub(1) as usize).min(LANE_COUNT - 2);
                return track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
            }
        }
    }
    let zone = sanitize_note_zone(note.note_type, note.lane);
    let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
    track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5
}
