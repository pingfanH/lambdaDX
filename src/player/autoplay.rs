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
    pad.autoplay_direct_held.clear();
}

/// Restore input ownership without emitting clicks, judgments or sounds.
pub(crate) fn restore_at(pad: &mut PadPreviewState, seconds: f32, events: &[TimedInputEvent]) {
    if !pad.autoplay {
        return;
    }
    let target = (seconds * 1e6) as i64;
    let mut held = Vec::new();
    for event in events {
        let tp = crate::player::engine::timed_input_tp(event);
        if tp >= target {
            break;
        }
        track_direct_hold(&mut held, event);
        if !pad.autoplay_direct_core && target - tp < 120_000 {
            let zone = match event {
                TimedInputEvent::ButtonClick { zone, .. } => {
                    Some(crate::player::engine::zone_for_button(*zone))
                }
                TimedInputEvent::SensorClick { area, .. } => {
                    Some(crate::player::engine::zone_for_sensor(*area))
                }
                _ => None,
            };
            if let Some(zone) = zone {
                pad.push_feedback(zone, (120_000 - (target - tp)) as f64 / 1e6);
            }
        }
    }
    for event in &held {
        if !pad.autoplay_direct_core {
            let zone = match event {
                TimedInputEvent::SensorHold { area, .. } => {
                    if !pad.autoplay_explicit_held.contains(area) {
                        pad.autoplay_explicit_held.push(*area);
                    }
                    crate::player::engine::zone_for_sensor(*area)
                }
                TimedInputEvent::ButtonHold { zone, .. } => {
                    pad.autoplay_direct_held.push(event.clone());
                    crate::player::engine::zone_for_button(*zone)
                }
                _ => continue,
            };
            pad.active_pointer_zones
                .insert(u64::from(zone.to_id()) | (1u64 << 40), vec![zone]);
        }
    }
    if pad.autoplay_direct_core {
        pad.autoplay_direct_held = held;
    }
}

pub fn set_direct_core(pad: &mut PadPreviewState, direct: bool) {
    if pad.autoplay_direct_core == direct {
        return;
    }
    flush_holds(pad);
    release_touches(pad);
    pad.autoplay_direct_core = direct;
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
    let now = (pad.song_time() * 1e6) as i64;
    pad.autoplay_tactic_cursor = pad
        .autoplay_tactic
        .iter()
        .position(|event| crate::player::engine::timed_input_tp(event) >= now)
        .unwrap_or(pad.autoplay_tactic.len());
    pad.set_status(if on { "Autoplay: ON" } else { "Autoplay: OFF" }.to_string());
}

pub fn tick(pad: &mut PadPreviewState) {
    pad.sync_autoplay_settings();
    if !pad.autoplay
        || pad.playback_pending
        || pad.audio_start_pending
        || pad.scrubbing
        || !pad.has_engine()
        || (pad.mode != Mode::Playing && pad.forced_time.is_none())
    {
        return;
    }

    let now = (pad.song_time() * 1e6) as i64;
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
    if pad.autoplay_direct_core {
        for event in due {
            track_direct_hold(&mut pad.autoplay_direct_held, &event);
            pad.engine_events.push(event);
        }
        return;
    }
    // External tactics (from `autoplay_gen`) are already a complete sensor
    // event stream; replay them verbatim rather than re-synthesising holds.
    if pad.external_autoplay || (pad.autoplay_plan.is_some() && !pad.legacy_autoplay) {
        for event in due {
            feed(pad, event);
        }
        return;
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
        feed(pad, event);
    }
}

/// Send an autoplay event to the backend, never during preview scrubs.
fn feed(pad: &mut PadPreviewState, event: TimedInputEvent) {
    if pad.scrubbing {
        return;
    }
    match &event {
        TimedInputEvent::ButtonHold { .. } => {
            track_direct_hold(&mut pad.autoplay_direct_held, &event);
        }
        TimedInputEvent::SensorHold { area, is_down, .. } => {
            pad.autoplay_explicit_held.retain(|held| held != area);
            if *is_down {
                pad.autoplay_explicit_held.push(*area);
            }
        }
        _ => {}
    }
    mirror_visual(pad, &event);
    pad.engine_events.push(event);
}

fn flush_holds(pad: &mut PadPreviewState) {
    let now = (pad.song_time() * 1e6) as i64;
    for event in pad.autoplay_direct_held.drain(..) {
        let release = match event {
            TimedInputEvent::ButtonHold { zone, .. } => TimedInputEvent::ButtonHold {
                tp: now,
                zone,
                is_down: false,
            },
            TimedInputEvent::SensorHold { area, .. } => TimedInputEvent::SensorHold {
                tp: now,
                area,
                is_down: false,
            },
            _ => continue,
        };
        pad.engine_events.push(release);
    }
    for event in release_click_holds(&mut pad.autoplay_click_held, now) {
        feed(pad, event);
    }
    for event in release_click_holds(&mut pad.autoplay_explicit_held, now) {
        feed(pad, event);
    }
}

fn track_direct_hold(held: &mut Vec<TimedInputEvent>, event: &TimedInputEvent) {
    let down = match event {
        TimedInputEvent::ButtonHold { zone, is_down, .. } => {
            held.retain(|e| !matches!(e, TimedInputEvent::ButtonHold { zone: z, .. } if z == zone));
            *is_down
        }
        TimedInputEvent::SensorHold { area, is_down, .. } => {
            held.retain(|e| !matches!(e, TimedInputEvent::SensorHold { area: a, .. } if a == area));
            *is_down
        }
        _ => false,
    };
    if down {
        held.push(event.clone());
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
    let mut events = Vec::new();
    // A synthesized click holds for exactly one frame. Lift the previous
    // frame's areas before this frame's events: gating this on an empty frame
    // leaves the sensor stuck down whenever the tactic fills every frame.
    events.extend(release_click_holds(click_held, now));
    for event in due {
        match event {
            TimedInputEvent::SensorClick { tp, area } => {
                // A dense tactic can revisit the same sensor within one frame.
                // Release only that sensor's synthetic click before retriggering
                // it; other areas may still be active parts of a slide path.
                events.extend(release_click_hold_for_area(click_held, area, tp));
                events.push(TimedInputEvent::SensorClick { tp, area });
                // A tap can overlap a slide body after K1..K8 are mapped to
                // A1..A8.  The explicit hold belongs to the slide and must
                // survive the tap; releasing and pressing it again creates a
                // same-timestamp transition that can consume the tap or
                // interrupt the slide state machine in lnmai-core.
                if !explicit_held.contains(&area) {
                    events.push(TimedInputEvent::SensorHold {
                        tp,
                        area,
                        is_down: true,
                    });
                }
                if !click_held.contains(&area) && !explicit_held.contains(&area) {
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
        pad.active_pointer_zones.insert(key, vec![zone]);
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
    fn scrub_restores_held_inputs_and_recent_clicks_without_replaying_events() {
        let chart = crate::app::types::ChartDoc {
            version: "1".into(),
            title: String::new(),
            artist: String::new(),
            simai_level: 1,
            bpm: 120.0,
            bpms: vec![],
            audio_offset: 0.0,
            notes: vec![],
            templates: vec![],
            template_instances: vec![],
            autoplay_overrides: vec![],
        };
        let events = vec![
            TimedInputEvent::SensorHold {
                tp: 0,
                area: SensorArea::A1,
                is_down: true,
            },
            TimedInputEvent::ButtonClick {
                tp: 950_000,
                zone: ButtonZone::K2,
            },
            TimedInputEvent::SensorHold {
                tp: 2_000_000,
                area: SensorArea::A1,
                is_down: false,
            },
        ];
        for direct in [false, true] {
            let mut pad = PadPreviewState::new_at(chart.clone(), None, None, 0.0, None);
            pad.forced_time = Some(1.0);
            pad.autoplay = true;
            pad.autoplay_direct_core = direct;
            pad.scrubbing = true;
            restore_at(&mut pad, 1.0, &events);
            assert!(pad.engine_events.is_empty());
            assert!(pad.judge_feedback.is_empty());
            assert!(pad.hit_fx.is_empty());
            if direct {
                assert_eq!(pad.autoplay_direct_held, vec![events[0].clone()]);
                assert!(pad.active_pointer_zones.is_empty());
                assert!(pad.pad_feedback.is_empty());
            } else {
                assert_eq!(pad.autoplay_explicit_held.len(), 1);
                assert_eq!(pad.active_pointer_zones.len(), 1);
                assert_eq!(pad.pad_feedback.len(), 1);
                assert!((pad.pad_feedback[0].until - 1.07).abs() < 1e-6);
            }
        }
    }

    #[cfg(feature = "backend-lean")]
    #[test]
    fn direct_core_replays_original_events_without_touch_simulation() {
        let text = "&inote_1=(120){4}1h[4:1],2,E";
        let chart = crate::app::maidata::from_maidata(text, None).unwrap();
        let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
        pad.load_engine(text, 1).unwrap();
        pad.forced_time = Some(0.0);
        pad.mode = Mode::Playing;
        set_on(&mut pad, true);
        set_direct_core(&mut pad, true);
        let expected: Vec<_> = pad
            .autoplay_tactic
            .iter()
            .filter(|e| crate::player::engine::timed_input_tp(e) <= 0)
            .cloned()
            .collect();
        assert!(!expected.is_empty());
        tick(&mut pad);
        assert_eq!(pad.engine_events, expected);
        assert!(pad.active_pointer_zones.is_empty());
        assert!(pad.pad_feedback.is_empty());
        assert!(pad.autoplay_click_held.is_empty());
        pad.engine_events.clear();
        set_on(&mut pad, false);
        assert!(
            !pad.engine_events.is_empty(),
            "held inputs must be released"
        );
        assert!(pad.autoplay_direct_held.is_empty());
    }

    /// The core's own plan is a perfect replay, so the editor must hand it
    /// over without dropping inputs at any frame rate or after a mid-play seek.
    #[cfg(feature = "backend-lean")]
    #[test]
    fn plan_replay_keeps_every_note_at_any_frame_rate_and_after_a_seek() {
        let text =
            "&inote_1=(120){4}1,2,3,4,5,6,7,8,1/2,3/4,1-5[4:1],2-6[4:1],3/4h[4:1],E";
        for direct in [false, true] {
            for fps in [8.0_f32, 15.0, 30.0, 60.0, 144.0] {
                for seek_at in [None, Some(0.75_f32)] {
                let chart = crate::app::maidata::from_maidata(text, None).unwrap();
                let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
                pad.load_engine(text, 1).unwrap();
                pad.mode = Mode::Playing;
                pad.forced_time = Some(0.0);
                set_on(&mut pad, true);
                set_direct_core(&mut pad, direct);
                let duration = pad.song_duration().max(1.0);
                let frames = (duration * fps).ceil() as usize + 2;
                let mut seeked = false;
                for frame in 0..frames {
                    let t = (frame as f32 / fps).min(duration);
                    pad.forced_time = Some(t);
                    if let Some(at) = seek_at.filter(|_| !seeked && t >= seek_at.unwrap()) {
                        seeked = true;
                        pad.try_seek_to(at).unwrap();
                    }
                    tick(&mut pad);
                    crate::player::engine::step_judge_engine(&mut pad);
                }
                let score = pad.core_score.as_ref().unwrap();
                assert_eq!(
                    score.lost_base, 0,
                    "direct={direct} fps={fps} seek={seek_at:?}: lost {}/{} base notes",
                    score.lost_base, score.total_base
                );
                }
            }
        }
    }

    /// A dense tactic has events in every frame; a synthesized click still has
    /// to lift on the next frame or the sensor stays pressed and the next note
    /// on that area is missed.
    #[test]
    fn click_hold_lifts_on_the_next_frame_even_when_it_carries_events() {
        let mut click_held = Vec::new();
        let mut explicit_held = Vec::new();
        let first = preprocess_tactic_frame_with_holds(
            &mut click_held,
            &mut explicit_held,
            100,
            [TimedInputEvent::SensorClick {
                tp: 100,
                area: SensorArea::A1,
            }],
        );
        assert!(first.iter().any(|event| matches!(
            event,
            TimedInputEvent::SensorHold {
                area: SensorArea::A1,
                is_down: true,
                ..
            }
        )));
        let next = preprocess_tactic_frame_with_holds(
            &mut click_held,
            &mut explicit_held,
            120,
            [TimedInputEvent::SensorClick {
                tp: 120,
                area: SensorArea::A2,
            }],
        );
        assert!(
            next.iter().any(|event| matches!(
                event,
                TimedInputEvent::SensorHold {
                    area: SensorArea::A1,
                    is_down: false,
                    ..
                }
            )),
            "A1 must not stay stuck down: {next:?}"
        );
        assert_eq!(click_held, vec![SensorArea::A2]);
    }

    /// The same bug end to end: a stream fast enough to fill every frame in
    /// touch mode used to leave sensors held, which the core judges as misses.
    #[cfg(feature = "backend-lean")]
    #[test]
    fn plan_replay_matrix_stays_perfect() {
        use crate::core::types::JudgeGrade;
        let charts = [
            ("taps", "1,2,3,4,5,6,7,8,1/2,3/4"),
            ("dense", "{16}1,2,1,2,1,2,1,2,1,2,1,2,1,2,1,2"),
            ("holds", "1h[4:1],2h[4:1],3/4h[4:1]"),
            ("slides", "1-5[4:1],2-6[4:1],3-7[4:1],4-8[4:1]"),
            ("conslide", "1-5-8[4:2]*-7[4:1],8-4-1[4:2]*-2[4:1]"),
            ("mixed", "1,1-5[4:1],2h[4:1],3/4,1-5-8[4:2]*-7[4:1],5,{32}1,2,3,4"),
        ];
        let mut failures = Vec::new();
        for (name, body) in charts {
            let text = format!("&inote_1=(150){{4}}{body},E");
            for direct in [false, true] {
                for fps in [15.0_f32, 20.0, 30.0, 45.0, 60.0, 144.0] {
                    let chart = crate::app::maidata::from_maidata(&text, None).unwrap();
                    let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
                    pad.load_engine(&text, 1).unwrap();
                    pad.mode = Mode::Playing;
                    pad.forced_time = Some(0.0);
                    set_on(&mut pad, true);
                    set_direct_core(&mut pad, direct);
                    let duration = pad.song_duration().max(1.0);
                    let step = 1.0 / fps;
                    let mut t = 0.0_f32;
                    let mut index = 0usize;
                    while t < duration {
                        // A hitch every seventh frame keeps the timing irregular.
                        let frame = if index % 7 == 3 { step * 3.0 } else { step };
                        pad.forced_time = Some(t);
                        tick(&mut pad);
                        crate::player::engine::step_judge_engine(&mut pad);
                        t += frame;
                        index += 1;
                    }
                    pad.forced_time = Some(duration);
                    tick(&mut pad);
                    crate::player::engine::step_judge_engine(&mut pad);
                    let score = pad.core_score.as_ref().unwrap();
                    let misses = score.counts.grade_count(JudgeGrade::Miss);
                    let lost_extra = score.lost_extra + score.lost_classic_extra;
                    if score.lost_base != 0 || lost_extra != 0 || misses != 0 {
                        failures.push(format!(
                            "{name} direct={direct} fps={fps}: lost={}/{}+{} misses={misses}",
                            score.lost_base, score.total_base, lost_extra
                        ));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[cfg(feature = "backend-lean")]
    #[test]
    fn dense_tap_stream_survives_touch_simulation() {
        let text = format!("&inote_1=(400){{16}}{}E", "1,2,1,2,".repeat(16));
        let chart = crate::app::maidata::from_maidata(&text, None).unwrap();
        let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
        pad.load_engine(&text, 1).unwrap();
        pad.mode = Mode::Playing;
        pad.forced_time = Some(0.0);
        set_on(&mut pad, true);
        set_direct_core(&mut pad, false);
        let fps = 30.0_f32;
        let duration = pad.song_duration().max(1.0);
        for frame in 0..((duration * fps).ceil() as usize + 2) {
            pad.forced_time = Some((frame as f32 / fps).min(duration));
            tick(&mut pad);
            crate::player::engine::step_judge_engine(&mut pad);
        }
        let score = pad.core_score.as_ref().unwrap();
        assert_eq!(
            score.lost_base, 0,
            "touch simulation lost {}/{} base notes",
            score.lost_base, score.total_base
        );
    }

    #[test]
    fn sensor_click_holds_a_sensor_for_one_frame() {
        let mut held = Vec::new();
        let first = preprocess_tactic_frame(
            &mut held,
            100,
            [TimedInputEvent::SensorClick {
                tp: 100,
                area: SensorArea::A3,
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
        assert!(
            preprocess_tactic_frame_with_holds(&mut click_held, &mut explicit_held, 120, [],)
                .is_empty()
        );
    }

    #[test]
    fn click_on_an_explicitly_held_sensor_does_not_release_the_slide_hold() {
        let mut click_held = Vec::new();
        let mut explicit_held = Vec::new();
        let prepared = preprocess_tactic_frame_with_holds(
            &mut click_held,
            &mut explicit_held,
            392_947_60,
            [
                TimedInputEvent::SensorHold {
                    tp: 392_857_15,
                    area: SensorArea::A8,
                    is_down: true,
                },
                TimedInputEvent::SensorClick {
                    tp: 392_857_17,
                    area: SensorArea::A8,
                },
                TimedInputEvent::SensorHold {
                    tp: 392_857_17,
                    area: SensorArea::A8,
                    is_down: true,
                },
            ],
        );

        assert_eq!(
            prepared,
            vec![
                TimedInputEvent::SensorHold {
                    tp: 392_857_15,
                    area: SensorArea::A8,
                    is_down: true,
                },
                TimedInputEvent::SensorClick {
                    tp: 392_857_17,
                    area: SensorArea::A8,
                },
            ],
            "a tap sharing a slide sensor must not synthesize a release/re-press",
        );
        assert!(click_held.is_empty());
        assert_eq!(explicit_held, vec![SensorArea::A8]);
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
                // The lift belongs to the frame it happens in, not to the new
                // click's timestamp; the core sorts by time either way.
                TimedInputEvent::SensorHold {
                    tp: 150,
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
