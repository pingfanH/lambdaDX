//! Shared flat progress bar, used by both front-ends so the playback scrubber
//! looks identical in the pad preview and the player UI.

use macroquad::prelude::*;

use crate::app::types::RectF;

/// A 1px-outlined rectangle outline.
pub fn rect_outline(r: RectF, thickness: f32, c: Color) {
    draw_rectangle_lines(r.x, r.y, r.w, r.h, thickness, c);
}

/// A flat playback/scroll bar with a 1px border.
pub fn progress_bar(r: RectF, frac: f32, fill: Color, bg: Color, border: Color) {
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    let w = (r.w * frac.clamp(0.0, 1.0)).max(0.0);
    if w > 0.0 {
        draw_rectangle(r.x, r.y, w, r.h, fill);
    }
    rect_outline(r, 1.0, border);
}
