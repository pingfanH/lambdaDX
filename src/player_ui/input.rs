//! Minimal immediate-mode pointer handling on top of macroquad.
//!
//! There are no retained widgets: each frame a page calls [`Input::widget`] with
//! the screen rect of whatever it just drew, and gets hover / press / click
//! state back. A single active widget id is tracked so a press that starts on a
//! button and releases elsewhere does not fire a click.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::app::ui::rect_contains;

#[derive(Debug, Clone, Copy, Default)]
pub struct Response {
    pub hovered: bool,
    pub held: bool,
    pub active: bool,
    pub clicked: bool,
}

#[derive(Clone, Copy)]
pub struct Input {
    pub pos: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub wheel: f32,
    active: Option<u64>,
    consumed: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            down: false,
            pressed: false,
            released: false,
            wheel: 0.0,
            active: None,
            consumed: false,
        }
    }
}

impl Input {
    /// Sample macroquad's pointer for this frame. A touch (when present) takes
    /// priority over the mouse, so the UI is usable on a touchscreen too.
    ///
    /// Press/release edges are derived from the *held* state as well as the
    /// reported phase: touch stacks frequently drop a Started/Ended edge between
    /// frames, which otherwise leaves a button stuck showing "pressed" without
    /// ever firing its click.
    pub fn begin_frame(&mut self) {
        let prev_down = self.down;

        let touch = touches().into_iter().next();
        let (pos, raw_down, raw_pressed, raw_released) = match touch {
            Some(t) => (
                t.position,
                matches!(
                    t.phase,
                    TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary
                ),
                t.phase == TouchPhase::Started,
                matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled),
            ),
            None => {
                let (x, y) = mouse_position();
                (
                    vec2(x, y),
                    is_mouse_button_down(MouseButton::Left),
                    is_mouse_button_pressed(MouseButton::Left),
                    is_mouse_button_released(MouseButton::Left),
                )
            }
        };

        self.pos = pos;
        self.pressed = raw_pressed || (raw_down && !prev_down);
        self.released = raw_released || (!raw_down && prev_down);
        self.down = raw_down;
        self.wheel = mouse_wheel().1;
        self.consumed = false;
    }

    /// Drop the active widget once the button is up.
    pub fn end_frame(&mut self) {
        if self.released {
            self.active = None;
        }
    }

    /// Consume the pointer for this frame so no widget reacts to it (the pad
    /// takes priority over overlapping UI).
    pub fn consume(&mut self) {
        self.consumed = true;
    }

    /// Whether a widget already claimed the pointer this frame.
    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    pub fn hover(&self, r: RectF) -> bool {
        !self.consumed && rect_contains(r, self.pos)
    }

    /// Register an interactive region. `id` must be stable across frames.
    pub fn widget(&mut self, id: u64, r: RectF) -> Response {
        let hovered = self.hover(r);
        if hovered && self.pressed {
            self.active = Some(id);
        }
        let active = self.active == Some(id);
        if active {
            self.consumed = true;
        }
        let clicked = active && self.released && rect_contains(r, self.pos);
        Response {
            hovered,
            held: active && self.down,
            active,
            clicked,
        }
    }

    /// True while the pointer drags inside `r` (slider handles, scroll drag).
    pub fn drag(&mut self, id: u64, r: RectF) -> Response {
        self.widget(id, r)
    }

    /// Consume the wheel when the pointer is inside `r`; returns the delta.
    pub fn scroll(&mut self, r: RectF) -> f32 {
        if rect_contains(r, self.pos) && self.wheel.abs() > 0.0 {
            let w = self.wheel;
            self.wheel = 0.0;
            w
        } else {
            0.0
        }
    }
}

/// Stable widget id from a name and an optional list index.
pub fn id(name: &str, index: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}
