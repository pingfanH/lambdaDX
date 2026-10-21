//! SDEZ170 SlideFan (WiFi) lifecycle.

use super::input::InputFrame;
use super::judge::{
    get_slide_judge_timing, threshold, EJudgeType, ETiming, JUDGE_ADJUST_MSEC,
};
use super::note::{AutoplayMode, JudgedNote};
use super::slide_data::{hit_areas, SlideHitArea};
use super::timing::TimingSettings;

#[derive(Debug, Clone, PartialEq)]
struct Lane {
    areas: &'static [SlideHitArea],
    index: usize,
    hit_in: bool,
}

impl Lane {
    fn new(areas: &'static [SlideHitArea]) -> Self {
        Self {
            areas,
            index: 0,
            hit_in: false,
        }
    }

    fn complete(&self) -> bool {
        self.index >= self.areas.len()
    }

    fn step(&mut self, input: &InputFrame) {
        if self.complete() {
            return;
        }
        let area = &self.areas[self.index];
        if self.hit_in {
            if area.points.iter().all(|point| !input.panel_state(*point)) {
                self.index += 1;
                self.hit_in = false;
            }
        } else if area.points.iter().any(|point| input.panel_trigger(*point)) {
            self.hit_in = true;
            if self.index + 1 == self.areas.len() {
                self.index = self.areas.len();
                self.hit_in = false;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideFanNote {
    pub start: usize,
    pub target: usize,
    pub star_button: usize,
    pub appear_msec: f32,
    pub tail_msec: f32,
    pub star_launch_msec: f32,
    pub star_arrive_msec: f32,
    pub is_break: bool,
    lanes: [Lane; 3],
    pub last_wait_time: f32,
    pub last_wait_time_for_judge: f32,
    pending: Option<JudgedNote>,
    result: Option<JudgedNote>,
}

impl SlideFanNote {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start: usize,
        target: usize,
        star_button: usize,
        appear_msec: f32,
        tail_msec: f32,
        star_launch_msec: f32,
        star_arrive_msec: f32,
        is_break: bool,
    ) -> Self {
        let goals = [
            if target == 0 { 7 } else { target - 1 },
            target,
            if target == 7 { 0 } else { target + 1 },
        ];
        let lanes = goals.map(|goal| Lane::new(hit_areas(13, start, goal, star_button)));
        let middle = lanes[1].areas;
        let total: f64 = middle
            .iter()
            .map(|area| area.push_distance + area.release_distance)
            .sum();
        let initial_wait = middle.last().map_or(0.0, |area| {
            if total == 0.0 {
                0.0
            } else {
                (star_arrive_msec - star_launch_msec) * (area.push_distance / total) as f32
            }
        });
        Self {
            start,
            target,
            star_button,
            appear_msec,
            tail_msec,
            star_launch_msec,
            star_arrive_msec,
            is_break,
            lanes,
            last_wait_time: initial_wait,
            last_wait_time_for_judge: initial_wait,
            pending: None,
            result: None,
        }
    }

    pub fn result(&self) -> Option<JudgedNote> {
        self.result
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
        if current_msec + JUDGE_ADJUST_MSEC < self.appear_msec {
            return None;
        }
        if autoplay != AutoplayMode::None {
            if current_msec >= self.tail_msec {
                let judged = JudgedNote {
                    timing: autoplay.timing().unwrap_or(ETiming::TooLate),
                    diff_msec: 0.0,
                };
                self.result = Some(judged);
                return Some(judged);
            }
            return None;
        }

        if current_msec + JUDGE_ADJUST_MSEC >= self.appear_msec {
            for lane in &mut self.lanes {
                lane.step(input);
            }
            if self.lanes.iter().all(Lane::complete) && self.checkable(current_msec, settings) {
                self.judge_now(current_msec, settings);
            }
        }

        if self.pending.is_none() && self.checkable(current_msec, settings) {
            self.judge_too_late(current_msec, settings);
        }
        if let Some(pending) = self.pending {
            if pending.timing == ETiming::TooLate && self.lanes.iter().all(Lane::complete) {
                self.pending = Some(JudgedNote {
                    timing: ETiming::LateGood,
                    diff_msec: pending.diff_msec,
                });
            }
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

    #[test]
    fn fan_autoplay_completes() {
        let mut note = SlideFanNote::new(0, 4, 0, 0.0, 1000.0, 0.0, 1000.0, false);
        let mut input = InputFrame::default();
        assert!(note
            .step(999.0, 16.666666, &mut input, TimingSettings::default(), AutoplayMode::Critical)
            .is_none());
        let judged = note
            .step(1000.0, 16.666666, &mut input, TimingSettings::default(), AutoplayMode::Critical)
            .expect("fan result");
        assert_eq!(judged.timing, ETiming::Critical);
    }

    #[test]
    fn inactive_fan_does_not_age_out_before_its_head() {
        let mut note = SlideFanNote::new(0, 4, 0, 5000.0, 6000.0, 5000.0, 6000.0, false);
        note.last_wait_time = 1.0;
        let mut input = InputFrame::default();
        assert!(note
            .step(0.0, 16.666666, &mut input, TimingSettings::default(), AutoplayMode::None)
            .is_none());
        assert!(note.result().is_none());
    }
}
