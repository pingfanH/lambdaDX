//! Drive lnmai-core with a generated plan and report whether every slide arc
//! is judged non-Miss.
//!
//! Minimal: one sensor hold per judge segment (press on the first, move on the
//! rest), replayed frame-by-frame through the same stepping helper the player
//! uses. It is a feasibility check, not the final event generator.

use std::collections::BTreeMap;

use crate::app::types::zone::PadZone;
use crate::core::types::{ChartSpec, JudgeEventKind, JudgeGrade, OuterSlot, SensorArea, TimedInputEvent};
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
    /// Runtime arcs judged Great / Good (non-Perfect, non-Miss).
    pub imperfect: Vec<usize>,
    /// Runtime arcs with no judge event. Non-final arcs of a chain are hidden
    /// without a judge event, so this is informational, not a failure.
    pub unjudged: Vec<usize>,
    pub events: Vec<TimedInputEvent>,
}

impl VerifyResult {
    /// Every judged arc was a Perfect-grade (all-Perfect, not merely non-Miss).
    pub fn all_perfect(&self) -> bool {
        self.judged > 0 && self.misses.is_empty() && self.imperfect.is_empty()
    }

    /// Arcs that need fixing: Miss/TooFast or a non-Perfect grade.
    pub fn bad(&self) -> usize {
        self.misses.len() + self.imperfect.len()
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

fn slot_area(slot: OuterSlot) -> SensorArea {
    match slot {
        OuterSlot::S1 => SensorArea::A1,
        OuterSlot::S2 => SensorArea::A2,
        OuterSlot::S3 => SensorArea::A3,
        OuterSlot::S4 => SensorArea::A4,
        OuterSlot::S5 => SensorArea::A5,
        OuterSlot::S6 => SensorArea::A6,
        OuterSlot::S7 => SensorArea::A7,
        OuterSlot::S8 => SensorArea::A8,
    }
}

/// Build events from the core's **lowered chart**: each runtime arc holds every
/// sensor area of its judge queue in order, spread across the arc's span by
/// arrow progress. `offsets[i]` shifts the i-th runtime arc (the enumeration's
/// knob); slide heads stay at their chart time.
pub fn build_events_with_offsets(spec: &ChartSpec, offsets: &[i64]) -> Vec<TimedInputEvent> {
    let mut events = Vec::new();

    for head in &spec.slide_heads {
        let area = slot_area(head.slot);
        let tp = head.timing;
        events.push(TimedInputEvent::SensorClick { tp, area });
        events.push(TimedInputEvent::SensorHold {
            tp,
            area,
            is_down: true,
        });
        events.push(TimedInputEvent::SensorHold {
            tp: tp + 15_000,
            area,
            is_down: false,
        });
    }

    for (i, slide) in spec.slides.iter().enumerate() {
        let offset_us = offsets.get(i).copied().unwrap_or(0);
        let len = slide.length.max(1);
        let start = slide.start_timing + offset_us;
        let end = start + len;
        for track in &slide.judge_queues {
            let max_fin = track
                .iter()
                .map(|a| a.arrow_progress_when_finished)
                .max()
                .unwrap_or(1)
                .max(1);
            let mut prev: Option<SensorArea> = None;
            for area_spec in track {
                let Some(&area) = area_spec.target_areas.first() else {
                    continue;
                };
                let t = start + len * area_spec.arrow_progress_when_finished as i64 / max_fin as i64;
                if let Some(p) = prev {
                    events.push(TimedInputEvent::SensorHold {
                        tp: t,
                        area: p,
                        is_down: false,
                    });
                }
                events.push(TimedInputEvent::SensorHold {
                    tp: t,
                    area,
                    is_down: true,
                });
                prev = Some(area);
            }
            if let Some(p) = prev {
                events.push(TimedInputEvent::SensorHold {
                    tp: end,
                    area: p,
                    is_down: false,
                });
            }
        }
    }

    events.sort_by_key(timed_input_tp);
    events
}

/// [`build_events_with_offsets`] with every arc at its chart time.
pub fn build_events_from_spec(spec: &ChartSpec) -> Vec<TimedInputEvent> {
    build_events_with_offsets(spec, &[])
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
    let mut imperfect = Vec::new();
    let mut unjudged = Vec::new();
    for rt in 0..arcs {
        match grades.get(&rt) {
            None => unjudged.push(rt),
            Some(g) if g.is_miss_or_too_fast() => misses.push(rt),
            Some(g) if !is_perfect_grade(*g) => imperfect.push(rt),
            _ => {}
        }
    }
    VerifyResult {
        arcs,
        judged: grades.len(),
        misses,
        imperfect,
        unjudged,
        events: events.to_vec(),
    }
}

/// Perfect family (CPerfect / Perfect / 2nd / 3rd) — neither great, good nor
/// miss/too-fast.
fn is_perfect_grade(g: JudgeGrade) -> bool {
    !g.is_miss_or_too_fast() && !g.is_great_grade() && !g.is_good_grade()
}
