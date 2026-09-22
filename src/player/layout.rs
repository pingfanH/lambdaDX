//! Panel layout for the standalone preview.
//!
//! Mirrors `player_layout.rs` from the player: a header strip followed by one
//! large pad panel. `compute_pad_geom` then centers a circle in that panel.

use crate::app::types::{Layout, PadGeom, RectF};
use crate::player::render::ui_scale;
use crate::player::state::PadPreviewState;
use macroquad::prelude::{screen_height, screen_width};

/// Header strip + full-width pad panel below it.
pub fn compute_layout(app: &PadPreviewState) -> Layout {
    let scale = ui_scale(app);
    let sw = screen_width();
    let sh = screen_height();
    let margin = if app.mobile_ui { 12.0 } else { 20.0 } * scale;
    let header_h = 76.0 * scale;

    let header = RectF {
        x: margin,
        y: margin,
        w: sw - margin * 2.0,
        h: header_h,
    };
    let pad = RectF {
        x: margin,
        y: header_h,
        w: sw - margin * 2.0,
        h: sh - header_h - margin,
    };
    Layout {
        header,
        timeline: None,
        pad,
    }
}

/// Circle inscribed in `panel` at 42% of its shorter side.
pub fn compute_pad_geom(panel: RectF) -> PadGeom {
    PadGeom {
        cx: panel.x + panel.w * 0.5,
        cy: panel.y + panel.h * 0.5,
        outer_r: panel.w.min(panel.h) * 0.42,
    }
}
