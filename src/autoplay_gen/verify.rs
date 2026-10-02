//! Drive lnmai-core with a generated plan and report whether every slide arc
//! is judged non-Miss.
//!
//! Minimal: one sensor hold per judge segment (press on the first, move on the
//! rest), replayed frame-by-frame through the same stepping helper the player
//! uses. It is a feasibility check, not the final event generator.

use std::collections::BTreeMap;

use crate::app::types::zone::PadZone;
use crate::core::types::{JudgeEventKind, JudgeGrade, TimedInputEvent};
use crate::model::SlidePlan;
use crate::player::engine::{
    self, JudgeEngine, hold_events_for_zone, press_events_for_zone, release_events_for_zone,
    timed_input_tp,
};

/// Outcome of replaying a plan through the core.
pub struct VerifyResult {
    pub arcs: usize,
    /// Runtime arcs that produced a Slide judge event.
    pub judged: usize,
    /// Runtime arcs judged Miss / TooFast.
    pub misses: Vec<usize>,
    /// Runtime arcs with no judge event. Non-final arcs of a chain are hidden
    /// without a judge event, so this is informational, not a failure.
    pub unjudged: Vec<usize>,
    pub events: Vec<TimedInputEvent>,
}

impl VerifyResult {
    /// Every arc that produced a judge was non-Miss (and at least one did).
    pub fn all_perfect(&self) -> bool {
        self.judged > 0 && self.misses.is_empty()
    }
}

/// Build a naive sensor-event stream from the plan: press the first segment's
/// zone, move (release old + hold new) for each following segment, then release.
pub fn build_events(plans: &[SlidePlan], tail_pad_s: f64) -> Vec<TimedInputEvent> {
    let mut events = Vec::new();
    for slide in plans {
        let mut prev: Option<PadZone> = None;
        let mut last_tp = 0_i64;
        for (i, seg) in slide.segments.iter().enumerate() {
            let Some(&zone_id) = seg.zones.first() else {
                continue;
            };
            let zone = PadZone::from(zone_id);
            let tp = (seg.target_s * 1e6).round() as i64;
            if i == 0 {
                events.extend(press_events_for_zone(zone, tp));
            } else {
                if let Some(p) = prev {
                    events.extend(release_events_for_zone(p, tp));
                }
                events.extend(hold_events_for_zone(zone, tp));
            }
            prev = Some(zone);
            last_tp = tp;
        }
        if let Some(p) = prev {
            events.extend(release_events_for_zone(
                p,
                last_tp + (tail_pad_s * 1e6).round() as i64,
            ));
        }
    }
    events.sort_by_key(timed_input_tp);
    events
}

/// Replay `events` at 60 fps and collect the slide judge per runtime arc.
/// `end_s` should cover the chart's last slide (plus a margin).
pub fn verify(text: &str, level: u32, events: &[TimedInputEvent], end_s: f32) -> VerifyResult {
    let mut engine = JudgeEngine::load(text, level).expect("engine");
    let arcs = engine.slide_count();
    let mut grades: BTreeMap<usize, JudgeGrade> = BTreeMap::new();

    let end_s = end_s.max(events.iter().map(timed_input_tp).max().unwrap_or(0) as f32 / 1e6) + 2.0;
    let mut cursor = 0usize;
    let mut t = 0.0_f32;
    while t < end_s {
        t += 1.0 / 60.0;
        let now = (t * 1e6) as i64;
        let mut due = Vec::new();
        while cursor < events.len() && timed_input_tp(&events[cursor]) <= now {
            due.push(events[cursor].clone());
            cursor += 1;
        }
        for result in engine::step_engine_events(&mut engine, t, due) {
            let result = result.expect("step");
            for event in result.events {
                if event.kind == JudgeEventKind::Slide {
                    if let Some(rt) = engine.runtime_slide_index(event.note_index) {
                        grades.entry(rt).or_insert(event.grade);
                    }
                }
            }
        }
    }

    let mut misses = Vec::new();
    let mut unjudged = Vec::new();
    for rt in 0..arcs {
        match grades.get(&rt) {
            None => unjudged.push(rt),
            Some(g) if g.is_miss_or_too_fast() => misses.push(rt),
            _ => {}
        }
    }
    VerifyResult {
        arcs,
        judged: grades.len(),
        misses,
        unjudged,
        events: events.to_vec(),
    }
}
