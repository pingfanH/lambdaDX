//! Chart-driven autoplay: the pad presses itself at every note.
//!
//! Shared by both front-ends (the standalone pad preview and the UI player).
//! The schedule is built once per chart, then a cursor is advanced with the song
//! clock. At each event a *synthetic* pointer is inserted into
//! [`PadPreviewState::active_pointer_zones`] so the zone highlights, feedback and
//! slide trails all follow the same path as a real touch. Judged notes are
//! hidden and restored on a backward seek.

use crate::app::types::zone::PadZone;
use crate::app::types::{BpmChange, Note, NoteType, measure_to_secs, note_secs};
use crate::player::input::hit::judge_label_for_zone;
use crate::player::state::PadPreviewState;

/// How long a synthetic tap stays "held" before its release event.
const TAP_RELEASE: f32 = 0.06;
/// Events older than this (skipped by a seek) are consumed without applying.
const STALE_WINDOW: f32 = 0.20;

/// One synthetic press / release / hide marker.
#[derive(Debug, Clone, Copy)]
pub struct AutoplayEvent {
    pub t: f32,
    pub zone: PadZone,
    pub down: bool,
    pub hide: Option<u64>,
}

/// Build the sorted autoplay schedule from a chart.
pub fn events_for(notes: &[Note], bpms: &[BpmChange]) -> Vec<AutoplayEvent> {
    let mut events: Vec<AutoplayEvent> = Vec::with_capacity(notes.len() * 2);
    for note in notes {
        let zone = PadZone::from(note.lane);
        let head = note_secs(note, bpms);
        let (release, hide_at) = match note.note_type {
            NoteType::Hold => {
                let tail = measure_to_secs(note.time + note.hold_duration, bpms);
                (tail, tail)
            }
            NoteType::Slide => {
                let dur = note
                    .slide
                    .iter()
                    .map(|s| s.slide_duration)
                    .fold(0.0_f32, f32::max);
                let end = measure_to_secs(note.time + dur, bpms);
                (end, end)
            }
            _ => (head + TAP_RELEASE, head),
        };
        events.push(AutoplayEvent {
            t: head,
            zone,
            down: true,
            hide: (hide_at <= head + 1e-4).then_some(note.id),
        });
        events.push(AutoplayEvent {
            t: release,
            zone,
            down: false,
            hide: (hide_at > head + 1e-4).then_some(note.id),
        });
    }
    events.sort_by(|a, b| a.t.total_cmp(&b.t));
    events
}

/// Advance the cursor to `t`, returning the new cursor and the due events.
/// A backward seek rewinds instead of dumping a burst of stale events.
pub fn due(
    events: &[AutoplayEvent],
    cursor: usize,
    t: f32,
) -> (usize, Vec<AutoplayEvent>) {
    let mut cursor = cursor;
    if cursor > 0 && events.get(cursor - 1).is_some_and(|e| e.t > t + 0.02) {
        cursor = events.partition_point(|e| e.t < t);
    }
    let mut out = Vec::new();
    while let Some(&ev) = events.get(cursor) {
        if ev.t > t {
            break;
        }
        if t - ev.t <= STALE_WINDOW {
            out.push(ev);
        }
        cursor += 1;
    }
    (cursor, out)
}

/// Synthetic pointer id for a zone (kept clear of real touches and keyboard
/// lane ids by living just below `u64::MAX`).
fn pointer_id(zone: PadZone) -> u64 {
    const AUTOPLAY_ID_BASE: u64 = u64::MAX - 3000;
    AUTOPLAY_ID_BASE - zone.to_id() as u64
}

fn is_pointer_id(id: u64) -> bool {
    id > u64::MAX - 3100 && id < u64::MAX - 3000
}

/// Rebuild the schedule for the current chart.
pub fn rebuild(pad: &mut PadPreviewState) {
    pad.autoplay_events = events_for(&pad.chart.notes, &pad.chart.bpms);
    pad.autoplay_cursor = 0;
    pad.autoplay_hidden.clear();
}

/// Toggle autoplay, resyncing the cursor to the current song time.
pub fn set_on(pad: &mut PadPreviewState, on: bool) {
    if pad.autoplay == on {
        return;
    }
    pad.autoplay = on;
    if !on {
        release_touches(pad);
        clear_hidden(pad);
    }
    let t = pad.song_time();
    pad.autoplay_cursor = pad.autoplay_events.partition_point(|e| e.t < t);
    if on && pad.has_engine() {
        let now = (t.max(0.0) * 1e6) as i64;
        pad.autoplay_tactic_cursor = pad
            .autoplay_tactic
            .iter()
            .position(|e| crate::player::engine::timed_input_tp(e) >= now)
            .unwrap_or(pad.autoplay_tactic.len());
    }
    pad.set_status(if on {
        "Autoplay: ON".to_string()
    } else {
        "Autoplay: OFF".to_string()
    });
}

/// Drive autoplay for this frame.
pub fn tick(pad: &mut PadPreviewState) {
    if !pad.autoplay {
        return;
    }
    if pad.mode != crate::app::types::Mode::Playing || pad.playback_pending {
        release_touches(pad);
        return;
    }
    // Prefer lnmai-core's default replay tactic when an engine is loaded; the
    // local schedule below is only a fallback for engine-less charts.
    if pad.has_engine() && !pad.autoplay_tactic.is_empty() {
        tick_tactic(pad);
        return;
    }
    let t = pad.song_time();
    // Backward seek / restart: rewind instead of dumping a burst.
    if pad.autoplay_cursor > 0
        && pad
            .autoplay_events
            .get(pad.autoplay_cursor - 1)
            .is_some_and(|e| e.t > t + 0.02)
    {
        pad.autoplay_cursor = pad.autoplay_events.partition_point(|e| e.t < t);
        release_touches(pad);
        clear_hidden(pad);
    }
    let (cursor, due) = due(&pad.autoplay_events, pad.autoplay_cursor, t);
    pad.autoplay_cursor = cursor;
    for ev in due {
        apply(pad, ev);
    }
}

/// Feed lnmai-core's default tactic events that are due at the current song
/// time and mirror them onto the pad visuals.
fn tick_tactic(pad: &mut PadPreviewState) {
    use crate::player::engine::timed_input_tp;
    let now = (pad.song_time().max(0.0) * 1e6) as i64;
    // Backward seek / restart: rewind instead of dumping a burst.
    if pad.autoplay_tactic_cursor > 0
        && pad
            .autoplay_tactic
            .get(pad.autoplay_tactic_cursor - 1)
            .is_some_and(|e| timed_input_tp(e) > now + 20_000)
    {
        pad.autoplay_tactic_cursor = pad
            .autoplay_tactic
            .iter()
            .position(|e| timed_input_tp(e) >= now)
            .unwrap_or(pad.autoplay_tactic.len());
        release_touches(pad);
        clear_hidden(pad);
    }

    while let Some(event) = pad.autoplay_tactic.get(pad.autoplay_tactic_cursor).cloned() {
        if timed_input_tp(&event) > now {
            break;
        }
        mirror_tactic_visual(pad, &event);
        pad.engine_events.push(event);
        pad.autoplay_tactic_cursor += 1;
    }
}

/// Light the pad for an autoplay event so the sensor shows the core's input.
fn mirror_tactic_visual(pad: &mut PadPreviewState, event: &lnmai_core::types::TimedInputEvent) {
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
    // Synthetic pointer id keyed by zone so holds stay lit until release.
    let key = u64::from(zone.to_id()) | (1u64 << 40);
    if is_down {
        pad.active_pointer_zones.insert(key, zone);
        pad.push_feedback(zone, 0.12);
    } else {
        pad.active_pointer_zones.remove(&key);
    }
}

fn apply(pad: &mut PadPreviewState, ev: AutoplayEvent) {
    let id = pointer_id(ev.zone);
    if ev.down {
        pad.active_pointer_zones.insert(id, ev.zone);
        pad.push_feedback(ev.zone, 0.12);
        if pad.has_engine() {
            let tp = (ev.t.max(0.0) * 1e6) as i64;
            pad.queue_engine_press(ev.zone, tp);
        } else if let Some(label) = judge_label_for_zone(pad, ev.zone) {
            pad.push_judgement(ev.zone, label, 0.6);
        }
    } else {
        pad.active_pointer_zones.remove(&id);
        if pad.has_engine() {
            let tp = (ev.t.max(0.0) * 1e6) as i64;
            pad.queue_engine_release(ev.zone, tp);
        }
    }
    if let Some(note_id) = ev.hide {
        pad.hidden_notes.insert(note_id);
        pad.autoplay_hidden.push(note_id);
    }
}

fn release_touches(pad: &mut PadPreviewState) {
    let ids: Vec<u64> = pad
        .active_pointer_zones
        .keys()
        .copied()
        .filter(|id| is_pointer_id(*id) || (id & (1u64 << 40)) != 0)
        .collect();
    for id in ids {
        pad.active_pointer_zones.remove(&id);
    }
}

fn clear_hidden(pad: &mut PadPreviewState) {
    for id in pad.autoplay_hidden.drain(..) {
        pad.hidden_notes.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(simai: &str) -> crate::app::types::ChartDoc {
        crate::app::maidata::from_maidata(simai, None).expect("chart")
    }

    #[test]
    fn events_press_and_hide_on_judgment() {
        let c = chart("&title=T\n&inote_1=(120){4}1,2,3,4\n");
        let events = events_for(&c.notes, &c.bpms);
        assert_eq!(events.len(), c.notes.len() * 2);
        let presses = events.iter().filter(|e| e.down).count();
        assert_eq!(presses, c.notes.len());
        // Taps hide at the head.
        assert!(events.iter().filter(|e| e.down).all(|e| e.hide.is_some()));
        assert!(events.windows(2).all(|w| w[0].t <= w[1].t), "sorted");
    }

    #[test]
    fn due_fires_due_events() {
        let c = chart("&title=T\n&inote_1=(120){4}1,2,3,4\n");
        let events = events_for(&c.notes, &c.bpms);
        let (cursor, out) = due(&events, 0, 0.05);
        assert!(cursor >= 1);
        assert!(out.iter().any(|e| e.down), "the head press is due at t≈0");
    }

    #[test]
    fn due_skips_stale_events() {
        let events = vec![
            AutoplayEvent { t: 0.0, zone: PadZone::from(1), down: true, hide: Some(1) },
            AutoplayEvent { t: 5.0, zone: PadZone::from(2), down: true, hide: Some(2) },
        ];
        let (cursor, out) = due(&events, 0, 5.0);
        assert_eq!(cursor, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].zone, PadZone::from(2));
    }

    #[test]
    fn due_resyncs_after_backward_seek() {
        let events = vec![
            AutoplayEvent { t: 0.0, zone: PadZone::from(1), down: true, hide: Some(1) },
            AutoplayEvent { t: 1.0, zone: PadZone::from(2), down: true, hide: Some(2) },
        ];
        let (cursor, out) = due(&events, 2, 0.0);
        assert_eq!(cursor, 1, "rewinds to the event at t=0");
        assert_eq!(out[0].zone, PadZone::from(1));
    }
}
