//! SDEZ170 slide lifecycle over the generated HitArea tables.

use super::input::InputFrame;
use super::judge::{
    get_slide_judge_timing, threshold, EJudgeType, ETiming, JUDGE_ADJUST_MSEC,
};
use super::note::{AutoplayMode, JudgedNote};
use super::slide_data::{
    hit_areas, initial_last_wait_time, rotate_areas, SlideHitArea,
};
use super::timing::TimingSettings;

#[derive(Debug, Clone, PartialEq)]
pub struct SlideNote {
    pub slide_type: usize,
    pub start: usize,
    pub end: usize,
    pub star_button: usize,
    pub appear_msec: f32,
    pub tail_msec: f32,
    pub star_launch_msec: f32,
    pub star_arrive_msec: f32,
    pub is_break: bool,
    pub hit_areas: Vec<SlideHitArea>,
    pub hit_index: usize,
    pub hit_in: bool,
    pub last_wait_time: f32,
    pub last_wait_time_for_judge: f32,
    pending: Option<JudgedNote>,
    result: Option<JudgedNote>,
}

impl SlideNote {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slide_type: usize,
        start: usize,
        end: usize,
        star_button: usize,
        appear_msec: f32,
        tail_msec: f32,
        star_launch_msec: f32,
        star_arrive_msec: f32,
        is_break: bool,
    ) -> Self {
        let mut hit_areas = hit_areas(slide_type, start, end, star_button).to_vec();
        rotate_areas(&mut hit_areas, star_button);
        let initial_wait =
            initial_last_wait_time(&hit_areas, star_launch_msec, star_arrive_msec);
        Self {
            slide_type,
            start,
            end,
            star_button,
            appear_msec,
            tail_msec,
            star_launch_msec,
            star_arrive_msec,
            is_break,
            hit_areas,
            hit_index: 0,
            hit_in: false,
            last_wait_time: initial_wait,
            last_wait_time_for_judge: initial_wait,
            pending: None,
            result: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_areas(
        slide_type: usize,
        start: usize,
        end: usize,
        star_button: usize,
        appear_msec: f32,
        tail_msec: f32,
        star_launch_msec: f32,
        star_arrive_msec: f32,
        is_break: bool,
        hit_areas: Vec<SlideHitArea>,
    ) -> Self {
        let mut hit_areas = hit_areas;
        rotate_areas(&mut hit_areas, star_button);
        let initial_wait =
            initial_last_wait_time(&hit_areas, star_launch_msec, star_arrive_msec);
        Self {
            slide_type,
            start,
            end,
            star_button,
            appear_msec,
            tail_msec,
            star_launch_msec,
            star_arrive_msec,
            is_break,
            hit_areas,
            hit_index: 0,
            hit_in: false,
            last_wait_time: initial_wait,
            last_wait_time_for_judge: initial_wait,
            pending: None,
            result: None,
        }
    }

    pub fn result(&self) -> Option<JudgedNote> {
        self.result
    }

    /// Visual consumption of the slide trail (0..1).
    ///
    /// Autoplay uses the official SlideRoot.NoteCheck cutoff
    /// (current - StarLaunch) / (StarArrive - StarLaunch - lastWaitTime).
    /// Manual play consumes only areas the player actually hit, matching the
    /// arrow-deletion path in CheckSlideTouch.
    pub fn progress(&self, current_msec: f32, autoplay: bool) -> f32 {
        let by_hit = self.hit_fraction();
        if !autoplay {
            return by_hit;
        }
        let span = self.star_arrive_msec - self.star_launch_msec;
        let denom = span - self.last_wait_time;
        let by_time = if current_msec <= self.star_launch_msec {
            0.0
        } else if denom <= 0.0 {
            1.0
        } else {
            ((current_msec - self.star_launch_msec) / denom).clamp(0.0, 1.0)
        };
        by_time.max(by_hit)
    }

    fn hit_fraction(&self) -> f32 {
        let mut total = 0.0_f64;
        let mut consumed = 0.0_f64;
        for (index, area) in self.hit_areas.iter().enumerate() {
            total += area.push_distance + area.release_distance;
            if index < self.hit_index {
                consumed += area.push_distance + area.release_distance;
            } else if index == self.hit_index && self.hit_in {
                consumed += area.push_distance;
            }
        }
        if total <= 0.0 {
            0.0
        } else {
            (consumed / total).clamp(0.0, 1.0) as f32
        }
    }

    fn checkable(&self, current_msec: f32, settings: TimingSettings) -> bool {
        current_msec - settings.judge_offset_msec()
            >= self.star_launch_msec + threshold(EJudgeType::SlideOut, 0)
    }

    fn judge_now(&mut self, current_msec: f32, settings: TimingSettings) {
        let mut diff = current_msec - self.tail_msec + self.last_wait_time;
        let mut timing = get_slide_judge_timing(
            &mut diff,
            settings.judge_frame(),
            EJudgeType::SlideOut,
            self.last_wait_time_for_judge,
        );
        if timing == ETiming::TooFast {
            timing = ETiming::FastGood;
        }
        self.pending = Some(JudgedNote {
            timing,
            diff_msec: diff,
        });
    }

    fn judge_too_late(&mut self, current_msec: f32, settings: TimingSettings) {
        let mut diff = current_msec - self.tail_msec;
        let timing = get_slide_judge_timing(
            &mut diff,
            settings.judge_frame(),
            EJudgeType::SlideOut,
            self.last_wait_time,
        );
        if timing == ETiming::TooLate {
            self.pending = Some(JudgedNote {
                timing,
                diff_msec: diff,
            });
        }
    }

    pub fn step(
        &mut self,
        current_msec: f32,
        delta_msec: f32,
        input: &mut InputFrame,
        settings: TimingSettings,
        autoplay: AutoplayMode,
    ) -> Option<JudgedNote> {
        if self.result.is_some() {
            return None;
        }
        // The note is not active yet. Do not age lastWaitTime while it is still
        // off-screen, otherwise short slides can be finalized as TooLate before
        // their star ever appears.
        if current_msec + JUDGE_ADJUST_MSEC < self.appear_msec {
            return None;
        }

        if autoplay != AutoplayMode::None {
            if current_msec >= self.tail_msec {
                let timing = autoplay.timing().unwrap_or(ETiming::TooLate);
                let judged = JudgedNote {
                    timing,
                    diff_msec: 0.0,
                };
                self.result = Some(judged);
                return Some(judged);
            }
            return None;
        } else if current_msec + JUDGE_ADJUST_MSEC >= self.appear_msec
            && self.hit_index < self.hit_areas.len()
        {
            for _ in 0..8 {
                if self.hit_index >= self.hit_areas.len() {
                    break;
                }
                let area = &self.hit_areas[self.hit_index];
                if self.hit_in {
                    if area.points.iter().all(|point| !input.panel_state(*point)) {
                        self.hit_index += 1;
                        self.hit_in = false;
                    } else {
                        break;
                    }
                } else if area
                    .points
                    .iter()
                    .any(|point| input.panel_trigger(*point))
                {
                    self.hit_in = true;
                    if self.hit_index + 1 == self.hit_areas.len() {
                        self.hit_index = self.hit_areas.len();
                        self.hit_in = false;
                    }
                    break;
                } else {
                    break;
                }
            }
        }

        if self.hit_index >= self.hit_areas.len() && self.checkable(current_msec, settings) {
            self.judge_now(current_msec, settings);
        }

        if self.pending.is_none() && self.checkable(current_msec, settings) {
            self.judge_too_late(current_msec, settings);
        }

        if let Some(pending) = self.pending {
            if pending.timing == ETiming::TooLate
                && self.hit_index + 1 >= self.hit_areas.len()
            {
                self.pending = Some(JudgedNote {
                    timing: ETiming::LateGood,
                    diff_msec: pending.diff_msec,
                });
            }
        }

        // `SlideRoot.NoteCheck` returns early (`GetJudgeResult() == End &&
        // !JudgeToolate()`) while the slide is still too fast to judge, so
        // `lastWaitTime` is not aged during the approach. Aging it from the
        // head made short slides finalize before their star even launched.
        if self.pending.is_none() {
            return None;
        }

        if self.last_wait_time <= 0.0 || self.pending.map(|p| p.timing) == Some(ETiming::TooLate)
        {
            let judged = self.pending.unwrap_or(JudgedNote {
                timing: ETiming::TooLate,
                diff_msec: 0.0,
            });
            self.result = Some(judged);
            return Some(judged);
        }

        self.last_wait_time -= delta_msec;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdez::input::{A1, A2, A3};

    #[test]
    fn straight_slide_finishes_critical_when_the_last_area_is_on_time() {
        let mut note = SlideNote::new(1, 0, 2, 0, 0.0, 1000.0, 0.0, 1000.0, false);
        let mut input = InputFrame::default();
        let settings = TimingSettings::default();

        input.update_panel(1 << A1, true);
        note.step(0.0, 16.666666, &mut input, settings, AutoplayMode::None);
        input.update_panel(0, true);
        note.step(16.666666, 16.666666, &mut input, settings, AutoplayMode::None);

        input.update_panel(1 << A2, true);
        note.step(333.0, 16.666666, &mut input, settings, AutoplayMode::None);
        input.update_panel(0, true);
        note.step(350.0, 16.666666, &mut input, settings, AutoplayMode::None);

        note.last_wait_time = 0.0;
        input.update_panel(1 << A3, true);
        let judged = note
            .step(1000.0, 16.666666, &mut input, settings, AutoplayMode::None)
            .expect("slide result");
        assert_eq!(judged.timing, ETiming::Critical);
    }

    #[test]
    fn autoplay_completes_at_the_tail() {
        let mut note = SlideNote::new(1, 0, 2, 0, 0.0, 1000.0, 0.0, 1000.0, false);
        let mut input = InputFrame::default();
        let settings = TimingSettings::default();
        assert!(note
            .step(999.0, 16.666666, &mut input, settings, AutoplayMode::Critical)
            .is_none());
        let judged = note
            .step(1000.0, 16.666666, &mut input, settings, AutoplayMode::Critical)
            .expect("slide result");
        assert_eq!(judged.timing, ETiming::Critical);
    }

    #[test]
    fn inactive_slide_does_not_age_out_before_its_head() {
        let mut note = SlideNote::new(1, 0, 2, 0, 5000.0, 6000.0, 5000.0, 6000.0, false);
        note.last_wait_time = 1.0;
        let mut input = InputFrame::default();
        assert!(note
            .step(0.0, 16.666666, &mut input, TimingSettings::default(), AutoplayMode::None)
            .is_none());
        assert!(note.result().is_none());
    }

    #[test]
    fn real_chart_line_slide_advances_through_plan_holds() {
        // Slide 1-5 from jack_ripper, judged timestamps (plan time + 110 ms).
        let mut note = SlideNote::new(1, 0, 4, 0, 5774.0, 6202.857, 6060.0, 6202.857, false);
        let mut input = InputFrame::default();
        let settings = TimingSettings::default();
        let script = [
            (6110.0, 1usize << A1),
            (6140.284, 0),
            (6140.285, 1usize << 8),
            (6170.569, 0),
            (6170.570, 1usize << 16),
            (6200.854, 0),
            (6200.855, 1usize << 12),
            (6231.139, 0),
            (6231.140, 1usize << 4),
        ];
        let mut result = None;
        for (t, panel) in script {
            input.update_panel(panel as u64, true);
            if let Some(judged) = note.step(t, 1.0, &mut input, settings, AutoplayMode::None) {
                result = Some(judged);
            }
        }
        let mut t = 6231.14;
        while result.is_none() && t < 6300.0 {
            t += 1.0;
            if let Some(judged) = note.step(t, 1.0, &mut input, settings, AutoplayMode::None) {
                result = Some(judged);
            }
        }
        assert!(result.is_some(), "slide never resolved");
        assert_eq!(result.unwrap().timing, ETiming::Critical);
    }
}
