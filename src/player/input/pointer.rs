//! Pointer handling: turn macroquad touches/mouse into pad zone presses.
//!
//! Each pointer id is tracked independently, so multi-touch and slides between
//! zones all work. A zone transition lights the zone (blue), pulses it orange,
//! and forwards judgment input to lnmai-core.

use macroquad::math::Vec2;
use macroquad::prelude::*;

use crate::app::types::zone::PadZone;
use crate::app::types::{MOUSE_POINTER_ID, PadGeom, PointerEvent};
use crate::player::state::PadPreviewState;

/// Drain macroquad's touch/mouse state into a uniform pointer-event list.
/// Mouse is only emitted when no touch is active, so desktop mouse and a real
/// touchscreen never double-fire.
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

/// Interpolate straight-line motion between two pointer samples so a fast drag
/// cannot skip over a zone between frames.
fn sampled_motion_path(prev: Option<Vec2>, position: Vec2) -> Vec<Vec2> {
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

/// Apply a pointer's zone. No-op unless the zone actually changed, so holding
/// still does not spam feedback. Shared with the keyboard lane keys.
pub(super) fn update_pointer_zone(
    app: &mut PadPreviewState,
    pointer_id: u64,
    zone: Option<PadZone>,
) {
    let old = app.active_pointer_zones.get(&pointer_id).copied();
    if old == zone {
        return;
    }
    match zone {
        Some(zone) => {
            app.active_pointer_zones.insert(pointer_id, zone);
            app.push_feedback(zone, 0.12);
            if app.has_engine() {
                let tp = (app.song_time().max(0.0) * 1e6) as i64;
                app.queue_engine_press(zone, tp);
            }
        }
        None => {
            if app.has_engine() {
                if let Some(prev) = old {
                    let tp = (app.song_time().max(0.0) * 1e6) as i64;
                    app.queue_engine_release(prev, tp);
                }
            }
            app.active_pointer_zones.remove(&pointer_id);
        }
    }
}

/// Route touch/mouse events to zones.
pub fn handle_touch_controls(
    app: &mut PadPreviewState,
    pad: PadGeom,
    pointer_events: &[PointerEvent],
) {
    for ev in pointer_events {
        match ev.phase {
            TouchPhase::Started => {
                app.prev_pointer_pos.insert(ev.id, ev.position);
                let zone = app
                    .pad_svg
                    .as_ref()
                    .and_then(|svg| svg.hit_test(ev.position, &pad));
                update_pointer_zone(app, ev.id, zone);
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                let prev = app.prev_pointer_pos.get(&ev.id).copied();
                app.prev_pointer_pos.insert(ev.id, ev.position);
                for sample in sampled_motion_path(prev, ev.position) {
                    let new_zone = app
                        .pad_svg
                        .as_ref()
                        .and_then(|svg| svg.hit_test(sample, &pad));
                    update_pointer_zone(app, ev.id, new_zone);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                app.prev_pointer_pos.remove(&ev.id);
                update_pointer_zone(app, ev.id, None);
            }
        }
    }
}
