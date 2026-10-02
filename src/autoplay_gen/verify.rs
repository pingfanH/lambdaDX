//! Drive lnmai-core with a generated plan and report whether every slide arc
//! is judged non-Miss.
//!
//! Minimal: one sensor hold per judge segment (press on the first, move on the
//! rest), replayed frame-by-frame through the same stepping helper the player
//! uses. It is a feasibility check, not the final event generator.

use std::collections::{BTreeMap, HashMap};

use crate::app::types::zone::PadZone;
use crate::core::types::{
    ChartSpec, JudgeEventKind, JudgeGrade, OuterSlot, SensorArea, SlideChartNote,
    SlideHeadChartNote, TimedInputEvent,
};
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
    /// Final grade per judged runtime arc.
    pub grades: BTreeMap<usize, JudgeGrade>,
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

/// Maximum slide tracks (branches) knobs are stored for. Wifi slides have 3
/// (left / center / right).
pub const MAX_TRACKS: usize = 3;

/// Per-arc timing knobs for the enumeration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArcTiming {
    /// Shift the arc's start (and its head) by this many microseconds.
    pub offset_us: i64,
    /// Rush every area except the last, then land the **last** area at the
    /// judgment time (`judge_at`). This keeps the star off neighbouring slides
    /// while still completing the arc in its Perfect window.
    pub fast: bool,
    /// Extra shift for each **branch** of a multi-track (wifi) slide, so the
    /// three branches can be enumerated as three separate stars.
    pub track_offset_us: [i64; MAX_TRACKS],
    /// Emit no events for this arc at all ("完全不滑"). Useful when a
    /// neighbouring star's gesture already completes it.
    pub skip: bool,
    /// Emit no events for a specific **branch**. A wifi branch can be left to
    /// be completed by an overlapping star, so only the remaining branch(es)
    /// need our input.
    pub track_skip: [bool; MAX_TRACKS],
}

impl ArcTiming {
    /// True when every knob is at its default.
    pub fn is_default(&self) -> bool {
        *self
            == ArcTiming {
                offset_us: 0,
                fast: false,
                track_offset_us: [0; MAX_TRACKS],
                skip: false,
                track_skip: [false; MAX_TRACKS],
            }
    }
}

/// Gap between rushed early areas in `fast` mode.
const FAST_STEP_US: i64 = 12_000;

/// The timing of the arc each slide body shares a `logical_slide_id` with,
/// so slide heads follow their body arc (the head hit drives the slide grade).
fn logical_timings(spec: &ChartSpec, timings: &[ArcTiming]) -> HashMap<u64, ArcTiming> {
    let mut by_logical: HashMap<u64, ArcTiming> = HashMap::new();
    for (i, slide) in spec.slides.iter().enumerate() {
        by_logical.insert(
            slide.logical_slide_id,
            timings.get(i).copied().unwrap_or_default(),
        );
    }
    by_logical
}

/// Click + hold events for one slide head, shifted by `timing`.
fn push_head_events(events: &mut Vec<TimedInputEvent>, head: &SlideHeadChartNote, timing: ArcTiming) {
    let area = slot_area(head.slot);
    let tp = head.timing + timing.offset_us;
    let hold = if timing.fast { FAST_STEP_US } else { 15_000 };
    events.push(TimedInputEvent::SensorClick { tp, area });
    events.push(TimedInputEvent::SensorHold {
        tp,
        area,
        is_down: true,
    });
    events.push(TimedInputEvent::SensorHold {
        tp: tp + hold,
        area,
        is_down: false,
    });
}

/// Body events for one runtime slide arc, in judge-queue order. Each judge
/// track is a separate branch (wifi slides have three); `track_offset_us`
/// shifts a branch independently of the arc.
fn push_body_events(events: &mut Vec<TimedInputEvent>, slide: &SlideChartNote, timing: ArcTiming) {
    let len = slide.length.max(1);
    let base = slide.start_timing + timing.offset_us;
    let judge = slide
        .judge_at
        .map(|j| j + timing.offset_us)
        .unwrap_or(base + len);
    for (ti, track) in slide.judge_queues.iter().enumerate() {
        if timing.track_skip.get(ti).copied().unwrap_or(false) {
            continue;
        }
        let track_off = timing.track_offset_us.get(ti).copied().unwrap_or(0);
        let start = base + track_off;
        let end = start + len;
        let n = track.len();
        let max_fin = track
            .iter()
            .map(|a| a.arrow_progress_when_finished)
            .max()
            .unwrap_or(1)
            .max(1);
        let mut prev: Option<SensorArea> = None;
        let mut last_t = end;
        for (k, area_spec) in track.iter().enumerate() {
            let Some(&area) = area_spec.target_areas.first() else {
                continue;
            };
            let is_last = k + 1 == n;
            let t = if timing.fast {
                if is_last {
                    judge + track_off
                } else {
                    start + k as i64 * FAST_STEP_US
                }
            } else {
                start + len * area_spec.arrow_progress_when_finished as i64 / max_fin as i64
            };
            last_t = t;
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
            let tail_hold = if timing.fast { FAST_STEP_US } else { 15_000 };
            events.push(TimedInputEvent::SensorHold {
                tp: last_t + tail_hold,
                area: p,
                is_down: false,
            });
        }
    }
}

/// Build events from the core's **lowered chart**: each runtime arc holds every
/// sensor area of its judge queue in order. `timings[i]` picks the i-th arc's
/// offset / fast-mode (the enumeration's knobs); slide heads follow their arc.
pub fn build_events_with_offsets(spec: &ChartSpec, timings: &[ArcTiming]) -> Vec<TimedInputEvent> {
    let by_logical = logical_timings(spec, timings);
    let mut events = Vec::new();
    for head in &spec.slide_heads {
        let timing = by_logical
            .get(&head.logical_slide_id)
            .copied()
            .unwrap_or_default();
        if timing.skip {
            continue;
        }
        push_head_events(&mut events, head, timing);
    }
    for (i, slide) in spec.slides.iter().enumerate() {
        let timing = timings.get(i).copied().unwrap_or_default();
        if timing.skip {
            continue;
        }
        push_body_events(&mut events, slide, timing);
    }
    events.sort_by_key(timed_input_tp);
    events
}

/// [`build_events_with_offsets`] with every arc at its chart time.
pub fn build_events_from_spec(spec: &ChartSpec) -> Vec<TimedInputEvent> {
    build_events_with_offsets(spec, &[])
}

/// A time window (µs) occupied by a **non-ex** tap/hold. A slide A-ring press
/// inside it may steal the tap/hold's prime judgement, so enumeration treats it
/// as a conflict window (`docs/AUTOPLAY_GENERATOR.md`, "补充约束").
#[derive(Debug, Clone, Copy)]
pub struct ConflictWindow {
    pub start_us: i64,
    pub end_us: i64,
}

impl ConflictWindow {
    pub fn contains(&self, tp: i64) -> bool {
        tp >= self.start_us && tp <= self.end_us
    }
}

fn is_a_ring(area: SensorArea) -> bool {
    matches!(
        area,
        SensorArea::A1
            | SensorArea::A2
            | SensorArea::A3
            | SensorArea::A4
            | SensorArea::A5
            | SensorArea::A6
            | SensorArea::A7
            | SensorArea::A8
    )
}

/// Count slide A-ring presses (down transitions) inside a non-ex tap/hold
/// window. `events` is a generated tactic (e.g. [`VerifyResult::events`]).
pub fn count_a_zone_conflicts(events: &[TimedInputEvent], windows: &[ConflictWindow]) -> usize {
    if windows.is_empty() {
        return 0;
    }
    events
        .iter()
        .filter_map(|event| match event {
            TimedInputEvent::SensorClick { tp, area } => Some((*tp, *area, true)),
            TimedInputEvent::SensorHold { tp, area, is_down } => Some((*tp, *area, *is_down)),
            _ => None,
        })
        .filter(|(tp, area, is_press)| {
            *is_press && is_a_ring(*area) && windows.iter().any(|w| w.contains(*tp))
        })
        .count()
}

/// A-ring conflicts contributed by **one** runtime arc (its body plus the head
/// attributed to it). Lets the enumeration score a single-arc trial without
/// rebuilding every arc's events.
pub fn zone_conflict_for_arc(
    spec: &ChartSpec,
    timing: ArcTiming,
    arc: usize,
    windows: &[ConflictWindow],
) -> usize {
    if windows.is_empty() || timing.skip {
        return 0;
    }
    let Some(slide) = spec.slides.get(arc) else {
        return 0;
    };
    // Head attribution matches `zone_conflicts_by_arc` (first arc wins).
    let mut logical_to_arc: HashMap<u64, usize> = HashMap::new();
    for (i, s) in spec.slides.iter().enumerate() {
        logical_to_arc.entry(s.logical_slide_id).or_insert(i);
    }
    let mut events = Vec::new();
    for head in &spec.slide_heads {
        if logical_to_arc.get(&head.logical_slide_id) == Some(&arc) {
            push_head_events(&mut events, head, timing);
        }
    }
    push_body_events(&mut events, slide, timing);
    count_a_zone_conflicts(&events, windows)
}

/// A-ring conflicts per **runtime arc** (head conflicts attributed to the arc
/// whose `logical_slide_id` it shares). Lets the enumeration target the arcs
/// that actually collide with a non-ex tap/hold.
pub fn zone_conflicts_by_arc(
    spec: &ChartSpec,
    timings: &[ArcTiming],
    windows: &[ConflictWindow],
) -> Vec<usize> {
    let mut out = vec![0usize; spec.slides.len()];
    if windows.is_empty() {
        return out;
    }
    let mut logical_to_arc: HashMap<u64, usize> = HashMap::new();
    for (i, slide) in spec.slides.iter().enumerate() {
        logical_to_arc.insert(slide.logical_slide_id, i);
    }
    for head in &spec.slide_heads {
        let Some(&arc) = logical_to_arc.get(&head.logical_slide_id) else {
            continue;
        };
        let timing = timings.get(arc).copied().unwrap_or_default();
        if timing.skip {
            continue;
        }
        let mut events = Vec::new();
        push_head_events(&mut events, head, timing);
        out[arc] += count_a_zone_conflicts(&events, windows);
    }
    for (i, slide) in spec.slides.iter().enumerate() {
        let timing = timings.get(i).copied().unwrap_or_default();
        if timing.skip {
            continue;
        }
        let mut events = Vec::new();
        push_body_events(&mut events, slide, timing);
        out[i] += count_a_zone_conflicts(&events, windows);
    }
    out
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
        grades,
        events: events.to_vec(),
    }
}

/// Perfect family (CPerfect / Perfect / 2nd / 3rd) — neither great, good nor
/// miss/too-fast.
fn is_perfect_grade(g: JudgeGrade) -> bool {
    !g.is_miss_or_too_fast() && !g.is_great_grade() && !g.is_good_grade()
}
