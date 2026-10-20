//! SDEZ170 backend adapter for the player.

use std::collections::BTreeMap;

use crate::core::sdez::{SdezEvent, SdezSession, SdezStepResult};
use crate::core::types::{
    AudioCommand, ButtonZone, JudgeEvent, JudgeEventKind, JudgeGrade, JudgePhase,
    NoteTypeJudgeCounts, RenderCommand, RuntimePos, RuntimeStepLightResult, ScoreState, SensorArea,
    TimedInputEvent,
};

pub struct SdezEngine {
    session: SdezSession,
    panel_state: u64,
    buttons_push: u16,
    last_time_us: i64,
}

impl SdezEngine {
    pub fn load(text: &str, level_index: u32) -> Result<Self, String> {
        let mut session = SdezSession::create().map_err(|e| e.json)?;
        session
            .load_text(text, level_index, 0)
            .map_err(|e| e.json)?;
        Ok(Self {
            session,
            panel_state: 0,
            buttons_push: 0,
            last_time_us: 0,
        })
    }

    pub fn step(
        &mut self,
        current_time_us: i64,
        events: &[TimedInputEvent],
    ) -> Result<RuntimeStepLightResult, String> {
        let (buttons_down, panel_state, new_frame) = self.apply_events(events);
        let result = self
            .session
            .step(
                current_time_us as f32 / 1000.0,
                buttons_down,
                self.buttons_push,
                panel_state,
                new_frame,
            )
            .map_err(|e| e.json)?;
        self.last_time_us = current_time_us;
        Ok(self.to_runtime_result(result, current_time_us))
    }

    fn apply_events(&mut self, events: &[TimedInputEvent]) -> (u16, u64, bool) {
        let mut down = 0u16;
        let mut new_frame = false;
        for event in events {
            match event {
                TimedInputEvent::ButtonClick { zone, .. } => {
                    let bit = button_bit(*zone);
                    down |= bit;
                    self.buttons_push |= bit;
                    new_frame = true;
                }
                TimedInputEvent::ButtonHold { zone, is_down, .. } => {
                    let bit = button_bit(*zone);
                    if *is_down {
                        self.buttons_push |= bit;
                        down |= bit;
                    } else {
                        self.buttons_push &= !bit;
                    }
                    new_frame = true;
                }
                TimedInputEvent::SensorClick { area, .. } => {
                    self.panel_state |= sensor_bits(*area);
                    new_frame = true;
                }
                TimedInputEvent::SensorHold { area, is_down, .. } => {
                    if *is_down {
                        self.panel_state |= sensor_bits(*area);
                    } else {
                        self.panel_state &= !sensor_bits(*area);
                    }
                    new_frame = true;
                }
            }
        }
        (down, self.panel_state, new_frame)
    }

    fn to_runtime_result(
        &self,
        result: SdezStepResult,
        current_time_us: i64,
    ) -> RuntimeStepLightResult {
        let events: Vec<JudgeEvent> = result
            .events
            .iter()
            .map(|event| judge_event(event, current_time_us))
            .collect();
        let audio_commands = events
            .iter()
            .map(|event| AudioCommand::PlayJudgeSfx {
                kind: event.kind,
                grade: event.grade,
                is_break: event.is_break,
                at_time: current_time_us,
                note_index: event.note_index,
            })
            .collect();
        let render_commands = events
            .iter()
            .map(|event| RenderCommand::ShowJudgeResult {
                kind: event.kind,
                grade: event.grade,
                is_break: event.is_break,
                diff: event.diff,
                note_index: event.note_index,
            })
            .collect();
        let score = result
            .score
            .as_ref()
            .map(score_state)
            .unwrap_or_else(empty_score_state);
        RuntimeStepLightResult {
            events,
            audio_commands,
            render_commands,
            score,
            current_time: current_time_us,
            terminated: false,
        }
    }
}

fn judge_event(event: &SdezEvent, current_time_us: i64) -> JudgeEvent {
    let kind = match event.kind.as_str() {
        "hold" => JudgeEventKind::Hold,
        "slide" => JudgeEventKind::Slide,
        "touch" => JudgeEventKind::Touch,
        _ if event.is_break => JudgeEventKind::Break,
        _ => JudgeEventKind::Tap,
    };
    let position = match kind {
        JudgeEventKind::Touch | JudgeEventKind::Slide => RuntimePos {
            button: None,
            sensor: Some(SensorArea::A1),
        },
        _ => RuntimePos {
            button: Some(ButtonZone::K1),
            sensor: None,
        },
    };
    JudgeEvent {
        kind,
        phase: if event.phase == "tail" {
            JudgePhase::Tail
        } else {
            JudgePhase::Head
        },
        grade: grade_from_etiming(&event.etiming),
        diff: (event.diff_msec * 1000.0).round() as i64,
        position,
        note_index: event.note_index,
        is_break: event.is_break,
        is_ex: event.is_ex,
        multiple: 1,
    }
}

fn grade_from_etiming(timing: &str) -> JudgeGrade {
    match timing {
        "TooFast" => JudgeGrade::TooFast,
        "FastGood" => JudgeGrade::FastGood,
        "FastGreat3rd" => JudgeGrade::FastGreat3rd,
        "FastGreat2nd" => JudgeGrade::FastGreat2nd,
        "FastGreat" => JudgeGrade::FastGreat,
        "FastPerfect2nd" | "FastPerfect" => JudgeGrade::FastPerfect2nd,
        "Critical" => JudgeGrade::Perfect,
        "LatePerfect" | "LatePerfect2nd" => JudgeGrade::LatePerfect2nd,
        "LateGreat" => JudgeGrade::LateGreat,
        "LateGreat2nd" => JudgeGrade::LateGreat2nd,
        "LateGreat3rd" => JudgeGrade::LateGreat3rd,
        "LateGood" => JudgeGrade::LateGood,
        _ => JudgeGrade::Miss,
    }
}

fn score_state(score: &crate::core::sdez::SdezScore) -> ScoreState {
    let counts = NoteTypeJudgeCounts {
        tap_count: family_counts(&score.counts[0]),
        hold_count: family_counts(&score.counts[1]),
        slide_count: family_counts(&score.counts[2]),
        touch_count: family_counts(&score.counts[3]),
        break_count: family_counts(&score.counts[4]),
    };
    let total_notes: u64 = counts
        .tap_count
        .values()
        .chain(counts.hold_count.values())
        .chain(counts.slide_count.values())
        .chain(counts.touch_count.values())
        .chain(counts.break_count.values())
        .sum();
    let max_dx_score = total_notes * 3;
    ScoreState {
        combo: score.combo as u64,
        p_combo: 0,
        c_p_combo: 0,
        total_base: score.total_base as u64,
        total_extra: score.total_break_bonus as u64,
        earned_base: score.earned_base as u64,
        earned_extra: score.earned_break_bonus as u64,
        earned_classic_extra: 0,
        lost_base: (score.total_base - score.earned_base) as u64,
        lost_extra: (score.total_break_bonus - score.earned_break_bonus) as u64,
        lost_classic_extra: 0,
        dx_score: score.dx_score as i64 - max_dx_score as i64,
        max_dx_score,
        fast_count: fast_count(&counts),
        late_count: late_count(&counts),
        counts,
    }
}

fn family_counts(values: &[u32]) -> BTreeMap<JudgeGrade, u64> {
    let mut map = BTreeMap::new();
    let timings = [
        "TooFast",
        "FastGood",
        "FastGreat3rd",
        "FastGreat2nd",
        "FastGreat",
        "FastPerfect2nd",
        "FastPerfect",
        "Critical",
        "LatePerfect",
        "LatePerfect2nd",
        "LateGreat",
        "LateGreat2nd",
        "LateGreat3rd",
        "LateGood",
        "TooLate",
    ];
    for (index, count) in values.iter().enumerate() {
        if *count > 0 {
            *map.entry(grade_from_etiming(timings[index])).or_insert(0) += u64::from(*count);
        }
    }
    map
}

fn empty_score_state() -> ScoreState {
    let empty = BTreeMap::new();
    ScoreState {
        combo: 0,
        p_combo: 0,
        c_p_combo: 0,
        total_base: 0,
        total_extra: 0,
        earned_base: 0,
        earned_extra: 0,
        earned_classic_extra: 0,
        lost_base: 0,
        lost_extra: 0,
        lost_classic_extra: 0,
        dx_score: 0,
        max_dx_score: 0,
        fast_count: 0,
        late_count: 0,
        counts: NoteTypeJudgeCounts {
            tap_count: empty.clone(),
            hold_count: empty.clone(),
            slide_count: empty.clone(),
            touch_count: empty.clone(),
            break_count: empty,
        },
    }
}

fn fast_count(counts: &NoteTypeJudgeCounts) -> u64 {
    counts.grade_count_where(|grade| {
        matches!(
            grade,
            JudgeGrade::FastGood
                | JudgeGrade::FastGreat3rd
                | JudgeGrade::FastGreat2nd
                | JudgeGrade::FastGreat
                | JudgeGrade::FastPerfect2nd
                | JudgeGrade::TooFast
        )
    })
}

fn late_count(counts: &NoteTypeJudgeCounts) -> u64 {
    counts.grade_count_where(|grade| {
        matches!(
            grade,
            JudgeGrade::LateGood
                | JudgeGrade::LateGreat3rd
                | JudgeGrade::LateGreat2nd
                | JudgeGrade::LateGreat
                | JudgeGrade::LatePerfect2nd
                | JudgeGrade::Miss
        )
    })
}

fn button_bit(zone: ButtonZone) -> u16 {
    match zone {
        ButtonZone::K1 => 1 << 0,
        ButtonZone::K2 => 1 << 1,
        ButtonZone::K3 => 1 << 2,
        ButtonZone::K4 => 1 << 3,
        ButtonZone::K5 => 1 << 4,
        ButtonZone::K6 => 1 << 5,
        ButtonZone::K7 => 1 << 6,
        ButtonZone::K8 => 1 << 7,
    }
}

fn sensor_bits(area: SensorArea) -> u64 {
    match area {
        SensorArea::A1 => 1 << 0,
        SensorArea::A2 => 1 << 1,
        SensorArea::A3 => 1 << 2,
        SensorArea::A4 => 1 << 3,
        SensorArea::A5 => 1 << 4,
        SensorArea::A6 => 1 << 5,
        SensorArea::A7 => 1 << 6,
        SensorArea::A8 => 1 << 7,
        SensorArea::B1 => 1 << 8,
        SensorArea::B2 => 1 << 9,
        SensorArea::B3 => 1 << 10,
        SensorArea::B4 => 1 << 11,
        SensorArea::B5 => 1 << 12,
        SensorArea::B6 => 1 << 13,
        SensorArea::B7 => 1 << 14,
        SensorArea::B8 => 1 << 15,
        SensorArea::C => (1 << 16) | (1 << 17),
        SensorArea::D1 => 1 << 18,
        SensorArea::D2 => 1 << 19,
        SensorArea::D3 => 1 << 20,
        SensorArea::D4 => 1 << 21,
        SensorArea::D5 => 1 << 22,
        SensorArea::D6 => 1 << 23,
        SensorArea::D7 => 1 << 24,
        SensorArea::D8 => 1 << 25,
        SensorArea::E1 => 1 << 26,
        SensorArea::E2 => 1 << 27,
        SensorArea::E3 => 1 << 28,
        SensorArea::E4 => 1 << 29,
        SensorArea::E5 => 1 << 30,
        SensorArea::E6 => 1 << 31,
        SensorArea::E7 => 1 << 32,
        SensorArea::E8 => 1 << 33,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_event_maps_to_sdez_judgment_and_score() {
        let mut engine =
            SdezEngine::load("&inote_1=(120){4}1,E", 1).expect("SDEZ engine");
        let events = [TimedInputEvent::ButtonClick {
            tp: 110_000,
            zone: ButtonZone::K1,
        }];
        let result = engine.step(110_000, &events).expect("step");
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].grade, JudgeGrade::Perfect);
        assert_eq!(result.score.combo, 1);
        assert_eq!(result.score.earned_base, 500);
        assert_eq!(result.audio_commands.len(), 1);
        assert_eq!(result.render_commands.len(), 1);
    }
}
