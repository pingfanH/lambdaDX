//! Ordered removal using modeled slice swaps and the shrinking resize branch.

/// Remove a valid element while preserving the order of all retained elements.
/// `filler` is required by `resize`; this call always shrinks, so it is never
/// cloned or inserted. Runtime callers use value types without observable Drop.
pub fn erase<T: Clone>(values: &mut Vec<T>, mut index: usize, filler: T) {
    let len = values.len();
    assert!(index < len);
    let last = len - 1;
    while index < last {
        let next = index + 1;
        values.swap(index, next);
        index = next;
    }
    values.resize(last, filler);
}
