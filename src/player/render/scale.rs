//! Uniform UI scaling.

use crate::player::state::PadPreviewState;
use macroquad::prelude::{screen_height, screen_width};

/// UI scale factor. Defaults to 1.0; `MAI2_UI_SCALE` overrides it, and mobile
/// mode scales up so the pad stays finger-sized on high-DPI screens.
pub fn ui_scale(app: &PadPreviewState) -> f32 {
    if let Some(v) = app.ui_scale_override {
        return v;
    }
    if app.mobile_ui {
        let base = screen_width().min(screen_height()) / 760.0;
        base.max(1.35)
    } else {
        1.0
    }
}
