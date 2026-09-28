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
