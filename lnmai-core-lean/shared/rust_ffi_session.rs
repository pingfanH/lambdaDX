use std::ffi::{CStr, CString};
use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::OnceLock;
use std::time::Instant;

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

/// Nanosecond breakdown of one `advance_frame_light` call across the FFI
/// boundary. `ffi_ns` is the raw Lean runtime step; the other two fields are
/// the Rust-side JSON string bridge around it.
#[derive(Debug, Clone, Copy, Default)]
pub struct FfiStepTimings {
    pub mk_string_ns: u64,
    pub ffi_ns: u64,
    pub into_string_ns: u64,
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
        self.handle
            .expect("session handle has already been consumed")
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
            let _ = crate::native::free(handle);
        }
    }
}

fn ffi_error(message: impl Into<String>) -> LnmaiError {
    LnmaiError {
        json: message.into(),
    }
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
        let value = CStr::from_ptr(ptr as *const i8)
            .to_string_lossy()
            .into_owned();
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

struct RuntimeThread {
    initialized: Cell<bool>,
    owned: Cell<bool>,
}

impl Drop for RuntimeThread {
    fn drop(&mut self) {
        if self.owned.get() {
            unsafe { lean_sys::lean_finalize_thread() };
        }
    }
}

thread_local! {
    static RUNTIME_THREAD: RuntimeThread = const { RuntimeThread {
        initialized: Cell::new(false), owned: Cell::new(false),
    } };
}

/// Initialize Lean once per process and attach every calling thread.
pub fn ensure_runtime() -> std::result::Result<(), ()> {
    static INIT: OnceLock<std::result::Result<(), ()>> = OnceLock::new();
    (*INIT.get_or_init(|| {
        let result = unsafe { raw::initialize_lnmai_runtime() };
        if result.is_ok() {
            // lean_initialize already initialized the calling thread.
            RUNTIME_THREAD.with(|thread| thread.initialized.set(true));
        }
        result
    }))?;
    RUNTIME_THREAD.with(|thread| {
        if !thread.initialized.get() {
            unsafe { lean_sys::lean_initialize_thread() };
            thread.initialized.set(true);
            thread.owned.set(true);
        }
    });
    Ok(())
}

/// Initialize the embedded Lean runtime without the process-wide guard.
///
/// Prefer [`ensure_runtime`] for application code. This remains available for
/// low-level callers that own the runtime lifecycle themselves.
pub unsafe fn initialize_runtime() -> std::result::Result<(), ()> {
    unsafe { raw::initialize_lnmai_runtime() }
}

impl Session<Empty> {
    /// Create a session using ABI 3, without JSON transport.
    pub fn create_native() -> Result<Self> {
        ensure_runtime().map_err(|_| ffi_error("failed to initialize Lean runtime"))?;
        Ok(Self::new(crate::native::create()?))
    }

    pub fn load_chart_text_native(
        mut self,
        content: &str,
        level_index: u32,
    ) -> Result<Session<Loaded>> {
        crate::native::load(self.handle(), content, level_index)?;
        let handle = self.take_handle();
        Ok(Session::new(handle))
    }
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

    pub fn load_chart_text(
        mut self,
        content: &str,
        level_index: u32,
    ) -> Result<(Session<Loaded>, FfiEnvelope)> {
        let handle = self.handle();
        let content_obj = mk_lean_string(content)?;
        // Exported Lean functions consume owned Lean object arguments.
        let result = unsafe {
            raw::lnmai_load_chart_into_session_from_text(handle, content_obj, level_index)
        };
        let json = into_string(result)?;
        let envelope = ok_or_error(json)?;
        let handle = self.take_handle();
        Ok((Session::new(handle), envelope))
    }

    pub fn load_chart_json(
        mut self,
        chart_spec_json: &str,
    ) -> Result<(Session<Loaded>, FfiEnvelope)> {
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
    pub fn step_native(
        &mut self,
        batch: &types::TimedInputBatch,
    ) -> Result<types::RuntimeStepLightResult> {
        crate::native::step(self.handle(), batch)
    }

    pub fn seek(&mut self, target_us: i64) -> Result<types::SeekResult> {
        self.seek_with_events(target_us, &[], false)
    }

    /// Restore the chart runtime at `target_us` using a sorted event stream.
    /// Events at the target stay pending; `keep_held` preserves ongoing autoplay
    /// inputs. An empty stream uses the core's default autoplay plan.
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
        let json =
            unsafe { into_string(raw::lnmai_get_lowered_chart_json_by_handle(self.handle()))? };
        ok_or_error(json)
    }

    pub fn advance_frame_light(&mut self, batch_json: &str) -> Result<FfiEnvelope> {
        let (envelope, _timings) = self.advance_frame_light_timed(batch_json)?;
        Ok(envelope)
    }

    /// Like [`advance_frame_light`], but also reports where the frame time went
    /// inside the FFI bridge: building the Lean input string, the raw Lean step
    /// call itself, and copying the result JSON back into Rust.
    pub fn advance_frame_light_timed(
        &mut self,
        batch_json: &str,
    ) -> Result<(FfiEnvelope, FfiStepTimings)> {
        let t0 = Instant::now();
        let batch_obj = mk_lean_string(batch_json)?;
        let t1 = Instant::now();
        // Exported Lean functions consume owned Lean object arguments.
        let result = unsafe { raw::lnmai_step_game_state_handle_light(self.handle(), batch_obj) };
        let t2 = Instant::now();
        let json = into_string(result)?;
        let t3 = Instant::now();
        let envelope = ok_or_error(json)?;
        let timings = FfiStepTimings {
            mk_string_ns: (t1 - t0).as_nanos() as u64,
            ffi_ns: (t2 - t1).as_nanos() as u64,
            into_string_ns: (t3 - t2).as_nanos() as u64,
        };
        Ok((envelope, timings))
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
