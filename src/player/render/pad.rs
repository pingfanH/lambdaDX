//! Static pad chrome: panel background, the disc, the SVG zones, the outer
//! circle / occluding ring, and the A-ring tap indicators. No notes here.

use macroquad::color::{Color, WHITE};
use macroquad::math::{Vec2, vec2};
use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::{
    DrawTextureParams, draw_circle, draw_circle_lines, draw_line, draw_mesh, draw_rectangle,
    draw_rectangle_lines, draw_text, draw_texture_ex, measure_text,
};

use crate::app::pad_svg;
use crate::app::params;
use crate::app::types::zone::PadZone;
use crate::app::types::{PAD_ROTATION_RAD, PadGeom, RectF};
use crate::player::render::PadSurface;
use crate::player::state::PadPreviewState;

/// Panel surface: optional fill, dot grid and 1px border, drawn before the pad.
/// A background video, when present, replaces this entirely.
pub fn draw_surface(rect: RectF, surface: PadSurface, scale: f32) {
    if let Some(fill) = surface.fill {
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    }
    if let Some(color) = surface.dots {
        draw_dots(rect, 26.0 * scale, color);
    }
    if let Some(border) = surface.border {
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    }
}

/// Sparse dot grid underlay.
fn draw_dots(rect: RectF, spacing: f32, color: Color) {
    let spacing = spacing.max(4.0);
    let mut y = rect.y + spacing * 0.5;
    while y < rect.y + rect.h {
        let mut x = rect.x + spacing * 0.5;
        while x < rect.x + rect.w {
            draw_rectangle(x, y, 1.0, 1.0, color);
            x += spacing;
        }
        y += spacing;
    }
}

/// The dark disc the zones sit on.
pub fn draw_pad_disc(cx: f32, cy: f32, outer_r: f32) {
    draw_circle(cx, cy, outer_r, Color::from_rgba(35, 35, 35, 255));
}

/// Background video frame, cover-fit into `rect` then zoomed/offset by the
/// `bg_video_*` params. No-op until a frame has been decoded.
pub fn draw_video_background(app: &PadPreviewState, rect: RectF, scale: f32) {
    let Some(tex) = app.video_bg.texture() else {
        return;
    };
    let a = params::bg_video_alpha().clamp(0.0, 255.0) / 255.0;
    if a <= 0.0 {
        return;
    }
    let (tw, th) = (tex.width(), tex.height());
    if tw <= 0.0 || th <= 0.0 {
        return;
    }
    let cover = (rect.w / tw).max(rect.h / th);
    let zoom = params::bg_video_scale().max(0.01);
    let w = tw * cover * zoom;
    let h = th * cover * zoom;
    let cx = rect.x + rect.w * 0.5 + params::bg_video_x() * scale;
    let cy = rect.y + rect.h * 0.5 + params::bg_video_y() * scale;
    draw_texture_ex(
        tex,
        cx - w * 0.5,
        cy - h * 0.5,
        Color::new(1.0, 1.0, 1.0, a),
        DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            ..Default::default()
        },
    );
}

/// Circular cover art drawn inside the disc (square texture, circular UV fan).
pub fn draw_cover(app: &PadPreviewState, cx: f32, cy: f32, r: f32) {
    let Some(tex) = &app.cover_texture else {
        return;
    };
    const SEGS: usize = 72;
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(SEGS + 2),
        indices: Vec::with_capacity(SEGS * 3),
        texture: Some(tex.clone()),
    };
    mesh.vertices.push(Vertex::new(cx, cy, 0.0, 0.5, 0.5, WHITE));
    for i in 0..=SEGS {
        let a = i as f32 / SEGS as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (s, c) = a.sin_cos();
        mesh.vertices.push(Vertex::new(cx + c * r, cy + s * r, 0.0, 0.5 + c * 0.5, 0.5 + s * 0.5, WHITE));
    }
    for i in 1..=SEGS as u16 {
        mesh.indices.extend_from_slice(&[0, i, i + 1]);
    }
    draw_mesh(&mesh);
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

/// Draw every SVG zone polygon. Transparent graphite fill per the reference;
/// active (touched) zones tint blue, recent hits pulse warm.
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
                Color::from_rgba(74, 125, 170, 190),
                Color::from_rgba(120, 170, 220, 255),
            )
        } else if is_feedback {
            (
                Color::from_rgba(230, 149, 48, 170),
                Color::from_rgba(240, 180, 80, 255),
            )
        } else {
            // Match the reference SVG exactly: `fill="#3A3A3D" stroke="#4A4A4F"`
            // with `opacity="0.58"` on both.
            (
                Color::from_rgba(0x3a, 0x3a, 0x3d, 148),
                Color::from_rgba(0x4a, 0x4a, 0x4f, 148),
            )
        };

        pad_svg::draw_polygon_fill(&screen_verts, fill_color);
        pad_svg::draw_polygon_lines(&screen_verts, 1.0 * scale, stroke_color);

        let text_color = if is_active || is_feedback {
            WHITE
        } else {
            Color::from_rgba(200, 200, 205, 200)
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

/// Big circle just outside the sensor zones plus an opaque ring that occludes
/// anything drawn beyond it. Drawn **after** the notes. The occluding ring is
/// always kept; only the inner disc is dropped when a background video is
/// active (so the video shows inside the pad but notes outside are still
/// hidden).
pub fn draw_outer_circle(pad: &PadGeom, scale: f32) {
    let r = pad.outer_r * params::pad_circle_scale().max(1.0);
    draw_circle_lines(pad.cx, pad.cy, r, 1.5 * scale, Color::from_rgba(0x4a, 0x4a, 0x4f, 255));

    let a = params::pad_outside_alpha().clamp(0.0, 255.0) / 255.0;
    if a > 0.0 {
        let big = (pad.cx.max(pad.cy) + r) * 2.0;
        draw_annulus(
            pad.cx,
            pad.cy,
            r,
            big,
            Color::new(30.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0, a),
        );
    }
}

/// Filled annulus (inner radius `ri`, outer radius `ro`) as a triangle strip.
fn draw_annulus(cx: f32, cy: f32, ri: f32, ro: f32, color: Color) {
    const SEGS: usize = 96;
    let mut mesh = Mesh {
        vertices: Vec::with_capacity((SEGS + 1) * 2),
        indices: Vec::with_capacity(SEGS * 6),
        texture: None,
    };
    for i in 0..=SEGS {
        let a = i as f32 / SEGS as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        mesh.vertices.push(Vertex::new(cx + c * ri, cy + s * ri, 0.0, 0.0, 0.0, color));
        mesh.vertices.push(Vertex::new(cx + c * ro, cy + s * ro, 0.0, 0.0, 0.0, color));
    }
    for i in 0..SEGS as u16 {
        let b = i * 2;
        mesh.indices.extend_from_slice(&[b, b + 1, b + 2, b + 1, b + 3, b + 2]);
    }
    draw_mesh(&mesh);
}

/// The eight dots at the ring positions plus a light connecting octagon arc.
pub fn draw_ring_indicators(spawn_cx: Vec2, outer_r: f32, scale: f32) {
    let dot_r = outer_r + params::tap_ring_offset();
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

// Keep the old import used even when the cover path is skipped.
#[allow(dead_code)]
fn _keep_draw_texture_ex_used(tex: &macroquad::texture::Texture2D, x: f32, y: f32) {
    draw_texture_ex(tex, x, y, WHITE, DrawTextureParams::default());
}
