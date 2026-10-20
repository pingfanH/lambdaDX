//! Ordered checkpoint traversal, shared by replay and rendered gameplay.
//!
//! The output contains hide-progress values in emission order. The runtime
//! attaches the note and optional track identifiers to these values.

use crate::lifecycle::SlideArea;
use crate::storage::SensorVec;

#[allow(clippy::vec_init_then_push)]
fn with_head<P: Clone>(head: SlideArea<P>, tail: &[SlideArea<P>]) -> Vec<SlideArea<P>> {
    let mut result = Vec::new();
    result.push(head);
    result.extend_from_slice(tail);
    result
}

fn prepend_progress<P: Clone>(emit: bool, progress: &P, rest: &[P]) -> Vec<P> {
    let mut result = Vec::new();
    if emit {
        result.push(progress.clone());
        result.extend_from_slice(rest);
    }
    result
}

// Aeneas models Vec/slice lengths; keep emptiness and singleton construction
// within that model. Fuel is decremented on every recursive call.
#[allow(clippy::len_zero)]
pub fn advance_fuel<P: Clone>(
    fuel: usize,
    emit: bool,
    queue: &[SlideArea<P>],
    held: &SensorVec<bool>,
) -> (Vec<SlideArea<P>>, Vec<P>) {
    if queue.len() == 0 {
        return (Vec::new(), Vec::new());
    }
    if fuel == 0 {
        return (queue.to_vec(), Vec::new());
    }
    let fuel = fuel - 1;
    let first = queue[0].check(held);
    let rest = &queue[1..];
    if rest.len() == 0 {
        if first.is_finished() {
            let commands = prepend_progress(emit, &first.arrow_progress_when_finished, &[]);
            (Vec::new(), commands)
        } else if first.on() {
            let commands = prepend_progress(emit, &first.arrow_progress_when_on, &[]);
            (with_head(first, rest), commands)
        } else {
            (with_head(first, rest), Vec::new())
        }
    } else if first.is_skippable || first.on() {
        let second = rest[0].check(held);
        let rest2 = &rest[1..];
        if second.is_finished() {
            let (remaining, commands) = advance_fuel(fuel, emit, rest2, held);
            let commands = prepend_progress(emit, &second.arrow_progress_when_finished, &commands);
            (remaining, commands)
        } else if second.on() {
            let next = with_head(second.clone(), rest2);
            let (remaining, commands) = advance_fuel(fuel, emit, &next, held);
            let commands = prepend_progress(emit, &second.arrow_progress_when_on, &commands);
            (remaining, commands)
        } else if first.is_finished() {
            let next = with_head(second, rest2);
            let (remaining, commands) = advance_fuel(fuel, emit, &next, held);
            let commands = prepend_progress(emit, &first.arrow_progress_when_finished, &commands);
            (remaining, commands)
        } else {
            let next = with_head(second, rest2);
            (with_head(first, &next), Vec::new())
        }
    } else if first.is_finished() {
        let (remaining, commands) = advance_fuel(fuel, emit, rest, held);
        let commands = prepend_progress(emit, &first.arrow_progress_when_finished, &commands);
        (remaining, commands)
    } else {
        (with_head(first, rest), Vec::new())
    }
}

/// Queue length supplies the reference traversal's initial fuel.
pub fn advance<P: Clone>(
    emit: bool,
    queue: &[SlideArea<P>],
    held: &SensorVec<bool>,
) -> (Vec<SlideArea<P>>, Vec<P>) {
    advance_fuel(queue.len() + 1, emit, queue, held)
}
