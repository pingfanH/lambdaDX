//! Typed wrapper over the in-process SDEZ170 runtime.

use serde::Deserialize;

use crate::session::{LnmaiError, Result};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdezEvent {
    pub note_index: u64,
    pub kind: String,
    pub phase: String,
    pub etiming: String,
    pub diff_msec: f32,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub button: Option<u8>,
    #[serde(default)]
    pub sensor: Option<u8>,
    #[serde(default)]
    pub is_wifi: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdezStepResult {
    pub events: Vec<SdezEvent>,
    pub current_time: f32,
    #[serde(default)]
    pub slides: Vec<SdezSlideProgress>,
    #[serde(default)]
    pub score: Option<SdezScore>,
}

/// Per-frame trail consumption of one live SDEZ slide.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdezSlideProgress {
    pub note_index: u64,
    #[serde(default)]
    pub is_wifi: bool,
    pub progress: f32,
    #[serde(default)]
    pub tracks: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdezScore {
    pub combo: u32,
    pub max_combo: u32,
    pub counts: Vec<Vec<u32>>,
    pub total_base: u32,
    pub earned_base: u32,
    pub total_break_bonus: u32,
    pub earned_break_bonus: u32,
    pub dx_score: u32,
}

pub struct SdezSession {
    chart: Option<lnmai_core_rs::chart_loader::ChartSpec>,
    runtime: Option<lnmai_core_rs::sdez::SdezRuntime>,
}

impl SdezSession {
    pub fn create() -> Result<Self> {
        Ok(Self {
            chart: None,
            runtime: None,
        })
    }

    pub fn load_text(
        &mut self,
        content: &str,
        level_index: u32,
        autoplay: u8,
    ) -> Result<()> {
        let chart = lnmai_core_rs::simai::frontend::frontend_lowered_chart_chart_clock(
            content,
            (level_index as usize).into(),
        )
        .map_err(|error| LnmaiError {
            json: error.message,
        })?;
        let mut runtime = lnmai_core_rs::sdez::SdezRuntime::from_chart(&chart);
        runtime.autoplay = autoplay_mode(autoplay);
        self.chart = Some(chart);
        self.runtime = Some(runtime);
        Ok(())
    }

    pub fn step(
        &mut self,
        current_msec: f32,
        buttons_down: u16,
        buttons_push: u16,
        panel_state: u64,
        panel_new_frame: bool,
    ) -> Result<SdezStepResult> {
        let runtime = self.runtime.as_mut().ok_or_else(|| LnmaiError {
            json: "SDEZ session has no loaded chart".to_string(),
        })?;
        runtime.input.update_panel(panel_state, panel_new_frame);
        runtime.input.set_buttons(buttons_down, buttons_push);
        let events = runtime.step(current_msec);
        let slides = runtime
            .slide_progress()
            .into_iter()
            .map(|slide| SdezSlideProgress {
                note_index: slide.note_index,
                is_wifi: slide.is_wifi,
                progress: slide.progress,
                tracks: slide.tracks,
            })
            .collect();
        let score = score_state(&runtime.score);
        Ok(SdezStepResult {
            events: events.iter().map(event_state).collect(),
            current_time: current_msec,
            slides,
            score: Some(score),
        })
    }

    /// Reuse the chart already installed in the legacy runtime.
    pub fn load_session(&mut self, handle: u64, autoplay: u8) -> Result<()> {
        let chart = lnmai_core_rs::direct::chart(handle)
            .map_err(|error| LnmaiError { json: error })?;
        let mut runtime = lnmai_core_rs::sdez::SdezRuntime::from_chart(&chart);
        runtime.autoplay = autoplay_mode(autoplay);
        self.chart = Some(chart);
        self.runtime = Some(runtime);
        Ok(())
    }

    pub fn set_autoplay(&mut self, autoplay: u8) -> Result<()> {
        let runtime = self.runtime.as_mut().ok_or_else(|| LnmaiError {
            json: "SDEZ session has no loaded chart".to_string(),
        })?;
        runtime.autoplay = autoplay_mode(autoplay);
        Ok(())
    }

    /// Rebuild the runtime from the chart cached by the core session. This is
    /// the seek fast path: no Simai parse or chart lowering.
    pub fn reset(&mut self, autoplay: u8) -> Result<()> {
        let chart = self.chart.as_ref().ok_or_else(|| LnmaiError {
            json: "SDEZ session has no loaded chart".to_string(),
        })?;
        let mut runtime = lnmai_core_rs::sdez::SdezRuntime::from_chart(chart);
        runtime.autoplay = autoplay_mode(autoplay);
        self.runtime = Some(runtime);
        Ok(())
    }
}

fn autoplay_mode(autoplay: u8) -> lnmai_core_rs::sdez::AutoplayMode {
    use lnmai_core_rs::sdez::AutoplayMode::*;
    match autoplay {
        1 => Critical,
        2 => Perfect,
        3 => Great,
        4 => Good,
        5 => Miss,
        _ => None,
    }
}

fn event_state(event: &lnmai_core_rs::sdez::SdezEvent) -> SdezEvent {
    use lnmai_core_rs::sdez::{SdezNoteKind, SdezPhase};
    SdezEvent {
        note_index: event.note_index,
        kind: match event.kind {
            SdezNoteKind::Tap => "tap",
            SdezNoteKind::Hold => "hold",
            SdezNoteKind::Slide => "slide",
            SdezNoteKind::Touch => "touch",
        }
        .to_string(),
        phase: match event.phase {
            SdezPhase::Head => "head",
            SdezPhase::Tail => "tail",
        }
        .to_string(),
        etiming: format!("{:?}", event.judged.timing),
        diff_msec: event.judged.diff_msec,
        is_break: event.is_break,
        is_ex: event.is_ex,
        button: event.button,
        sensor: event.sensor,
        is_wifi: event.is_wifi,
    }
}

fn score_state(score: &lnmai_core_rs::sdez::SdezScore) -> SdezScore {
    SdezScore {
        combo: score.combo,
        max_combo: score.max_combo,
        counts: score
            .counts
            .iter()
            .map(|row| row.to_vec())
            .collect(),
        total_base: score.total_base,
        earned_base: score.earned_base,
        total_break_bonus: score.total_break_bonus,
        earned_break_bonus: score.earned_break_bonus,
        dx_score: score.dx_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_sdez_session_roundtrip() {
        let mut session = SdezSession::create().expect("session");
        session
            .load_text("&inote_1=(120){4}1,E", 1, 1)
            .expect("load");
        let result = session.step(110.0, 0, 0, 0, true).expect("step");
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].kind, "tap");
        assert_eq!(result.events[0].etiming, "Critical");
        let score = result.score.expect("score");
        assert_eq!(score.combo, 1);
        assert_eq!(score.earned_base, 500);
    }
}
