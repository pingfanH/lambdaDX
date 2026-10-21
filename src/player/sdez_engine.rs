//! SDEZ170 backend adapter for the player.

use std::collections::BTreeMap;
use std::cell::Cell;

use crate::core::sdez::{SdezEvent, SdezSession, SdezStepResult};
use crate::core::types::{
    AudioCommand, ButtonZone, JudgeEvent, JudgeEventKind, JudgeGrade, JudgePhase,
    NoteTypeJudgeCounts, RenderCommand, RuntimePos, RuntimeStepLightResult, ScoreState,
    SeekResult, SensorArea, TimedInputEvent,
};

/// SDEZ chart notes are shifted by the default 60 ms AdjustTiming and the
/// 50 ms judge-table center. Player-facing timestamps are chart/song time.
pub const SDEZ_INPUT_OFFSET_US: i64 = 110_000;
/// Autoplay is not driven by a human input edge. Its judge tick should land on
/// the chart note time so the hit SFX and answer cue share one timeline.
pub const SDEZ_AUTOPLAY_OFFSET_US: i64 = 60_000;

pub struct SdezEngine {
    session: SdezSession,
    panel_state: u64,
    buttons_push: u16,
    last_time_us: i64,
    text: String,
    level_index: u32,
    autoplay: Cell<bool>,
    applied_autoplay: Cell<Option<bool>>,
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
            text: text.to_string(),
            level_index,
            autoplay: Cell::new(false),
            applied_autoplay: Cell::new(None),
        })
    }

    pub fn step(
        &mut self,
        current_time_us: i64,
        events: &[TimedInputEvent],
    ) -> Result<RuntimeStepLightResult, String> {
        let wanted = self.autoplay.get();
        if self.applied_autoplay.get() != Some(wanted) {
            self.session
                .set_autoplay(if wanted { 1 } else { 0 })
                .map_err(|e| e.json)?;
            self.applied_autoplay.set(Some(wanted));
        }
        let (buttons_down, panel_state, new_frame) = self.apply_events(events);
        let offset = if wanted {
            SDEZ_AUTOPLAY_OFFSET_US
        } else {
            SDEZ_INPUT_OFFSET_US
        };
        let judged_time_us = current_time_us + offset;
        let result = self
            .session
            .step(
                judged_time_us as f32 / 1000.0,
                buttons_down,
                self.buttons_push,
                panel_state,
                new_frame,
            )
            .map_err(|e| e.json)?;
        self.last_time_us = current_time_us;
        Ok(self.to_runtime_result(result, current_time_us))
    }

    pub fn set_autoplay(&mut self, enabled: bool) {
        self.autoplay.set(enabled);
        self.applied_autoplay.set(None);
    }

    pub fn enable_autoplay(&self) {
        self.autoplay.set(true);
        self.applied_autoplay.set(None);
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

    pub fn seek(
        &mut self,
        target_us: i64,
        events: &[TimedInputEvent],
    ) -> Result<SeekResult, String> {
        let mut session = SdezSession::create().map_err(|e| e.json)?;
        let autoplay = if events.is_empty() { 1 } else { 0 };
        session
            .load_text(&self.text, self.level_index, autoplay)
            .map_err(|e| e.json)?;
        self.session = session;
        self.autoplay.set(events.is_empty());
        self.applied_autoplay.set(None);
        self.panel_state = 0;
        self.buttons_push = 0;
        self.last_time_us = 0;

        let mut sorted = events.to_vec();
        sorted.sort_by_key(|event| event_tp(event));
        let mut cursor = 0usize;
        let mut frame_us = 0i64;
        let mut last = None;
        while frame_us < target_us {
            frame_us = (frame_us + 16_667).min(target_us);
            let mut frame_events = Vec::new();
            while cursor < sorted.len() && event_tp(&sorted[cursor]) <= frame_us {
                frame_events.push(sorted[cursor].clone());
                cursor += 1;
            }
            last = Some(self.step(frame_us, &frame_events)?);
        }
        let last = match last {
            Some(last) => last,
            None => self.step(target_us, &[])?,
        };
        Ok(SeekResult {
            score: last.score,
            current_time: target_us,
            terminated: false,
            retained_note_indices: Vec::new(),
            skipped_note_indices: Vec::new(),
            render_commands: last.render_commands,
        })
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
        let mut render_commands = Vec::with_capacity(events.len());
        for (source, event) in result.events.iter().zip(events.iter()) {
            render_commands.push(RenderCommand::ShowJudgeResult {
                kind: event.kind,
                grade: event.grade,
                is_break: event.is_break,
                diff: event.diff,
                note_index: event.note_index,
            });
            if event.kind == JudgeEventKind::Slide {
                render_commands.push(RenderCommand::HideAllSlideBars {
                    note_index: event.note_index,
                });
                if source.is_wifi {
                    for track_index in 0..3 {
                        render_commands.push(RenderCommand::HideSlideTrackBars {
                            note_index: event.note_index,
                            track_index,
                            end_index: 11,
                        });
                    }
                }
            }
        }
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
    let position = RuntimePos {
        button: event.button.and_then(button_zone),
        sensor: event.sensor.and_then(sensor_area),
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

fn event_tp(event: &TimedInputEvent) -> i64 {
    match event {
        TimedInputEvent::ButtonClick { tp, .. }
        | TimedInputEvent::ButtonHold { tp, .. }
        | TimedInputEvent::SensorClick { tp, .. }
        | TimedInputEvent::SensorHold { tp, .. } => *tp,
    }
}

fn button_zone(index: u8) -> Option<ButtonZone> {
    Some(match index {
        0 => ButtonZone::K1,
        1 => ButtonZone::K2,
        2 => ButtonZone::K3,
        3 => ButtonZone::K4,
        4 => ButtonZone::K5,
        5 => ButtonZone::K6,
        6 => ButtonZone::K7,
        7 => ButtonZone::K8,
        _ => return None,
    })
}

fn sensor_area(index: u8) -> Option<SensorArea> {
    Some(match index {
        0 => SensorArea::A1,
        1 => SensorArea::A2,
        2 => SensorArea::A3,
        3 => SensorArea::A4,
        4 => SensorArea::A5,
        5 => SensorArea::A6,
        6 => SensorArea::A7,
        7 => SensorArea::A8,
        8 => SensorArea::B1,
        9 => SensorArea::B2,
        10 => SensorArea::B3,
        11 => SensorArea::B4,
        12 => SensorArea::B5,
        13 => SensorArea::B6,
        14 => SensorArea::B7,
        15 => SensorArea::B8,
        16 | 17 => SensorArea::C,
        18 => SensorArea::D1,
        19 => SensorArea::D2,
        20 => SensorArea::D3,
        21 => SensorArea::D4,
        22 => SensorArea::D5,
        23 => SensorArea::D6,
        24 => SensorArea::D7,
        25 => SensorArea::D8,
        26 => SensorArea::E1,
        27 => SensorArea::E2,
        28 => SensorArea::E3,
        29 => SensorArea::E4,
        30 => SensorArea::E5,
        31 => SensorArea::E6,
        32 => SensorArea::E7,
        33 => SensorArea::E8,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_event_maps_to_sdez_judgment_and_score() {
        let mut engine =
            SdezEngine::load("&inote_1=(120){4}1,E", 1).expect("SDEZ engine");
        let events = [TimedInputEvent::ButtonClick {
            tp: 0,
            zone: ButtonZone::K1,
        }];
        let result = engine.step(0, &events).expect("step");
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].grade, JudgeGrade::Perfect);
        assert_eq!(result.score.combo, 1);
        assert_eq!(result.score.earned_base, 500);
        assert_eq!(result.audio_commands.len(), 1);
        assert_eq!(result.render_commands.len(), 1);
    }

    #[test]
    fn seek_rebuilds_and_fast_forwards_the_sdez_session() {
        let mut engine = SdezEngine::load("&inote_1=(120){4}1,E", 1).expect("SDEZ engine");
        let result = engine.seek(500_000, &[]).expect("seek");
        assert_eq!(result.current_time, 500_000);
        assert_eq!(result.score.combo, 1);
    }

    #[test]
    fn autoplay_hit_sfx_lands_on_chart_time() {
        let mut engine = SdezEngine::load("&inote_1=(60){4},1,E", 1).expect("SDEZ engine");
        engine.set_autoplay(true);
        let mut judged_at = None;
        let mut t = 0i64;
        while t <= 1_500_000 {
            let result = engine.step(t, &[]).expect("step");
            if result.events.iter().any(|event| event.kind == JudgeEventKind::Tap) {
                judged_at = Some(t);
                break;
            }
            t += 16_667;
        }
        let judged_at = judged_at.expect("autoplay tap");
        assert!(
            (980_000..=1_010_000).contains(&judged_at),
            "autoplay hit SFX should land on chart time, got {judged_at}us"
        );
    }
}
