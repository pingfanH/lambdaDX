//! Simai parsing pipeline.
//!
//! Mirrors the `LnmaiCore.Simai.*` modules:
//!
//! `syntax` → `timing` → `shape` → `slide_tables` → `slide_parser`
//! → `tokenize` → `ir` → `normalize` → `frontend`.

pub mod dsl;
pub mod frontend;
pub mod ir;
pub mod maidata;
pub mod normalize;
pub mod shape;
pub mod slide_parser;
pub mod slide_tables;
pub mod syntax;
pub mod timing;
pub mod tokenize;
pub mod typecheck;
