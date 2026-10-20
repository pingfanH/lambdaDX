//! Shared concrete TouchHold body storage adapters.
//!
//! Both trait implementations call the same generic record/Vec kernels used by
//! the runtime. SliceHoldSource supplies ordered source access and Id cloning;
//! the runtime supplies three borrowed note fields. Scheduling forwards the
//! existing head/lifecycle operations once,
//! without changing the pass's decisions, evaluation order or accumulated work.

use crate::hold_schedule::HoldPhase;
use crate::touch_hold_body::{self, TouchHoldBodyGroupState};
use crate::touch_hold_rebuild::TouchHoldRebuild;
use crate::touch_hold_schedule::{BodyGroups, HeadGroups, TouchHoldSchedule};

/// Immutable source boundary for body-group reconstruction.
/// Reads must preserve the full source order, duplicate notes, exact IDs, and
/// trigger flags. Indexed reads require index < hold_length(holds).
/// No lifecycle filter, storage operation, search or mutation belongs here.
pub trait HoldSource<S, Id> {
    fn hold_length(&self, holds: &S) -> usize;
    fn hold_group_id(&self, holds: &S, index: usize) -> Option<Id>;
    fn hold_note_index(&self, holds: &S, index: usize) -> Id;
    fn hold_triggered(&self, holds: &S, index: usize) -> bool;
}

/// Three borrowed fields from one note. These methods perform no slice access,
/// source filtering, cloning, group lookup, or mutation. The body group ID is
/// intentionally distinct from the head-sharing touch_group_id field.
pub trait HoldFields<Note, Id> {
    fn body_group_id<'note>(&self, note: &'note Note) -> &'note Option<Id>;
    fn note_index<'note>(&self, note: &'note Note) -> &'note Id;
    fn body_triggered(&self, note: &Note) -> bool;
}

/// Full ordered-slice source reads, shared by both reconstruction callers.
/// Only the three literal note-field projections stay in the runtime facade.
pub struct SliceHoldSource<Fields> {
    pub fields: Fields,
}

/// Clone one optional ID through a direct borrowed input.
/// None clones nothing; Some clones only the contained ID once.
fn clone_optional_id<Id: Clone>(value: &Option<Id>) -> Option<Id> {
    match value {
        None => None,
        Some(id) => Some(Id::clone(id)),
    }
}

impl<'source, Area, Note, Id, Fields> HoldSource<&'source [(Area, Note)], Id>
    for SliceHoldSource<Fields>
where
    Id: Clone,
    Fields: HoldFields<Note, Id>,
{
    fn hold_length(&self, holds: &&'source [(Area, Note)]) -> usize {
        holds.len()
    }

    fn hold_group_id(&self, holds: &&'source [(Area, Note)], index: usize) -> Option<Id> {
        let note = &holds[index].1;
        clone_optional_id(self.fields.body_group_id(note))
    }

    fn hold_note_index(&self, holds: &&'source [(Area, Note)], index: usize) -> Id {
        let note = &holds[index].1;
        Id::clone(self.fields.note_index(note))
    }

    fn hold_triggered(&self, holds: &&'source [(Area, Note)], index: usize) -> bool {
        let note = &holds[index].1;
        self.fields.body_triggered(note)
    }
}

/// The same wrapper supplies concrete body storage for rebuild and scheduling.
/// Ops is a HoldSource for rebuild, or the existing head/lifecycle adapter for
/// scheduling. No trait on Ops can override the body-group storage operations.
pub struct TouchHoldBodyAdapter<Ops> {
    pub ops: Ops,
}

/// Strict majority of a nonempty member list. This is equivalent to
/// member_len > 0 && 2 * triggered_len > member_len over unbounded naturals.
/// The zero-member guard is intentional even if triggers are nonempty. No
/// triggered_len <= member_len assumption, multiplication or increment is used.
pub fn strict_majority_lengths(triggered_len: usize, member_len: usize) -> bool {
    member_len > 0 && triggered_len > member_len / 2
}

/// Read trigger length before member length, as in the runtime's former call.
pub fn body_majority<Id>(groups: &[TouchHoldBodyGroupState<Id>], index: usize) -> bool {
    let triggered_len = touch_hold_body::trigger_length(groups, index);
    let member_len = touch_hold_body::member_length(groups, index);
    strict_majority_lengths(triggered_len, member_len)
}

/// The first matching group decides, including when its majority is false.
/// Later duplicate IDs are never used as a fallback.
pub fn body_pressed<Id: PartialEq>(groups: &[TouchHoldBodyGroupState<Id>], id: &Id) -> bool {
    match touch_hold_body::find_group(groups, id) {
        Some(index) => body_majority(groups, index),
        None => false,
    }
}

impl<S, Id, Ops> TouchHoldRebuild<S, Vec<TouchHoldBodyGroupState<Id>>, Id>
    for TouchHoldBodyAdapter<Ops>
where
    Id: PartialEq + Clone + Default,
    Ops: HoldSource<S, Id>,
{
    fn hold_length(&self, holds: &S) -> usize {
        self.ops.hold_length(holds)
    }

    fn hold_group_id(&self, holds: &S, index: usize) -> Option<Id> {
        self.ops.hold_group_id(holds, index)
    }

    fn hold_note_index(&self, holds: &S, index: usize) -> Id {
        self.ops.hold_note_index(holds, index)
    }

    fn hold_triggered(&self, holds: &S, index: usize) -> bool {
        self.ops.hold_triggered(holds, index)
    }

    fn empty(&self) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::empty()
    }

    fn group_length(&self, groups: &Vec<TouchHoldBodyGroupState<Id>>) -> usize {
        touch_hold_body::group_length(groups)
    }

    fn group_matches(
        &self,
        groups: &Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        id: &Id,
    ) -> bool {
        touch_hold_body::group_matches(groups, index, id)
    }

    fn member_length(&self, groups: &Vec<TouchHoldBodyGroupState<Id>>, group: usize) -> usize {
        touch_hold_body::member_length(groups, group)
    }

    fn member_matches(
        &self,
        groups: &Vec<TouchHoldBodyGroupState<Id>>,
        group: usize,
        index: usize,
        note: &Id,
    ) -> bool {
        touch_hold_body::member_matches(groups, group, index, note)
    }

    fn trigger_length(&self, groups: &Vec<TouchHoldBodyGroupState<Id>>, group: usize) -> usize {
        touch_hold_body::trigger_length(groups, group)
    }

    fn trigger_matches(
        &self,
        groups: &Vec<TouchHoldBodyGroupState<Id>>,
        group: usize,
        index: usize,
        note: &Id,
    ) -> bool {
        touch_hold_body::trigger_matches(groups, group, index, note)
    }

    fn prepend_member(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        group: usize,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::prepend_member(groups, group, note)
    }

    fn prepend_trigger(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        group: usize,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::prepend_trigger(groups, group, note)
    }

    fn remove_trigger(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        group: usize,
        index: usize,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::remove_trigger(groups, group, index)
    }

    fn append_empty(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        id: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::append_empty(groups, id)
    }

    fn swap_groups(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        left: usize,
        right: usize,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::swap_groups(groups, left, right)
    }
}

impl<Id, Ops> BodyGroups<Vec<TouchHoldBodyGroupState<Id>>, Id> for TouchHoldBodyAdapter<Ops>
where
    Id: PartialEq + Clone + Default,
{
    fn body_length(&self, groups: &Vec<TouchHoldBodyGroupState<Id>>) -> usize {
        touch_hold_body::group_length(groups)
    }

    fn body_matches(
        &self,
        groups: &Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        id: &Id,
    ) -> bool {
        touch_hold_body::group_matches(groups, index, id)
    }

    fn body_majority(&self, groups: &Vec<TouchHoldBodyGroupState<Id>>, index: usize) -> bool {
        body_majority(groups, index)
    }

    fn body_has_member(
        &self,
        groups: &Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        note: &Id,
    ) -> bool {
        touch_hold_body::has_member(groups, index, note)
    }

    fn body_has_trigger(
        &self,
        groups: &Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        note: &Id,
    ) -> bool {
        touch_hold_body::has_trigger(groups, index, note)
    }

    fn body_prepend_member(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::prepend_member(groups, index, note)
    }

    fn body_prepend_trigger(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::prepend_trigger(groups, index, note)
    }

    fn body_erase_member(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::erase_member(groups, index, note)
    }

    fn body_erase_trigger(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::erase_trigger(groups, index, note)
    }

    fn body_members_empty(&self, groups: &Vec<TouchHoldBodyGroupState<Id>>, index: usize) -> bool {
        touch_hold_body::members_empty(groups, index)
    }

    fn body_remove(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        index: usize,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::remove_group(groups, index)
    }

    fn body_append(
        &self,
        groups: Vec<TouchHoldBodyGroupState<Id>>,
        id: &Id,
        note: &Id,
    ) -> Vec<TouchHoldBodyGroupState<Id>> {
        touch_hold_body::append(groups, id, note)
    }
}

// Keep the existing schedule kernel's public API. Only these forwarding impls
// adapt its other two trait dictionaries; their arguments/results pass through
// unchanged, without cloning source notes, groups, IDs, events or cursors.
impl<G, Id, Grade, D, Ops> HeadGroups<G, Id, Grade, D> for TouchHoldBodyAdapter<Ops>
where
    Ops: HeadGroups<G, Id, Grade, D>,
{
    fn head_length(&self, groups: &G) -> usize {
        self.ops.head_length(groups)
    }

    fn head_matches(&self, groups: &G, index: usize, id: &Id) -> bool {
        self.ops.head_matches(groups, index, id)
    }

    fn head_majority(&self, groups: &G, index: usize) -> bool {
        self.ops.head_majority(groups, index)
    }

    fn head_result(&self, groups: &G, index: usize) -> (Grade, D) {
        self.ops.head_result(groups, index)
    }

    fn head_increment(&self, groups: G, index: usize) -> G {
        self.ops.head_increment(groups, index)
    }

    fn head_set_size(&self, groups: G, index: usize, size: Id) -> G {
        self.ops.head_set_size(groups, index, size)
    }

    fn head_set_result(&self, groups: G, index: usize, grade: Grade, diff: D) -> G {
        self.ops.head_set_result(groups, index, grade, diff)
    }

    fn head_append(&self, groups: G, id: &Id, size: Id, grade: Grade, diff: D) -> G {
        self.ops.head_append(groups, id, size, grade, diff)
    }
}

impl<S, A, N, F, Q, C, H, E, Event, D, Z, Id, Grade, Ops>
    TouchHoldSchedule<S, A, N, F, Q, C, H, E, Event, D, Z, Id, Grade> for TouchHoldBodyAdapter<Ops>
where
    Ops: TouchHoldSchedule<S, A, N, F, Q, C, H, E, Event, D, Z, Id, Grade>,
{
    fn length(&self, holds: &S) -> usize {
        self.ops.length(holds)
    }

    fn get(&self, holds: &S, index: usize) -> (A, N) {
        self.ops.get(holds, index)
    }

    fn sensor_diff(&self, note: &N) -> D {
        self.ops.sensor_diff(note)
    }

    fn button_diff(&self, note: &N) -> D {
        self.ops.button_diff(note)
    }

    fn local_pressed(&self, area: &A) -> bool {
        self.ops.local_pressed(area)
    }

    fn body_check_active(&self, note: &N) -> bool {
        self.ops.body_check_active(note)
    }

    fn body_group_id(&self, note: &N) -> Option<Id> {
        self.ops.body_group_id(note)
    }

    fn head_group_id(&self, note: &N) -> Option<Id> {
        self.ops.head_group_id(note)
    }

    fn note_index(&self, note: &N) -> Id {
        self.ops.note_index(note)
    }

    fn group_size(&self, note: &N) -> Id {
        self.ops.group_size(note)
    }

    fn phase(&self, note: &N) -> HoldPhase {
        self.ops.phase(note)
    }

    fn head_grade(&self, note: &N) -> Option<Grade> {
        self.ops.head_grade(note)
    }

    fn head_diff(&self, note: &N) -> D {
        self.ops.head_diff(note)
    }

    fn grade_rejected(&self, grade: &Grade) -> bool {
        self.ops.grade_rejected(grade)
    }

    fn within_head_window(&self, note: &N) -> bool {
        self.ops.within_head_window(note)
    }

    fn unlocked(&self, frontiers: &F, area: &A, note: &N) -> bool {
        self.ops.unlocked(frontiers, area, note)
    }

    fn eligible(&self, note: &N) -> bool {
        self.ops.eligible(note)
    }

    fn button_ring(&self) -> bool {
        self.ops.button_ring()
    }

    fn outer_button(&self, area: &A) -> Option<Z> {
        self.ops.outer_button(area)
    }

    fn button_available(&self, cursor: &C, zone: &Z) -> bool {
        self.ops.button_available(cursor, zone)
    }

    fn sensor_available(&self, cursor: &C, area: &A) -> bool {
        self.ops.sensor_available(cursor, area)
    }

    fn use_button(&self, cursor: C, zone: &Z) -> C {
        self.ops.use_button(cursor, zone)
    }

    fn use_sensor(&self, cursor: C, area: &A) -> C {
        self.ops.use_sensor(cursor, area)
    }

    fn step(
        &self,
        note: &N,
        diff: D,
        clicked: bool,
        pressed: bool,
        shared: Option<(Grade, D)>,
    ) -> (N, Option<Event>) {
        self.ops.step(note, diff, clicked, pressed, shared)
    }

    fn advance_frontier(&self, frontiers: F, area: &A) -> F {
        self.ops.advance_frontier(frontiers, area)
    }

    fn update_queue(&self, queues: Q, area: &A, note: &N) -> Q {
        self.ops.update_queue(queues, area, note)
    }

    fn append_active(&self, active: H, area: A, note: N) -> H {
        self.ops.append_active(active, area, note)
    }

    fn append_event(&self, events: E, event: Event) -> E {
        self.ops.append_event(events, event)
    }
}
