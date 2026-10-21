//! Score and combo computation.
//!
//! Mirrors `LnmaiCore/Score.lean`.

use super::time::Duration;
use super::types::{JudgeDisplayOption, JudgeGrade, NoteType};

/// Base score for a note type.
pub fn base_score(nt: NoteType) -> u32 {
    nt.base_score()
}

/// Score a non-break note: returns `(earned, lost)`.
pub fn score_non_break(base: u32, grade: JudgeGrade, multiple: u32) -> (u32, u32) {
    let b = base * multiple;
    match grade {
        JudgeGrade::Miss | JudgeGrade::TooFast => (0, b),
        JudgeGrade::LateGood | JudgeGrade::FastGood => (b / 2, b - b / 2),
        JudgeGrade::LateGreat
        | JudgeGrade::LateGreat2nd
        | JudgeGrade::LateGreat3rd
        | JudgeGrade::FastGreat
        | JudgeGrade::FastGreat2nd
        | JudgeGrade::FastGreat3rd => (b * 4 / 5, b - b * 4 / 5),
        JudgeGrade::LatePerfect3rd
        | JudgeGrade::LatePerfect2nd
        | JudgeGrade::Perfect
        | JudgeGrade::FastPerfect2nd
        | JudgeGrade::FastPerfect3rd => (b, 0),
    }
}

/// Score a Break note: returns
/// `(baseEarned, extraEarned, classicExtraEarned, baseLost, extraLost, classicExtraLost)`.
pub fn score_break(grade: JudgeGrade, multiple: u32) -> (u32, u32, u32, u32, u32, u32) {
    let m = multiple;
    match grade {
        JudgeGrade::Miss | JudgeGrade::TooFast => (0, 0, 0, 2500 * m, 100 * m, 100 * m),
        JudgeGrade::LateGood | JudgeGrade::FastGood => {
            (1000 * m, 30 * m, 0, 1500 * m, 70 * m, 100 * m)
        }
        JudgeGrade::LateGreat3rd | JudgeGrade::FastGreat3rd => {
            (1250 * m, 40 * m, 0, 1250 * m, 60 * m, 100 * m)
        }
        JudgeGrade::LateGreat2nd | JudgeGrade::FastGreat2nd => {
            (1500 * m, 40 * m, 0, 1000 * m, 60 * m, 100 * m)
        }
        JudgeGrade::LateGreat | JudgeGrade::FastGreat => {
            (2000 * m, 40 * m, 0, 500 * m, 60 * m, 100 * m)
        }
        JudgeGrade::LatePerfect3rd | JudgeGrade::FastPerfect3rd => {
            (2500 * m, 50 * m, 0, 0, 50 * m, 100 * m)
        }
        JudgeGrade::LatePerfect2nd | JudgeGrade::FastPerfect2nd => {
            (2500 * m, 75 * m, 50 * m, 0, 25 * m, 50 * m)
        }
        JudgeGrade::Perfect => (2500 * m, 100 * m, 100 * m, 0, 0, 0),
    }
}

/// Combo delta produced by one judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ComboDelta {
    pub combo: u32,
    pub p_combo: u32,
    pub c_p_combo: u32,
    pub dx_score_lost: i64,
}

/// Update combo counters for a single note judgment.
pub fn update_combo(
    combo: u32,
    p_combo: u32,
    c_p_combo: u32,
    dx_score_lost: i64,
    grade: JudgeGrade,
    multiple: u32,
) -> ComboDelta {
    let m = multiple as i64;
    match grade {
        JudgeGrade::Perfect => ComboDelta {
            combo: combo + multiple,
            p_combo: p_combo + multiple,
            c_p_combo: c_p_combo + multiple,
            dx_score_lost,
        },
        JudgeGrade::LatePerfect2nd
        | JudgeGrade::FastPerfect2nd
        | JudgeGrade::LatePerfect3rd
        | JudgeGrade::FastPerfect3rd => ComboDelta {
            combo: combo + multiple,
            p_combo: p_combo + multiple,
            c_p_combo: 0,
            dx_score_lost: dx_score_lost - m,
        },
        JudgeGrade::LateGreat3rd
        | JudgeGrade::LateGreat2nd
        | JudgeGrade::LateGreat
        | JudgeGrade::FastGreat
        | JudgeGrade::FastGreat2nd
        | JudgeGrade::FastGreat3rd => ComboDelta {
            combo: combo + multiple,
            p_combo: 0,
            c_p_combo: 0,
            dx_score_lost: dx_score_lost - 2 * m,
        },
        JudgeGrade::LateGood | JudgeGrade::FastGood => ComboDelta {
            combo: combo + multiple,
            p_combo: 0,
            c_p_combo: 0,
            dx_score_lost: dx_score_lost - 3 * m,
        },
        JudgeGrade::Miss | JudgeGrade::TooFast => ComboDelta {
            combo: 0,
            p_combo: 0,
            c_p_combo: 0,
            dx_score_lost: dx_score_lost - 3 * m,
        },
    }
}

/// Fast/late increment flags for a grade under a display option.
pub fn count_fast_late(
    grade: JudgeGrade,
    diff: Duration,
    display: JudgeDisplayOption,
) -> (bool, bool) {
    if grade.is_miss_or_too_fast() {
        (false, false)
    } else {
        let d = grade.dist_from_perfect();
        match display {
            JudgeDisplayOption::All => {
                if diff.to_micros() == 0 {
                    (false, false)
                } else if diff.to_micros() < 0 {
                    (true, false)
                } else {
                    (false, true)
                }
            }
            JudgeDisplayOption::BelowCP => {
                if matches!(grade, JudgeGrade::Perfect) {
                    (false, false)
                } else if diff.to_micros() < 0 {
                    (true, false)
                } else {
                    (false, true)
                }
            }
            JudgeDisplayOption::BelowP
            | JudgeDisplayOption::BelowGR
            | JudgeDisplayOption::Disable => {
                if d <= 2 {
                    (false, false)
                } else if diff.to_micros() < 0 {
                    (true, false)
                } else {
                    (false, true)
                }
            }
            JudgeDisplayOption::MissOnly => (false, false),
        }
    }
}

/// DX score rank (5 = SSS+, 0 = none).
pub fn dx_score_rank(achieved: u32, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    // Products are at most u32::MAX * 100 and fit exactly in u64.
    let meets = |threshold: u64| -> bool { (achieved as u64) * 100 >= (max as u64) * threshold };
    if meets(97) {
        5
    } else if meets(95) {
        4
    } else if meets(93) {
        3
    } else if meets(90) {
        2
    } else if meets(85) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_score() {
        assert!(matches!(base_score(NoteType::Tap), 500));
        assert!(matches!(base_score(NoteType::Break), 2500));
    }

    #[test]
    fn test_score_non_break() {
        assert!(matches!(
            score_non_break(500, JudgeGrade::Perfect, 1),
            (500, 0)
        ));
        assert!(matches!(
            score_non_break(500, JudgeGrade::Miss, 1),
            (0, 500)
        ));
    }
}
