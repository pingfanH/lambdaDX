//! Scalar queue budgets shared by the runtime scheduler and extraction.
//!
//! The runtime reads length before converting its Natural cursor, then calls
//! one scalar helper. Conversion remains a single runtime operation; these
//! helpers neither interpret BigUint nor copy a queue.

/// Keep the actual Vec length primitive in the extracted subset. The Vec
/// reference is intentional: no slice conversion or element clone is needed.
pub fn vec_len<T>(notes: &[T]) -> usize {
    notes.len()
}

/// Remaining slots, including failed conversion and cursors past the end.
/// Tap and queue normalization use this count without adding one.
pub fn remaining_scalar(len: usize, converted_cursor: Option<usize>) -> usize {
    len.saturating_sub(converted_cursor.unwrap_or(usize::MAX))
}

/// The outer Touch head budget retains the runtime's ordinary +1.
/// Its successful extracted execution requires remaining + 1 <= usize::MAX;
/// this helper deliberately preserves the original arithmetic operation.
pub fn head_fuel_scalar(len: usize, converted_cursor: Option<usize>) -> usize {
    remaining_scalar(len, converted_cursor) + 1
}
