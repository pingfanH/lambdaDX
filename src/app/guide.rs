//! Shared "guide" texture drawing for notes.
//!
//! Each note kind (tap / hold head / hold tail / star) and each skin variant
//! (normal / each / break) can have a guide texture drawn underneath it, all
//! with the same rules:
//!
//! * **pixel-aligned** with the note's own texture (1 guide texel == 1 note
//!   texel): `size = base_px * guide.width() / note_tex.width()`;
//! * size does **not** follow the note's spawn scale; it only grows with the
//!   note's travel progress (`params::tap_guide_grow`);
//! * anchored on the note by `params::tap_guide_anchor` (0 = top edge, 0.5 =
//!   centre, 1 = bottom edge) so scaling keeps that point fixed;
//! * oriented along the flight direction `ang` plus `params::tap_guide_rot`,
//!   with `tap_guide_off_x/off_y` offsets.

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::{Color, Texture2D, draw_mesh, vec2};

use crate::app::params;

/// Draw `guide` under a note sprite.
///
/// * `note_tex` — the note's own texture, used for the pixel ratio.
/// * `base_px` — the note's configured on-screen size (e.g. `tap_size * scale`).
/// * `note_scale` — the note's current size factor (spawn scale); the guide
///   scales with it so it stays consistent with the note. Pass `1.0` to keep a
///   fixed size.
/// * `ang` — flight direction (radians); the guide is oriented along it.
/// * `progress` — the note's travel progress (0..1) for `tap_guide_grow`.
pub fn draw(
    guide: &Texture2D,
    note_tex: Option<&Texture2D>,
    base_px: f32,
    px: f32,
    py: f32,
    ang: f32,
    progress: f32,
    scale: f32,
    note_scale: f32,
) {
    let alpha = params::tap_guide_alpha();
    if alpha <= 0.0 {
        return;
    }
    let px_ratio = match note_tex {
        Some(t) if t.width() > 0.0 => guide.width() / t.width(),
        _ => 1.0,
    };
    let base = base_px * px_ratio * params::tap_guide_size() * note_scale.max(0.0);
    let size = base * (1.0 + params::tap_guide_grow() * progress.clamp(0.0, 1.0));
    let aspect = if guide.width() > 0.0 {
        guide.height() / guide.width()
    } else {
        1.0
    };
    let tw = size;
    let th = size * aspect;

    let a = ang + params::tap_guide_rot();
    let u = vec2(a.cos(), a.sin()); // flight direction (outward)
    let p = vec2(-u.y, u.x);
    let half_w = tw * 0.5;
    // Anchor a point at `tap_guide_anchor` along the guide's length on the note
    // centre, plus the offset (perpendicular `off_x`, along-flight `off_y`).
    let off = p * (params::tap_guide_off_x() * scale) + u * (params::tap_guide_off_y() * scale);
    let top = vec2(px, py) + off + u * (th * params::tap_guide_anchor());

    let tl = top - p * half_w;
    let tr = top + p * half_w;
    let bl = tl - u * th;
    let br = tr - u * th;

    let tint = Color::from_rgba(255, 255, 255, alpha.clamp(0.0, 255.0) as u8);
    let mesh = Mesh {
        vertices: vec![
            Vertex::new(tl.x, tl.y, 0.0, 0.0, 0.0, tint),
            Vertex::new(tr.x, tr.y, 0.0, 1.0, 0.0, tint),
            Vertex::new(br.x, br.y, 0.0, 1.0, 1.0, tint),
            Vertex::new(bl.x, bl.y, 0.0, 0.0, 1.0, tint),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(guide.clone()),
    };
    draw_mesh(&mesh);
}
