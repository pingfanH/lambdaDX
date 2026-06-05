use macroquad::input::{is_key_down, is_key_pressed, is_key_released, KeyCode, TouchPhase};
use lambda_dx::chart;
use lambda_dx::state::AppState;
use lambda_dx::types::{Mode, PadGeom, PointerEvent, RecordInputId, SlidePoint, SlideShape, UiAction, UiButton, SPEED_MAX, SPEED_MIN, SPEED_STEP};
use lambda_dx::types::zone::PadZone;
use lambda_dx::ui::{rect_contains};
use serde_json::json;
use crate::state::PlayerState;

fn push_lnmai_sensor_click(app: &mut PlayerState, zone: PadZone) {
    if app.mode != Mode::Playing {
        return;
    }
    let t_us = (app.song_time() as f64 * 1_000_000.0) as u64;
    app.lnmai_input_events.push((t_us, json!({"sensorClick": {"tp": t_us, "area": zone.to_string()}})));
}

fn update_lnmai_sensor_hold(app: &mut PlayerState, pointer_id: u64, new_zone: Option<PadZone>) {
    if app.mode != Mode::Playing {
        app.active_sensor_holds.remove(&pointer_id);
        return;
    }

    let old_zone = app.active_sensor_holds.get(&pointer_id).copied();
    if old_zone == new_zone {
        return;
    }

    let t_us = (app.song_time() as f64 * 1_000_000.0) as u64;

    if let Some(zone) = old_zone {
        app.lnmai_input_events.push((t_us, json!({
            "sensorHold": {"tp": t_us, "area": zone.to_string(), "isDown": false}
        })));
        app.active_sensor_holds.remove(&pointer_id);
    }

    if let Some(zone) = new_zone {
        app.lnmai_input_events.push((t_us, json!({
            "sensorHold": {"tp": t_us, "area": zone.to_string(), "isDown": true}
        })));
        app.active_sensor_holds.insert(pointer_id, zone);
    }
}

pub fn trigger_ui_action(app: &mut PlayerState, action: UiAction) {
    match action {
        UiAction::TogglePlay => app.toggle_play(),
        UiAction::ToggleRecord => app.toggle_record(),
        UiAction::Save =>{},
        //     match chart::save_recording_doc(app) {
        //     Ok(path) => app.set_status(format!("Saved recording: {}", path.display())),
        //     Err(err) => app.set_status(format!("Save failed: {err}")),
        // },
        UiAction::Load => match chart::load_latest_saved_chart() {
            Ok(chart) => {
                app.set_chart(chart);
                app.set_status("Loaded latest saved chart".to_string());
            }
            Err(err) => app.set_status(format!("Load latest failed: {err}")),
        },
        UiAction::Clear => {
            app.recording_hits.clear();
            app.recording_notes.clear();
            app.active_record_holds.clear();
            app.active_pointer_zones.clear();
            app.active_sensor_holds.clear();
            app.set_status("Cleared recording hits".to_string());
        }
        UiAction::ToggleAudio => {
            app.audio_enabled = !app.audio_enabled;
            app.set_status(format!("Audio enabled: {}", app.audio_enabled));
            if !app.audio_enabled {
                app.stop_audio_if_any();
            } else if matches!(app.mode, Mode::Playing | Mode::Recording) {
                app.request_audio_start();
            }
        }
        UiAction::RecSpeedDown => {
            app.set_record_speed((app.record_speed - SPEED_STEP).max(SPEED_MIN));
            app.set_status(format!("Record speed: {:.1}x", app.record_speed));
        }
        UiAction::RecSpeedUp => {
            app.set_record_speed((app.record_speed + SPEED_STEP).min(SPEED_MAX));
            app.set_status(format!("Record speed: {:.1}x", app.record_speed));
        }
        UiAction::PlaySpeedDown => {
            app.set_play_speed((app.play_speed - SPEED_STEP).max(SPEED_MIN));
            app.set_status(format!("Playback speed: {:.1}x", app.play_speed));
        }
        UiAction::PlaySpeedUp => {
            app.set_play_speed((app.play_speed + SPEED_STEP).min(SPEED_MAX));
            app.set_status(format!("Playback speed: {:.1}x", app.play_speed));
        }
        // UiAction::TouchSpeedDown => {
        //     app.set_touch_speed((app.touch_speed - TOUCH_SPEED_STEP).max(TOUCH_SPEED_MIN));
        //     app.status = format!("Touch speed: {:.1}x", app.touch_speed);
        // }
        // UiAction::TouchSpeedUp => {
        //     app.set_touch_speed((app.touch_speed + TOUCH_SPEED_STEP).min(TOUCH_SPEED_MAX));
        //     app.status = format!("Touch speed: {:.1}x", app.touch_speed);
        // }
        UiAction::TogglePadOnly => {
            app.show_pad_only = !app.show_pad_only;
            app.set_status(format!("Pad only: {}", app.show_pad_only));
        }
        UiAction::ToggleMobileUi => {
            app.mobile_ui = !app.mobile_ui;
            app.set_status(format!("Mobile UI mode: {}", app.mobile_ui));
        }
    }
}

pub fn handle_lane_input(app: &mut PlayerState) {
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
          //  app.start_record_hold_input(input_id, PadZone::from(lane));
        }
        if is_key_released(key) {
            let input_id = RecordInputId::Key(lane);
           // app.finish_record_hold_input(input_id);
        }
    }
}
pub fn handle_touch_controls(
    app: &mut PlayerState,
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
                                        let pattern = lambda_dx::simai_io::shape_to_simai_pattern(Some(shape));
                                        let points =  lambda_dx::simai_io::simai_pattern_to_points(
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
                                    seg.shape =  lambda_dx::slide_match::match_slide_shape(
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
                    update_lnmai_sensor_hold(app, ev.id, Some(zone));
                    push_lnmai_sensor_click(app, zone);
                    app.push_feedback(zone, 0.12);
                    //app.start_record_hold_input(RecordInputId::Pointer(ev.id), zone);
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
                            //app.record_slide_zone(RecordInputId::Pointer(ev.id), zone);
                            app.active_pointer_zones.insert(ev.id, zone);
                            update_lnmai_sensor_hold(app, ev.id, Some(zone));
                            push_lnmai_sensor_click(app, zone);
                            app.push_feedback(zone, 0.10);
                        } else {
                            app.active_pointer_zones.remove(&ev.id);
                            update_lnmai_sensor_hold(app, ev.id, None);
                        }
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                app.prev_pointer_pos.remove(&ev.id);
                app.active_pointer_zones.remove(&ev.id);
                update_lnmai_sensor_hold(app, ev.id, None);
                app.finish_record_hold_input(RecordInputId::Pointer(ev.id));
            }
            _ => {}
        }
    }
}

pub fn handle_global_hotkeys(app: &mut PlayerState) {
    if is_key_pressed(KeyCode::Space) {
        app.toggle_play();
    }
    if is_key_pressed(KeyCode::R) {
        app.toggle_replay();
    }
}

fn lane_to_lnmai_button_zone(lane: u8) -> String {
    if lane == super::types::PAD_C_ZONE {
        "C".to_string()
    } else {
        format!("K{}", lane)
    }
}

pub fn collect_lnmai_input_events(app: &mut PlayerState) {
    if app.mode != Mode::Playing { return; }
    if app.editing_slide_path.is_some() { return; }

    let t_us = (app.song_time() as f64 * 1_000_000.0) as u64;

    // Autoplay: inject events from pre-generated tactic
    if app.autoplay {
        let autoplay_pointer_id = u64::MAX;

        // Collect all due events (clone them to avoid borrow conflicts)
        let due_events: Vec<(u64, serde_json::Value)> = app.autoplay_events[app.autoplay_index..]
            .iter()
            .take_while(|(et, _)| *et <= t_us)
            .map(|(et, ev)| (*et, ev.clone()))
            .collect();
        let due_count = due_events.len();

        for (_t, event_json) in &due_events {
            if let Some(obj) = event_json.as_object() {
                for (key, val) in obj {
                    match key.as_str() {
                        "buttonClick" | "buttonHold" => {
                            if let Some(zone_str) = val.get("zone").and_then(|v| v.as_str()) {
                                if let Some(lane) = zone_str.strip_prefix('K').and_then(|s| s.parse::<u8>().ok()) {
                                    let pad_zone = PadZone::from(lane);
                                    app.push_feedback(pad_zone, 0.12);
                                    if key == "buttonHold" {
                                        let is_down = val.get("isDown").and_then(|v| v.as_bool()).unwrap_or(true);
                                        if is_down {
                                            app.active_pointer_zones.insert(autoplay_pointer_id, pad_zone);
                                        } else {
                                            app.active_pointer_zones.remove(&autoplay_pointer_id);
                                        }
                                    }
                                }
                            }
                        }
                        "sensorClick" | "sensorHold" => {
                            if let Some(area_str) = val.get("area").and_then(|v| v.as_str()) {
                                let pad_zone = PadZone::from(area_str);
                                app.push_feedback(pad_zone, 0.10);
                                if key == "sensorHold" {
                                    let is_down = val.get("isDown").and_then(|v| v.as_bool()).unwrap_or(true);
                                    if is_down {
                                        app.active_sensor_holds.insert(autoplay_pointer_id, pad_zone);
                                        app.active_pointer_zones.insert(autoplay_pointer_id, pad_zone);
                                    } else {
                                        app.active_sensor_holds.remove(&autoplay_pointer_id);
                                        app.active_pointer_zones.remove(&autoplay_pointer_id);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    break;
                }
            }
        }

        // Push events to lnmai input queue
        for (event_t_us, event_json) in due_events {
            app.lnmai_input_events.push((event_t_us, event_json));
        }

        app.autoplay_index += due_count;
        return;
    }

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
            let zone_str = lane_to_lnmai_button_zone(lane);
            app.lnmai_input_events.push((t_us, json!({"buttonHold": {"tp": t_us, "zone": zone_str, "isDown": true}})));
            app.lnmai_input_events.push((t_us, json!({"buttonClick": {"tp": t_us, "zone": zone_str}})));
            app.push_feedback(PadZone::from(lane), 0.12);
        }
        if is_key_released(key) {
            let zone_str = lane_to_lnmai_button_zone(lane);
            app.lnmai_input_events.push((t_us, json!({"buttonHold": {"tp": t_us, "zone": zone_str, "isDown": false}})));
        }
    }

}
