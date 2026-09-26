//! Autoplay driven exclusively by lnmai-core's default tactic.

use crate::app::types::Mode;
use crate::player::state::PadPreviewState;
use lnmai_core::types::{SensorArea, TimedInputEvent};

pub fn rebuild(pad: &mut PadPreviewState) {
    pad.autoplay_tactic_cursor = 0;
    pad.autoplay_click_held.clear();
}

pub fn set_on(pad: &mut PadPreviewState, on: bool) {
    if pad.autoplay == on {
        return;
    }
    pad.autoplay = on;
    if !on {
        flush_click_holds(pad);
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
            flush_click_holds(pad);
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
        flush_click_holds(pad);
        release_touches(pad);
    }

    let mut due = Vec::new();
    while let Some(event) = pad.autoplay_tactic.get(pad.autoplay_tactic_cursor).cloned() {
        if crate::player::engine::timed_input_tp(&event) > now {
            break;
        }
        due.push(event);
        pad.autoplay_tactic_cursor += 1;
    }
    for event in preprocess_tactic_frame(&mut pad.autoplay_click_held, now, due) {
        mirror_visual(pad, &event);
        pad.engine_events.push(event);
    }
}

fn flush_click_holds(pad: &mut PadPreviewState) {
    let now = (pad.song_time().max(0.0) * 1e6) as i64;
    for event in release_click_holds(&mut pad.autoplay_click_held, now) {
        mirror_visual(pad, &event);
        pad.engine_events.push(event);
    }
}

fn release_click_holds(held: &mut Vec<SensorArea>, now: i64) -> Vec<TimedInputEvent> {
    held.drain(..)
        .map(|area| TimedInputEvent::SensorHold {
            tp: now,
            area,
            is_down: false,
        })
        .collect()
}

fn preprocess_tactic_frame(
    held: &mut Vec<SensorArea>,
    now: i64,
    due: impl IntoIterator<Item = TimedInputEvent>,
) -> Vec<TimedInputEvent> {
    let due: Vec<_> = due.into_iter().collect();
    let release_time = due
        .iter()
        .map(crate::player::engine::timed_input_tp)
        .min()
        .unwrap_or(now)
        .min(now);
    let mut events = release_click_holds(held, release_time);
    for event in due {
        let event = crate::player::engine::normalize_tactic_event(event);
        match event {
            TimedInputEvent::SensorClick { tp, area } => {
                events.push(TimedInputEvent::SensorClick { tp, area });
                events.push(TimedInputEvent::SensorHold {
                    tp,
                    area,
                    is_down: true,
                });
                if !held.contains(&area) {
                    held.push(area);
                }
            }
            TimedInputEvent::SensorHold {
                area,
                is_down: true,
                ..
            } => {
                held.retain(|pending| *pending != area);
                events.push(event);
            }
            TimedInputEvent::SensorHold {
                area,
                is_down: false,
                ..
            } if held.contains(&area) => {}
            _ => events.push(event),
        }
    }
    events
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

#[cfg(test)]
mod tests {
    use super::*;
    use lnmai_core::types::ButtonZone;

    #[test]
    fn button_click_holds_a_sensor_for_one_frame() {
        let mut held = Vec::new();
        let first = preprocess_tactic_frame(
            &mut held,
            100,
            [TimedInputEvent::ButtonClick {
                tp: 100,
                zone: ButtonZone::K3,
            }],
        );
        assert_eq!(
            first,
            vec![
                TimedInputEvent::SensorClick {
                    tp: 100,
                    area: SensorArea::A3,
                },
                TimedInputEvent::SensorHold {
                    tp: 100,
                    area: SensorArea::A3,
                    is_down: true,
                },
            ]
        );
        assert_eq!(held, vec![SensorArea::A3]);
        assert_eq!(
            preprocess_tactic_frame(&mut held, 120, []),
            vec![TimedInputEvent::SensorHold {
                tp: 120,
                area: SensorArea::A3,
                is_down: false,
            }]
        );
        assert!(held.is_empty());
    }

    #[test]
    fn sensor_clicks_hold_independently_and_defer_same_frame_release() {
        let mut held = Vec::new();
        let first = preprocess_tactic_frame(
            &mut held,
            100,
            [
                TimedInputEvent::SensorClick {
                    tp: 100,
                    area: SensorArea::B2,
                },
                TimedInputEvent::SensorHold {
                    tp: 100,
                    area: SensorArea::B2,
                    is_down: false,
                },
                TimedInputEvent::SensorClick {
                    tp: 100,
                    area: SensorArea::C,
                },
            ],
        );
        assert_eq!(first.len(), 4);
        assert_eq!(held, vec![SensorArea::B2, SensorArea::C]);
        let next = preprocess_tactic_frame(&mut held, 120, []);
        assert_eq!(
            next,
            vec![
                TimedInputEvent::SensorHold {
                    tp: 120,
                    area: SensorArea::B2,
                    is_down: false,
                },
                TimedInputEvent::SensorHold {
                    tp: 120,
                    area: SensorArea::C,
                    is_down: false,
                },
            ]
        );
    }

    #[test]
    fn explicit_hold_continues_after_click_frame() {
        let mut held = Vec::new();
        let first = preprocess_tactic_frame(
            &mut held,
            100,
            [
                TimedInputEvent::SensorClick {
                    tp: 100,
                    area: SensorArea::A1,
                },
                TimedInputEvent::SensorHold {
                    tp: 100,
                    area: SensorArea::A1,
                    is_down: true,
                },
            ],
        );
        assert_eq!(first.len(), 3);
        assert!(held.is_empty());
        assert!(preprocess_tactic_frame(&mut held, 120, []).is_empty());
    }

    #[test]
    fn previous_hold_releases_before_next_click_in_a_delayed_frame() {
        let mut held = Vec::new();
        preprocess_tactic_frame(
            &mut held,
            100,
            [TimedInputEvent::SensorClick {
                tp: 100,
                area: SensorArea::A1,
            }],
        );
        let next = preprocess_tactic_frame(
            &mut held,
            150,
            [TimedInputEvent::SensorClick {
                tp: 140,
                area: SensorArea::A1,
            }],
        );
        assert_eq!(
            next,
            vec![
                TimedInputEvent::SensorHold {
                    tp: 140,
                    area: SensorArea::A1,
                    is_down: false,
                },
                TimedInputEvent::SensorClick {
                    tp: 140,
                    area: SensorArea::A1,
                },
                TimedInputEvent::SensorHold {
                    tp: 140,
                    area: SensorArea::A1,
                    is_down: true,
                },
            ]
        );
        assert_eq!(held, vec![SensorArea::A1]);
    }
}
