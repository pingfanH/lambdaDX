//! Thin Rust wrapper over the pure-Rust `lnmai-core` port.
//!
//! It preserves the JSON schema/API of the previous Lean FFI backend (the
//! `shared/rust_ffi_*` files) but calls the in-process Rust core instead of a
//! linked Lean runtime, so no Nix/Lake artifacts are needed to build.

pub mod api;
pub mod session;
pub mod types;
