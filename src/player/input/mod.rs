//! Pointer + keyboard input, split by source.
//!
//! | module | responsibility |
//! |---|---|
//! | [`pointer`] | touch/mouse events, zone hit-testing, active highlights |
//! | [`keyboard`] | lane keys and playback hotkeys |
//! | [`hit`] | tiny stand-in "judge" that labels a zone hit |

pub mod hit;
pub mod keyboard;
pub mod pointer;

pub use keyboard::{handle_global_hotkeys, handle_lane_input};
pub use pointer::{collect_pointer_events, handle_touch_controls};
