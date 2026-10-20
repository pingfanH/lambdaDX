//! SDEZ170 judgment semantics.
//!
//! The module is intentionally independent from the legacy MajdataPlay-shaped
//! judge module so hosts can migrate one call site at a time.

pub mod input;
pub mod judge;
pub mod note;
pub mod runtime;
pub mod slide;
pub mod slide_data;
pub mod timing;

pub use input::{
    InputFrame, PanelLog, A1, A2, A3, A8, B1, B8, C1, C2, D1, D8, E1, E8, PANEL_AREA_COUNT,
};

pub use note::{
    AutoplayMode, HoldInput, HoldNote, HoldStep, JudgedNote, TapNote, TouchArea, TouchNote,
};

pub use slide::SlideNote;

pub use runtime::{SdezEvent, SdezNoteKind, SdezPhase, SdezRuntime};

pub use slide_data::{
    hit_areas, initial_last_wait_time, slide_type_length, total_distance, SlideHitArea,
    LENGTH_COUNT, ROTATION_COUNT, SHAPE_COUNT,
};

pub use judge::{
    adjust_msec, convert_judge, get_judge_score, get_judge_timing, get_slide_judge_timing,
    judge_hold_total, judge_timing_frame, EJudgeType, ETiming, JudgeBox, NoteScoreType,
};

pub use timing::{
    auto_play_trigger_msec, is_note_check_time_start, is_too_late, judge_slide, judge_tap_like,
    TimingSettings, AUTO_PLAY_LEAD_MSEC, NORMAL_TIMING,
};
