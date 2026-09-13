use crate::state::{PlayerPage, PlayerState};
use lambda_dx::chart;
use lambda_dx::types::zone::PadZone;
use lambda_dx::types::{
    Mode, PadGeom, PointerEvent, RecordInputId, SPEED_MAX, SPEED_MIN, SPEED_STEP, SlidePoint,
    UiAction, UiButton,
};
use lambda_dx::ui::rect_contains;
use macroquad::input::{KeyCode, TouchPhase, is_key_pressed, is_key_released};
use std::collections::HashMap;

fn update_active_sensor_hold(app: &mut PlayerState, pointer_id: u64, zone: Option<PadZone>) {
    if let Some(zone) = zone {
        app.active_sensor_holds.insert(pointer_id, zone);
    } else {
        app.active_sensor_holds.remove(&pointer_id);
    }
}

fn sampled_motion_path(
    prev: Option<macroquad::prelude::Vec2>,
    position: macroquad::prelude::Vec2,
) -> Vec<macroquad::prelude::Vec2> {
    let Some(start) = prev else {
        return vec![position];
    };
    let delta = position - start;
    let dist = delta.length();
    if dist <= f32::EPSILON {
        return vec![position];
    }

    let steps = (dist / 4.0).ceil().max(1.0) as usize;
    (1..=steps)
        .map(|i| start + delta * (i as f32 / steps as f32))
        .collect()
}

fn zone_state_transitions(
    active_sources: &mut HashMap<u64, PadZone>,
    source_id: u64,
    new_zone: Option<PadZone>,
) -> Vec<(PadZone, bool)> {
    let old_zone = active_sources.get(&source_id).copied();
    if old_zone == new_zone {
        return Vec::new();
    }

    let old_zone_still_held = old_zone.is_some_and(|old_zone| {
        active_sources
            .iter()
            .any(|(id, zone)| *id != source_id && *zone == old_zone)
    });
    let new_zone_was_held = new_zone.is_some_and(|new_zone| {
        active_sources
            .iter()
            .any(|(id, zone)| *id != source_id && *zone == new_zone)
    });

    match new_zone {
        Some(new_zone) => {
            active_sources.insert(source_id, new_zone);
        }
        None => {
            active_sources.remove(&source_id);
        }
    }

    let mut transitions = Vec::with_capacity(2);
    if let Some(old_zone) = old_zone {
        if !old_zone_still_held {
            transitions.push((old_zone, false));
        }
    }
    if let Some(new_zone) = new_zone {
        if !new_zone_was_held {
            transitions.push((new_zone, true));
        }
    }
    transitions
}

fn update_active_zone(app: &mut PlayerState, source_id: u64, zone: Option<PadZone>) {
    update_active_sensor_hold(app, source_id, zone);
    for (zone, is_down) in zone_state_transitions(&mut app.active_pointer_zones, source_id, zone) {
        app.record_engine_input(zone, is_down);
    }
}

pub fn trigger_ui_action(app: &mut PlayerState, action: UiAction) {
    match action {
        UiAction::TogglePlay => app.toggle_play(),
        UiAction::ToggleRecord => app.toggle_record(),
        UiAction::Save => {}
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
            app.clear_active_screen_inputs();
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
    if app.player_ui.page != PlayerPage::Gameplay || app.mode != Mode::Playing {
        return;
    }
    // Don't record taps while editing slide trajectory
    if app.editing_slide_path.is_some() {
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
        let input_id = u64::MAX - 100 - lane as u64;
        let zone = PadZone::from(lane);
        if is_key_pressed(key) {
            update_active_zone(app, input_id, Some(zone));
            app.push_feedback(zone, 0.12);
            if app.judge_engine.is_none() {
                if let (Some(sfx), Some(player)) = (&app.sfx_tap, &mut app.sfx_player) {
                    player.play(sfx, 1.0);
                }
            }
        }
        if is_key_released(key) {
            update_active_zone(app, input_id, None);
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

                let zone = app
                    .pad_svg
                    .as_ref()
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
                                    if matches!(n.note_type, super::types::NoteType::Slide)
                                        && n.lane >= 1
                                        && n.lane <= 8
                                    {
                                        let pattern = lambda_dx::simai_io::shape_to_simai_pattern(
                                            Some(shape),
                                        );
                                        let points = lambda_dx::simai_io::simai_pattern_to_points(
                                            n.lane.saturating_sub(1),
                                            z.to_id().saturating_sub(1),
                                            pattern,
                                            None,
                                        );
                                        if n.slide.is_empty() {
                                            n.slide.push(super::types::Slide {
                                                segments: vec![super::types::SlideSegment {
                                                    points,
                                                    shape,
                                                }],
                                                slide_duration: 0.5,
                                                slide_start_delay: 0.0625,
                                                slide_is_break: false,
                                            });
                                            app.editing_slide_idx = Some(0);
                                        } else {
                                            let edit_idx = app
                                                .editing_slide_idx
                                                .unwrap_or(0)
                                                .min(n.slide.len().saturating_sub(1));
                                            let sl = &mut n.slide[edit_idx];
                                            sl.segments
                                                .push(super::types::SlideSegment { points, shape });
                                        }
                                        app.set_status(format!(
                                            "Set shape {:?} → lane {}",
                                            shape, z
                                        ));
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
                                            points: vec![],
                                            shape: super::types::SlideShape::Line,
                                        }],
                                        slide_duration: 0.5,
                                        slide_start_delay: 0.0625,
                                        slide_is_break: false,
                                    });
                                    app.editing_slide_idx = Some(0);
                                }
                                let edit_idx = app
                                    .editing_slide_idx
                                    .unwrap_or(0)
                                    .min(n.slide.len().saturating_sub(1));
                                let sl = &mut n.slide[edit_idx];
                                if sl.segments.is_empty() {
                                    sl.segments.push(super::types::SlideSegment {
                                        points: vec![],
                                        shape: super::types::SlideShape::Line,
                                    });
                                }
                                let seg = &mut sl.segments[0];
                                let dur = sl.slide_duration;
                                let beat_offset = (seg.points.len() as f32 + 1.0)
                                    * (dur.max(0.3) / (seg.points.len() as f32 + 2.0));
                                let last_zone = seg
                                    .points
                                    .last()
                                    .copied()
                                    .unwrap_or(SlidePoint::from(PadZone::from(n.lane)));
                                if z != last_zone.zone {
                                    seg.points.push(SlidePoint::from(PadZone::from(z)));
                                    seg.shape = lambda_dx::slide_match::match_slide_shape(
                                        n.lane,
                                        &seg.points,
                                    )
                                    .unwrap_or(super::types::SlideShape::Line);
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
                    update_active_zone(app, ev.id, Some(zone));
                    app.push_feedback(zone, 0.12);
                    //app.start_record_hold_input(RecordInputId::Pointer(ev.id), zone);
                }
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                let prev = app.prev_pointer_pos.get(&ev.id).copied();
                app.prev_pointer_pos.insert(ev.id, ev.position);

                let samples = sampled_motion_path(prev, ev.position);

                for sample in &samples {
                    let old_zone = app.active_pointer_zones.get(&ev.id).copied();
                    let new_zone = app
                        .pad_svg
                        .as_ref()
                        .and_then(|svg| svg.hit_test(*sample, &pad));

                    if old_zone != new_zone {
                        if let Some(zone) = new_zone {
                            //app.record_slide_zone(RecordInputId::Pointer(ev.id), zone);
                            update_active_zone(app, ev.id, Some(zone));
                            app.push_feedback(zone, 0.10);
                        } else {
                            update_active_zone(app, ev.id, None);
                        }
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                app.prev_pointer_pos.remove(&ev.id);
                update_active_zone(app, ev.id, None);
                app.finish_record_hold_input(RecordInputId::Pointer(ev.id));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PadZone, sampled_motion_path, zone_state_transitions};
    use macroquad::prelude::vec2;
    use std::collections::HashMap;

    #[test]
    fn sampled_motion_path_includes_the_current_endpoint() {
        let samples = sampled_motion_path(Some(vec2(0.0, 0.0)), vec2(10.0, 0.0));

        assert_eq!(samples.last().copied(), Some(vec2(10.0, 0.0)));
        assert_ne!(samples.first().copied(), Some(vec2(0.0, 0.0)));
    }

    #[test]
    fn releasing_one_of_two_sources_keeps_zone_held() {
        let zone = PadZone::from(9);
        let mut active_sources = HashMap::new();

        assert_eq!(
            zone_state_transitions(&mut active_sources, 1, Some(zone)),
            vec![(zone, true)]
        );
        assert!(zone_state_transitions(&mut active_sources, 2, Some(zone)).is_empty());

        assert!(zone_state_transitions(&mut active_sources, 1, None).is_empty());
        assert_eq!(active_sources.get(&2), Some(&zone));

        assert_eq!(
            zone_state_transitions(&mut active_sources, 2, None),
            vec![(zone, false)]
        );
    }

    #[test]
    fn moving_source_releases_only_its_last_zone_and_enters_new_zone_once() {
        let old_zone = PadZone::from(9);
        let new_zone = PadZone::from(10);
        let mut active_sources = HashMap::new();

        assert_eq!(
            zone_state_transitions(&mut active_sources, 1, Some(old_zone)),
            vec![(old_zone, true)]
        );
        assert_eq!(
            zone_state_transitions(&mut active_sources, 2, Some(old_zone)),
            Vec::<(PadZone, bool)>::new()
        );

        assert_eq!(
            zone_state_transitions(&mut active_sources, 1, Some(new_zone)),
            vec![(new_zone, true)]
        );
        assert_eq!(active_sources.get(&2), Some(&old_zone));
        assert_eq!(active_sources.get(&1), Some(&new_zone));
        assert_eq!(
            zone_state_transitions(&mut active_sources, 2, None),
            vec![(old_zone, false)]
        );
    }
}

pub fn handle_global_hotkeys(app: &mut PlayerState) {
    if is_key_pressed(KeyCode::Escape) {
        match app.player_ui.page {
            PlayerPage::Start => {}
            PlayerPage::SongSelect => app.player_ui.page = PlayerPage::Start,
            PlayerPage::Settings => app.player_ui.close_settings(),
            PlayerPage::Gameplay => {
                app.toggle_play();
                app.player_ui.page = PlayerPage::Pause;
            }
            PlayerPage::Pause => {
                app.toggle_play();
                app.player_ui.page = PlayerPage::Gameplay;
            }
        }
        return;
    }

    if is_key_pressed(KeyCode::Space) {
        match app.player_ui.page {
            PlayerPage::Gameplay => {
                app.toggle_play();
                app.player_ui.page = PlayerPage::Pause;
            }
            PlayerPage::Pause => {
                app.toggle_play();
                app.player_ui.page = PlayerPage::Gameplay;
            }
            PlayerPage::Start | PlayerPage::SongSelect | PlayerPage::Settings => {}
        }
    }
    if is_key_pressed(KeyCode::A) {
        app.autoplay = !app.autoplay;
        app.set_status(format!("Autoplay: {}", app.autoplay));
    }
    if is_key_pressed(KeyCode::R) && app.player_ui.page == PlayerPage::Gameplay {
        app.toggle_replay();
    }
}
