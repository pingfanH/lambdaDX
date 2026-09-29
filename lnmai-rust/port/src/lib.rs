//! Stub for the external pure-Rust `lnmai-core` port.
//!
//! The real port is a machine-specific path dependency (see `lnmai-rust`'s
//! `Cargo.toml`). When it is absent, this stub keeps the crate graph resolvable;
//! every `ffi` call returns an error envelope, so the engine simply disables
//! itself and the player falls back to Lean / no-core.

pub mod ffi {
    fn unavailable() -> String {
        serde_json::json!({
            "ok": false,
            "error": "lnmai Rust port is not available (stub); build with --features backend-lean",
        })
        .to_string()
    }

    pub fn create_empty_session_handle() -> String {
        unavailable()
    }
    pub fn load_chart_into_session_from_text(
        _handle: u64,
        _content: &str,
        _level_index: u32,
    ) -> String {
        unavailable()
    }
    pub fn load_chart_into_session_from_json(_handle: u64, _chart_spec_json: &str) -> String {
        unavailable()
    }
    pub fn free_game_state_handle(_handle: u64) -> String {
        unavailable()
    }
    pub fn get_lowered_chart_json_by_handle(_handle: u64) -> String {
        unavailable()
    }
    pub fn step_game_state_handle_light(_handle: u64, _batch_json: &str) -> String {
        unavailable()
    }
    pub fn step_game_state_handle(_handle: u64, _batch_json: &str) -> String {
        unavailable()
    }
    pub fn get_game_state_json_by_handle(_handle: u64) -> String {
        unavailable()
    }
    pub fn unload_chart_from_session(_handle: u64) -> String {
        unavailable()
    }
    pub fn default_tactic_from_chart_json(_chart_json: &str) -> String {
        unavailable()
    }
}
