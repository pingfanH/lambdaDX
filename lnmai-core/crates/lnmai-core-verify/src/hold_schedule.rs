//! The ordinary button-hold traversal shared with the runtime scheduler.
//!
//! Read the original input list once, in order, threading frontier, queue and
//! cursor updates between notes. Forward appends preserve the reference's
//! recursive-cons output order without a recursive Rust traversal.

/// Scheduling observes the constructor, not a HeadJudged/Ended grade payload.
/// The complete note is still passed to the lifecycle operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldPhase {
    HeadWaiting,
    HeadJudgeable,
    HeadJudged,
    BodyHeld,
    BodyReleased,
    Ended,
}

/// Forward-order output buffers. Runtime calls start active/events empty.
pub struct HoldWork<F, Q, C, H, E> {
    pub frontiers: F,
    pub queues: Q,
    pub cursor: C,
    pub active: H,
    pub events: E,
}

/// Point reads/writes and one-note lifecycle, with one explicit exception:
/// update_queue retains the separate replace+normalize composition contract.
/// No method decides an entire item or pass. length/get observe the immutable
/// source list; appends preserve order and require sufficient container capacity.
pub trait HoldSchedule<S, Z, N, F, Q, C, H, E, Event, D, Area> {
    fn length(&self, holds: &S) -> usize;
    fn get(&self, holds: &S, index: usize) -> (Z, N);
    fn button_diff(&self, note: &N) -> D;
    fn sensor_diff(&self, note: &N) -> D;
    fn button_pressed(&self, zone: &Z) -> bool;
    fn sensor_pressed(&self, zone: &Z) -> bool;
    fn prev_sensor_pressed(&self, zone: &Z) -> bool;
    fn phase(&self, note: &N) -> HoldPhase;
    fn unlocked(&self, frontiers: &F, zone: &Z, note: &N) -> bool;
    fn eligible(&self, note: &N) -> bool;
    fn fallback_area(&self, zone: &Z) -> Area;
    fn button_available(&self, cursor: &C, zone: &Z) -> bool;
    fn sensor_available(&self, cursor: &C, area: &Area) -> bool;
    fn use_button(&self, cursor: C, zone: &Z) -> C;
    fn use_sensor(&self, cursor: C, area: &Area) -> C;
    fn step(
        &self,
        note: &N,
        diff: D,
        clicked: bool,
        pressed: bool,
        current_button_pressed: bool,
        prev_sensor_pressed: bool,
    ) -> (N, Option<Event>);
    fn advance_frontier(&self, frontiers: F, zone: &Z) -> F;
    fn update_queue(&self, queues: Q, zone: &Z, note: &N) -> Q;
    fn append_active(&self, active: H, zone: Z, note: N) -> H;
    fn append_event(&self, events: E, event: Event) -> E;
}

/// Full ordinary-hold pass. The index increases only under index < len, so
/// index + 1 <= len <= usize::MAX. No recursive traversal or len + 1 is needed.
/// The queue adapter's separate normalization boundary is unaffected by this.
pub fn process<S, Z, N, F, Q, C, H, E, Event, D, Area, Ops>(
    holds: &S,
    mut work: HoldWork<F, Q, C, H, E>,
    ops: Ops,
) -> HoldWork<F, Q, C, H, E>
where
    Ops: HoldSchedule<S, Z, N, F, Q, C, H, E, Event, D, Area>,
{
    let len = ops.length(holds);
    let mut index = 0;
    while index < len {
        let (zone, before) = ops.get(holds, index);
        let button_diff = ops.button_diff(&before);
        let sensor_diff = ops.sensor_diff(&before);
        let current_button_pressed = ops.button_pressed(&zone);
        let current_sensor_pressed = ops.sensor_pressed(&zone);
        let prev_sensor_pressed = ops.prev_sensor_pressed(&zone);
        let before_phase = ops.phase(&before);
        let allow_input = matches!(
            before_phase,
            HoldPhase::HeadWaiting | HoldPhase::HeadJudgeable
        ) && ops.unlocked(&work.frontiers, &zone, &before)
            && ops.eligible(&before);
        let fallback_area = ops.fallback_area(&zone);
        // Both availability reads observe the cursor before either update.
        let plan = crate::scheduler::consume_click(
            allow_input,
            ops.button_available(&work.cursor, &zone),
            ops.sensor_available(&work.cursor, &fallback_area),
        );
        if plan.used_button {
            work.cursor = ops.use_button(work.cursor, &zone);
        }
        if plan.used_sensor {
            work.cursor = ops.use_sensor(work.cursor, &fallback_area);
        }
        let clicked = plan.used_button || plan.used_sensor;
        // Ordinary Hold uses the sensor diff even when no click was consumed.
        let diff = if plan.used_button {
            button_diff
        } else {
            sensor_diff
        };
        let pressed = current_button_pressed || current_sensor_pressed;
        let (after, event) = ops.step(
            &before,
            diff,
            clicked,
            pressed,
            current_button_pressed,
            prev_sensor_pressed,
        );
        let after_phase = ops.phase(&after);
        // Match the final lifecycle state, including HeadJudged(Miss). A
        // transition straight to Body/Ended does not satisfy this condition.
        if !matches!(before_phase, HoldPhase::HeadJudged)
            && matches!(after_phase, HoldPhase::HeadJudged)
        {
            work.frontiers = ops.advance_frontier(work.frontiers, &zone);
        }
        // Sync every result; event presence and active retention are irrelevant.
        work.queues = ops.update_queue(work.queues, &zone, &after);
        if !matches!(after_phase, HoldPhase::Ended) {
            work.active = ops.append_active(work.active, zone, after);
        }
        if let Some(event) = event {
            work.events = ops.append_event(work.events, event);
        }
        index += 1;
    }
    work
}
