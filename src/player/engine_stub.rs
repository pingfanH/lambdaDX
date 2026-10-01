//! `backend-none` engine stub.
//!
//! Provides the same public surface as [`super::engine`] but performs no
//! judging: the pad preview runs on local feedback / autoplay only. This keeps
//! the rest of the player compiling without any gameplay core dependency.

use crate::app::types::zone::PadZone;
use crate::app::types::{ChartDoc, NoteType};
use crate::core::types::{ButtonZone, SensorArea, TimedInputEvent};
use crate::player::state::PadPreviewState;

/// A no-op stand-in for the judging session. Loading always fails, so
/// `PadPreviewState::has_engine()` stays `false`.
pub struct JudgeEngine;

impl JudgeEngine {
    pub fn load(_simai_text: &str, _level_index: u32) -> Result<Self, String> {
        Err("lnmai-core backend disabled (backend-none)".to_string())
    }
    pub fn default_tactic(&self) -> Result<Vec<TimedInputEvent>, String> {
        Ok(Vec::new())
    }
    pub fn step(&mut self, _current_secs: f32, _events: Vec<TimedInputEvent>) -> Result<(), String> {
        Ok(())
    }
    pub fn runtime_slide_index(&self, _note_index: u64) -> Option<usize> {
        None
    }
    pub fn slide_count(&self) -> usize {
        0
    }
    pub fn slide_head_timing(&self, _runtime_slide_index: usize) -> Option<i64> {
        None
    }
    pub fn slide_queue_total(&self, _runtime_slide_index: usize) -> Option<(u64, u64, u64)> {
        None
    }
    pub fn debug_dump_slide_bindings(&self) {}
    pub fn slide_progress_updates(&self, _commands: &[()]) -> Vec<SlideArcProgress> {
        Vec::new()
    }
}

/// Consumed fraction of one runtime slide arc (no-op backend never emits any).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlideArcProgress {
    pub runtime_slide_index: usize,
    /// Wifi track this progress belongs to, if any.
    pub track_index: Option<u64>,
    pub frac: f32,
    /// Explicit wifi trail-bar cutoff (`HideSlideTrackBars`), if any.
    pub hidden_until_bar: Option<usize>,
}

/// Whether the `MAI2_DEBUG_SLIDE` diagnostic trace is enabled (never here).
pub(crate) fn debug_slide_enabled() -> bool {
    false
}

/// Diagnostics are disabled in the no-core backend.
pub(crate) fn debug_dedup(_tag: impl std::fmt::Display, _key: &str) -> bool {
    false
}

/// Microsecond timestamp of a core input event.
pub fn timed_input_tp(event: &TimedInputEvent) -> i64 {
    match event {
        TimedInputEvent::ButtonClick { tp, .. }
        | TimedInputEvent::ButtonHold { tp, .. }
        | TimedInputEvent::SensorClick { tp, .. }
        | TimedInputEvent::SensorHold { tp, .. } => *tp,
    }
}

/// No core: button tactics are never produced, so this is an identity pass.
pub fn normalize_tactic_event(event: TimedInputEvent) -> TimedInputEvent {
    event
}

/// No core, so presses produce no events.
pub fn press_events_for_zone(_zone: PadZone, _tp: i64) -> Vec<TimedInputEvent> {
    Vec::new()
}

pub fn release_events_for_zone(_zone: PadZone, _tp: i64) -> Vec<TimedInputEvent> {
    Vec::new()
}

/// No core, so a body hold produces no events.
pub fn hold_events_for_zone(_zone: PadZone, _tp: i64) -> Vec<TimedInputEvent> {
    Vec::new()
}

pub fn zone_for_button(btn: ButtonZone) -> PadZone {
    let id = match btn {
        ButtonZone::K1 => 1,
        ButtonZone::K2 => 2,
        ButtonZone::K3 => 3,
        ButtonZone::K4 => 4,
        ButtonZone::K5 => 5,
        ButtonZone::K6 => 6,
        ButtonZone::K7 => 7,
        ButtonZone::K8 => 8,
    };
    PadZone::from(id)
}

pub fn zone_for_sensor(area: SensorArea) -> PadZone {
    let id = match area {
        SensorArea::A1 => 1,
        SensorArea::A2 => 2,
        SensorArea::A3 => 3,
        SensorArea::A4 => 4,
        SensorArea::A5 => 5,
        SensorArea::A6 => 6,
        SensorArea::A7 => 7,
        SensorArea::A8 => 8,
        SensorArea::B1 => 9,
        SensorArea::B2 => 10,
        SensorArea::B3 => 11,
        SensorArea::B4 => 12,
        SensorArea::B5 => 13,
        SensorArea::B6 => 14,
        SensorArea::B7 => 15,
        SensorArea::B8 => 16,
        SensorArea::C => 17,
        SensorArea::D1 => 18,
        SensorArea::D2 => 19,
        SensorArea::D3 => 20,
        SensorArea::D4 => 21,
        SensorArea::D5 => 22,
        SensorArea::D6 => 23,
        SensorArea::D7 => 24,
        SensorArea::D8 => 25,
        SensorArea::E1 => 26,
        SensorArea::E2 => 27,
        SensorArea::E3 => 28,
        SensorArea::E4 => 29,
        SensorArea::E5 => 30,
        SensorArea::E6 => 31,
        SensorArea::E7 => 32,
        SensorArea::E8 => 33,
    };
    PadZone::from(id)
}

/// Map a runtime slide index onto the chart's `(note_id, slide_idx)`.
pub fn chart_slide_key(chart: &ChartDoc, runtime_slide_index: usize) -> Option<(u64, usize)> {
    chart_slide_position(chart, runtime_slide_index).map(|(note_id, slide_idx, _, _)| (note_id, slide_idx))
}

/// Like [`chart_slide_key`] but also resolves the segment within the sub-slide.
pub fn chart_slide_position(
    chart: &ChartDoc,
    runtime_slide_index: usize,
) -> Option<(u64, usize, usize, usize)> {
    let mut current = 0;
    chart
        .notes
        .iter()
        .filter(|note| matches!(note.note_type, NoteType::Slide))
        .find_map(|note| {
            for (slide_idx, slide) in note.slide.iter().enumerate() {
                let parts = slide.runtime_parts.max(1);
                if runtime_slide_index < current + parts {
                    return Some((note.id, slide_idx, runtime_slide_index - current, parts));
                }
                current += parts;
            }
            None
        })
}

/// No core: nothing to step.
pub fn step_judge_engine(_app: &mut PadPreviewState) {}
