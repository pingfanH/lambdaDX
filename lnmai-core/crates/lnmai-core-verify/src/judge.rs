//! Aeneas-friendly judge module
//!
//! Pure judgment functions

use super::constants::*;
use super::time::Duration;
use super::types::JudgeGrade;

/// Judge a tap note
pub fn judge_tap(diff: Duration, is_ex: bool) -> JudgeGrade {
    if is_ex {
        return JudgeGrade::Perfect;
    }

    let is_fast = diff.micros < 0;
    let abs_diff = diff.abs();

    if abs_diff.micros <= TAP_PERFECT_1ST.micros {
        JudgeGrade::Perfect
    } else if abs_diff.micros <= TAP_PERFECT_2ND.micros {
        if is_fast {
            JudgeGrade::FastPerfect2nd
        } else {
            JudgeGrade::LatePerfect2nd
        }
    } else if abs_diff.micros <= TAP_PERFECT_3RD.micros {
        if is_fast {
            JudgeGrade::FastPerfect3rd
        } else {
            JudgeGrade::LatePerfect3rd
        }
    } else if abs_diff.micros <= TAP_GREAT_1ST.micros {
        if is_fast {
            JudgeGrade::FastGreat
        } else {
            JudgeGrade::LateGreat
        }
    } else if abs_diff.micros <= TAP_GREAT_2ND.micros {
        if is_fast {
            JudgeGrade::FastGreat2nd
        } else {
            JudgeGrade::LateGreat2nd
        }
    } else if abs_diff.micros <= TAP_GREAT_3RD.micros {
        if is_fast {
            JudgeGrade::FastGreat3rd
        } else {
            JudgeGrade::LateGreat3rd
        }
    } else {
        if is_fast {
            JudgeGrade::FastGood
        } else {
            JudgeGrade::LateGood
        }
    }
}

/// Judge a touch note
pub fn judge_touch(diff: Duration, _is_ex: bool) -> Option<JudgeGrade> {
    let is_fast = diff.micros < 0;
    let abs_diff = diff.abs();

    if is_fast && abs_diff.micros > TOUCH_PERFECT_1ST.micros {
        return None;
    }

    let grade = if abs_diff.micros <= TOUCH_PERFECT_1ST.micros {
        JudgeGrade::Perfect
    } else if abs_diff.micros <= TOUCH_PERFECT_2ND.micros {
        JudgeGrade::LatePerfect2nd
    } else if abs_diff.micros <= TOUCH_PERFECT_3RD.micros {
        JudgeGrade::LatePerfect3rd
    } else if abs_diff.micros <= TOUCH_GREAT_1ST.micros {
        JudgeGrade::LateGreat
    } else if abs_diff.micros <= TOUCH_GREAT_2ND.micros {
        JudgeGrade::LateGreat2nd
    } else if abs_diff.micros <= TOUCH_GREAT_3RD.micros {
        JudgeGrade::LateGreat3rd
    } else {
        JudgeGrade::LateGood
    };

    Some(grade)
}

/// Judge a classic slide
pub fn judge_slide_classic(diff: Duration) -> JudgeGrade {
    let is_fast = diff.micros < 0;
    let abs_diff = diff.abs();

    if is_fast {
        if abs_diff.micros <= 66668 {
            JudgeGrade::Perfect
        } else if abs_diff.micros <= 133336 {
            JudgeGrade::FastPerfect2nd
        } else if abs_diff.micros <= 200004 {
            JudgeGrade::FastPerfect3rd
        } else if abs_diff.micros <= 266672 {
            JudgeGrade::FastGreat
        } else if abs_diff.micros <= 333340 {
            JudgeGrade::FastGreat2nd
        } else if abs_diff.micros <= 400008 {
            JudgeGrade::FastGreat3rd
        } else {
            JudgeGrade::FastGood
        }
    } else {
        if abs_diff.micros <= 66668 {
            JudgeGrade::Perfect
        } else if abs_diff.micros <= 133336 {
            JudgeGrade::LatePerfect2nd
        } else if abs_diff.micros <= 200004 {
            JudgeGrade::LatePerfect3rd
        } else if abs_diff.micros <= 266672 {
            JudgeGrade::LateGreat
        } else if abs_diff.micros <= 333340 {
            JudgeGrade::LateGreat2nd
        } else if abs_diff.micros <= 400008 {
            JudgeGrade::LateGreat3rd
        } else {
            JudgeGrade::LateGood
        }
    }
}

/// Correct slide grade (collapse subdivided grades)
pub fn correct_slide_grade(grade: JudgeGrade) -> JudgeGrade {
    match grade {
        JudgeGrade::LatePerfect3rd
        | JudgeGrade::LatePerfect2nd
        | JudgeGrade::FastPerfect2nd
        | JudgeGrade::FastPerfect3rd => JudgeGrade::Perfect,
        _ => grade,
    }
}

/// Judge slide too late
pub fn judge_slide_too_late(queue_remaining: u32) -> JudgeGrade {
    if queue_remaining == 1 {
        JudgeGrade::LateGood
    } else {
        JudgeGrade::Miss
    }
}

// ============================================================================
// Modern Slide Judgment (dynamic extension)
// From SlideBase.Judge(), lines 224-273
// ============================================================================

/// Judge a modern (deluxe) slide. The 3rd-perfect window is dynamically extended
/// based on `stay_time` (last wait time at slide end).
pub fn judge_slide_modern(diff: Duration, stay_time: Duration, _is_ex: bool) -> JudgeGrade {
    let is_fast = diff.micros < 0;
    let diff_msec = diff.abs();

    // Dynamic extension: ext = min(stay_time / 4, 22-frame max)
    let stay_div4 = stay_time.div_nat(4);
    let ext = if stay_div4.micros < SLIDE_MAX_EXT.micros {
        stay_div4
    } else {
        SLIDE_MAX_EXT
    };
    let seg_3rd_perfect = Duration {
        micros: SLIDE_PERFECT_3RD.micros + ext.micros,
    };
    let seg_1st_perfect = seg_3rd_perfect.div_nat(3);
    let seg_2nd_perfect = seg_3rd_perfect.scale_nat(2).div_nat(3);

    if diff_msec.micros <= seg_1st_perfect.micros {
        JudgeGrade::Perfect
    } else if diff_msec.micros <= seg_2nd_perfect.micros {
        if is_fast {
            JudgeGrade::FastPerfect2nd
        } else {
            JudgeGrade::LatePerfect2nd
        }
    } else if diff_msec.micros <= seg_3rd_perfect.micros {
        if is_fast {
            JudgeGrade::FastPerfect3rd
        } else {
            JudgeGrade::LatePerfect3rd
        }
    } else if diff_msec.micros <= SLIDE_GREAT_1ST.micros {
        if is_fast {
            JudgeGrade::FastGreat
        } else {
            JudgeGrade::LateGreat
        }
    } else if diff_msec.micros <= SLIDE_GREAT_2ND.micros {
        if is_fast {
            JudgeGrade::FastGreat2nd
        } else {
            JudgeGrade::LateGreat2nd
        }
    } else if diff_msec.micros <= SLIDE_GREAT_3RD.micros {
        if is_fast {
            JudgeGrade::FastGreat3rd
        } else {
            JudgeGrade::LateGreat3rd
        }
    } else {
        if is_fast {
            JudgeGrade::FastGood
        } else {
            JudgeGrade::LateGood
        }
    }
}

// ============================================================================
// Hold End Judgment (Deluxe/Modern) — press-band lookup table
// From NoteLongDrop.HoldEndJudge(), lines 66-255
// ============================================================================

/// Compute the press band index from held percentage:
///   0: >= 100%, 1: [67%,100%), 2: [33%,67%), 3: [5%,33%), 4: [0%,5%)
fn press_band_micros(held_micros: i64, reality_micros: i64) -> u32 {
    if held_micros >= reality_micros {
        0
    } else if held_micros * 100 >= reality_micros * 67 {
        1
    } else if held_micros * 100 >= reality_micros * 33 {
        2
    } else if held_micros * 100 >= reality_micros * 5 {
        3
    } else {
        4
    }
}

/// HoldEndJudge: computes the final hold grade from head grade and how
/// long the player held the button.
pub fn judge_hold_end(
    head_grade: JudgeGrade,
    judge_diff: Duration,
    length: Duration,
    ignore_time: Duration,
    player_release_time: Duration,
) -> JudgeGrade {
    let reality_ht_raw = Duration {
        micros: length.micros - ignore_time.micros,
    };
    let hold_max = Duration::from_micros(300000);
    let reality_ht_max = if length.micros > hold_max.micros {
        Duration {
            micros: length.micros - hold_max.micros,
        }
    } else {
        Duration::zero()
    };
    let zero = Duration::zero();
    let reality_ht = if reality_ht_raw.micros < zero.micros {
        zero
    } else if reality_ht_raw.micros > reality_ht_max.micros {
        reality_ht_max
    } else {
        reality_ht_raw
    };

    if reality_ht.micros <= 0 {
        return head_grade;
    }

    let held_raw = Duration {
        micros: reality_ht.micros - player_release_time.micros,
    };
    let held = if held_raw.micros < zero.micros {
        zero
    } else {
        held_raw
    };
    let band = press_band_micros(held.to_micros(), reality_ht.to_micros());
    hold_band_grade(head_grade, judge_diff.micros >= 0, band)
}

/// Finite modern-hold grade table, shared by the bounded kernel and the
/// arbitrary-precision runtime. Band 4 (and larger values) is below 5%.
pub fn hold_band_grade(head_grade: JudgeGrade, is_late: bool, band: u32) -> JudgeGrade {
    match band {
        0 => match head_grade {
            JudgeGrade::LatePerfect3rd
            | JudgeGrade::LatePerfect2nd
            | JudgeGrade::Perfect
            | JudgeGrade::FastPerfect2nd
            | JudgeGrade::FastPerfect3rd => head_grade,
            JudgeGrade::LateGood
            | JudgeGrade::LateGreat3rd
            | JudgeGrade::LateGreat2nd
            | JudgeGrade::LateGreat => JudgeGrade::LateGreat,
            JudgeGrade::FastGreat
            | JudgeGrade::FastGreat2nd
            | JudgeGrade::FastGreat3rd
            | JudgeGrade::FastGood => JudgeGrade::FastGreat,
            JudgeGrade::Miss => JudgeGrade::LateGood,
            JudgeGrade::TooFast => JudgeGrade::FastGood,
        },
        1 => match head_grade {
            JudgeGrade::Perfect => {
                if is_late {
                    JudgeGrade::LatePerfect2nd
                } else {
                    JudgeGrade::FastPerfect2nd
                }
            }
            JudgeGrade::LatePerfect3rd
            | JudgeGrade::LatePerfect2nd
            | JudgeGrade::FastPerfect2nd
            | JudgeGrade::FastPerfect3rd => head_grade,
            JudgeGrade::LateGood
            | JudgeGrade::LateGreat3rd
            | JudgeGrade::LateGreat2nd
            | JudgeGrade::LateGreat => JudgeGrade::LateGreat,
            JudgeGrade::FastGreat
            | JudgeGrade::FastGreat2nd
            | JudgeGrade::FastGreat3rd
            | JudgeGrade::FastGood => JudgeGrade::FastGreat,
            JudgeGrade::Miss => JudgeGrade::LateGood,
            JudgeGrade::TooFast => JudgeGrade::FastGood,
        },
        2 => match head_grade {
            JudgeGrade::Perfect => {
                if is_late {
                    JudgeGrade::LateGreat2nd
                } else {
                    JudgeGrade::FastGreat2nd
                }
            }
            JudgeGrade::LateGood
            | JudgeGrade::LateGreat3rd
            | JudgeGrade::LateGreat2nd
            | JudgeGrade::LateGreat
            | JudgeGrade::LatePerfect3rd
            | JudgeGrade::LatePerfect2nd => JudgeGrade::LateGreat,
            JudgeGrade::FastPerfect2nd
            | JudgeGrade::FastPerfect3rd
            | JudgeGrade::FastGreat
            | JudgeGrade::FastGreat2nd
            | JudgeGrade::FastGreat3rd
            | JudgeGrade::FastGood => JudgeGrade::FastGreat,
            JudgeGrade::Miss => JudgeGrade::LateGood,
            JudgeGrade::TooFast => JudgeGrade::FastGood,
        },
        3 => match head_grade {
            JudgeGrade::Perfect => {
                if is_late {
                    JudgeGrade::LateGood
                } else {
                    JudgeGrade::FastGood
                }
            }
            JudgeGrade::Miss
            | JudgeGrade::LateGood
            | JudgeGrade::LateGreat3rd
            | JudgeGrade::LateGreat2nd
            | JudgeGrade::LateGreat
            | JudgeGrade::LatePerfect3rd
            | JudgeGrade::LatePerfect2nd => JudgeGrade::LateGood,
            JudgeGrade::FastPerfect2nd
            | JudgeGrade::FastPerfect3rd
            | JudgeGrade::FastGreat
            | JudgeGrade::FastGreat2nd
            | JudgeGrade::FastGreat3rd
            | JudgeGrade::FastGood
            | JudgeGrade::TooFast => JudgeGrade::FastGood,
        },
        _ => match head_grade {
            JudgeGrade::Perfect => {
                if is_late {
                    JudgeGrade::LateGood
                } else {
                    JudgeGrade::FastGood
                }
            }
            JudgeGrade::LateGood
            | JudgeGrade::LateGreat3rd
            | JudgeGrade::LateGreat2nd
            | JudgeGrade::LateGreat
            | JudgeGrade::LatePerfect3rd
            | JudgeGrade::LatePerfect2nd => JudgeGrade::LateGood,
            JudgeGrade::FastPerfect2nd
            | JudgeGrade::FastPerfect3rd
            | JudgeGrade::FastGreat
            | JudgeGrade::FastGreat2nd
            | JudgeGrade::FastGreat3rd
            | JudgeGrade::FastGood => JudgeGrade::FastGood,
            JudgeGrade::Miss | JudgeGrade::TooFast => head_grade,
        },
    }
}

// ============================================================================
// Hold Classic End Judgment
// From NoteLongDrop.HoldClassicEndJudge(), lines 257-307
// ============================================================================

/// Classic hold end judge: evaluates release timing independently,
/// then takes the WORSE of head grade vs end grade.
pub fn judge_hold_classic_end(
    head_grade: JudgeGrade,
    timing: i64,
    length: Duration,
    release_timing: i64,
) -> JudgeGrade {
    if head_grade.is_miss_or_too_fast() {
        return head_grade;
    }

    let diff = Duration {
        micros: timing + length.micros - release_timing,
    };
    let is_fast = diff.micros > 0;
    let abs_diff = diff.abs();

    let end_grade = if is_fast {
        if abs_diff.micros < HOLD_CLASSIC_END_JUDGE_PERFECT_FAST.micros {
            JudgeGrade::Perfect
        } else {
            JudgeGrade::FastGood
        }
    } else {
        if abs_diff.micros < HOLD_CLASSIC_END_JUDGE_PERFECT_LATE.micros {
            JudgeGrade::Perfect
        } else {
            JudgeGrade::LateGood
        }
    };

    let head_dist = head_grade.dist_from_perfect();
    let end_dist = end_grade.dist_from_perfect();
    if end_dist > head_dist {
        end_grade
    } else {
        head_grade
    }
}

/// Check if a slide is too late
pub fn is_too_late_slide(diff: Duration, user_offset: Duration) -> bool {
    let min_offset = if user_offset.micros < 0 {
        user_offset
    } else {
        Duration::zero()
    };
    let threshold = Duration {
        micros: SLIDE_GOOD.micros + min_offset.micros,
    };
    diff.micros > threshold.micros
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_judge_tap_perfect() {
        assert_eq!(
            judge_tap(Duration::from_micros(0), false),
            JudgeGrade::Perfect
        );
        assert_eq!(
            judge_tap(Duration::from_micros(10000), false),
            JudgeGrade::Perfect
        );
    }

    #[test]
    fn test_judge_tap_ex() {
        assert_eq!(
            judge_tap(Duration::from_micros(100000), true),
            JudgeGrade::Perfect
        );
    }

    #[test]
    fn test_judge_slide_too_late() {
        assert_eq!(judge_slide_too_late(1), JudgeGrade::LateGood);
        assert_eq!(judge_slide_too_late(2), JudgeGrade::Miss);
    }

    #[test]
    fn test_correct_slide_grade() {
        assert_eq!(
            correct_slide_grade(JudgeGrade::LatePerfect2nd),
            JudgeGrade::Perfect
        );
        assert_eq!(
            correct_slide_grade(JudgeGrade::LateGreat),
            JudgeGrade::LateGreat
        );
    }
}
