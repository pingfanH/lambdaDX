//! Automatic Touch traversal, shared with the scheduler.
//!
//! Group results are a fixed frame snapshot. This traversal never consumes
//! clicks or registers new group results; it visits every original slot once
//! and prepends emitted events while keeping note order.

pub struct TouchAutomaticWork<Notes, F, E> {
    pub notes: Notes,
    pub frontiers: F,
    pub events_rev: E,
}

/// Container/field operations and one note's lifecycle call. No callback
/// selects the next slot, skips a note, or traverses the queue.
pub trait TouchAutomatic<S, N, Notes, F, E, Event, D, Shared> {
    fn length(&self, source: &S) -> usize;
    fn get(&self, source: &S, index: usize) -> N;
    fn before_cursor(&self, source: &S, index: usize) -> bool;
    fn resolved(&self, note: &N) -> bool;
    fn sensor_diff(&self, note: &N) -> D;
    fn shared(&self, note: &N) -> Option<Shared>;
    fn too_late(&self, note: &N) -> bool;
    fn step(&self, note: &N, diff: D, shared: Option<Shared>) -> (N, Option<Event>);
    fn advance_frontier(&self, frontiers: F) -> F;
    fn prepend_event(&self, events: E, event: Event) -> E;
    fn append_note(&self, notes: Notes, note: N) -> Notes;
}

pub fn process<S, N, Notes, F, E, Event, D, Shared, Ops>(
    source: &S,
    mut work: TouchAutomaticWork<Notes, F, E>,
    ops: Ops,
) -> TouchAutomaticWork<Notes, F, E>
where
    Ops: TouchAutomatic<S, N, Notes, F, E, Event, D, Shared>,
{
    let len = ops.length(source);
    let mut index = 0;
    while index < len {
        let before = ops.get(source, index);
        if ops.before_cursor(source, index) || ops.resolved(&before) {
            work.notes = ops.append_note(work.notes, before);
        } else {
            let diff = ops.sensor_diff(&before);
            let shared = ops.shared(&before);
            let too_late = ops.too_late(&before);
            if shared.is_none() && !too_late {
                work.notes = ops.append_note(work.notes, before);
            } else {
                let (after, event) = ops.step(&before, diff, shared);
                let resolved_now = !ops.resolved(&before) && ops.resolved(&after);
                if resolved_now {
                    work.frontiers = ops.advance_frontier(work.frontiers);
                }
                if let Some(event) = event {
                    work.events_rev = ops.prepend_event(work.events_rev, event);
                }
                work.notes = ops.append_note(work.notes, after);
            }
        }
        index += 1;
    }
    work
}
