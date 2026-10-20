//! LambdaDX particle/shader effects. Animate and primitive effects remain
//! available as fallbacks when particle assets cannot load.

use macroquad::color::Color;
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{draw_circle, draw_circle_lines, draw_line};

use crate::app::params;
use crate::app::types::PadGeom;
use crate::player::feedback::{HitEffect, HitFx};
use crate::player::render::feedback::{anchor_rotation, hit_anchor};
use crate::player::state::PadPreviewState;

/// Fixed back-to-front layers, preserving spawn order within each layer.
fn ordered_hits(hits: &[HitFx]) -> impl Iterator<Item = &HitFx> {
    (0..3).flat_map(move |layer| {
        hits.iter().filter(move |hit| {
            let hit_layer = match hit.kind {
                HitEffect::Center => 0,
                HitEffect::Tap | HitEffect::ExTap => 2,
                _ => 1,
            };
            hit_layer == layer
        })
    })
}

/// Draw every live hit effect. `scale` is the pad/UI scale.
pub fn draw(app: &PadPreviewState, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2, scale: f32) {
    if !params::hit_fx() || app.hit_fx.is_empty() {
        return;
    }
    if let Some(effects) = app
        .feedback_resources
        .as_ref()
        .and_then(|resources| resources.effects.as_ref())
    {
        let now = app.fx_clock();
        let zoom = (outer_r + params::tap_target_offset()) / 480.0;
        for hit in ordered_hits(&app.hit_fx) {
            let elapsed = (now - hit.started) as f32;
            if elapsed < 0.0 || elapsed >= hit.duration {
                continue;
            }
            let Some(instance) = &hit.instance else {
                continue;
            };
            let mut instance = instance.clone();
            instance.elapsed = elapsed;
            let pos = hit_anchor(app, pad, outer_r, spawn_cx, hit.zone, hit.is_touch);
            effects.draw_placed(
                &instance,
                pos,
                zoom * hit.kind.size_scale(),
                anchor_rotation(hit.zone, hit.is_touch),
                params::hit_fx_alpha() / 255.0,
            );
        }
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
fn draw_flash(
    app: &PadPreviewState,
    pad: &PadGeom,
    outer_r: f32,
    spawn_cx: Vec2,
    scale: f32,
) -> bool {
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
        let tint_by_grade = params::hit_fx_anim_tint();

        for hit in ordered_hits(&app.hit_fx) {
            let elapsed = (now - hit.started) as f32;
            if elapsed < 0.0 || elapsed >= hit.duration {
                continue;
            }
            let p = (elapsed / hit.duration).clamp(0.0, 1.0);
            let frame = ((p * frames as f32) as usize).min(frames - 1);
            let pos = hit_anchor(app, pad, outer_r, spawn_cx, hit.zone, hit.is_touch);
            let xf = macroanimate::XflDrawXf {
                pos: (pos.x, pos.y),
                scale: scale * hit.kind.size_scale(),
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
    let ring_w = params::hit_fx_ring() * scale;
    let grow = params::hit_fx_grow();
    let sparks = params::hit_fx_sparks().round().max(0.0) as u32;
    let spark_len = params::hit_fx_spark_len();
    let flash = params::hit_fx_flash();
    let peak = params::hit_fx_alpha().clamp(0.0, 255.0);

    for fx in ordered_hits(&app.hit_fx) {
        let base = 46.0 * fx.kind.size_scale() * scale;
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
        let pos = hit_anchor(app, pad, outer_r, spawn_cx, fx.zone, fx.is_touch);

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
    use super::*;

    #[test]
    fn fireworks_stay_below_other_effects_and_tap_stays_above() {
        let kinds = [
            HitEffect::Tap, HitEffect::Touch, HitEffect::Center,
            HitEffect::ExTap, HitEffect::Hold, HitEffect::Center,
            HitEffect::Break, HitEffect::HoldRelease, HitEffect::Tap,
        ];
        let hits: Vec<_> = kinds.into_iter().enumerate().map(|(i, kind)| HitFx {
            zone: crate::app::types::zone::PadZone::A1,
            is_touch: false,
            note_id: Some(i as u64),
            kind,
            started: i as f64,
            duration: 1.0,
            color: macroquad::color::WHITE,
            seed: 0.0,
            instance: None,
        }).collect();
        let order: Vec<_> = ordered_hits(&hits).map(|hit| hit.note_id.unwrap()).collect();
        assert_eq!(order, [2, 5, 1, 4, 6, 7, 0, 3, 8]);
        assert_eq!(hits.iter().map(|hit| hit.kind).collect::<Vec<_>>(), kinds);
        assert_eq!(ordered_hits(&[]).count(), 0);
    }

    #[test]
    fn ui_project_exposes_named_clips() {
        // Parse only (no GPU): the `ui` project exposes movies by name.
        let path = format!("{}/assets/ui", env!("CARGO_MANIFEST_DIR"));
        let atlas = macroanimate::XflAtlas::load(&path).expect("load ui project");
        assert!(atlas.animations.contains_key("tap_perfect"));
        assert!(atlas.animations["tap_perfect"].frames > 1);
    }
}
