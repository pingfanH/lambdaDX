//! Typed wrapper over the in-process SDEZ170 runtime.

use serde::Deserialize;

use crate::session::{LnmaiError, Result};
use crate::types;

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
    pub score: Option<SdezScore>,
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
    handle: Option<u64>,
}

impl SdezSession {
    pub fn create() -> Result<Self> {
        let json = lnmai_core_rs::ffi::create_sdez_session_handle();
        let envelope: types::FfiEnvelope<serde_json::Value> =
            serde_json::from_str(&json).map_err(|error| LnmaiError {
                json: error.to_string(),
            })?;
        if !envelope.ok {
            return Err(LnmaiError { json });
        }
        let handle = envelope
            .result
            .as_ref()
            .and_then(|value| value.get("handle"))
            .and_then(|handle| {
                handle
                    .as_u64()
                    .or_else(|| handle.as_str()?.parse::<u64>().ok())
            })
            .ok_or_else(|| LnmaiError { json })?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub fn load_text(
        &mut self,
        content: &str,
        level_index: u32,
        autoplay: u8,
    ) -> Result<()> {
        let json = lnmai_core_rs::ffi::load_sdez_chart_text(
            self.handle(),
            content,
            level_index,
            autoplay,
        );
        expect_ok(json)
    }

    pub fn step(
        &mut self,
        current_msec: f32,
        buttons_down: u16,
        buttons_push: u16,
        panel_state: u64,
        panel_new_frame: bool,
    ) -> Result<SdezStepResult> {
        let json = lnmai_core_rs::ffi::step_sdez_handle(
            self.handle(),
            current_msec,
            buttons_down,
            buttons_push,
            panel_state,
            panel_new_frame,
        );
        decode_result(json)
    }

    pub fn set_autoplay(&mut self, autoplay: u8) -> Result<()> {
        expect_ok(lnmai_core_rs::ffi::set_sdez_autoplay(
            self.handle(),
            autoplay,
        ))
    }

    pub fn handle(&self) -> u64 {
        self.handle.expect("SDEZ session has been consumed")
    }
}

impl Drop for SdezSession {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = lnmai_core_rs::ffi::free_sdez_handle(handle);
        }
    }
}

fn expect_ok(json: String) -> Result<()> {
    let envelope: types::FfiEnvelope<serde_json::Value> =
        serde_json::from_str(&json).map_err(|error| LnmaiError {
            json: error.to_string(),
        })?;
    if envelope.ok {
        Ok(())
    } else {
        Err(LnmaiError { json })
    }
}

fn decode_result<T: serde::de::DeserializeOwned>(json: String) -> Result<T> {
    let envelope: types::FfiEnvelope<T> =
        serde_json::from_str(&json).map_err(|error| LnmaiError {
            json: error.to_string(),
        })?;
    if envelope.ok {
        envelope.result.ok_or_else(|| LnmaiError {
            json: "SDEZ envelope declared ok without a result".to_string(),
        })
    } else {
        Err(LnmaiError { json })
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
