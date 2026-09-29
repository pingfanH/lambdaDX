//! Raw multi-touch input for the player, sourced from the Linux evdev layer.
//!
//! Macroquad only exposes touch input on Android/iOS; on desktop Linux it falls
//! back to the mouse. This bridges the kernel's Type B multitouch protocol
//! directly into the player's [`PointerEvent`] stream so a real touchscreen
//! works with independent finger tracking.
//!
//! Self-contained: device discovery, the background reader thread and the frame
//! event translation all live here. [`poll`] lazily finds a device once and
//! returns `None` when there is none, so callers keep their normal mouse input.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use evdev::{AbsoluteAxisCode, Device, EventSummary, PropType, SynchronizationCode};
use macroquad::input::TouchPhase;
use macroquad::math::{Vec2, vec2};

use crate::app::types::PointerEvent;

/// Environment override for the touchscreen device path.
const DEVICE_ENV: &str = "MAI2_TOUCH_DEVICE";

/// A single active contact, normalized to [0, 1] on both axes.
#[derive(Clone, Copy, Debug, Default)]
struct TouchPoint {
    tracking_id: i32,
    x: f32,
    y: f32,
}

/// State shared between the evdev reader thread and the game loop.
#[derive(Default)]
struct TouchState {
    contacts: Vec<TouchPoint>,
    device_name: String,
    error: Option<String>,
}

fn normalize(v: i32, min: i32, max: i32) -> f32 {
    let range = (max - min) as f32;
    if range <= 0.0 {
        0.0
    } else {
        ((v - min) as f32 / range).clamp(0.0, 1.0)
    }
}

/// Locate the multitouch screen: the explicit path if given, else the first
/// `/dev/input` device advertising `ABS_MT_POSITION_X` and `INPUT_PROP_DIRECT`
/// (the "direct input" flag skips touchpads).
fn find_touchscreen(explicit: Option<PathBuf>) -> Option<(PathBuf, String)> {
    if let Some(path) = explicit {
        let device = Device::open(&path).ok()?;
        let name = device.name().unwrap_or("unknown").to_string();
        return Some((path, name));
    }

    for (path, device) in evdev::enumerate() {
        let has_mt = device
            .supported_absolute_axes()
            .map(|axes| axes.contains(AbsoluteAxisCode::ABS_MT_POSITION_X))
            .unwrap_or(false);
        if has_mt && device.properties().contains(PropType::DIRECT) {
            let name = device.name().unwrap_or("unknown").to_string();
            return Some((path, name));
        }
    }
    None
}

/// Blocking reader thread. Follows the Type B multitouch protocol:
/// `ABS_MT_SLOT` selects a slot, `ABS_MT_TRACKING_ID` identifies a contact
/// (`-1` = lifted), `ABS_MT_POSITION_X/Y` give the position, and a frame is
/// committed on `SYN_REPORT`.
fn touch_reader(
    path: PathBuf,
    state: Arc<Mutex<TouchState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut device = Device::open(path)?;
    device.grab()?;

    let mut x_min = 0i32;
    let mut x_max = 1i32;
    let mut y_min = 0i32;
    let mut y_max = 1i32;
    let mut current_slot = 0i32;

    for (axis, info) in device.get_absinfo()? {
        match axis {
            AbsoluteAxisCode::ABS_MT_POSITION_X => {
                x_min = info.minimum();
                x_max = info.maximum();
            }
            AbsoluteAxisCode::ABS_MT_POSITION_Y => {
                y_min = info.minimum();
                y_max = info.maximum();
            }
            _ => {}
        }
    }

    let mut slots: HashMap<i32, TouchPoint> = HashMap::new();

    loop {
        for event in device.fetch_events()? {
            match event.destructure() {
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_SLOT, v) => {
                    current_slot = v;
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_TRACKING_ID, v) => {
                    let p = slots.entry(current_slot).or_default();
                    p.tracking_id = v;
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_X, v) => {
                    let p = slots.entry(current_slot).or_default();
                    p.x = normalize(v, x_min, x_max);
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_Y, v) => {
                    let p = slots.entry(current_slot).or_default();
                    p.y = normalize(v, y_min, y_max);
                }
                EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                    let mut st = state.lock().unwrap();
                    st.contacts.clear();
                    for p in slots.values() {
                        if p.tracking_id >= 0 {
                            st.contacts.push(*p);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Owns the reader thread and translates raw contacts into `PointerEvent`s.
pub struct EvdevTouch {
    state: Arc<Mutex<TouchState>>,
    seen: HashSet<i32>,
    last_pos: HashMap<i32, Vec2>,
}

impl EvdevTouch {
    /// Find a multitouch touchscreen and start the reader thread. `None` when no
    /// device is found (the caller then keeps mouse / macroquad touch input).
    pub fn start(explicit: Option<PathBuf>) -> Option<Self> {
        let (path, name) = find_touchscreen(explicit)?;
        let state = Arc::new(Mutex::new(TouchState {
            device_name: name,
            ..Default::default()
        }));
        let thread_state = Arc::clone(&state);
        std::thread::spawn(move || {
            if let Err(e) = touch_reader(path, Arc::clone(&thread_state)) {
                thread_state.lock().unwrap().error = Some(e.to_string());
            }
        });
        Some(Self {
            state,
            seen: HashSet::new(),
            last_pos: HashMap::new(),
        })
    }

    pub fn device_name(&self) -> String {
        self.state.lock().unwrap().device_name.clone()
    }

    pub fn last_error(&self) -> Option<String> {
        self.state.lock().unwrap().error.clone()
    }

    /// Poll the current contacts and emit this frame's pointer events. `screen`
    /// is the window size in pixels; contacts are keyed by tracking id so the
    /// caller can track individual fingers across frames.
    pub fn collect_pointer_events(&mut self, screen: Vec2) -> Vec<PointerEvent> {
        let mut events = Vec::new();
        let contacts: Vec<TouchPoint> = self.state.lock().unwrap().contacts.clone();

        let mut current: HashSet<i32> = HashSet::new();
        for p in &contacts {
            current.insert(p.tracking_id);
            self.last_pos
                .insert(p.tracking_id, vec2(p.x * screen.x, p.y * screen.y));
        }

        for id in self.seen.difference(&current).copied().collect::<Vec<_>>() {
            events.push(PointerEvent {
                id: id as u64,
                phase: TouchPhase::Ended,
                position: self.last_pos.get(&id).copied().unwrap_or(Vec2::ZERO),
            });
        }

        for p in &contacts {
            let pos = self.last_pos[&p.tracking_id];
            let phase = if self.seen.contains(&p.tracking_id) {
                TouchPhase::Stationary
            } else {
                TouchPhase::Started
            };
            events.push(PointerEvent {
                id: p.tracking_id as u64,
                phase,
                position: pos,
            });
        }

        self.seen = current;
        events
    }
}

thread_local! {
    /// Lazily-initialized touchscreen. `Some(None)` = probed, no device found.
    static TOUCH: RefCell<Option<Option<EvdevTouch>>> = const { RefCell::new(None) };
}

/// Poll the evdev touchscreen. Returns `None` when no device is available, so
/// the caller falls back to macroquad's mouse/touch input.
pub fn poll(screen: Vec2) -> Option<Vec<PointerEvent>> {
    let explicit = std::env::var(DEVICE_ENV).ok().map(PathBuf::from);
    TOUCH.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(EvdevTouch::start(explicit));
        }
        slot.as_mut()
            .and_then(|d| d.as_mut())
            .map(|d| d.collect_pointer_events(screen))
    })
}

/// Human-readable status of the touchscreen (device name or last error).
pub fn status() -> Option<String> {
    TOUCH.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let explicit = std::env::var(DEVICE_ENV).ok().map(PathBuf::from);
            *slot = Some(EvdevTouch::start(explicit));
        }
        slot.as_ref().and_then(|d| d.as_ref()).map(|d| {
            d.last_error()
                .map(|e| format!("Multi-touch: {e}"))
                .unwrap_or_else(|| format!("Multi-touch: {}", d.device_name()))
        })
    })
}
