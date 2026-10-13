//! `backend-none` stub: just enough of the core's type/session API for the
//! player to compile without any gameplay core. No judging happens.

use serde::{Deserialize, Serialize};

pub mod types {
    include!("../../lnmai-core/shared/rust_ffi_types.rs");
}

pub mod session {
    /// Placeholder error type.
    #[derive(Debug)]
    pub struct LnmaiError {
        pub json: String,
    }

    /// The core is disabled; there is nothing to initialise.
    pub fn ensure_runtime() -> Result<(), LnmaiError> {
        Ok(())
    }
}

pub mod api {}
