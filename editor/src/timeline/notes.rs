//! Note drawing primitives shared by both timeline orientations.
//!
//! These are pure "draw a shape at (x, y)" helpers plus the texture-selection
//! rules (break / each / ex variants). The horizontal and vertical renderers
//! differ only in how they turn (time, lane) into screen coordinates, so the
//! visual recipes live here once.

use macroquad::prelude as mq;
use macroquad::prelude::{Color, Rect as MqRect, Texture2D};
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
    let r = size * 0.3125;
    mq::draw_circle(x, y, r, c(17, 24, 39));
    mq::draw_circle_lines(x, y, r, r * 0.3, c(244, 114, 182));
    mq::draw_circle(x, y, r * 0.3, c(249, 168, 212));
}

/// Draw a touch note: cross of four triangles plus a centre point. Identical in
/// both orientations, so it lives here.
pub fn draw_touch(app: &PadPreviewState, s: f32, note: &Note, x: f32, y: f32) {
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
        let color = ca(255, 255, 255, 200);
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
        draw_tex(p, x, y, ps, ps, ca(255, 255, 255, 200));
    } else {
        mq::draw_circle(x, y, 3.0 * s, ca(103, 232, 249, 200));
    }
}

// ── Hold 9-slice (both orientations) ───────────────────────────────────────

/// Horizontal 9-slice of the (vertical) hold texture: fixed left/right caps,
/// stretched middle. `x0..x1` are the head/tail judgment centres.
pub fn draw_hold_9slice_horizontal(tex: &Texture2D, x0: f32, x1: f32, y: f32, width: f32, tint: Color) {
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_len = (cap_h * (width / tex_w)).max(1.0);

    let left = x0.min(x1);
    let right = x0.max(x1);
    let center_len = (right - left).max(1.0);
    let (head_len, tail_len) = if center_len < cap_len * 2.0 {
        let squeezed = (center_len * 0.5).max(1.0);
        (squeezed, squeezed)
    } else {
        (cap_len, cap_len)
    };

    let head_center = left;
    let tail_center = right;
    let body_start = head_center + head_len * 0.5;
    let body_end = tail_center - tail_len * 0.5;
    let body_len = (body_end - body_start).max(0.0);

    hold_piece(tex, 0.0, cap_h, head_center, y, width, head_len, tint);
    if body_len > 0.0 {
        hold_piece(
            tex,
            cap_h,
            body_src_h,
            (body_start + body_end) * 0.5,
            y,
            width,
            body_len,
            tint,
        );
    }
    hold_piece(tex, tex_h - cap_h, cap_h, tail_center, y, width, tail_len, tint);
}

fn hold_piece(tex: &Texture2D, src_y: f32, src_h: f32, cx: f32, cy: f32, width: f32, len: f32, tint: Color) {
    mq::draw_texture_ex(
        tex,
        cx - width * 0.5,
        cy - len * 0.5,
        tint,
        DrawTextureParams {
            source: Some(MqRect {
                x: 0.0,
                y: src_y,
                w: tex.width(),
                h: src_h,
            }),
            dest_size: Some(mq::vec2(width, len)),
            rotation: std::f32::consts::FRAC_PI_2,
            pivot: Some(mq::vec2(width * 0.5, len * 0.5)),
            ..Default::default()
        },
    );
}

/// Vertical 9-slice of the (vertical) hold texture: fixed top/bottom caps,
/// stretched middle. `y0..y1` are the head/tail judgment centres.
pub fn draw_hold_9slice_vertical(tex: &Texture2D, cx: f32, y0: f32, y1: f32, width: f32, tint: Color) {
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_dest = (cap_h * (width / tex_w)).max(1.0);

    let top_center = y0.min(y1);
    let bottom_center = y0.max(y1);
    let center_len = (bottom_center - top_center).max(1.0);
    let (top_h, bottom_h) = if center_len < cap_dest * 2.0 {
        let squeezed = (center_len * 0.5).max(1.0);
        (squeezed, squeezed)
    } else {
        (cap_dest, cap_dest)
    };
    let top_y = top_center - top_h * 0.5;
    let bottom_y = bottom_center - bottom_h * 0.5;
    let body_y = top_y + top_h;
    let body_h = (bottom_y - body_y).max(0.0);
    let x = cx - width * 0.5;

    v_piece(tex, x, top_y, 0.0, cap_h, tex_w, width, top_h, tint);
    if body_h > 0.0 {
        v_piece(tex, x, body_y, cap_h, body_src_h, tex_w, width, body_h, tint);
    }
    v_piece(tex, x, bottom_y, tex_h - cap_h, cap_h, tex_w, width, bottom_h, tint);
}

#[allow(clippy::too_many_arguments)]
fn v_piece(tex: &Texture2D, x: f32, y: f32, src_y: f32, src_h: f32, src_w: f32, w: f32, h: f32, tint: Color) {
    mq::draw_texture_ex(
        tex,
        x,
        y,
        tint,
        DrawTextureParams {
            source: Some(MqRect {
                x: 0.0,
                y: src_y,
                w: src_w,
                h: src_h,
            }),
            dest_size: Some(mq::vec2(w, h)),
            ..Default::default()
        },
    );
}
