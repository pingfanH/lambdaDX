//! Session API over the pure-Rust `lnmai_core` port.
//!
//! Mirrors the previous Lean FFI `session` module so callers (`engine.rs`) are
//! unchanged: the JSON envelope schema is identical, only the backend differs.

use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use crate::types;

pub struct Empty;
pub struct Loaded;

/// A source token on the same exact microsecond clock as the loaded chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartTextToken {
    pub time_us: i64,
    pub text: String,
}

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

impl<State> Drop for Session<State> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = lnmai_core_rs::direct::free(handle);
        }
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
    /// Create a session through the in-process ABI-3 bridge.
    pub fn create_native() -> Result<Self> {
        Ok(Self::new(lnmai_core_rs::direct::create()))
    }

    /// Load chart text through the in-process ABI-3 bridge.
    pub fn load_chart_text_native(
        mut self,
        content: &str,
        level_index: u32,
    ) -> Result<Session<Loaded>> {
        let parsed = crate::frontend::parse(content, level_index)?;
        lnmai_core_rs::direct::load_spec(self.handle(), parsed.semantic.lowered)
            .map_err(|error| LnmaiError { json: error })?;
        Ok(Session::new(self.take_handle()))
    }

    /// Reuse a frontend parse for judgment and source highlighting.
    pub fn load_parsed_native(
        mut self,
        parsed: crate::frontend::FrontendChartResult,
    ) -> Result<(Session<Loaded>, Vec<ChartTextToken>)> {
        let tokens = parsed.inspection.tokens.into_iter().map(|token| {
            let time_us = token.timing.micros.to_i64().ok_or_else(|| LnmaiError {
                json: "source token timing exceeds the microsecond clock range".into(),
            })?;
            Ok(ChartTextToken { time_us, text: token.raw_text })
        }).collect::<Result<Vec<_>>>()?;
        lnmai_core_rs::direct::load_spec(self.handle(), parsed.semantic.lowered)
            .map_err(|error| LnmaiError { json: error })?;
        Ok((Session::new(self.take_handle()), tokens))
    }

    /// Parse once, keeping source timing for hosts that display the current
    /// Simai fragment alongside the judgment runtime.
    pub fn load_chart_text_native_with_timeline(
        self,
        content: &str,
        level_index: u32,
    ) -> Result<(Session<Loaded>, Vec<ChartTextToken>)> {
        self.load_parsed_native(crate::frontend::parse(content, level_index)?)
    }

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
    pub fn step_native(
        &mut self,
        batch: &types::TimedInputBatch,
    ) -> Result<types::RuntimeStepLightResult> {
        crate::native::step(self.handle(), batch)
    }

    pub fn seek(&mut self, target_us: i64) -> Result<types::SeekResult> {
        self.seek_with_events(target_us, &[], false)
    }

    pub fn seek_with_events(
        &mut self,
        target_us: i64,
        events: &[types::TimedInputEvent],
        keep_held: bool,
    ) -> Result<types::SeekResult> {
        crate::native::seek(self.handle(), target_us, events, keep_held)
    }

    pub fn autoplay_plan(&self) -> Result<types::AutoplayPlan> {
        crate::native::plan(self.handle())
    }

    pub fn chart_spec_native(&self) -> Result<types::ChartSpec> {
        crate::native::chart(self.handle())
    }

    pub fn slide_bindings_native(&self) -> Result<Vec<types::RuntimeSlideBinding>> {
        crate::native::bindings(self.handle())
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_timeline_load_preserves_chart_and_exact_source_clock() {
        let text = "&first=0.125\n&wholebpm=120\n&inote_1=8,E\n&inote_6={4},,(180),,1/2,3h[4:1],A1/B1,4-8[8:1],E\n";
        let original = Session::<Empty>::create_native().unwrap()
            .load_chart_text_native(text, 6).unwrap();
        let (cached, tokens) = Session::<Empty>::create_native().unwrap()
            .load_chart_text_native_with_timeline(text, 6).unwrap();
        assert_eq!(
            original.get_lowered_chart_json().unwrap().json,
            cached.get_lowered_chart_json().unwrap().json,
        );
        assert_eq!(tokens[0], ChartTextToken { time_us: 1_666_667, text: "1".into() });
        assert_eq!(tokens[1], ChartTextToken { time_us: 1_666_667, text: "2".into() });
        assert!(tokens.iter().any(|token| token.text == "3h[4:1]"));
        assert!(tokens.iter().any(|token| token.text == "4-8[8:1]"));
        assert!(tokens.windows(2).all(|pair| pair[0].time_us <= pair[1].time_us));
    }

    #[test]
    fn native_timeline_load_reports_frontend_errors() {
        for (text, level) in [
            ("&inote_1=(120){4}1,E", 9),
            ("&inote_1=(120){4}1w3[4:1],E", 1),
        ] {
            let original = Session::<Empty>::create_native().unwrap()
                .load_chart_text_native(text, level).err().expect("invalid chart");
            let cached = Session::<Empty>::create_native().unwrap()
                .load_chart_text_native_with_timeline(text, level).err().expect("invalid chart");
            assert_eq!(cached.json, original.json);
            let envelope: serde_json::Value = serde_json::from_str(&cached.json).unwrap();
            assert_eq!(envelope["ok"], false);
            assert_eq!(envelope["error"]["code"], "parse_error");
        }
    }
}
