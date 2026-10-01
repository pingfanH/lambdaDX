//! Autoplay driven exclusively by lnmai-core's default tactic.

use crate::app::types::Mode;
use crate::core::types::{SensorArea, TimedInputEvent};
use crate::player::state::PadPreviewState;

fn debug_touchzone() -> bool {
    std::env::var_os("MAI2_DEBUG_TOUCHZONE").is_some()
}

pub fn rebuild(pad: &mut PadPreviewState) {
    pad.autoplay_tactic_cursor = 0;
    pad.autoplay_click_held.clear();
    pad.autoplay_explicit_held.clear();
}

pub fn set_on(pad: &mut PadPreviewState, on: bool) {
    if pad.autoplay == on {
        return;
    }
    pad.autoplay = on;
    if !on {
        flush_holds(pad);
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
    if !pad.autoplay || pad.playback_pending || !pad.has_engine() {
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
        flush_holds(pad);
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
    if debug_touchzone() && !due.is_empty() {
        eprintln!("[touchzone/autoplay] now={now} due={due:?}");
    }
    let prepared = preprocess_tactic_frame_with_holds(
        &mut pad.autoplay_click_held,
        &mut pad.autoplay_explicit_held,
        now,
        due,
    );
    if debug_touchzone() && !prepared.is_empty() {
        eprintln!("[touchzone/prepared] now={now} events={prepared:?}");
    }
    for event in prepared {
        mirror_visual(pad, &event);
        pad.engine_events.push(event);
    }
}

fn flush_holds(pad: &mut PadPreviewState) {
    let now = (pad.song_time().max(0.0) * 1e6) as i64;
    for event in release_click_holds(&mut pad.autoplay_click_held, now) {
        mirror_visual(pad, &event);
        pad.engine_events.push(event);
    }
    for event in release_click_holds(&mut pad.autoplay_explicit_held, now) {
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
    let mut explicit_held = Vec::new();
    preprocess_tactic_frame_with_holds(held, &mut explicit_held, now, due)
}

fn preprocess_tactic_frame_with_holds(
    click_held: &mut Vec<SensorArea>,
    explicit_held: &mut Vec<SensorArea>,
    now: i64,
    due: impl IntoIterator<Item = TimedInputEvent>,
) -> Vec<TimedInputEvent> {
    let due: Vec<_> = due.into_iter().collect();
    let has_due = !due.is_empty();
    let mut events = Vec::new();
    for event in due {
        let event = crate::player::engine::normalize_tactic_event(event);
        match event {
            TimedInputEvent::SensorClick { tp, area } => {
                // A dense tactic can revisit the same sensor within one frame.
                // Release only that sensor's synthetic click before retriggering
                // it; other areas may still be active parts of a slide path.
                events.extend(release_click_hold_for_area(click_held, area, tp));
                events.extend(release_click_hold_for_area(explicit_held, area, tp));
                events.push(TimedInputEvent::SensorClick { tp, area });
                events.push(TimedInputEvent::SensorHold {
                    tp,
                    area,
                    is_down: true,
                });
                if !click_held.contains(&area) {
                    click_held.push(area);
                }
            }
            TimedInputEvent::SensorHold {
                tp,
                area,
                is_down: true,
            } => {
                // An explicit hold replaces the one-frame hold synthesized for
                // a preceding click on the same sensor.
                click_held.retain(|pending| *pending != area);
                if explicit_held.contains(&area) {
                    // The real tactic can repeat hold-down for the same area
                    // while a fast slide is being sampled. Core expects one
                    // transition, not duplicate down events.
                    continue;
                }
                // ButtonClick tactics include an immediate ButtonHold(true).
                // The click above already emitted this transition.
                if events.iter().any(|previous| {
                    matches!(previous,
                        TimedInputEvent::SensorHold {
                            tp: previous_tp,
                            area: previous_area,
                            is_down: true,
                        } if *previous_tp == tp && *previous_area == area
                    )
                }) {
                    explicit_held.push(area);
                    continue;
                }
                explicit_held.push(area);
                events.push(event);
            }
            TimedInputEvent::SensorHold {
                tp,
                area,
                is_down: false,
                ..
            } if click_held.contains(&area) || explicit_held.contains(&area) => {
                events.extend(release_click_hold_for_area(click_held, area, tp));
                events.extend(release_click_hold_for_area(explicit_held, area, tp));
            }
            _ => events.push(event),
        }
    }
    if !has_due {
        events.extend(release_click_holds(click_held, now));
    }
    events
}

fn release_click_hold_for_area(
    held: &mut Vec<SensorArea>,
    area: SensorArea,
    tp: i64,
) -> Vec<TimedInputEvent> {
    if let Some(index) = held.iter().position(|held_area| *held_area == area) {
        held.remove(index);
        vec![TimedInputEvent::SensorHold {
            tp,
            area,
            is_down: false,
        }]
    } else {
        Vec::new()
    }
}

fn mirror_visual(pad: &mut PadPreviewState, event: &crate::core::types::TimedInputEvent) {
    use crate::core::types::TimedInputEvent;
    use crate::player::engine::{zone_for_button, zone_for_sensor};

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
    use crate::core::types::ButtonZone;

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
    fn sensor_clicks_release_in_order_within_dense_frame() {
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
        assert_eq!(
            first,
            vec![
                TimedInputEvent::SensorClick {
                    tp: 100,
                    area: SensorArea::B2,
                },
                TimedInputEvent::SensorHold {
                    tp: 100,
                    area: SensorArea::B2,
                    is_down: true,
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
                TimedInputEvent::SensorHold {
                    tp: 100,
                    area: SensorArea::C,
                    is_down: true,
                },
            ]
        );
        assert_eq!(held, vec![SensorArea::C]);
        let next = preprocess_tactic_frame(&mut held, 120, []);
        assert_eq!(
            next,
            vec![TimedInputEvent::SensorHold {
                tp: 120,
                area: SensorArea::C,
                is_down: false,
            }]
        );
    }

    #[test]
    fn explicit_hold_continues_after_click_frame() {
        let mut click_held = Vec::new();
        let mut explicit_held = Vec::new();
        let first = preprocess_tactic_frame_with_holds(
            &mut click_held,
            &mut explicit_held,
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
        assert_eq!(first.len(), 2);
        assert!(click_held.is_empty());
        assert_eq!(explicit_held, vec![SensorArea::A1]);
        assert!(preprocess_tactic_frame_with_holds(
            &mut click_held,
            &mut explicit_held,
            120,
            [],
        )
        .is_empty());
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
