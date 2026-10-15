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
    function_keys: Vec<egui::Event>,
    /// Pointer held state and last position, so a dropped touch edge still
    /// produces the matching press/release for egui (a button otherwise shows
    /// pressed forever without ever firing).
    pointer_down: bool,
    /// Held state of the middle / secondary buttons. The explicit feed above is
    /// left-only (it follows the unified touch/mouse pointer), so these are
    /// derived from macroquad's own mouse state and pushed as press/release
    /// edges — otherwise `Response::middle_clicked()` / `secondary_clicked()`
    /// never fire.
    middle_down: bool,
    secondary_down: bool,
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
            function_keys: Vec::new(),
            pointer_down: false,
            middle_down: false,
            secondary_down: false,
            last_pos: Vec2::ZERO,
            wants_pointer: false,
        });
        f(egui)
    })
}

fn sync_dpi(ctx: &egui::Context, dpi: f32) {
    ctx.set_pixels_per_point(dpi);
}

fn pointer_pixels(pos: Vec2, dpi: f32) -> Vec2 {
    pos * dpi
}

// egui-miniquad 0.16 does not translate function keys. Keep this supplemental
// path limited to those keys so ordinary keyboard input is not duplicated.
fn queue_function_key(events: &mut Vec<egui::Event>, key: mq::KeyCode, mods: mq::KeyMods, pressed: bool, repeat: bool) {
    let key = match key {
        mq::KeyCode::F1 => egui::Key::F1, mq::KeyCode::F2 => egui::Key::F2,
        mq::KeyCode::F3 => egui::Key::F3, mq::KeyCode::F4 => egui::Key::F4,
        mq::KeyCode::F5 => egui::Key::F5, mq::KeyCode::F6 => egui::Key::F6,
        mq::KeyCode::F7 => egui::Key::F7, mq::KeyCode::F8 => egui::Key::F8,
        mq::KeyCode::F9 => egui::Key::F9, mq::KeyCode::F10 => egui::Key::F10,
        mq::KeyCode::F11 => egui::Key::F11, mq::KeyCode::F12 => egui::Key::F12,
        _ => return,
    };
    events.push(egui::Event::Key {
        key, physical_key: Some(key), pressed, repeat,
        modifiers: egui::Modifiers {
            alt: mods.alt, ctrl: mods.ctrl, shift: mods.shift,
            mac_cmd: mods.logo && cfg!(target_os = "macos"),
            command: if cfg!(target_os = "macos") { mods.logo } else { mods.ctrl },
        },
    });
}

fn feed_function_keys(ctx: &egui::Context, events: &mut Vec<egui::Event>) {
    ctx.input_mut(|input| {
        for event in events.drain(..) {
            if let egui::Event::Key { key, pressed, .. } = &event {
                if *pressed { input.keys_down.insert(*key); }
                else { input.keys_down.remove(key); }
            }
            input.raw.events.push(event.clone());
            input.events.push(event);
        }
    });
}

impl Egui {
    fn ui<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut dyn mq::RenderingBackend, &egui::Context),
    {
        let gl = unsafe { get_internal_gl() };
        let dpi = mq::window::dpi_scale();
        sync_dpi(self.egui_mq.egui_ctx(), dpi);
        macroquad::input::utils::repeat_all_miniquad_input(self, self.input_subscriber_id);
        let function_keys = &mut self.function_keys;
        self.egui_mq.run(gl.quad_context, |backend, ctx| {
            // EguiMq only publishes native DPI after a monitor change. Keep
            // its initial frame and later native-DPI updates at logical 1x.
            sync_dpi(ctx, dpi);
            feed_function_keys(ctx, function_keys);
            f(backend, ctx);
        });
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
        let dpi = mq::window::dpi_scale();
        sync_dpi(self.egui_mq.egui_ctx(), dpi);
        // EguiMq expects physical pixels and divides by pixels_per_point.
        let pos = pointer_pixels(pos, dpi);

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

        // Middle / secondary buttons. These never travel through the unified
        // pointer, so forward macroquad's own button state as press/release
        // edges (the editor uses middle-click to enter star-edit mode and
        // right-click to delete a note).
        self.sync_button(mq::MouseButton::Middle, pos);
        self.sync_button(mq::MouseButton::Right, pos);
    }

    /// Mirror a macroquad mouse button into an egui press/release edge, using
    /// the stored held state to detect the transition.
    fn sync_button(&mut self, button: mq::MouseButton, pos: Vec2) {
        let down = macroquad::input::is_mouse_button_down(button);
        let held = match button {
            mq::MouseButton::Middle => self.middle_down,
            mq::MouseButton::Right => self.secondary_down,
            _ => return,
        };
        if down == held {
            return;
        }
        match button {
            mq::MouseButton::Middle => self.middle_down = down,
            mq::MouseButton::Right => self.secondary_down = down,
            _ => return,
        }
        if down {
            self.egui_mq.mouse_button_down_event(button, pos.x, pos.y);
        } else {
            self.egui_mq.mouse_button_up_event(button, pos.x, pos.y);
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
    use macroquad::miniquad as mq;

    #[test]
    fn native_function_keys_reach_egui_with_modifiers_and_release() {
        let ctx = egui::Context::default();
        let cases = [
            (mq::KeyCode::F1, egui::Key::F1), (mq::KeyCode::F2, egui::Key::F2),
            (mq::KeyCode::F3, egui::Key::F3), (mq::KeyCode::F4, egui::Key::F4),
            (mq::KeyCode::F5, egui::Key::F5), (mq::KeyCode::F6, egui::Key::F6),
            (mq::KeyCode::F7, egui::Key::F7), (mq::KeyCode::F8, egui::Key::F8),
            (mq::KeyCode::F9, egui::Key::F9), (mq::KeyCode::F10, egui::Key::F10),
            (mq::KeyCode::F11, egui::Key::F11), (mq::KeyCode::F12, egui::Key::F12),
        ];
        for (native, key) in cases {
            let mut pending = Vec::new();
            super::queue_function_key(&mut pending, native, mq::KeyMods { shift: true, ..Default::default() }, true, false);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                super::feed_function_keys(ctx, &mut pending);
                ctx.input(|i| {
                    assert!(i.key_pressed(key), "{native:?} press was lost");
                    assert!(i.key_down(key));
                    assert!(i.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, modifiers, .. } if *k == key && modifiers.shift)));
                });
            });
            super::queue_function_key(&mut pending, native, mq::KeyMods::default(), true, true);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                super::feed_function_keys(ctx, &mut pending);
                ctx.input(|i| assert!(i.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, repeat: true, .. } if *k == key))));
            });
            super::queue_function_key(&mut pending, native, mq::KeyMods::default(), false, false);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                super::feed_function_keys(ctx, &mut pending);
                ctx.input(|i| { assert!(i.key_released(key)); assert!(!i.key_down(key)); });
            });
        }
        let mut pending = Vec::new();
        super::queue_function_key(&mut pending, mq::KeyCode::A, mq::KeyMods::default(), true, false);
        assert!(pending.is_empty(), "ordinary keys must stay on the existing backend path");
    }

    #[test]
    fn dpi_changes_preserve_logical_ui_dimensions_and_pointer_position() {
        let ctx = egui::Context::default();
        for dpi in [1.0, 2.0, 1.0, 2.0] {
            for _ in 0..3 {
                super::sync_dpi(&ctx, dpi);
                let mut raw = egui::RawInput::default();
                raw.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 760.0) * dpi / ctx.pixels_per_point(),
                ));
                raw.viewports
                    .get_mut(&raw.viewport_id)
                    .unwrap()
                    .native_pixels_per_point = Some(dpi);
                let _ = ctx.run(raw, |ctx| super::sync_dpi(ctx, dpi));
            }
            assert_eq!(ctx.pixels_per_point(), dpi);
            assert_eq!(ctx.screen_rect().size(), egui::vec2(1280.0, 760.0));
            let pos = macroquad::prelude::vec2(460.0, 316.0);
            // EguiMq divides physical pointer coordinates by its context scale.
            assert_eq!(super::pointer_pixels(pos, dpi) / ctx.pixels_per_point(), pos);
        }
    }

    #[test]
    fn initial_hidpi_frame_without_native_viewport_info_uses_logical_sizes() {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            super::sync_dpi(&ctx, 2.0);
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2560.0, 1520.0) / ctx.pixels_per_point(),
            ));
            let _ = ctx.run(raw, |ctx| super::sync_dpi(ctx, 2.0));
        }
        assert_eq!(ctx.pixels_per_point(), 2.0);
        assert_eq!(ctx.screen_rect().size(), egui::vec2(1280.0, 760.0));
    }

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

    fn key_down_event(&mut self, keycode: mq::KeyCode, keymods: mq::KeyMods, repeat: bool) {
        self.egui_mq.key_down_event(keycode, keymods);
        queue_function_key(&mut self.function_keys, keycode, keymods, true, repeat);
    }

    fn key_up_event(&mut self, keycode: mq::KeyCode, keymods: mq::KeyMods) {
        self.egui_mq.key_up_event(keycode, keymods);
        queue_function_key(&mut self.function_keys, keycode, keymods, false, false);
    }
}
