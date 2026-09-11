//! Raw multi-touch input for the player, sourced from the Linux evdev layer.
//!
//! The player (macroquad) only exposes touch input on Android/iOS; on desktop
//! Linux it falls back to the mouse. This module bridges the kernel's Type B
//! multitouch protocol directly into the player's `PointerEvent` stream so a
//! touchscreen works the same way it does in the standalone `test_touch` demo.
//!
//! It is deliberately self-contained: device discovery, the background reader
//! thread, and frame event translation all live here. The player only calls
//! [`EvdevTouch::start`] once and [`EvdevTouch::collect_pointer_events`] per
//! frame; nothing is merged into the shared `input` module.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use evdev::{AbsoluteAxisCode, Device, EventSummary, PropType, SynchronizationCode};
use lambda_dx::app::types::PointerEvent;
use macroquad::input::TouchPhase;
use macroquad::math::{Vec2, vec2};

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

/// Normalize a raw axis value to [0, 1].
fn normalize(v: i32, min: i32, max: i32) -> f32 {
    let range = (max - min) as f32;
    if range <= 0.0 {
        0.0
    } else {
        ((v - min) as f32 / range).clamp(0.0, 1.0)
    }
}

/// Locate the multitouch screen. Uses the explicit path if given, otherwise
/// scans `/dev/input` for the first device advertising `ABS_MT_POSITION_X` and
/// `INPUT_PROP_DIRECT` (the kernel's "direct input" flag, to skip touchpads).
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
        let is_direct = device.properties().contains(PropType::DIRECT);
        if has_mt && is_direct {
            let name = device.name().unwrap_or("unknown").to_string();
            return Some((path, name));
        }
    }
    None
}

/// Blocking reader thread: consumes evdev events and maintains the current set
/// of contacts following the Linux Type B multitouch protocol.
///
/// `ABS_MT_SLOT` selects a slot, `ABS_MT_TRACKING_ID` identifies a contact
/// (-1 means "lifted"), and `ABS_MT_POSITION_X/Y` give the position. A frame
/// is committed on `SYN_REPORT`.
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
    /// Find a multitouch touchscreen and start the reader thread.
    ///
    /// Returns `None` when no device is found; the player then keeps its
    /// normal mouse/macroquad pointer input.
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

    /// Poll the current contacts and emit this frame's pointer events.
    ///
    /// `screen` is the window size in pixels; raw normalized coordinates are
    /// scaled into it. Contacts are keyed by tracking id, so the player can
    /// track individual fingers across frames.
    pub fn collect_pointer_events(&mut self, screen: Vec2) -> Vec<PointerEvent> {
        let mut events = Vec::new();

        let contacts: Vec<TouchPoint> = self.state.lock().unwrap().contacts.clone();

        let mut current: HashSet<i32> = HashSet::new();
        for p in &contacts {
            current.insert(p.tracking_id);
            let pos = vec2(p.x * screen.x, p.y * screen.y);
            self.last_pos.insert(p.tracking_id, pos);
        }

        // Lifted since the previous frame.
        let ended: Vec<i32> = self.seen.difference(&current).copied().collect();
        for id in ended {
            events.push(PointerEvent {
                id: id as u64,
                phase: TouchPhase::Ended,
                position: self.last_pos.get(&id).copied().unwrap_or(Vec2::ZERO),
            });
        }

        // New or still-held contacts.
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
