//! Keyboard bindings.
//!
//! * `1`–`8` / `T` — synthetic lane presses (A-ring / centre), driving the same
//!   active-zone highlight as touches.
//! * `Space` — play / pause.
//! * `R` — restart from 0. `Home` — jump to 0.
//! * `←` / `→` — seek ∓1 s. `↑` / `↓` — playback speed.
//! * `A` — toggle audio. `O` — toggle autoplay.

use macroquad::prelude::*;

use crate::app::types::zone::PadZone;
use crate::app::types::{Mode, PAD_C_ZONE};
use crate::player::input::pointer::update_pointer_zone;
use crate::player::state::PadPreviewState;

/// Keyboard lane bindings: 1-8 for the A ring, T for the centre zone.
pub fn handle_lane_input(app: &mut PadPreviewState) {
    let bindings = [
        (KeyCode::Key1, 1_u8),
        (KeyCode::Key2, 2_u8),
        (KeyCode::Key3, 3_u8),
        (KeyCode::Key4, 4_u8),
        (KeyCode::Key5, 5_u8),
        (KeyCode::Key6, 6_u8),
        (KeyCode::Key7, 7_u8),
        (KeyCode::Key8, 8_u8),
        (KeyCode::T, PAD_C_ZONE),
    ];

    for (key, lane) in bindings {
        // Synthetic pointer ids well below any real pointer id.
        let pointer_id = u64::MAX - 100 - lane as u64;
        let zone = PadZone::from(lane);
        if is_key_pressed(key) {
            update_pointer_zone(app, pointer_id, Some(zone));
        }
        if is_key_released(key) {
            update_pointer_zone(app, pointer_id, None);
        }
    }
}

/// Global playback hotkeys.
pub fn handle_global_hotkeys(app: &mut PadPreviewState) {
    if is_key_pressed(KeyCode::F1) {
        app.show_params = !app.show_params;
    }
    if is_key_pressed(KeyCode::Space) {
        app.toggle_play();
    }
    if is_key_pressed(KeyCode::R) {
        app.start_playback_at(0.0);
    }
    if is_key_pressed(KeyCode::Home) {
        app.seek_audio_to(0.0);
        app.mode_song_offset = 0.0;
        app.timeline_view_time = 0.0;
        app.mode_wall_anchor = get_time();
        app.reconcile_slide_progress_for(0.0);
    }
    if is_key_pressed(KeyCode::Left) {
        seek_relative(app, -1.0);
    }
    if is_key_pressed(KeyCode::Right) {
        seek_relative(app, 1.0);
    }
    if is_key_pressed(KeyCode::Up) {
        app.nudge_play_speed(0.1);
    }
    if is_key_pressed(KeyCode::Down) {
        app.nudge_play_speed(-0.1);
    }
    if is_key_pressed(KeyCode::A) {
        app.audio_enabled = !app.audio_enabled;
        if !app.audio_enabled {
            app.stop_audio_if_any();
        } else if app.mode == Mode::Playing {
            app.request_audio_start();
        }
        app.set_status(format!("Audio enabled: {}", app.audio_enabled));
    }
    if is_key_pressed(KeyCode::O) {
        let on = !app.autoplay;
        crate::player::autoplay::set_on(app, on);
    }
}

/// Seek by `delta` seconds from the current song time.
fn seek_relative(app: &mut PadPreviewState, delta: f32) {
    let t = (app.song_time() + delta).max(0.0);
    app.seek_audio_to(t);
    app.mode_song_offset = t;
    app.timeline_view_time = t;
    app.mode_wall_anchor = get_time();
    app.reconcile_slide_progress_for(t);
}
