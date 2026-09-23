//! Static pad chrome: panel background, the disc, the SVG zones, and the A-ring
//! tap indicators. No notes are drawn here.

use macroquad::color::{Color, WHITE};
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{draw_circle, draw_line, draw_rectangle, draw_text, measure_text};

use crate::app::pad_svg;
use crate::app::types::zone::PadZone;
use crate::app::types::{PAD_ROTATION_RAD, PadGeom, RectF};
use crate::player::state::PadPreviewState;
use crate::app::params;

/// Panel fill plus the "Pad View" label.
pub fn draw_panel_background(rect: RectF, scale: f32) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::from_rgba(38, 38, 38, 255),
    );
    draw_text(
        "Pad View",
        rect.x + 12.0 * scale,
        rect.y + 24.0 * scale,
        24.0 * scale,
        Color::from_rgba(180, 180, 180, 255),
    );
}

/// The dark disc the zones sit on.
pub fn draw_pad_disc(cx: f32, cy: f32, outer_r: f32) {
    draw_circle(cx, cy, outer_r, Color::from_rgba(35, 35, 35, 255));
}

/// White dot marking the tap spawn centre (the SVG C-zone centroid).
pub fn draw_spawn_dot(spawn_cx: Vec2, scale: f32) {
    draw_circle(
        spawn_cx.x,
        spawn_cx.y,
        3.0 * scale,
        Color::from_rgba(255, 255, 255, 180),
    );
}

/// Draw every SVG zone polygon. Active (currently touched) zones are blue;
/// zones with a recent hit pulse are orange; all others are flat grey.
pub fn draw_zones(app: &PadPreviewState, pad: &PadGeom, scale: f32) {
    let Some(ref pad_svg) = app.pad_svg else {
        return;
    };

    let active_zones: Vec<PadZone> = app.active_pointer_zones.values().copied().collect();
    let feedback_zones: Vec<PadZone> = app.pad_feedback.iter().map(|fb| fb.zone).collect();

    for def in &pad_svg.zones {
        let screen_verts = pad_svg.def_screen_verts(def, pad);
        let centroid = pad_svg.def_screen_centroid(def, pad);

        let is_active = active_zones.contains(&def.zone);
        let is_feedback = feedback_zones.contains(&def.zone);

        let (fill_color, stroke_color) = if is_active {
            (
                Color::from_rgba(74, 125, 170, 180),
                Color::from_rgba(120, 170, 220, 255),
            )
        } else if is_feedback {
            (
                Color::from_rgba(230, 149, 48, 160),
                Color::from_rgba(240, 180, 80, 255),
            )
        } else {
            (
                Color::from_rgba(50, 50, 50, 255),
                Color::from_rgba(70, 70, 70, 255),
            )
        };

        pad_svg::draw_polygon_fill(&screen_verts, fill_color);
        pad_svg::draw_polygon_lines(&screen_verts, 2.0 * scale, stroke_color);

        let text_color = if is_active || is_feedback {
            WHITE
        } else {
            Color::from_rgba(160, 160, 160, 255)
        };
        let text_size = 17.0 * scale;
        let text_dims = measure_text(&def.label, None, text_size as _, 1.0);
        draw_text(
            &def.label,
            centroid.x - text_dims.width * 0.5,
            centroid.y + text_dims.height * 0.35,
            text_size,
            text_color,
        );
    }
}

/// The eight white dots at the ring positions plus a light connecting octagon
/// arc. These mark where tap/slide notes land and travel outward.
pub fn draw_ring_indicators(spawn_cx: Vec2, outer_r: f32, scale: f32) {
    let dot_r = outer_r + params::tap_ring_offset() * scale;
    let mut a_dots: Vec<Vec2> = Vec::new();
    for i in 0..8 {
        let ang = -std::f32::consts::FRAC_PI_2
            + PAD_ROTATION_RAD
            + i as f32 * std::f32::consts::TAU / 8.0;
        a_dots.push(vec2(
            spawn_cx.x + ang.cos() * dot_r,
            spawn_cx.y + ang.sin() * dot_r,
        ));
    }

    // Arc segments between consecutive dots (8 sub-steps each).
    let arc_steps = 8;
    for i in 0..8 {
        let a0 = -std::f32::consts::FRAC_PI_2
            + PAD_ROTATION_RAD
            + i as f32 * std::f32::consts::TAU / 8.0;
        let a1 = -std::f32::consts::FRAC_PI_2
            + PAD_ROTATION_RAD
            + (i + 1) as f32 * std::f32::consts::TAU / 8.0;
        for j in 0..arc_steps {
            let t0 = j as f32 / arc_steps as f32;
            let t1 = (j + 1) as f32 / arc_steps as f32;
            let ang0 = a0 + (a1 - a0) * t0;
            let ang1 = a0 + (a1 - a0) * t1;
            draw_line(
                spawn_cx.x + ang0.cos() * dot_r,
                spawn_cx.y + ang0.sin() * dot_r,
                spawn_cx.x + ang1.cos() * dot_r,
                spawn_cx.y + ang1.sin() * dot_r,
                2.0 * scale,
                Color::from_rgba(255, 255, 255, 120),
            );
        }
    }

    for dot in &a_dots {
        draw_circle(
            dot.x,
            dot.y,
            5.0 * scale,
            Color::from_rgba(255, 255, 255, 220),
        );
    }
}
