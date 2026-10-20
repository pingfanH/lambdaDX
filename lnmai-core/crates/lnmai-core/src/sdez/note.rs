//! SDEZ170 note lifecycles for Tap, Hold, and Touch.

use super::input::{
    InputFrame, A1, B1, C1, C2, D1, E1,
};
use super::judge::{judge_hold_total, EJudgeType, ETiming};
use super::timing::{
    auto_play_trigger_msec, is_note_check_time_start, is_too_late, judge_tap_like,
    TimingSettings,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JudgedNote {
    pub timing: ETiming,
    pub diff_msec: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplayMode {
    None,
    Critical,
    Perfect,
    Great,
    Good,
    Miss,
}

impl AutoplayMode {
    pub fn timing(self) -> Option<ETiming> {
        match self {
            AutoplayMode::None => None,
            AutoplayMode::Critical => Some(ETiming::Critical),
            AutoplayMode::Perfect => Some(ETiming::LatePerfect2nd),
            AutoplayMode::Great => Some(ETiming::LateGreat),
            AutoplayMode::Good => Some(ETiming::LateGood),
            AutoplayMode::Miss => Some(ETiming::TooLate),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchArea {
    A(usize),
    B(usize),
    C,
    D(usize),
    E(usize),
}

impl TouchArea {
    fn len(self) -> usize {
        if matches!(self, TouchArea::C) {
            2
        } else {
            1
        }
    }

    fn area(self, index: usize) -> usize {
        match self {
            TouchArea::A(index) => A1 + index,
            TouchArea::B(index) => B1 + index,
            TouchArea::C => {
                if index == 0 {
                    C1
                } else {
                    C2
                }
            }
            TouchArea::D(index) => D1 + index,
            TouchArea::E(index) => E1 + index,
        }
    }

    pub fn trigger(self, input: &InputFrame) -> bool {
        (0..self.len()).any(|index| input.panel_trigger(self.area(index)))
    }

    pub fn state(self, input: &InputFrame) -> bool {
        (0..self.len()).any(|index| input.panel_state(self.area(index)))
    }

    pub fn is_used(self, input: &InputFrame) -> bool {
        (0..self.len()).any(|index| input.panel_used(self.area(index)))
    }

    pub fn set_used(self, input: &mut InputFrame) {
        for index in 0..self.len() {
            input.set_panel_used(self.area(index));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldInput {
    Button(usize),
    Touch(TouchArea),
}

impl HoldInput {
    fn down(self, input: &InputFrame) -> bool {
        match self {
            HoldInput::Button(button) => {
                input.button_down(button) || input.panel_trigger(A1 + button)
            }
            HoldInput::Touch(area) => area.trigger(input),
        }
    }

    fn push(self, input: &InputFrame) -> bool {
        match self {
            HoldInput::Button(button) => {
                input.button_push(button) || input.panel_state(A1 + button)
            }
            HoldInput::Touch(area) => area.state(input),
        }
    }

    fn is_used(self, input: &InputFrame) -> bool {
        match self {
            HoldInput::Button(button) => input.panel_used(A1 + button),
            HoldInput::Touch(area) => area.is_used(input),
        }
    }

    fn set_used(self, input: &mut InputFrame) {
        match self {
            HoldInput::Button(button) => input.set_panel_used(A1 + button),
            HoldInput::Touch(area) => area.set_used(input),
        }
    }
}

fn head_type(is_break: bool, is_ex: bool) -> EJudgeType {
    if is_ex {
        EJudgeType::ExTap
    } else if is_break {
        EJudgeType::Break
    } else {
        EJudgeType::Tap
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TapNote {
    pub appear_msec: f32,
    pub button: usize,
    pub is_break: bool,
    pub is_ex: bool,
    pub result: Option<JudgedNote>,
}

impl TapNote {
    pub fn new(appear_msec: f32, button: usize, is_break: bool, is_ex: bool) -> Self {
        Self {
            appear_msec,
            button,
            is_break,
            is_ex,
            result: None,
        }
    }

    pub fn step(
        &mut self,
        current_msec: f32,
        input: &mut InputFrame,
        settings: TimingSettings,
        autoplay: AutoplayMode,
    ) -> Option<JudgedNote> {
        if self.result.is_some() {
            return None;
        }
        let ty = head_type(self.is_break, self.is_ex);
        if current_msec > auto_play_trigger_msec(self.appear_msec) {
            if let Some(timing) = autoplay.timing() {
                let judged = JudgedNote {
                    timing,
                    diff_msec: 0.0,
                };
                self.result = Some(judged);
                return Some(judged);
            }
        }
        let down = input.button_down(self.button) || input.panel_trigger(A1 + self.button);
        if autoplay == AutoplayMode::None
            && down
            && !input.panel_used(A1 + self.button)
            && is_note_check_time_start(current_msec, self.appear_msec, settings, ty)
        {
            let (timing, diff_msec) =
                judge_tap_like(current_msec, self.appear_msec, settings, ty);
            let judged = JudgedNote { timing, diff_msec };
            self.result = Some(judged);
            if timing != ETiming::TooLate {
                input.set_panel_used(A1 + self.button);
            }
            return Some(judged);
        }
        if is_too_late(current_msec, self.appear_msec, settings, ty) {
            let (timing, diff_msec) =
                judge_tap_like(current_msec, self.appear_msec, settings, ty);
            let judged = JudgedNote { timing, diff_msec };
            self.result = Some(judged);
            return Some(judged);
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TouchNote {
    pub appear_msec: f32,
    pub area: TouchArea,
    pub is_break: bool,
    pub is_ex: bool,
    pub result: Option<JudgedNote>,
}

impl TouchNote {
    pub fn new(appear_msec: f32, area: TouchArea, is_break: bool, is_ex: bool) -> Self {
        Self {
            appear_msec,
            area,
            is_break,
            is_ex,
            result: None,
        }
    }

    pub fn step(
        &mut self,
        current_msec: f32,
        input: &mut InputFrame,
        settings: TimingSettings,
        autoplay: AutoplayMode,
    ) -> Option<JudgedNote> {
        if self.result.is_some() {
            return None;
        }
        let ty = if self.is_ex {
            EJudgeType::ExTap
        } else {
            EJudgeType::Touch
        };
        if current_msec > auto_play_trigger_msec(self.appear_msec) {
            if let Some(timing) = autoplay.timing() {
                let judged = JudgedNote {
                    timing,
                    diff_msec: 0.0,
                };
                self.result = Some(judged);
                return Some(judged);
            }
        }
        if autoplay == AutoplayMode::None
            && self.area.trigger(input)
            && !self.area.is_used(input)
            && is_note_check_time_start(current_msec, self.appear_msec, settings, ty)
        {
            let (timing, diff_msec) =
                judge_tap_like(current_msec, self.appear_msec, settings, ty);
            let judged = JudgedNote { timing, diff_msec };
            self.result = Some(judged);
            self.area.set_used(input);
            return Some(judged);
        }
        if is_too_late(current_msec, self.appear_msec, settings, ty) {
            let (timing, diff_msec) =
                judge_tap_like(current_msec, self.appear_msec, settings, ty);
            let judged = JudgedNote { timing, diff_msec };
            self.result = Some(judged);
            return Some(judged);
        }
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HoldStep {
    pub head: Option<JudgedNote>,
    pub tail: Option<JudgedNote>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoldNote {
    pub appear_msec: f32,
    pub tail_msec: f32,
    pub input: HoldInput,
    pub is_break: bool,
    pub is_ex: bool,
    pub is_touch_hold: bool,
    head_result: Option<JudgedNote>,
    head_judged: bool,
    body_on: bool,
    release_time: f64,
    correct_time: f64,
    push_list: [bool; 4],
    last_hold_state: bool,
    trigger_on: bool,
    result: Option<JudgedNote>,
}

impl HoldNote {
    pub fn new(
        appear_msec: f32,
        tail_msec: f32,
        input: HoldInput,
        is_break: bool,
        is_ex: bool,
        is_touch_hold: bool,
    ) -> Self {
        Self {
            appear_msec,
            tail_msec,
            input,
            is_break,
            is_ex,
            is_touch_hold,
            head_result: None,
            head_judged: false,
            body_on: false,
            release_time: 0.0,
            correct_time: 0.0,
            push_list: [false; 4],
            last_hold_state: true,
            trigger_on: false,
            result: None,
        }
    }

    pub fn head_result(&self) -> Option<JudgedNote> {
        self.head_result
    }

    pub fn result(&self) -> Option<JudgedNote> {
        self.result
    }

    pub fn step(
        &mut self,
        current_msec: f32,
        delta_msec: f32,
        input: &mut InputFrame,
        settings: TimingSettings,
        autoplay: AutoplayMode,
    ) -> HoldStep {
        let mut out = HoldStep::default();
        if self.result.is_some() {
            return out;
        }
        let head_ty = if self.is_touch_hold {
            EJudgeType::Touch
        } else {
            head_type(self.is_break, self.is_ex)
        };
        let head_frame = if self.is_touch_hold { 250.0 } else { 100.0 };
        let tail_frame = 200.0;

        if is_note_check_time_start(current_msec, self.appear_msec, settings, head_ty) {
            if current_msec > auto_play_trigger_msec(self.appear_msec) {
                if autoplay != AutoplayMode::None && self.head_result.is_none() {
                    let judged = JudgedNote {
                        timing: autoplay.timing().unwrap_or(ETiming::TooFast),
                        diff_msec: 0.0,
                    };
                    self.head_result = Some(judged);
                    out.head = Some(judged);
                }
            }
            if autoplay == AutoplayMode::None
                && self.input.down(input)
                && !self.input.is_used(input)
                && self.head_result.is_none()
            {
                let (timing, diff_msec) =
                    judge_tap_like(current_msec, self.appear_msec, settings, head_ty);
                let judged = JudgedNote { timing, diff_msec };
                self.head_result = Some(judged);
                self.head_judged = true;
                self.body_on = true;
                self.input.set_used(input);
                out.head = Some(judged);
            }
            if is_too_late(current_msec, self.appear_msec, settings, head_ty)
                && !self.head_judged
            {
                let (timing, diff_msec) =
                    judge_tap_like(current_msec, self.appear_msec, settings, head_ty);
                self.head_result = Some(JudgedNote { timing, diff_msec });
                self.head_judged = true;
            }
            if autoplay != AutoplayMode::None
                && self.head_result.is_some()
                && !self.head_judged
            {
                self.head_judged = true;
                self.trigger_on = true;
                self.body_on = true;
            }
        }

        self.push_list[3] = self.push_list[2];
        self.push_list[2] = self.push_list[1];
        self.push_list[1] = self.push_list[0];
        let mut pressed = if autoplay == AutoplayMode::None {
            self.input.push(input)
        } else {
            self.head_result.map(|head| head.timing) != Some(ETiming::TooLate)
        };
        if autoplay != AutoplayMode::None
            && self.head_result.map(|head| head.timing) == Some(ETiming::TooLate)
        {
            pressed = false;
        }
        self.push_list[0] = pressed;

        if current_msec >= self.appear_msec + head_frame
            && current_msec <= self.tail_msec - tail_frame
        {
            let mut holding = false;
            if self.head_judged {
                if pressed {
                    holding = true;
                    self.correct_time = 0.0;
                } else if self.push_list[1] || self.push_list[2] {
                    self.correct_time += delta_msec as f64;
                    holding = true;
                }
            }
            if !holding {
                self.release_time += delta_msec as f64;
                if self.push_list[3] {
                    self.release_time += self.correct_time;
                }
                self.correct_time = 0.0;
            }
            self.last_hold_state = holding;
            if holding {
                self.body_on = true;
            }
        }

        if current_msec >= self.tail_msec && self.head_judged {
            let head = self.head_result.unwrap_or(JudgedNote {
                timing: ETiming::TooLate,
                diff_msec: 0.0,
            });
            let timing = judge_hold_total(
                self.tail_msec - self.appear_msec,
                self.release_time as f32,
                head.timing,
                self.body_on,
                self.is_touch_hold,
            );
            let judged = JudgedNote {
                timing,
                diff_msec: head.diff_msec,
            };
            self.result = Some(judged);
            out.tail = Some(judged);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdez::input::InputFrame;

    fn settings() -> TimingSettings {
        TimingSettings::default()
    }

    #[test]
    fn tap_hits_critical_and_owns_the_area() {
        let mut input = InputFrame::default();
        input.update_panel(1 << 0, true);
        input.set_buttons(1, 1);
        let mut note = TapNote::new(0.0, 0, false, false);
        let judged = note
            .step(50.0, &mut input, settings(), AutoplayMode::None)
            .expect("tap");
        assert_eq!(judged.timing, ETiming::Critical);
        assert!(input.panel_used(0));
    }

    #[test]
    fn tap_times_out() {
        let mut input = InputFrame::default();
        let mut note = TapNote::new(0.0, 0, false, false);
        let judged = note
            .step(201.0, &mut input, settings(), AutoplayMode::None)
            .expect("miss");
        assert_eq!(judged.timing, ETiming::TooLate);
    }

    #[test]
    fn touch_center_owns_both_c_areas() {
        let mut input = InputFrame::default();
        input.update_panel(1 << C1, true);
        let mut note = TouchNote::new(0.0, TouchArea::C, false, false);
        let judged = note
            .step(0.0, &mut input, settings(), AutoplayMode::None)
            .expect("touch");
        assert_eq!(judged.timing, ETiming::Critical);
        assert!(input.panel_used(C1));
        assert!(input.panel_used(C2));
    }

    #[test]
    fn hold_perfect_hold_keeps_the_head_grade() {
        let mut input = InputFrame::default();
        input.set_buttons(1, 1);
        let mut note = HoldNote::new(0.0, 1000.0, HoldInput::Button(0), false, false, false);
        let mut tail = None;
        let mut t = 50.0;
        while t <= 1020.0 {
            let step = note.step(t, 16.666666, &mut input, settings(), AutoplayMode::None);
            if step.tail.is_some() {
                tail = step.tail;
            }
            t += 16.666666;
        }
        let tail = tail.expect("hold tail");
        assert_eq!(note.head_result().unwrap().timing, ETiming::Critical);
        assert_eq!(tail.timing, ETiming::Critical);
    }

    #[test]
    fn hold_early_release_degrades_the_grade() {
        let mut input = InputFrame::default();
        input.set_buttons(1, 1);
        let mut note = HoldNote::new(0.0, 1000.0, HoldInput::Button(0), false, false, false);
        let mut tail = None;
        let mut t = 50.0;
        while t <= 1020.0 {
            if t > 500.0 {
                input.set_buttons(0, 0);
            }
            let step = note.step(t, 16.666666, &mut input, settings(), AutoplayMode::None);
            if step.tail.is_some() {
                tail = step.tail;
            }
            t += 16.666666;
        }
        let tail = tail.expect("hold tail");
        assert_ne!(tail.timing, ETiming::Critical);
    }
}
