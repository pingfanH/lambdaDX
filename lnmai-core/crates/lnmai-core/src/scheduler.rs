//! Frame scheduler: advance all active notes one frame.
//!
//! Mirrors `LnmaiCore/Scheduler.lean`. The subsystem processing order
//! (tap → hold → slide → touch → touch-hold) is part of the observable
//! semantics and is preserved exactly.

use crate::areas::{ButtonZone, SensorArea};
use crate::constants as C;
use crate::events::{AudioCommand, RenderCommand};
use crate::input_model::TouchHoldBodyGroupState;
use crate::input_model::{
    button_queue_at, sensor_queue_at, set_button_queue_at, set_sensor_queue_at, FrameInput,
    GameState, TimedInputBatch, ZoneQueue,
};
use crate::lifecycle::{
    hold_step, slide_queue_remaining, slide_step, tap_family_step, touch_step, HoldNote,
    HoldSubState, SlideNote, SlideState, TapFamilyNote, TapState, TouchNote, TouchState,
};
use crate::natural::Natural;
use crate::runtime_score::{fold_events_into_score, RuntimeScoreState};
use crate::storage::{ButtonVec, SensorVec};
use crate::time::{time_point_add_duration, time_point_sub_duration, Duration, TimePoint};
use crate::types::{GroupState, JudgeEvent, JudgeGrade};
use lnmai_core_verify::frame_commands;
use lnmai_core_verify::hold_schedule::{self as hold_schedule_kernel, HoldWork};
use lnmai_core_verify::queue_maintenance as queue_maintenance_kernel;
use lnmai_core_verify::queue_remaining as queue_remaining_kernel;
use lnmai_core_verify::scheduler::{
    self as scheduler_kernel, FrameEvents, FramePhase, TouchGroupAction, TouchGroupProbe,
};
use lnmai_core_verify::slide_parents::{
    self as slide_parents_kernel, ParentEdit, ParentInfo, SensorPhase,
};
use lnmai_core_verify::slide_schedule::{self as slide_schedule_kernel, SlideWork};
use lnmai_core_verify::tap_schedule::{self as tap_schedule_kernel, TapWork};
use lnmai_core_verify::touch_hold_body_adapter::{
    self as touch_hold_body_adapter_kernel, SliceHoldSource, TouchHoldBodyAdapter,
};
use lnmai_core_verify::touch_hold_rebuild as touch_hold_rebuild_kernel;
use lnmai_core_verify::touch_schedule::{self as touch_schedule_kernel, TouchWork};

/// The public pooled note representation from `Scheduler.ActiveNote`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveNote {
    TapNote(TapFamilyNote),
    HoldNote(HoldNote),
    TouchNote(TouchNote),
    SlideNote(SlideNote),
}

impl Default for ActiveNote {
    fn default() -> Self {
        Self::TapNote(TapFamilyNote::default())
    }
}

fn list_set_at<T: Clone>(items: &[T], index: Natural, value: T) -> Vec<T> {
    let Some(index) = index.to_usize() else {
        return items.to_vec();
    };
    let mut out = items.to_vec();
    if index < out.len() {
        out[index] = value;
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClickCursor {
    pub button_used: ButtonVec<Natural>,
    pub sensor_used: SensorVec<Natural>,
}

impl ClickCursor {
    /// Lean `{}` initializes every zone; `default` has empty backing vectors.
    pub fn with_field_defaults() -> Self {
        ClickCursor {
            button_used: ButtonVec::replicate(C::BUTTON_ZONE_COUNT as usize, (0).into()),
            sensor_used: SensorVec::replicate(C::SENSOR_AREA_COUNT as usize, (0).into()),
        }
    }
}

/// Snapshot returned by Lean's `probeTouchHeadAt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchHeadProbe {
    pub area: SensorArea,
    pub note_index: Natural,
    pub frontier: Natural,
    pub queue_current_index: Natural,
    pub state_before: TouchState,
    pub cursor_sensor_used_before: Natural,
    pub input_sensor_click_count: Natural,
    pub can_consume_click: bool,
    pub used_button: bool,
    pub used_sensor: bool,
    pub clicked: bool,
    pub shared_result: Option<(JudgeGrade, Duration)>,
    pub diff: Duration,
    pub state_after: TouchState,
    pub event: Option<JudgeEvent>,
}

impl Default for TouchHeadProbe {
    fn default() -> Self {
        Self {
            area: SensorArea::A1,
            note_index: 0.into(),
            frontier: 0.into(),
            queue_current_index: 0.into(),
            state_before: TouchState::Waiting,
            cursor_sensor_used_before: 0.into(),
            input_sensor_click_count: 0.into(),
            can_consume_click: false,
            used_button: false,
            used_sensor: false,
            clicked: false,
            shared_result: None,
            diff: Duration::zero(),
            state_after: TouchState::Waiting,
            event: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SensorClickConsumerKind {
    #[default]
    Tap,
    Hold,
}

/// A sensor click attributed to a tap or hold by the Lean diagnostic pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorClickConsumerProbe {
    pub kind: SensorClickConsumerKind,
    pub note_index: Natural,
    pub zone: ButtonZone,
    pub sensor_area: SensorArea,
    pub queue_index: Natural,
    pub frontier: Natural,
    pub diff: Duration,
}

impl Default for SensorClickConsumerProbe {
    fn default() -> Self {
        Self {
            kind: SensorClickConsumerKind::Tap,
            note_index: 0.into(),
            zone: ButtonZone::K1,
            sensor_area: SensorArea::A1,
            queue_index: 0.into(),
            frontier: 0.into(),
            diff: Duration::zero(),
        }
    }
}

fn try_use_sensor_click_at(
    input: &FrameInput,
    cursor: &ClickCursor,
    area: SensorArea,
) -> (bool, ClickCursor) {
    let used = cursor.sensor_used.get_d(area, (0).into());
    let available = input.get_sensor_click_count(area);
    if used < available {
        (
            true,
            ClickCursor {
                sensor_used: cursor.sensor_used.set(area, used + 1),
                ..cursor.clone()
            },
        )
    } else {
        (false, cursor.clone())
    }
}

fn has_unused_button_click_at(input: &FrameInput, cursor: &ClickCursor, zone: ButtonZone) -> bool {
    cursor.button_used.get_d(zone, (0).into()) < input.get_button_click_count(zone)
}

fn has_unused_sensor_click_at(input: &FrameInput, cursor: &ClickCursor, area: SensorArea) -> bool {
    cursor.sensor_used.get_d(area, (0).into()) < input.get_sensor_click_count(area)
}

fn fallback_sensor_area_for_button_note(zone: ButtonZone) -> SensorArea {
    zone.to_outer_sensor_area()
}

fn fallback_sensor_held_for_button_note(input: &FrameInput, zone: ButtonZone) -> bool {
    input.get_sensor_held(fallback_sensor_area_for_button_note(zone))
}

fn fallback_prev_sensor_held_for_button_note(
    prev_sensor: &SensorVec<bool>,
    zone: ButtonZone,
) -> bool {
    prev_sensor.get_d(fallback_sensor_area_for_button_note(zone), false)
}

fn hold_body_pressed_from_button_or_sensor(cur_btn: bool, cur_sensor: bool) -> bool {
    cur_btn || cur_sensor
}

/// Buttons are consumed before the mapped sensor fallback. The kernel fixes
/// that ownership decision; this function supplies the caller's comparisons and
/// performs the counter updates.
fn consume_button_then_fallback_sensor(
    input: &FrameInput,
    cursor: &ClickCursor,
    allow_input: bool,
    zone: ButtonZone,
    sensor_area: SensorArea,
) -> (bool, bool, ClickCursor) {
    let button_used = cursor.button_used.get_d(zone, (0).into());
    let button_available = button_used < input.get_button_click_count(zone);
    let sensor_used = cursor.sensor_used.get_d(sensor_area, (0).into());
    let sensor_available = sensor_used < input.get_sensor_click_count(sensor_area);
    let plan = lnmai_core_verify::scheduler::consume_click(
        allow_input,
        button_available,
        sensor_available,
    );
    let mut next = cursor.clone();
    if plan.used_button {
        next.button_used = next.button_used.set(zone, button_used + 1);
    }
    if plan.used_sensor {
        next.sensor_used = next.sensor_used.set(sensor_area, sensor_used + 1);
    }
    (plan.used_button, plan.used_sensor, next)
}

fn tap_family_queue_has_unused_click(
    input: &FrameInput,
    cursor: &ClickCursor,
    zone: ButtonZone,
    fallback_area: SensorArea,
) -> bool {
    lnmai_core_verify::scheduler::tap_queue_has_unused_click(
        has_unused_button_click_at(input, cursor, zone),
        has_unused_sensor_click_at(input, cursor, fallback_area),
    )
}

fn slide_remaining(slide: &SlideNote) -> usize {
    slide_queue_remaining(&slide.judge_queues)
}

fn empty_slide_queues(slide: &SlideNote) -> SlideNote {
    SlideNote {
        judge_queues: slide.judge_queues.iter().map(|_| Vec::new()).collect(),
        ..slide.clone()
    }
}

fn slide_parent_info(slide: &SlideNote) -> ParentInfo<Natural> {
    let remaining = slide_remaining(slide);
    ParentInfo {
        note_index: slide.params.note_index.clone(),
        parent_index: slide.parent_note_index.clone(),
        zero_remaining: remaining == 0,
        one_remaining: remaining == 1,
        connected: slide.is_conn_slide,
        final_part: slide.is_group_part_end,
        ended: matches!(slide.state, SlideState::Ended),
    }
}

struct SlideParentRead;

impl<S: AsRef<[SlideNote]>> slide_parents_kernel::ParentRead<S, Natural> for SlideParentRead {
    fn read(&self, state: &S, index: usize) -> ParentInfo<Natural> {
        slide_parent_info(&state.as_ref()[index])
    }
}

struct SlideParentEqual;

impl slide_parents_kernel::ParentEqual<Natural> for SlideParentEqual {
    fn equal(&self, left: &Natural, right: &Natural) -> bool {
        left == right
    }
}

/// The kernel owns traversal; this adapter changes exactly one selected slot.
fn apply_slide_parent_edit(slide: &mut SlideNote, edit: ParentEdit) {
    match edit {
        ParentEdit::Flags { finished, pending } => {
            slide.parent_finished = finished;
            slide.parent_pending_finish = pending;
        }
        ParentEdit::ClearQueues => *slide = empty_slide_queues(slide),
        ParentEdit::End => slide.state = SlideState::Ended,
    }
}

fn write_slide_parent(
    slides: &mut [SlideNote],
    index: usize,
    edit: ParentEdit,
) -> &mut [SlideNote] {
    apply_slide_parent_edit(&mut slides[index], edit);
    slides
}

fn append_parent_hide(mut commands: Vec<RenderCommand>, note_index: Natural) -> Vec<RenderCommand> {
    commands.push(RenderCommand::HideAllSlideBars { note_index });
    commands
}

fn update_slide_parent_flags(slides: &[SlideNote]) -> Vec<SlideNote> {
    slide_parents_kernel::refresh_flags(
        slides.len(),
        &slides,
        slides.to_vec(),
        SlideParentRead,
        SlideParentEqual,
        |mut updated: Vec<SlideNote>, index, edit| {
            apply_slide_parent_edit(&mut updated[index], edit);
            updated
        },
    )
}

fn slide_sensor_phase(slide: &SlideNote) -> SensorPhase {
    match &slide.state {
        SlideState::Waiting => SensorPhase::Waiting,
        SlideState::Active(_) => SensorPhase::Active,
        _ => SensorPhase::Other,
    }
}

pub fn slide_sensor_triggered(before: &SlideNote, after: &SlideNote) -> bool {
    // The total is a Lean Nat, even when individual queue lengths fit usize.
    let total = |slide: &SlideNote| {
        slide
            .judge_queues
            .iter()
            .map(|queue| Natural::from(queue.len()))
            .sum::<Natural>()
    };
    slide_parents_kernel::sensor_triggered(
        slide_sensor_phase(before),
        slide_sensor_phase(after),
        after.is_checkable,
        total(after) < total(before),
        after.judge_queues.len(),
        |index| {
            after.judge_queues[index]
                .first()
                .is_some_and(|area| area.on())
        },
    )
}

fn force_finish_direct_parent(
    slides: &mut [SlideNote],
    before: &SlideNote,
    child: &SlideNote,
) -> Vec<RenderCommand> {
    slide_parents_kernel::force_finish(
        slides.len(),
        slides,
        Vec::new(),
        slide_sensor_triggered(before, child),
        child.parent_note_index.clone(),
        SlideParentRead,
        SlideParentEqual,
        write_slide_parent,
        append_parent_hide,
    )
    .1
}

fn end_slide_ancestors(
    slides: &mut [SlideNote],
    before: &SlideNote,
    after: &SlideNote,
) -> Vec<RenderCommand> {
    slide_parents_kernel::end_ancestors(
        slides.len(),
        slides,
        Vec::new(),
        matches!(before.state, SlideState::Ended),
        matches!(after.state, SlideState::Ended),
        after.parent_note_index.clone(),
        SlideParentRead,
        SlideParentEqual,
        write_slide_parent,
        append_parent_hide,
    )
    .1
}

fn touch_click_can_engage_judge(
    note: &TouchNote,
    current_time: TimePoint,
    touch_panel_offset: Duration,
) -> bool {
    let diff = time_point_sub_duration(current_time.clone(), touch_panel_offset)
        - note.params.effective_timing();
    touch_eligible_for_click(note, current_time)
        && !(diff.to_micros() < 0 && diff.abs().to_micros() > C::TOUCH_PERFECT_1ST.to_micros())
}

fn tap_eligible_for_click(note: &TapFamilyNote, current_time: TimePoint) -> bool {
    let timing = note.params().effective_timing();
    current_time.to_micros()
        >= time_point_sub_duration(timing.clone(), C::JUDGABLE_RANGE).to_micros()
        && current_time.to_micros() <= time_point_add_duration(timing, C::TAP_GOOD).to_micros()
}

fn button_queue_index_unlocked(
    frontiers: &ButtonVec<Natural>,
    zone: ButtonZone,
    index: Natural,
) -> bool {
    index <= frontiers.get_d(zone, (0).into())
}

fn advance_shared_button_queue(
    frontiers: &ButtonVec<Natural>,
    zone: ButtonZone,
) -> ButtonVec<Natural> {
    frontiers.set(zone, frontiers.get_d(zone, (0).into()) + 1)
}

fn touch_eligible_for_click(note: &TouchNote, current_time: TimePoint) -> bool {
    let timing = note.params.effective_timing();
    current_time.to_micros()
        >= time_point_sub_duration(timing.clone(), C::JUDGABLE_RANGE).to_micros()
        && current_time.to_micros() <= time_point_add_duration(timing, C::TOUCH_GOOD).to_micros()
}

fn hold_head_eligible_for_click(note: &HoldNote, current_time: TimePoint) -> bool {
    let timing = note.params.effective_timing();
    let late_limit = if note.is_touch_hold {
        time_point_add_duration(
            time_point_add_duration(timing.clone(), C::JUDGABLE_RANGE),
            C::TOUCH_JUDGABLE_RANGE_LATE_EXTRA,
        )
    } else {
        time_point_add_duration(timing.clone(), C::JUDGABLE_RANGE)
    };
    current_time.to_micros() >= time_point_sub_duration(timing, C::JUDGABLE_RANGE).to_micros()
        && current_time.to_micros() <= late_limit.to_micros()
}

// ---------------------------------------------------------------------------
// Tap processing
// ---------------------------------------------------------------------------

/// Point adapters for the shared tap-family queue traversal. Context is fixed
/// for one zone/frame call; queue/frontier/cursor updates stay in TapWork.
struct TapScheduleFrame<'a> {
    zone: ButtonZone,
    input: &'a FrameInput,
    current_time: TimePoint,
    touch_panel_offset: Duration,
    style: crate::types::JudgeStyle,
}

impl
    tap_schedule_kernel::TapSchedule<
        ZoneQueue<TapFamilyNote>,
        TapFamilyNote,
        ButtonVec<Natural>,
        ClickCursor,
        Vec<JudgeEvent>,
        JudgeEvent,
        Duration,
        SensorArea,
    > for TapScheduleFrame<'_>
{
    fn peek(&self, queue: &ZoneQueue<TapFamilyNote>) -> Option<TapFamilyNote> {
        queue.peek()
    }

    fn button_diff(&self, note: &TapFamilyNote) -> Duration {
        self.current_time.clone() - note.params().effective_timing()
    }

    fn sensor_diff(&self, note: &TapFamilyNote) -> Duration {
        time_point_sub_duration(self.current_time.clone(), self.touch_panel_offset.clone())
            - note.params().effective_timing()
    }

    fn eligible(&self, note: &TapFamilyNote) -> bool {
        tap_eligible_for_click(note, self.current_time.clone())
    }

    fn unlocked(&self, frontiers: &ButtonVec<Natural>, note: &TapFamilyNote) -> bool {
        button_queue_index_unlocked(frontiers, self.zone, note.button_queue_index())
    }

    fn fallback_area(&self, note: &TapFamilyNote) -> SensorArea {
        fallback_sensor_area_for_button_note(note.lane().to_button_zone())
    }

    fn button_available(&self, cursor: &ClickCursor) -> bool {
        has_unused_button_click_at(self.input, cursor, self.zone)
    }

    fn sensor_available(&self, cursor: &ClickCursor, area: &SensorArea) -> bool {
        has_unused_sensor_click_at(self.input, cursor, *area)
    }

    fn use_button(&self, mut cursor: ClickCursor) -> ClickCursor {
        let used = cursor.button_used.get_d(self.zone, (0).into());
        cursor.button_used = cursor.button_used.set(self.zone, used + 1);
        cursor
    }

    fn use_sensor(&self, mut cursor: ClickCursor, area: &SensorArea) -> ClickCursor {
        let used = cursor.sensor_used.get_d(*area, (0).into());
        cursor.sensor_used = cursor.sensor_used.set(*area, used + 1);
        cursor
    }

    fn step(
        &self,
        note: &TapFamilyNote,
        diff: Duration,
        clicked: bool,
    ) -> (TapFamilyNote, Option<JudgeEvent>) {
        tap_family_step(note, self.current_time.clone(), diff, clicked, self.style)
    }

    fn ended(&self, note: &TapFamilyNote) -> bool {
        matches!(note.state(), TapState::Ended)
    }

    fn prepend_event(&self, mut events_rev: Vec<JudgeEvent>, event: JudgeEvent) -> Vec<JudgeEvent> {
        events_rev.insert(0, event);
        events_rev
    }

    fn advance(&self, queue: ZoneQueue<TapFamilyNote>) -> ZoneQueue<TapFamilyNote> {
        queue.advance()
    }

    fn advance_frontier(&self, frontiers: ButtonVec<Natural>) -> ButtonVec<Natural> {
        advance_shared_button_queue(&frontiers, self.zone)
    }

    fn replace_current(
        &self,
        queue: ZoneQueue<TapFamilyNote>,
        note: &TapFamilyNote,
    ) -> ZoneQueue<TapFamilyNote> {
        ZoneQueue {
            notes: list_set_at(&queue.notes, queue.current_index.clone(), note.clone()),
            ..queue
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_tap_queue_fuel(
    fuel: usize,
    zone: ButtonZone,
    queue: &ZoneQueue<TapFamilyNote>,
    frontiers: &ButtonVec<Natural>,
    input: &FrameInput,
    current_time: TimePoint,
    touch_panel_offset: Duration,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
    evs_rev: &[JudgeEvent],
) -> (
    ZoneQueue<TapFamilyNote>,
    ButtonVec<Natural>,
    ClickCursor,
    Vec<JudgeEvent>,
) {
    let out = tap_schedule_kernel::process(
        fuel,
        TapWork {
            queue: queue.clone(),
            frontiers: frontiers.clone(),
            cursor: cursor.clone(),
            events_rev: evs_rev.to_vec(),
        },
        TapScheduleFrame {
            zone,
            input,
            current_time,
            touch_panel_offset,
            style,
        },
    );
    (out.queue, out.frontiers, out.cursor, out.events_rev)
}

#[allow(clippy::too_many_arguments)]
fn process_tap_queue(
    zone: ButtonZone,
    queue: &ZoneQueue<TapFamilyNote>,
    frontiers: &ButtonVec<Natural>,
    input: &FrameInput,
    current_time: TimePoint,
    touch_panel_offset: Duration,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
    evs_rev: &[JudgeEvent],
) -> (
    ZoneQueue<TapFamilyNote>,
    ButtonVec<Natural>,
    ClickCursor,
    Vec<JudgeEvent>,
) {
    // Every continuation advances one slot. The reference's extra fuel
    // can only observe an empty queue, so no machine-sized +1 is needed.
    let len = queue_remaining_kernel::vec_len(&queue.notes);
    let converted = queue.current_index.to_usize();
    let fuel = queue_remaining_kernel::remaining_scalar(len, converted);
    process_tap_queue_fuel(
        fuel,
        zone,
        queue,
        frontiers,
        input,
        current_time,
        touch_panel_offset,
        style,
        cursor,
        evs_rev,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_tap_notes(
    frontiers: &ButtonVec<Natural>,
    queues: &crate::input_model::ButtonQueueVec<TapFamilyNote>,
    input: &FrameInput,
    current_time: TimePoint,
    touch_panel_offset: Duration,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
) -> (
    ButtonVec<Natural>,
    crate::input_model::ButtonQueueVec<TapFamilyNote>,
    Vec<JudgeEvent>,
    ClickCursor,
) {
    let (next_queues, (frontiers2, cursor2, mut evs_rev)) = queues.map_accum(
        (frontiers.clone(), cursor.clone(), Vec::<JudgeEvent>::new()),
        |zone, q, state| {
            let (frontiers, cursor, evs_rev) = state;
            let (next_queue, frontiers2, cursor2, evs_rev2) = process_tap_queue(
                zone,
                &q,
                &frontiers,
                input,
                current_time.clone(),
                touch_panel_offset.clone(),
                style,
                &cursor,
                &evs_rev,
            );
            (next_queue, (frontiers2, cursor2, evs_rev2))
        },
    );
    evs_rev.reverse();
    (frontiers2, next_queues, evs_rev, cursor2)
}

// ---------------------------------------------------------------------------
// Hold processing
// ---------------------------------------------------------------------------

fn is_head_judged_state(state: HoldSubState) -> bool {
    matches!(state, HoldSubState::HeadJudged(_))
}

fn entered_head_judged(before: HoldSubState, after: HoldSubState) -> bool {
    !is_head_judged_state(before) && is_head_judged_state(after)
}

fn hold_head_awaits_resolution(state: HoldSubState) -> bool {
    matches!(
        state,
        HoldSubState::HeadWaiting | HoldSubState::HeadJudgeable
    )
}

fn hold_queue_resolved(note: &HoldNote) -> bool {
    !hold_head_awaits_resolution(note.state)
}

struct HoldQueueMaintenance;

impl queue_maintenance_kernel::QueueCursor<ZoneQueue<HoldNote>, HoldNote> for HoldQueueMaintenance {
    fn peek(&self, queue: &ZoneQueue<HoldNote>) -> Option<HoldNote> {
        queue.peek()
    }

    fn resolved(&self, note: &HoldNote) -> bool {
        hold_queue_resolved(note)
    }

    fn advance(&self, mut queue: ZoneQueue<HoldNote>) -> ZoneQueue<HoldNote> {
        queue.current_index += 1;
        queue
    }
}

impl queue_maintenance_kernel::QueueReplace<ZoneQueue<HoldNote>, HoldNote>
    for HoldQueueMaintenance
{
    fn get(&self, queue: &ZoneQueue<HoldNote>, index: usize) -> HoldNote {
        queue.notes[index].clone()
    }

    fn index_equal(&self, existing: &HoldNote, replacement: &HoldNote) -> bool {
        existing.params.note_index == replacement.params.note_index
    }

    fn write(
        &self,
        mut queue: ZoneQueue<HoldNote>,
        index: usize,
        replacement: &HoldNote,
    ) -> ZoneQueue<HoldNote> {
        queue.notes[index] = replacement.clone();
        queue
    }
}

fn normalize_hold_queue_cursor(queue: &ZoneQueue<HoldNote>) -> ZoneQueue<HoldNote> {
    // Fuel sufficiency against Lean's remaining + 1 is still to be proved.
    let len = queue_remaining_kernel::vec_len(&queue.notes);
    let converted = queue.current_index.to_usize();
    let remaining = queue_remaining_kernel::remaining_scalar(len, converted);
    queue_maintenance_kernel::normalize_cursor_fuel(remaining, queue.clone(), HoldQueueMaintenance)
}

fn replace_hold_queue_note(queue: &ZoneQueue<HoldNote>, note: &HoldNote) -> ZoneQueue<HoldNote> {
    queue_maintenance_kernel::replace_all(
        queue_remaining_kernel::vec_len(&queue.notes),
        queue.clone(),
        note,
        HoldQueueMaintenance,
    )
}

fn update_button_hold_queue(
    queues: &crate::input_model::ButtonQueueVec<HoldNote>,
    zone: ButtonZone,
    note: &HoldNote,
) -> crate::input_model::ButtonQueueVec<HoldNote> {
    let queue = button_queue_at(queues, zone);
    set_button_queue_at(
        queues,
        zone,
        normalize_hold_queue_cursor(&replace_hold_queue_note(&queue, note)),
    )
}

fn update_sensor_hold_queue(
    queues: &crate::input_model::SensorQueueVec<HoldNote>,
    area: SensorArea,
    note: &HoldNote,
) -> crate::input_model::SensorQueueVec<HoldNote> {
    let queue = sensor_queue_at(queues, area);
    set_sensor_queue_at(
        queues,
        area,
        normalize_hold_queue_cursor(&replace_hold_queue_note(&queue, note)),
    )
}

fn has_strict_majority(count: Natural, size: Natural) -> bool {
    size > 0 && count * 2 > size
}

/// The extracted loop owns first-match selection. Identifier equality and the
/// majority comparison stay in the runtime's unbounded arithmetic adapter.
fn touch_group_plan(groups: &[GroupState], group_id: &Natural) -> TouchGroupAction {
    scheduler_kernel::touch_group_plan(groups.len(), |index| {
        let group = &groups[index];
        let matches_group = group.group_id == *group_id;
        TouchGroupProbe {
            matches_group,
            has_majority: matches_group
                && has_strict_majority(group.count.clone(), group.size.clone()),
        }
    })
}

fn group_share_result(groups: &[GroupState], group_id: Natural) -> Option<(JudgeGrade, Duration)> {
    match touch_group_plan(groups, &group_id) {
        TouchGroupAction::Update {
            index,
            keep_result: true,
        } => {
            let group = &groups[index];
            Some((group.grade, group.diff.clone()))
        }
        _ => None,
    }
}

fn register_touch_group_result(
    groups: &[GroupState],
    group_id: Natural,
    group_size: Natural,
    grade: JudgeGrade,
    diff: Duration,
) -> Vec<GroupState> {
    let plan = touch_group_plan(groups, &group_id);
    let mut out = groups.to_vec();
    match plan {
        TouchGroupAction::Update { index, keep_result } => {
            let group = &mut out[index];
            group.count = group.count.clone() + 1;
            group.size = group_size;
            if !keep_result {
                group.grade = grade;
                group.diff = diff;
            }
        }
        TouchGroupAction::Append => out.push(GroupState {
            group_id,
            count: (1).into(),
            size: group_size,
            grade,
            diff,
        }),
    }
    out
}

/// Literal note fields shared by scheduler and ChartLoader reconstruction.
/// Slice access and Id cloning live in the verification crate's SliceHoldSource.
pub(crate) struct TouchHoldRebuildAtoms;

impl touch_hold_body_adapter_kernel::HoldFields<HoldNote, Natural> for TouchHoldRebuildAtoms {
    fn body_group_id<'note>(&self, note: &'note HoldNote) -> &'note Option<Natural> {
        &note.touch_hold_group_id
    }

    fn note_index<'note>(&self, note: &'note HoldNote) -> &'note Natural {
        &note.params.note_index
    }

    fn body_triggered(&self, note: &HoldNote) -> bool {
        note.touch_hold_group_triggered
    }
}

fn touch_hold_body_group_states_from_holds(
    holds: &[(SensorArea, HoldNote)],
) -> Vec<TouchHoldBodyGroupState> {
    touch_hold_rebuild_kernel::rebuild(
        &holds,
        TouchHoldBodyAdapter {
            ops: SliceHoldSource {
                fields: TouchHoldRebuildAtoms,
            },
        },
    )
}

fn touch_hold_body_group_majority_pressed(
    groups: &[TouchHoldBodyGroupState],
    group_id: Natural,
) -> bool {
    touch_hold_body_adapter_kernel::body_pressed(groups, &group_id)
}

fn touch_hold_body_check_active(note: &HoldNote, current_time: TimePoint) -> bool {
    let timing = note.params.judge_timing.clone();
    let body_check_start = time_point_add_duration(timing.clone(), C::TOUCH_HOLD_HEAD_IGNORE);
    let body_check_end = time_point_sub_duration(
        time_point_add_duration(timing, note.length.clone()),
        C::TOUCH_HOLD_TAIL_IGNORE,
    );
    let body_window_disabled = !note.is_classic
        && note.length.to_micros()
            <= C::TOUCH_HOLD_HEAD_IGNORE.to_micros() + C::TOUCH_HOLD_TAIL_IGNORE.to_micros();
    !body_window_disabled
        && current_time.to_micros() >= body_check_start.to_micros()
        && current_time.to_micros() <= body_check_end.to_micros()
}

/// Ordinary-hold point adapters. Source order belongs to the immutable slice;
/// shared frontiers, queues and click counters are threaded by the kernel.
struct HoldScheduleFrame<'a> {
    input: &'a FrameInput,
    current_time: TimePoint,
    delta: Duration,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    prev_sensor: &'a SensorVec<bool>,
}

impl<'notes>
    hold_schedule_kernel::HoldSchedule<
        &'notes [(ButtonZone, HoldNote)],
        ButtonZone,
        HoldNote,
        ButtonVec<Natural>,
        crate::input_model::ButtonQueueVec<HoldNote>,
        ClickCursor,
        Vec<(ButtonZone, HoldNote)>,
        Vec<JudgeEvent>,
        JudgeEvent,
        Duration,
        SensorArea,
    > for HoldScheduleFrame<'_>
{
    fn length(&self, holds: &&'notes [(ButtonZone, HoldNote)]) -> usize {
        holds.len()
    }

    fn get(
        &self,
        holds: &&'notes [(ButtonZone, HoldNote)],
        index: usize,
    ) -> (ButtonZone, HoldNote) {
        holds[index].clone()
    }

    fn button_diff(&self, note: &HoldNote) -> Duration {
        self.current_time.clone() - note.params.effective_timing()
    }

    fn sensor_diff(&self, note: &HoldNote) -> Duration {
        time_point_sub_duration(self.current_time.clone(), self.touch_panel_offset.clone())
            - note.params.effective_timing()
    }

    fn button_pressed(&self, zone: &ButtonZone) -> bool {
        self.input.get_button_held(*zone)
    }

    fn sensor_pressed(&self, zone: &ButtonZone) -> bool {
        fallback_sensor_held_for_button_note(self.input, *zone)
    }

    fn prev_sensor_pressed(&self, zone: &ButtonZone) -> bool {
        fallback_prev_sensor_held_for_button_note(self.prev_sensor, *zone)
    }

    fn phase(&self, note: &HoldNote) -> hold_schedule_kernel::HoldPhase {
        match note.state {
            HoldSubState::HeadWaiting => hold_schedule_kernel::HoldPhase::HeadWaiting,
            HoldSubState::HeadJudgeable => hold_schedule_kernel::HoldPhase::HeadJudgeable,
            HoldSubState::HeadJudged(_) => hold_schedule_kernel::HoldPhase::HeadJudged,
            HoldSubState::BodyHeld => hold_schedule_kernel::HoldPhase::BodyHeld,
            HoldSubState::BodyReleased => hold_schedule_kernel::HoldPhase::BodyReleased,
            HoldSubState::Ended(_) => hold_schedule_kernel::HoldPhase::Ended,
        }
    }

    fn unlocked(&self, frontiers: &ButtonVec<Natural>, zone: &ButtonZone, note: &HoldNote) -> bool {
        button_queue_index_unlocked(frontiers, *zone, note.button_queue_index.clone())
    }

    fn eligible(&self, note: &HoldNote) -> bool {
        hold_head_eligible_for_click(note, self.current_time.clone())
    }

    fn fallback_area(&self, zone: &ButtonZone) -> SensorArea {
        fallback_sensor_area_for_button_note(*zone)
    }

    fn button_available(&self, cursor: &ClickCursor, zone: &ButtonZone) -> bool {
        has_unused_button_click_at(self.input, cursor, *zone)
    }

    fn sensor_available(&self, cursor: &ClickCursor, area: &SensorArea) -> bool {
        has_unused_sensor_click_at(self.input, cursor, *area)
    }

    fn use_button(&self, mut cursor: ClickCursor, zone: &ButtonZone) -> ClickCursor {
        let used = cursor.button_used.get_d(*zone, (0).into());
        cursor.button_used = cursor.button_used.set(*zone, used + 1);
        cursor
    }

    fn use_sensor(&self, mut cursor: ClickCursor, area: &SensorArea) -> ClickCursor {
        let used = cursor.sensor_used.get_d(*area, (0).into());
        cursor.sensor_used = cursor.sensor_used.set(*area, used + 1);
        cursor
    }

    fn step(
        &self,
        note: &HoldNote,
        diff: Duration,
        clicked: bool,
        pressed: bool,
        current_button_pressed: bool,
        prev_sensor_pressed: bool,
    ) -> (HoldNote, Option<JudgeEvent>) {
        hold_step(
            note,
            self.current_time.clone(),
            diff,
            C::HOLD_HEAD_IGNORE,
            C::HOLD_TAIL_IGNORE,
            clicked,
            pressed,
            current_button_pressed,
            prev_sensor_pressed,
            self.touch_panel_offset.clone(),
            None,
            self.delta.clone(),
            self.style,
        )
    }

    fn advance_frontier(
        &self,
        frontiers: ButtonVec<Natural>,
        zone: &ButtonZone,
    ) -> ButtonVec<Natural> {
        advance_shared_button_queue(&frontiers, *zone)
    }

    fn update_queue(
        &self,
        queues: crate::input_model::ButtonQueueVec<HoldNote>,
        zone: &ButtonZone,
        note: &HoldNote,
    ) -> crate::input_model::ButtonQueueVec<HoldNote> {
        // Composite boundary: replace matching note IDs, normalize resolved
        // heads, and store the zone queue. Separate queue-kernel work owns it.
        update_button_hold_queue(&queues, *zone, note)
    }

    fn append_active(
        &self,
        mut active: Vec<(ButtonZone, HoldNote)>,
        zone: ButtonZone,
        note: HoldNote,
    ) -> Vec<(ButtonZone, HoldNote)> {
        active.push((zone, note));
        active
    }

    fn append_event(&self, mut events: Vec<JudgeEvent>, event: JudgeEvent) -> Vec<JudgeEvent> {
        events.push(event);
        events
    }
}

// Tuple components preserve the order of the Lean state transition.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn process_hold_notes(
    frontiers: &ButtonVec<Natural>,
    queues: &crate::input_model::ButtonQueueVec<HoldNote>,
    holds: &[(ButtonZone, HoldNote)],
    input: &FrameInput,
    current_time: TimePoint,
    delta: Duration,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    prev_sensor: &SensorVec<bool>,
    cursor: &ClickCursor,
) -> (
    ButtonVec<Natural>,
    crate::input_model::ButtonQueueVec<HoldNote>,
    Vec<(ButtonZone, HoldNote)>,
    Vec<JudgeEvent>,
    ClickCursor,
) {
    let out = hold_schedule_kernel::process(
        &holds,
        HoldWork {
            frontiers: frontiers.clone(),
            queues: queues.clone(),
            cursor: cursor.clone(),
            active: Vec::new(),
            events: Vec::new(),
        },
        HoldScheduleFrame {
            input,
            current_time,
            delta,
            style,
            touch_panel_offset,
            prev_sensor,
        },
    );
    (
        out.frontiers,
        out.queues,
        out.active,
        out.events,
        out.cursor,
    )
}

// Installation fragment: import, new facade, and replacement function only.
// Existing queue/time/click helpers remain in scheduler.rs.
use lnmai_core_verify::touch_hold_schedule::{self as touch_hold_schedule_kernel, TouchHoldWork};

struct TouchHoldScheduleFrame<'a> {
    input: &'a FrameInput,
    current_time: TimePoint,
    delta: Duration,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
}

impl touch_hold_schedule_kernel::HeadGroups<Vec<GroupState>, Natural, JudgeGrade, Duration>
    for TouchHoldScheduleFrame<'_>
{
    fn head_length(&self, groups: &Vec<GroupState>) -> usize {
        groups.len()
    }

    fn head_matches(&self, groups: &Vec<GroupState>, index: usize, id: &Natural) -> bool {
        groups[index].group_id == *id
    }

    fn head_majority(&self, groups: &Vec<GroupState>, index: usize) -> bool {
        has_strict_majority(groups[index].count.clone(), groups[index].size.clone())
    }

    fn head_result(&self, groups: &Vec<GroupState>, index: usize) -> (JudgeGrade, Duration) {
        (groups[index].grade, groups[index].diff.clone())
    }

    fn head_increment(&self, mut groups: Vec<GroupState>, index: usize) -> Vec<GroupState> {
        groups[index].count = groups[index].count.clone() + 1;
        groups
    }

    fn head_set_size(
        &self,
        mut groups: Vec<GroupState>,
        index: usize,
        size: Natural,
    ) -> Vec<GroupState> {
        groups[index].size = size;
        groups
    }

    fn head_set_result(
        &self,
        mut groups: Vec<GroupState>,
        index: usize,
        grade: JudgeGrade,
        diff: Duration,
    ) -> Vec<GroupState> {
        groups[index].grade = grade;
        groups[index].diff = diff;
        groups
    }

    fn head_append(
        &self,
        mut groups: Vec<GroupState>,
        id: &Natural,
        size: Natural,
        grade: JudgeGrade,
        diff: Duration,
    ) -> Vec<GroupState> {
        groups.push(GroupState {
            group_id: id.clone(),
            count: 1.into(),
            size,
            grade,
            diff,
        });
        groups
    }
}

impl<'notes>
    touch_hold_schedule_kernel::TouchHoldSchedule<
        &'notes [(SensorArea, HoldNote)],
        SensorArea,
        HoldNote,
        SensorVec<Natural>,
        crate::input_model::SensorQueueVec<HoldNote>,
        ClickCursor,
        Vec<(SensorArea, HoldNote)>,
        Vec<JudgeEvent>,
        JudgeEvent,
        Duration,
        ButtonZone,
        Natural,
        JudgeGrade,
    > for TouchHoldScheduleFrame<'_>
{
    fn length(&self, holds: &&'notes [(SensorArea, HoldNote)]) -> usize {
        holds.len()
    }

    fn get(
        &self,
        holds: &&'notes [(SensorArea, HoldNote)],
        index: usize,
    ) -> (SensorArea, HoldNote) {
        holds[index].clone()
    }

    fn sensor_diff(&self, note: &HoldNote) -> Duration {
        time_point_sub_duration(self.current_time.clone(), self.touch_panel_offset.clone())
            - note.params.effective_timing()
    }

    fn button_diff(&self, note: &HoldNote) -> Duration {
        self.current_time.clone() - note.params.effective_timing()
    }

    fn local_pressed(&self, area: &SensorArea) -> bool {
        self.input.get_sensor_held(*area)
    }

    fn body_check_active(&self, note: &HoldNote) -> bool {
        touch_hold_body_check_active(note, self.current_time.clone())
    }

    fn body_group_id(&self, note: &HoldNote) -> Option<Natural> {
        note.touch_hold_group_id.clone()
    }

    fn head_group_id(&self, note: &HoldNote) -> Option<Natural> {
        note.touch_group_id.clone()
    }

    fn note_index(&self, note: &HoldNote) -> Natural {
        note.params.note_index.clone()
    }

    fn group_size(&self, note: &HoldNote) -> Natural {
        note.touch_group_size.clone()
    }

    fn phase(&self, note: &HoldNote) -> hold_schedule_kernel::HoldPhase {
        match note.state {
            HoldSubState::HeadWaiting => hold_schedule_kernel::HoldPhase::HeadWaiting,
            HoldSubState::HeadJudgeable => hold_schedule_kernel::HoldPhase::HeadJudgeable,
            HoldSubState::HeadJudged(_) => hold_schedule_kernel::HoldPhase::HeadJudged,
            HoldSubState::BodyHeld => hold_schedule_kernel::HoldPhase::BodyHeld,
            HoldSubState::BodyReleased => hold_schedule_kernel::HoldPhase::BodyReleased,
            HoldSubState::Ended(_) => hold_schedule_kernel::HoldPhase::Ended,
        }
    }

    fn head_grade(&self, note: &HoldNote) -> Option<JudgeGrade> {
        match note.state {
            HoldSubState::HeadJudged(grade) => Some(grade),
            _ => None,
        }
    }

    fn head_diff(&self, note: &HoldNote) -> Duration {
        note.head_diff.clone()
    }

    fn grade_rejected(&self, grade: &JudgeGrade) -> bool {
        grade.is_miss_or_too_fast()
    }

    fn within_head_window(&self, note: &HoldNote) -> bool {
        self.current_time.to_micros()
            <= time_point_add_duration(note.params.effective_timing(), C::TOUCH_GOOD).to_micros()
    }

    fn unlocked(&self, frontiers: &SensorVec<Natural>, area: &SensorArea, note: &HoldNote) -> bool {
        touch_queue_index_unlocked(frontiers, *area, note.touch_queue_index.clone())
    }

    fn eligible(&self, note: &HoldNote) -> bool {
        hold_head_eligible_for_click(note, self.current_time.clone())
    }

    fn button_ring(&self) -> bool {
        self.button_ring_for_touch
    }

    fn outer_button(&self, area: &SensorArea) -> Option<ButtonZone> {
        area.to_outer_button_zone()
    }

    fn button_available(&self, cursor: &ClickCursor, zone: &ButtonZone) -> bool {
        has_unused_button_click_at(self.input, cursor, *zone)
    }

    fn sensor_available(&self, cursor: &ClickCursor, area: &SensorArea) -> bool {
        has_unused_sensor_click_at(self.input, cursor, *area)
    }

    fn use_button(&self, mut cursor: ClickCursor, zone: &ButtonZone) -> ClickCursor {
        let used = cursor.button_used.get_d(*zone, 0.into());
        cursor.button_used = cursor.button_used.set(*zone, used + 1);
        cursor
    }

    fn use_sensor(&self, mut cursor: ClickCursor, area: &SensorArea) -> ClickCursor {
        let used = cursor.sensor_used.get_d(*area, 0.into());
        cursor.sensor_used = cursor.sensor_used.set(*area, used + 1);
        cursor
    }

    fn step(
        &self,
        note: &HoldNote,
        diff: Duration,
        clicked: bool,
        pressed: bool,
        shared: Option<(JudgeGrade, Duration)>,
    ) -> (HoldNote, Option<JudgeEvent>) {
        hold_step(
            note,
            self.current_time.clone(),
            diff,
            C::TOUCH_HOLD_HEAD_IGNORE,
            C::TOUCH_HOLD_TAIL_IGNORE,
            clicked,
            pressed,
            false,
            false,
            self.touch_panel_offset.clone(),
            shared,
            self.delta.clone(),
            self.style,
        )
    }

    fn advance_frontier(
        &self,
        frontiers: SensorVec<Natural>,
        area: &SensorArea,
    ) -> SensorVec<Natural> {
        advance_shared_touch_queue(&frontiers, *area)
    }

    fn update_queue(
        &self,
        queues: crate::input_model::SensorQueueVec<HoldNote>,
        area: &SensorArea,
        note: &HoldNote,
    ) -> crate::input_model::SensorQueueVec<HoldNote> {
        // This delegates to the installed replace_all + normalize + slot-store
        // kernels. Their composition/primitive laws are a separate proof boundary.
        update_sensor_hold_queue(&queues, *area, note)
    }

    fn append_active(
        &self,
        mut active: Vec<(SensorArea, HoldNote)>,
        area: SensorArea,
        note: HoldNote,
    ) -> Vec<(SensorArea, HoldNote)> {
        active.push((area, note));
        active
    }

    fn append_event(&self, mut events: Vec<JudgeEvent>, event: JudgeEvent) -> Vec<JudgeEvent> {
        events.push(event);
        events
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn process_touch_hold_notes(
    touch_frontiers: &SensorVec<Natural>,
    queues: &crate::input_model::SensorQueueVec<HoldNote>,
    holds: &[(SensorArea, HoldNote)],
    input: &FrameInput,
    current_time: TimePoint,
    delta: Duration,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
    cursor: &ClickCursor,
    touch_group_states: &[GroupState],
    touch_hold_body_groups: &[TouchHoldBodyGroupState],
) -> (
    SensorVec<Natural>,
    crate::input_model::SensorQueueVec<HoldNote>,
    Vec<(SensorArea, HoldNote)>,
    Vec<JudgeEvent>,
    ClickCursor,
    Vec<GroupState>,
    Vec<TouchHoldBodyGroupState>,
) {
    let out = touch_hold_schedule_kernel::process(
        &holds,
        TouchHoldWork {
            frontiers: touch_frontiers.clone(),
            queues: queues.clone(),
            cursor: cursor.clone(),
            active: Vec::new(),
            events: Vec::new(),
            groups: touch_group_states.to_vec(),
            body_groups: touch_hold_body_groups.to_vec(),
        },
        TouchHoldBodyAdapter {
            ops: TouchHoldScheduleFrame {
                input,
                current_time,
                delta,
                style,
                touch_panel_offset,
                button_ring_for_touch,
            },
        },
    );
    (
        out.frontiers,
        out.queues,
        out.active,
        out.events,
        out.cursor,
        out.groups,
        out.body_groups,
    )
}

fn touch_queue_index_unlocked(
    frontiers: &SensorVec<Natural>,
    area: SensorArea,
    index: Natural,
) -> bool {
    index <= frontiers.get_d(area, (0).into())
}

fn advance_shared_touch_queue(
    frontiers: &SensorVec<Natural>,
    area: SensorArea,
) -> SensorVec<Natural> {
    frontiers.set(area, frontiers.get_d(area, (0).into()) + 1)
}

fn touch_queue_resolved(note: &TouchNote) -> bool {
    matches!(note.state, TouchState::Judged(_) | TouchState::Ended)
}

struct TouchQueueMaintenance;

impl queue_maintenance_kernel::QueueCursor<ZoneQueue<TouchNote>, TouchNote>
    for TouchQueueMaintenance
{
    fn peek(&self, queue: &ZoneQueue<TouchNote>) -> Option<TouchNote> {
        queue.peek()
    }

    fn resolved(&self, note: &TouchNote) -> bool {
        touch_queue_resolved(note)
    }

    fn advance(&self, mut queue: ZoneQueue<TouchNote>) -> ZoneQueue<TouchNote> {
        queue.current_index += 1;
        queue
    }
}

impl queue_maintenance_kernel::QueueReplace<ZoneQueue<TouchNote>, TouchNote>
    for TouchQueueMaintenance
{
    fn get(&self, queue: &ZoneQueue<TouchNote>, index: usize) -> TouchNote {
        queue.notes[index].clone()
    }

    fn index_equal(&self, existing: &TouchNote, replacement: &TouchNote) -> bool {
        existing.params.note_index == replacement.params.note_index
    }

    fn write(
        &self,
        mut queue: ZoneQueue<TouchNote>,
        index: usize,
        replacement: &TouchNote,
    ) -> ZoneQueue<TouchNote> {
        queue.notes[index] = replacement.clone();
        queue
    }
}

fn normalize_touch_queue_cursor(queue: &ZoneQueue<TouchNote>) -> ZoneQueue<TouchNote> {
    // Fuel sufficiency against Lean's remaining + 1 is still to be proved.
    let len = queue_remaining_kernel::vec_len(&queue.notes);
    let converted = queue.current_index.to_usize();
    let remaining = queue_remaining_kernel::remaining_scalar(len, converted);
    queue_maintenance_kernel::normalize_cursor_fuel(remaining, queue.clone(), TouchQueueMaintenance)
}

// Proposed insertion in runtime scheduler.rs immediately before
// process_touch_queue_head_fuel. Imports/module registration are in the patch.

struct TouchHeadSchedule<'a> {
    area: SensorArea,
    input: &'a FrameInput,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
}

impl
    touch_schedule_kernel::TouchSchedule<
        ZoneQueue<TouchNote>,
        TouchNote,
        SensorVec<Natural>,
        ClickCursor,
        Vec<GroupState>,
        Vec<JudgeEvent>,
        JudgeEvent,
        TimePoint,
        Duration,
        SensorArea,
        ButtonZone,
        Natural,
        JudgeGrade,
        TouchQueueMaintenance,
    > for TouchHeadSchedule<'_>
{
    fn queue_ops(&self) -> TouchQueueMaintenance {
        TouchQueueMaintenance
    }

    fn queue_remaining(&self, queue: &ZoneQueue<TouchNote>) -> usize {
        let len = queue_remaining_kernel::vec_len(&queue.notes);
        let converted = queue.current_index.to_usize();
        queue_remaining_kernel::remaining_scalar(len, converted)
    }

    fn queue_len(&self, queue: &ZoneQueue<TouchNote>) -> usize {
        queue_remaining_kernel::vec_len(&queue.notes)
    }

    fn timing(&self, note: &TouchNote) -> TimePoint {
        note.params.effective_timing()
    }

    fn sensor_diff(&self, timing: &TimePoint) -> Duration {
        time_point_sub_duration(self.current_time.clone(), self.touch_panel_offset.clone())
            - timing.clone()
    }

    fn button_diff(&self, timing: TimePoint) -> Duration {
        self.current_time.clone() - timing
    }

    fn group_id(&self, note: &TouchNote) -> Option<Natural> {
        note.touch_group_id.clone()
    }

    fn group_size(&self, note: &TouchNote) -> Natural {
        note.touch_group_size.clone()
    }

    fn shared_result(
        &self,
        groups: &Vec<GroupState>,
        id: Natural,
    ) -> Option<(JudgeGrade, Duration)> {
        group_share_result(groups, id)
    }

    fn eligible(&self, note: &TouchNote) -> bool {
        touch_click_can_engage_judge(
            note,
            self.current_time.clone(),
            self.touch_panel_offset.clone(),
        )
    }

    fn unlocked(&self, frontiers: &SensorVec<Natural>, note: &TouchNote) -> bool {
        touch_queue_index_unlocked(frontiers, self.area, note.touch_queue_index.clone())
    }

    fn button_ring_for_touch(&self) -> bool {
        self.button_ring_for_touch
    }

    fn sensor_area(&self, note: &TouchNote) -> SensorArea {
        note.sensor_pos
    }

    fn outer_button(&self, area: &SensorArea) -> Option<ButtonZone> {
        area.to_outer_button_zone()
    }

    fn button_available(&self, cursor: &ClickCursor, zone: &ButtonZone) -> bool {
        has_unused_button_click_at(self.input, cursor, *zone)
    }

    fn sensor_available(&self, cursor: &ClickCursor, area: &SensorArea) -> bool {
        has_unused_sensor_click_at(self.input, cursor, *area)
    }

    fn use_button(&self, mut cursor: ClickCursor, zone: &ButtonZone) -> ClickCursor {
        let used = cursor.button_used.get_d(*zone, (0).into());
        cursor.button_used = cursor.button_used.set(*zone, used + 1);
        cursor
    }

    fn use_sensor(&self, mut cursor: ClickCursor, area: &SensorArea) -> ClickCursor {
        let used = cursor.sensor_used.get_d(*area, (0).into());
        cursor.sensor_used = cursor.sensor_used.set(*area, used + 1);
        cursor
    }

    fn step(
        &self,
        note: &TouchNote,
        diff: &Duration,
        clicked: bool,
        shared: Option<(JudgeGrade, Duration)>,
    ) -> (TouchNote, Option<JudgeEvent>) {
        touch_step(
            note,
            self.current_time.clone(),
            diff.clone(),
            clicked,
            shared,
            self.style,
        )
    }

    fn ended(&self, note: &TouchNote) -> bool {
        matches!(note.state, TouchState::Ended)
    }

    fn event_grade(&self, event: &JudgeEvent) -> JudgeGrade {
        event.grade
    }

    fn is_miss_or_too_fast(&self, grade: &JudgeGrade) -> bool {
        grade.is_miss_or_too_fast()
    }

    fn prepend_event(
        &self,
        mut events_rev: Vec<JudgeEvent>,
        event: &JudgeEvent,
    ) -> Vec<JudgeEvent> {
        events_rev.insert(0, event.clone());
        events_rev
    }

    fn register_group(
        &self,
        groups: Vec<GroupState>,
        id: Natural,
        size: Natural,
        grade: JudgeGrade,
        diff: Duration,
    ) -> Vec<GroupState> {
        register_touch_group_result(&groups, id, size, grade, diff)
    }

    fn advance_frontier(&self, frontiers: SensorVec<Natural>) -> SensorVec<Natural> {
        advance_shared_touch_queue(&frontiers, self.area)
    }
}

#[allow(clippy::too_many_arguments)]
fn process_touch_queue_head_fuel(
    fuel: usize,
    area: SensorArea,
    queue: &ZoneQueue<TouchNote>,
    frontiers: &SensorVec<Natural>,
    input: &FrameInput,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
    groups: &[GroupState],
    evs_rev: &[JudgeEvent],
) -> (
    ZoneQueue<TouchNote>,
    SensorVec<Natural>,
    ClickCursor,
    Vec<GroupState>,
    Vec<JudgeEvent>,
) {
    let work = touch_schedule_kernel::process(
        fuel,
        TouchWork {
            queue: queue.clone(),
            frontiers: frontiers.clone(),
            cursor: cursor.clone(),
            groups: groups.to_vec(),
            events_rev: evs_rev.to_vec(),
        },
        TouchHeadSchedule {
            area,
            input,
            current_time,
            style,
            touch_panel_offset,
            button_ring_for_touch,
        },
    );
    (
        work.queue,
        work.frontiers,
        work.cursor,
        work.groups,
        work.events_rev,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_touch_queue_head(
    area: SensorArea,
    queue: &ZoneQueue<TouchNote>,
    frontiers: &SensorVec<Natural>,
    input: &FrameInput,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
    groups: &[GroupState],
    evs_rev: &[JudgeEvent],
) -> (
    ZoneQueue<TouchNote>,
    SensorVec<Natural>,
    ClickCursor,
    Vec<GroupState>,
    Vec<JudgeEvent>,
) {
    let len = queue_remaining_kernel::vec_len(&queue.notes);
    let converted = queue.current_index.to_usize();
    let fuel = queue_remaining_kernel::head_fuel_scalar(len, converted);
    process_touch_queue_head_fuel(
        fuel,
        area,
        queue,
        frontiers,
        input,
        current_time,
        style,
        cursor,
        touch_panel_offset,
        button_ring_for_touch,
        groups,
        evs_rev,
    )
}

/// Immutable group results and frame timing for the automatic Touch pass.
struct TouchAutomaticFrame<'a> {
    area: SensorArea,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    groups: &'a [GroupState],
}

impl
    lnmai_core_verify::touch_automatic::TouchAutomatic<
        ZoneQueue<TouchNote>,
        TouchNote,
        Vec<TouchNote>,
        SensorVec<Natural>,
        Vec<JudgeEvent>,
        JudgeEvent,
        Duration,
        (JudgeGrade, Duration),
    > for TouchAutomaticFrame<'_>
{
    fn length(&self, source: &ZoneQueue<TouchNote>) -> usize {
        source.notes.len()
    }

    fn get(&self, source: &ZoneQueue<TouchNote>, index: usize) -> TouchNote {
        source.notes[index].clone()
    }

    fn before_cursor(&self, source: &ZoneQueue<TouchNote>, index: usize) -> bool {
        index < source.current_index
    }

    fn resolved(&self, note: &TouchNote) -> bool {
        touch_queue_resolved(note)
    }

    fn sensor_diff(&self, note: &TouchNote) -> Duration {
        time_point_sub_duration(self.current_time.clone(), self.touch_panel_offset.clone())
            - note.params.effective_timing()
    }

    fn shared(&self, note: &TouchNote) -> Option<(JudgeGrade, Duration)> {
        match note.touch_group_id.clone() {
            Some(group_id) => group_share_result(self.groups, group_id),
            None => None,
        }
    }

    fn too_late(&self, note: &TouchNote) -> bool {
        self.current_time.to_micros()
            > time_point_add_duration(note.params.effective_timing(), C::TOUCH_GOOD).to_micros()
    }

    fn step(
        &self,
        note: &TouchNote,
        diff: Duration,
        shared: Option<(JudgeGrade, Duration)>,
    ) -> (TouchNote, Option<JudgeEvent>) {
        touch_step(
            note,
            self.current_time.clone(),
            diff,
            false,
            shared,
            self.style,
        )
    }

    fn advance_frontier(&self, frontiers: SensorVec<Natural>) -> SensorVec<Natural> {
        advance_shared_touch_queue(&frontiers, self.area)
    }

    fn prepend_event(&self, mut events: Vec<JudgeEvent>, event: JudgeEvent) -> Vec<JudgeEvent> {
        events.insert(0, event);
        events
    }

    fn append_note(&self, mut notes: Vec<TouchNote>, note: TouchNote) -> Vec<TouchNote> {
        notes.push(note);
        notes
    }
}

#[allow(clippy::too_many_arguments)]
fn process_touch_queue_automatic(
    area: SensorArea,
    queue: &ZoneQueue<TouchNote>,
    frontiers: &SensorVec<Natural>,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    touch_panel_offset: Duration,
    groups: &[GroupState],
    evs_rev: &[JudgeEvent],
) -> (
    ZoneQueue<TouchNote>,
    SensorVec<Natural>,
    Vec<GroupState>,
    Vec<JudgeEvent>,
) {
    let out = lnmai_core_verify::touch_automatic::process(
        queue,
        lnmai_core_verify::touch_automatic::TouchAutomaticWork {
            notes: Vec::new(),
            frontiers: frontiers.clone(),
            events_rev: evs_rev.to_vec(),
        },
        TouchAutomaticFrame {
            area,
            current_time,
            style,
            touch_panel_offset,
            groups,
        },
    );
    let queue = normalize_touch_queue_cursor(&ZoneQueue {
        notes: out.notes,
        ..queue.clone()
    });
    (queue, out.frontiers, groups.to_vec(), out.events_rev)
}

#[allow(clippy::too_many_arguments)]
fn process_touch_queue(
    area: SensorArea,
    queue: &ZoneQueue<TouchNote>,
    frontiers: &SensorVec<Natural>,
    input: &FrameInput,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
    groups: &[GroupState],
    evs_rev: &[JudgeEvent],
) -> (
    ZoneQueue<TouchNote>,
    SensorVec<Natural>,
    ClickCursor,
    Vec<GroupState>,
    Vec<JudgeEvent>,
) {
    let (queue1, frontiers1, cursor1, groups1, evs1) = process_touch_queue_head(
        area,
        queue,
        frontiers,
        input,
        current_time.clone(),
        style,
        cursor,
        touch_panel_offset.clone(),
        button_ring_for_touch,
        groups,
        evs_rev,
    );
    let (queue2, frontiers2, groups2, evs2) = process_touch_queue_automatic(
        area,
        &queue1,
        &frontiers1,
        current_time,
        style,
        touch_panel_offset,
        &groups1,
        &evs1,
    );
    (queue2, frontiers2, cursor1, groups2, evs2)
}

#[allow(clippy::too_many_arguments)]
fn process_touch_notes(
    frontiers: &SensorVec<Natural>,
    queues: &crate::input_model::SensorQueueVec<TouchNote>,
    input: &FrameInput,
    current_time: TimePoint,
    style: crate::types::JudgeStyle,
    cursor: &ClickCursor,
    touch_panel_offset: Duration,
    button_ring_for_touch: bool,
    group_states: &[GroupState],
) -> (
    SensorVec<Natural>,
    crate::input_model::SensorQueueVec<TouchNote>,
    Vec<JudgeEvent>,
    ClickCursor,
    Vec<GroupState>,
) {
    let (next_queues, (frontiers2, cursor2, groups2, mut evs_rev)) = queues.map_accum(
        (
            frontiers.clone(),
            cursor.clone(),
            group_states.to_vec(),
            Vec::<JudgeEvent>::new(),
        ),
        |area, q, state| {
            let (frontiers, cursor, groups, evs_rev) = state;
            let (next_queue, frontiers2, cursor2, groups2, evs_rev2) = process_touch_queue(
                area,
                &q,
                &frontiers,
                input,
                current_time.clone(),
                style,
                &cursor,
                touch_panel_offset.clone(),
                button_ring_for_touch,
                &groups,
                &evs_rev,
            );
            (next_queue, (frontiers2, cursor2, groups2, evs_rev2))
        },
    );
    evs_rev.reverse();
    (frontiers2, next_queues, evs_rev, cursor2, groups2)
}

// ---------------------------------------------------------------------------
// Slide processing
// ---------------------------------------------------------------------------

/// Per-operation adapters for the shared slide traversal. All calls see the
/// same frame snapshot; parent operations receive the complete live list.
struct SlideScheduleFrame<'a> {
    input: &'a FrameInput,
    current_time: TimePoint,
    touch_panel_offset: Duration,
    delta: Duration,
    style: crate::types::JudgeStyle,
    subdivide_slide_judge_grade: bool,
}

impl
    slide_schedule_kernel::SlideSchedule<
        Vec<SlideNote>,
        SlideNote,
        Vec<JudgeEvent>,
        Vec<AudioCommand>,
        Vec<RenderCommand>,
    > for SlideScheduleFrame<'_>
{
    fn read(&self, slides: &Vec<SlideNote>, index: usize) -> SlideNote {
        slides[index].clone()
    }

    fn step(
        &self,
        note: &SlideNote,
    ) -> (
        SlideNote,
        Vec<JudgeEvent>,
        Vec<AudioCommand>,
        Vec<RenderCommand>,
    ) {
        let (after, event, audio, render) = slide_step(
            note,
            self.current_time.clone(),
            &self.input.sensor_held,
            self.touch_panel_offset.clone(),
            self.delta.clone(),
            self.style,
            self.subdivide_slide_judge_grade,
        );
        (after, event.into_iter().collect(), audio, render)
    }

    fn replace(
        &self,
        mut slides: Vec<SlideNote>,
        index: usize,
        note: &SlideNote,
    ) -> Vec<SlideNote> {
        slides[index] = note.clone();
        slides
    }

    fn force_direct(
        &self,
        mut slides: Vec<SlideNote>,
        before: &SlideNote,
        after: &SlideNote,
    ) -> (Vec<SlideNote>, Vec<RenderCommand>) {
        let commands = force_finish_direct_parent(&mut slides, before, after);
        (slides, commands)
    }

    fn end_ancestors(
        &self,
        mut slides: Vec<SlideNote>,
        before: &SlideNote,
        after: &SlideNote,
    ) -> (Vec<SlideNote>, Vec<RenderCommand>) {
        let commands = end_slide_ancestors(&mut slides, before, after);
        (slides, commands)
    }

    fn refresh(&self, slides: Vec<SlideNote>) -> Vec<SlideNote> {
        update_slide_parent_flags(&slides)
    }

    fn append_events(
        &self,
        mut accumulated: Vec<JudgeEvent>,
        batch: Vec<JudgeEvent>,
    ) -> Vec<JudgeEvent> {
        accumulated.extend(batch);
        accumulated
    }

    fn append_audio(
        &self,
        mut accumulated: Vec<AudioCommand>,
        batch: Vec<AudioCommand>,
    ) -> Vec<AudioCommand> {
        accumulated.extend(batch);
        accumulated
    }

    fn append_render(
        &self,
        mut accumulated: Vec<RenderCommand>,
        batch: Vec<RenderCommand>,
    ) -> Vec<RenderCommand> {
        accumulated.extend(batch);
        accumulated
    }
}

/// `processSlideNotesCore`: the prefix and existing outputs are supplied in
/// reverse order. Only pending bodies are stepped, while parent effects may
/// update bodies in either part of the list.
#[allow(clippy::too_many_arguments)]
pub fn process_slide_notes_core(
    processed_rev: &[SlideNote],
    pending: &[SlideNote],
    input: &FrameInput,
    current_time: TimePoint,
    touch_panel_offset: Duration,
    delta: Duration,
    style: crate::types::JudgeStyle,
    subdivide_slide_judge_grade: bool,
    events_rev: &[JudgeEvent],
    audio_rev: &[AudioCommand],
    render_rev: &[RenderCommand],
) -> (
    Vec<SlideNote>,
    Vec<JudgeEvent>,
    Vec<AudioCommand>,
    Vec<RenderCommand>,
) {
    let mut slides: Vec<_> = processed_rev.iter().rev().cloned().collect();
    slides.extend_from_slice(pending);
    let work = slide_schedule_kernel::process(
        slides.len(),
        SlideWork {
            slides,
            events: events_rev.iter().rev().cloned().collect(),
            audio: audio_rev.iter().rev().cloned().collect(),
            render: render_rev.iter().rev().cloned().collect(),
            index: processed_rev.len(),
        },
        SlideScheduleFrame {
            input,
            current_time,
            touch_panel_offset,
            delta,
            style,
            subdivide_slide_judge_grade,
        },
    );
    (work.slides, work.events, work.audio, work.render)
}

fn events_to_audio_commands(events: &[JudgeEvent], time_point: TimePoint) -> Vec<AudioCommand> {
    frame_commands::collect_indexed(events.len(), |index| {
        let evt = &events[index];
        let kind = frame_commands::event_audio_kind(evt.kind, evt.phase, evt.grade, evt.position)?;
        Some(AudioCommand::PlayJudgeSfx {
            kind,
            grade: evt.grade,
            is_break: evt.is_break,
            at_time: time_point.clone(),
            note_index: evt.note_index.clone(),
        })
    })
}

fn events_to_render_commands(events: &[JudgeEvent], display_hold_head: bool) -> Vec<RenderCommand> {
    frame_commands::collect_indexed(events.len(), |index| {
        let evt = &events[index];
        if frame_commands::event_render_visible(evt.kind, evt.phase, display_hold_head) {
            Some(RenderCommand::ShowJudgeResult {
                kind: evt.kind,
                grade: evt.grade,
                is_break: evt.is_break,
                diff: evt.diff.clone(),
                note_index: evt.note_index.clone(),
            })
        } else {
            None
        }
    })
}

/// Accumulator passed through the extracted five-pass driver. Shared frontiers,
/// click ownership, and touch groups live in one state rather than snapshots.
struct FrameWork {
    state: GameState,
    cursor: ClickCursor,
    events: FrameEvents<Vec<JudgeEvent>>,
    slide_audio: Vec<AudioCommand>,
    slide_render: Vec<RenderCommand>,
}

fn process_frame_phase(phase: FramePhase, mut frame: FrameWork, input: &FrameInput) -> FrameWork {
    let st = &mut frame.state;
    match phase {
        FramePhase::Tap => {
            (
                st.button_queue_frontiers,
                st.tap_queues,
                frame.events.tap,
                frame.cursor,
            ) = process_tap_notes(
                &st.button_queue_frontiers,
                &st.tap_queues,
                input,
                st.current_time.clone(),
                st.touch_panel_offset.clone(),
                st.judge_style,
                &frame.cursor,
            );
        }
        FramePhase::Hold => {
            (
                st.button_queue_frontiers,
                st.hold_queues,
                st.active_holds,
                frame.events.hold,
                frame.cursor,
            ) = process_hold_notes(
                &st.button_queue_frontiers,
                &st.hold_queues,
                &st.active_holds,
                input,
                st.current_time.clone(),
                input.delta.clone(),
                st.judge_style,
                st.touch_panel_offset.clone(),
                &st.prev_sensor,
                &frame.cursor,
            );
        }
        FramePhase::Slide => {
            (
                st.slides,
                frame.events.slide,
                frame.slide_audio,
                frame.slide_render,
            ) = process_slide_notes_core(
                &[],
                &st.slides,
                input,
                st.current_time.clone(),
                st.touch_panel_offset.clone(),
                input.delta.clone(),
                st.judge_style,
                st.subdivide_slide_judge_grade,
                &[],
                &[],
                &[],
            );
        }
        FramePhase::Touch => {
            (
                st.touch_queue_frontiers,
                st.touch_queues,
                frame.events.touch,
                frame.cursor,
                st.touch_group_states,
            ) = process_touch_notes(
                &st.touch_queue_frontiers,
                &st.touch_queues,
                input,
                st.current_time.clone(),
                st.judge_style,
                &frame.cursor,
                st.touch_panel_offset.clone(),
                st.button_ring_for_touch,
                &st.touch_group_states,
            );
        }
        FramePhase::TouchHold => {
            (
                st.touch_queue_frontiers,
                st.touch_hold_queues,
                st.active_touch_holds,
                frame.events.touch_hold,
                frame.cursor,
                st.touch_group_states,
                st.touch_hold_group_states,
            ) = process_touch_hold_notes(
                &st.touch_queue_frontiers,
                &st.touch_hold_queues,
                &st.active_touch_holds,
                input,
                st.current_time.clone(),
                input.delta.clone(),
                st.judge_style,
                st.touch_panel_offset.clone(),
                st.button_ring_for_touch,
                &frame.cursor,
                &st.touch_group_states,
                &st.touch_hold_group_states,
            );
        }
    }
    frame
}

/// Adapter for the opaque batches in the ordering kernels. Moving each suffix
/// preserves its internal order and avoids cloning event/command payloads.
fn append_frame_batch<T>(mut prefix: Vec<T>, mut suffix: Vec<T>) -> Vec<T> {
    prefix.append(&mut suffix);
    prefix
}

/// `stepFrame`.
pub fn step_frame(
    st: &GameState,
    input: &FrameInput,
) -> (
    GameState,
    Vec<JudgeEvent>,
    Vec<AudioCommand>,
    Vec<RenderCommand>,
) {
    let new_time = time_point_add_duration(st.current_time.clone(), input.delta.clone());
    let resolved_slides = update_slide_parent_flags(&st.slides);
    let touch_hold_body_groups = if st.touch_hold_group_states.is_empty() {
        touch_hold_body_group_states_from_holds(&st.active_touch_holds)
    } else {
        st.touch_hold_group_states.clone()
    };
    let frame = scheduler_kernel::process_frame(
        FrameWork {
            state: GameState {
                current_time: new_time.clone(),
                slides: resolved_slides,
                touch_hold_group_states: touch_hold_body_groups,
                ..st.clone()
            },
            cursor: ClickCursor::with_field_defaults(),
            events: FrameEvents {
                tap: Vec::new(),
                hold: Vec::new(),
                slide: Vec::new(),
                touch: Vec::new(),
                touch_hold: Vec::new(),
            },
            slide_audio: Vec::new(),
            slide_render: Vec::new(),
        },
        |phase, frame| process_frame_phase(phase, frame, input),
    );
    let mut next_state = frame.state;
    let all_events = scheduler_kernel::combine_events(frame.events, append_frame_batch);
    let new_score = fold_events_into_score(
        st.note_fast_late_display,
        st.break_fast_late_display,
        &st.score,
        &all_events,
    );
    let body_audio =
        frame_commands::collect_indexed(next_state.active_touch_holds.len(), |index| {
            let (area, note) = &next_state.active_touch_holds[index];
            let grouped_pressed = note.touch_hold_group_id.clone().is_some_and(|id| {
                touch_hold_body_group_majority_pressed(&next_state.touch_hold_group_states, id)
            });
            if frame_commands::touch_hold_body_active(
                note.state,
                input.get_sensor_held(*area),
                grouped_pressed,
            ) {
                Some(AudioCommand::PlayTouchHoldBody {
                    note_index: note.params.note_index.clone(),
                    at_time: new_time.clone(),
                })
            } else {
                None
            }
        });
    let (audio_commands, render_commands) = scheduler_kernel::combine_commands(
        frame.slide_audio,
        body_audio,
        events_to_audio_commands(&all_events, new_time),
        frame.slide_render,
        events_to_render_commands(&all_events, st.display_hold_head_judge_result),
        append_frame_batch,
        append_frame_batch,
    );
    next_state.prev_button = input.button_held.clone();
    next_state.prev_sensor = input.sensor_held.clone();
    next_state.score = new_score;
    (next_state, all_events, audio_commands, render_commands)
}

/// `isTerminated`.
pub fn is_terminated(st: &GameState) -> bool {
    st.active_holds.is_empty()
        && st.active_touch_holds.is_empty()
        && st
            .slides
            .iter()
            .all(|s| matches!(s.state, SlideState::Ended))
        && st
            .tap_queues
            .data
            .iter()
            .all(|q| q.current_index >= q.notes.len())
        && st
            .hold_queues
            .data
            .iter()
            .all(|q| q.current_index >= q.notes.len())
        && st
            .touch_queues
            .data
            .iter()
            .all(|q| q.current_index >= q.notes.len())
        && st
            .touch_hold_queues
            .data
            .iter()
            .all(|q| q.current_index >= q.notes.len())
}

/// `stepFrameTimed`.
pub fn step_frame_timed(
    st: &GameState,
    batch: &TimedInputBatch,
) -> (
    GameState,
    Vec<JudgeEvent>,
    Vec<AudioCommand>,
    Vec<RenderCommand>,
) {
    let delta = batch.current_time.clone() - st.current_time.clone();
    let input = batch.to_frame_input(delta, st.prev_button.clone(), st.prev_sensor.clone());
    let st_with_batch = GameState {
        current_batch: batch.clone(),
        ..st.clone()
    };
    let (next_state, events, audio, render) = step_frame(&st_with_batch, &input);
    (
        GameState {
            current_batch: batch.clone(),
            ..next_state
        },
        events,
        audio,
        render,
    )
}

/// `probeTouchHeadAt`. The probe uses the Lean diagnostic's sensor-only click
/// policy and broad touch window, independently of the frame's touch pass.
pub fn probe_touch_head_at(
    st: &GameState,
    input: &FrameInput,
    area: SensorArea,
) -> Option<TouchHeadProbe> {
    let new_time = st.current_time.clone() + input.delta.clone();
    let (button_frontiers, _, _, cursor_tap) = process_tap_notes(
        &st.button_queue_frontiers,
        &st.tap_queues,
        input,
        new_time.clone(),
        st.touch_panel_offset.clone(),
        st.judge_style,
        &ClickCursor::with_field_defaults(),
    );
    let (_, _, _, _, cursor) = process_hold_notes(
        &button_frontiers,
        &st.hold_queues,
        &st.active_holds,
        input,
        new_time.clone(),
        input.delta.clone(),
        st.judge_style,
        st.touch_panel_offset.clone(),
        &st.prev_sensor,
        &cursor_tap,
    );
    let queue = st.touch_queues.get_d(area, ZoneQueue::default());
    let note = queue.peek()?;
    let diff = time_point_sub_duration(new_time.clone(), st.touch_panel_offset.clone())
        - note.params.effective_timing();
    let shared_result = note
        .touch_group_id
        .clone()
        .and_then(|id| group_share_result(&st.touch_group_states, id));
    let can_consume_click = shared_result.is_none()
        && touch_eligible_for_click(&note, new_time.clone())
        && touch_queue_index_unlocked(
            &st.touch_queue_frontiers,
            area,
            note.touch_queue_index.clone(),
        );
    let used_sensor =
        can_consume_click && try_use_sensor_click_at(input, &cursor, note.sensor_pos).0;
    let (new_note, event) = touch_step(
        &note,
        new_time,
        diff.clone(),
        used_sensor,
        shared_result.clone(),
        st.judge_style,
    );
    Some(TouchHeadProbe {
        area,
        note_index: note.params.note_index,
        frontier: st.touch_queue_frontiers.get_d(area, 0.into()),
        queue_current_index: queue.current_index,
        state_before: note.state,
        cursor_sensor_used_before: cursor.sensor_used.get_d(note.sensor_pos, 0.into()),
        input_sensor_click_count: input.get_sensor_click_count(note.sensor_pos),
        can_consume_click,
        used_button: false,
        used_sensor,
        clicked: used_sensor,
        shared_result,
        diff,
        state_after: new_note.state,
        event,
    })
}

/// `probeTapHoldSensorConsumers`. Its final reversal also reverses the tap
/// prefix passed into the hold accumulator; preserve that observable order.
pub fn probe_tap_hold_sensor_consumers(
    st: &GameState,
    input: &FrameInput,
) -> Vec<SensorClickConsumerProbe> {
    let new_time = st.current_time.clone() + input.delta.clone();
    let mut cursor = ClickCursor::with_field_defaults();
    let mut frontiers = st.button_queue_frontiers.clone();
    let mut consumers = Vec::new();
    for zone in crate::storage::button_zone_storage_order() {
        let mut queue = st.tap_queues.get_d(zone, ZoneQueue::default());
        while let Some(note) = queue.peek() {
            let timing = note.params().effective_timing();
            let sensor_diff =
                time_point_sub_duration(new_time.clone(), st.touch_panel_offset.clone())
                    - timing.clone();
            let can_consume_click = tap_eligible_for_click(&note, new_time.clone())
                && button_queue_index_unlocked(&frontiers, zone, note.button_queue_index());
            let fallback_area = fallback_sensor_area_for_button_note(note.lane().to_button_zone());
            let (used_button, used_sensor, next_cursor) = consume_button_then_fallback_sensor(
                input,
                &cursor,
                can_consume_click,
                zone,
                fallback_area,
            );
            if used_sensor {
                consumers.push(SensorClickConsumerProbe {
                    kind: SensorClickConsumerKind::Tap,
                    note_index: note.params().note_index.clone(),
                    zone,
                    sensor_area: fallback_area,
                    queue_index: note.button_queue_index(),
                    frontier: frontiers.get_d(zone, 0.into()),
                    diff: sensor_diff.clone(),
                });
            }
            let diff = if used_button {
                new_time.clone() - timing
            } else {
                sensor_diff
            };
            let (new_note, _) = tap_family_step(
                &note,
                new_time.clone(),
                diff,
                used_button || used_sensor,
                st.judge_style,
            );
            cursor = next_cursor;
            if !matches!(new_note.state(), TapState::Ended) {
                break;
            }
            queue = queue.advance();
            frontiers = advance_shared_button_queue(&frontiers, zone);
            if !tap_family_queue_has_unused_click(input, &cursor, zone, fallback_area) {
                break;
            }
        }
    }
    // Lean passes the forward tap list as the hold pass's reverse accumulator.
    for (zone, note) in &st.active_holds {
        let zone = *zone;
        let timing = note.params.effective_timing();
        let sensor_diff = time_point_sub_duration(new_time.clone(), st.touch_panel_offset.clone())
            - timing.clone();
        let allow_input = hold_head_awaits_resolution(note.state)
            && button_queue_index_unlocked(&frontiers, zone, note.button_queue_index.clone());
        let fallback_area = fallback_sensor_area_for_button_note(zone);
        let (used_button, used_sensor, next_cursor) =
            consume_button_then_fallback_sensor(input, &cursor, allow_input, zone, fallback_area);
        if used_sensor {
            consumers.insert(
                0,
                SensorClickConsumerProbe {
                    kind: SensorClickConsumerKind::Hold,
                    note_index: note.params.note_index.clone(),
                    zone,
                    sensor_area: fallback_area,
                    queue_index: note.button_queue_index.clone(),
                    frontier: frontiers.get_d(zone, 0.into()),
                    diff: sensor_diff.clone(),
                },
            );
        }
        let diff = if used_button {
            new_time.clone() - timing
        } else {
            sensor_diff
        };
        let (new_note, _) = hold_step(
            note,
            new_time.clone(),
            diff,
            C::HOLD_HEAD_IGNORE,
            C::HOLD_TAIL_IGNORE,
            used_button || used_sensor,
            hold_body_pressed_from_button_or_sensor(
                input.get_button_held(zone),
                fallback_sensor_held_for_button_note(input, zone),
            ),
            input.get_button_held(zone),
            fallback_prev_sensor_held_for_button_note(&st.prev_sensor, zone),
            st.touch_panel_offset.clone(),
            None,
            input.delta.clone(),
            st.judge_style,
        );
        if entered_head_judged(note.state, new_note.state) {
            frontiers = advance_shared_button_queue(&frontiers, zone);
        }
        cursor = next_cursor;
    }
    consumers.reverse();
    consumers
}

#[allow(dead_code)]
fn _unused_runtime_score(_s: RuntimeScoreState) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_loader::ChartSpec;
    use crate::input_model::TimedInputEvent;
    use crate::simai::frontend::frontend_lowered_chart;

    const MAIDATA: &str = "&first=0\n&inote_1=\n(120)\n1,2,3,\n";

    #[test]
    fn step_frame_advances_time_and_scores_misses() {
        let spec: ChartSpec = frontend_lowered_chart(MAIDATA, (1).into()).unwrap();
        let state = crate::chart_loader::build_game_state(&spec);
        // Jump far past all notes with no input → they should miss.
        let mut input = FrameInput {
            delta: Duration::from_micros(3_000_000),
            ..FrameInput::with_field_defaults()
        };
        input.button_held = ButtonVec::replicate(8, false);
        let (next, events, _audio, _render) = step_frame(&state, &input);
        assert_eq!(next.current_time.to_micros(), 3_000_000);
        assert!(!events.is_empty());
        assert_eq!(next.score.combo, Natural::from(0u32));
    }

    #[test]
    fn timed_batch_click_scores_perfect() {
        let spec: ChartSpec = frontend_lowered_chart(MAIDATA, (1).into()).unwrap();
        let state = crate::chart_loader::build_game_state(&spec);
        let mut batch = TimedInputBatch {
            current_time: TimePoint::zero(),
            ..Default::default()
        };
        batch.events.push(TimedInputEvent::ButtonClick(
            TimePoint::zero(),
            ButtonZone::K1,
        ));
        let (next, events, _audio, _render) = step_frame_timed(&state, &batch);
        // First tap judged at time 0 (delta 0 window contains exact point).
        assert!(!events.is_empty());
        assert!(next.score.combo >= 1u32);
    }
}
