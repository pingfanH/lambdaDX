use crate::raw;
use crate::session::{LnmaiError, Result};
use crate::types;
use lean_sys::{lean_io_result_is_error, lean_io_result_take_value, lean_object, lean_string_cstr};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::ffi::{CStr, CString};

fn ffi_error(message: impl Into<String>) -> LnmaiError {
    LnmaiError { json: message.into() }
}

fn mk_lean_string(content: &str) -> Result<*mut lean_object> {
    let c = CString::new(content)
        .map_err(|_| ffi_error("FFI string input contains an interior NUL byte"))?;
    Ok(unsafe { raw::lean_mk_string(c.as_ptr()) })
}

fn into_string(result: *mut lean_object) -> Result<String> {
    if result.is_null() {
        return Err(ffi_error("Lean FFI returned a null string object"));
    }
    unsafe {
        let ptr = lean_string_cstr(result);
        let value = CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned();
        lean_sys::lean_dec_ref(result);
        Ok(value)
    }
}

fn into_io_string(result: *mut lean_object) -> Result<String> {
    if result.is_null() {
        return Err(ffi_error("Lean FFI returned a null IO result object"));
    }
    unsafe {
        if lean_io_result_is_error(result) {
            lean_sys::lean_io_result_show_error(result);
            lean_sys::lean_dec_ref(result);
            return Err(ffi_error("Lean FFI returned an IO error"));
        }
        let value_obj = lean_io_result_take_value(result);
        let ptr = lean_string_cstr(value_obj);
        let value = CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned();
        lean_sys::lean_dec_ref(value_obj);
        Ok(value)
    }
}

fn decode_envelope<T: DeserializeOwned>(json: String) -> Result<T> {
    let envelope: types::FfiEnvelope<T> =
        serde_json::from_str(&json).map_err(|_| LnmaiError { json: json.clone() })?;
    if envelope.ok {
        envelope.result.ok_or(LnmaiError { json })
    } else {
        Err(LnmaiError { json })
    }
}

fn call_parse<T: DeserializeOwned>(
    content: &str,
    level_index: u32,
    f: unsafe extern "C" fn(*mut lean_object, u32) -> *mut lean_object,
) -> Result<T> {
    let content_obj = mk_lean_string(content)?;
    // Exported Lean functions consume owned Lean object arguments.
    let result = unsafe { f(content_obj, level_index) };
    let json = into_string(result)?;
    decode_envelope(json)
}

fn call_json_input<I: Serialize, O: DeserializeOwned>(
    input: &I,
    f: unsafe extern "C" fn(*mut lean_object) -> *mut lean_object,
) -> Result<O> {
    let input_json = serde_json::to_string(input)
        .map_err(|err| LnmaiError { json: err.to_string() })?;
    let input_obj = mk_lean_string(&input_json)?;
    // Exported Lean functions consume owned Lean object arguments.
    let result = unsafe { f(input_obj) };
    let json = into_string(result)?;
    decode_envelope(json)
}

pub fn ffi_version() -> Result<types::FfiVersion> {
    let json = unsafe { into_io_string(raw::lnmai_ffi_version_json())? };
    decode_envelope(json)
}

pub fn parse_frontend_chart(content: &str, level_index: u32) -> Result<types::FrontendChartResult> {
    call_parse(content, level_index, raw::lnmai_parse_frontend_chart_json)
}

pub fn parse_frontend_semantic_chart(
    content: &str,
    level_index: u32,
) -> Result<types::FrontendSemanticChart> {
    call_parse(content, level_index, raw::lnmai_parse_frontend_semantic_chart_json)
}

pub fn parse_frontend_inspection_chart(
    content: &str,
    level_index: u32,
) -> Result<types::FrontendChartInspection> {
    call_parse(content, level_index, raw::lnmai_parse_frontend_inspection_chart_json)
}

pub fn parse_normalized_chart(content: &str, level_index: u32) -> Result<types::NormalizedChart> {
    call_parse(content, level_index, raw::lnmai_parse_normalized_chart_json)
}

pub fn parse_lowered_chart(content: &str, level_index: u32) -> Result<types::ChartSpec> {
    call_parse(content, level_index, raw::lnmai_parse_lowered_chart_json)
}

/// Builds runtime state from a lowered chart payload.
///
/// The lowered schema now uses explicit `slideHeads` plus slide-body `slides`.
/// Matching lowered head/body objects are linked by `logicalSlideId`.
/// Lowered slide bodies serialize `headTiming` as the body-side preserved head
/// anchor. Touch-mode policy is intentionally absent from `GameState`; hosts
/// that want desktop-style touch behavior should synthesize sensor input before
/// stepping the runtime.
pub fn build_game_state(chart_spec: &types::ChartSpec) -> Result<types::GameState> {
    call_json_input(chart_spec, raw::lnmai_build_game_state_json)
}

pub fn default_tactic_from_chart(
    chart_spec: &types::ChartSpec,
) -> Result<types::ManualTacticSequence> {
    call_json_input(chart_spec, raw::lnmai_default_tactic_from_chart_json)
}

/// Steps a full JSON-visible runtime state by one timed input batch.
///
/// Button events drive outer-button notes such as taps, holds, and slide heads.
/// Sensor events drive screen notes and bodies such as touch notes, touch holds,
/// and slide bodies. Any desktop outer-button-to-sensor mapping belongs in the
/// host input layer before calling this helper.
pub fn step_game_state(
    state: &types::GameState,
    batch: &types::TimedInputBatch,
) -> Result<types::RuntimeStepResult> {
    let state_json = serde_json::to_string(state)
        .map_err(|err| LnmaiError { json: err.to_string() })?;
    let batch_json = serde_json::to_string(batch)
        .map_err(|err| LnmaiError { json: err.to_string() })?;
    let state_obj = mk_lean_string(&state_json)?;
    let batch_obj = mk_lean_string(&batch_json)?;
    // Exported Lean functions consume owned Lean object arguments.
    let result = unsafe { raw::lnmai_step_game_state_json(state_obj, batch_obj) };
    let json = into_string(result)?;
    decode_envelope(json)
}
