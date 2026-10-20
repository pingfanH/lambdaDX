//! SDEZ170 judgment semantics.
//!
//! The module is intentionally independent from the legacy MajdataPlay-shaped
//! judge module so hosts can migrate one call site at a time.

pub mod judge;

pub use judge::{
    adjust_msec, convert_judge, get_judge_score, get_judge_timing, get_slide_judge_timing,
    judge_hold_total, judge_timing_frame, EJudgeType, ETiming, JudgeBox, NoteScoreType,
};
