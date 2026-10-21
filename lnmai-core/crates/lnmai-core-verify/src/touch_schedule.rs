//! Proposed shared kernel for Scheduler.processTouchQueueHeadFuel only.
//!
//! The caller supplies the head fuel unchanged. Queue normalization is composed
//! from queue_maintenance, including at fuel zero and at each recursive-entry
//! equivalent. This module does not implement the separate automatic pass.

use crate::queue_maintenance::{self, QueueCursor, QueueReplace};

/// Groups and clicks are shared across iterations; events stay in reverse
/// accumulator order. The outer sensor pass owns the final reversal.
pub struct TouchWork<Q, F, C, G, E> {
    pub queue: Q,
    pub frontiers: F,
    pub cursor: C,
    pub groups: G,
    pub events_rev: E,
}

/// Scheduler point operations, plus two explicitly identified group subcalls.
///
/// `step` is one note's lifecycle call, never a queue/click/group iteration.
/// `shared_result` and `register_group` retain the existing group-plan helpers;
/// their list traversal is a SEPARATE proof boundary (SchedulerFrameProof's
/// group-plan interpretation plus concrete container/number laws). They must
/// not be described as single field accesses. Queue traversal is composed
/// below, rather than delegated to normalize/replace callbacks.
///
/// M supplies QueueCursor's peek/resolved/advance and QueueReplace's
/// get/index_equal/write. Each queue_ops result must implement the same laws.
/// Id represents both group identifiers and group sizes; it is not a usize.
pub trait TouchSchedule<Q, N, F, C, G, E, Event, T, D, Area, Zone, Id, Grade, M> {
    fn queue_ops(&self) -> M;
    fn queue_remaining(&self, queue: &Q) -> usize;
    fn queue_len(&self, queue: &Q) -> usize;
    fn timing(&self, note: &N) -> T;
    fn sensor_diff(&self, timing: &T) -> D;
    fn button_diff(&self, timing: T) -> D;
    fn group_id(&self, note: &N) -> Option<Id>;
    fn group_size(&self, note: &N) -> Id;
    fn shared_result(&self, groups: &G, id: Id) -> Option<(Grade, D)>;
    fn eligible(&self, note: &N) -> bool;
    fn unlocked(&self, frontiers: &F, note: &N) -> bool;
    fn button_ring_for_touch(&self) -> bool;
    fn sensor_area(&self, note: &N) -> Area;
    fn outer_button(&self, area: &Area) -> Option<Zone>;
    fn button_available(&self, cursor: &C, zone: &Zone) -> bool;
    fn sensor_available(&self, cursor: &C, area: &Area) -> bool;
    fn use_button(&self, cursor: C, zone: &Zone) -> C;
    fn use_sensor(&self, cursor: C, area: &Area) -> C;
    fn step(
        &self,
        note: &N,
        diff: &D,
        clicked: bool,
        shared: Option<(Grade, D)>,
    ) -> (N, Option<Event>);
    fn ended(&self, note: &N) -> bool;
    fn event_grade(&self, event: &Event) -> Grade;
    fn is_miss_or_too_fast(&self, grade: &Grade) -> bool;
    fn prepend_event(&self, events_rev: E, event: &Event) -> E;
    fn register_group(&self, groups: G, id: Id, size: Id, grade: Grade, diff: D) -> G;
    fn advance_frontier(&self, frontiers: F) -> F;
}

/// Iterative form of the original head-fuel recursion. All head-level
/// branching is here. A continuation spends one fuel unit, after the
/// nonzero guard, and normalizes once again at the top of the loop.
pub fn process<Q, N, F, C, G, E, Event, T, D, Area, Zone, Id, Grade, M, Ops>(
    mut fuel: usize,
    mut work: TouchWork<Q, F, C, G, E>,
    ops: Ops,
) -> TouchWork<Q, F, C, G, E>
where
    Ops: TouchSchedule<Q, N, F, C, G, E, Event, T, D, Area, Zone, Id, Grade, M>,
    M: QueueCursor<Q, N> + QueueReplace<Q, N>,
{
    loop {
        let remaining = ops.queue_remaining(&work.queue);
        work.queue =
            queue_maintenance::normalize_cursor_fuel(remaining, work.queue, ops.queue_ops());
        if fuel == 0 {
            return work;
        }
        let maintenance = ops.queue_ops();
        let before = match maintenance.peek(&work.queue) {
            None => return work,
            Some(note) => note,
        };

        // Compute this even on a shared judgment or a button click.
        let timing = ops.timing(&before);
        let sensor_diff = ops.sensor_diff(&timing);
        let shared = match ops.group_id(&before) {
            Some(id) => ops.shared_result(&work.groups, id),
            None => None,
        };
        let can_consume_click =
            shared.is_none() && ops.eligible(&before) && ops.unlocked(&work.frontiers, &before);
        let area = ops.sensor_area(&before);
        let mut used_button = false;
        if can_consume_click && ops.button_ring_for_touch() {
            if let Some(zone) = ops.outer_button(&area) {
                if ops.button_available(&work.cursor, &zone) {
                    work.cursor = ops.use_button(work.cursor, &zone);
                    used_button = true;
                }
            }
        }
        let mut used_sensor = false;
        if can_consume_click && !used_button && ops.sensor_available(&work.cursor, &area) {
            work.cursor = ops.use_sensor(work.cursor, &area);
            used_sensor = true;
        }
        let clicked = used_button || used_sensor;
        let diff = if used_button {
            ops.button_diff(timing)
        } else {
            sensor_diff
        };
        // No click is required: timeout/shared judgments are part of the head.
        let (after, event) = ops.step(&before, &diff, clicked, shared);
        let resolved_now = !maintenance.resolved(&before) && maintenance.resolved(&after);
        if let Some(event) = event {
            work.events_rev = ops.prepend_event(work.events_rev, &event);
            if clicked {
                let grade = ops.event_grade(&event);
                if !ops.is_miss_or_too_fast(&grade) {
                    if let Some(id) = ops.group_id(&before) {
                        work.groups = ops.register_group(
                            work.groups,
                            id,
                            ops.group_size(&before),
                            grade,
                            diff,
                        );
                    }
                }
            }
        }

        // Replace EVERY matching id, including duplicate slots before the
        // cursor. Normalization is independent of whether an event exists.
        let len = ops.queue_len(&work.queue);
        work.queue = queue_maintenance::replace_all(len, work.queue, &after, maintenance);
        let remaining = ops.queue_remaining(&work.queue);
        work.queue =
            queue_maintenance::normalize_cursor_fuel(remaining, work.queue, ops.queue_ops());
        if resolved_now {
            work.frontiers = ops.advance_frontier(work.frontiers);
        }

        // Judged is resolved, but only Ended permits another head iteration.
        if !ops.ended(&after) {
            return work;
        }
        // Match the existing helper's eager ring probe, including when the
        // ring is disabled; the availability kernel applies the ring flag.
        let ring_available = match ops.outer_button(&area) {
            Some(zone) => ops.button_available(&work.cursor, &zone),
            None => false,
        };
        let unused = crate::scheduler::touch_input_available(
            ops.button_ring_for_touch(),
            ops.sensor_available(&work.cursor, &area),
            Some(ring_available),
        );
        if !unused {
            return work;
        }
        fuel -= 1;
    }
}
