//! Ordered front insertion using the modeled push and slice-swap operations.
//! No old element is cloned or dropped; duplicates retain their source order.

/// Move `head` to the front after pushing it into the existing allocation.
/// Every swap shifts the new element one slot left. Successful execution
/// requires space for one extra element, just as insertion at zero does.
pub fn prepend<T>(values: &mut Vec<T>, head: T) {
    let mut index = values.len();
    values.push(head);
    while index > 0 {
        let previous = index - 1;
        values.swap(index, previous);
        index = previous;
    }
}
