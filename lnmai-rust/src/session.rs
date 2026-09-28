//! Session API over the pure-Rust `lnmai_core` port.
//!
//! Mirrors the previous Lean FFI `session` module so callers (`engine.rs`) are
//! unchanged: the JSON envelope schema is identical, only the backend differs.

use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use crate::types;

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

/// Nanosecond breakdown of one step; kept for API parity (the Rust backend is
/// in-process, so the string-bridge timings are zero).
#[derive(Debug, Clone, Copy, Default)]
pub struct FfiStepTimings {
    pub mk_string_ns: u64,
    pub ffi_ns: u64,
    pub into_string_ns: u64,
}

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

/// No-op: the Rust core has no separate runtime to initialize.
pub fn ensure_runtime() -> std::result::Result<(), ()> {
    Ok(())
}

/// No-op: kept for API parity with the Lean FFI backend.
pub unsafe fn initialize_runtime() -> std::result::Result<(), ()> {
    Ok(())
}

fn ok_or_error(json: String) -> Result<FfiEnvelope> {
    match serde_json::from_str::<types::FfiEnvelope<serde_json::Value>>(&json) {
        Ok(envelope) if envelope.ok => Ok(FfiEnvelope { json }),
        _ => Err(LnmaiError { json }),
    }
}

fn handle_from_json(json: &str) -> Result<u64> {
    let envelope: types::FfiEnvelope<serde_json::Value> =
        serde_json::from_str(json).map_err(|_| LnmaiError { json: json.to_string() })?;
    if !envelope.ok {
        return Err(LnmaiError { json: json.to_string() });
    }
    envelope
        .result
        .as_ref()
        .and_then(|value| value.get("handle"))
        .and_then(|handle| {
            handle
                .as_u64()
                .or_else(|| handle.as_str()?.parse::<u64>().ok())
        })
        .ok_or_else(|| LnmaiError { json: json.to_string() })
}

impl Session<Empty> {
    pub fn create() -> Result<Self> {
        let json = lnmai_core_rs::ffi::create_empty_session_handle();
        Ok(Self::new(handle_from_json(&json)?))
    }

    pub fn load_chart_text(
        mut self,
        content: &str,
        level_index: u32,
    ) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let handle = self.handle();
        let json =
            lnmai_core_rs::ffi::load_chart_into_session_from_text(handle, content, level_index);
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn load_chart_json(
        mut self,
        chart_spec_json: &str,
    ) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let handle = self.handle();
        let json = lnmai_core_rs::ffi::load_chart_into_session_from_json(handle, chart_spec_json);
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn free(mut self) -> Result<FfiEnvelope> {
        let handle = self.take_handle();
        ok_or_error(lnmai_core_rs::ffi::free_game_state_handle(handle))
    }
}

impl Session<Loaded> {
    pub fn get_lowered_chart_json(&self) -> Result<FfiEnvelope> {
        ok_or_error(lnmai_core_rs::ffi::get_lowered_chart_json_by_handle(
            self.handle(),
        ))
    }

    pub fn advance_frame_light(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        self.advance_frame_light_timed(batch_json).map(|(e, _)| e)
    }

    pub fn advance_frame_light_timed(
        &mut self,
        batch_json: &str,
    ) -> Result<(FfiEnvelope, FfiStepTimings)> {
        let t0 = std::time::Instant::now();
        let json = lnmai_core_rs::ffi::step_game_state_handle_light(self.handle(), batch_json);
        let t1 = std::time::Instant::now();
        let envelope = ok_or_error(json)?;
        let timings = FfiStepTimings {
            mk_string_ns: 0,
            ffi_ns: (t1 - t0).as_nanos() as u64,
            into_string_ns: 0,
        };
        Ok((envelope, timings))
    }

    pub fn advance_frame_full(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        ok_or_error(lnmai_core_rs::ffi::step_game_state_handle(
            self.handle(),
            batch_json,
        ))
    }

    pub fn get_state_json(&self) -> Result<FfiEnvelope> {
        ok_or_error(lnmai_core_rs::ffi::get_game_state_json_by_handle(
            self.handle(),
        ))
    }

    pub fn unload_chart(mut self) -> Result<(Session<Empty>, FfiEnvelope)> {
        let handle = self.handle();
        let json = lnmai_core_rs::ffi::unload_chart_from_session(handle);
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn free(mut self) -> Result<FfiEnvelope> {
        let handle = self.take_handle();
        ok_or_error(lnmai_core_rs::ffi::free_game_state_handle(handle))
    }
}
