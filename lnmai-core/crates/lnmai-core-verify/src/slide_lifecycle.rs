//! Slide/WiFi lifecycle control. Exact durations and queue snapshots are
//! supplied by the runtime; the transition never converts or bounds them.

use crate::convert::convert_grade;
use crate::judge::correct_slide_grade;
use crate::types::{JudgeGrade, JudgeStyle};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SlideState<D> {
    #[default]
    Waiting,
    Active(D),
    Judged(JudgeGrade, D, D),
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlideAction<D> {
    Dormant,
    Activate(D),
    Timeout,
    Finish(JudgeGrade, D),
    Wait(JudgeGrade, D, D),
    Ended,
}

pub fn should_be_checkable(
    latched: bool,
    connected: bool,
    group_head: bool,
    parent_finished: bool,
    parent_pending: bool,
    head_ready: bool,
) -> bool {
    if latched {
        true
    } else if connected {
        if group_head {
            head_ready
        } else {
            parent_finished || parent_pending
        }
    } else {
        head_ready
    }
}

pub fn unjudged_action<D>(
    wait: D,
    checkable: bool,
    judgable: bool,
    too_late: bool,
) -> SlideAction<D> {
    if checkable {
        SlideAction::Activate(wait)
    } else if judgable && too_late {
        SlideAction::Timeout
    } else {
        SlideAction::Dormant
    }
}

/// The caller compares the stored wait before subtracting this frame delta.
/// Crossing zero during the frame therefore emits the result next frame.
pub fn transition<D>(
    state: SlideState<D>,
    initial_wait: D,
    checkable: bool,
    judgable: bool,
    too_late: bool,
    wait_expired: bool,
) -> SlideAction<D> {
    match state {
        SlideState::Waiting => unjudged_action(initial_wait, checkable, judgable, too_late),
        SlideState::Active(wait) => unjudged_action(wait, checkable, judgable, too_late),
        SlideState::Judged(grade, wait, diff) => {
            if wait_expired {
                SlideAction::Finish(grade, diff)
            } else {
                SlideAction::Wait(grade, wait, diff)
            }
        }
        SlideState::Ended => SlideAction::Ended,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveAction {
    Complete,
    Timeout,
    SampleSensors,
}

/// Completion and timeout refer to the queues BEFORE this frame samples
/// sensors. Completion wins if both predicates are true.
pub fn active_action(judgable: bool, cleared: bool, too_late: bool) -> ActiveAction {
    if judgable && cleared {
        ActiveAction::Complete
    } else if judgable && too_late {
        ActiveAction::Timeout
    } else {
        ActiveAction::SampleSensors
    }
}

pub fn effective_grade(style: JudgeStyle, subdivide: bool, raw: JudgeGrade) -> JudgeGrade {
    let converted = convert_grade(style, raw);
    if subdivide {
        converted
    } else {
        correct_slide_grade(converted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputFlags {
    pub mark_sound_played: bool,
    pub emit_audio: bool,
    pub emit_render: bool,
}

pub fn output_flags(
    ended: bool,
    should_play: bool,
    sound_played: bool,
    has_track_ons: bool,
    hide_slide: bool,
) -> OutputFlags {
    OutputFlags {
        mark_sound_played: should_play && !sound_played && has_track_ons,
        emit_audio: !ended && should_play && has_track_ons,
        emit_render: !ended || hide_slide,
    }
}
