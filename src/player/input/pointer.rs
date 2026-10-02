//! Pointer handling: turn macroquad touches/mouse into pad zone presses.
//!
//! Each pointer id is tracked independently, so multi-touch and slides between
//! zones all work. A zone transition lights the zone (blue), pulses it orange,
//! and (via `input::hit`) may add a judgment label.

use macroquad::math::Vec2;
use macroquad::prelude::*;

use crate::app::types::zone::PadZone;
use crate::app::types::{MOUSE_POINTER_ID, PadGeom, PointerEvent};
use crate::player::input::hit::judge_label_for_zone;
use crate::player::state::PadPreviewState;

/// Append the desktop mouse's press/hold/release as pointer events.
fn push_mouse_events(events: &mut Vec<PointerEvent>) {
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

/// Drain macroquad's touch/mouse state into a uniform pointer-event list.
/// Mouse is only emitted when no touch is active, so desktop mouse and a real
/// touchscreen never double-fire.
pub fn collect_pointer_events() -> Vec<PointerEvent> {
    // On desktop Linux, prefer a real Type-B multitouch touchscreen (evdev) so
    // individual fingers are tracked. A plugged mouse still works: the evdev
    // reader sees no contact then, so the mouse events are merged in.
    #[cfg(target_os = "linux")]
    if let Some(mut events) =
        crate::player::input::touch_evdev::poll(vec2(screen_width(), screen_height()))
    {
        let touch_active = events.iter().any(|e| {
            matches!(
                e.phase,
                TouchPhase::Started | TouchPhase::Stationary | TouchPhase::Moved
            )
        });
        if !touch_active {
            push_mouse_events(&mut events);
        }
        return events;
    }

    let touch_events = touches();
    let mut events = Vec::with_capacity(touch_events.len() + 2);

    for t in touch_events {
        events.push(PointerEvent {
            id: t.id,
            phase: t.phase,
            position: t.position,
        });
    }

    if events.is_empty() {
        push_mouse_events(&mut events);
    }

    events
}

/// A single, frame-coherent pointer for driving UI widgets (buttons, sliders).
///
/// Touch, mouse and the Linux evdev touchscreen all feed the same pointer-event
/// list, so building the UI pointer from it makes every UI respond to touch as
/// well as to the mouse. When nothing is down the current mouse position is
/// used, so hover still works.
#[derive(Debug, Clone, Copy, Default)]
pub struct UiPointer {
    pub id: u64,
    pub pos: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
}

/// Build the UI pointer from this frame's pointer events.
pub fn ui_pointer(events: &[PointerEvent]) -> UiPointer {
    let rank = |p: TouchPhase| match p {
        TouchPhase::Started => 3,
        TouchPhase::Moved | TouchPhase::Stationary => 2,
        TouchPhase::Ended | TouchPhase::Cancelled => 1,
    };

    let mut out = UiPointer::default();
    let mut best: Option<(u8, &PointerEvent)> = None;
    for ev in events {
        let r = rank(ev.phase);
        if best.map(|(br, _)| r >= br).unwrap_or(true) {
            best = Some((r, ev));
        }
    }
    match best {
        Some((_, ev)) => {
            out.id = ev.id;
            out.pos = ev.position;
        }
        None => {
            let (mx, my) = mouse_position();
            out.pos = vec2(mx, my);
        }
    }
    out.pressed = events.iter().any(|e| e.phase == TouchPhase::Started);
    out.released = events
        .iter()
        .any(|e| matches!(e.phase, TouchPhase::Ended | TouchPhase::Cancelled));
    out.down = events.iter().any(|e| {
        matches!(
            e.phase,
            TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary
        )
    });
    out
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

/// Apply a pointer's currently-held zones. No-op unless the set actually
/// changed, so holding still does not spam feedback. Shared with the keyboard
/// lane keys.
///
/// When the sensor range trigger is on a pointer can hold several zones at
/// once, so this works on sets: newly-entered zones are pressed (a click + hold
/// on first contact, a hold-only slide when moving between zones) and left
/// zones are released.
pub(super) fn update_pointer_zones(
    app: &mut PadPreviewState,
    pointer_id: u64,
    zones: &[PadZone],
) {
    let old: Vec<PadZone> = app
        .active_pointer_zones
        .get(&pointer_id)
        .cloned()
        .unwrap_or_default();

    let same = old.len() == zones.len() && zones.iter().all(|z| old.contains(z));
    if same {
        return;
    }

    let tp = (app.song_time().max(0.0) * 1e6) as i64;
    let was_empty = old.is_empty();

    // Zones the pointer left: release them.
    for &prev in &old {
        if zones.contains(&prev) {
            continue;
        }
        if app.use_core() {
            app.queue_engine_release(prev, tp);
        }
    }

    // Zones the pointer entered: feedback + engine press/hold.
    for &zone in zones {
        if old.contains(&zone) {
            continue;
        }
        app.push_feedback(zone, 0.12);
        if app.use_core() {
            if was_empty {
                // First contact: a click (head tap / slide star) + hold.
                app.queue_engine_press(zone, tp);
            } else {
                // Sliding onto another sensor: hold-only — no click (the core's
                // slide body areas are hold-only; a click there advances an area
                // too early).
                app.queue_engine_hold(zone, tp);
            }
        } else if let Some(label) = judge_label_for_zone(app, zone) {
            app.push_judgement(zone, label, 0.6);
            app.push_hit_fx(zone, label, false);
        }
    }

    if zones.is_empty() {
        app.active_pointer_zones.remove(&pointer_id);
    } else {
        app.active_pointer_zones.insert(pointer_id, zones.to_vec());
    }
}

/// Zones under (or within the range-trigger radius of) `pos`. With the range
/// trigger off this is at most one zone; on, it is every zone the pointer's
/// circle touches. Applies to touch and mouse alike.
fn zones_at(app: &PadPreviewState, pad: &PadGeom, pos: Vec2) -> Vec<PadZone> {
    let Some(svg) = app.pad_svg.as_ref() else {
        return Vec::new();
    };
    let range = crate::app::params::sensor_range_radius();
    if range > 0.0 {
        svg.hit_test_range(pos, pad, range)
    } else {
        svg.hit_test(pos, pad).into_iter().collect()
    }
}

/// Route touch/mouse events to zones.
///
/// Returns `true` if any event landed on a pad zone (so callers can give the
/// pad priority over overlapping UI while paused).
pub fn handle_touch_controls(
    app: &mut PadPreviewState,
    pad: PadGeom,
    pointer_events: &[PointerEvent],
) -> bool {
    let mut hit_zone = false;
    for ev in pointer_events {
        match ev.phase {
            TouchPhase::Started => {
                app.prev_pointer_pos.insert(ev.id, ev.position);
                let zones = zones_at(app, &pad, ev.position);
                hit_zone |= !zones.is_empty();
                update_pointer_zones(app, ev.id, &zones);
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                let prev = app.prev_pointer_pos.get(&ev.id).copied();
                app.prev_pointer_pos.insert(ev.id, ev.position);
                for sample in sampled_motion_path(prev, ev.position) {
                    let zones = zones_at(app, &pad, sample);
                    hit_zone |= !zones.is_empty();
                    update_pointer_zones(app, ev.id, &zones);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                app.prev_pointer_pos.remove(&ev.id);
                update_pointer_zones(app, ev.id, &[]);
            }
        }
    }
    hit_zone
}
