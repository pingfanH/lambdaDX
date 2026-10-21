//! Exact-integer adapters shared with the runtime judgment API.
//!
//! Integer representation and individual arithmetic operations remain outside
//! extraction. Clamping, exact hold formulas, percentage selection and calls
//! into the existing judgment/time/score kernels live here.

use super::{
    judge, score,
    time::Duration,
    types::{JudgeDisplayOption, JudgeGrade},
};

/// Each method denotes one exact integer operation, with no machine-width
/// reduction. `less` is strict integer order, `equal` is integer equality,
/// and `min`/`max` return the corresponding integer value. `to_i64` returns
/// `Some(n)` exactly when the represented integer is the representable `n`;
/// it returns `None` outside the i64 range. Implementations must be pure.
/// No method performs a judgment, clamp, or compound hold calculation.
#[allow(clippy::wrong_self_convention)]
pub trait IntegerOps<I> {
    fn from_i64(&self, value: i64) -> I;
    fn less(&self, left: &I, right: &I) -> bool;
    fn equal(&self, left: &I, right: &I) -> bool;
    fn add(&self, left: &I, right: &I) -> I;
    fn sub(&self, left: &I, right: &I) -> I;
    fn mul_i64(&self, value: &I, factor: i64) -> I;
    fn min(&self, left: &I, right: &I) -> I;
    fn max(&self, left: &I, right: &I) -> I;
    fn to_i64(&self, value: &I) -> Option<i64>;
}

/// Clamp to an inclusive interval, whose callers must supply ordered ends.
/// Taking both ends explicitly avoids negating an arbitrary i64 limit.
/// Conversion occurs only after the exact comparisons establish its range.
/// A failed conversion remains a failure, never a truncated/fallback value.
pub fn bounded<I, Ops: IntegerOps<I>>(value: &I, lower: i64, upper: i64, ops: &Ops) -> i64 {
    if ops.less(value, &ops.from_i64(lower)) {
        lower
    } else if ops.less(&ops.from_i64(upper), value) {
        upper
    } else {
        match ops.to_i64(value) {
            Some(value) => value,
            None => panic!("value lies in the checked i64 interval"),
        }
    }
}

/// The endpoints lie beyond every fixed judgment threshold, preserve zero
/// and sign, and keep the bounded kernels' absolute values safe.
pub fn diff_rep<I, Ops: IntegerOps<I>>(value: &I, ops: &Ops) -> Duration {
    Duration::from_micros(bounded(value, -1_000_000, 1_000_000, ops))
}

pub fn judge_tap<I, Ops: IntegerOps<I>>(diff: &I, is_ex: bool, ops: &Ops) -> JudgeGrade {
    judge::judge_tap(diff_rep(diff, ops), is_ex)
}

pub fn judge_touch<I, Ops: IntegerOps<I>>(diff: &I, is_ex: bool, ops: &Ops) -> Option<JudgeGrade> {
    judge::judge_touch(diff_rep(diff, ops), is_ex)
}

pub fn judge_slide_classic<I, Ops: IntegerOps<I>>(diff: &I, ops: &Ops) -> JudgeGrade {
    judge::judge_slide_classic(diff_rep(diff, ops))
}

pub fn judge_slide_modern<I, Ops: IntegerOps<I>>(
    diff: &I,
    stay: &I,
    is_ex: bool,
    ops: &Ops,
) -> JudgeGrade {
    // Below the lower bound all perfect windows are negative; above the
    // upper bound the extension is capped. Division remains in the existing
    // time kernel, including its Euclidean treatment of negative values.
    judge::judge_slide_modern(
        diff_rep(diff, ops),
        Duration::from_micros(bounded(stay, -4_000_000, 4_000_000, ops)),
        is_ex,
    )
}

/// Only equality with one matters; no arbitrary integer is narrowed to u32.
pub fn judge_slide_too_late<I, Ops: IntegerOps<I>>(remaining: &I, ops: &Ops) -> JudgeGrade {
    let representative = if ops.equal(remaining, &ops.from_i64(1)) {
        1
    } else {
        0
    };
    judge::judge_slide_too_late(representative)
}

pub fn is_too_late_slide<I, Ops: IntegerOps<I>>(diff: &I, user_offset: &I, ops: &Ops) -> bool {
    let zero = ops.from_i64(0);
    let min_offset = ops.min(user_offset, &zero);
    // diff > 600012 + min_offset iff diff - min_offset > 600012.
    // Subtract exactly before clamping: large inputs can cancel near the
    // boundary, so independently clamping diff and offset would be wrong.
    let adjusted_diff = ops.sub(diff, &min_offset);
    judge::is_too_late_slide(diff_rep(&adjusted_diff, ops), Duration::zero())
}

/// Exact cross-multiplication in the original first-matching band order.
/// No division, rounding, bounded products, or formula callback is used.
pub fn press_band_micros<I, Ops: IntegerOps<I>>(held: &I, reality: &I, ops: &Ops) -> u32 {
    if !ops.less(held, reality) {
        0
    } else if !ops.less(&ops.mul_i64(held, 100), &ops.mul_i64(reality, 67)) {
        1
    } else if !ops.less(&ops.mul_i64(held, 100), &ops.mul_i64(reality, 33)) {
        2
    } else if !ops.less(&ops.mul_i64(held, 100), &ops.mul_i64(reality, 5)) {
        3
    } else {
        4
    }
}

pub fn judge_hold_end<I, Ops: IntegerOps<I>>(
    head: JudgeGrade,
    diff: &I,
    length: &I,
    ignore: &I,
    release: &I,
    ops: &Ops,
) -> JudgeGrade {
    let zero = ops.from_i64(0);
    let maximum_raw = ops.sub(length, &ops.from_i64(300000));
    let maximum = ops.max(&maximum_raw, &zero);
    let reality_raw = ops.sub(length, ignore);
    let nonnegative_reality = ops.max(&reality_raw, &zero);
    let reality = ops.min(&nonnegative_reality, &maximum);
    if !ops.less(&zero, &reality) {
        return head;
    }
    let held_raw = ops.sub(&reality, release);
    let held = ops.max(&held_raw, &zero);
    let band = press_band_micros(&held, &reality, ops);
    judge::hold_band_grade(head, !ops.less(diff, &zero), band)
}

pub fn judge_hold_classic_end<I, Ops: IntegerOps<I>>(
    head: JudgeGrade,
    timing: &I,
    length: &I,
    release: &I,
    ops: &Ops,
) -> JudgeGrade {
    let end = ops.add(timing, length);
    let diff = ops.sub(&end, release);
    judge::judge_hold_classic_end(
        head,
        bounded(&diff, -1_000_000, 1_000_000, ops),
        Duration::zero(),
        0,
    )
}

pub fn count_fast_late<I, Ops: IntegerOps<I>>(
    grade: JudgeGrade,
    diff: &I,
    display: JudgeDisplayOption,
    ops: &Ops,
) -> (bool, bool) {
    score::count_fast_late(grade, diff_rep(diff, ops), display)
}
