//! Gameplay-core backend facade.
//!
//! Exactly one backend is selected by the `backend-*` Cargo features (see the
//! root `Cargo.toml`):
//!
//! * `backend-rust` (default) — the pure-Rust port shim in `lnmai-rust/`
//!   (crate `lnmai-core-rust`); leaves the vendored `lnmai-core/` untouched.
//! * `backend-lean` — the vendored, **unmodified** `lnmai-core/` Lean FFI.
//! * `backend-none` — no gameplay core; [`crate::player::engine`] is stubbed.

#[cfg(feature = "backend-rust")]
pub use lnmai_core_rust::{api, session, types};

#[cfg(all(feature = "backend-lean", not(feature = "backend-rust")))]
pub use lnmai_core::{api, session, types};

// `backend-none` (or `--no-default-features` with no backend feature): stub.
#[cfg(not(any(feature = "backend-rust", feature = "backend-lean")))]
mod none;
#[cfg(not(any(feature = "backend-rust", feature = "backend-lean")))]
pub use none::{api, session, types};
