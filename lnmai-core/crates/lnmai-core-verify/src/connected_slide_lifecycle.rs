//! Connected-slide timing state transition.
//!
//! This kernel is the exact arithmetic part of the normalization source
//! function. Source-group identity and flags are computed by
//! connected_slide_metadata; this module owns the derived timing fields and
//! the state carried to the next slide. Times use the same canonical signed
//! integer representation as the runtime Integer, so the transition does not
//! narrow the Lean timeline.

use crate::big_int::{self, BigIntKernel};

/// Timing fields written to the current slide and the chain state consumed by
/// the next source-group item.
pub struct TimingStep {
    pub start_timing: BigIntKernel,
    pub stored_group_start_timing: Option<BigIntKernel>,
    pub judge_at: Option<BigIntKernel>,
    pub next_parent_end_timing: Option<BigIntKernel>,
    pub next_group_start_timing: Option<BigIntKernel>,
}

/// Apply the timing part of one valid connected-slide metadata step.
///
/// original_start_timing and original_judge_at are the fields produced by
/// initial slide lowering. Missing predecessor state falls back to the
/// original start exactly as the Lean source does. The caller clears all
/// returned state when the metadata step is Clear or Reset.
pub fn apply(
    is_conn_slide: bool,
    is_group_head: bool,
    is_group_end: bool,
    original_start_timing: BigIntKernel,
    original_judge_at: Option<BigIntKernel>,
    length: BigIntKernel,
    parent_end_timing: Option<BigIntKernel>,
    group_start_timing: Option<BigIntKernel>,
) -> TimingStep {
    let start_timing = if is_group_head {
        original_start_timing.clone()
    } else {
        match parent_end_timing {
            Some(value) => value,
            None => original_start_timing.clone(),
        }
    };
    let group_start = if is_group_head {
        original_start_timing.clone()
    } else {
        match group_start_timing {
            Some(value) => value,
            None => original_start_timing.clone(),
        }
    };
    let stored_group_start_timing = if is_conn_slide {
        Some(group_start.clone())
    } else {
        None
    };
    let start_shift = big_int::sub(&start_timing, &original_start_timing);
    let judge_at = match original_judge_at {
        Some(value) => Some(big_int::add(&value, &start_shift)),
        None => None,
    };
    let next_parent_end = big_int::add(&start_timing, &length);
    let next_parent_end_timing = if is_group_end {
        None
    } else {
        Some(next_parent_end)
    };
    let next_group_start_timing = if is_group_end {
        None
    } else {
        Some(group_start)
    };
    TimingStep {
        start_timing,
        stored_group_start_timing,
        judge_at,
        next_parent_end_timing,
        next_group_start_timing,
    }
}
