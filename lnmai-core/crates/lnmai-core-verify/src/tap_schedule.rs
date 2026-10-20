//! The tap-family queue traversal shared with the runtime scheduler.
//!
//! The queue's current slot is peeked before inspecting fuel. Only an Ended
//! result advances the queue/frontier, and another iteration requires an
//! unused button or the just-processed note's fallback sensor click.

/// Events use the reference's reverse accumulator order. The outer runtime
/// pass reverses the accumulated events once, after processing all zones.
pub struct TapWork<Q, F, C, E> {
    pub queue: Q,
    pub frontiers: F,
    pub cursor: C,
    pub events_rev: E,
}

/// Named point operations, following SlideSchedule's extraction interface.
/// No method traverses a queue or decides whether to continue it. `step` is
/// exactly one note's lifecycle call; its result does not include a queue,
/// frontier, cursor, or accumulated event buffer. Time, note fields, Natural
/// counters and container/allocation laws remain concrete adapter boundaries.
pub trait TapSchedule<Q, N, F, C, E, Event, D, Area> {
    fn peek(&self, queue: &Q) -> Option<N>;
    fn button_diff(&self, note: &N) -> D;
    fn sensor_diff(&self, note: &N) -> D;
    fn eligible(&self, note: &N) -> bool;
    fn unlocked(&self, frontiers: &F, note: &N) -> bool;
    fn fallback_area(&self, note: &N) -> Area;
    fn button_available(&self, cursor: &C) -> bool;
    fn sensor_available(&self, cursor: &C, area: &Area) -> bool;
    fn use_button(&self, cursor: C) -> C;
    fn use_sensor(&self, cursor: C, area: &Area) -> C;
    fn step(&self, note: &N, diff: D, clicked: bool) -> (N, Option<Event>);
    fn ended(&self, note: &N) -> bool;
    fn prepend_event(&self, events_rev: E, event: Event) -> E;
    fn advance(&self, queue: Q) -> Q;
    fn advance_frontier(&self, frontiers: F) -> F;
    fn replace_current(&self, queue: Q, note: &N) -> Q;
}

/// The complete queue control flow. `fuel` decreases only on a continuing
/// Ended branch and only after its nonzero guard. Passing the remaining slot
/// count suffices; the runtime wrapper need not compute `remaining + 1`.
pub fn process<Q, N, F, C, E, Event, D, Area, Ops>(
    mut fuel: usize,
    mut work: TapWork<Q, F, C, E>,
    ops: Ops,
) -> TapWork<Q, F, C, E>
where
    Ops: TapSchedule<Q, N, F, C, E, Event, D, Area>,
{
    loop {
        let before = match ops.peek(&work.queue) {
            None => return work,
            Some(note) => note,
        };
        if fuel == 0 {
            return work;
        }

        let button_diff = ops.button_diff(&before);
        let sensor_diff = ops.sensor_diff(&before);
        let allow_input = ops.eligible(&before) && ops.unlocked(&work.frontiers, &before);
        let fallback_area = ops.fallback_area(&before);
        // Both comparisons observe the cursor before either counter update.
        let plan = crate::scheduler::consume_click(
            allow_input,
            ops.button_available(&work.cursor),
            ops.sensor_available(&work.cursor, &fallback_area),
        );
        if plan.used_button {
            work.cursor = ops.use_button(work.cursor);
        }
        if plan.used_sensor {
            work.cursor = ops.use_sensor(work.cursor, &fallback_area);
        }
        let (clicked, diff) = if plan.used_button {
            (true, button_diff)
        } else if plan.used_sensor {
            (true, sensor_diff)
        } else {
            (false, button_diff)
        };
        let (after, event) = ops.step(&before, diff, clicked);
        let ended = ops.ended(&after);
        if let Some(event) = event {
            work.events_rev = ops.prepend_event(work.events_rev, event);
        }

        if ended {
            // Ended with no event still advances both positions. The old
            // queue slot is deliberately retained rather than replaced.
            work.queue = ops.advance(work.queue);
            work.frontiers = ops.advance_frontier(work.frontiers);
            let continue_queue = crate::scheduler::tap_queue_has_unused_click(
                ops.button_available(&work.cursor),
                ops.sensor_available(&work.cursor, &fallback_area),
            );
            if !continue_queue {
                return work;
            }
            fuel -= 1;
        } else {
            work.queue = ops.replace_current(work.queue, &after);
            return work;
        }
    }
}
