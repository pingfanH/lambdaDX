//! Complete TouchHold pass: immutable source order, current groups and cursor.
//! All searches and scheduling branches live here. Adapters supply point fields,
//! list primitives, one-note lifecycle and the separate queue composition.

use crate::hold_schedule::HoldPhase;

pub struct TouchHoldWork<F, Q, C, H, E, G, B> {
    pub frontiers: F,
    pub queues: Q,
    pub cursor: C,
    pub active: H,
    pub events: E,
    pub groups: G,
    pub body_groups: B,
}

/// Indexed single-group fields/mutations. No search or upsert callback.
pub trait HeadGroups<G, Id, Grade, D> {
    fn head_length(&self, groups: &G) -> usize;
    fn head_matches(&self, groups: &G, index: usize, id: &Id) -> bool;
    fn head_majority(&self, groups: &G, index: usize) -> bool;
    fn head_result(&self, groups: &G, index: usize) -> (Grade, D);
    fn head_increment(&self, groups: G, index: usize) -> G;
    fn head_set_size(&self, groups: G, index: usize, size: Id) -> G;
    fn head_set_result(&self, groups: G, index: usize, grade: Grade, diff: D) -> G;
    fn head_append(&self, groups: G, id: &Id, size: Id, grade: Grade, diff: D) -> G;
}

/// Membership-list primitives on one group. Erase means FIRST occurrence;
/// prepend means index zero. Neither primitive decides a whole member update.
pub trait BodyGroups<B, Id> {
    fn body_length(&self, groups: &B) -> usize;
    fn body_matches(&self, groups: &B, index: usize, id: &Id) -> bool;
    fn body_majority(&self, groups: &B, index: usize) -> bool;
    fn body_has_member(&self, groups: &B, index: usize, note: &Id) -> bool;
    fn body_has_trigger(&self, groups: &B, index: usize, note: &Id) -> bool;
    fn body_prepend_member(&self, groups: B, index: usize, note: &Id) -> B;
    fn body_prepend_trigger(&self, groups: B, index: usize, note: &Id) -> B;
    fn body_erase_member(&self, groups: B, index: usize, note: &Id) -> B;
    fn body_erase_trigger(&self, groups: B, index: usize, note: &Id) -> B;
    fn body_members_empty(&self, groups: &B, index: usize) -> bool;
    fn body_remove(&self, groups: B, index: usize) -> B;
    fn body_append(&self, groups: B, id: &Id, note: &Id) -> B;
}

pub trait TouchHoldSchedule<S, A, N, F, Q, C, H, E, Event, D, Z, Id, Grade> {
    fn length(&self, holds: &S) -> usize;
    fn get(&self, holds: &S, index: usize) -> (A, N);
    fn sensor_diff(&self, note: &N) -> D;
    fn button_diff(&self, note: &N) -> D;
    fn local_pressed(&self, area: &A) -> bool;
    fn body_check_active(&self, note: &N) -> bool;
    fn body_group_id(&self, note: &N) -> Option<Id>;
    fn head_group_id(&self, note: &N) -> Option<Id>;
    fn note_index(&self, note: &N) -> Id;
    fn group_size(&self, note: &N) -> Id;
    fn phase(&self, note: &N) -> HoldPhase;
    fn head_grade(&self, note: &N) -> Option<Grade>;
    fn head_diff(&self, note: &N) -> D;
    fn grade_rejected(&self, grade: &Grade) -> bool;
    fn within_head_window(&self, note: &N) -> bool;
    fn unlocked(&self, frontiers: &F, area: &A, note: &N) -> bool;
    fn eligible(&self, note: &N) -> bool;
    fn button_ring(&self) -> bool;
    fn outer_button(&self, area: &A) -> Option<Z>;
    fn button_available(&self, cursor: &C, zone: &Z) -> bool;
    fn sensor_available(&self, cursor: &C, area: &A) -> bool;
    fn use_button(&self, cursor: C, zone: &Z) -> C;
    fn use_sensor(&self, cursor: C, area: &A) -> C;
    fn step(
        &self,
        note: &N,
        diff: D,
        clicked: bool,
        pressed: bool,
        shared: Option<(Grade, D)>,
    ) -> (N, Option<Event>);
    fn advance_frontier(&self, frontiers: F, area: &A) -> F;
    /// Existing replace-all + normalize + sensor-slot-store composition.
    fn update_queue(&self, queues: Q, area: &A, note: &N) -> Q;
    fn append_active(&self, active: H, area: A, note: N) -> H;
    fn append_event(&self, events: E, event: Event) -> E;
}

/// Preserve first matching group even when that group's majority is false.
pub fn find_head<G, Id, Grade, D, Ops>(groups: &G, id: &Id, ops: &Ops) -> Option<usize>
where
    Ops: HeadGroups<G, Id, Grade, D>,
{
    let len = ops.head_length(groups);
    let mut index = 0;
    while index < len {
        if ops.head_matches(groups, index, id) {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub fn shared_result<G, Id, Grade, D, Ops>(groups: &G, id: &Id, ops: &Ops) -> Option<(Grade, D)>
where
    Ops: HeadGroups<G, Id, Grade, D>,
{
    match find_head::<G, Id, Grade, D, Ops>(groups, id, ops) {
        Some(index) => {
            if ops.head_majority(groups, index) {
                Some(ops.head_result(groups, index))
            } else {
                None
            }
        }
        None => None,
    }
}

pub fn register_head<G, Id, Grade, D, Ops>(
    mut groups: G,
    id: &Id,
    size: Id,
    grade: Grade,
    diff: D,
    ops: &Ops,
) -> G
where
    Ops: HeadGroups<G, Id, Grade, D>,
{
    match find_head::<G, Id, Grade, D, Ops>(&groups, id, ops) {
        Some(index) => {
            let keep_result = ops.head_majority(&groups, index);
            groups = ops.head_increment(groups, index);
            groups = ops.head_set_size(groups, index, size);
            if !keep_result {
                groups = ops.head_set_result(groups, index, grade, diff);
            }
            groups
        }
        None => ops.head_append(groups, id, size, grade, diff),
    }
}

pub fn find_body<B, Id, Ops>(groups: &B, id: &Id, ops: &Ops) -> Option<usize>
where
    Ops: BodyGroups<B, Id>,
{
    let len = ops.body_length(groups);
    let mut index = 0;
    while index < len {
        if ops.body_matches(groups, index, id) {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub fn body_pressed<B, Id, Ops>(groups: &B, id: &Id, ops: &Ops) -> bool
where
    Ops: BodyGroups<B, Id>,
{
    match find_body(groups, id, ops) {
        Some(index) => ops.body_majority(groups, index),
        None => false,
    }
}

pub fn register_body<B, Id, Ops>(mut groups: B, id: &Id, note: &Id, ops: &Ops) -> B
where
    Ops: BodyGroups<B, Id>,
{
    match find_body(&groups, id, ops) {
        Some(index) => {
            if !ops.body_has_member(&groups, index, note) {
                groups = ops.body_prepend_member(groups, index, note);
            }
            if !ops.body_has_trigger(&groups, index, note) {
                groups = ops.body_prepend_trigger(groups, index, note);
            }
            groups
        }
        None => ops.body_append(groups, id, note),
    }
}

pub fn unregister_body<B, Id, Ops>(groups: B, id: &Id, note: &Id, ops: &Ops) -> B
where
    Ops: BodyGroups<B, Id>,
{
    match find_body(&groups, id, ops) {
        Some(index) => ops.body_erase_trigger(groups, index, note),
        None => groups,
    }
}

pub fn exit_body<B, Id, Ops>(mut groups: B, id: &Id, note: &Id, ops: &Ops) -> B
where
    Ops: BodyGroups<B, Id>,
{
    match find_body(&groups, id, ops) {
        Some(index) => {
            groups = ops.body_erase_member(groups, index, note);
            if ops.body_members_empty(&groups, index) {
                ops.body_remove(groups, index)
            } else {
                ops.body_erase_trigger(groups, index, note)
            }
        }
        None => groups,
    }
}

/// No recursive source traversal, no whole-item callback, no len + 1.
/// Every bounded scan increments only under index < len. Forward appends
/// preserve the successful reference's recursive-cons order, with old prefixes.
pub fn process<S, A, N, F, Q, C, H, E, Event, D, Z, Id, Grade, G, B, Ops>(
    holds: &S,
    mut work: TouchHoldWork<F, Q, C, H, E, G, B>,
    ops: Ops,
) -> TouchHoldWork<F, Q, C, H, E, G, B>
where
    Ops: TouchHoldSchedule<S, A, N, F, Q, C, H, E, Event, D, Z, Id, Grade>
        + HeadGroups<G, Id, Grade, D>
        + BodyGroups<B, Id>,
{
    let len = ops.length(holds);
    let mut index = 0;
    while index < len {
        let (area, before) = ops.get(holds, index);
        let sensor_diff = ops.sensor_diff(&before);
        let local_pressed = ops.local_pressed(&area);
        let body_id = ops.body_group_id(&before);
        let note_id = ops.note_index(&before);
        if ops.body_check_active(&before) {
            if let Some(id) = &body_id {
                work.body_groups = if local_pressed {
                    register_body(work.body_groups, id, &note_id, &ops)
                } else {
                    unregister_body(work.body_groups, id, &note_id, &ops)
                };
            }
        }
        let pressed = local_pressed
            || match &body_id {
                Some(id) => body_pressed(&work.body_groups, id, &ops),
                None => false,
            };
        let head_id = ops.head_group_id(&before);
        let shared = match &head_id {
            Some(id) => shared_result::<G, Id, Grade, D, Ops>(&work.groups, id, &ops),
            None => None,
        };
        let before_phase = ops.phase(&before);
        let allow_input = shared.is_none()
            && ops.within_head_window(&before)
            && matches!(
                before_phase,
                HoldPhase::HeadWaiting | HoldPhase::HeadJudgeable
            )
            && ops.unlocked(&work.frontiers, &area, &before)
            && ops.eligible(&before);
        let outer_button = if allow_input && ops.button_ring() {
            ops.outer_button(&area)
        } else {
            None
        };
        let button_available = match &outer_button {
            Some(zone) => ops.button_available(&work.cursor, zone),
            None => false,
        };
        // If a button is consumed the sensor is not even probed. Otherwise
        // the cursor before and after the button branch is identical.
        let sensor_available = if allow_input && !button_available {
            ops.sensor_available(&work.cursor, &area)
        } else {
            false
        };
        let plan = crate::scheduler::consume_click(allow_input, button_available, sensor_available);
        if plan.used_button {
            if let Some(zone) = outer_button {
                work.cursor = ops.use_button(work.cursor, &zone);
            }
        }
        if plan.used_sensor {
            work.cursor = ops.use_sensor(work.cursor, &area);
        }
        let diff = if plan.used_button {
            ops.button_diff(&before)
        } else {
            sensor_diff
        };
        let (after, event) = ops.step(
            &before,
            diff,
            plan.used_button || plan.used_sensor,
            pressed,
            shared,
        );
        let after_phase = ops.phase(&after);
        let entered_head = !matches!(before_phase, HoldPhase::HeadJudged)
            && matches!(after_phase, HoldPhase::HeadJudged);
        if entered_head {
            work.frontiers = ops.advance_frontier(work.frontiers, &area);
        }
        work.queues = ops.update_queue(work.queues, &area, &after);
        if entered_head {
            if let Some(id) = &head_id {
                if let Some(grade) = ops.head_grade(&after) {
                    if !ops.grade_rejected(&grade) {
                        work.groups = register_head(
                            work.groups,
                            id,
                            ops.group_size(&before),
                            grade,
                            ops.head_diff(&after),
                            &ops,
                        );
                    }
                }
            }
        }
        let keep_active = !matches!(after_phase, HoldPhase::Ended);
        if !keep_active {
            if let Some(id) = &body_id {
                work.body_groups = exit_body(work.body_groups, id, &note_id, &ops);
            }
        }
        if keep_active {
            work.active = ops.append_active(work.active, area, after);
        }
        if let Some(event) = event {
            work.events = ops.append_event(work.events, event);
        }
        index += 1;
    }
    work
}
