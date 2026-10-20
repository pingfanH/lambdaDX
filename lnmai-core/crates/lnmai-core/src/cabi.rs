//! C ABI over the JSON FFI (`ffi.rs`).
//!
//! The Lean implementation exports functions taking Lean `String`/`IO String`
//! objects; this Rust port exposes a plain C-string ABI instead. Every function
//! returns a heap-allocated, NUL-terminated JSON envelope that the caller must
//! release with [`lnmai_string_free`].
//!
//! Symbol names intentionally mirror `LnmaiCore/FFI.lean` so hosts can be
//! ported by swapping the string marshalling.

use std::ffi::{c_char, CStr, CString};

use crate::ffi;

fn to_c(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => CString::new("{\"ok\":false,\"error\":{\"code\":\"nul_byte\",\"message\":\"response contained NUL\"}}")
            .unwrap()
            .into_raw(),
    }
}

/// # Safety
/// `ptr` must be null or readable through a terminating NUL for this call.
unsafe fn from_c(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Free a string returned by any `lnmai_*` function.
///
/// # Safety
/// `ptr` must be null or a pointer previously returned by this library, and not
/// already freed.
#[no_mangle]
pub unsafe extern "C" fn lnmai_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

#[no_mangle]
pub extern "C" fn lnmai_abi_version() -> u64 {
    ffi::FFI_ABI_VERSION
}

#[no_mangle]
pub extern "C" fn lnmai_ffi_version_json() -> *mut c_char {
    to_c(ffi::ffi_version_json())
}

/// # Safety
/// `content` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_lowered_chart_json(
    content: *const c_char,
    level_index: u32,
) -> *mut c_char {
    to_c(ffi::parse_lowered_chart_json(
        &unsafe { from_c(content) },
        level_index,
    ))
}

/// # Safety
/// `content` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_normalized_chart_json(
    content: *const c_char,
    level_index: u32,
) -> *mut c_char {
    to_c(ffi::parse_normalized_chart_json(
        &unsafe { from_c(content) },
        level_index,
    ))
}

/// # Safety
/// `content` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_frontend_semantic_chart_json(
    content: *const c_char,
    level_index: u32,
) -> *mut c_char {
    to_c(ffi::parse_frontend_semantic_chart_json(
        &unsafe { from_c(content) },
        level_index,
    ))
}

#[no_mangle]
pub extern "C" fn lnmai_create_empty_session_handle() -> *mut c_char {
    to_c(ffi::create_empty_session_handle())
}

/// # Safety
/// `chart_spec_json` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_default_tactic_from_chart_json(
    chart_spec_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::default_tactic_from_chart_json(&unsafe {
        from_c(chart_spec_json)
    }))
}

/// # Safety
/// `content` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_load_chart_into_session_from_text(
    handle: u64,
    content: *const c_char,
    level_index: u32,
) -> *mut c_char {
    to_c(ffi::load_chart_into_session_from_text(
        handle,
        &unsafe { from_c(content) },
        level_index,
    ))
}

/// # Safety
/// `chart_spec_json` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_load_chart_into_session_from_json(
    handle: u64,
    chart_spec_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::load_chart_into_session_from_json(handle, &unsafe {
        from_c(chart_spec_json)
    }))
}

#[no_mangle]
pub extern "C" fn lnmai_unload_chart_from_session(handle: u64) -> *mut c_char {
    to_c(ffi::unload_chart_from_session(handle))
}

#[no_mangle]
pub extern "C" fn lnmai_get_lowered_chart_json_by_handle(handle: u64) -> *mut c_char {
    to_c(ffi::get_lowered_chart_json_by_handle(handle))
}

#[no_mangle]
pub extern "C" fn lnmai_free_game_state_handle(handle: u64) -> *mut c_char {
    to_c(ffi::free_game_state_handle(handle))
}

/// # Safety
/// `batch_json` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_step_game_state_handle_light(
    handle: u64,
    batch_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::step_game_state_handle_light(handle, &unsafe {
        from_c(batch_json)
    }))
}

/// # Safety
/// `batch_json` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn lnmai_step_game_state_handle(
    handle: u64,
    batch_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::step_game_state_handle(handle, &unsafe {
        from_c(batch_json)
    }))
}

#[no_mangle]
pub extern "C" fn lnmai_get_game_state_json_by_handle(handle: u64) -> *mut c_char {
    to_c(ffi::get_game_state_json_by_handle(handle))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_frontend_chart_json(
    content: *const c_char,
    level_index: u32,
) -> *mut c_char {
    to_c(ffi::parse_frontend_chart_json(
        &unsafe { from_c(content) },
        level_index,
    ))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_frontend_chart_mode_json(
    content: *const c_char,
    level_index: u32,
    is_classic: u8,
) -> *mut c_char {
    to_c(ffi::parse_frontend_chart_mode_json(
        &unsafe { from_c(content) },
        level_index,
        is_classic != 0,
    ))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_frontend_inspection_chart_json(
    content: *const c_char,
    level_index: u32,
) -> *mut c_char {
    to_c(ffi::parse_frontend_inspection_chart_json(
        &unsafe { from_c(content) },
        level_index,
    ))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_parse_lowered_chart_mode_json(
    content: *const c_char,
    level_index: u32,
    is_classic: u8,
) -> *mut c_char {
    to_c(ffi::parse_lowered_chart_mode_json(
        &unsafe { from_c(content) },
        level_index,
        is_classic != 0,
    ))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_build_game_state_json(
    chart_spec_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::build_game_state_json(&unsafe {
        from_c(chart_spec_json)
    }))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_step_game_state_json(
    state_json: *const c_char,
    batch_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::step_game_state_json(
        &unsafe { from_c(state_json) },
        &unsafe { from_c(batch_json) },
    ))
}

/// # Safety
/// Input strings must be null or readable through a terminating NUL for this call.
#[no_mangle]
pub unsafe extern "C" fn lnmai_create_game_state_handle(
    chart_spec_json: *const c_char,
) -> *mut c_char {
    to_c(ffi::create_game_state_handle(&unsafe {
        from_c(chart_spec_json)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabi_roundtrip() {
        unsafe {
            let created = lnmai_create_empty_session_handle();
            let created_str = CStr::from_ptr(created).to_str().unwrap().to_string();
            lnmai_string_free(created);
            let v: serde_json::Value = serde_json::from_str(&created_str).unwrap();
            let handle = v["result"]["handle"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();

            let content = CString::new("&first=0\n&inote_1=\n(120)\n1,\n").unwrap();
            let loaded = lnmai_load_chart_into_session_from_text(handle, content.as_ptr(), 1);
            let loaded_str = CStr::from_ptr(loaded).to_str().unwrap().to_string();
            lnmai_string_free(loaded);
            let lv: serde_json::Value = serde_json::from_str(&loaded_str).unwrap();
            assert_eq!(lv["result"]["state"], "loaded");

            let batch = CString::new(
                "{\"currentTime\":0,\"events\":[{\"buttonClick\":{\"tp\":0,\"zone\":\"K1\"}}]}",
            )
            .unwrap();
            let stepped = lnmai_step_game_state_handle_light(handle, batch.as_ptr());
            let stepped_str = CStr::from_ptr(stepped).to_str().unwrap().to_string();
            lnmai_string_free(stepped);
            let sv: serde_json::Value = serde_json::from_str(&stepped_str).unwrap();
            assert_eq!(sv["result"]["events"][0]["grade"], "Perfect");

            let freed = lnmai_free_game_state_handle(handle);
            lnmai_string_free(freed);
        }
    }

    #[test]
    fn cabi_default_tactic() {
        unsafe {
            let content = CString::new("&first=0\n&inote_1=\n(120)\n1,\n").unwrap();
            let lowered = lnmai_parse_lowered_chart_json(content.as_ptr(), 1);
            let envelope: serde_json::Value =
                serde_json::from_str(CStr::from_ptr(lowered).to_str().unwrap()).unwrap();
            let spec = CString::new(envelope["result"].to_string()).unwrap();
            let tactic = lnmai_default_tactic_from_chart_json(spec.as_ptr());
            let tactic_str = CStr::from_ptr(tactic).to_str().unwrap().to_string();
            lnmai_string_free(lowered);
            lnmai_string_free(tactic);
            let v: serde_json::Value = serde_json::from_str(&tactic_str).unwrap();
            assert_eq!(v["result"]["events"][0]["buttonClick"]["zone"], "K1");
            assert_eq!(v["result"]["events"][0]["buttonClick"]["tp"], 0);
        }
    }
}
