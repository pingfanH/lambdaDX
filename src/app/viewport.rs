//! Window → canvas coordinate transform for the editor's uniformly scaled
//! player view.
//!
//! The editor renders the whole player in a fixed design canvas
//! (`DESIGN_W × DESIGN_H` world units) and fits that canvas into a resizable
//! window with a single uniform scale plus a centered letterbox offset. Only the
//! player as a whole is scaled; nothing inside it ever changes proportion.
//!
//! macroquad reports mouse/touch positions in **window** pixels. The gameplay
//! code hit-tests in **canvas** units, so pointer reads are funneled through
//! [`to_canvas`]. The transform defaults to identity, so the preview and
//! `player_ui` binaries (which draw directly in window space) are unaffected
//! until the editor publishes its mapping each frame via [`set`].

use std::cell::Cell;

use macroquad::math::Vec2;

thread_local! {
    /// `(offset_x, offset_y, scale)` mapping canvas → window:
    /// `window = canvas * scale + offset`. Identity by default.
    static XFORM: Cell<(f32, f32, f32)> = const { Cell::new((0.0, 0.0, 1.0)) };
}

/// Publish this frame's canvas → window mapping (`window = canvas*scale + off`).
pub fn set(offset_x: f32, offset_y: f32, scale: f32) {
    XFORM.with(|c| c.set((offset_x, offset_y, scale.max(1e-6))));
}

/// Map a window-space point (mouse/touch) into canvas units.
pub fn to_canvas(p: Vec2) -> Vec2 {
    XFORM.with(|c| {
        let (ox, oy, s) = c.get();
        Vec2::new((p.x - ox) / s, (p.y - oy) / s)
    })
}

/// Inverse of [`to_canvas`]: map a canvas point back to window pixels.
pub fn to_window(p: Vec2) -> Vec2 {
    XFORM.with(|c| {
        let (ox, oy, s) = c.get();
        Vec2::new(p.x * s + ox, p.y * s + oy)
    })
}