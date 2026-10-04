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
use crate::app::types::{PadGeom, hold_tail_time, touch_motion};
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

    // Reference `TouchDrop` motion: fade in over `0.2·whole`, then the arms ease
    // inward over `0.8·whole` following `-exp(8·(t·0.43/move) − 0.85) + 0.42`.
    let Some(motion) = touch_motion(
        t.dt_scaled,
        t.touch_flight,
        params::touch_duration_scale(),
    ) else {
        return;
    };
    let alpha = (motion.alpha * 255.0) as u8;
    // `motion.distance` runs 0.4 (outer) → 0 (on the point); map it onto the
    // configurable start/end arm distances.
    let move_in = (1.0 - motion.distance / 0.4).clamp(0.0, 1.0);
    let dist = (params::touch_start_dist()
        + (params::touch_end_dist() - params::touch_start_dist()) * move_in)
        * params::touch_scale()
        * scale;
    let ts = params::touch_cross_size() * params::touch_scale() * scale;

    // ── Regular touch cross (not for holds) ──
    if !matches!(note.note_type, crate::app::types::NoteType::TouchHold) {
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
    if !matches!(note.note_type, crate::app::types::NoteType::TouchHold) {
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

    if matches!(note.note_type, crate::app::types::NoteType::TouchHold) {
        draw_touch_hold(app, note, t, bpms, center, alpha, move_in, current_t, scale);
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
    // MajdataPlay `TouchHoldDrop.SetFansPosition` uses the **same**
    // `(0.226 + distance)` fan radius as `TouchDrop` (only the angle differs:
    // diagonal vs cardinal), so the arms ease inward over the same range.
    let hold_dist = (params::touch_start_dist()
        + (params::touch_end_dist() - params::touch_start_dist()) * move_amount)
        * params::touch_scale()
        * scale;
    let d = hold_dist * 0.707; // √2/2 for the diagonal arms
    let hts = params::touchhold_cross_base() * params::touchhold_scale() * scale;
    let ro = params::touchhold_rot_offset();
    // FX touch-holds blink while held (same as ring FX).
    let tint = Color::from_rgba(255, 255, 255, (alpha as f32 * skin::fx_blink(app, note)) as u8);

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
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(tw, th)),
                    rotation: *rot,
                    ..Default::default()
                },
            );
        }
    }

    // Progress border. The reference activates it **at the head**
    // (`SetBorderActive(true)` on arrival) with progress 0, so nothing shows
    // during the fade-in/approach. The first (transparent) pass is a ghost so
    // the shader only paints the swept portion.
    if current_t >= t.ns {
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
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(ps, ps)),
                ..Default::default()
            },
        );
    }
}
