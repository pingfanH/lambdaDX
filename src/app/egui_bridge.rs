//! egui ↔ macroquad bridge with **touch** support.
//!
//! This is a small, self-contained copy of `egui-macroquad`'s bridge (which
//! only forwards mouse/keyboard). The addition is an explicit [`pointer`] feed:
//! each frame the caller hands in the *unified* pointer (desktop mouse, macroquad
//! touch, or the Linux evdev touchscreen), which is pushed into egui. That makes
//! the egui params panel respond to a touchscreen even when the touch events
//! bypass macroquad entirely (the evdev reader does).
//!
//! Mouse events in the miniquad `EventHandler` are intentionally ignored — the
//! explicit feed already carries the mouse — so pointer input is never doubled.
//! Wheel and keyboard still arrive through macroquad's input subscriber.

use std::cell::RefCell;

use egui_macroquad::egui;
use egui_miniquad::EguiMq;
use macroquad::miniquad as mq;
use macroquad::prelude::{Vec2, get_internal_gl};

struct Egui {
    egui_mq: EguiMq,
    input_subscriber_id: usize,
}

thread_local! {
    static EGUI: RefCell<Option<Egui>> = const { RefCell::new(None) };
}

fn with_egui<R>(f: impl FnOnce(&mut Egui) -> R) -> R {
    EGUI.with(|cell| {
        let mut slot = cell.borrow_mut();
        let egui = slot.get_or_insert_with(|| Egui {
            egui_mq: EguiMq::new(unsafe { get_internal_gl() }.quad_context),
            input_subscriber_id: macroquad::input::utils::register_input_subscriber(),
        });
        f(egui)
    })
}

impl Egui {
    fn ui<F>(&mut self, f: F)
    where
        F: FnMut(&mut dyn mq::RenderingBackend, &egui::Context),
    {
        let gl = unsafe { get_internal_gl() };
        macroquad::input::utils::repeat_all_miniquad_input(self, self.input_subscriber_id);
        self.egui_mq.run(gl.quad_context, f);
    }

    fn draw(&mut self) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.egui_mq.draw(gl.quad_context);
    }

    /// Push this frame's pointer (mouse or touch) into egui. `pos` is fed every
    /// frame so hover works even with no button down.
    fn pointer(&mut self, pos: Vec2, pressed: bool, released: bool) {
        self.egui_mq.mouse_motion_event(pos.x, pos.y);
        if pressed {
            self.egui_mq
                .mouse_button_down_event(mq::MouseButton::Left, pos.x, pos.y);
        }
        if released {
            self.egui_mq
                .mouse_button_up_event(mq::MouseButton::Left, pos.x, pos.y);
        }
    }
}

/// Run one egui frame. Call once per frame (after [`pointer`], before [`draw`]).
pub fn ui<F: FnMut(&egui::Context)>(mut f: F) {
    with_egui(|egui| egui.ui(|_, ctx| f(ctx)));
}

/// Configure egui without beginning or ending a frame.
pub fn cfg<F: FnOnce(&egui::Context)>(f: F) {
    with_egui(|egui| f(egui.egui_mq.egui_ctx()));
}

/// Draw the egui buffers. Must be called after [`ui`].
pub fn draw() {
    with_egui(|egui| egui.draw());
}

/// Feed the unified pointer for this frame. `pos` is the cursor / first-touch
/// position; `pressed`/`released` describe the button edges.
pub fn pointer(pos: Vec2, pressed: bool, released: bool) {
    with_egui(|egui| egui.pointer(pos, pressed, released));
}

impl mq::EventHandler for Egui {
    fn update(&mut self) {}
    fn draw(&mut self) {}

    // Pointer input is fed explicitly via `pointer`, so ignore it here.
    fn mouse_motion_event(&mut self, _x: f32, _y: f32) {}
    fn mouse_button_down_event(&mut self, _mb: mq::MouseButton, _x: f32, _y: f32) {}
    fn mouse_button_up_event(&mut self, _mb: mq::MouseButton, _x: f32, _y: f32) {}

    fn mouse_wheel_event(&mut self, dx: f32, dy: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        self.egui_mq.mouse_wheel_event(dx / 90., dy / 90.);

        #[cfg(target_arch = "wasm32")]
        self.egui_mq.mouse_wheel_event(dx / 30., dy / 30.);
    }

    fn char_event(&mut self, character: char, _keymods: mq::KeyMods, _repeat: bool) {
        self.egui_mq.char_event(character);
    }

    fn key_down_event(&mut self, keycode: mq::KeyCode, keymods: mq::KeyMods, _repeat: bool) {
        self.egui_mq.key_down_event(keycode, keymods);
    }

    fn key_up_event(&mut self, keycode: mq::KeyCode, keymods: mq::KeyMods) {
        self.egui_mq.key_up_event(keycode, keymods);
    }
}
