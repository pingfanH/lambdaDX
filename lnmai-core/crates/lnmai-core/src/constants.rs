//! Runtime constants embedded from the extracted core.
//!
//! All timing constants in microseconds

use super::time::Duration;
use lnmai_core_verify::constants as kernel;

/// Frame length (60fps = 16667μs)
pub const FRAME_LENGTH: Duration = Duration::from_const_micros(kernel::FRAME_LENGTH.micros);

// Tap judgment windows
pub const TAP_PERFECT_1ST: Duration = Duration::from_const_micros(kernel::TAP_PERFECT_1ST.micros);
pub const TAP_PERFECT_2ND: Duration = Duration::from_const_micros(kernel::TAP_PERFECT_2ND.micros);
pub const TAP_PERFECT_3RD: Duration = Duration::from_const_micros(kernel::TAP_PERFECT_3RD.micros);
pub const TAP_GREAT_1ST: Duration = Duration::from_const_micros(kernel::TAP_GREAT_1ST.micros);
pub const TAP_GREAT_2ND: Duration = Duration::from_const_micros(kernel::TAP_GREAT_2ND.micros);
pub const TAP_GREAT_3RD: Duration = Duration::from_const_micros(kernel::TAP_GREAT_3RD.micros);
pub const TAP_GOOD: Duration = Duration::from_const_micros(kernel::TAP_GOOD.micros);

// Touch judgment windows
pub const TOUCH_PERFECT_1ST: Duration =
    Duration::from_const_micros(kernel::TOUCH_PERFECT_1ST.micros);
pub const TOUCH_PERFECT_2ND: Duration =
    Duration::from_const_micros(kernel::TOUCH_PERFECT_2ND.micros);
pub const TOUCH_PERFECT_3RD: Duration =
    Duration::from_const_micros(kernel::TOUCH_PERFECT_3RD.micros);
pub const TOUCH_GREAT_1ST: Duration = Duration::from_const_micros(kernel::TOUCH_GREAT_1ST.micros);
pub const TOUCH_GREAT_2ND: Duration = Duration::from_const_micros(kernel::TOUCH_GREAT_2ND.micros);
pub const TOUCH_GREAT_3RD: Duration = Duration::from_const_micros(kernel::TOUCH_GREAT_3RD.micros);
pub const TOUCH_GOOD: Duration = Duration::from_const_micros(kernel::TOUCH_GOOD.micros);

// Slide judgment windows
pub const SLIDE_PERFECT_3RD: Duration =
    Duration::from_const_micros(kernel::SLIDE_PERFECT_3RD.micros);
pub const SLIDE_GREAT_1ST: Duration = Duration::from_const_micros(kernel::SLIDE_GREAT_1ST.micros);
pub const SLIDE_GREAT_2ND: Duration = Duration::from_const_micros(kernel::SLIDE_GREAT_2ND.micros);
pub const SLIDE_GREAT_3RD: Duration = Duration::from_const_micros(kernel::SLIDE_GREAT_3RD.micros);
pub const SLIDE_GOOD: Duration = Duration::from_const_micros(kernel::SLIDE_GOOD.micros);
pub const SLIDE_MAX_EXT: Duration = Duration::from_const_micros(kernel::SLIDE_MAX_EXT.micros);

// Hold constants
pub const HOLD_HEAD_IGNORE: Duration = Duration::from_const_micros(kernel::HOLD_HEAD_IGNORE.micros);
pub const HOLD_TAIL_IGNORE: Duration = Duration::from_const_micros(kernel::HOLD_TAIL_IGNORE.micros);
pub const TOUCH_HOLD_HEAD_IGNORE: Duration =
    Duration::from_const_micros(kernel::TOUCH_HOLD_HEAD_IGNORE.micros);
pub const TOUCH_HOLD_TAIL_IGNORE: Duration =
    Duration::from_const_micros(kernel::TOUCH_HOLD_TAIL_IGNORE.micros);
pub const DELUXE_HOLD_RELEASE_IGNORE_TIME: Duration =
    Duration::from_const_micros(kernel::DELUXE_HOLD_RELEASE_IGNORE_TIME.micros);
pub const CLASSIC_HOLD_ALLOW_OVER_LENGTH: Duration =
    Duration::from_const_micros(kernel::CLASSIC_HOLD_ALLOW_OVER_LENGTH.micros);

// Classic hold end windows (9 frames / 12 frames)
pub const HOLD_CLASSIC_END_JUDGE_PERFECT_FAST: Duration =
    Duration::from_const_micros(kernel::HOLD_CLASSIC_END_JUDGE_PERFECT_FAST.micros);
pub const HOLD_CLASSIC_END_JUDGE_PERFECT_LATE: Duration =
    Duration::from_const_micros(kernel::HOLD_CLASSIC_END_JUDGE_PERFECT_LATE.micros);

// Late clear wait
pub const SLIDE_JUDGED_LATE_CLEAR_WAIT: Duration =
    Duration::from_const_micros(kernel::SLIDE_JUDGED_LATE_CLEAR_WAIT.micros);

// Judgable range
pub const JUDGABLE_RANGE: Duration = Duration::from_const_micros(kernel::JUDGABLE_RANGE.micros);
pub const TOUCH_JUDGABLE_RANGE_LATE_EXTRA: Duration =
    Duration::from_const_micros(kernel::TOUCH_JUDGABLE_RANGE_LATE_EXTRA.micros);

// Counts
pub use kernel::{BUTTON_ZONE_COUNT, SENSOR_AREA_COUNT, SUBDIVIDE_SLIDE_JUDGE_GRADE};

pub const JUDGE_OFFSET: Duration = Duration::zero();
pub const TOUCH_PANEL_OFFSET: Duration = Duration::zero();

// The public classic-slide windows use the same exact 60 Hz frame length.
pub const SLIDE_JUDGE_CLASSIC_FAST_SEG_1ST_PERFECT_MSEC: Duration =
    Duration::from_const_micros(kernel::FRAME_LENGTH.micros * 4);
pub const SLIDE_JUDGE_CLASSIC_FAST_SEG_2ND_PERFECT_MSEC: Duration =
    Duration::from_const_micros(kernel::FRAME_LENGTH.micros * 8);
pub const SLIDE_JUDGE_CLASSIC_FAST_SEG_3RD_PERFECT_MSEC: Duration =
    Duration::from_const_micros(kernel::FRAME_LENGTH.micros * 12);
pub const SLIDE_JUDGE_CLASSIC_FAST_SEG_1ST_GREAT_MSEC: Duration =
    Duration::from_const_micros(kernel::FRAME_LENGTH.micros * 16);
pub const SLIDE_JUDGE_CLASSIC_FAST_SEG_2ND_GREAT_MSEC: Duration =
    Duration::from_const_micros(kernel::FRAME_LENGTH.micros * 20);
pub const SLIDE_JUDGE_CLASSIC_FAST_SEG_3RD_GREAT_MSEC: Duration =
    Duration::from_const_micros(kernel::FRAME_LENGTH.micros * 24);
pub const SLIDE_JUDGE_CLASSIC_LATE_SEG_1ST_PERFECT_MSEC: Duration =
    SLIDE_JUDGE_CLASSIC_FAST_SEG_1ST_PERFECT_MSEC;
pub const SLIDE_JUDGE_CLASSIC_LATE_SEG_2ND_PERFECT_MSEC: Duration =
    SLIDE_JUDGE_CLASSIC_FAST_SEG_2ND_PERFECT_MSEC;
pub const SLIDE_JUDGE_CLASSIC_LATE_SEG_3RD_PERFECT_MSEC: Duration =
    SLIDE_JUDGE_CLASSIC_FAST_SEG_3RD_PERFECT_MSEC;
pub const SLIDE_JUDGE_CLASSIC_LATE_SEG_1ST_GREAT_MSEC: Duration =
    SLIDE_JUDGE_CLASSIC_FAST_SEG_1ST_GREAT_MSEC;
pub const SLIDE_JUDGE_CLASSIC_LATE_SEG_2ND_GREAT_MSEC: Duration =
    SLIDE_JUDGE_CLASSIC_FAST_SEG_2ND_GREAT_MSEC;
pub const SLIDE_JUDGE_CLASSIC_LATE_SEG_3RD_GREAT_MSEC: Duration =
    SLIDE_JUDGE_CLASSIC_FAST_SEG_3RD_GREAT_MSEC;

// Public names from `LnmaiCore.Constants`, including its convenience aliases.
pub use self::{
    CLASSIC_HOLD_ALLOW_OVER_LENGTH as CLASSIC_HOLD_ALLOW_OVER_LENGTH_SEC,
    DELUXE_HOLD_RELEASE_IGNORE_TIME as DELUXE_HOLD_RELEASE_IGNORE_TIME_SEC,
    FRAME_LENGTH as FRAME_LENGTH_MSEC,
    HOLD_CLASSIC_END_JUDGE_PERFECT_FAST as HOLD_CLASSIC_END_JUDGE_PERFECT_FAST_MSEC,
    HOLD_CLASSIC_END_JUDGE_PERFECT_LATE as HOLD_CLASSIC_END_JUDGE_PERFECT_LATE_MSEC,
    HOLD_HEAD_IGNORE as HOLD_HEAD_IGNORE_LENGTH_SEC, HOLD_HEAD_IGNORE as HOLD_HEAD_IGNORE_MS,
    HOLD_TAIL_IGNORE as HOLD_TAIL_IGNORE_LENGTH_SEC, HOLD_TAIL_IGNORE as HOLD_TAIL_IGNORE_MS,
    JUDGABLE_RANGE as JUDGABLE_RANGE_SEC, SLIDE_GOOD as SLIDE_JUDGE_GOOD_AREA_MSEC,
    SLIDE_GREAT_1ST as SLIDE_JUDGE_SEG_1ST_GREAT_MSEC,
    SLIDE_GREAT_2ND as SLIDE_JUDGE_SEG_2ND_GREAT_MSEC,
    SLIDE_GREAT_3RD as SLIDE_JUDGE_SEG_3RD_GREAT_MSEC,
    SLIDE_MAX_EXT as SLIDE_JUDGE_MAXIMUM_ALLOWED_EXT_LENGTH_MSEC,
    SLIDE_PERFECT_3RD as SLIDE_JUDGE_SEG_BASE_3RD_PERFECT_MSEC, TAP_GOOD as TAP_GOOD_MS,
    TAP_GOOD as TAP_JUDGE_GOOD_AREA_MSEC, TAP_GREAT_1ST as TAP_GREAT_1_MS,
    TAP_GREAT_1ST as TAP_JUDGE_SEG_1ST_GREAT_MSEC, TAP_GREAT_2ND as TAP_GREAT_2_MS,
    TAP_GREAT_2ND as TAP_JUDGE_SEG_2ND_GREAT_MSEC, TAP_GREAT_3RD as TAP_GREAT_3_MS,
    TAP_GREAT_3RD as TAP_JUDGE_SEG_3RD_GREAT_MSEC,
    TAP_PERFECT_1ST as TAP_JUDGE_SEG_1ST_PERFECT_MSEC, TAP_PERFECT_1ST as TAP_PERFECT_1_MS,
    TAP_PERFECT_2ND as TAP_JUDGE_SEG_2ND_PERFECT_MSEC, TAP_PERFECT_2ND as TAP_PERFECT_2_MS,
    TAP_PERFECT_3RD as TAP_JUDGE_SEG_3RD_PERFECT_MSEC, TAP_PERFECT_3RD as TAP_PERFECT_3_MS,
    TOUCH_GOOD as TOUCH_GOOD_MS, TOUCH_GOOD as TOUCH_JUDGE_GOOD_AREA_MSEC,
    TOUCH_GREAT_1ST as TOUCH_GREAT_1_MS, TOUCH_GREAT_1ST as TOUCH_JUDGE_SEG_1ST_GREAT_MSEC,
    TOUCH_GREAT_2ND as TOUCH_GREAT_2_MS, TOUCH_GREAT_2ND as TOUCH_JUDGE_SEG_2ND_GREAT_MSEC,
    TOUCH_GREAT_3RD as TOUCH_GREAT_3_MS, TOUCH_GREAT_3RD as TOUCH_JUDGE_SEG_3RD_GREAT_MSEC,
    TOUCH_HOLD_HEAD_IGNORE as TOUCH_HOLD_HEAD_IGNORE_LENGTH_SEC,
    TOUCH_HOLD_TAIL_IGNORE as TOUCH_HOLD_TAIL_IGNORE_LENGTH_SEC,
    TOUCH_JUDGABLE_RANGE_LATE_EXTRA as TOUCH_JUDGABLE_RANGE_LATE_EXTRA_SEC,
    TOUCH_PERFECT_1ST as TOUCH_JUDGE_SEG_1ST_PERFECT_MSEC, TOUCH_PERFECT_1ST as TOUCH_PERFECT_1_MS,
    TOUCH_PERFECT_2ND as TOUCH_JUDGE_SEG_2ND_PERFECT_MSEC, TOUCH_PERFECT_2ND as TOUCH_PERFECT_2_MS,
    TOUCH_PERFECT_3RD as TOUCH_JUDGE_SEG_3RD_PERFECT_MSEC, TOUCH_PERFECT_3RD as TOUCH_PERFECT_3_MS,
};
