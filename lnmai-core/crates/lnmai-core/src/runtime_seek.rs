//! Sparse seek restoration, following LnmaiCore/RuntimeSeek.lean.

use crate::natural::Natural;
use std::collections::{HashMap, HashSet};

use crate::chart_loader::{chart_score_totals, ChartSpec, SlideChartNote};
use crate::constants::{JUDGABLE_RANGE, TOUCH_GOOD};
use crate::events::RenderCommand;
use crate::input_model::{FrameInput, GameState, TimedInputBatch, TimedInputEvent};
use crate::lifecycle::{HoldNote, SlideState, TapFamilyNote};
use crate::runtime_score::RuntimeScoreState;
use crate::scheduler::step_frame_timed;
use crate::storage::{ButtonVec, SensorVec};
use crate::time::TimePoint;

pub fn slide_root(slides: &[SlideChartNote], mut id: Natural) -> Natural {
    for _ in slides {
        match slides
            .iter()
            .find(|n| n.note_index == id)
            .and_then(|n| n.parent_note_index.clone())
        {
            Some(parent) => id = parent,
            None => break,
        }
    }
    id
}

pub fn note_indices(chart: &ChartSpec) -> Vec<Natural> {
    chart
        .taps
        .iter()
        .map(|n| n.note_index.clone())
        .chain(chart.holds.iter().map(|n| n.note_index.clone()))
        .chain(chart.touches.iter().map(|n| n.note_index.clone()))
        .chain(chart.touch_holds.iter().map(|n| n.note_index.clone()))
        .chain(chart.slide_heads.iter().map(|n| n.note_index.clone()))
        .chain(chart.slides.iter().map(|n| n.note_index.clone()))
        .collect()
}

pub fn chart_from(chart: &ChartSpec, target: TimePoint) -> ChartSpec {
    // slideRoots uses the last parent for repeated IDs, unlike slideRoot's find.
    let parents: HashMap<_, _> = chart
        .slides
        .iter()
        .map(|n| (n.note_index.clone(), n.parent_note_index.clone()))
        .collect();
    let roots: HashMap<_, _> = chart
        .slides
        .iter()
        .map(|n| {
            let mut id = n.note_index.clone();
            for _ in &chart.slides {
                match parents.get(&id) {
                    Some(Some(parent)) => id = parent.clone(),
                    _ => break,
                }
            }
            (n.note_index.clone(), id)
        })
        .collect();
    let active: HashSet<_> = chart
        .slides
        .iter()
        .filter(|n| n.start_timing.micros.clone() + n.length.micros.clone() >= target.micros)
        .map(|n| <Natural as Clone>::clone(&roots[&n.note_index]))
        .collect();
    let mut remaining = chart.clone();
    remaining
        .slides
        .retain(|n| active.contains(&roots[&n.note_index]));
    let logical: HashSet<_> = remaining
        .slides
        .iter()
        .map(|n| n.logical_slide_id.clone())
        .collect();
    remaining
        .slide_heads
        .retain(|n| n.timing.micros >= target.micros || logical.contains(&n.logical_slide_id));
    remaining.taps.retain(|n| n.timing.micros >= target.micros);
    remaining
        .holds
        .retain(|n| n.timing.micros.clone() + n.length.micros.clone() >= target.micros);
    remaining
        .touches
        .retain(|n| n.timing.micros >= target.micros);
    remaining
        .touch_holds
        .retain(|n| n.timing.micros.clone() + n.length.micros.clone() >= target.micros);
    remaining
}

fn crossing_view(initial: &GameState, remaining: &ChartSpec, target: TimePoint) -> GameState {
    let crossing = |n: &HoldNote| {
        n.params.judge_timing.micros < target.micros
            && n.params.judge_timing.micros.clone() + n.length.micros.clone() >= target.micros
    };
    let timings: Vec<_> = initial
        .active_holds
        .iter()
        .map(|(_, n)| n)
        .chain(initial.active_touch_holds.iter().map(|(_, n)| n))
        .filter(|n| crossing(n))
        .map(|n| n.params.judge_timing.clone())
        .collect();
    let needs_head = |t: TimePoint| {
        t.micros < target.micros
            && timings.iter().any(|h| {
                (t.clone() - h.clone()).abs().micros <= TOUCH_GOOD.micros + JUDGABLE_RANGE.micros
            })
    };
    let mut state = initial.clone();
    // Lean reconstructs these sparse queues with ofFn, including missing
    // slots and truncating excess backing entries in restored states. Both
    // ranks use the original filtered indices before either queue is changed.
    let button_queues = ButtonVec::of_fn(|zone| {
        let mut taps = initial.tap_queues.get_d(zone, Default::default());
        let mut holds = initial.hold_queues.get_d(zone, Default::default());
        taps.notes.retain(|n| needs_head(n.params().judge_timing));
        holds
            .notes
            .retain(|n| crossing(n) || needs_head(n.params.judge_timing.clone()));
        taps.current_index = (0).into();
        holds.current_index = (0).into();
        let indices: Vec<_> = taps
            .notes
            .iter()
            .map(TapFamilyNote::button_queue_index)
            .chain(holds.notes.iter().map(|n| n.button_queue_index.clone()))
            .collect();
        for n in &mut taps.notes {
            let rank = indices
                .iter()
                .filter(|i| **i < n.button_queue_index())
                .count();
            match n {
                TapFamilyNote::Tap(n) => n.button_queue_index = (rank).into(),
                TapFamilyNote::SlideHead(n) => n.button_queue_index = (rank).into(),
            }
        }
        for n in &mut holds.notes {
            n.button_queue_index = (indices
                .iter()
                .filter(|i| **i < n.button_queue_index)
                .count())
            .into();
        }
        (taps, holds)
    });
    let (taps, holds) = button_queues.data.into_iter().unzip();
    state.tap_queues = ButtonVec { data: taps };
    state.hold_queues = ButtonVec { data: holds };
    let sensor_queues = SensorVec::of_fn(|area| {
        let mut touches = initial.touch_queues.get_d(area, Default::default());
        let mut holds = initial.touch_hold_queues.get_d(area, Default::default());
        touches
            .notes
            .retain(|n| needs_head(n.params.judge_timing.clone()));
        holds
            .notes
            .retain(|n| crossing(n) || needs_head(n.params.judge_timing.clone()));
        touches.current_index = (0).into();
        holds.current_index = (0).into();
        let indices: Vec<_> = touches
            .notes
            .iter()
            .map(|n| n.touch_queue_index.clone())
            .chain(holds.notes.iter().map(|n| n.touch_queue_index.clone()))
            .collect();
        for n in &mut touches.notes {
            n.touch_queue_index =
                (indices.iter().filter(|i| **i < n.touch_queue_index).count()).into();
        }
        for n in &mut holds.notes {
            n.touch_queue_index =
                (indices.iter().filter(|i| **i < n.touch_queue_index).count()).into();
        }
        (touches, holds)
    });
    let (touches, holds) = sensor_queues.data.into_iter().unzip();
    state.touch_queues = SensorVec { data: touches };
    state.touch_hold_queues = SensorVec { data: holds };
    let logical: HashSet<_> = remaining
        .slides
        .iter()
        .filter(|n| n.head_timing.micros < target.micros)
        .map(|n| n.logical_slide_id.clone())
        .collect();
    let ids: HashSet<_> = remaining
        .slides
        .iter()
        .filter(|n| logical.contains(&n.logical_slide_id))
        .map(|n| n.note_index.clone())
        .collect();
    state.slides.retain(|n| ids.contains(&n.params.note_index));
    state.active_holds = state
        .hold_queues
        .entries()
        .into_iter()
        .flat_map(|(z, q)| q.notes.into_iter().map(move |n| (z, n)))
        .collect();
    state.active_touch_holds = state
        .touch_hold_queues
        .entries()
        .into_iter()
        .flat_map(|(z, q)| q.notes.into_iter().map(move |n| (z, n)))
        .collect();
    state.touch_group_states.clear();
    state.touch_hold_group_states.clear();
    state
}

fn latch(held: &mut FrameInput, event: &TimedInputEvent) {
    match *event {
        TimedInputEvent::ButtonHold(_, zone, down) => {
            held.button_held = held.button_held.set(zone, down)
        }
        TimedInputEvent::SensorHold(_, area, down) => {
            held.sensor_held = held.sensor_held.set(area, down)
        }
        _ => (),
    }
}

type RenderKey = (Natural, usize, Natural);

// RuntimeSeek exposes Std.HashMap.toList order. Keep its Lean 4.30 bucket,
// collision, replacement and resize rules instead of using Rust's random hash.
struct RenderMemory {
    buckets: Vec<Vec<(RenderKey, RenderCommand)>>,
    len: usize,
}

impl RenderMemory {
    fn new() -> Self {
        Self {
            buckets: vec![vec![]; 16],
            len: 0,
        }
    }

    fn bucket(key: &RenderKey, count: usize) -> usize {
        fn mix(h: u64, k: u64) -> u64 {
            const M: u64 = 0xc6a4a7935bd1e995;
            let k = k.wrapping_mul(M);
            (h ^ (k ^ (k >> 47) ^ M)).wrapping_mul(M)
        }
        let h = mix(key.0.to_word(), mix(key.1 as u64, key.2.to_word()));
        let h = h ^ (h >> 32);
        ((h ^ (h >> 16)) as usize) & (count - 1)
    }

    fn remember(&mut self, command: RenderCommand) {
        let key: RenderKey = match command.clone() {
            RenderCommand::ShowJudgeResult { .. } => return,
            RenderCommand::UpdateSlideProgress { note_index, .. } => (note_index, 0, 0.into()),
            RenderCommand::UpdateSlideTrackProgress {
                note_index,
                track_index,
                ..
            } => (note_index, 1, track_index),
            RenderCommand::HideSlideBars { note_index, .. } => (note_index, 2, 0.into()),
            RenderCommand::HideSlideTrackBars {
                note_index,
                track_index,
                ..
            } => (note_index, 3, track_index),
            RenderCommand::HideAllSlideBars { note_index } => (note_index, 4, 0.into()),
        };
        let i = Self::bucket(&key, self.buckets.len());
        if let Some((_, value)) = self.buckets[i].iter_mut().find(|(k, _)| *k == key) {
            *value = command;
            return;
        }
        self.buckets[i].insert(0, (key, command));
        self.len += 1;
        if self.len * 4 / 3 > self.buckets.len() {
            let count = self.buckets.len() * 2;
            let old = std::mem::replace(&mut self.buckets, vec![vec![]; count]);
            for (key, value) in old.into_iter().flatten() {
                self.buckets[Self::bucket(&key, count)].insert(0, (key, value));
            }
        }
    }

    fn into_commands(self) -> Vec<RenderCommand> {
        self.buckets.into_iter().flatten().map(|(_, v)| v).collect()
    }
}

pub fn seek(
    chart: &ChartSpec,
    initial: &GameState,
    previous: &GameState,
    target: TimePoint,
    events: &[TimedInputEvent],
    keep_held: bool,
) -> (GameState, Vec<RenderCommand>) {
    let remaining = chart_from(chart, target.clone());
    let mut base = initial.clone();
    base.judge_style = previous.judge_style;
    base.touch_panel_offset = previous.touch_panel_offset.clone();
    base.subdivide_slide_judge_grade = previous.subdivide_slide_judge_grade;
    base.note_fast_late_display = previous.note_fast_late_display;
    base.break_fast_late_display = previous.break_fast_late_display;
    base.display_hold_head_judge_result = previous.display_hold_head_judge_result;
    base.button_ring_for_touch = previous.button_ring_for_touch;
    let mut state = crossing_view(&base, &remaining, target.clone());
    let starts: Vec<_> = state
        .active_holds
        .iter()
        .map(|(_, n)| n.params.judge_timing.clone())
        .chain(
            state
                .active_touch_holds
                .iter()
                .map(|(_, n)| n.params.judge_timing.clone()),
        )
        .chain(
            state
                .tap_queues
                .data
                .iter()
                .flat_map(|q| q.notes.iter().map(|n| n.params().judge_timing)),
        )
        .chain(
            state
                .touch_queues
                .data
                .iter()
                .flat_map(|q| q.notes.iter().map(|n| n.params.judge_timing.clone())),
        )
        .chain(state.slides.iter().map(|n| n.head_timing.clone()))
        .collect();
    let start = TimePoint::from_micros(
        crate::integer::Integer::small(0).max(
            starts
                .iter()
                .map(|t| t.micros.clone())
                .fold(target.micros.clone(), std::cmp::min)
                - JUDGABLE_RANGE.micros,
        ),
    );
    state.current_time = start.clone();
    let mut held = FrameInput::with_field_defaults();
    let mut render = RenderMemory::new();
    for event in events.iter().filter(|e| e.at().micros < target.micros) {
        if event.at().micros >= start.micros && !starts.is_empty() {
            let (next, _, _, commands) = step_frame_timed(
                &state,
                &TimedInputBatch {
                    current_time: TimePoint::from_micros(
                        state.current_time.micros.clone().max(event.at().micros),
                    ),
                    events: vec![event.clone()],
                },
            );
            state = next;
            for command in commands {
                render.remember(command);
            }
        } else {
            let mut latched = FrameInput {
                button_held: state.prev_button.clone(),
                sensor_held: state.prev_sensor.clone(),
                ..FrameInput::with_field_defaults()
            };
            latch(&mut latched, event);
            state.prev_button = latched.button_held;
            state.prev_sensor = latched.sensor_held;
        }
        latch(&mut held, event);
    }
    let boundary = TimePoint::from_micros(
        state
            .current_time
            .micros
            .clone()
            .max(target.micros.clone() - 1),
    );
    let (restored, _, _, commands) = step_frame_timed(
        &state,
        &TimedInputBatch {
            current_time: boundary,
            events: vec![],
        },
    );
    for command in commands {
        render.remember(command);
    }
    let restored_holds: HashMap<_, _> = restored
        .active_holds
        .iter()
        .map(|(_, n)| n)
        .chain(restored.active_touch_holds.iter().map(|(_, n)| n))
        .map(|n| (n.params.note_index.clone(), n))
        .collect();
    let restore_hold = |n: &HoldNote| {
        let mut result = restored_holds
            .get(&n.params.note_index)
            .map_or_else(|| n.clone(), |r| (*r).clone());
        result.button_queue_index = n.button_queue_index.clone();
        result.touch_queue_index = n.touch_queue_index.clone();
        result
    };
    let restored_slides: HashMap<_, _> = restored
        .slides
        .iter()
        .map(|n| (n.params.note_index.clone(), n))
        .collect();
    let mut next = base;
    next.current_time = target.clone();
    next.current_batch = TimedInputBatch {
        current_time: target.clone(),
        events: vec![],
    };
    if keep_held {
        next.prev_button = held.button_held;
        next.prev_sensor = held.sensor_held;
    }
    for q in &mut next.tap_queues.data {
        q.current_index = (q
            .notes
            .iter()
            .take_while(|n| n.params().judge_timing.micros < target.micros)
            .count())
        .into();
    }
    for q in &mut next.hold_queues.data {
        q.current_index = (q
            .notes
            .iter()
            .take_while(|n| n.params.judge_timing.micros < target.micros)
            .count())
        .into();
    }
    for q in &mut next.touch_queues.data {
        q.current_index = (q
            .notes
            .iter()
            .take_while(|n| n.params.judge_timing.micros < target.micros)
            .count())
        .into();
    }
    for q in &mut next.touch_hold_queues.data {
        q.current_index = (q
            .notes
            .iter()
            .take_while(|n| n.params.judge_timing.micros < target.micros)
            .count())
        .into();
    }
    next.button_queue_frontiers = ButtonVec::of_fn(|z| {
        next.tap_queues.get_d(z, Default::default()).current_index
            + next.hold_queues.get_d(z, Default::default()).current_index
    });
    next.touch_queue_frontiers = SensorVec::of_fn(|z| {
        next.touch_queues.get_d(z, Default::default()).current_index
            + next
                .touch_hold_queues
                .get_d(z, Default::default())
                .current_index
    });
    next.active_holds = initial
        .active_holds
        .iter()
        .filter(|(_, n)| {
            n.params.judge_timing.micros.clone() + n.length.micros.clone() >= target.micros
        })
        .map(|(z, n)| (*z, restore_hold(n)))
        .collect();
    next.active_touch_holds = initial
        .active_touch_holds
        .iter()
        .filter(|(_, n)| {
            n.params.judge_timing.micros.clone() + n.length.micros.clone() >= target.micros
        })
        .map(|(z, n)| (*z, restore_hold(n)))
        .collect();
    for n in &mut next.slides {
        if n.start_timing.micros.clone() + n.length.micros.clone() < target.micros {
            n.state = SlideState::Ended;
            for queue in &mut n.judge_queues {
                queue.clear();
            }
        } else if let Some(restored) = restored_slides.get(&n.params.note_index) {
            *n = (*restored).clone();
        }
    }
    next.touch_group_states.clear();
    next.touch_hold_group_states.clear();
    let mut scored = remaining;
    scored
        .slide_heads
        .retain(|n| n.timing.micros >= target.micros);
    scored
        .slides
        .retain(|n| n.start_timing.micros.clone() + n.length.micros.clone() >= target.micros);
    let totals = chart_score_totals(&scored);
    next.score = RuntimeScoreState {
        total_base: totals.total_base,
        total_extra: totals.total_extra,
        max_dx_score: totals.note_count * 3u32,
        ..RuntimeScoreState::default()
    };
    let mut commands = render.into_commands();
    commands.extend(
        initial
            .slides
            .iter()
            .filter(|n| n.start_timing.micros.clone() + n.length.micros.clone() < target.micros)
            .map(|n| RenderCommand::HideAllSlideBars {
                note_index: n.params.note_index.clone(),
            }),
    );
    (next, commands)
}
