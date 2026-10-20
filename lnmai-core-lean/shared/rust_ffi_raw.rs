use lean_sys::{lean_dec_ref, lean_io_result_is_error, lean_object};
use std::os::raw::c_char;

unsafe extern "C" {
    pub fn lean_initialize();

    pub fn lean_io_result_show_error(result: *mut lean_object);
    pub fn lean_mk_string(s: *const c_char) -> *mut lean_object;

    pub fn initialize_lnmai_x2dcore_LnmaiCore(builtin: u8) -> *mut lean_object;
    pub fn initialize_lnmai_x2dcore_LnmaiCore_FFI(builtin: u8) -> *mut lean_object;

    pub fn lnmai_ffi_version_json() -> *mut lean_object;

    pub fn lnmai_parse_frontend_chart_json(content: *mut lean_object, level_index: u32) -> *mut lean_object;
    pub fn lnmai_parse_frontend_semantic_chart_json(content: *mut lean_object, level_index: u32) -> *mut lean_object;
    pub fn lnmai_parse_frontend_inspection_chart_json(content: *mut lean_object, level_index: u32) -> *mut lean_object;
    pub fn lnmai_parse_normalized_chart_json(content: *mut lean_object, level_index: u32) -> *mut lean_object;
    pub fn lnmai_parse_lowered_chart_json(content: *mut lean_object, level_index: u32) -> *mut lean_object;

    pub fn lnmai_build_game_state_json(chart_spec_json: *mut lean_object) -> *mut lean_object;
    pub fn lnmai_default_tactic_from_chart_json(chart_spec_json: *mut lean_object) -> *mut lean_object;
    pub fn lnmai_step_game_state_json(state_json: *mut lean_object, batch_json: *mut lean_object) -> *mut lean_object;

    pub fn lnmai_create_empty_session_handle() -> *mut lean_object;
    pub fn lnmai_load_chart_into_session_from_text(handle: u64, content: *mut lean_object, level_index: u32) -> *mut lean_object;
    pub fn lnmai_load_chart_into_session_from_json(handle: u64, chart_spec_json: *mut lean_object) -> *mut lean_object;
    pub fn lnmai_unload_chart_from_session(handle: u64) -> *mut lean_object;
    pub fn lnmai_get_lowered_chart_json_by_handle(handle: u64) -> *mut lean_object;

    pub fn lnmai_create_game_state_handle(chart_spec_json: *mut lean_object) -> *mut lean_object;
    pub fn lnmai_free_game_state_handle(handle: u64) -> *mut lean_object;
    pub fn lnmai_get_game_state_json_by_handle(handle: u64) -> *mut lean_object;
    pub fn lnmai_step_game_state_handle(handle: u64, batch_json: *mut lean_object) -> *mut lean_object;
    pub fn lnmai_step_game_state_handle_light(handle: u64, batch_json: *mut lean_object) -> *mut lean_object;
}

pub unsafe fn initialize_lnmai_runtime() -> Result<(), ()> {
    unsafe { lean_initialize() };
    let core_result = unsafe { initialize_lnmai_x2dcore_LnmaiCore(1) };
    if unsafe { lean_io_result_is_error(core_result) } {
        unsafe { lean_io_result_show_error(core_result) };
        unsafe { lean_dec_ref(core_result) };
        return Err(());
    }
    unsafe { lean_dec_ref(core_result) };

    let ffi_result = unsafe { initialize_lnmai_x2dcore_LnmaiCore_FFI(1) };
    if unsafe { lean_io_result_is_error(ffi_result) } {
        unsafe { lean_io_result_show_error(ffi_result) };
        unsafe { lean_dec_ref(ffi_result) };
        Err(())
    } else {
        unsafe { lean_dec_ref(ffi_result) };
        Ok(())
    }
}
