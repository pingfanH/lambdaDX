//! Direct in-process Rust API over the runtime registry.
//!
//! This module is the library-call replacement for hosts that previously went
//! through the ABI-3 record protocol. The native C ABI remains available and
//! uses the same registry and scheduler.

use std::collections::HashSet;
use std::sync::{MutexGuard, PoisonError};

use crate::chart_loader::ChartSpec;
use crate::default_tactic::{autoplay_plan_from_chart, AutoplayPlan};
use crate::events::{AudioCommand, RenderCommand};
use crate::ffi::{registry, RuntimeRegistry};
use crate::input_model::{GameState, TimedInputBatch, TimedInputEvent};
use crate::natural::Natural;
use crate::runtime_seek;
use crate::scheduler;
use crate::time::TimePoint;
use crate::types::JudgeEvent;

pub struct StepOutcome {
    pub state: GameState,
    pub events: Vec<JudgeEvent>,
    pub audio: Vec<AudioCommand>,
    pub render: Vec<RenderCommand>,
}

pub struct SeekOutcome {
    pub state: GameState,
    pub render: Vec<RenderCommand>,
    pub retained_note_indices: Vec<u64>,
    pub skipped_note_indices: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideBinding {
    pub note_index: u64,
    pub head_timing: i64,
    pub total_judge_queue_len: u64,
    pub track_count: u64,
    pub initial_queue_remaining: u64,
}

fn reg() -> Result<MutexGuard<'static, RuntimeRegistry>, String> {
    registry()
        .lock()
        .map_err(|error: PoisonError<_>| error.to_string())
}

pub fn create() -> u64 {
    reg().expect("runtime registry poisoned").create_empty_handle()
}

pub fn load_text(handle: u64, text: &str, level_index: u32) -> Result<(), String> {
    let spec = crate::simai::frontend::frontend_lowered_chart(
        text,
        (level_index as usize).into(),
    )
    .map_err(|error| error.message)?;
    reg()?.load_handle_chart(handle, spec)
}

pub fn load_spec(handle: u64, spec: ChartSpec) -> Result<(), String> {
    reg()?.load_handle_chart(handle, spec)
}

pub fn free(handle: u64) -> Result<(), String> {
    if reg()?.erase_handle_state(handle) {
        Ok(())
    } else {
        Err(format!("unknown runtime handle: {handle}"))
    }
}

pub fn state(handle: u64) -> Result<GameState, String> {
    reg()?.get_handle_state(handle).cloned()
}

pub fn chart(handle: u64) -> Result<ChartSpec, String> {
    reg()?.get_handle_chart_spec(handle).cloned()
}

pub fn plan(handle: u64) -> Result<AutoplayPlan, String> {
    let spec = chart(handle)?;
    Ok(autoplay_plan_from_chart(&spec))
}

pub fn bindings(handle: u64) -> Result<Vec<SlideBinding>, String> {
    let state = state(handle)?;
    Ok(state
        .slides
        .iter()
        .map(|slide| SlideBinding {
            note_index: slide.params.note_index.to_word(),
            head_timing: slide
                .head_timing
                .micros
                .to_i64()
                .unwrap_or(i64::MAX),
            total_judge_queue_len: slide.total_judge_queue_len.to_word(),
            track_count: slide.track_count.to_word(),
            initial_queue_remaining: slide.initial_queue_remaining.to_word(),
        })
        .collect())
}

pub fn step(handle: u64, batch: &TimedInputBatch) -> Result<StepOutcome, String> {
    let mut registry = reg()?;
    let previous = registry.get_handle_state(handle)?.clone();
    if batch.current_time.micros < previous.current_time.micros {
        return Err("clock moved backwards: use seek".to_string());
    }
    let (state, events, audio, render) = scheduler::step_frame_timed(&previous, batch);
    registry.update_handle_state(handle, state.clone())?;
    Ok(StepOutcome {
        state,
        events,
        audio,
        render,
    })
}

pub fn seek(
    handle: u64,
    target_us: i64,
    events: &[TimedInputEvent],
    keep_held: bool,
) -> Result<SeekOutcome, String> {
    let target = TimePoint::from_micros(target_us);
    let mut registry = reg()?;
    let spec = registry.get_handle_chart_spec(handle)?.clone();
    let previous = registry.get_handle_state(handle)?.clone();
    let (initial, tactic) = {
        let (initial, tactic) = registry
            .seek_origins
            .get(&handle)
            .ok_or_else(|| "seek requires a loaded session".to_string())?;
        (initial.clone(), tactic.clone())
    };
    let inputs = if events.is_empty() { &tactic } else { events };
    let (state, render) =
        runtime_seek::seek(&spec, &initial, &previous, target.clone(), inputs, keep_held);
    let retained_ids = runtime_seek::note_indices(&runtime_seek::chart_from(&spec, target));
    let retained_set: HashSet<Natural> = retained_ids.iter().cloned().collect();
    let retained_note_indices = retained_ids.iter().map(Natural::to_word).collect();
    let skipped_note_indices = runtime_seek::note_indices(&spec)
        .into_iter()
        .filter(|id| !retained_set.contains(id))
        .map(|id| id.to_word())
        .collect();
    registry.update_handle_state(handle, state.clone())?;
    Ok(SeekOutcome {
        state,
        render,
        retained_note_indices,
        skipped_note_indices,
    })
}
