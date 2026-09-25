//! Autoplay driven exclusively by lnmai-core's default tactic.

use crate::app::types::Mode;
use crate::player::state::PadPreviewState;

pub fn rebuild(pad: &mut PadPreviewState) {
    pad.autoplay_tactic_cursor = 0;
}

pub fn set_on(pad: &mut PadPreviewState, on: bool) {
    if pad.autoplay == on {
        return;
    }
    pad.autoplay = on;
    if !on {
        release_touches(pad);
    }
    let now = (pad.song_time().max(0.0) * 1e6) as i64;
    pad.autoplay_tactic_cursor = pad
        .autoplay_tactic
        .iter()
        .position(|event| crate::player::engine::timed_input_tp(event) >= now)
        .unwrap_or(pad.autoplay_tactic.len());
    pad.set_status(if on { "Autoplay: ON" } else { "Autoplay: OFF" }.to_string());
}

pub fn tick(pad: &mut PadPreviewState) {
    if !pad.autoplay || pad.mode != Mode::Playing || pad.playback_pending || !pad.has_engine() {
        if pad.mode != Mode::Playing || pad.playback_pending {
            release_touches(pad);
        }
        return;
    }

    let now = (pad.song_time().max(0.0) * 1e6) as i64;
    if pad.autoplay_tactic_cursor > 0
        && pad
            .autoplay_tactic
            .get(pad.autoplay_tactic_cursor - 1)
            .is_some_and(|event| crate::player::engine::timed_input_tp(event) > now + 20_000)
    {
        pad.autoplay_tactic_cursor = pad
            .autoplay_tactic
            .iter()
            .position(|event| crate::player::engine::timed_input_tp(event) >= now)
            .unwrap_or(pad.autoplay_tactic.len());
        release_touches(pad);
    }

    while let Some(event) = pad.autoplay_tactic.get(pad.autoplay_tactic_cursor).cloned() {
        if crate::player::engine::timed_input_tp(&event) > now {
            break;
        }
        mirror_visual(pad, &event);
        pad.engine_events.push(event);
        pad.autoplay_tactic_cursor += 1;
    }
}

fn mirror_visual(pad: &mut PadPreviewState, event: &lnmai_core::types::TimedInputEvent) {
    use crate::player::engine::{zone_for_button, zone_for_sensor};
    use lnmai_core::types::TimedInputEvent;

    let (zone, is_down, is_click) = match event {
        TimedInputEvent::ButtonClick { zone, .. } => (zone_for_button(*zone), true, true),
        TimedInputEvent::SensorClick { area, .. } => (zone_for_sensor(*area), true, true),
        TimedInputEvent::ButtonHold { zone, is_down, .. } => {
            (zone_for_button(*zone), *is_down, false)
        }
        TimedInputEvent::SensorHold { area, is_down, .. } => {
            (zone_for_sensor(*area), *is_down, false)
        }
    };

    if is_click {
        pad.push_feedback(zone, 0.12);
        return;
    }
    let key = u64::from(zone.to_id()) | (1u64 << 40);
    if is_down {
        pad.active_pointer_zones.insert(key, zone);
        pad.push_feedback(zone, 0.12);
    } else {
        pad.active_pointer_zones.remove(&key);
    }
}

fn release_touches(pad: &mut PadPreviewState) {
    let ids: Vec<u64> = pad
        .active_pointer_zones
        .keys()
        .copied()
        .filter(|id| (id & (1u64 << 40)) != 0)
        .collect();
    for id in ids {
        pad.active_pointer_zones.remove(&id);
    }
}
