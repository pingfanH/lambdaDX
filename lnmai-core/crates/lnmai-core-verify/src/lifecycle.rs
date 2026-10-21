//! Note transitions and slide checkpoint history, shared with the runtime.
//!
//! Progress values are opaque: the runtime supplies its unbounded `Natural`,
//! while the checkpoint logic only preserves those values.

use crate::areas::SensorArea;
use crate::convert::convert_grade;
use crate::storage::SensorVec;
use crate::types::{AreaPolicy, JudgeGrade, JudgeStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TapState {
    #[default]
    Waiting,
    Judgeable,
    Judged(JudgeGrade),
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapAction {
    NoEvent,
    Miss,
    Judge,
}

/// Tap and star-head control flow. The runtime computes both time predicates
/// with arbitrary-precision arithmetic and attaches the selected event.
pub fn tap_transition(
    state: TapState,
    too_late: bool,
    can_judge: bool,
    input_clicked: bool,
) -> (TapState, TapAction) {
    match state {
        TapState::Waiting => {
            if too_late {
                (TapState::Ended, TapAction::Miss)
            } else if can_judge {
                if input_clicked {
                    (TapState::Ended, TapAction::Judge)
                } else {
                    (TapState::Judgeable, TapAction::NoEvent)
                }
            } else {
                (state, TapAction::NoEvent)
            }
        }
        TapState::Judgeable => {
            if too_late {
                (TapState::Ended, TapAction::Miss)
            } else if input_clicked && can_judge {
                (TapState::Ended, TapAction::Judge)
            } else {
                (state, TapAction::NoEvent)
            }
        }
        TapState::Judged(_) | TapState::Ended => (state, TapAction::NoEvent),
    }
}

pub fn accepted_input_engages_judge(input_clicked: bool) -> bool {
    input_clicked
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TouchState {
    #[default]
    Waiting,
    Judgeable,
    Judged(JudgeGrade),
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    NoEvent,
    Miss,
    Shared,
    Judge,
}

/// Select the touch action before evaluating a local judgment. Shared results
/// precede local input, but an expired note always emits its timeout miss.
pub fn touch_transition(
    state: TouchState,
    too_late: bool,
    can_judge: bool,
    input_clicked: bool,
    has_shared: bool,
) -> (TouchState, TouchAction) {
    match state {
        TouchState::Waiting => {
            if too_late {
                (TouchState::Ended, TouchAction::Miss)
            } else if has_shared {
                (TouchState::Ended, TouchAction::Shared)
            } else if can_judge {
                if input_clicked {
                    (TouchState::Judgeable, TouchAction::Judge)
                } else {
                    (TouchState::Judgeable, TouchAction::NoEvent)
                }
            } else {
                (state, TouchAction::NoEvent)
            }
        }
        TouchState::Judgeable => {
            if too_late {
                (TouchState::Ended, TouchAction::Miss)
            } else if has_shared {
                (TouchState::Ended, TouchAction::Shared)
            } else if input_clicked {
                (TouchState::Judgeable, TouchAction::Judge)
            } else {
                (state, TouchAction::NoEvent)
            }
        }
        TouchState::Judged(_) | TouchState::Ended => (state, TouchAction::NoEvent),
    }
}

/// A too-early local touch stays judgeable and emits no result.
pub fn finish_touch_judgment(
    raw: Option<JudgeGrade>,
    style: JudgeStyle,
) -> (TouchState, Option<JudgeGrade>) {
    match raw {
        Some(grade) => (TouchState::Ended, Some(convert_grade(style, grade))),
        None => (TouchState::Judgeable, None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldSubState {
    #[default]
    HeadWaiting,
    HeadJudgeable,
    HeadJudged(JudgeGrade),
    BodyHeld,
    BodyReleased,
    Ended(JudgeGrade),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldHeadAction {
    Unchanged,
    MakeJudgeable,
    Miss,
    Shared,
    JudgeTap,
    JudgeTouch,
}

/// The head priority depends on both family and state. In particular, an
/// already judgeable regular hold accepts its click before checking timeout.
pub fn hold_head_action(
    state: HoldSubState,
    is_touch_hold: bool,
    too_late: bool,
    can_judge: bool,
    clicked: bool,
    has_shared: bool,
) -> HoldHeadAction {
    match state {
        HoldSubState::HeadWaiting => {
            if too_late {
                HoldHeadAction::Miss
            } else if is_touch_hold && has_shared {
                HoldHeadAction::Shared
            } else if can_judge {
                if clicked {
                    if is_touch_hold {
                        HoldHeadAction::JudgeTouch
                    } else {
                        HoldHeadAction::JudgeTap
                    }
                } else {
                    HoldHeadAction::MakeJudgeable
                }
            } else {
                HoldHeadAction::Unchanged
            }
        }
        HoldSubState::HeadJudgeable => {
            if is_touch_hold {
                if too_late {
                    HoldHeadAction::Miss
                } else if has_shared {
                    HoldHeadAction::Shared
                } else if clicked && can_judge {
                    HoldHeadAction::JudgeTouch
                } else {
                    HoldHeadAction::Unchanged
                }
            } else if clicked && can_judge {
                HoldHeadAction::JudgeTap
            } else if too_late {
                HoldHeadAction::Miss
            } else {
                HoldHeadAction::Unchanged
            }
        }
        _ => HoldHeadAction::Unchanged,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldBodyAction {
    Unchanged,
    EndNow(JudgeGrade),
    EndRelease(JudgeGrade),
    StartPressed,
    StartClassicPressed,
    KeepClassicPressed,
    KeepPressed,
    ReleaseHead(JudgeGrade),
    Release,
    Recover,
    ReleasedOff,
}

/// Head resolution may sample the body immediately or replace head feedback
/// with a tail event. Shared/missed touch heads without a click wait one frame.
pub fn hold_after_head_action(
    state: HoldSubState,
    is_touch_hold: bool,
    clicked: bool,
    body_started: bool,
    reached_end: bool,
    pressed: bool,
) -> HoldBodyAction {
    match state {
        HoldSubState::HeadJudged(grade) => {
            if is_touch_hold && !clicked {
                HoldBodyAction::Unchanged
            } else if body_started {
                if reached_end {
                    HoldBodyAction::EndNow(grade)
                } else if pressed {
                    HoldBodyAction::StartPressed
                } else {
                    HoldBodyAction::ReleaseHead(grade)
                }
            } else {
                HoldBodyAction::Unchanged
            }
        }
        _ => HoldBodyAction::Unchanged,
    }
}

/// All comparisons are evaluated by the runtime using its unbounded times.
/// No ordering/reachability relationship between these facts is assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldBodyFacts {
    pub is_classic: bool,
    pub before_classic_start: bool,
    pub before_body_start: bool,
    pub body_window_disabled: bool,
    pub reached_end: bool,
    pub classic_limit_reached: bool,
    pub after_body_end: bool,
    pub pressed: bool,
}

pub fn hold_body_action(
    state: HoldSubState,
    head_grade: JudgeGrade,
    facts: HoldBodyFacts,
) -> HoldBodyAction {
    match state {
        HoldSubState::HeadJudged(grade) => {
            if facts.is_classic {
                if facts.before_classic_start {
                    HoldBodyAction::Unchanged
                } else if facts.classic_limit_reached || grade.is_miss_or_too_fast() {
                    HoldBodyAction::EndNow(grade)
                } else if facts.pressed {
                    HoldBodyAction::StartClassicPressed
                } else {
                    HoldBodyAction::EndRelease(grade)
                }
            } else if facts.before_body_start {
                HoldBodyAction::Unchanged
            } else if facts.body_window_disabled {
                if facts.reached_end {
                    HoldBodyAction::EndNow(grade)
                } else {
                    HoldBodyAction::Unchanged
                }
            } else if facts.reached_end {
                HoldBodyAction::EndNow(grade)
            } else if facts.after_body_end {
                HoldBodyAction::Unchanged
            } else if facts.pressed {
                HoldBodyAction::StartPressed
            } else {
                HoldBodyAction::ReleaseHead(grade)
            }
        }
        HoldSubState::BodyHeld => {
            if facts.is_classic {
                if facts.classic_limit_reached || head_grade.is_miss_or_too_fast() {
                    HoldBodyAction::EndNow(head_grade)
                } else if facts.pressed {
                    HoldBodyAction::KeepClassicPressed
                } else {
                    HoldBodyAction::EndRelease(head_grade)
                }
            } else if facts.body_window_disabled {
                if facts.reached_end {
                    HoldBodyAction::EndNow(head_grade)
                } else {
                    HoldBodyAction::Unchanged
                }
            } else if facts.reached_end {
                HoldBodyAction::EndNow(head_grade)
            } else if facts.after_body_end {
                HoldBodyAction::Unchanged
            } else if facts.pressed {
                HoldBodyAction::KeepPressed
            } else {
                HoldBodyAction::Release
            }
        }
        HoldSubState::BodyReleased => {
            // Restored Classic BodyReleased states follow this branch too.
            if facts.body_window_disabled {
                if facts.reached_end {
                    HoldBodyAction::EndNow(head_grade)
                } else {
                    HoldBodyAction::Unchanged
                }
            } else if facts.reached_end {
                HoldBodyAction::EndNow(head_grade)
            } else if facts.after_body_end {
                HoldBodyAction::Unchanged
            } else if facts.pressed {
                HoldBodyAction::Recover
            } else {
                HoldBodyAction::ReleasedOff
            }
        }
        _ => HoldBodyAction::Unchanged,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideArea<P> {
    pub target_areas: Vec<SensorArea>,
    pub policy: AreaPolicy,
    pub is_last: bool,
    pub is_skippable: bool,
    pub arrow_progress_when_on: P,
    pub arrow_progress_when_finished: P,
    pub was_on: bool,
    pub was_off: bool,
    pub target_was_on: Vec<bool>,
    pub target_was_off: Vec<bool>,
}

impl<P: Default> Default for SlideArea<P> {
    fn default() -> Self {
        Self {
            target_areas: Vec::new(),
            policy: AreaPolicy::Or,
            is_last: false,
            // Lean's derived Inhabited uses Bool.default, not the field initializer.
            is_skippable: false,
            arrow_progress_when_on: P::default(),
            arrow_progress_when_finished: P::default(),
            was_on: false,
            was_off: false,
            target_was_on: Vec::new(),
            target_was_off: Vec::new(),
        }
    }
}

fn history_any(history: &[bool]) -> bool {
    let mut result = false;
    let mut i = 0;
    while i < history.len() {
        result = result || history[i];
        i += 1;
    }
    result
}

// Lean zips the histories even for malformed restored states. In particular,
// AND over an empty zip is true; it does not pad the shorter history.
fn history_finished(on: &[bool], off: &[bool], is_last: bool, policy: AreaPolicy) -> bool {
    let mut result = match policy {
        AreaPolicy::Or => false,
        AreaPolicy::And => true,
    };
    let mut i = 0;
    while i < on.len() && i < off.len() {
        let finished = on[i] && (is_last || off[i]);
        result = match policy {
            AreaPolicy::Or => result || finished,
            AreaPolicy::And => result && finished,
        };
        i += 1;
    }
    result
}

fn history_get(history: &[bool], index: usize) -> bool {
    if index < history.len() {
        history[index]
    } else {
        false
    }
}

fn target_history(
    targets: &[SensorArea],
    was_on: &[bool],
    was_off: &[bool],
    sensor_held: &SensorVec<bool>,
) -> (Vec<bool>, Vec<bool>) {
    let mut on = Vec::new();
    let mut off = Vec::new();
    let mut i = 0;
    while i < targets.len() {
        let old_on = history_get(was_on, i);
        let old_off = history_get(was_off, i);
        let held = sensor_held.get_d(targets[i], false);
        let new_on = old_on || held;
        let new_off = old_off || (old_on && !held);
        on.push(new_on);
        off.push(new_off);
        i += 1;
    }
    (on, off)
}

// Aeneas models Vec::len; Vec::is_empty is currently an opaque external call.
#[allow(clippy::len_zero)]
impl<P> SlideArea<P> {
    pub fn on(&self) -> bool {
        self.was_on
    }

    pub fn is_finished(&self) -> bool {
        if self.target_areas.len() == 0 {
            false
        } else if self.target_was_on.len() == 0 {
            self.was_on && (self.is_last || self.was_off)
        } else {
            history_finished(
                &self.target_was_on,
                &self.target_was_off,
                self.is_last,
                self.policy,
            )
        }
    }
}

// Keep singleton construction and emptiness checks within the extracted model.
#[allow(clippy::len_zero, clippy::vec_init_then_push)]
impl<P: Clone> SlideArea<P> {
    pub fn check(&self, sensor_held: &SensorVec<bool>) -> Self {
        let source_on = if self.target_was_on.len() == 0 && self.target_areas.len() == 1 {
            let mut history = Vec::new();
            history.push(self.was_on);
            history
        } else {
            self.target_was_on.clone()
        };
        let source_off = if self.target_was_off.len() == 0 && self.target_areas.len() == 1 {
            let mut history = Vec::new();
            history.push(self.was_off);
            history
        } else {
            self.target_was_off.clone()
        };
        let (target_was_on, target_was_off) =
            target_history(&self.target_areas, &source_on, &source_off, sensor_held);
        let was_on = history_any(&target_was_on);
        let was_off = match self.policy {
            AreaPolicy::Or => {
                history_finished(&target_was_on, &target_was_off, false, AreaPolicy::Or)
            }
            AreaPolicy::And => {
                self.target_areas.len() != 0
                    && history_finished(&target_was_on, &target_was_off, false, AreaPolicy::And)
            }
        };
        Self {
            was_on,
            was_off,
            target_was_on,
            target_was_off,
            ..self.clone()
        }
    }
}
