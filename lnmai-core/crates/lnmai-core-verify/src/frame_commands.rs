//! Frame feedback selection and ordered command collection.
//!
//! Event payloads, timestamps, and arbitrary-precision identifiers stay in the
//! runtime. These functions own every branch deciding whether a command is
//! emitted, its sound kind, and the order of the collected commands.

use crate::lifecycle::HoldSubState;
use crate::types::{JudgeEventKind, JudgeGrade, JudgePhase, RuntimePos};

/// Hold heads on the touch panel are silent; rejected button hold heads are
/// silent too. An accepted button hold head uses the tap sound.
pub fn event_audio_kind(
    kind: JudgeEventKind,
    phase: JudgePhase,
    grade: JudgeGrade,
    position: RuntimePos,
) -> Option<JudgeEventKind> {
    let hold_head = matches!(kind, JudgeEventKind::Hold) && matches!(phase, JudgePhase::Head);
    if hold_head && (matches!(position, RuntimePos::Sensor(_)) || grade.is_miss_or_too_fast()) {
        None
    } else if hold_head {
        Some(JudgeEventKind::Tap)
    } else {
        Some(kind)
    }
}

pub fn event_render_visible(
    kind: JudgeEventKind,
    phase: JudgePhase,
    display_hold_head: bool,
) -> bool {
    !(matches!(kind, JudgeEventKind::Hold)
        && matches!(phase, JudgePhase::Head)
        && !display_hold_head)
}

/// Ended, waiting, and merely judgeable heads never emit body audio.
pub fn touch_hold_body_active(
    state: HoldSubState,
    local_pressed: bool,
    grouped_pressed: bool,
) -> bool {
    let head_accepted = match state {
        HoldSubState::HeadJudged(grade) => !grade.is_miss_or_too_fast(),
        HoldSubState::BodyHeld | HoldSubState::BodyReleased => true,
        _ => false,
    };
    head_accepted && (local_pressed || grouped_pressed)
}

/// Visit every source slot once, retaining each present command in source order.
/// Each source emits at most one command, so the result never exceeds `len`.
pub fn collect_indexed<T, F>(len: usize, command_at: F) -> Vec<T>
where
    F: Fn(usize) -> Option<T>,
{
    let mut commands = Vec::new();
    let mut index = 0;
    while index < len {
        if let Some(command) = command_at(index) {
            commands.push(command);
        }
        index += 1;
    }
    commands
}
