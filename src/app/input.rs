use macroquad::prelude::*;

use super::chart;
use super::state::AppState;
use super::types::{PadGeom, PointerEvent, RecordInputId, UiButton, MOUSE_POINTER_ID, SPEED_MAX, SPEED_MIN, SPEED_STEP};
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

    // Touch speed: , and .
    // if is_key_pressed(KeyCode::Comma) {
    //     app.set_touch_speed((app.touch_speed - TOUCH_SPEED_STEP).max(TOUCH_SPEED_MIN));
    //     app.status = format!("Touch speed: {:.1}x", app.touch_speed);
    // }
    // if is_key_pressed(KeyCode::Period) {
    //     app.set_touch_speed((app.touch_speed + TOUCH_SPEED_STEP).min(TOUCH_SPEED_MAX));
    //     app.status = format!("Touch speed: {:.1}x", app.touch_speed);
    // }

    if is_key_pressed(KeyCode::L) {
        match chart::load_latest_saved_chart() {
            Ok(chart) => {
                app.chart = chart;
                app.status = "Loaded latest saved chart".to_string();
            }
            Err(err) => {
                app.status = format!("Load latest failed: {err}");
            }
        }
    }

    if is_key_pressed(KeyCode::S) {
        match chart::save_recording_doc(app) {
            Ok(path) => {
                app.status = format!("Saved recording: {}", path.display());
            }
            Err(err) => {
                app.status = format!("Save failed: {err}");
            }
        }
    }
}

pub(crate) fn handle_lane_input(app: &mut AppState) {
    if app.mode != super::types::Mode::Recording {
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
        (KeyCode::T, 9_u8),
    ];

    for (key, lane) in bindings {
        if is_key_pressed(key) {
            app.start_record_hold_input(RecordInputId::Key(lane), lane);
        }
        if is_key_released(key) {
            app.finish_record_hold_input(RecordInputId::Key(lane));
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

    // Avoid duplicate events when touch is also mapped to mouse.
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
                if let Some(btn) = buttons.iter().find(|b| rect_contains(b.rect, ev.position)) {
                    trigger_ui_action(app, btn.action);
                    continue;
                }

                let zone = app
                    .pad_svg
                    .as_ref()
                    .and_then(|svg| svg.hit_test(ev.position, &pad));
                if let Some(zone) = zone {
                    app.active_pointer_zones.insert(ev.id, zone);
                    app.push_feedback(zone, 0.12);
                    if app.mode == super::types::Mode::Recording {
                        app.start_record_hold_input(RecordInputId::Pointer(ev.id), zone);
                    }
                }
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                let old_zone = app.active_pointer_zones.get(&ev.id).copied();
                let new_zone = app
                    .pad_svg
                    .as_ref()
                    .and_then(|svg| svg.hit_test(ev.position, &pad));
                if old_zone != new_zone {
                    if app.mode == super::types::Mode::Recording {
                        if old_zone.is_some() {
                            app.finish_record_hold_input(RecordInputId::Pointer(ev.id));
                        }
                        if let Some(zone) = new_zone {
                            app.start_record_hold_input(RecordInputId::Pointer(ev.id), zone);
                        }
                    }
                    if let Some(zone) = new_zone {
                        app.active_pointer_zones.insert(ev.id, zone);
                        app.push_feedback(zone, 0.10);
                    } else {
                        app.active_pointer_zones.remove(&ev.id);
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                app.active_pointer_zones.remove(&ev.id);
                if app.mode == super::types::Mode::Recording {
                    app.finish_record_hold_input(RecordInputId::Pointer(ev.id));
                }
            }
        }
    }
}
