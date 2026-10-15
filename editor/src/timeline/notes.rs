//! Note drawing primitives shared by both timeline orientations.
//!
//! These are pure "draw a shape at (x, y)" helpers plus the texture-selection
//! rules (break / each / ex variants). The horizontal and vertical renderers
//! differ only in how they turn (time, lane) into screen coordinates, so the
//! visual recipes live here once.

use macroquad::prelude as mq;
use macroquad::prelude::{Color, Texture2D};
use macroquad::texture::DrawTextureParams;

use crate::app::types::{Note, NoteType, Slide};
use crate::player::state::PadPreviewState;

// Reference note metrics (macroquad_sim, desktop profile, `scale = 1.0`).
pub const TAP_SIZE: f32 = 40.0;
pub const HOLD_WIDTH: f32 = 40.0;
pub const TOUCH_TRI: f32 = 30.0;
pub const TOUCH_POINT: f32 = 12.0;
pub const STAR_SIZE: f32 = 45.0;
pub const SLIDE_TILE_SPACING: f32 = 20.0;
pub const SLIDE_TILE_SCALE: f32 = 0.4;

pub fn draw_order(notes: &[Note]) -> Vec<&Note> {
    let mut ordered: Vec<_> = notes.iter().collect();
    ordered.sort_by(|a, b| crate::app::types::note_draw_order(a, b, true));
    ordered
}

#[cfg(test)]
mod order_tests {
    use super::*;

    #[test]
    fn earlier_notes_draw_last_independent_of_insertion_order() {
        let notes = [3.0, 1.0, 4.0, 2.0].map(|time| Note { time, ..Default::default() });
        let times: Vec<_> = draw_order(&notes).iter().map(|n| n.time).collect();
        assert_eq!(times, [4.0, 3.0, 2.0, 1.0]);
    }
}

pub fn head_size(note: &Note) -> f32 {
    match note.note_type {
        NoteType::Slide => STAR_SIZE,
        NoteType::Touch => TOUCH_TRI * 1.6,
        _ => TAP_SIZE,
    }
}

pub fn selection_outline(note: &Note, x: f32, y: f32, scale: f32) {
    let size = head_size(note) * scale;
    mq::draw_rectangle_lines(x - size * 0.5, y - size * 0.5, size, size, 2.0, ca(56, 189, 248, 220));
}

pub fn c(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

pub fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

pub fn tint(color: Color, tint: Color) -> Color {
    Color::new(color.r * tint.r, color.g * tint.g, color.b * tint.b, color.a * tint.a)
}

pub fn note_tint(note: &Note, backdrop: bool) -> Color {
    if backdrop { ca(160, 170, 190, 65) }
    else if note.template_source.is_some() { ca(180, 200, 255, 220) }
    else { mq::WHITE }
}

// ── Texture selection ──────────────────────────────────────────────────────

/// Head texture for a tap: break / each variant, falling back to the base tap.
pub fn tap_tex<'a>(app: &'a PadPreviewState, note: &Note) -> Option<&'a Texture2D> {
    if note.is_break {
        app.tap_break_tex.as_ref()
    } else if note.is_each {
        app.tap_each_tex.as_ref()
    } else {
        app.tap_texture.as_ref()
    }
    .or(app.tap_texture.as_ref())
}

/// Body texture for a hold: break / each variant, falling back to the base hold.
pub fn hold_tex<'a>(app: &'a PadPreviewState, note: &Note) -> Option<&'a Texture2D> {
    if note.is_break {
        app.hold_break_tex.as_ref()
    } else if note.is_each {
        app.hold_each_tex.as_ref()
    } else {
        app.hold_texture.as_ref()
    }
    .or(app.hold_texture.as_ref())
}

/// Trail texture for one slide leg: per-leg break, else note each, else base.
pub fn slide_tex<'a>(
    app: &'a PadPreviewState,
    note: &Note,
    slide: &Slide,
) -> Option<&'a Texture2D> {
    if slide.slide_is_break {
        app.slide_break_tex.as_ref()
    } else if note.is_each {
        app.slide_each_tex.as_ref()
    } else {
        app.slide_tex.as_ref()
    }
}

/// Star head texture honouring break / each / star (double) variants.
pub fn star_tex<'a>(app: &'a PadPreviewState, note: &Note) -> Option<&'a Texture2D> {
    let double = note.is_star;
    let primary = if note.is_break {
        if double {
            app.star_double_break_tex.as_ref()
        } else {
            app.star_break_tex.as_ref()
        }
    } else if note.is_each {
        if double {
            app.star_double_each_tex.as_ref()
        } else {
            app.star_each_tex.as_ref()
        }
    } else if double {
        app.star_double_tex.as_ref()
    } else {
        app.star_tex.as_ref()
    };
    let fallback = if double {
        app.star_double_tex.as_ref()
    } else {
        app.star_tex.as_ref()
    };
    primary.or(fallback).or(app.star_tex.as_ref())
}

// ── Basic image helpers ────────────────────────────────────────────────────

/// Draw a texture centred on (cx, cy) with the given size.
pub fn draw_tex(tex: &Texture2D, cx: f32, cy: f32, w: f32, h: f32, color: Color) {
    mq::draw_texture_ex(
        tex,
        cx - w * 0.5,
        cy - h * 0.5,
        color,
        DrawTextureParams {
            dest_size: Some(mq::vec2(w, h)),
            ..Default::default()
        },
    );
}

/// Draw a texture at a top-left position with a rotation (no custom pivot).
pub fn tex_at(tex: &Texture2D, x: f32, y: f32, w: f32, h: f32, rot: f32, color: Color) {
    mq::draw_texture_ex(
        tex,
        x,
        y,
        color,
        DrawTextureParams {
            dest_size: Some(mq::vec2(w, h)),
            rotation: rot,
            ..Default::default()
        },
    );
}

/// Vector stand-in used when no tap texture is available.
pub fn fallback_tap(x: f32, y: f32, size: f32) {
    fallback_tap_tinted(x, y, size, mq::WHITE);
}

pub fn fallback_tap_tinted(x: f32, y: f32, size: f32, color: Color) {
    let r = size * 0.3125;
    mq::draw_circle(x, y, r, tint(c(17, 24, 39), color));
    mq::draw_circle_lines(x, y, r, r * 0.3, tint(c(244, 114, 182), color));
    mq::draw_circle(x, y, r * 0.3, tint(c(249, 168, 212), color));
}

/// Draw a touch note: cross of four triangles plus a centre point. Identical in
/// both orientations, so it lives here.
pub fn draw_touch(app: &PadPreviewState, s: f32, note: &Note, x: f32, y: f32) {
    draw_touch_tinted(app, s, note, x, y, mq::WHITE);
}

pub fn draw_touch_tinted(app: &PadPreviewState, s: f32, note: &Note, x: f32, y: f32, color: Color) {
    let tri = if note.is_each {
        app.touch_tri_each_tex.as_ref()
    } else {
        app.touch_tri_tex.as_ref()
    }
    .or(app.touch_tri_tex.as_ref());
    let point = if note.is_each {
        app.touch_point_each_tex.as_ref()
    } else {
        app.touch_point_tex.as_ref()
    }
    .or(app.touch_point_tex.as_ref());

    if let Some(t) = tri {
        let ratio = t.width() / t.height().max(1.0);
        let ts = TOUCH_TRI * s;
        let tw = ts;
        let th = ts / ratio.max(0.001);
        let color = tint(ca(255, 255, 255, 200), color);
        tex_at(t, x - tw * 0.5, y + ts * 0.3 - th * 0.5, tw, th, 0.0, color);
        tex_at(
            t,
            x - tw * 0.5,
            y - ts * 0.3 - th * 0.5,
            tw,
            th,
            std::f32::consts::PI,
            color,
        );
        tex_at(
            t,
            x - ts * 0.3 - tw * 0.5,
            y - th * 0.5,
            tw,
            th,
            std::f32::consts::FRAC_PI_2,
            color,
        );
        tex_at(
            t,
            x + ts * 0.3 - tw * 0.5,
            y - th * 0.5,
            tw,
            th,
            -std::f32::consts::FRAC_PI_2,
            color,
        );
    }
    if let Some(p) = point {
        let ps = TOUCH_POINT * s;
        draw_tex(p, x, y, ps, ps, tint(ca(255, 255, 255, 200), color));
    } else {
        mq::draw_circle(x, y, 3.0 * s, tint(ca(103, 232, 249, 200), color));
    }
}

// ── Hold 9-slice (both orientations) ───────────────────────────────────────

/// Horizontal 9-slice of the (vertical) hold texture: fixed left/right caps,
/// stretched middle. `x0..x1` are the head/tail judgment centres.
pub fn draw_hold_9slice_horizontal(tex: &Texture2D, x0: f32, x1: f32, y: f32, width: f32, tint: Color) {
    crate::app::ui::draw_hold_9slice_segment(
        tex, mq::vec2(x0, y), mq::vec2(x1, y), width, tint, mq::vec2(-1.0, 0.0),
    );
}

/// Vertical 9-slice of the (vertical) hold texture: fixed top/bottom caps,
/// stretched middle. `y0..y1` are the head/tail judgment centres.
pub fn draw_hold_9slice_vertical(tex: &Texture2D, cx: f32, y0: f32, y1: f32, width: f32, tint: Color) {
    crate::app::ui::draw_hold_9slice_segment(
        tex, mq::vec2(cx, y0), mq::vec2(cx, y1), width, tint, mq::vec2(0.0, 1.0),
    );
}
