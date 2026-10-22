//! Flat drawing primitives: panels, cut corners, hairlines, text, bars and the
//! faint grid/hatch underlays.

use macroquad::prelude::*;
use std::cell::Cell;

use crate::app::types::RectF;

thread_local! {
    static CLIP: Cell<Option<RectF>> = const { Cell::new(None) };
    static SCREEN_CLIP_ONLY: Cell<bool> = const { Cell::new(false) };
}

fn physical_scissor(r: RectF, width: f32, height: f32, dpi: f32) -> (i32, i32, i32, i32) {
    let x = (r.x.clamp(0.0, width) * dpi).floor() as i32;
    let y = (r.y.clamp(0.0, height) * dpi).floor() as i32;
    let right = ((r.x + r.w.max(0.0)).clamp(0.0, width) * dpi).ceil() as i32;
    let bottom = ((r.y + r.h.max(0.0)).clamp(0.0, height) * dpi).ceil() as i32;
    (x, y, (right - x).max(0), (bottom - y).max(0))
}

pub fn is_visible(r: RectF) -> bool {
    r.w > 0.0 && r.h > 0.0 && (SCREEN_CLIP_ONLY.with(Cell::get) || CLIP.with(|clip| {
        clip.get().is_none_or(|c| r.x < c.x + c.w && r.x + r.w > c.x
            && r.y < c.y + c.h && r.y + r.h > c.y)
    }))
}

pub fn contains_visible(pos: Vec2) -> bool {
    SCREEN_CLIP_ONLY.with(Cell::get)
        || CLIP.with(|clip| clip.get().is_none_or(|r| crate::app::ui::rect_contains(r, pos)))
}

pub struct ClipGuard(Option<RectF>, bool);

fn apply_clip(r: Option<RectF>) {
    let scissor = r.map(|r| physical_scissor(r, screen_width(), screen_height(), screen_dpi_scale()));
    unsafe { get_internal_gl().quad_gl.scissor(scissor); }
}

pub fn clip(r: RectF) -> ClipGuard {
    let previous = CLIP.with(|clip| {
        let previous = clip.get();
        let r = if let Some(c) = previous {
            let x = r.x.max(c.x);
            let y = r.y.max(c.y);
            RectF { x, y, w: ((r.x + r.w).min(c.x + c.w) - x).max(0.0),
                h: ((r.y + r.h).min(c.y + c.h) - y).max(0.0) }
        } else { r };
        clip.set(Some(r));
        apply_clip(Some(r));
        previous
    });
    ClipGuard(previous, SCREEN_CLIP_ONLY.with(Cell::get))
}

/// Clip transformed drawing in screen space without comparing its local
/// geometry with the screen-space rectangle. Input needs its own mapping.
pub fn screen_clip(r: RectF) -> ClipGuard {
    let guard = clip(r);
    SCREEN_CLIP_ONLY.with(|screen_only| screen_only.set(true));
    guard
}

impl Drop for ClipGuard {
    fn drop(&mut self) {
        CLIP.with(|clip| clip.set(self.0));
        SCREEN_CLIP_ONLY.with(|screen_only| screen_only.set(self.1));
        apply_clip(self.0);
    }
}

// ── Color helpers ────────────────────────────────────────────────────

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub fn shade(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

pub fn rect(r: RectF, c: Color) {
    draw_rectangle(r.x, r.y, r.w, r.h, c);
}

pub fn rect_outline(r: RectF, thickness: f32, c: Color) {
    draw_rectangle_lines(r.x, r.y, r.w, r.h, thickness, c);
}

/// Hard offset rectangle — the "sticker" shadow, kept for pressed/selected
/// states only so it reads as depth rather than decoration.
pub fn shadow_rect(r: RectF, dx: f32, dy: f32, c: Color) {
    draw_rectangle(r.x + dx, r.y + dy, r.w, r.h, c);
}

pub fn line(a: Vec2, b: Vec2, thickness: f32, c: Color) {
    draw_line(a.x, a.y, b.x, b.y, thickness, c);
}

/// Convex polygon fill via a triangle fan.
pub fn fill_poly(points: &[Vec2], c: Color) {
    if points.len() < 3 {
        return;
    }
    for i in 1..points.len() - 1 {
        draw_triangle(points[0], points[i], points[i + 1], c);
    }
}

pub fn polyline(points: &[Vec2], thickness: f32, c: Color) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        line(pair[0], pair[1], thickness, c);
    }
    if let (Some(first), Some(last)) = (points.first(), points.last()) {
        line(*last, *first, thickness, c);
    }
}

pub fn cut_rect_points(r: RectF, cut: f32) -> [Vec2; 5] {
    let cut = cut.min(r.w * 0.5).min(r.h * 0.5);
    [
        vec2(r.x, r.y),
        vec2(r.x + r.w - cut, r.y),
        vec2(r.x + r.w, r.y + cut),
        vec2(r.x + r.w, r.y + r.h),
        vec2(r.x, r.y + r.h),
    ]
}

/// A rectangle with its top-right corner cut (the reference site's clipped
/// paper), filled and optionally outlined.
pub fn cut_rect(r: RectF, cut: f32, fill: Color, border: Option<Color>, t: f32) {
    let pts = cut_rect_points(r, cut);
    fill_poly(&pts, fill);
    if let Some(b) = border {
        polyline(&pts, t, b);
    }
}

// ── Text ─────────────────────────────────────────────────────────────

pub fn text_params<'a>(font: Option<&'a Font>, size: f32, c: Color) -> TextParams<'a> {
    TextParams {
        font,
        font_size: size.round() as u16,
        font_scale: 1.0,
        color: c,
        ..Default::default()
    }
}

pub fn text(font: Option<&Font>, s: &str, x: f32, y: f32, size: f32, c: Color) {
    if !is_visible(RectF { x, y: y - size * 1.2, w: f32::MAX, h: size * 1.6 }) {
        return;
    }
    draw_text_ex(s, x, y, text_params(font, size, c));
}

pub fn text_ellipsized(font: Option<&Font>, s: &str, x: f32, y: f32, size: f32, c: Color, max_width: f32) {
    if max_width <= 0.0 { return; }
    if text_width(font, s, size) <= max_width {
        text(font, s, x, y, size, c);
        return;
    }
    let suffix = "…";
    let available = max_width - text_width(font, suffix, size);
    if available < 0.0 { return; }
    let boundaries: Vec<usize> = s.char_indices().map(|(i, _)| i).chain([s.len()]).collect();
    let mut low = 0;
    let mut high = boundaries.len() - 1;
    while low < high {
        let mid = (low + high + 1) / 2;
        if text_width(font, &s[..boundaries[mid]], size) <= available { low = mid; }
        else { high = mid - 1; }
    }
    text(font, &format!("{}{suffix}", &s[..boundaries[low]]), x, y, size, c);
}

pub fn text_width(font: Option<&Font>, s: &str, size: f32) -> f32 {
    measure_text(s, font, size.round() as u16, 1.0).width
}

/// Draw text centred horizontally on `cx`, baseline at `y`.
pub fn text_centered(font: Option<&Font>, s: &str, cx: f32, y: f32, size: f32, c: Color) -> f32 {
    let w = text_width(font, s, size);
    text(font, s, cx - w * 0.5, y, size, c);
    w
}

/// Draw text right-aligned so its end sits at `x_right`.
pub fn text_right(font: Option<&Font>, s: &str, x_right: f32, y: f32, size: f32, c: Color) -> f32 {
    let w = text_width(font, s, size);
    text(font, s, x_right - w, y, size, c);
    w
}

/// Text rotated about `(x, y)` (radians). Used sparingly for tilted accents.
pub fn text_rot(
    font: Option<&Font>,
    s: &str,
    x: f32,
    y: f32,
    size: f32,
    c: Color,
    rotation: f32,
) {
    draw_text_ex(
        s,
        x,
        y,
        TextParams {
            font,
            font_size: size.round() as u16,
            font_scale: 1.0,
            color: c,
            rotation,
            ..Default::default()
        },
    );
}

// ── Composed bits ────────────────────────────────────────────────────

/// Hairline divider across a rect at `y`.
pub fn hairline(r: RectF, y: f32) {
    draw_rectangle(r.x, y, r.w, 1.0, crate::player_ui::theme::BORDER_SOFT);
}

pub fn pill(
    font: Option<&Font>,
    r: RectF,
    fill: Color,
    label: &str,
    label_color: Color,
    size: f32,
) {
    let radius = r.h * 0.5;
    draw_rectangle(r.x + radius, r.y, r.w - radius * 2.0, r.h, fill);
    draw_circle(r.x + radius, r.y + radius, radius, fill);
    draw_circle(r.x + r.w - radius, r.y + radius, radius, fill);
    text_centered(font, label, r.x + r.w * 0.5, r.y + r.h * 0.5 + size * 0.35, size, label_color);
}

const TRACK: Color = Color::new(0.20, 0.20, 0.22, 1.0);

/// A flat playback/scroll bar with a 1px border (shared with the pad preview).
pub fn progress_bar(r: RectF, frac: f32, fill: Color, bg: Color, border: Color) {
    crate::player::render::progress::progress_bar(r, frac, fill, bg, border);
}

/// Faint diagonal hatch clipped to `r`. `slope` is dy/dx.
pub fn hatch(r: RectF, spacing: f32, slope: f32, c: Color) {
    let spacing = spacing.max(2.0);
    let dir = vec2(1.0, slope).normalize_or_zero();
    let nrm = vec2(-dir.y, dir.x);
    let span = (r.w + r.h) * 1.2;
    let center = vec2(r.x + r.w * 0.5, r.y + r.h * 0.5);
    let steps = (span / spacing).ceil() as i32;
    for i in -steps..=steps {
        let off = nrm * (i as f32 * spacing);
        let a = center + off - dir * span;
        let b = center + off + dir * span;
        if let Some((ca, cb)) = clip_segment(a, b, r) {
            draw_line(ca.x, ca.y, cb.x, cb.y, 1.0, c);
        }
    }
}

/// Sparse dot grid underlay.
pub fn dots(r: RectF, spacing: f32, c: Color) {
    let spacing = spacing.max(4.0);
    let mut y = r.y + spacing * 0.5;
    while y < r.y + r.h {
        let mut x = r.x + spacing * 0.5;
        while x < r.x + r.w {
            draw_rectangle(x, y, 1.0, 1.0, c);
            x += spacing;
        }
        y += spacing;
    }
}

/// Liang–Barsky segment/rect clip.
fn clip_segment(a: Vec2, b: Vec2, r: RectF) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-d.x, a.x - r.x),
        (d.x, r.x + r.w - a.x),
        (-d.y, a.y - r.y),
        (d.y, r.y + r.h - a.y),
    ] {
        if p.abs() < 1e-6 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((a + d * t0, a + d * t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retina_scissor_matches_the_entire_logical_viewport() {
        let r = RectF { x: 0.0, y: 156.0, w: 420.0, h: 604.0 };
        assert_eq!(physical_scissor(r, 1280.0, 760.0, 2.0), (0, 312, 840, 1208));
        assert_eq!(physical_scissor(r, 1280.0, 760.0, 1.0), (0, 156, 420, 604));
    }

    #[test]
    fn scissor_clamps_both_edges_including_offscreen_rects() {
        assert_eq!(physical_scissor(RectF { x: -5.5, y: 750.5, w: 40.0, h: 30.0 },
            1280.0, 760.0, 2.0), (0, 1501, 69, 19));
        assert_eq!(physical_scissor(RectF { x: 1400.0, y: 800.0, w: 40.0, h: 30.0 },
            1280.0, 760.0, 2.0), (2560, 1520, 0, 0));
    }

    #[test]
    fn offscreen_settings_are_not_visible_or_interactive() {
        CLIP.with(|clip| clip.set(Some(RectF { x: 240.0, y: 146.0, w: 1040.0, h: 614.0 })));
        assert!(is_visible(RectF { x: 272.0, y: 150.0, w: 620.0, h: 56.0 }));
        assert!(!is_visible(RectF { x: 272.0, y: 900.0, w: 620.0, h: 56.0 }));
        assert!(!contains_visible(vec2(300.0, 120.0)));
        CLIP.with(|clip| clip.set(None));
    }

    #[test]
    fn screen_scissor_does_not_cull_transformed_local_geometry() {
        CLIP.with(|clip| clip.set(Some(RectF { x: 0.0, y: 64.0, w: 1280.0, h: 696.0 })));
        let local_button = RectF { x: 16.0, y: 13.0, w: 84.0, h: 40.0 };
        assert!(!is_visible(local_button));
        SCREEN_CLIP_ONLY.with(|screen_only| screen_only.set(true));
        assert!(is_visible(local_button));
        SCREEN_CLIP_ONLY.with(|screen_only| screen_only.set(false));
        CLIP.with(|clip| clip.set(None));
    }
}
