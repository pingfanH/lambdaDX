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
    /// Pointer movement since the previous sampled frame. Used by touch
    /// scrolling; mouse dragging keeps the existing widget path.
    pub delta: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub wheel: f32,
    active: Option<u64>,
    active_drag: bool,
    touch: bool,
    touch_id: Option<u64>,
    cancelled: bool,
    touch_start: Vec2,
    touch_dragging: bool,
    consumed: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            delta: Vec2::ZERO,
            down: false,
            pressed: false,
            released: false,
            wheel: 0.0,
            active: None,
            active_drag: false,
            touch: false,
            touch_id: None,
            cancelled: false,
            touch_start: Vec2::ZERO,
            touch_dragging: false,
            consumed: false,
        }
    }
}

impl Input {
    /// Keep active-widget state across frames while hit-testing local geometry.
    pub fn with_transform<R>(
        &mut self,
        transform: crate::player_ui::transform::ViewTransform,
        draw: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let (pos, delta, start) = (self.pos, self.delta, self.touch_start);
        self.pos = transform.to_local(pos);
        self.delta = transform.vector_to_local(delta);
        self.touch_start = transform.to_local(start);
        let result = draw(self);
        (self.pos, self.delta, self.touch_start) = (pos, delta, start);
        result
    }

    /// Sample macroquad's pointer for this frame. A touch (when present) takes
    /// priority over the mouse, so the UI is usable on a touchscreen too.
    ///
    /// Press/release edges are derived from the *held* state as well as the
    /// reported phase: touch stacks frequently drop a Started/Ended edge between
    /// frames, which otherwise leaves a button stuck showing "pressed" without
    /// ever firing its click.
    pub fn begin_frame(&mut self) {
        let (x, y) = mouse_position();
        let mut touches = touches();
        let dpi = macroquad::miniquad::window::dpi_scale().max(0.01);
        for touch in &mut touches { touch.position /= dpi; }
        self.sample_pointer(
            &touches,
            (vec2(x, y), is_mouse_button_down(MouseButton::Left),
                is_mouse_button_pressed(MouseButton::Left), is_mouse_button_released(MouseButton::Left)),
        );
        self.wheel = mouse_wheel().1;
    }

    fn sample_pointer(&mut self, touches: &[Touch], mouse: (Vec2, bool, bool, bool)) {
        let prev_down = self.down;
        let prev_pos = self.pos;

        let touch = if let Some(id) = self.touch_id {
            touches.iter().find(|t| t.id == id)
        } else {
            touches.iter().min_by_key(|t| (
                matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled), t.id))
        };
        let has_touch = touch.is_some() || self.touch_id.is_some();
        self.cancelled = touch.is_some_and(|t| t.phase == TouchPhase::Cancelled)
            || self.touch_id.is_some() && touch.is_none();
        let (pos, raw_down, raw_pressed, raw_released) = match touch {
            Some(t) => {
                self.touch_id = Some(t.id);
                (
                t.position,
                matches!(
                    t.phase,
                    TouchPhase::Started | TouchPhase::Moved | TouchPhase::Stationary
                ),
                t.phase == TouchPhase::Started
                    || !prev_down && t.phase == TouchPhase::Ended && mouse.2,
                matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled),
                )
            }
            None if has_touch => (prev_pos, false, false, true),
            None => mouse,
        };

        self.pos = pos;
        self.delta = if raw_down && prev_down {
            pos - prev_pos
        } else {
            Vec2::ZERO
        };
        self.touch = has_touch;
        self.pressed = raw_pressed || (raw_down && !prev_down);
        self.released = raw_released || (!raw_down && prev_down);
        self.down = raw_down;
        if self.cancelled { self.active = None; }
        self.update_touch_gesture();
        self.consumed = false;
    }

    fn update_touch_gesture(&mut self) {
        if self.touch && self.pressed {
            self.touch_start = self.pos;
            self.touch_dragging = false;
        } else if !self.touch || !self.down {
            // Keep the flag through the release frame so widgets can suppress
            // a click after a drag; end_frame clears it afterwards.
            if !self.released {
                self.touch_dragging = false;
            }
        } else if !self.active_drag && !self.touch_dragging
            && self.pos.distance_squared(self.touch_start) >= 12.0_f32.powi(2)
        {
            self.touch_dragging = true;
            // Cancel the row/slider that received the initial touch. The
            // release must be treated as the end of a scroll gesture.
            self.active = None;
        }
    }

    /// Drop the active widget once the button is up.
    pub fn end_frame(&mut self) {
        if self.released {
            self.active = None;
            self.active_drag = false;
            self.touch_dragging = false;
            self.touch_id = None;
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
        !self.consumed
            && crate::player_ui::draw::contains_visible(self.pos)
            && rect_contains(r, self.pos)
    }

    /// Register an interactive region. `id` must be stable across frames.
    pub fn widget(&mut self, id: u64, r: RectF) -> Response {
        let hovered = self.hover(r);
        if hovered && self.pressed {
            self.active = Some(id);
            self.active_drag = false;
        }
        let active = self.active == Some(id);
        if active {
            self.consumed = true;
        }
        // A finger that moved far enough to scroll must not activate the
        // row/button under its release point.
        let clicked = active
            && self.released
            && !self.cancelled
            && !self.touch_dragging
            && crate::player_ui::draw::contains_visible(self.pos)
            && rect_contains(r, self.pos);
        Response {
            hovered,
            held: active && self.down,
            active,
            clicked,
        }
    }

    /// True while the pointer drags inside `r` (slider handles, scroll drag).
    pub fn drag(&mut self, id: u64, r: RectF) -> Response {
        let response = self.widget(id, r);
        if response.active { self.active_drag = true; }
        response
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

    /// Return vertical movement for a touch gesture that started inside r.
    /// Calling this consumes the pointer once the drag threshold is crossed,
    /// so a row or slider cannot react to the same gesture.
    pub fn touch_scroll(&mut self, r: RectF) -> f32 {
        if !self.touch
            || self.active_drag
            || !self.touch_dragging
            || !rect_contains(r, self.touch_start)
        {
            return 0.0;
        }
        self.consumed = true;
        if self.down { self.delta.y } else { 0.0 }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::params::Params;
    use crate::player_ui::transform::hud_layout;

    #[test]
    fn fast_touch_tap_between_frames_still_clicks() {
        let r = RectF { x: 0.0, y: 0.0, w: 300.0, h: 100.0 };
        let pos = vec2(20.0, 20.0);
        let mut input = Input::default();
        input.sample_pointer(&[Touch { id: 9, phase: TouchPhase::Ended, position: pos }],
            (pos, false, true, true));
        assert!(input.widget(1, r).clicked, "Started and Ended in one frame must not lose the click");
    }

    #[test]
    fn touch_slider_does_not_switch_to_another_finger() {
        let r = RectF { x: 0.0, y: 0.0, w: 300.0, h: 100.0 };
        let mut input = Input::default();
        let finger = |id, phase, x| Touch { id, phase, position: vec2(x, 20.0) };
        let mouse = (Vec2::ZERO, false, false, false);
        input.sample_pointer(&[finger(9, TouchPhase::Started, 20.0)], mouse);
        assert!(input.drag(1, r).held);
        input.sample_pointer(&[finger(10, TouchPhase::Started, 250.0),
            finger(9, TouchPhase::Moved, 100.0)], mouse);
        assert_eq!(input.pos.x, 100.0);
        assert!(input.drag(1, r).held);
    }

    #[test]
    fn touch_drag_control_keeps_capture_instead_of_scrolling() {
        let r = RectF { x: 0.0, y: 0.0, w: 300.0, h: 100.0 };
        let mut input = Input { pos: vec2(20.0, 20.0), down: true, pressed: true,
            touch: true, ..Input::default() };
        input.update_touch_gesture();
        assert!(input.drag(1, r).held);
        input.pressed = false;
        input.pos = vec2(180.0, 50.0);
        input.delta = vec2(160.0, 30.0);
        input.consumed = false;
        input.update_touch_gesture();
        assert_eq!(input.touch_scroll(r), 0.0);
        assert!(input.drag(1, r).held);
        input.down = false;
        input.released = true;
        input.end_frame();
        assert!(!input.active_drag);
        assert_eq!(input.active, None);
    }

    #[test]
    fn touch_button_gives_way_to_page_scrolling() {
        let r = RectF { x: 0.0, y: 0.0, w: 300.0, h: 100.0 };
        let mut input = Input { pos: vec2(20.0, 20.0), down: true, pressed: true,
            touch: true, ..Input::default() };
        input.update_touch_gesture();
        assert!(input.widget(1, r).held);
        input.pressed = false;
        input.pos = vec2(20.0, 50.0);
        input.delta = vec2(0.0, 30.0);
        input.consumed = false;
        input.update_touch_gesture();
        assert_eq!(input.touch_scroll(r), 30.0);
        assert!(!input.widget(1, r).active);
    }

    #[test]
    fn rotated_hud_button_keeps_capture_and_restores_screen_pointer() {
        let p = Params { hud_rotation_deg: 90.0, hud_offset_x: -40.0,
            hud_offset_y: 20.0, ..Params::default() };
        let (_, xf) = hud_layout(1280.0, 760.0, 1.0, &p);
        let r = RectF { x: 16.0, y: 13.0, w: 84.0, h: 40.0 };
        let screen = xf.to_screen(vec2(40.0, 30.0));
        let mut input = Input { pos: screen, down: true, pressed: true, ..Input::default() };
        let response = input.with_transform(xf, |i| i.widget(42, r));
        assert!(response.held && response.hovered);
        assert_eq!(input.pos, screen);
        assert!(input.is_consumed());
        input.down = false;
        input.pressed = false;
        input.released = true;
        input.consumed = false;
        assert!(input.with_transform(xf, |i| i.widget(42, r)).clicked);
        assert_eq!(input.pos, screen);
        input.end_frame();
        assert_eq!(input.active, None);
    }

    #[test]
    fn rotated_progress_drag_maps_to_the_same_fraction_even_outside_track() {
        for angle in [0.0, 90.0, 180.0, 270.0, 33.0] {
            let p = Params { hud_rotation_deg: angle, ..Params::default() };
            let (_, xf) = hud_layout(1280.0, 760.0, 1.0, &p);
            let r = RectF { x: 200.0, y: 20.0, w: 200.0, h: 26.0 };
            let mut input = Input { pos: xf.to_screen(vec2(250.0, 33.0)),
                down: true, pressed: true, ..Input::default() };
            assert!(input.with_transform(xf, |i| i.widget(7, r)).held);
            input.pressed = false;
            input.consumed = false;
            input.pos = xf.to_screen(vec2(450.0, 90.0));
            input.with_transform(xf, |i| {
                assert!(i.widget(7, r).held);
                assert!(((i.pos.x - r.x) / r.w - 1.25).abs() < 0.001);
            });
        }
    }
}
