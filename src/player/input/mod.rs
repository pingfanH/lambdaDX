//! Pointer + keyboard input, split by source.
//!
//! | module | responsibility |
//! |---|---|
//! | [`pointer`] | touch/mouse events, zone hit-testing, active highlights |
//! | [`keyboard`] | lane keys and playback hotkeys |
//! | `touch_evdev` | Linux desktop multi-touch touchscreen (evdev, Type B) |
//! Judgment is provided by `lnmai-core`.

pub mod keyboard;
pub mod pointer;
#[cfg(target_os = "linux")]
pub mod touch_evdev;

pub use keyboard::{handle_global_hotkeys, handle_lane_input};
pub use pointer::{collect_pointer_events, handle_touch_controls};
