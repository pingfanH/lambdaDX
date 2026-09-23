//! Touch-zone notes (`zone > 8`): the screen `B/C/D/E` areas.
//!
//! Unlike ring notes these do not fly in radially. They fade in at the zone
//! centroid over a whole duration derived from the touch speed, then their four
//! cross arms slide inward (`TOUCH_START_DIST` → `TOUCH_END_DIST`). Touch-holds
//! add four diagonal sprites and a shader-driven progress ring.

use macroquad::color::{Color, WHITE};
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{DrawTextureParams, draw_texture_ex};

use crate::app::types::zone::PadZone;
use crate::app::types::{PadGeom, hold_tail_time, touch_whole_duration};
use crate::player::render::timing::NoteTiming;
use crate::player::render::skin;
use crate::player::state::PadPreviewState;
use crate::app::params;

/// Draw a touch or touch-hold note at its zone centroid.
pub fn draw(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    t: &NoteTiming,
    pad: &PadGeom,
    scale: f32,
    current_t: f32,
) {
    let bpms = &app.chart.bpms;
    let Some(center) = app
        .pad_svg
        .as_ref()
        .and_then(|svg| svg.zone_screen_centroid(PadZone::from(t.zone), pad))
    else {
        return;
    };

    // Whole-duration progress: 0 when the note first appears, 1 at hit time.
    // `raw` is the remaining fraction; `progress` is eased.
    let travel = touch_whole_duration(t.touch_flight);
    let raw = (travel - t.dt_scaled) / travel;
    let progress = smoothstep(raw.clamp(0.0, 1.0));

    // Phases:
    //   1. fade in  [0, fade_frac)          — appears, no movement
    //   2. stall    [fade_frac, GROW_FRAC)  — fully opaque, still held at the
    //                                          outer distance (brief pause)
    //   3. move in  [GROW_FRAC, 1]          — arms travel inward, slow -> fast
    //
    // The stall is taken out of the fade-in, so the inward-motion window
    // (`GROW_FRAC..1`, the same 75% as before) keeps its duration.
    let fade_frac = (params::touch_grow_frac() - params::touch_stall_frac()).max(0.01);
    let alpha = if progress < fade_frac {
        (progress / fade_frac * 255.0) as u8
    } else {
        255
    };
    let move_progress = if progress < params::touch_grow_frac() {
        0.0
    } else {
        ((progress - params::touch_grow_frac()) / (1.0 - params::touch_grow_frac())).clamp(0.0, 1.0)
    };
    // Slow -> fast: the speed ramps linearly from 0.5x (outer half, slower) to
    // 1.5x (inner half, faster), averaging 1x so the total time is unchanged.
    // eased(t) = t·(0.5 + 0.5t) is the integral of v(t) = 0.5 + t.
    let ramp = params::touch_move_ramp();
    let move_eased = move_progress * ((1.0 - ramp) + ramp * move_progress);
    let dist = (params::touch_start_dist() + (params::touch_end_dist() - params::touch_start_dist()) * move_eased)
        * params::touch_scale()
        * scale;
    let ts = params::touch_cross_size() * params::touch_scale() * scale;

    // ── Regular touch cross (not for holds) ──
    if !matches!(note.note_type, crate::app::types::NoteType::Hold) {
        let tri_tex = skin::body_or_normal(
            app,
            skin::SkinKind::TouchTri,
            skin::SkinVariant::of(note),
        );
        if let Some(tex) = tri_tex {
            let ratio = tex.width() / tex.height();
            let tw = ts;
            let th = ts / ratio;
            let draw_tri = |cx: f32, cy: f32, rot: f32| {
                draw_texture_ex(
                    tex,
                    cx - tw * 0.5,
                    cy - th * 0.5,
                    Color::from_rgba(255, 255, 255, alpha),
                    DrawTextureParams {
                        dest_size: Some(vec2(tw, th)),
                        rotation: rot,
                        ..Default::default()
                    },
                );
            };
            draw_tri(center.x, center.y + dist, 0.0);
            draw_tri(center.x, center.y - dist, std::f32::consts::PI);
            draw_tri(center.x - dist, center.y, std::f32::consts::FRAC_PI_2);
            draw_tri(center.x + dist, center.y, -std::f32::consts::FRAC_PI_2);
        }
    }

    // Centre dot (holds draw theirs on top later).
    if !matches!(note.note_type, crate::app::types::NoteType::Hold) {
        let pt_tex = skin::body_or_normal(
            app,
            skin::SkinKind::TouchPoint,
            skin::SkinVariant::of(note),
        );
        if let Some(tex) = pt_tex {
            let ps = ts * 0.4;
            draw_texture_ex(
                tex,
                center.x - ps * 0.5,
                center.y - ps * 0.5,
                Color::from_rgba(255, 255, 255, alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(ps, ps)),
                    ..Default::default()
                },
            );
        }
    }

    if matches!(note.note_type, crate::app::types::NoteType::Hold) {
        draw_touch_hold(app, note, t, bpms, center, alpha, move_eased, current_t, scale);
    }
}

/// Touch-hold: 4 diagonal sprites + a circular progress border.
fn draw_touch_hold(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    t: &NoteTiming,
    bpms: &[crate::app::types::BpmChange],
    center: Vec2,
    alpha: u8,
    move_amount: f32,
    current_t: f32,
    scale: f32,
) {
    // Progress through the hold (0..1), used to sweep the border.
    let hold_progress = ((current_t - t.ns) / (hold_tail_time(note, bpms) - t.ns).max(0.01))
        .clamp(0.0, 1.0);
    let hold_dist = (params::touchhold_start_dist()
        + (params::touchhold_end_dist() - params::touchhold_start_dist()) * move_amount)
        * params::touchhold_scale()
        * scale;
    let d = hold_dist * 0.707; // √2/2 for the diagonal arms
    let hts = params::touchhold_cross_base() * params::touchhold_scale() * scale;
    let ro = params::touchhold_rot_offset();

    // Four arms, 45° off the regular touch cross, starting top-right.
    let positions = [
        (
            center.x + d,
            center.y - d,
            -3.0 * std::f32::consts::FRAC_PI_4 + ro,
        ),
        (center.x + d, center.y + d, -std::f32::consts::FRAC_PI_4 + ro),
        (center.x - d, center.y + d, std::f32::consts::FRAC_PI_4 + ro),
        (
            center.x - d,
            center.y - d,
            3.0 * std::f32::consts::FRAC_PI_4 + ro,
        ),
    ];
    for (i, (px, py, rot)) in positions.iter().enumerate() {
        if let Some(tex) = &app.touchhold_tex[i] {
            let ratio = tex.width() / tex.height();
            let tw = hts;
            let th = hts / ratio;
            draw_texture_ex(
                tex,
                px - tw * 0.5,
                py - th * 0.5,
                Color::from_rgba(255, 255, 255, alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(tw, th)),
                    rotation: *rot,
                    ..Default::default()
                },
            );
        }
    }

    // Progress border. The first (transparent) pass is a ghost so the shader
    // only paints the swept portion; `progress` drives the mask shader.
    if let Some(border) = &app.touchhold_border_tex {
        let bs = params::touchhold_border_base() * params::touchhold_scale() * scale;
        draw_texture_ex(
            border,
            center.x - bs * 0.5,
            center.y - bs * 0.5,
            Color::from_rgba(255, 255, 255, 0),
            DrawTextureParams {
                dest_size: Some(vec2(bs, bs)),
                ..Default::default()
            },
        );
        if let Some(ref mat) = app.mask_material {
            macroquad::material::gl_use_material(mat);
            mat.set_uniform("progress", hold_progress);
        }
        draw_texture_ex(
            border,
            center.x - bs * 0.5,
            center.y - bs * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(bs, bs)),
                ..Default::default()
            },
        );
        if app.mask_material.is_some() {
            macroquad::material::gl_use_default_material();
        }
    }

    // Centre dot on top for holds.
    let pt_tex = skin::body_or_normal(
        app,
        skin::SkinKind::TouchPoint,
        skin::SkinVariant::of(note),
    );
    if let Some(tex) = pt_tex {
        let ps = hts * 0.4;
        draw_texture_ex(
            tex,
            center.x - ps * 0.5,
            center.y - ps * 0.5,
            Color::from_rgba(255, 255, 255, alpha),
            DrawTextureParams {
                dest_size: Some(vec2(ps, ps)),
                ..Default::default()
            },
        );
    }
}

/// Hermite smoothstep on `[0, 1]`.
fn smoothstep(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
