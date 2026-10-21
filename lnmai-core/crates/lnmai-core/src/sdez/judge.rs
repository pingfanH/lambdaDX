//! Literal Rust port of SDEZ170 NoteJudge/NoteScore.

pub const FRAME_MSEC: f32 = 16.666666;
pub const JUDGE_ADJUST_MSEC: f32 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EJudgeType {
    Tap = 0,
    HoldOut = 1,
    SlideOut = 2,
    Touch = 3,
    ExTap = 4,
    Break = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ETiming {
    TooFast = 0,
    FastGood = 1,
    FastGreat3rd = 2,
    FastGreat2nd = 3,
    FastGreat = 4,
    FastPerfect2nd = 5,
    FastPerfect = 6,
    Critical = 7,
    LatePerfect = 8,
    LatePerfect2nd = 9,
    LateGreat = 10,
    LateGreat2nd = 11,
    LateGreat3rd = 12,
    LateGood = 13,
    TooLate = 14,
    End = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum JudgeBox {
    Miss = 0,
    Good = 1,
    Great = 2,
    Perfect = 3,
    Critical = 4,
    End = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NoteScoreType {
    Tap = 0,
    Hold = 1,
    Slide = 2,
    Break = 3,
    Touch = 4,
    BreakBonus = 5,
    End = 6,
}

pub const JUDGE_COEFFICIENTS: [[f32; 15]; 6] = [
    [-9.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 9.0],
    [-9.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 4.0, 8.0, 9.0, 10.0, 11.0, 14.0],
    [-36.0, -26.0, -22.0, -18.0, -14.0, -14.0, -14.0, 0.0, 14.0, 14.0, 14.0, 16.0, 22.0, 26.0, 36.0],
    [-9.0, -9.0, -9.0, -9.0, -9.0, -9.0, -9.0, 0.0, 9.0, 10.5, 12.0, 13.0, 14.0, 15.0, 18.0],
    [-9.0, -9.0, -9.0, -9.0, -9.0, -9.0, -9.0, 0.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0],
    [-9.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 9.0],
];

const HOLD_PERCENT: [i32; 5] = [0, 33, 67, 95, 100];

const HOLD_PARAM: [[ETiming; 15]; 5] = [
    [
        ETiming::FastGood,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastPerfect,
        ETiming::FastPerfect,
        ETiming::Critical,
        ETiming::LatePerfect,
        ETiming::LatePerfect,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGood,
    ],
    [
        ETiming::FastGood,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastPerfect,
        ETiming::FastPerfect,
        ETiming::LatePerfect,
        ETiming::LatePerfect,
        ETiming::LatePerfect,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGood,
    ],
    [
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::FastGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGreat,
        ETiming::LateGood,
        ETiming::LateGood,
    ],
    [
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
    ],
    [
        ETiming::TooFast,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::FastGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::LateGood,
        ETiming::TooLate,
    ],
];

const JUDGE_SCORE: [[u32; 15]; 6] = [
    [0, 250, 400, 400, 400, 500, 500, 500, 500, 500, 400, 400, 400, 250, 0],
    [0, 500, 800, 800, 800, 1000, 1000, 1000, 1000, 1000, 800, 800, 800, 500, 0],
    [0, 750, 1200, 1200, 1200, 1500, 1500, 1500, 1500, 1500, 1200, 1200, 1200, 750, 0],
    [0, 1000, 1250, 1500, 2000, 2500, 2500, 2500, 2500, 2500, 2000, 1500, 1250, 1000, 0],
    [0, 250, 400, 400, 400, 500, 500, 500, 500, 500, 400, 400, 400, 250, 0],
    [0, 30, 40, 40, 40, 50, 75, 100, 75, 50, 40, 40, 40, 30, 0],
];

#[inline]
pub fn threshold(ty: EJudgeType, index: usize) -> f32 {
    JUDGE_COEFFICIENTS[ty as usize][index] * FRAME_MSEC + JUDGE_ADJUST_MSEC
}

/// SDEZ170 GetAdjustMSec: the chart timeline offset.
#[inline]
pub fn adjust_msec(adjust_timing: i32) -> f32 {
    (adjust_timing as f32 - 20.0 + 36.0) / 10.0 * FRAME_MSEC
}

/// SDEZ170 GetJudgeTimingFrame: the player judge offset in frames.
#[inline]
pub fn judge_timing_frame(judge_timing: i32) -> f32 {
    (judge_timing as f32 - 20.0) / 10.0
}

/// Literal GetJudgeTiming. The diff is the ref parameter: it is shifted by the
/// option offset, and the final 50 ms is subtracted before returning.
pub fn get_judge_timing(
    diff_msec: &mut f32,
    option_judge_timing: f32,
    ty: EJudgeType,
) -> ETiming {
    *diff_msec -= option_judge_timing * FRAME_MSEC;
    if ty == EJudgeType::SlideOut {
        *diff_msec += JUDGE_ADJUST_MSEC;
    }
    let d = *diff_msec;
    let result = if !(d < threshold(ty, 0)) {
        if d < threshold(ty, 1) {
            ETiming::FastGood
        } else if d < threshold(ty, 2) {
            ETiming::FastGreat3rd
        } else if d < threshold(ty, 3) {
            ETiming::FastGreat2nd
        } else if d < threshold(ty, 4) {
            ETiming::FastGreat
        } else if d < threshold(ty, 5) {
            ETiming::FastPerfect2nd
        } else if d < threshold(ty, 6) {
            ETiming::FastPerfect
        } else if d <= threshold(ty, 8) {
            ETiming::Critical
        } else if d <= threshold(ty, 9) {
            ETiming::LatePerfect
        } else if d <= threshold(ty, 10) {
            ETiming::LatePerfect2nd
        } else if d <= threshold(ty, 11) {
            ETiming::LateGreat
        } else if d <= threshold(ty, 12) {
            ETiming::LateGreat2nd
        } else if d <= threshold(ty, 13) {
            ETiming::LateGreat3rd
        } else if !(d <= threshold(ty, 14)) {
            ETiming::TooLate
        } else {
            ETiming::LateGood
        }
    } else {
        ETiming::TooFast
    };
    *diff_msec -= JUDGE_ADJUST_MSEC;
    result
}

/// Literal GetSlideJudgeTiming. An extended Critical returns without applying
/// the 50 ms ref subtraction, matching the C# branch.
pub fn get_slide_judge_timing(
    diff_msec: &mut f32,
    option_judge_timing: f32,
    ty: EJudgeType,
    last_wait_time: f32,
) -> ETiming {
    if ty != EJudgeType::SlideOut {
        return get_judge_timing(diff_msec, option_judge_timing, EJudgeType::SlideOut);
    }
    let num = last_wait_time / 4.0;
    let num2 = threshold(EJudgeType::SlideOut, 6) - num;
    let num3 = threshold(EJudgeType::SlideOut, 8) + num;
    let num4 = threshold(EJudgeType::SlideOut, 0);
    let num5 = threshold(EJudgeType::SlideOut, 14);
    let num6 = *diff_msec - option_judge_timing * FRAME_MSEC;
    if num2 <= num6 && num6 <= num3 && num4 <= num6 && num6 <= num5 {
        ETiming::Critical
    } else {
        get_judge_timing(diff_msec, option_judge_timing, EJudgeType::SlideOut)
    }
}

pub fn convert_judge(timing: ETiming) -> JudgeBox {
    match timing {
        ETiming::TooFast | ETiming::TooLate => JudgeBox::Miss,
        ETiming::FastGood | ETiming::LateGood => JudgeBox::Good,
        ETiming::FastGreat3rd
        | ETiming::FastGreat2nd
        | ETiming::FastGreat
        | ETiming::LateGreat
        | ETiming::LateGreat2nd
        | ETiming::LateGreat3rd => JudgeBox::Great,
        ETiming::FastPerfect2nd
        | ETiming::FastPerfect
        | ETiming::LatePerfect
        | ETiming::LatePerfect2nd => JudgeBox::Perfect,
        ETiming::Critical => JudgeBox::Critical,
        ETiming::End => JudgeBox::End,
    }
}

pub fn judge_hold_total(
    hold_time: f32,
    hold_release_time: f32,
    timing: ETiming,
    body_on: bool,
    is_touch_hold: bool,
) -> ETiming {
    let head_frame = if is_touch_hold { 250.0 } else { 100.0 };
    let tail_frame = 200.0;
    if hold_time <= head_frame + tail_frame {
        return timing;
    }
    let hold_time = hold_time - (head_frame + tail_frame);
    if !body_on {
        return HOLD_PARAM[4][timing as usize];
    }
    let mut percent = (hold_release_time / hold_time * 100.0).ceil() as i32;
    if percent > 100 {
        percent = 100;
    }
    for (index, threshold) in HOLD_PERCENT.iter().enumerate() {
        if *threshold >= percent {
            return HOLD_PARAM[index][timing as usize];
        }
    }
    HOLD_PARAM[4][timing as usize]
}

pub fn get_judge_score(timing: ETiming, note_type: NoteScoreType) -> u32 {
    JUDGE_SCORE[note_type as usize][timing as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_offset_and_judge_frame_match_source() {
        assert!((adjust_msec(20) - 60.0).abs() < 0.001);
        assert!((adjust_msec(21) - 61.666_664).abs() < 0.01);
        assert_eq!(judge_timing_frame(20), 0.0);
        assert!((judge_timing_frame(21) - 0.1).abs() < 0.0001);
    }

    #[test]
    fn tap_critical_uses_the_shifted_table() {
        let mut diff = 60.0;
        assert_eq!(
            get_judge_timing(&mut diff, 0.0, EJudgeType::Tap),
            ETiming::Critical
        );
        assert!((diff - 10.0).abs() < 0.001);

        let mut diff = 0.0;
        assert_eq!(
            get_judge_timing(&mut diff, 0.0, EJudgeType::Tap),
            ETiming::FastPerfect2nd
        );
    }

    #[test]
    fn tap_boundaries_use_strict_early_and_inclusive_late() {
        let early = threshold(EJudgeType::Tap, 6);
        let late = threshold(EJudgeType::Tap, 8);
        let mut just_before = early - 0.001;
        let mut at = early;
        let mut just_after = late + 0.001;
        assert_eq!(
            get_judge_timing(&mut just_before, 0.0, EJudgeType::Tap),
            ETiming::FastPerfect
        );
        assert_eq!(
            get_judge_timing(&mut at, 0.0, EJudgeType::Tap),
            ETiming::Critical
        );
        assert_eq!(
            get_judge_timing(&mut just_after, 0.0, EJudgeType::Tap),
            ETiming::LatePerfect
        );
    }

    #[test]
    fn slide_extension_expands_the_critical_window() {
        let base = threshold(EJudgeType::SlideOut, 6);
        let diff = base - 100.0;
        let mut without_wait = diff;
        assert_ne!(
            get_slide_judge_timing(&mut without_wait, 0.0, EJudgeType::SlideOut, 0.0),
            ETiming::Critical
        );
        let mut with_wait = diff;
        assert_eq!(
            get_slide_judge_timing(&mut with_wait, 0.0, EJudgeType::SlideOut, 400.0),
            ETiming::Critical
        );
        assert_eq!(with_wait, diff, "extended critical keeps the raw ref value");
    }

    #[test]
    fn hold_total_matches_release_bands() {
        assert_eq!(
            judge_hold_total(1000.0, 0.0, ETiming::Critical, true, false),
            ETiming::Critical
        );
        assert_eq!(
            judge_hold_total(1000.0, 500.0, ETiming::Critical, true, false),
            ETiming::LateGood
        );
        assert_eq!(
            judge_hold_total(1000.0, 400.0, ETiming::Critical, true, false),
            ETiming::LateGreat
        );
        assert_eq!(
            judge_hold_total(1000.0, 500.0, ETiming::Critical, false, false),
            ETiming::LateGood
        );
        assert_eq!(
            judge_hold_total(200.0, 200.0, ETiming::Critical, true, false),
            ETiming::Critical
        );
        assert_eq!(
            judge_hold_total(400.0, 400.0, ETiming::Critical, true, true),
            ETiming::Critical
        );
    }

    #[test]
    fn score_table_matches_source() {
        assert_eq!(get_judge_score(ETiming::Critical, NoteScoreType::Tap), 500);
        assert_eq!(
            get_judge_score(ETiming::LateGreat, NoteScoreType::Slide),
            1200
        );
        assert_eq!(
            get_judge_score(ETiming::Critical, NoteScoreType::Break),
            2500
        );
        assert_eq!(
            get_judge_score(ETiming::Critical, NoteScoreType::BreakBonus),
            100
        );
        assert_eq!(get_judge_score(ETiming::TooLate, NoteScoreType::Tap), 0);
    }

    #[test]
    fn convert_judge_groups_the_fifteen_timings() {
        assert_eq!(convert_judge(ETiming::Critical), JudgeBox::Critical);
        assert_eq!(convert_judge(ETiming::FastPerfect2nd), JudgeBox::Perfect);
        assert_eq!(convert_judge(ETiming::LateGreat3rd), JudgeBox::Great);
        assert_eq!(convert_judge(ETiming::FastGood), JudgeBox::Good);
        assert_eq!(convert_judge(ETiming::TooFast), JudgeBox::Miss);
    }
}
