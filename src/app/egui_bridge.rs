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
    /// Pointer held state and last position, so a dropped touch edge still
    /// produces the matching press/release for egui (a button otherwise shows
    /// pressed forever without ever firing).
    pointer_down: bool,
    last_pos: Vec2,
    /// Whether egui wanted the pointer on the last frame it ran.
    wants_pointer: bool,
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
            pointer_down: false,
            last_pos: Vec2::ZERO,
            wants_pointer: false,
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
        // Recorded so the caller can keep the pad from stealing a touch that is
        // interacting with a panel widget.
        self.wants_pointer = self.egui_mq.egui_ctx().wants_pointer_input();
    }

    fn draw(&mut self) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.egui_mq.draw(gl.quad_context);
    }

    /// Push this frame's pointer (mouse or touch) into egui. `pos` is fed every
    /// frame so hover works even with no button down.
    ///
    /// The press/release edges are derived from the `down` transition as well as
    /// the reported phase, so a dropped Started/Ended event still resolves to a
    /// complete click (using the last known position for a synthesised release).
    fn pointer(&mut self, pos: Vec2, down: bool, pressed: bool, released: bool) {
        let press = pressed || (down && !self.pointer_down);
        let release = released || (!down && self.pointer_down);
        if down || pressed {
            self.last_pos = pos;
        }

        // Hover / drag position, every frame.
        self.egui_mq.mouse_motion_event(pos.x, pos.y);

        if press {
            self.egui_mq
                .mouse_button_down_event(mq::MouseButton::Left, pos.x, pos.y);
        }
        if release {
            // An explicit Ended carries its own position; a synthesised release
            // (dropped touch edge) reuses the last held position.
            let up = if released { pos } else { self.last_pos };
            self.egui_mq
                .mouse_button_up_event(mq::MouseButton::Left, up.x, up.y);
        }
        self.pointer_down = down;
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
/// position; `down`/`pressed`/`released` describe the button state.
pub fn pointer(pos: Vec2, down: bool, pressed: bool, released: bool) {
    with_egui(|egui| egui.pointer(pos, down, pressed, released));
}

/// Whether egui wanted the pointer on the last frame it ran (a panel widget is
/// hovered or being interacted with). The pad should then ignore the touch.
pub fn wants_pointer() -> bool {
    with_egui(|egui| egui.wants_pointer)
}

#[cfg(test)]
mod tests {
    use egui_macroquad::egui;

    /// One egui frame with a button pinned at `rect`; returns whether it fired.
    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, rect: egui::Rect) -> bool {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 760.0),
        ));
        raw.events = events;
        let mut clicked = false;
        let _ = ctx.run(raw, |c| {
            egui::CentralPanel::default().show(c, |ui| {
                if ui.put(rect, egui::Button::new("x")).clicked() {
                    clicked = true;
                }
            });
        });
        clicked
    }

    /// The press-then-release sequence we feed from touch must register a click.
    #[test]
    fn pointer_press_then_release_clicks_a_button() {
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(400.0, 300.0), egui::vec2(120.0, 32.0));
        let p = egui::pos2(460.0, 316.0);
        let md = egui::Modifiers::default();

        // Warm-up frame establishes the layout.
        frame(&ctx, vec![egui::Event::PointerMoved(p)], rect);
        // Press.
        assert!(!frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: md,
                },
            ],
            rect,
        ));
        // Release in place.
        let clicked = frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: md,
                },
            ],
            rect,
        );
        assert!(clicked, "a press+release on the button must click it");
    }
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
