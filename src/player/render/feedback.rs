//! Judgment text overlay. Shows a short label ("PERFECT", "GOOD", …) at the
//! hit zone, fading out over the last 0.2 s of its lifetime.

use macroquad::color::Color;
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{draw_text, measure_text};

use crate::app::types::{PAD_ROTATION_RAD, PadGeom};
use crate::player::state::PadPreviewState;
use crate::app::params;

/// Draw all live judgment labels.
pub fn draw(
    app: &PadPreviewState,
    pad: &PadGeom,
    outer_r: f32,
    spawn_cx: Vec2,
    scale: f32,
) {
    let now = app.now();
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
        let pos = zone_anchor(app, pad, outer_r, spawn_cx, feedback.zone);

        let text = feedback.label.to_uppercase();
        let font_size = 36.0 * scale;
        let dims = measure_text(&text, None, font_size as _, 1.0);
        draw_text(&text, pos.x - dims.width * 0.5, pos.y, font_size, color);
    }
}

/// Screen position a judgment/effect anchors to for `zone`: the A/B ring point
/// for zones 1-8, the sensor centroid otherwise.
pub fn zone_anchor(
    app: &PadPreviewState,
    pad: &PadGeom,
    outer_r: f32,
    spawn_cx: Vec2,
    zone: crate::app::types::zone::PadZone,
) -> Vec2 {
    if zone.to_id() <= 8 {
        let idx = (zone.to_id() - 1) as f32;
        let ang =
            -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
        let dir = vec2(ang.cos(), ang.sin());
        let target_r = outer_r + params::tap_target_offset();
        vec2(spawn_cx.x + dir.x * target_r, spawn_cx.y + dir.y * target_r)
    } else {
        app.pad_svg
            .as_ref()
            .and_then(|svg| svg.zone_screen_centroid(zone, pad))
            .unwrap_or(vec2(pad.cx, pad.cy))
    }
}
