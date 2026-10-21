//! Exact score arithmetic; finite grade coefficients come from the verified core.
pub use crate::accuracy::{compute_acc_rates, AccRates};
pub use crate::judge::count_fast_late;
use crate::{integer::Integer, natural::Natural, types::JudgeGrade};
pub use kernel::base_score;
use lnmai_core_verify::score as kernel;

/// `LnmaiCore.Score.extraScore`: DX extra score of one Break note.
pub const EXTRA_SCORE: u32 = 100;

pub fn score_non_break(
    base: impl Into<Natural>,
    grade: JudgeGrade,
    multiple: impl Into<Natural>,
) -> (Natural, Natural) {
    let b = base.into() * multiple.into();
    let coefficient = kernel::score_non_break(10, grade, 1).0;
    let earned = &b * coefficient / 10;
    let lost = b - &earned;
    (earned, lost)
}

pub fn score_break(
    grade: JudgeGrade,
    multiple: impl Into<Natural>,
) -> (Natural, Natural, Natural, Natural, Natural, Natural) {
    let m = multiple.into();
    let (a, b, c, d, e, f) = kernel::score_break(grade, 1);
    (&m * a, &m * b, &m * c, &m * d, &m * e, &m * f)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComboDelta {
    pub combo: Natural,
    pub p_combo: Natural,
    pub c_p_combo: Natural,
    pub dx_score_lost: Integer,
}

pub fn update_combo(
    combo: impl Into<Natural>,
    p_combo: impl Into<Natural>,
    c_p_combo: impl Into<Natural>,
    dx_score_lost: impl Into<Integer>,
    grade: JudgeGrade,
    multiple: impl Into<Natural>,
) -> ComboDelta {
    let m = multiple.into();
    let delta = kernel::update_combo(0, 0, 0, 0, grade, 1);
    ComboDelta {
        combo: if delta.combo == 0 {
            Natural::default()
        } else {
            combo.into() + &m
        },
        p_combo: if delta.p_combo == 0 {
            Natural::default()
        } else {
            p_combo.into() + &m
        },
        c_p_combo: if delta.c_p_combo == 0 {
            Natural::default()
        } else {
            c_p_combo.into() + &m
        },
        dx_score_lost: dx_score_lost.into() + Integer::from(delta.dx_score_lost) * Integer::from(m),
    }
}

pub fn dx_score_rank(achieved: impl Into<Natural>, max: impl Into<Natural>) -> u32 {
    let max = max.into();
    if max == 0 {
        return 0;
    }
    let a = achieved.into() * 100;
    for (threshold, rank) in [(97, 5), (95, 4), (93, 3), (90, 2), (85, 1)] {
        if a >= &max * threshold {
            return rank;
        }
    }
    0
}
