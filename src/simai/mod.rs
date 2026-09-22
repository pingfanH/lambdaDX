//! Vendored pure-Rust Simai parser.
//!
//! Source: `maisimai` (MIT), a native-Rust port of MaiConverter's Simai
//! parsing/exporting (`maiconverter/simai/*.py`). Only the data model and the
//! parser are included here — the exporter is unused by this project.
//!
//! Parses `maidata.txt` metadata and `&inote_N=` chart bodies into
//! [`SimaiFile`] / [`SimaiChart`]. `app/maidata.rs` converts that model into the
//! internal `ChartDoc`.

pub mod model;
pub mod parser;

pub use model::{Bpm, SimaiChart, SimaiFile, SimaiNote, SlidePattern};
pub use parser::{ParseError, parse_chart_text, parse_file};
