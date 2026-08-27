#[cfg(feature = "ffi")]
pub use lnmai_core_ffi::{session, build};

#[cfg(feature = "rs")]
pub mod session;
