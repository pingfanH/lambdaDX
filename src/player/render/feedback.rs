//! Judgment text overlay. Shows a short label ("PERFECT", "GOOD", …) at the
//! hit zone, fading out over the last 0.2 s of its lifetime.

use macroquad::color::Color;
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{draw_text, get_time, measure_text};

use crate::app::types::{PAD_ROTATION_RAD, PadGeom, TAP_TARGET_OFFSET};
use crate::player::state::PadPreviewState;

/// Draw all live judgment labels.
pub fn draw(
    app: &PadPreviewState,
    pad: &PadGeom,
    outer_r: f32,
    spawn_cx: Vec2,
    scale: f32,
) {
    let now = get_time();
    for feedback in &app.judge_feedback {
        let remaining = feedback.until - now;
        if remaining <= 0.0 {
            continue;
        }
        // Fade only in the final 0.2 s.
        let alpha = if remaining < 0.2 {
            (remaining / 0.2 * 255.0) as u8
        } else {
            255u8
        };
        let color = Color::new(1.0, 1.0, 1.0, alpha as f32 / 255.0);

        // Ring zones anchor at the outer ring; touch zones at their centroid.
        let pos = if feedback.zone.to_id() <= 8 {
            let idx = (feedback.zone.to_id() - 1) as f32;
            let ang =
                -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
            let dir = vec2(ang.cos(), ang.sin());
            let target_r = outer_r + TAP_TARGET_OFFSET;
            vec2(spawn_cx.x + dir.x * target_r, spawn_cx.y + dir.y * target_r)
        } else {
            app.pad_svg
                .as_ref()
                .and_then(|svg| svg.zone_screen_centroid(feedback.zone, pad))
                .unwrap_or(vec2(pad.cx, pad.cy))
        };

        let text = feedback.label.to_uppercase();
        let font_size = 36.0 * scale;
        let dims = measure_text(&text, None, font_size as _, 1.0);
        draw_text(&text, pos.x - dims.width * 0.5, pos.y, font_size, color);
    }
}
