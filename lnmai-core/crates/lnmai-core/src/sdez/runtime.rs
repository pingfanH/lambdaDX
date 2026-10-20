//! Minimal SDEZ170 runtime over ChartSpec for Tap, Hold, and Touch.

use crate::areas::{OuterSlot, SensorArea};
use crate::chart_loader::{ChartSpec, HoldChartNote};

use super::input::InputFrame;
use super::note::{
    AutoplayMode, HoldInput, HoldNote, JudgedNote, TapNote, TouchArea, TouchNote,
};
use super::timing::TimingSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdezNoteKind {
    Tap,
    Hold,
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
    pub judged: JudgedNote,
}

struct TapSlot {
    note_index: u64,
    note: TapNote,
}

struct HoldSlot {
    note_index: u64,
    note: HoldNote,
}

struct TouchSlot {
    note_index: u64,
    note: TouchNote,
}

pub struct SdezRuntime {
    pub settings: TimingSettings,
    pub autoplay: AutoplayMode,
    pub input: InputFrame,
    taps: Vec<TapSlot>,
    holds: Vec<HoldSlot>,
    touches: Vec<TouchSlot>,
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
                note: TouchNote::new(
                    settings.chart_time_to_appear(time_point_msec(touch.timing.clone())),
                    touch_area(touch.sensor_pos),
                    touch.is_break,
                    false,
                ),
            })
            .collect();
        Self {
            settings,
            autoplay: AutoplayMode::None,
            input: InputFrame::default(),
            taps,
            holds,
            touches,
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
                    judged,
                });
            }
            if let Some(judged) = out.tail {
                events.push(SdezEvent {
                    note_index: slot.note_index,
                    kind: SdezNoteKind::Hold,
                    phase: SdezPhase::Tail,
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
                    judged,
                });
            }
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
}
