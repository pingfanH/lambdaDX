//! In-process frontend data. No JSON or fixed-width FFI conversion is needed.

pub use lnmai_core_rs::simai::ir::FrontendChartResult;
pub use lnmai_core_rs::simai::syntax::{RawNoteKind, RawNoteToken, SourceEvent};
pub use lnmai_core_rs::{areas::{OuterSlot, SensorArea}, rat::Rat as Rational};
pub use lnmai_core_rs::simai::syntax::SlideBodyKind;
pub use lnmai_core_rs::{natural::Natural, time::{Duration, TimePoint}};

use crate::session::{LnmaiError, Result};

pub fn parse(content: &str, level_index: u32) -> Result<FrontendChartResult> {
    lnmai_core_rs::simai::frontend::parse_frontend_chart_result_chart_clock(
        content, (level_index as usize).into(),
    ).map_err(|error| LnmaiError {
        json: lnmai_core_rs::ffi::parse_error_json(&error),
    })
}
