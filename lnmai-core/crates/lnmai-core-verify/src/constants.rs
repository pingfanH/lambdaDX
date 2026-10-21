//! Aeneas-friendly constants module
//!
//! All timing constants in microseconds

use super::time::Duration;

/// Frame length (60fps = 16667μs)
pub const FRAME_LENGTH: Duration = Duration { micros: 16667 };

// Tap judgment windows
pub const TAP_PERFECT_1ST: Duration = Duration { micros: 16667 };
pub const TAP_PERFECT_2ND: Duration = Duration { micros: 33334 };
pub const TAP_PERFECT_3RD: Duration = Duration { micros: 50001 };
pub const TAP_GREAT_1ST: Duration = Duration { micros: 66668 };
pub const TAP_GREAT_2ND: Duration = Duration { micros: 83335 };
pub const TAP_GREAT_3RD: Duration = Duration { micros: 100002 };
pub const TAP_GOOD: Duration = Duration { micros: 150000 };

// Touch judgment windows
pub const TOUCH_PERFECT_1ST: Duration = Duration { micros: 150003 };
pub const TOUCH_PERFECT_2ND: Duration = Duration { micros: 175004 };
pub const TOUCH_PERFECT_3RD: Duration = Duration { micros: 200004 };
pub const TOUCH_GREAT_1ST: Duration = Duration { micros: 216671 };
pub const TOUCH_GREAT_2ND: Duration = Duration { micros: 233338 };
pub const TOUCH_GREAT_3RD: Duration = Duration { micros: 250005 };
pub const TOUCH_GOOD: Duration = Duration { micros: 300006 };

// Slide judgment windows
pub const SLIDE_PERFECT_3RD: Duration = Duration { micros: 233338 };
pub const SLIDE_GREAT_1ST: Duration = Duration { micros: 350007 };
pub const SLIDE_GREAT_2ND: Duration = Duration { micros: 416675 };
pub const SLIDE_GREAT_3RD: Duration = Duration { micros: 483343 };
pub const SLIDE_GOOD: Duration = Duration { micros: 600012 };
pub const SLIDE_MAX_EXT: Duration = Duration { micros: 366674 };

// Hold constants
pub const HOLD_HEAD_IGNORE: Duration = Duration { micros: 100002 };
pub const HOLD_TAIL_IGNORE: Duration = Duration { micros: 200004 };
pub const TOUCH_HOLD_HEAD_IGNORE: Duration = Duration { micros: 250005 };
pub const TOUCH_HOLD_TAIL_IGNORE: Duration = Duration { micros: 200004 };
pub const DELUXE_HOLD_RELEASE_IGNORE_TIME: Duration = Duration { micros: 33334 };
pub const CLASSIC_HOLD_ALLOW_OVER_LENGTH: Duration = Duration { micros: 333340 };

// Classic hold end windows (9 frames / 12 frames)
pub const HOLD_CLASSIC_END_JUDGE_PERFECT_FAST: Duration = Duration { micros: 150003 };
pub const HOLD_CLASSIC_END_JUDGE_PERFECT_LATE: Duration = Duration { micros: 200004 };

// Late clear wait
pub const SLIDE_JUDGED_LATE_CLEAR_WAIT: Duration = Duration { micros: 50000 };

// Judgable range
pub const JUDGABLE_RANGE: Duration = Duration { micros: 150000 };
pub const TOUCH_JUDGABLE_RANGE_LATE_EXTRA: Duration = Duration { micros: 166670 };

// Counts
pub const BUTTON_ZONE_COUNT: u32 = 8;
pub const SENSOR_AREA_COUNT: u32 = 33;

// Judge offset applied to slide sub-grade subdivision.
pub const SUBDIVIDE_SLIDE_JUDGE_GRADE: bool = false;
