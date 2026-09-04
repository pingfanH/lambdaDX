use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::sync::OnceLock;

use crate::raw;
use crate::types;
use lean_sys::{lean_io_result_is_error, lean_io_result_take_value, lean_string_cstr};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub struct Empty;
pub struct Loaded;

#[derive(Debug, Clone)]
pub struct FfiEnvelope {
    pub json: String,
}

impl FfiEnvelope {
    pub fn decode<T: DeserializeOwned>(&self) -> serde_json::Result<types::FfiEnvelope<T>> {
        serde_json::from_str(&self.json)
    }

    pub fn decode_result<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        let envelope: types::FfiEnvelope<T> = self.decode()?;
        envelope.result.ok_or_else(|| {
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "FFI envelope does not contain a result payload",
            ))
        })
    }
}

#[derive(Debug, Clone)]
pub struct LnmaiError {
    pub json: String,
}

pub type Result<T> = std::result::Result<T, LnmaiError>;

pub struct Session<State> {
    handle: Option<u64>,
    _state: PhantomData<State>,
}

impl<State> Session<State> {
    pub fn handle(&self) -> u64 {
        self.handle.expect("session handle has already been consumed")
    }

    fn new(handle: u64) -> Self {
        Self {
            handle: Some(handle),
            _state: PhantomData,
        }
    }

    fn take_handle(&mut self) -> u64 {
        self.handle
            .take()
            .expect("session handle has already been consumed")
    }
}

fn ffi_error(message: impl Into<String>) -> LnmaiError {
    LnmaiError { json: message.into() }
}

fn into_string(result: *mut lean_sys::lean_object) -> Result<String> {
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

fn mk_lean_string(content: &str) -> Result<*mut lean_sys::lean_object> {
    let c = CString::new(content)
        .map_err(|_| ffi_error("FFI string input contains an interior NUL byte"))?;
    Ok(unsafe { raw::lean_mk_string(c.as_ptr()) })
}

fn decode_raw_envelope(json: &str) -> Option<types::FfiEnvelope<Value>> {
    serde_json::from_str(json).ok()
}

fn is_ok(json: &str) -> bool {
    decode_raw_envelope(json).is_some_and(|envelope| envelope.ok)
}

fn handle_from_value(value: &Value) -> Option<u64> {
    value
        .get("handle")
        .and_then(|handle| handle.as_u64().or_else(|| handle.as_str()?.parse().ok()))
}

fn ok_or_error(json: String) -> Result<FfiEnvelope> {
    if is_ok(&json) {
        Ok(FfiEnvelope { json })
    } else {
        Err(LnmaiError { json })
    }
}

/// Initialize the embedded Lean runtime once for the whole process.
///
/// Lean's runtime initialization is not idempotent. Keeping the guard in the
/// FFI crate gives all Rust callers (parser and gameplay session alike) the
/// same process-wide initialization state.
pub fn ensure_runtime() -> std::result::Result<(), ()> {
    static INIT: OnceLock<std::result::Result<(), ()>> = OnceLock::new();
    INIT.get_or_init(|| unsafe { raw::initialize_lnmai_runtime() })
        .clone()
}

/// Initialize the embedded Lean runtime without the process-wide guard.
///
/// Prefer [`ensure_runtime`] for application code. This remains available for
/// low-level callers that own the runtime lifecycle themselves.
pub unsafe fn initialize_runtime() -> std::result::Result<(), ()> {
    unsafe { raw::initialize_lnmai_runtime() }
}

impl Session<Empty> {
    pub fn create() -> Result<Self> {
        let json = unsafe { into_string(raw::lnmai_create_empty_session_handle())? };
        let envelope: types::FfiEnvelope<Value> =
            serde_json::from_str(&json).map_err(|_| LnmaiError { json: json.clone() })?;
        if !envelope.ok {
            return Err(LnmaiError { json });
        }
        let handle = envelope
            .result
            .as_ref()
            .and_then(handle_from_value)
            .ok_or_else(|| LnmaiError { json: json.clone() })?;
        Ok(Self::new(handle))
    }

    pub fn load_chart_text(mut self, content: &str, level_index: u32) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let handle = self.handle();
        let content_obj = mk_lean_string(content)?;
        // Exported Lean functions consume owned Lean object arguments.
        let result = unsafe { raw::lnmai_load_chart_into_session_from_text(handle, content_obj, level_index) };
        let json = into_string(result)?;
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn load_chart_json(mut self, chart_spec_json: &str) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let handle = self.handle();
        let chart_obj = mk_lean_string(chart_spec_json)?;
        // Exported Lean functions consume owned Lean object arguments.
        let result = unsafe { raw::lnmai_load_chart_into_session_from_json(handle, chart_obj) };
        let json = into_string(result)?;
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn free(mut self) -> Result<FfiEnvelope> {
        let handle = self.take_handle();
        let json = unsafe { into_string(raw::lnmai_free_game_state_handle(handle))? };
        ok_or_error(json)
    }
}

impl Session<Loaded> {
    pub fn get_lowered_chart_json(&self) -> Result<FfiEnvelope> {
        let json = unsafe { into_string(raw::lnmai_get_lowered_chart_json_by_handle(self.handle()))? };
        ok_or_error(json)
    }

    pub fn advance_frame_light(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        let batch_obj = mk_lean_string(batch_json)?;
        // Exported Lean functions consume owned Lean object arguments.
        let result = unsafe { raw::lnmai_step_game_state_handle_light(self.handle(), batch_obj) };
        let json = into_string(result)?;
        ok_or_error(json)
    }

    pub fn advance_frame_full(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        let batch_obj = mk_lean_string(batch_json)?;
        // Exported Lean functions consume owned Lean object arguments.
        let result = unsafe { raw::lnmai_step_game_state_handle(self.handle(), batch_obj) };
        let json = into_string(result)?;
        ok_or_error(json)
    }

    pub fn get_state_json(&self) -> Result<FfiEnvelope> {
        let json = unsafe { into_string(raw::lnmai_get_game_state_json_by_handle(self.handle()))? };
        ok_or_error(json)
    }

    pub fn unload_chart(mut self) -> Result<(Session<Empty>, FfiEnvelope)> {
        let handle = self.handle();
        let json = unsafe { into_string(raw::lnmai_unload_chart_from_session(handle))? };
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn free(mut self) -> Result<FfiEnvelope> {
        let handle = self.take_handle();
        let json = unsafe { into_string(raw::lnmai_free_game_state_handle(handle))? };
        ok_or_error(json)
    }
}
