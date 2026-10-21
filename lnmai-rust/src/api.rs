//! JSON API helpers over the pure-Rust `lnmai_core` port.

use serde::de::DeserializeOwned;

use crate::session::{LnmaiError, Result};
use crate::types;

fn decode_envelope<T: DeserializeOwned>(json: String) -> Result<T> {
    let envelope: types::FfiEnvelope<T> = serde_json::from_str(&json).map_err(|error| {
        LnmaiError {
            json: format!("failed to decode lnmai-core FFI envelope: {error}"),
        }
    })?;
    if envelope.ok {
        envelope.result.ok_or_else(|| LnmaiError {
            json: "lnmai-core FFI envelope declared ok but carried no result".to_string(),
        })
    } else {
        Err(LnmaiError { json })
    }
}

pub fn ffi_version() -> Result<types::FfiVersion> {
    decode_envelope(lnmai_core_rs::ffi::ffi_version_json())
}

pub fn parse_frontend_chart(
    content: &str,
    level_index: u32,
) -> Result<types::FrontendChartResult> {
    decode_envelope(lnmai_core_rs::ffi::parse_frontend_chart_json(
        content,
        level_index,
    ))
}

pub fn parse_frontend_semantic_chart(
    content: &str,
    level_index: u32,
) -> Result<types::FrontendSemanticChart> {
    decode_envelope(lnmai_core_rs::ffi::parse_frontend_semantic_chart_json(
        content,
        level_index,
    ))
}

pub fn parse_frontend_inspection_chart(
    content: &str,
    level_index: u32,
) -> Result<types::FrontendChartInspection> {
    decode_envelope(lnmai_core_rs::ffi::parse_frontend_inspection_chart_json(
        content,
        level_index,
    ))
}

pub fn parse_normalized_chart(
    content: &str,
    level_index: u32,
) -> Result<types::NormalizedChart> {
    decode_envelope(lnmai_core_rs::ffi::parse_normalized_chart_json(
        content,
        level_index,
    ))
}

pub fn parse_lowered_chart(content: &str, level_index: u32) -> Result<types::ChartSpec> {
    decode_envelope(lnmai_core_rs::ffi::parse_lowered_chart_json(
        content,
        level_index,
    ))
}

pub fn build_game_state(chart_spec: &types::ChartSpec) -> Result<types::GameState> {
    let chart_json = serde_json::to_string(chart_spec).map_err(|error| LnmaiError {
        json: error.to_string(),
    })?;
    decode_envelope(lnmai_core_rs::ffi::build_game_state_json(&chart_json))
}

pub fn step_game_state(
    state: &types::GameState,
    batch: &types::TimedInputBatch,
) -> Result<types::RuntimeStepResult> {
    let state_json = serde_json::to_string(state).map_err(|error| LnmaiError {
        json: error.to_string(),
    })?;
    let batch_json = serde_json::to_string(batch).map_err(|error| LnmaiError {
        json: error.to_string(),
    })?;
    decode_envelope(lnmai_core_rs::ffi::step_game_state_json(
        &state_json, &batch_json,
    ))
}

/// Build lnmai-core's default replay tactic (autoplay events) for a lowered
/// chart. JSON schema matches the Lean FFI backend.
pub fn default_tactic_from_chart(
    chart_spec: &types::ChartSpec,
) -> Result<types::ManualTacticSequence> {
    let chart_json = serde_json::to_string(chart_spec).map_err(|error| LnmaiError {
        json: error.to_string(),
    })?;
    decode_envelope(lnmai_core_rs::ffi::default_tactic_from_chart_json(
        &chart_json,
    ))
}
