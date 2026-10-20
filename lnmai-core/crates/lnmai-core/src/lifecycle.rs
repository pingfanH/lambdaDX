//! Note lifecycle state machines (Tap, Hold, Touch, Slide).
//!
//! Mirrors `LnmaiCore/Lifecycle.lean`.

use crate::areas::{ButtonZone, OuterSlot, SensorArea};
use crate::constants as C;
use crate::convert::convert_grade;
use crate::events::{AudioCommand, RenderCommand};
use crate::judge::{
    judge_hold_classic_end, judge_hold_end, judge_slide_classic, judge_slide_modern,
    judge_slide_too_late, judge_tap, judge_touch,
};
use crate::natural::Natural;
use crate::storage::SensorVec;
use crate::time::{self, time_point_add_duration, time_point_sub_duration, Duration, TimePoint};
#[cfg(test)]
use crate::types::AreaPolicy;
use crate::types::{
    JudgeEvent, JudgeEventKind, JudgeGrade, JudgePhase, JudgeStyle, RuntimePos, SlideKind,
};
pub use lnmai_core_verify::lifecycle::{
    accepted_input_engages_judge, HoldSubState, TapState, TouchState,
};
use lnmai_core_verify::lifecycle::{
    finish_touch_judgment, hold_after_head_action, hold_body_action, hold_head_action,
    tap_transition, touch_transition, HoldBodyAction, HoldBodyFacts, HoldHeadAction, TapAction,
    TouchAction,
};
use lnmai_core_verify::slide_lifecycle::{self, ActiveAction, SlideAction};

fn jd_event(
    kind: JudgeEventKind,
    grade: JudgeGrade,
    diff: Duration,
    position: RuntimePos,
    note_index: Natural,
    is_break: bool,
) -> JudgeEvent {
    JudgeEvent {
        kind,
        phase: JudgePhase::Head,
        grade,
        diff,
        position,
        note_index,
        is_break,
        is_ex: false,
        multiple: 1.into(),
    }
}

// ---------------------------------------------------------------------------
// Common parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommonNoteParams {
    pub judge_timing: TimePoint,
    pub judge_offset: Duration,
    pub is_break: bool,
    pub is_ex: bool,
    pub note_index: Natural,
}

impl CommonNoteParams {
    pub fn effective_timing(&self) -> TimePoint {
        time_point_add_duration(self.judge_timing.clone(), self.judge_offset.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldStart {
    Button(ButtonZone),
    Sensor(SensorArea),
}

impl Default for HoldStart {
    fn default() -> Self {
        Self::Button(ButtonZone::K1)
    }
}

impl HoldStart {
    pub fn to_runtime_pos(&self) -> RuntimePos {
        match self {
            HoldStart::Button(z) => RuntimePos::Button(*z),
            HoldStart::Sensor(a) => RuntimePos::Sensor(*a),
        }
    }
}

// ---------------------------------------------------------------------------
// Tap family
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapNote {
    pub params: CommonNoteParams,
    pub lane: OuterSlot,
    pub state: TapState,
    pub button_queue_index: Natural,
}

impl Default for TapNote {
    fn default() -> Self {
        Self {
            params: CommonNoteParams::default(),
            lane: OuterSlot::S1,
            state: TapState::default(),
            button_queue_index: Natural::default(),
        }
    }
}

impl TapNote {
    pub fn position(&self) -> RuntimePos {
        RuntimePos::Button(self.lane.to_button_zone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideHeadNote {
    pub params: CommonNoteParams,
    pub lane: OuterSlot,
    pub state: TapState,
    pub logical_slide_id: Natural,
    pub button_queue_index: Natural,
}

impl Default for SlideHeadNote {
    fn default() -> Self {
        Self {
            params: CommonNoteParams::default(),
            lane: OuterSlot::S1,
            state: TapState::default(),
            logical_slide_id: Natural::default(),
            button_queue_index: Natural::default(),
        }
    }
}

impl SlideHeadNote {
    pub fn position(&self) -> RuntimePos {
        RuntimePos::Button(self.lane.to_button_zone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapFamilyNote {
    Tap(TapNote),
    SlideHead(SlideHeadNote),
}

impl Default for TapFamilyNote {
    fn default() -> Self {
        Self::Tap(TapNote::default())
    }
}

impl From<TapNote> for TapFamilyNote {
    fn from(note: TapNote) -> Self {
        Self::Tap(note)
    }
}

impl From<SlideHeadNote> for TapFamilyNote {
    fn from(note: SlideHeadNote) -> Self {
        Self::SlideHead(note)
    }
}

impl TapFamilyNote {
    pub fn params(&self) -> CommonNoteParams {
        match self {
            TapFamilyNote::Tap(n) => n.params.clone(),
            TapFamilyNote::SlideHead(n) => n.params.clone(),
        }
    }
    pub fn lane(&self) -> OuterSlot {
        match self {
            TapFamilyNote::Tap(n) => n.lane,
            TapFamilyNote::SlideHead(n) => n.lane,
        }
    }
    pub fn state(&self) -> TapState {
        match self {
            TapFamilyNote::Tap(n) => n.state,
            TapFamilyNote::SlideHead(n) => n.state,
        }
    }
    pub fn button_queue_index(&self) -> Natural {
        match self {
            TapFamilyNote::Tap(n) => n.button_queue_index.clone(),
            TapFamilyNote::SlideHead(n) => n.button_queue_index.clone(),
        }
    }
    pub fn position(&self) -> RuntimePos {
        match self {
            TapFamilyNote::Tap(n) => n.position(),
            TapFamilyNote::SlideHead(n) => n.position(),
        }
    }
}

fn can_enter_judgeable(current_time: TimePoint, judgeable_start: TimePoint) -> bool {
    current_time.to_micros() >= judgeable_start.to_micros()
}

fn is_too_late_for_tap_like(current_time: TimePoint, late_limit: TimePoint) -> bool {
    current_time.to_micros() > late_limit.to_micros()
}

fn tap_like_miss_event(params: CommonNoteParams, lane: OuterSlot, style: JudgeStyle) -> JudgeEvent {
    let grade = convert_grade(style, JudgeGrade::Miss);
    jd_event(
        JudgeEventKind::Tap,
        grade,
        Duration::from_micros(-1000),
        RuntimePos::Button(lane.to_button_zone()),
        params.note_index,
        params.is_break,
    )
}

fn tap_like_judge_event(
    params: CommonNoteParams,
    lane: OuterSlot,
    grade: JudgeGrade,
    judge_diff: Duration,
) -> JudgeEvent {
    jd_event(
        JudgeEventKind::Tap,
        grade,
        judge_diff,
        RuntimePos::Button(lane.to_button_zone()),
        params.note_index,
        params.is_break,
    )
}

fn tap_transition_event(
    action: TapAction,
    params: CommonNoteParams,
    lane: OuterSlot,
    style: JudgeStyle,
    judge_diff: Duration,
) -> Option<JudgeEvent> {
    match action {
        TapAction::NoEvent => None,
        TapAction::Miss => Some(tap_like_miss_event(params, lane, style)),
        TapAction::Judge => {
            let raw = judge_tap(judge_diff.clone(), params.is_ex);
            let grade = convert_grade(style, raw);
            Some(tap_like_judge_event(params, lane, grade, judge_diff))
        }
    }
}

/// `tapStep`.
pub fn tap_step(
    note: &TapNote,
    current_time: TimePoint,
    judge_diff: Duration,
    input_clicked: bool,
    style: JudgeStyle,
) -> (TapNote, Option<JudgeEvent>) {
    let timing = note.params.effective_timing();
    let judgeable_start = time_point_sub_duration(timing.clone(), C::JUDGABLE_RANGE);
    let late_limit = time_point_add_duration(timing, C::TAP_GOOD);
    let (state, action) = tap_transition(
        note.state,
        is_too_late_for_tap_like(current_time.clone(), late_limit),
        can_enter_judgeable(current_time, judgeable_start),
        input_clicked,
    );
    let event = tap_transition_event(action, note.params.clone(), note.lane, style, judge_diff);
    (
        TapNote {
            state,
            ..note.clone()
        },
        event,
    )
}

/// `slideHeadStep`.
pub fn slide_head_step(
    note: &SlideHeadNote,
    current_time: TimePoint,
    judge_diff: Duration,
    input_clicked: bool,
    style: JudgeStyle,
) -> (SlideHeadNote, Option<JudgeEvent>) {
    let timing = note.params.effective_timing();
    let judgeable_start = time_point_sub_duration(timing.clone(), C::JUDGABLE_RANGE);
    let late_limit = time_point_add_duration(timing, C::TAP_GOOD);
    let (state, action) = tap_transition(
        note.state,
        is_too_late_for_tap_like(current_time.clone(), late_limit),
        can_enter_judgeable(current_time, judgeable_start),
        input_clicked,
    );
    let event = tap_transition_event(action, note.params.clone(), note.lane, style, judge_diff);
    (
        SlideHeadNote {
            state,
            ..note.clone()
        },
        event,
    )
}

/// `tapFamilyStep`.
pub fn tap_family_step(
    note: &TapFamilyNote,
    current_time: TimePoint,
    judge_diff: Duration,
    input_clicked: bool,
    style: JudgeStyle,
) -> (TapFamilyNote, Option<JudgeEvent>) {
    match note {
        TapFamilyNote::Tap(tap) => {
            let (next, evt) = tap_step(tap, current_time, judge_diff, input_clicked, style);
            (TapFamilyNote::Tap(next), evt)
        }
        TapFamilyNote::SlideHead(head) => {
            let (next, evt) = slide_head_step(head, current_time, judge_diff, input_clicked, style);
            (TapFamilyNote::SlideHead(next), evt)
        }
    }
}

// ---------------------------------------------------------------------------
// Hold
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldNote {
    pub params: CommonNoteParams,
    pub start: HoldStart,
    pub state: HoldSubState,
    pub length: Duration,
    pub button_queue_index: Natural,
    pub head_diff: Duration,
    pub head_grade: JudgeGrade,
    pub player_release_time: Duration,
    pub release_ignore_time: Duration,
    pub is_classic: bool,
    pub is_touch_hold: bool,
    pub touch_queue_index: Natural,
    pub touch_group_id: Option<Natural>,
    pub touch_group_size: Natural,
    pub touch_hold_group_id: Option<Natural>,
    pub touch_hold_group_size: Natural,
    pub touch_hold_group_triggered: bool,
}

impl Default for HoldNote {
    fn default() -> Self {
        Self {
            params: CommonNoteParams::default(),
            start: HoldStart::default(),
            state: HoldSubState::default(),
            length: Duration::zero(),
            button_queue_index: Natural::default(),
            head_diff: Duration::zero(),
            head_grade: JudgeGrade::Miss,
            player_release_time: Duration::zero(),
            release_ignore_time: Duration::zero(),
            is_classic: false,
            is_touch_hold: false,
            touch_queue_index: Natural::default(),
            touch_group_id: None,
            touch_group_size: 0.into(),
            touch_hold_group_id: None,
            touch_hold_group_size: 0.into(),
            touch_hold_group_triggered: false,
        }
    }
}

impl HoldNote {
    pub fn position(&self) -> RuntimePos {
        self.start.to_runtime_pos()
    }
}

fn hold_head_judged(
    note: &HoldNote,
    grade: JudgeGrade,
    head_diff: Duration,
    group_triggered: bool,
) -> HoldNote {
    HoldNote {
        state: HoldSubState::HeadJudged(grade),
        head_diff,
        head_grade: grade,
        touch_hold_group_triggered: group_triggered,
        ..note.clone()
    }
}

fn hold_head_miss(note: &HoldNote, head_diff: Duration) -> HoldNote {
    hold_head_judged(note, JudgeGrade::Miss, head_diff, false)
}

fn hold_head_shared(note: &HoldNote, grade: JudgeGrade, head_diff: Duration) -> HoldNote {
    hold_head_judged(note, grade, head_diff, true)
}

fn hold_head_judge_event(note: &HoldNote, grade: JudgeGrade, judge_diff: Duration) -> JudgeEvent {
    JudgeEvent {
        is_ex: note.params.is_ex,
        ..jd_event(
            JudgeEventKind::Hold,
            grade,
            judge_diff,
            note.position(),
            note.params.note_index.clone(),
            note.params.is_break,
        )
    }
}

fn judge_hold_head_tap_now(
    note: &HoldNote,
    style: JudgeStyle,
    judge_diff: Duration,
) -> (HoldNote, Option<JudgeEvent>) {
    let raw = judge_tap(judge_diff.clone(), note.params.is_ex);
    let grade = convert_grade(style, raw);
    (
        hold_head_judged(note, grade, judge_diff.clone(), false),
        Some(hold_head_judge_event(note, grade, judge_diff)),
    )
}

fn judge_hold_head_touch_now(
    note: &HoldNote,
    style: JudgeStyle,
    judge_diff: Duration,
) -> (HoldNote, Option<JudgeEvent>) {
    match judge_touch(judge_diff.clone(), note.params.is_ex) {
        Some(raw) => {
            let grade = convert_grade(style, raw);
            (
                hold_head_judged(note, grade, judge_diff.clone(), true),
                Some(hold_head_judge_event(note, grade, judge_diff)),
            )
        }
        None => (note.clone(), None),
    }
}

fn apply_hold_head_action(
    note: &HoldNote,
    action: HoldHeadAction,
    judge_diff: Duration,
    shared_result: Option<(JudgeGrade, Duration)>,
    style: JudgeStyle,
) -> (HoldNote, Option<JudgeEvent>) {
    match action {
        HoldHeadAction::Unchanged => (note.clone(), None),
        HoldHeadAction::MakeJudgeable => (
            HoldNote {
                state: HoldSubState::HeadJudgeable,
                ..note.clone()
            },
            None,
        ),
        HoldHeadAction::Miss => {
            let late_diff = if note.is_touch_hold {
                C::TOUCH_GOOD
            } else {
                C::TAP_GOOD
            };
            (
                hold_head_miss(note, late_diff.clone()),
                Some(hold_head_judge_event(
                    note,
                    convert_grade(style, JudgeGrade::Miss),
                    late_diff,
                )),
            )
        }
        HoldHeadAction::Shared => match shared_result {
            Some((grade, diff)) => (hold_head_shared(note, grade, diff), None),
            None => (note.clone(), None),
        },
        HoldHeadAction::JudgeTap => judge_hold_head_tap_now(note, style, judge_diff),
        HoldHeadAction::JudgeTouch => judge_hold_head_touch_now(note, style, judge_diff),
    }
}

fn hold_head_release_transition(
    note: &HoldNote,
    delta: Duration,
) -> (HoldNote, Option<JudgeEvent>) {
    if note.release_ignore_time.to_micros() <= C::DELUXE_HOLD_RELEASE_IGNORE_TIME.to_micros() {
        (
            HoldNote {
                release_ignore_time: note.release_ignore_time.clone() + delta,
                touch_hold_group_triggered: false,
                ..note.clone()
            },
            None,
        )
    } else {
        (
            HoldNote {
                state: HoldSubState::BodyReleased,
                player_release_time: note.player_release_time.clone()
                    + note.release_ignore_time.clone()
                    + delta,
                release_ignore_time: Duration::zero(),
                touch_hold_group_triggered: false,
                ..note.clone()
            },
            None,
        )
    }
}

fn hold_pressed_transition(note: &HoldNote) -> HoldNote {
    HoldNote {
        state: HoldSubState::BodyHeld,
        player_release_time: Duration::zero(),
        release_ignore_time: Duration::zero(),
        touch_hold_group_triggered: note.is_touch_hold,
        ..note.clone()
    }
}

fn hold_keep_pressed(note: &HoldNote) -> HoldNote {
    HoldNote {
        release_ignore_time: Duration::zero(),
        touch_hold_group_triggered: note.is_touch_hold,
        ..note.clone()
    }
}

fn hold_release_transition(note: &HoldNote, delta: Duration) -> HoldNote {
    if note.release_ignore_time.to_micros() <= C::DELUXE_HOLD_RELEASE_IGNORE_TIME.to_micros() {
        HoldNote {
            release_ignore_time: note.release_ignore_time.clone() + delta,
            touch_hold_group_triggered: false,
            ..note.clone()
        }
    } else {
        HoldNote {
            state: HoldSubState::BodyReleased,
            player_release_time: note.player_release_time.clone()
                + note.release_ignore_time.clone()
                + delta,
            release_ignore_time: Duration::zero(),
            touch_hold_group_triggered: false,
            ..note.clone()
        }
    }
}

fn hold_released_still_off(note: &HoldNote, delta: Duration) -> HoldNote {
    HoldNote {
        player_release_time: note.player_release_time.clone() + delta,
        touch_hold_group_triggered: false,
        ..note.clone()
    }
}

fn hold_released_recovered(note: &HoldNote) -> HoldNote {
    HoldNote {
        state: HoldSubState::BodyHeld,
        release_ignore_time: Duration::zero(),
        touch_hold_group_triggered: note.is_touch_hold,
        ..note.clone()
    }
}

/// `holdStep`.
#[allow(clippy::too_many_arguments)]
pub fn hold_step(
    note: &HoldNote,
    current_time: TimePoint,
    judge_diff: Duration,
    head_ignore: Duration,
    tail_ignore: Duration,
    input_clicked: bool,
    input_pressed: bool,
    current_button_pressed: bool,
    prev_sensor_pressed: bool,
    touch_panel_offset: Duration,
    shared_result: Option<(JudgeGrade, Duration)>,
    delta: Duration,
    style: JudgeStyle,
) -> (HoldNote, Option<JudgeEvent>) {
    let timing = note.params.effective_timing();
    let body_timing = if note.is_touch_hold {
        note.params.judge_timing.clone()
    } else {
        timing.clone()
    };
    let diff = (current_time.clone() - timing.clone()).to_micros();
    let body_check_start = time_point_add_duration(body_timing.clone(), head_ignore.clone());
    let body_check_end = time_point_sub_duration(
        time_point_add_duration(body_timing, note.length.clone()),
        tail_ignore.clone(),
    );
    let classic_body_check_start = time_point_sub_duration(timing.clone(), C::TAP_GOOD);
    let body_window_disabled = !note.is_classic
        && note.length.to_micros() <= head_ignore.to_micros() + tail_ignore.to_micros();
    let judgeable_start = time_point_sub_duration(timing.clone(), C::JUDGABLE_RANGE);
    let release_offset = if prev_sensor_pressed && !current_button_pressed {
        Duration::zero()
    } else {
        touch_panel_offset
    };

    let end_hold = |note: &HoldNote,
                    head_grade: JudgeGrade,
                    classic_release_timing: TimePoint,
                    release_time: Duration| {
        let final_grade = if note.is_classic {
            judge_hold_classic_end(
                head_grade,
                timing.to_micros(),
                note.length.clone(),
                classic_release_timing.to_micros(),
            )
        } else {
            judge_hold_end(
                head_grade,
                note.head_diff.clone(),
                note.length.clone(),
                head_ignore.clone() + tail_ignore.clone(),
                release_time,
            )
        };
        let final_grade = convert_grade(style, final_grade);
        let event_diff = if note.head_diff.to_micros() == 0 && head_grade == JudgeGrade::Miss {
            time::from_millis(150)
        } else {
            note.head_diff.clone()
        };
        let mut evt = jd_event(
            JudgeEventKind::Hold,
            final_grade,
            event_diff,
            note.position(),
            note.params.note_index.clone(),
            note.params.is_break,
        );
        evt.phase = JudgePhase::Tail;
        (
            HoldNote {
                state: HoldSubState::Ended(final_grade),
                touch_hold_group_triggered: false,
                ..note.clone()
            },
            Some(evt),
        )
    };

    let finish_body_step = |note: &HoldNote, action: HoldBodyAction| match action {
        HoldBodyAction::Unchanged => (note.clone(), None),
        HoldBodyAction::EndNow(grade) => end_hold(
            note,
            grade,
            current_time.clone(),
            note.player_release_time.clone(),
        ),
        HoldBodyAction::EndRelease(grade) => end_hold(
            note,
            grade,
            time_point_sub_duration(current_time.clone(), release_offset.clone()),
            note.player_release_time.clone(),
        ),
        HoldBodyAction::StartPressed => (hold_pressed_transition(note), None),
        HoldBodyAction::StartClassicPressed => (
            HoldNote {
                state: HoldSubState::BodyHeld,
                touch_hold_group_triggered: note.is_touch_hold,
                ..note.clone()
            },
            None,
        ),
        HoldBodyAction::KeepClassicPressed => (
            HoldNote {
                touch_hold_group_triggered: note.is_touch_hold,
                ..note.clone()
            },
            None,
        ),
        HoldBodyAction::KeepPressed => (hold_keep_pressed(note), None),
        HoldBodyAction::ReleaseHead(grade) => hold_head_release_transition(
            &HoldNote {
                head_grade: grade,
                ..note.clone()
            },
            delta.clone(),
        ),
        HoldBodyAction::Release => (hold_release_transition(note, delta.clone()), None),
        HoldBodyAction::Recover => (hold_released_recovered(note), None),
        HoldBodyAction::ReleasedOff => (hold_released_still_off(note, delta.clone()), None),
    };
    let facts = HoldBodyFacts {
        is_classic: note.is_classic,
        before_classic_start: current_time.to_micros() < classic_body_check_start.to_micros(),
        before_body_start: current_time.to_micros() < body_check_start.to_micros(),
        body_window_disabled,
        reached_end: diff >= note.length.to_micros(),
        classic_limit_reached: diff
            >= note.length.to_micros() + C::CLASSIC_HOLD_ALLOW_OVER_LENGTH.to_micros(),
        after_body_end: current_time.to_micros() > body_check_end.to_micros(),
        pressed: input_pressed,
    };
    match note.state {
        HoldSubState::HeadWaiting | HoldSubState::HeadJudgeable => {
            let late_diff = if note.is_touch_hold {
                C::TOUCH_GOOD
            } else {
                C::TAP_GOOD
            };
            let action = hold_head_action(
                note.state,
                note.is_touch_hold,
                current_time.to_micros()
                    > time_point_add_duration(timing.clone(), late_diff).to_micros(),
                can_enter_judgeable(current_time.clone(), judgeable_start),
                input_clicked,
                shared_result.is_some(),
            );
            let (next, event) =
                apply_hold_head_action(note, action, judge_diff, shared_result, style);
            let body_action = hold_after_head_action(
                next.state,
                note.is_touch_hold,
                input_clicked,
                current_time.to_micros() >= body_check_start.to_micros(),
                facts.reached_end,
                input_pressed,
            );
            let (next, tail_event) = finish_body_step(&next, body_action);
            (next, tail_event.or(event))
        }
        _ => finish_body_step(note, hold_body_action(note.state, note.head_grade, facts)),
    }
}

// ---------------------------------------------------------------------------
// Touch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchNote {
    pub params: CommonNoteParams,
    pub state: TouchState,
    pub sensor_pos: SensorArea,
    pub touch_queue_index: Natural,
    pub touch_group_id: Option<Natural>,
    pub touch_group_size: Natural,
}

impl Default for TouchNote {
    fn default() -> Self {
        Self {
            params: CommonNoteParams::default(),
            state: TouchState::default(),
            sensor_pos: SensorArea::A1,
            touch_queue_index: Natural::default(),
            touch_group_id: None,
            touch_group_size: 0.into(),
        }
    }
}

fn touch_miss_event(note: &TouchNote, judge_diff: Duration) -> JudgeEvent {
    JudgeEvent {
        is_ex: note.params.is_ex,
        ..jd_event(
            JudgeEventKind::Touch,
            JudgeGrade::Miss,
            judge_diff,
            RuntimePos::Sensor(note.sensor_pos),
            note.params.note_index.clone(),
            note.params.is_break,
        )
    }
}

fn touch_judge_event(note: &TouchNote, grade: JudgeGrade, judge_diff: Duration) -> JudgeEvent {
    JudgeEvent {
        is_ex: note.params.is_ex,
        ..jd_event(
            JudgeEventKind::Touch,
            grade,
            judge_diff,
            RuntimePos::Sensor(note.sensor_pos),
            note.params.note_index.clone(),
            note.params.is_break,
        )
    }
}

/// `touchStep`.
pub fn touch_step(
    note: &TouchNote,
    current_time: TimePoint,
    judge_diff: Duration,
    input_clicked: bool,
    shared_result: Option<(JudgeGrade, Duration)>,
    style: JudgeStyle,
) -> (TouchNote, Option<JudgeEvent>) {
    let timing = note.params.effective_timing();
    let judgeable_start = time_point_sub_duration(timing.clone(), C::JUDGABLE_RANGE);
    let late = time_point_add_duration(timing, C::TOUCH_GOOD);
    let (state, action) = touch_transition(
        note.state.clone(),
        current_time.to_micros() > late.to_micros(),
        can_enter_judgeable(current_time, judgeable_start),
        input_clicked,
        shared_result.is_some(),
    );
    let (state, event) = match action {
        TouchAction::NoEvent => (state, None),
        TouchAction::Miss => (state, Some(touch_miss_event(note, C::TOUCH_GOOD))),
        TouchAction::Shared => (
            state,
            shared_result.map(|(grade, diff)| touch_judge_event(note, grade, diff)),
        ),
        TouchAction::Judge => {
            let (state, grade) =
                finish_touch_judgment(judge_touch(judge_diff.clone(), note.params.is_ex), style);
            (
                state,
                grade.map(|grade| touch_judge_event(note, grade, judge_diff)),
            )
        }
    };
    (
        TouchNote {
            state,
            ..note.clone()
        },
        event,
    )
}

// ---------------------------------------------------------------------------
// Slide queue core
// ---------------------------------------------------------------------------

pub type SlideState = slide_lifecycle::SlideState<Duration>;

/// The extracted checkpoint implementation preserves unbounded progress values.
pub type SlideArea = lnmai_core_verify::lifecycle::SlideArea<Natural>;

pub type SlideQueue = Vec<SlideArea>;

pub fn slide_queue_remaining(queues: &[SlideQueue]) -> usize {
    lnmai_core_verify::slide_queue_metrics::remaining(queues)
}

fn wifi_queue_progress_remaining(is_classic: bool, queues: &[SlideQueue]) -> Natural {
    match queues {
        [left, center, right] => {
            if is_classic {
                if left.len() <= 1 && center.len() <= 1 && right.len() <= 1 {
                    (8).into()
                } else {
                    let max_len = left.len().max(center.len()).max(right.len());
                    let pick = |cands: [&SlideQueue; 3]| -> Natural {
                        cands
                            .iter()
                            .find(|q| q.len() == max_len)
                            .and_then(|q| q.first())
                            .map(|area| area.arrow_progress_when_finished.clone())
                            .unwrap_or((0).into())
                    };
                    pick([left, center, right])
                }
            } else if center.is_empty() && left.len() <= 1 && right.len() <= 1 {
                (9).into()
            } else {
                let max_len = left.len().max(center.len()).max(right.len());
                let pick = |cands: [&SlideQueue; 3]| -> Natural {
                    cands
                        .iter()
                        .find(|q| q.len() == max_len)
                        .and_then(|q| q.first())
                        .map(|area| area.arrow_progress_when_finished.clone())
                        .unwrap_or((0).into())
                };
                pick([left, center, right])
            }
        }
        _ => slide_queue_remaining(queues).into(),
    }
}

fn slide_progress_remaining(
    slide_kind: SlideKind,
    is_classic: bool,
    queues: &[SlideQueue],
) -> Natural {
    match slide_kind {
        SlideKind::Wifi => wifi_queue_progress_remaining(is_classic, queues),
        _ => slide_queue_remaining(queues).into(),
    }
}

pub fn slide_queues_cleared(queues: &[SlideQueue]) -> bool {
    lnmai_core_verify::slide_queue_metrics::cleared(queues)
}

fn slide_head_on(queue: &SlideQueue) -> bool {
    queue.first().map(|a| a.on()).unwrap_or(false)
}

fn slide_hide_bar_cmd(
    note_index: Natural,
    track_index: Option<Natural>,
    end_index: Natural,
) -> RenderCommand {
    match track_index {
        None => RenderCommand::HideSlideBars {
            note_index,
            end_index,
        },
        Some(track_index) => RenderCommand::HideSlideTrackBars {
            note_index,
            track_index,
            end_index,
        },
    }
}

fn flatten_render_cmds(cmds: Vec<Vec<RenderCommand>>) -> Vec<RenderCommand> {
    cmds.into_iter().flatten().collect()
}

fn collect_new_slide_on_tracks(
    index: Natural,
    old_queues: &[SlideQueue],
    new_queues: &[SlideQueue],
) -> Vec<Natural> {
    match (old_queues.split_first(), new_queues.split_first()) {
        (Some((old_q, old_rest)), Some((new_q, new_rest))) => {
            let mut rest = collect_new_slide_on_tracks(index.clone() + 1, old_rest, new_rest);
            if slide_head_on(new_q) && !slide_head_on(old_q) {
                let mut out = vec![index];
                out.append(&mut rest);
                out
            } else {
                rest
            }
        }
        _ => Vec::new(),
    }
}

pub fn update_slide_area(area: &SlideArea, sensor_held: &SensorVec<bool>) -> SlideArea {
    area.check(sensor_held)
}

pub fn flatten_slide_queues(queues: &[SlideQueue]) -> SlideQueue {
    queues.iter().flatten().cloned().collect()
}

fn slide_queue_core(
    note_index: Natural,
    track_index: Option<Natural>,
    emit_cmds: bool,
    queue: &SlideQueue,
    sensor_held: &SensorVec<bool>,
) -> (SlideQueue, Vec<RenderCommand>) {
    let (remaining, progress) =
        lnmai_core_verify::slide_queue::advance(emit_cmds, queue, sensor_held);
    let commands = progress
        .into_iter()
        .map(|end_index| slide_hide_bar_cmd(note_index.clone(), track_index.clone(), end_index))
        .collect();
    (remaining, commands)
}

pub fn replay_slide_queue(queue: &SlideQueue, sensor_held: &SensorVec<bool>) -> SlideQueue {
    slide_queue_core((0).into(), None, false, queue, sensor_held).0
}

fn update_slide_queue(
    note_index: Natural,
    track_index: Option<Natural>,
    queue: &SlideQueue,
    sensor_held: &SensorVec<bool>,
) -> (SlideQueue, Vec<RenderCommand>) {
    slide_queue_core(note_index, track_index, true, queue, sensor_held)
}

// ---------------------------------------------------------------------------
// Slide note stepping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideNote {
    pub params: CommonNoteParams,
    pub lane: OuterSlot,
    pub state: SlideState,
    pub length: Duration,
    pub head_timing: TimePoint,
    pub start_timing: TimePoint,
    pub group_start_timing: Option<TimePoint>,
    pub slide_kind: SlideKind,
    pub is_classic: bool,
    pub is_conn_slide: bool,
    pub parent_note_index: Option<Natural>,
    pub is_group_part_head: bool,
    pub is_group_part_end: bool,
    pub parent_finished: bool,
    pub parent_pending_finish: bool,
    pub initial_queue_remaining: Natural,
    pub total_judge_queue_len: Natural,
    pub track_count: Natural,
    pub is_checkable: bool,
    pub slide_sound_played: bool,
    pub multiple: Natural,
    pub judge_queues: Vec<SlideQueue>,
}

impl Default for SlideNote {
    fn default() -> Self {
        Self {
            params: CommonNoteParams::default(),
            lane: OuterSlot::S1,
            state: SlideState::default(),
            length: Duration::zero(),
            head_timing: TimePoint::zero(),
            start_timing: TimePoint::zero(),
            group_start_timing: None,
            slide_kind: SlideKind::Single,
            is_classic: false,
            is_conn_slide: false,
            parent_note_index: None,
            is_group_part_head: false,
            is_group_part_end: false,
            parent_finished: false,
            parent_pending_finish: false,
            initial_queue_remaining: Natural::default(),
            total_judge_queue_len: Natural::default(),
            track_count: 0.into(),
            is_checkable: false,
            slide_sound_played: false,
            multiple: 0.into(),
            judge_queues: Vec::new(),
        }
    }
}

impl SlideNote {
    pub fn position(&self) -> RuntimePos {
        RuntimePos::Button(self.lane.to_button_zone())
    }

    fn queue_tracks(&self) -> Vec<(Option<Natural>, SlideQueue)> {
        if self.track_count == 1 {
            self.judge_queues
                .iter()
                .map(|q| (None, q.clone()))
                .collect()
        } else {
            (0..self.judge_queues.len())
                .map(|i| Some(i.into()))
                .zip(self.judge_queues.clone())
                .collect()
        }
    }

    fn track_render_indices(&self) -> Vec<Natural> {
        self.track_count.range().collect()
    }
}

fn slide_progress_render_cmds(note: &SlideNote, remaining: Natural) -> Vec<RenderCommand> {
    match note.slide_kind {
        SlideKind::Single => vec![RenderCommand::UpdateSlideProgress {
            note_index: note.params.note_index.clone(),
            remaining,
        }],
        SlideKind::Wifi | SlideKind::ConnPart => note
            .track_render_indices()
            .into_iter()
            .map(|track_index| RenderCommand::UpdateSlideTrackProgress {
                note_index: note.params.note_index.clone(),
                track_index,
                remaining: remaining.clone(),
            })
            .collect(),
    }
}

fn slide_hide_render_cmds(note: &SlideNote) -> Vec<RenderCommand> {
    vec![RenderCommand::HideAllSlideBars {
        note_index: note.params.note_index.clone(),
    }]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideStepContext<'a> {
    pub current_time: TimePoint,
    pub touch_panel_offset: Duration,
    pub delta: Duration,
    pub style: JudgeStyle,
    pub subdivide_slide_judge_grade: bool,
    pub sensor_held: &'a SensorVec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideStepSemantic {
    pub note: SlideNote,
    pub event: Option<JudgeEvent>,
    pub queue_render_cmds: Vec<RenderCommand>,
    pub old_remaining: Natural,
    pub new_remaining: Natural,
    pub track_ons: Vec<Natural>,
    pub progress_changed: bool,
    pub hide_slide: bool,
    pub should_play_track_ons: bool,
    pub emit_progress_render: bool,
}

fn slide_effective_judge_grade(style: JudgeStyle, subdivide: bool, raw: JudgeGrade) -> JudgeGrade {
    slide_lifecycle::effective_grade(style, subdivide, raw)
}

fn slide_current_judge_diff(
    note: &SlideNote,
    current_time: TimePoint,
    touch_panel_offset: Duration,
) -> Duration {
    time_point_sub_duration(current_time, touch_panel_offset) - note.params.effective_timing()
}

fn slide_initial_wait_time(note: &SlideNote) -> Duration {
    time_point_add_duration(note.start_timing.clone(), note.length.clone())
        - note.params.judge_timing.clone()
}

pub fn slide_should_be_checkable(note: &SlideNote, current_time: TimePoint) -> bool {
    let head_timing = current_time - note.head_timing.clone();
    slide_lifecycle::should_be_checkable(
        note.is_checkable,
        note.is_conn_slide,
        note.is_group_part_head,
        note.parent_finished,
        note.parent_pending_finish,
        head_timing.to_micros() >= -50000,
    )
}

pub fn slide_updated_queues_with_cmds(
    note: &SlideNote,
    is_checkable: bool,
    sensor_held: &SensorVec<bool>,
) -> Vec<(SlideQueue, Vec<RenderCommand>)> {
    if is_checkable {
        note.queue_tracks()
            .into_iter()
            .map(|(track_index, queue)| {
                update_slide_queue(
                    note.params.note_index.clone(),
                    track_index,
                    &queue,
                    sensor_held,
                )
            })
            .collect()
    } else {
        note.judge_queues
            .iter()
            .map(|q| (q.clone(), Vec::new()))
            .collect()
    }
}

fn slide_too_late_timing(note: &SlideNote) -> TimePoint {
    let base = time_point_add_duration(
        time_point_add_duration(note.start_timing.clone(), note.length.clone()),
        C::SLIDE_GOOD,
    );
    let min_offset = if note.params.judge_offset.to_micros() < 0 {
        note.params.judge_offset.clone()
    } else {
        Duration::zero()
    };
    time_point_add_duration(base, min_offset)
}

fn slide_adjusted_judged_wait_time(
    note: &SlideNote,
    current_time: TimePoint,
    wait_time: Duration,
    judge_diff: Duration,
) -> Duration {
    let start = note
        .group_start_timing
        .clone()
        .unwrap_or(note.start_timing.clone());
    let remaining_start_time = current_time - start;
    if remaining_start_time.to_micros() < 0 {
        remaining_start_time.abs().div_nat(2)
    } else if !note.is_classic && judge_diff.to_micros() >= C::SLIDE_GOOD.to_micros() {
        C::SLIDE_JUDGED_LATE_CLEAR_WAIT
    } else {
        wait_time
    }
}

fn slide_judge_event(note: &SlideNote, grade: JudgeGrade, judge_diff: Duration) -> JudgeEvent {
    JudgeEvent {
        kind: JudgeEventKind::Slide,
        phase: JudgePhase::Head,
        is_ex: note.params.is_ex,
        grade,
        diff: judge_diff,
        position: note.position(),
        note_index: note.params.note_index.clone(),
        is_break: note.params.is_break,
        multiple: note.multiple.clone().max((1).into()),
    }
}

fn build_slide_semantic_base(
    note: &SlideNote,
    updated_queues: Vec<SlideQueue>,
    queue_render_cmds: Vec<RenderCommand>,
    old_remaining: Natural,
    new_remaining: Natural,
    track_ons: Vec<Natural>,
) -> SlideStepSemantic {
    let progress_changed = new_remaining != old_remaining
        || slide_queue_remaining(&updated_queues) != slide_queue_remaining(&note.judge_queues);
    SlideStepSemantic {
        note: SlideNote {
            judge_queues: updated_queues,
            ..note.clone()
        },
        event: None,
        queue_render_cmds,
        old_remaining,
        new_remaining,
        track_ons,
        progress_changed,
        hide_slide: false,
        should_play_track_ons: false,
        emit_progress_render: false,
    }
}

fn build_slide_static_semantic_base(note: &SlideNote, is_checkable: bool) -> SlideStepSemantic {
    let remaining = slide_progress_remaining(note.slide_kind, note.is_classic, &note.judge_queues);
    SlideStepSemantic {
        note: SlideNote {
            is_checkable,
            ..note.clone()
        },
        event: None,
        queue_render_cmds: Vec::new(),
        old_remaining: remaining.clone(),
        new_remaining: remaining,
        track_ons: Vec::new(),
        progress_changed: false,
        hide_slide: false,
        should_play_track_ons: false,
        emit_progress_render: false,
    }
}

fn build_slide_sensor_semantic_base(
    note: &SlideNote,
    ctx: &SlideStepContext,
    is_checkable: bool,
) -> SlideStepSemantic {
    let updated = slide_updated_queues_with_cmds(note, is_checkable, ctx.sensor_held);
    let updated_queues: Vec<SlideQueue> = updated.iter().map(|(q, _)| q.clone()).collect();
    let queue_render_cmds = flatten_render_cmds(updated.iter().map(|(_, c)| c.clone()).collect());
    let old_remaining =
        slide_progress_remaining(note.slide_kind, note.is_classic, &note.judge_queues);
    let new_remaining = slide_progress_remaining(note.slide_kind, note.is_classic, &updated_queues);
    let track_ons = if is_checkable {
        collect_new_slide_on_tracks((0).into(), &note.judge_queues, &updated_queues)
    } else {
        Vec::new()
    };
    let note_with = SlideNote {
        is_checkable,
        ..note.clone()
    };
    build_slide_semantic_base(
        &note_with,
        updated_queues,
        queue_render_cmds,
        old_remaining,
        new_remaining,
        track_ons,
    )
}

pub fn build_slide_dormant_semantic_base(note: &SlideNote) -> SlideStepSemantic {
    SlideStepSemantic {
        note: SlideNote {
            state: SlideState::Waiting,
            is_checkable: false,
            ..note.clone()
        },
        event: None,
        queue_render_cmds: Vec::new(),
        old_remaining: (0).into(),
        new_remaining: (0).into(),
        track_ons: Vec::new(),
        progress_changed: false,
        hide_slide: false,
        should_play_track_ons: false,
        emit_progress_render: false,
    }
}

fn slide_too_late_step_semantic(
    note: &SlideNote,
    ctx: &SlideStepContext,
    is_checkable: bool,
) -> SlideStepSemantic {
    let base = build_slide_static_semantic_base(note, is_checkable);
    let raw = judge_slide_too_late(slide_queue_remaining(&note.judge_queues));
    let grade = slide_effective_judge_grade(ctx.style, ctx.subdivide_slide_judge_grade, raw);
    SlideStepSemantic {
        note: SlideNote {
            state: SlideState::Ended,
            ..base.note.clone()
        },
        event: if note.is_conn_slide && !note.is_group_part_end {
            None
        } else {
            Some(slide_judge_event(note, grade, Duration::from_micros(-1000)))
        },
        hide_slide: true,
        ..base
    }
}

fn slide_active_step_semantic(
    note: &SlideNote,
    ctx: &SlideStepContext,
    is_judgable: bool,
    wait_time: Duration,
) -> SlideStepSemantic {
    let active_note = SlideNote {
        state: SlideState::Active(wait_time.clone()),
        is_checkable: true,
        ..note.clone()
    };
    let static_base = build_slide_static_semantic_base(&active_note, true);
    let is_too_late =
        ctx.current_time.to_micros() > slide_too_late_timing(&active_note).to_micros();
    match slide_lifecycle::active_action(
        is_judgable,
        slide_queues_cleared(&active_note.judge_queues),
        is_too_late,
    ) {
        ActiveAction::Complete => {
            let judge_diff = slide_current_judge_diff(
                &active_note,
                ctx.current_time.clone(),
                ctx.touch_panel_offset.clone(),
            );
            let raw = if active_note.is_classic {
                judge_slide_classic(judge_diff.clone())
            } else {
                judge_slide_modern(
                    judge_diff.clone(),
                    wait_time.clone(),
                    active_note.params.is_ex,
                )
            };
            let stored_grade = if active_note.is_classic {
                raw
            } else {
                convert_grade(ctx.style, raw)
            };
            let judged_wait_time = slide_adjusted_judged_wait_time(
                &active_note,
                ctx.current_time.clone(),
                wait_time,
                judge_diff.clone(),
            );
            SlideStepSemantic {
                note: SlideNote {
                    state: SlideState::Judged(stored_grade, judged_wait_time, judge_diff),
                    ..static_base.note.clone()
                },
                should_play_track_ons: active_note.is_group_part_head || !active_note.is_conn_slide,
                emit_progress_render: true,
                ..static_base
            }
        }
        ActiveAction::Timeout => slide_too_late_step_semantic(&active_note, ctx, true),
        ActiveAction::SampleSensors => {
            let semantic_base = build_slide_sensor_semantic_base(&active_note, ctx, true);
            SlideStepSemantic {
                should_play_track_ons: active_note.is_group_part_head || !active_note.is_conn_slide,
                emit_progress_render: semantic_base.progress_changed,
                ..semantic_base
            }
        }
    }
}

pub fn slide_step_semantic(note: &SlideNote, ctx: &SlideStepContext) -> SlideStepSemantic {
    let is_checkable = slide_should_be_checkable(note, ctx.current_time.clone());
    let is_judgable = note.is_group_part_end || !note.is_conn_slide;
    let initial_wait = match note.state {
        SlideState::Waiting => slide_initial_wait_time(note),
        _ => Duration::zero(),
    };
    let wait_expired = match &note.state {
        SlideState::Judged(_, wait, _) => wait.to_micros() <= 0,
        _ => false,
    };
    let action = slide_lifecycle::transition(
        note.state.clone(),
        initial_wait,
        is_checkable,
        is_judgable,
        ctx.current_time.to_micros() > slide_too_late_timing(note).to_micros(),
        wait_expired,
    );
    match action {
        SlideAction::Dormant => build_slide_dormant_semantic_base(note),
        SlideAction::Activate(wait) => slide_active_step_semantic(note, ctx, is_judgable, wait),
        SlideAction::Timeout => slide_too_late_step_semantic(note, ctx, false),
        SlideAction::Finish(grade, stored_judge_diff) => {
            let static_base = build_slide_static_semantic_base(note, is_checkable);
            let final_grade =
                slide_effective_judge_grade(ctx.style, ctx.subdivide_slide_judge_grade, grade);
            SlideStepSemantic {
                note: SlideNote {
                    state: SlideState::Ended,
                    ..static_base.note.clone()
                },
                event: if note.is_conn_slide && !note.is_group_part_end {
                    None
                } else {
                    Some(slide_judge_event(note, final_grade, stored_judge_diff))
                },
                hide_slide: true,
                ..static_base
            }
        }
        SlideAction::Wait(grade, wait_time, stored_judge_diff) => {
            let static_base = build_slide_static_semantic_base(note, is_checkable);
            let new_wait = wait_time - ctx.delta.clone();
            SlideStepSemantic {
                note: SlideNote {
                    state: SlideState::Judged(grade, new_wait, stored_judge_diff),
                    ..static_base.note.clone()
                },
                should_play_track_ons: note.is_group_part_head || !note.is_conn_slide,
                emit_progress_render: static_base.progress_changed,
                ..static_base
            }
        }
        SlideAction::Ended => {
            let static_base = build_slide_static_semantic_base(note, is_checkable);
            SlideStepSemantic {
                note: SlideNote {
                    state: SlideState::Ended,
                    ..static_base.note.clone()
                },
                ..static_base
            }
        }
    }
}

fn slide_semantic_audio_cmds(
    semantic: &SlideStepSemantic,
    current_time: TimePoint,
) -> Vec<AudioCommand> {
    if semantic.should_play_track_ons {
        semantic
            .track_ons
            .iter()
            .map(|track_index| AudioCommand::PlaySlideCue {
                note_index: semantic.note.params.note_index.clone(),
                track_index: track_index.clone(),
                is_break: semantic.note.params.is_break,
                at_time: current_time.clone(),
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn slide_semantic_render_cmds(semantic: &SlideStepSemantic) -> Vec<RenderCommand> {
    let progress_cmds = if semantic.emit_progress_render {
        slide_progress_render_cmds(&semantic.note, semantic.new_remaining.clone())
    } else {
        Vec::new()
    };
    let hide_cmds = if semantic.hide_slide {
        slide_hide_render_cmds(&semantic.note)
    } else {
        Vec::new()
    };
    let mut out = semantic.queue_render_cmds.clone();
    out.extend(progress_cmds);
    out.extend(hide_cmds);
    out
}

/// `slideStep`.
pub fn slide_step(
    note: &SlideNote,
    current_time: TimePoint,
    sensor_held: &SensorVec<bool>,
    touch_panel_offset: Duration,
    delta: Duration,
    style: JudgeStyle,
    subdivide_slide_judge_grade: bool,
) -> (
    SlideNote,
    Option<JudgeEvent>,
    Vec<AudioCommand>,
    Vec<RenderCommand>,
) {
    let ctx = SlideStepContext {
        current_time: current_time.clone(),
        touch_panel_offset,
        delta,
        style,
        subdivide_slide_judge_grade,
        sensor_held,
    };
    let mut semantic = slide_step_semantic(note, &ctx);
    let flags = slide_lifecycle::output_flags(
        matches!(semantic.note.state, SlideState::Ended),
        semantic.should_play_track_ons,
        semantic.note.slide_sound_played,
        !semantic.track_ons.is_empty(),
        semantic.hide_slide,
    );
    if flags.mark_sound_played {
        semantic.note.slide_sound_played = true;
    }
    let audio_cmds = if flags.emit_audio {
        slide_semantic_audio_cmds(&semantic, current_time)
    } else {
        Vec::new()
    };
    let render_cmds = if flags.emit_render {
        slide_semantic_render_cmds(&semantic)
    } else {
        Vec::new()
    };
    (semantic.note, semantic.event, audio_cmds, render_cmds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(timing_us: i64) -> CommonNoteParams {
        CommonNoteParams {
            judge_timing: TimePoint::from_micros(timing_us),
            judge_offset: Duration::zero(),
            is_break: false,
            is_ex: false,
            note_index: 1.into(),
        }
    }

    #[test]
    fn tap_clicks_perfect() {
        let note = TapNote {
            params: params(1_000_000),
            lane: OuterSlot::S1,
            state: TapState::Waiting,
            button_queue_index: 0.into(),
        };
        let (next, evt) = tap_step(
            &note,
            TimePoint::from_micros(1_000_000),
            Duration::zero(),
            true,
            JudgeStyle::Default,
        );
        assert_eq!(next.state, TapState::Ended);
        assert_eq!(evt.unwrap().grade, JudgeGrade::Perfect);
    }

    #[test]
    fn tap_too_late_is_miss() {
        let note = TapNote {
            params: params(0),
            lane: OuterSlot::S2,
            state: TapState::Waiting,
            button_queue_index: 0.into(),
        };
        let (_next, evt) = tap_step(
            &note,
            TimePoint::from_micros(400_000),
            Duration::zero(),
            false,
            JudgeStyle::Default,
        );
        assert_eq!(evt.unwrap().grade, JudgeGrade::Miss);
    }

    #[test]
    fn touch_too_early_stays() {
        let note = TouchNote {
            params: params(0),
            state: TouchState::Judgeable,
            sensor_pos: SensorArea::A1,
            touch_queue_index: 0.into(),
            touch_group_id: None,
            touch_group_size: 1.into(),
        };
        // Very fast hit beyond the 1st perfect window → judgeTouch returns none.
        let (_next, evt) = touch_step(
            &note,
            TimePoint::from_micros(-200_000),
            Duration::from_micros(-200_000),
            true,
            None,
            JudgeStyle::Default,
        );
        assert_eq!(evt, None);
    }

    #[test]
    fn slide_queue_remaining_uses_max() {
        let mk = |n: usize| -> SlideQueue {
            (0..n)
                .map(|_| SlideArea {
                    target_areas: vec![SensorArea::A1],
                    policy: AreaPolicy::Or,
                    is_last: false,
                    is_skippable: true,
                    arrow_progress_when_on: 0.into(),
                    arrow_progress_when_finished: 0.into(),
                    was_on: false,
                    was_off: false,
                    target_was_on: Vec::new(),
                    target_was_off: Vec::new(),
                })
                .collect()
        };
        assert_eq!(slide_queue_remaining(&[mk(3), mk(5), mk(1)]), 5);
    }
}
