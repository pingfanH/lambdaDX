//! Host-facing command events emitted by the runtime.
//!
//! Mirrors the `AudioCommand` / `RenderCommand` inductives in
//! `LnmaiCore/Types.lean`.

use crate::areas::{OuterSlot, SensorArea};
use crate::natural::Natural;
use crate::time::{Duration, TimePoint};
use crate::types::{JudgeEventKind, JudgeGrade, RuntimePos};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioCommand {
    PlayTouchHoldBody {
        note_index: Natural,
        at_time: TimePoint,
    },
    PlayJudgeSfx {
        kind: JudgeEventKind,
        grade: JudgeGrade,
        is_break: bool,
        at_time: TimePoint,
        note_index: Natural,
    },
    PlaySlideCue {
        note_index: Natural,
        track_index: Natural,
        is_break: bool,
        at_time: TimePoint,
    },
}

impl Default for AudioCommand {
    fn default() -> Self {
        Self::PlayJudgeSfx {
            kind: JudgeEventKind::Tap,
            grade: JudgeGrade::Miss,
            is_break: false,
            at_time: TimePoint::zero(),
            note_index: 0.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderCommand {
    ShowJudgeResult {
        kind: JudgeEventKind,
        grade: JudgeGrade,
        is_break: bool,
        diff: Duration,
        note_index: Natural,
    },
    UpdateSlideProgress {
        note_index: Natural,
        remaining: Natural,
    },
    UpdateSlideTrackProgress {
        note_index: Natural,
        track_index: Natural,
        remaining: Natural,
    },
    HideAllSlideBars {
        note_index: Natural,
    },
    HideSlideBars {
        note_index: Natural,
        end_index: Natural,
    },
    HideSlideTrackBars {
        note_index: Natural,
        track_index: Natural,
        end_index: Natural,
    },
}

impl Default for RenderCommand {
    fn default() -> Self {
        Self::ShowJudgeResult {
            kind: JudgeEventKind::Tap,
            grade: JudgeGrade::Miss,
            is_break: false,
            diff: Duration::zero(),
            note_index: 0.into(),
        }
    }
}

/// Convenience constructors matching the Lean `JudgeEvent` shape.
#[allow(dead_code)]
pub(crate) fn judge_event(
    kind: JudgeEventKind,
    grade: JudgeGrade,
    diff: Duration,
    position: RuntimePos,
    note_index: Natural,
    is_break: bool,
) -> crate::types::JudgeEvent {
    crate::types::JudgeEvent {
        kind,
        phase: crate::types::JudgePhase::Head,
        grade,
        diff,
        position,
        note_index,
        is_break,
        is_ex: false,
        multiple: 1.into(),
    }
}

#[allow(dead_code)]
pub(crate) fn button_pos(slot: OuterSlot) -> RuntimePos {
    RuntimePos::Button(slot.to_button_zone())
}

#[allow(dead_code)]
pub(crate) fn sensor_pos(area: SensorArea) -> RuntimePos {
    RuntimePos::Sensor(area)
}
