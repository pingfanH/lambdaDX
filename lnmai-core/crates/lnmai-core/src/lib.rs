#![allow(clippy::result_large_err)]

//! `lnmai-core` — Rust port of the Lean `LnmaiCore` gameplay core.
//!
//! The runtime uses arbitrary-precision integers, naturals and rational values
//! to preserve Lean's numerical semantics. Areas and conversions are shared
//! with `lnmai_core_verify`; judgment adapters select bounded representatives
//! for its finite tables. `verification/` proves the extracted kernels under
//! explicit arithmetic preconditions. The parser, runtime, adapters and FFI
//! are also compared against Lean by the differential suite; those checks do
//! not constitute a proof of whole-program equivalence.

pub use lnmai_core_verify::convert;
pub mod areas;
pub mod constants;
pub mod integer;
pub mod judge;
pub mod natural;
pub mod types;

mod accuracy;
pub mod score;

pub mod cabi;
pub mod chart_loader;
pub mod default_tactic;
pub mod direct;
pub mod events;
pub mod ffi;
pub mod input_model;
pub mod json_decode;
pub mod json_encode;
mod lean_json;
pub mod lifecycle;
pub mod native;
pub mod rat;
pub mod runtime_score;
pub mod runtime_seek;
pub mod sdez;
pub mod scheduler;
pub mod simai;
pub mod storage;
pub mod symmetry;
pub mod time;
