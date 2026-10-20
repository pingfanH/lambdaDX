//! Judgment sprites, with distinct outer-ring and touch-sensor anchors.

use macroquad::color::Color;
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{draw_text, measure_text};

use crate::app::params;
use crate::app::types::{PAD_ROTATION_RAD, PadGeom};
use crate::player::state::PadPreviewState;

/// Draw all live judgment labels.
pub fn draw(app: &PadPreviewState, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2, scale: f32) {
    let now = app.fx_clock();
    for feedback in &app.judge_feedback {
        let remaining = feedback.until - now;
        let age = (now - feedback.started) as f32;
        if remaining <= 0.0 || age < 0.0 {
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
        let hit = hit_anchor(
            app,
            pad,
            outer_r,
            spawn_cx,
            feedback.zone,
            feedback.is_touch,
        );
        let unit = (outer_r + params::tap_target_offset()) / 480.0;
        let dir = (hit - spawn_cx).normalize_or_zero();
        let pos = hit - dir * unit * if feedback.is_touch { 90.0 } else { 150.0 };
        let rotation = anchor_rotation(feedback.zone, feedback.is_touch);
        let pop = 1.0 + 0.3 * (1.0 - age / (1.0 / 12.0)).clamp(0.0, 1.0);
        if let Some(resources) = &app.feedback_resources {
            let key = format!("judge/UI_GAM_{}", feedback.grade.sprite_suffix(false));
            if resources.draw_sprite(&key, pos, Vec2::splat(unit * pop), rotation, color.a, age) {
                if feedback.is_break && (age * 30.0) as u32 % 2 == 1 {
                    resources.draw_sprite(
                        &format!("{key}_Break"),
                        pos,
                        Vec2::splat(unit * pop),
                        rotation,
                        color.a,
                        age,
                    );
                }
                continue;
            }
        }

        let text = feedback.grade.label().to_uppercase();
        let font_size = 36.0 * scale;
        let dims = measure_text(&text, None, font_size as _, 1.0);
        draw_text(&text, pos.x - dims.width * 0.5, pos.y, font_size, color);
    }
}

pub fn hit_anchor(
    app: &PadPreviewState,
    pad: &PadGeom,
    outer_r: f32,
    spawn_cx: Vec2,
    zone: crate::app::types::zone::PadZone,
    is_touch: bool,
) -> Vec2 {
    if is_touch {
        app.pad_svg
            .as_ref()
            .and_then(|svg| svg.zone_screen_centroid(zone, pad))
            .unwrap_or(vec2(pad.cx, pad.cy))
    } else {
        zone_anchor(app, pad, outer_r, spawn_cx, zone)
    }
}

pub fn anchor_rotation(zone: crate::app::types::zone::PadZone, is_touch: bool) -> f32 {
    let id = zone.to_id();
    let angle = if !is_touch || id <= 16 {
        id.saturating_sub(1) % 8
    } else if id == 17 {
        return 0.0;
    } else {
        id.saturating_sub(18) % 8
    } as f32
        * std::f32::consts::FRAC_PI_4;
    // Sprite local -Y faces outward, in the source's 1080px coordinate space.
    angle
        + if is_touch && id >= 18 {
            0.0
        } else {
            PAD_ROTATION_RAD
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{pad_svg::PadSvgDef, types::zone::PadZone};

    #[test]
    fn a_touch_does_not_use_the_outer_note_anchor() {
        let mut app = super::super::scale::tests::app();
        app.pad_svg =
            Some(PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg")).unwrap());
        let pad = PadGeom {
            cx: 400.0,
            cy: 400.0,
            outer_r: 300.0,
        };
        let center = vec2(pad.cx, pad.cy);
        let touch = hit_anchor(&app, &pad, pad.outer_r, center, PadZone::A1, true);
        let ring = hit_anchor(&app, &pad, pad.outer_r, center, PadZone::A1, false);
        assert!(touch.distance(ring) > 10.0);
        assert_eq!(
            touch,
            app.pad_svg
                .as_ref()
                .unwrap()
                .zone_screen_centroid(PadZone::A1, &pad)
                .unwrap()
        );
        for id in 1..=8 {
            let zone = PadZone::from(id);
            let dir =
                (hit_anchor(&app, &pad, pad.outer_r, center, zone, false) - center).normalize();
            let local_up = Vec2::from_angle(anchor_rotation(zone, false)).rotate(-Vec2::Y);
            assert!(local_up.distance(dir) < 1e-5);
        }
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
