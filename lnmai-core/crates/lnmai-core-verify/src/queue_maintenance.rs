//! Queue cursor normalization and replacement shared with the runtime.
//!
//! The kernel owns every traversal, branch, fuel decrement, and slot counter.
//! Adapters perform only one point operation; note identifiers stay opaque.
//! No uniqueness or well-formed-cursor assumption is used by the runtime wiring.

/// Point operations for one cursor step. `advance` preserves the notes and
/// increments only the cursor. `resolved` observes exactly one note's state.
/// Named traits avoid higher-ranked borrowed closure bounds in extraction.
pub trait QueueCursor<Q, N> {
    fn peek(&self, queue: &Q) -> Option<N>;
    fn resolved(&self, note: &N) -> bool;
    fn advance(&self, queue: Q) -> Q;
}

/// Mirrors `normalize{Hold,Touch}QueueCursorFuel`: peek first, then inspect
/// fuel, then inspect resolution. Even fuel zero does not call `resolved`.
/// The runtime passes the number of remaining slots, without adding one.
/// Equivalence of that budget to the reference's remaining + 1 is a pending
/// proof obligation, dependent on the concrete peek/advance adapter laws.
pub fn normalize_cursor_fuel<Q, N, Ops>(mut fuel: usize, mut queue: Q, ops: Ops) -> Q
where
    Ops: QueueCursor<Q, N>,
{
    loop {
        let Some(note) = ops.peek(&queue) else {
            return queue;
        };
        if fuel == 0 {
            return queue;
        }
        if !ops.resolved(&note) {
            return queue;
        }
        queue = ops.advance(queue);
        fuel -= 1;
    }
}

/// Read and write exactly one in-bounds slot. `index_equal` compares the
/// complete note identifiers, without converting them to a machine integer.
/// `write` preserves queue length, cursor, order, and every other slot.
pub trait QueueReplace<Q, N> {
    fn get(&self, queue: &Q, index: usize) -> N;
    fn index_equal(&self, existing: &N, replacement: &N) -> bool;
    fn write(&self, queue: Q, index: usize, replacement: &N) -> Q;
}

/// Replace every matching identifier across the entire queue, including
/// slots before its cursor and every duplicate. Never stop after a match.
/// `len` is the initial slot count; point writes must preserve that count.
/// Thus `index < len` justifies each read/write and also `index += 1`, even
/// when len is usize::MAX. The cursor is neither read nor normalized here.
pub fn replace_all<Q, N, Ops>(len: usize, mut queue: Q, replacement: &N, ops: Ops) -> Q
where
    Ops: QueueReplace<Q, N>,
{
    let mut index = 0;
    while index < len {
        let existing = ops.get(&queue, index);
        if ops.index_equal(&existing, replacement) {
            queue = ops.write(queue, index, replacement);
        }
        index += 1;
    }
    queue
}
