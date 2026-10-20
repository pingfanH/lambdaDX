//! Exact-time adapters to the finite judgment tables.
//!
//! Values outside the last threshold can use a representative with the same
//! sign. Hold percentages are computed exactly before calling the shared
//! grade table. No unbounded input is cast to a machine integer.

use crate::{
    integer::Integer,
    time::Duration,
    types::{JudgeDisplayOption, JudgeGrade},
};
use lnmai_core_verify::{judge as kernel, judgment_adapter as adapter};

pub use kernel::correct_slide_grade;

/// Only representation-level integer operations remain in this facade.
/// Arithmetic keeps Integer's promotion to BigInt and checked normalization.
struct ExactIntegerOps;

impl adapter::IntegerOps<Integer> for ExactIntegerOps {
    fn from_i64(&self, value: i64) -> Integer {
        Integer::small(value)
    }

    fn less(&self, left: &Integer, right: &Integer) -> bool {
        left < right
    }

    fn equal(&self, left: &Integer, right: &Integer) -> bool {
        left == right
    }

    fn add(&self, left: &Integer, right: &Integer) -> Integer {
        left + right
    }

    fn sub(&self, left: &Integer, right: &Integer) -> Integer {
        left - right
    }

    fn mul_i64(&self, value: &Integer, factor: i64) -> Integer {
        value * factor
    }

    fn min(&self, left: &Integer, right: &Integer) -> Integer {
        left.clone().min(right.clone())
    }

    fn max(&self, left: &Integer, right: &Integer) -> Integer {
        left.clone().max(right.clone())
    }

    fn to_i64(&self, value: &Integer) -> Option<i64> {
        value.to_i64()
    }
}

pub fn judge_tap(diff: Duration, is_ex: bool) -> JudgeGrade {
    adapter::judge_tap(&diff.micros, is_ex, &ExactIntegerOps)
}
pub fn judge_touch(diff: Duration, is_ex: bool) -> Option<JudgeGrade> {
    adapter::judge_touch(&diff.micros, is_ex, &ExactIntegerOps)
}

/// `Judge.slideClassicFastThresholds`, in first-matching threshold order.
pub fn slide_classic_fast_thresholds() -> Vec<(Duration, JudgeGrade)> {
    use crate::constants::*;
    use JudgeGrade::*;
    vec![
        (SLIDE_JUDGE_CLASSIC_FAST_SEG_1ST_PERFECT_MSEC, Perfect),
        (
            SLIDE_JUDGE_CLASSIC_FAST_SEG_2ND_PERFECT_MSEC,
            FastPerfect2nd,
        ),
        (
            SLIDE_JUDGE_CLASSIC_FAST_SEG_3RD_PERFECT_MSEC,
            FastPerfect3rd,
        ),
        (SLIDE_JUDGE_CLASSIC_FAST_SEG_1ST_GREAT_MSEC, FastGreat),
        (SLIDE_JUDGE_CLASSIC_FAST_SEG_2ND_GREAT_MSEC, FastGreat2nd),
        (SLIDE_JUDGE_CLASSIC_FAST_SEG_3RD_GREAT_MSEC, FastGreat3rd),
    ]
}

/// `Judge.slideClassicLateThresholds`, in first-matching threshold order.
pub fn slide_classic_late_thresholds() -> Vec<(Duration, JudgeGrade)> {
    use crate::constants::*;
    use JudgeGrade::*;
    vec![
        (SLIDE_JUDGE_CLASSIC_LATE_SEG_1ST_PERFECT_MSEC, Perfect),
        (
            SLIDE_JUDGE_CLASSIC_LATE_SEG_2ND_PERFECT_MSEC,
            LatePerfect2nd,
        ),
        (
            SLIDE_JUDGE_CLASSIC_LATE_SEG_3RD_PERFECT_MSEC,
            LatePerfect3rd,
        ),
        (SLIDE_JUDGE_CLASSIC_LATE_SEG_1ST_GREAT_MSEC, LateGreat),
        (SLIDE_JUDGE_CLASSIC_LATE_SEG_2ND_GREAT_MSEC, LateGreat2nd),
        (SLIDE_JUDGE_CLASSIC_LATE_SEG_3RD_GREAT_MSEC, LateGreat3rd),
    ]
}

pub fn judge_slide_classic(diff: Duration) -> JudgeGrade {
    adapter::judge_slide_classic(&diff.micros, &ExactIntegerOps)
}
pub fn judge_slide_modern(diff: Duration, stay: Duration, is_ex: bool) -> JudgeGrade {
    adapter::judge_slide_modern(&diff.micros, &stay.micros, is_ex, &ExactIntegerOps)
}
pub fn judge_slide_too_late(remaining: impl Into<Integer>) -> JudgeGrade {
    let remaining = remaining.into();
    adapter::judge_slide_too_late(&remaining, &ExactIntegerOps)
}
pub fn is_too_late_slide(diff: Duration, user_offset: Duration) -> bool {
    adapter::is_too_late_slide(&diff.micros, &user_offset.micros, &ExactIntegerOps)
}
pub fn judge_hold_end(
    head: JudgeGrade,
    diff: Duration,
    length: Duration,
    ignore: Duration,
    release: Duration,
) -> JudgeGrade {
    adapter::judge_hold_end(
        head,
        &diff.micros,
        &length.micros,
        &ignore.micros,
        &release.micros,
        &ExactIntegerOps,
    )
}
pub fn judge_hold_classic_end(
    head: JudgeGrade,
    timing: impl Into<Integer>,
    length: Duration,
    release: impl Into<Integer>,
) -> JudgeGrade {
    let timing = timing.into();
    let release = release.into();
    adapter::judge_hold_classic_end(head, &timing, &length.micros, &release, &ExactIntegerOps)
}
pub fn count_fast_late(
    grade: JudgeGrade,
    diff: Duration,
    display: JudgeDisplayOption,
) -> (bool, bool) {
    adapter::count_fast_late(grade, &diff.micros, display, &ExactIntegerOps)
}
