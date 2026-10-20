//! Minimal SDEZ170 runtime over ChartSpec for Tap, Hold, and Touch.

use crate::areas::{OuterSlot, SensorArea};
use crate::chart_loader::{ChartSpec, HoldChartNote};

use super::input::InputFrame;
use super::note::{
    AutoplayMode, HoldInput, HoldNote, JudgedNote, TapNote, TouchArea, TouchNote,
};
use super::score::SdezScore;
use super::slide::SlideNote;
use super::timing::TimingSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdezNoteKind {
    Tap,
    Hold,
    Slide,
    Touch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdezPhase {
    Head,
    Tail,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdezEvent {
    pub note_index: u64,
    pub kind: SdezNoteKind,
    pub phase: SdezPhase,
    pub is_break: bool,
    pub is_ex: bool,
    pub judged: JudgedNote,
}

struct TapSlot {
    note_index: u64,
    is_break: bool,
    is_ex: bool,
    note: TapNote,
}

struct HoldSlot {
    note_index: u64,
    is_break: bool,
    is_ex: bool,
    note: HoldNote,
}

struct TouchSlot {
    note_index: u64,
    is_break: bool,
    is_ex: bool,
    note: TouchNote,
}

struct SlideSlot {
    note_index: u64,
    is_break: bool,
    is_ex: bool,
    note: SlideNote,
}

pub struct SdezRuntime {
    pub settings: TimingSettings,
    pub autoplay: AutoplayMode,
    pub input: InputFrame,
    taps: Vec<TapSlot>,
    holds: Vec<HoldSlot>,
    touches: Vec<TouchSlot>,
    slides: Vec<SlideSlot>,
    pub score: SdezScore,
    current_msec: f32,
}

impl SdezRuntime {
    pub fn from_chart(chart: &ChartSpec) -> Self {
        Self::from_chart_with_settings(chart, TimingSettings::default())
    }

    pub fn from_chart_with_settings(chart: &ChartSpec, settings: TimingSettings) -> Self {
        let taps = chart
            .taps
            .iter()
            .map(|tap| TapSlot {
                note_index: tap.note_index.to_word(),
                is_break: tap.is_break,
                is_ex: tap.is_ex,
                note: TapNote::new(
                    settings.chart_time_to_appear(time_point_msec(tap.timing.clone())),
                    slot_button(tap.slot),
                    tap.is_break,
                    tap.is_ex,
                ),
            })
            .collect();
        let holds = chart
            .holds
            .iter()
            .map(|hold| hold_slot(hold, settings))
            .chain(
                chart
                    .touch_holds
                    .iter()
                    .map(|hold| touch_hold_slot(hold, settings)),
            )
            .collect();
        let touches = chart
            .touches
            .iter()
            .map(|touch| TouchSlot {
                note_index: touch.note_index.to_word(),
                is_break: touch.is_break,
                is_ex: false,
                note: TouchNote::new(
                    settings.chart_time_to_appear(time_point_msec(touch.timing.clone())),
                    touch_area(touch.sensor_pos),
                    touch.is_break,
                    false,
                ),
            })
            .collect();
        let slides = chart
            .slides
            .iter()
            .map(|slide| {
                let start = slot_button(slide.slot);
                let star_launch =
                    settings.chart_time_to_appear(time_point_msec(slide.start_timing.clone()));
                let star_arrive = star_launch + duration_msec(slide.length.clone());
                SlideSlot {
                    note_index: slide.note_index.to_word(),
                    is_break: slide.is_break,
                    is_ex: slide.is_ex,
                    note: SlideNote::new(
                        slide.sdez_slide_type as usize,
                        start,
                        slide.sdez_end as usize,
                        start,
                        settings.chart_time_to_appear(time_point_msec(slide.head_timing.clone())),
                        star_arrive,
                        star_launch,
                        star_arrive,
                        slide.is_break,
                    ),
                }
            })
            .collect();
        Self {
            settings,
            autoplay: AutoplayMode::None,
            input: InputFrame::default(),
            taps,
            holds,
            touches,
            slides,
            score: SdezScore::default(),
            current_msec: 0.0,
        }
    }

    pub fn set_current_time(&mut self, current_msec: f32) {
        self.current_msec = current_msec;
    }

    pub fn step(&mut self, current_msec: f32) -> Vec<SdezEvent> {
        let delta = (current_msec - self.current_msec).max(0.0);
        self.current_msec = current_msec;
        let mut events = Vec::new();

        for slot in &mut self.taps {
            if let Some(judged) = slot.note.step(
                current_msec,
                &mut self.input,
                self.settings,
                self.autoplay,
            ) {
                events.push(SdezEvent {
                    note_index: slot.note_index,
                    kind: SdezNoteKind::Tap,
                    phase: SdezPhase::Head,
                    is_break: slot.is_break,
                    is_ex: slot.is_ex,
                    judged,
                });
            }
        }

        for slot in &mut self.holds {
            let out = slot.note.step(
                current_msec,
                delta,
                &mut self.input,
                self.settings,
                self.autoplay,
            );
            if let Some(judged) = out.head {
                events.push(SdezEvent {
                    note_index: slot.note_index,
                    kind: SdezNoteKind::Hold,
                    phase: SdezPhase::Head,
                    is_break: slot.is_break,
                    is_ex: slot.is_ex,
                    judged,
                });
            }
            if let Some(judged) = out.tail {
                events.push(SdezEvent {
                    note_index: slot.note_index,
                    kind: SdezNoteKind::Hold,
                    phase: SdezPhase::Tail,
                    is_break: slot.is_break,
                    is_ex: slot.is_ex,
                    judged,
                });
            }
        }

        for slot in &mut self.touches {
            if let Some(judged) = slot.note.step(
                current_msec,
                &mut self.input,
                self.settings,
                self.autoplay,
            ) {
                events.push(SdezEvent {
                    note_index: slot.note_index,
                    kind: SdezNoteKind::Touch,
                    phase: SdezPhase::Head,
                    is_break: slot.is_break,
                    is_ex: slot.is_ex,
                    judged,
                });
            }
        }

        for slot in &mut self.slides {
            if let Some(judged) = slot.note.step(
                current_msec,
                delta,
                &mut self.input,
                self.settings,
                self.autoplay,
            ) {
                events.push(SdezEvent {
                    note_index: slot.note_index,
                    kind: SdezNoteKind::Slide,
                    phase: SdezPhase::Head,
                    is_break: slot.is_break,
                    is_ex: slot.is_ex,
                    judged,
                });
            }
        }

        for event in &events {
            self.score.fold(event);
        }
        events
    }
}

fn time_point_msec(time: crate::time::TimePoint) -> f32 {
    time.micros.to_i64().unwrap_or(0) as f32 / 1000.0
}

fn duration_msec(duration: crate::time::Duration) -> f32 {
    duration.micros.to_i64().unwrap_or(0) as f32 / 1000.0
}

fn slot_button(slot: OuterSlot) -> usize {
    slot.to_index()
}

fn hold_slot(hold: &HoldChartNote, settings: TimingSettings) -> HoldSlot {
    let appear = settings.chart_time_to_appear(time_point_msec(hold.timing.clone()));
    let tail = appear + duration_msec(hold.length.clone());
    let (input, is_touch_hold) = if hold.is_touch {
        (HoldInput::Touch(sensor_touch_area(hold.slot)), true)
    } else {
        (HoldInput::Button(slot_button(hold.slot)), false)
    };
    HoldSlot {
        note_index: hold.note_index.to_word(),
        is_break: hold.is_break,
        is_ex: hold.is_ex,
        note: HoldNote::new(
            appear,
            tail,
            input,
            hold.is_break,
            hold.is_ex,
            is_touch_hold,
        ),
    }
}

fn touch_hold_slot(
    hold: &crate::chart_loader::TouchHoldChartNote,
    settings: TimingSettings,
) -> HoldSlot {
    let appear = settings.chart_time_to_appear(time_point_msec(hold.timing.clone()));
    let tail = appear + duration_msec(hold.length.clone());
    HoldSlot {
        note_index: hold.note_index.to_word(),
        is_break: hold.is_break,
        is_ex: hold.is_ex,
        note: HoldNote::new(
            appear,
            tail,
            HoldInput::Touch(touch_area(hold.sensor_pos)),
            hold.is_break,
            hold.is_ex,
            true,
        ),
    }
}

fn sensor_touch_area(slot: OuterSlot) -> TouchArea {
    TouchArea::A(slot.to_index())
}

fn touch_area(area: SensorArea) -> TouchArea {
    match area {
        SensorArea::A1 => TouchArea::A(0),
        SensorArea::A2 => TouchArea::A(1),
        SensorArea::A3 => TouchArea::A(2),
        SensorArea::A4 => TouchArea::A(3),
        SensorArea::A5 => TouchArea::A(4),
        SensorArea::A6 => TouchArea::A(5),
        SensorArea::A7 => TouchArea::A(6),
        SensorArea::A8 => TouchArea::A(7),
        SensorArea::B1 => TouchArea::B(0),
        SensorArea::B2 => TouchArea::B(1),
        SensorArea::B3 => TouchArea::B(2),
        SensorArea::B4 => TouchArea::B(3),
        SensorArea::B5 => TouchArea::B(4),
        SensorArea::B6 => TouchArea::B(5),
        SensorArea::B7 => TouchArea::B(6),
        SensorArea::B8 => TouchArea::B(7),
        SensorArea::C => TouchArea::C,
        SensorArea::D1 => TouchArea::D(0),
        SensorArea::D2 => TouchArea::D(1),
        SensorArea::D3 => TouchArea::D(2),
        SensorArea::D4 => TouchArea::D(3),
        SensorArea::D5 => TouchArea::D(4),
        SensorArea::D6 => TouchArea::D(5),
        SensorArea::D7 => TouchArea::D(6),
        SensorArea::D8 => TouchArea::D(7),
        SensorArea::E1 => TouchArea::E(0),
        SensorArea::E2 => TouchArea::E(1),
        SensorArea::E3 => TouchArea::E(2),
        SensorArea::E4 => TouchArea::E(3),
        SensorArea::E5 => TouchArea::E(4),
        SensorArea::E6 => TouchArea::E(5),
        SensorArea::E7 => TouchArea::E(6),
        SensorArea::E8 => TouchArea::E(7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::input::A1;
    use super::super::judge::ETiming;
    use crate::chart_loader::TapChartNote;
    use crate::time::TimePoint;

    #[test]
    fn tap_chart_spec_flows_through_the_sdez_runtime() {
        let mut chart = ChartSpec::default();
        chart.taps.push(TapChartNote {
            timing: TimePoint::from_micros(50_000),
            slot: OuterSlot::S1,
            note_index: (7).into(),
            ..Default::default()
        });
        let mut runtime = SdezRuntime::from_chart(&chart);
        runtime.input.set_buttons(1, 1);
        // appear = 50 + 60 = 110ms; critical center = 160ms.
        let events = runtime.step(160.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].note_index, 7);
        assert_eq!(events[0].judged.timing, ETiming::Critical);
    }

    #[test]
    fn touch_chart_spec_uses_the_sensor_area() {
        let mut chart = ChartSpec::default();
        chart.touches.push(crate::chart_loader::TouchChartNote {
            timing: TimePoint::from_micros(0),
            sensor_pos: SensorArea::A1,
            note_index: (3).into(),
            ..Default::default()
        });
        let mut runtime = SdezRuntime::from_chart(&chart);
        runtime.input.update_panel(1 << A1, true);
        let events = runtime.step(60.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].judged.timing, ETiming::Critical);
    }

    #[test]
    fn slide_chart_spec_completes_with_autoplay() {
        let mut chart = ChartSpec::default();
        chart.slides.push(crate::chart_loader::SlideChartNote {
            head_timing: TimePoint::from_micros(0),
            slot: OuterSlot::S1,
            start_timing: TimePoint::from_micros(0),
            length: crate::time::Duration::from_micros(1_000_000),
            sdez_slide_type: 1,
            sdez_end: 2,
            note_index: (9).into(),
            ..Default::default()
        });
        let mut runtime = SdezRuntime::from_chart(&chart);
        runtime.autoplay = AutoplayMode::Critical;
        let events = runtime.step(1060.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, SdezNoteKind::Slide);
        assert_eq!(events[0].judged.timing, ETiming::Critical);
    }

    #[test]
    fn mixed_simai_chart_completes_every_note_type() {
        let chart = crate::simai::frontend::frontend_lowered_chart(
            "&inote_1=(120){4}1,2h[1:2],3-5[4:1],B4,E",
            (1).into(),
        )
        .expect("chart");
        let mut runtime = SdezRuntime::from_chart(&chart);
        runtime.autoplay = AutoplayMode::Critical;
        let mut seen = std::collections::HashSet::new();
        let mut t = 0.0_f32;
        while t <= 5000.0 {
            for event in runtime.step(t) {
                seen.insert(event.kind);
                assert_eq!(
                    event.judged.timing,
                    ETiming::Critical,
                    "autoplay must not produce {:?} for {:?}",
                    event.judged.timing,
                    event.kind
                );
            }
            t += 16.666666;
        }
        assert!(seen.contains(&SdezNoteKind::Tap));
        assert!(seen.contains(&SdezNoteKind::Hold));
        assert!(seen.contains(&SdezNoteKind::Slide));
        assert!(seen.contains(&SdezNoteKind::Touch));
    }

    fn run_real_chart(text: &str, level: usize) {
        let chart = crate::simai::frontend::frontend_lowered_chart(text, level.into())
            .expect("real chart parses");
        let expected = chart.taps.len()
            + chart.holds.len()
            + chart.touch_holds.len()
            + chart.touches.len()
            + chart.slides.len();
        assert!(expected > 0, "chart has no runtime notes");
        let mut runtime = SdezRuntime::from_chart(&chart);
        runtime.autoplay = AutoplayMode::Critical;

        let mut end_ms = 0.0_f32;
        for tap in &chart.taps {
            end_ms = end_ms.max(time_point_msec(tap.timing.clone()));
        }
        for hold in &chart.holds {
            end_ms = end_ms.max(
                time_point_msec(hold.timing.clone()) + duration_msec(hold.length.clone()),
            );
        }
        for hold in &chart.touch_holds {
            end_ms = end_ms.max(
                time_point_msec(hold.timing.clone()) + duration_msec(hold.length.clone()),
            );
        }
        for touch in &chart.touches {
            end_ms = end_ms.max(time_point_msec(touch.timing.clone()));
        }
        for slide in &chart.slides {
            end_ms = end_ms.max(
                time_point_msec(slide.start_timing.clone()) + duration_msec(slide.length.clone()),
            );
        }
        end_ms += 5000.0;

        let mut scored = 0usize;
        let mut t = 0.0_f32;
        while t <= end_ms {
            for event in runtime.step(t) {
                if event.kind == SdezNoteKind::Hold && event.phase == SdezPhase::Head {
                    continue;
                }
                assert_eq!(
                    event.judged.timing,
                    ETiming::Critical,
                    "autoplay produced {:?} at {:.3}ms for {:?}",
                    event.judged.timing,
                    t,
                    event.kind
                );
                scored += 1;
            }
            t += 16.666666;
        }
        assert_eq!(scored, expected, "not every runtime note was scored");
    }

    #[test]
    fn real_jack_ripper_completes() {
        run_real_chart(
            include_str!("../../tests/fixtures/real_charts/jack_ripper.txt"),
            5,
        );
    }

    #[test]
    fn real_pandora_paraboxxx_completes() {
        run_real_chart(
            include_str!("../../tests/fixtures/real_charts/pandora_paraboxxx.txt"),
            6,
        );
    }

    #[test]
    fn real_credit_completes() {
        run_real_chart(
            include_str!("../../tests/fixtures/real_charts/credit.txt"),
            5,
        );
    }
}
