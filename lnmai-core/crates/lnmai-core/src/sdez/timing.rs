//! SDEZ170 timing offsets and note-check windows.

use super::judge::{
    adjust_msec, get_judge_timing, judge_timing_frame, threshold, EJudgeType, ETiming,
    FRAME_MSEC,
};

pub const NORMAL_TIMING: i32 = 20;
pub const AUTO_PLAY_LEAD_MSEC: f32 = 4.1666665;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingSettings {
    /// OptionJudgetimingID, where 20 is Normal.
    pub adjust_timing: i32,
    /// OptionJudgetimingID, where 20 is Normal.
    pub judge_timing: i32,
}

impl Default for TimingSettings {
    fn default() -> Self {
        Self {
            adjust_timing: NORMAL_TIMING,
            judge_timing: NORMAL_TIMING,
        }
    }
}

impl TimingSettings {
    #[inline]
    pub fn chart_offset_msec(self) -> f32 {
        adjust_msec(self.adjust_timing)
    }

    #[inline]
    pub fn judge_frame(self) -> f32 {
        judge_timing_frame(self.judge_timing)
    }

    #[inline]
    pub fn judge_offset_msec(self) -> f32 {
        self.judge_frame() * FRAME_MSEC
    }

    /// Apply the chart offset once when runtime note times are built.
    #[inline]
    pub fn chart_time_to_appear(self, chart_time_msec: f32) -> f32 {
        chart_time_msec + self.chart_offset_msec()
    }
}

/// NoteBase.IsNoteCheckTimeStart.
#[inline]
pub fn is_note_check_time_start(
    current_msec: f32,
    appear_msec: f32,
    settings: TimingSettings,
    ty: EJudgeType,
) -> bool {
    current_msec - settings.judge_offset_msec() >= appear_msec + threshold(ty, 0)
}

/// Judge a note whose diff is current - appear.
pub fn judge_tap_like(
    current_msec: f32,
    appear_msec: f32,
    settings: TimingSettings,
    ty: EJudgeType,
) -> (ETiming, f32) {
    let mut diff = current_msec - appear_msec;
    let timing = get_judge_timing(&mut diff, settings.judge_frame(), ty);
    (timing, diff)
}

/// Judge a slide whose diff is current - tail + lastWaitTime.
pub fn judge_slide(
    current_msec: f32,
    tail_msec: f32,
    last_wait_time: f32,
    settings: TimingSettings,
) -> (ETiming, f32) {
    let mut diff = current_msec - tail_msec + last_wait_time;
    let timing = super::judge::get_slide_judge_timing(
        &mut diff,
        settings.judge_frame(),
        EJudgeType::SlideOut,
        last_wait_time,
    );
    (timing, diff)
}

/// NoteBase.JudgeToolate.
#[inline]
pub fn is_too_late(
    current_msec: f32,
    appear_msec: f32,
    settings: TimingSettings,
    ty: EJudgeType,
) -> bool {
    let (timing, _) = judge_tap_like(current_msec, appear_msec, settings, ty);
    timing == ETiming::TooLate
}

/// The point where NoteBase.SetAutoPlayJudge records a result.
#[inline]
pub fn auto_play_trigger_msec(appear_msec: f32) -> f32 {
    appear_msec - AUTO_PLAY_LEAD_MSEC
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_shift_notes_by_sixty_ms() {
        let settings = TimingSettings::default();
        assert!((settings.chart_offset_msec() - 60.0).abs() < 0.001);
        assert_eq!(settings.judge_frame(), 0.0);
        assert!((settings.chart_time_to_appear(0.0) - 60.0).abs() < 0.001);
    }

    #[test]
    fn check_window_uses_table_zero() {
        let settings = TimingSettings::default();
        let appear = 1000.0;
        let start = appear + threshold(EJudgeType::Tap, 0);
        assert!(!is_note_check_time_start(start - 0.001, appear, settings, EJudgeType::Tap));
        assert!(is_note_check_time_start(start, appear, settings, EJudgeType::Tap));
    }

    #[test]
    fn critical_center_is_fifty_ms_after_appear() {
        let settings = TimingSettings::default();
        let appear = settings.chart_time_to_appear(0.0);
        let (timing, diff) = judge_tap_like(appear + 50.0, appear, settings, EJudgeType::Tap);
        assert_eq!(timing, ETiming::Critical);
        assert!(diff.abs() < 0.001);
    }

    #[test]
    fn judge_offset_shifts_the_window() {
        let mut settings = TimingSettings::default();
        settings.judge_timing = 21;
        let (timing, _) = judge_tap_like(44.0, 10.0, settings, EJudgeType::Tap);
        assert_eq!(timing, ETiming::FastPerfect);
    }
}
