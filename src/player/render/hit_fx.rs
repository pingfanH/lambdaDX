//! Tap-hit effect.
//!
//! When [`params::hit_fx_anim`] is on, the hit plays a movie from the loaded
//! Animate project (`app::anim`, e.g. `ui.get("tap_perfect")`), tinted per judge
//! grade. Otherwise it falls back to the procedural ring+sparks+flash below.

use macroquad::color::Color;
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{draw_circle, draw_circle_lines, draw_line};

use crate::app::params;
use crate::app::types::PadGeom;
use crate::player::render::feedback::zone_anchor;
use crate::player::state::PadPreviewState;

/// Draw every live hit effect. `scale` is the pad/UI scale.
pub fn draw(app: &PadPreviewState, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2, scale: f32) {
    if !params::hit_fx() || app.hit_fx.is_empty() {
        return;
    }
    if params::hit_fx_anim() && draw_flash(app, pad, outer_r, spawn_cx, scale) {
        return;
    }
    draw_procedural(app, pad, outer_r, spawn_cx, scale);
}

// ── Animate movie playback ────────────────────────────────────────────────

/// Play the Animate movie for every live hit. Returns `false` (so the caller
/// falls back to the procedural effect) when the project/clip is unavailable.
fn draw_flash(app: &PadPreviewState, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2, scale: f32) -> bool {
    crate::app::anim::with(|ui| {
        let Some(ui) = ui else {
            return false;
        };
        // Reference the movie by name inside the loaded project.
        let clip_name = params::hit_fx_anim_clip();
        let Some(clip) = ui.get(&clip_name) else {
            return false;
        };
        let frames = clip.frames().max(1);
        let now = app.fx_clock();
        // The procedural default size (46) maps to scale 1.
        let draw_scale = scale * (params::hit_fx_size() / 46.0).max(0.05);
        let tint_by_grade = params::hit_fx_anim_tint();

        for hit in &app.hit_fx {
            let elapsed = (now - hit.started) as f32;
            if elapsed < 0.0 || elapsed >= hit.duration {
                continue;
            }
            let p = (elapsed / hit.duration).clamp(0.0, 1.0);
            let frame = ((p * frames as f32) as usize).min(frames - 1);
            let pos = zone_anchor(app, pad, outer_r, spawn_cx, hit.zone);
            let xf = macroanimate::XflDrawXf {
                pos: (pos.x, pos.y),
                scale: draw_scale,
                rotation: 0.0,
                alpha: 1.0,
                tint: tint_by_grade.then(|| grade_rgb(hit.color)),
            };
            clip.draw(frame, &xf);
        }
        true
    })
}

fn grade_rgb(c: Color) -> [u8; 3] {
    [
        (c.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.b * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

// ── Procedural fallback ───────────────────────────────────────────────────

fn draw_procedural(app: &PadPreviewState, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2, scale: f32) {
    let now = app.fx_clock();
    let base = params::hit_fx_size() * scale;
    let ring_w = params::hit_fx_ring() * scale;
    let grow = params::hit_fx_grow();
    let sparks = params::hit_fx_sparks().round().max(0.0) as u32;
    let spark_len = params::hit_fx_spark_len();
    let flash = params::hit_fx_flash();
    let peak = params::hit_fx_alpha().clamp(0.0, 255.0);

    for fx in &app.hit_fx {
        let elapsed = (now - fx.started) as f32;
        if elapsed < 0.0 || elapsed > fx.duration {
            continue;
        }
        let p = (elapsed / fx.duration).clamp(0.0, 1.0);
        // Ease-out fade; quick enough that overlapping hits stack visibly.
        let fade = (1.0 - p) * (1.0 - p);
        let alpha = (peak * fade) as u8;
        if alpha == 0 {
            continue;
        }
        let tint = Color::new(fx.color.r, fx.color.g, fx.color.b, alpha as f32 / 255.0);
        let pos = zone_anchor(app, pad, outer_r, spawn_cx, fx.zone);

        // Central flash: shrinks while it fades.
        let flash_r = (base * 0.85 * (1.0 - 0.55 * p) * flash).max(0.5);
        let flash_col = Color::new(
            fx.color.r,
            fx.color.g,
            fx.color.b,
            (alpha as f32 * 0.45 / 255.0).min(1.0),
        );
        draw_circle(pos.x, pos.y, flash_r, flash_col);

        // Expanding ring.
        let ring_r = base * (0.5 + grow * p);
        draw_circle_lines(pos.x, pos.y, ring_r, (ring_w * fade).max(0.5), tint);

        // Radial sparks: a rotating star burst around the hit.
        for i in 0..sparks {
            let ang = fx.seed + i as f32 * std::f32::consts::TAU / sparks as f32;
            let dir = vec2(ang.cos(), ang.sin());
            let inner = base * (0.45 + grow * p * 0.7);
            let outer = inner + base * spark_len * (0.35 + 0.65 * p);
            let (p0, p1) = (pos + dir * inner, pos + dir * outer);
            draw_line(p0.x, p0.y, p1.x, p1.y, (ring_w * fade).max(0.5), tint);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ui_project_exposes_named_clips() {
        // Parse only (no GPU): the `ui` project exposes movies by name.
        let path = format!("{}/assets/ui", env!("CARGO_MANIFEST_DIR"));
        let atlas = macroanimate::XflAtlas::load(&path).expect("load ui project");
        assert!(atlas.animations.contains_key("tap_perfect"));
        assert!(atlas.animations["tap_perfect"].frames > 1);
    }
}
