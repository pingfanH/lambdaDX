//! Verification-facing subset of lnmai-core.
//!
//! These finite tables and bounded arithmetic operations are extracted by
//! Charon/Aeneas and related to Lean under explicit preconditions. The runtime
//! shares area/conversion code and calls judgment tables through exact-number
//! adapters; unbounded time and scoring arithmetic live in `lnmai_core`.

pub mod areas;
pub mod big_int;
pub mod big_nat;
pub mod big_rat;
pub mod connected_slide_lifecycle;
pub mod connected_slide_metadata;
pub mod constants;
pub mod convert;
pub mod frame_commands;
pub mod grade_counts;
pub mod judge;
pub mod judgment_adapter;
pub mod lifecycle;
pub mod maidata_line_adapter;
pub mod maidata_scan;
pub mod natural_parser;
pub mod prefix_directives;
pub mod queue_maintenance;
pub mod queue_remaining;
pub mod scheduler;
pub mod score;
pub mod shape;
pub mod slide_chain;
pub mod slide_lifecycle;
pub mod slide_parents;
pub mod slide_parser;
pub mod slide_parser_frontend;
pub mod slide_parser_sequence;
pub mod slide_parser_text;
pub mod slide_queue;
pub mod slide_queue_metrics;
pub mod slide_schedule;
pub mod slide_tables;
pub mod storage;
pub mod symmetry;
pub mod tap_schedule;
pub mod time;
pub mod timing_aggregate;
pub mod timing_number;
pub mod timing_parser;
pub mod tokenize;
pub mod touch_schedule;
pub mod types;
pub mod vec_erase;
pub mod vec_prepend;

pub mod hold_schedule;

pub mod normalization;
pub mod touch_automatic;
pub mod touch_hold_body;
pub mod touch_hold_body_adapter;
pub mod touch_hold_rebuild;
pub mod touch_hold_schedule;
