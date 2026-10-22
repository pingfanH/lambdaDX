//! Rigid local-to-screen transforms shared by rendering and pointer routing.

use macroquad::math::{Mat4, Vec2, vec2};

use crate::app::params::Params;

#[derive(Debug, Clone, Copy)]
pub struct ViewTransform {
    pub pivot: Vec2,
    pub center: Vec2,
    pub rotation: f32,
}

impl ViewTransform {
    pub fn to_screen(self, point: Vec2) -> Vec2 {
        self.center + Vec2::from_angle(self.rotation).rotate(point - self.pivot)
    }

    pub fn to_local(self, point: Vec2) -> Vec2 {
        self.pivot + self.vector_to_local(point - self.center)
    }

    pub fn vector_to_local(self, vector: Vec2) -> Vec2 {
        Vec2::from_angle(-self.rotation).rotate(vector)
    }

    pub fn matrix(self) -> Mat4 {
        Mat4::from_translation(self.center.extend(0.0))
            * Mat4::from_rotation_z(self.rotation)
            * Mat4::from_translation((-self.pivot).extend(0.0))
    }
}

pub fn pad_transform(width: f32, height: f32, p: &Params) -> ViewTransform {
    let center = vec2(width, height) * 0.5;
    ViewTransform { pivot: center, center, rotation: p.pad_rotation_deg.to_radians() }
}

pub fn hud_scale(width: f32, height: f32, requested: f32) -> f32 {
    // Compact playback controls need at least 400 design pixels along the
    // strip, including on mobile where the requested UI scale is enlarged.
    requested.min(width.min(height) / 400.0).max(0.01)
}

pub fn hud_layout(width: f32, height: f32, scale: f32, p: &Params) -> (f32, ViewTransform) {
    let angle = if p.hud_follow_pad_rotation { p.pad_rotation_deg } else { p.hud_rotation_deg };
    let rotation = angle.to_radians();
    let (sin, cos) = rotation.sin_cos();
    let (s, c) = (sin.abs(), cos.abs());
    let bar_h = 66.0 * scale;
    // Fit the rotated strip itself, not a rotated full-window rectangle.
    let mut bar_w = f32::MAX;
    if c > 1e-5 { bar_w = bar_w.min((width - s * bar_h) / c); }
    if s > 1e-5 { bar_w = bar_w.min((height - c * bar_h) / s); }
    let bar_w = bar_w.max(1.0);
    let bounds = vec2(c * bar_w + s * bar_h, s * bar_w + c * bar_h);
    let mut distance = f32::MAX;
    if s > 1e-5 { distance = distance.min((width - bounds.x).max(0.0) * 0.5 / s); }
    if c > 1e-5 { distance = distance.min((height - bounds.y).max(0.0) * 0.5 / c); }
    let outward = vec2(sin, -cos);
    let center = vec2(width, height) * 0.5 + outward * distance
        + vec2(p.hud_offset_x, p.hud_offset_y) * scale;
    (bar_w, ViewTransform { pivot: vec2(bar_w * 0.5, bar_h * 0.5), center, rotation })
}

pub struct ModelGuard;

pub fn model(transform: ViewTransform) -> ModelGuard {
    unsafe { macroquad::window::get_internal_gl().quad_gl.push_model_matrix(transform.matrix()); }
    ModelGuard
}

impl Drop for ModelGuard {
    fn drop(&mut self) {
        unsafe { macroquad::window::get_internal_gl().quad_gl.pop_model_matrix(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: Vec2, expected: Vec2) {
        assert!(actual.distance(expected) < 0.002, "{actual:?} != {expected:?}");
    }

    #[test]
    fn pad_rotation_and_render_matrix_match_inverse_pointer_mapping() {
        for angle in [0.0, 22.5, 90.0, 180.0, -90.0] {
            let p = Params { pad_rotation_deg: angle, ..Params::default() };
            let xf = pad_transform(1280.0, 760.0, &p);
            for point in [vec2(640.0, 380.0), vec2(800.0, 160.0), vec2(400.0, 560.0)] {
                let screen = xf.matrix().transform_point3(point.extend(0.0)).truncate();
                close(screen, xf.to_screen(point));
                close(xf.to_local(screen), point);
            }
        }
    }

    #[test]
    fn rotated_playback_bar_fits_landscape_and_portrait() {
        for (w, h) in [(1280.0_f32, 760.0_f32), (760.0, 1280.0), (600.0, 360.0)] {
            let scale = w.min(h) / 760.0_f32;
            for angle in [0.0, 22.5, 45.0, 90.0, 180.0, 270.0] {
                let p = Params { hud_rotation_deg: angle, ..Params::default() };
                let (bar_w, xf) = hud_layout(w, h, scale, &p);
                for point in [Vec2::ZERO, vec2(bar_w, 0.0), vec2(0.0, 66.0 * scale), vec2(bar_w, 66.0 * scale)] {
                    let screen = xf.to_screen(point);
                    assert!(screen.x >= -0.002 && screen.x <= w + 0.002, "{angle}: {screen:?}");
                    assert!(screen.y >= -0.002 && screen.y <= h + 0.002, "{angle}: {screen:?}");
                    close(xf.to_local(screen), point);
                }
            }
        }
    }

    #[test]
    fn enlarged_mobile_playback_bar_keeps_room_for_controls() {
        for (w, h) in [(360.0_f32, 800.0_f32), (800.0, 360.0), (320.0, 568.0)] {
            let scale = hud_scale(w, h, 1.6);
            for angle in [0.0, 45.0, 90.0, 180.0, 270.0] {
                let p = Params { hud_rotation_deg: angle, ..Params::default() };
                let (width, xf) = hud_layout(w, h, scale, &p);
                assert!(width / scale >= 399.9, "{w}x{h} at {angle}: {width}/{scale}");
                for point in [Vec2::ZERO, vec2(width, 66.0 * scale)] {
                    let screen = xf.to_screen(point);
                    assert!(screen.x >= -0.002 && screen.x <= w + 0.002);
                    assert!(screen.y >= -0.002 && screen.y <= h + 0.002);
                }
            }
        }
    }

    #[test]
    fn following_pad_keeps_offsets_in_screen_coordinates() {
        let p = Params { pad_rotation_deg: 90.0, hud_rotation_deg: 180.0,
            hud_follow_pad_rotation: true, hud_offset_x: -20.0, hud_offset_y: 30.0,
            ..Params::default() };
        let (width, xf) = hud_layout(1280.0, 760.0, 1.0, &p);
        assert!((width - 760.0).abs() < 0.002);
        close(xf.to_screen(Vec2::ZERO), vec2(1260.0, 30.0));
        assert!((xf.rotation.to_degrees() - 90.0).abs() < 0.002);
    }
}
