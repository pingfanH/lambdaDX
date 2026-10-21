//! Queue observations shared by slide lifecycles and parent scheduling.
//! These scans inspect lengths only; checkpoint payloads stay unrestricted.

/// Maximum remaining checkpoint count, including zero for no tracks.
pub fn remaining<T>(queues: &[Vec<T>]) -> usize {
    let mut maximum = 0usize;
    let mut index = 0usize;
    while index < queues.len() {
        let count = queues[index].len();
        if maximum < count {
            maximum = count;
        }
        index += 1;
    }
    maximum
}

/// Stop at the first nonempty track, matching the reference recursion.
pub fn cleared<T>(queues: &[Vec<T>]) -> bool {
    let mut index = 0usize;
    while index < queues.len() {
        let count = queues[index].len();
        if count != 0 {
            return false;
        }
        index += 1;
    }
    true
}
