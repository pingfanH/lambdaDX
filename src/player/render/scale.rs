//! Uniform UI / pad scaling.
//!
//! Everything in the pad view (note sizes, zone strokes, labels, the disc and
//! the whole UI) must scale with the same factor as the pad radius. Otherwise a
//! window resize or a DPI change (which change the framebuffer size) would grow
//! the pad but leave the notes fixed, breaking their proportions.
//!
//! The factor is derived from the window's shorter side against a design
//! reference, so it is DPI-independent: if the framebuffer doubles (Retina),
//! every dimension doubles too and the apparent size in points is unchanged.

use crate::player::state::PadPreviewState;
use macroquad::prelude::{screen_height, screen_width};

/// Shorter-side length the visuals were tuned at (1280×760 default window).
pub const DESIGN_MIN_SIDE: f32 = 760.0;

/// Scale factor for all pad/UI dimensions. `MAI2_UI_SCALE` / the settings
/// override wins; mobile keeps a larger floor so the pad stays finger-sized.
pub fn ui_scale(app: &PadPreviewState) -> f32 {
    if let Some(v) = app.ui_scale_override {
        return v;
    }
    let base = screen_width().min(screen_height()) / DESIGN_MIN_SIDE;
    if app.mobile_ui {
        base.max(1.35)
    } else {
        base.max(0.1)
    }
}
