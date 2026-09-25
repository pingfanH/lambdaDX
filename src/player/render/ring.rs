//! A/B-ring notes: tap and hold. These use the radial flight model described in
//! `render/mod.rs` — fly inward-locked, grow, then travel to the outer ring.

use macroquad::color::{Color, WHITE};
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{
    DrawTextureParams, draw_circle, draw_circle_lines, draw_line, draw_texture_ex,
};

use crate::app::types::{HIT_WINDOW, HOLD_SPAWN_BODY_WIDTH_FRAC, NoteMotion, NoteType, PAD_ROTATION_RAD, note_lock_radius, note_radial_motion, note_radial_motion_continue, note_radial_motion_hold};
use crate::app::ui::draw_hold_9slice_segment;
use crate::player::render::timing::NoteTiming;
use crate::player::render::skin;
use crate::player::state::PadPreviewState;
use crate::app::params;

/// Draw a tap or hold note on one of the eight ring directions.
///
/// The head always uses the tap flight. A hold additionally draws a body whose
/// tail is pinned at the lock radius and only drains outward in the final
/// `drain_t` seconds (see the hold section in `render/mod.rs`).
pub fn draw(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    t: &NoteTiming,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
) {
    let idx = (t.zone - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let dir = vec2(ang.cos(), ang.sin());

    // Fixed radial direction per lane; `motion` gives radius + growth scale.
    //
    // Taps use the unclamped variant so they keep flying outward after the
    // judgment point instead of stopping on the ring. Holds keep the clamped
    // version (the head must stay on the ring while the body is held) and must
    // share `draw_hold`'s spawn motion, otherwise the visibility gate would
    // cull them until *after* they have already scaled up (a visible pop-in).
    let motion = match note.note_type {
        NoteType::Tap => {
            note_radial_motion_continue(t.dt_scaled, t.speed, outer_r, params::tap_target_offset())
        }
        NoteType::Hold => note_radial_motion_hold(
            t.dt_scaled,
            t.speed,
            outer_r,
            params::tap_target_offset(),
            params::hold_spawn_time_effective(),
        ),
        _ => note_radial_motion(t.dt_scaled, t.speed, outer_r, params::tap_target_offset()),
    };
    let Some(motion) = motion else {
        return;
    };
    let px = spawn_cx.x + dir.x * motion.radius;
    let py = spawn_cx.y + dir.y * motion.radius;

    if matches!(note.note_type, NoteType::Hold) {
        draw_hold(app, note, t, ang, dir, scale, spawn_cx, outer_r);
    }

    // Slide heads are drawn by the slide renderer; everything else draws a tap.
    if !matches!(note.note_type, NoteType::Hold | NoteType::Slide) {
        draw_tap(app, note, motion, ang, px, py, scale);
    }

    // A brief white ring when the note is exactly on its hit instant.
    if t.dt.abs() <= HIT_WINDOW {
        draw_circle_lines(
            px,
            py,
            params::tap_size() * 0.53 * scale,
            2.0 * scale,
            Color::from_rgba(255, 255, 255, params::judge_ring_alpha() as u8),
        );
    }
}

/// The note's judgment point(s) (tap = 1, hold = head + tail). Empty for
/// slides / touch. `judge_off_*` shifts each point along the flight direction.
pub fn judge_points(
    note: &crate::app::types::Note,
    t: &NoteTiming,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
) -> Vec<Vec2> {
    let idx = (t.zone - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let dir = vec2(ang.cos(), ang.sin());
    match note.note_type {
        NoteType::Slide | NoteType::Touch => Vec::new(),
        NoteType::Hold => {
            let lock_r = note_lock_radius(outer_r, params::tap_target_offset());
            let head_motion = note_radial_motion_hold(
                t.dt_scaled,
                t.speed,
                outer_r,
                params::tap_target_offset(),
                params::hold_spawn_time_effective(),
            )
            .unwrap_or(NoteMotion {
                radius: lock_r,
                scale: 0.0,
                progress: 0.0,
            });
            let tail_motion =
                note_radial_motion(t.tail_dt_scaled, t.speed, outer_r, params::tap_target_offset());
            // Judgment points sit at the scaling centre: during birth they stay
            // at the lock point (their own flight radii) instead of following
            // the sprite's symmetric min-body extension.
            let head_r = head_motion.radius;
            let tail_r = tail_motion.map(|m| m.radius).unwrap_or(lock_r);
            // Judgment points use the note's own flight radius + `judge_off_*`,
            // identical to taps (no texture offset).
            let ho = params::judge_off_hold() * scale;
            let to = params::judge_off_hold_end() * scale;
            vec![
                spawn_cx + dir * (head_r + ho),
                spawn_cx + dir * (tail_r + to),
            ]
        }
        NoteType::Tap => {
            if t.zone > 8 {
                return Vec::new();
            }
            let Some(m) = note_radial_motion_continue(
                t.dt_scaled,
                t.speed,
                outer_r,
                params::tap_target_offset(),
            ) else {
                return Vec::new();
            };
            let o = params::judge_off_tap() * scale;
            vec![spawn_cx + dir * (m.radius + o)]
        }
    }
}

/// Draw this note's guide(s) in a standalone pass, so every guide sits under
/// every note sprite regardless of note kind / pass ordering. Slides are
/// handled by the slide renderer.
pub fn draw_guides(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    t: &NoteTiming,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
) {
    if !params::tap_guide() {
        return;
    }
    let variant = skin::SkinVariant::of(note);
    let idx = (t.zone - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let dir = vec2(ang.cos(), ang.sin());

    match note.note_type {
        NoteType::Slide | NoteType::Touch => {}
        NoteType::Hold => {
            let lock_r = note_lock_radius(outer_r, params::tap_target_offset());
            let head_motion = note_radial_motion_hold(
                t.dt_scaled,
                t.speed,
                outer_r,
                params::tap_target_offset(),
                params::hold_spawn_time_effective(),
            )
            .unwrap_or(NoteMotion {
                radius: lock_r,
                scale: 0.0,
                progress: 0.0,
            });
            let tail_motion =
                note_radial_motion(t.tail_dt_scaled, t.speed, outer_r, params::tap_target_offset());
            // Guides follow the judgment points (scaling centre), not the
            // sprite's min-body extension.
            let head_r = head_motion.radius;
            let tail_r = tail_motion.map(|m| m.radius).unwrap_or(lock_r);
            let hx = spawn_cx.x + dir.x * head_r;
            let hy = spawn_cx.y + dir.y * head_r;
            let tx = spawn_cx.x + dir.x * tail_r;
            let ty = spawn_cx.y + dir.y * tail_r;
            let hold_tex = skin::body_or_normal(app, skin::SkinKind::Hold, variant);
            let head_guide = match variant {
                skin::SkinVariant::Each => {
                    app.tap_guide_each_tex.as_ref().or(app.tap_guide_tex.as_ref())
                }
                skin::SkinVariant::Break => {
                    app.tap_guide_break_tex.as_ref().or(app.tap_guide_tex.as_ref())
                }
                skin::SkinVariant::Normal => app.tap_guide_tex.as_ref(),
            };
            if let Some(g) = head_guide {
                let off = (params::judge_off_hold() + params::hold_guide_off()) * scale;
                crate::app::guide::draw(
                    g,
                    hold_tex,
                    params::hold_width() * scale,
                    hx + dir.x * off,
                    hy + dir.y * off,
                    ang,
                    head_motion.progress,
                    scale,
                    1.0,
                );
            }
            let tail_guide = match variant {
                skin::SkinVariant::Each => app
                    .hold_end_each_guide_tex
                    .as_ref()
                    .or(app.hold_end_guide_tex.as_ref()),
                skin::SkinVariant::Break => app
                    .hold_end_break_guide_tex
                    .as_ref()
                    .or(app.hold_end_guide_tex.as_ref()),
                skin::SkinVariant::Normal => app.hold_end_guide_tex.as_ref(),
            };
            if let Some(g) = tail_guide {
                let off = (params::judge_off_hold_end() + params::hold_end_guide_off()) * scale;
                crate::app::guide::draw(
                    g,
                    hold_tex,
                    params::hold_width() * scale,
                    tx + dir.x * off,
                    ty + dir.y * off,
                    ang,
                    tail_motion.map(|m| m.progress).unwrap_or(0.0),
                    scale,
                    1.0,
                );
            }
        }
        NoteType::Tap => {
            if t.zone > 8 {
                return;
            }
            let Some(m) = note_radial_motion_continue(
                t.dt_scaled,
                t.speed,
                outer_r,
                params::tap_target_offset(),
            ) else {
                return;
            };
            let px = spawn_cx.x + dir.x * m.radius;
            let py = spawn_cx.y + dir.y * m.radius;
            let tap_tex = skin::body_or_normal(app, skin::SkinKind::Tap, variant);
            let guide = match variant {
                skin::SkinVariant::Each => {
                    app.tap_guide_each_tex.as_ref().or(app.tap_guide_tex.as_ref())
                }
                skin::SkinVariant::Break => {
                    app.tap_guide_break_tex.as_ref().or(app.tap_guide_tex.as_ref())
                }
                skin::SkinVariant::Normal => app.tap_guide_tex.as_ref(),
            };
            if let Some(g) = guide {
                let off = params::judge_off_tap() * scale;
                crate::app::guide::draw(
                    g,
                    tap_tex,
                    params::tap_size() * scale,
                    px + dir.x * off,
                    py + dir.y * off,
                    ang,
                    m.progress,
                    scale,
                    1.0,
                );
            }
        }
    }
}

/// Hold head/tail render radii.
///
/// The bar keeps a **minimum length** proportional to the note's spawn scale, so
/// its length:width ratio stays fixed while it is born. The extra length is
/// added **outward from the head only** — the tail stays on its own flight
/// radius — so the tail never slides (no jump when the flight starts).
fn hold_render_radii(
    head: &NoteMotion,
    tail: Option<NoteMotion>,
    lock_r: f32,
    scale: f32,
) -> (f32, f32) {
    let head_r = head.radius;
    let tail_r = tail.map(|m| m.radius).unwrap_or(lock_r);
    let sep = (head_r - tail_r).max(0.0);
    let min_body = params::hold_width() * scale * head.scale * HOLD_SPAWN_BODY_WIDTH_FRAC;
    let extra = (min_body - sep).max(0.0);
    (head_r + extra, tail_r)
}

/// Tap body: a skin texture (plus an Ex overlay) or a fallback pink circle.
fn draw_tap(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    motion: NoteMotion,
    ang: f32,
    px: f32,
    py: f32,
    scale: f32,
) {
    let motion_scale = motion.scale;
    let ts = params::tap_size() * scale * motion_scale;
    let tap_tex = skin::body_or_normal(app, skin::SkinKind::Tap, skin::SkinVariant::of(note));

    if let Some(tex) = tap_tex {
        draw_texture_ex(
            tex,
            px - ts * 0.5,
            py - ts * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(ts, ts)),
                ..Default::default()
            },
        );
        if note.is_ex {
            if let Some(ex_tex) = skin::ex(app, skin::SkinKind::Tap) {
                draw_texture_ex(
                    ex_tex,
                    px - ts * 0.5,
                    py - ts * 0.5,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(ts, ts)),
                        ..Default::default()
                    },
                );
            }
        }
    } else {
        // Fallback when no skin textures are present.
        let tr = params::tap_size() * 0.375 * scale * motion_scale;
        draw_circle(px, py, tr, Color::from_rgba(17, 24, 39, 255));
        draw_circle_lines(px, py, tr, tr * 0.25, Color::from_rgba(244, 114, 182, 255));
        draw_circle(px, py, tr * 0.317, Color::from_rgba(249, 168, 212, 255));
    }
}

/// Hold body between a flying head and tail.
///
/// Both ends are evaluated with the **same** radial model at their own note
/// times — `dt_scaled` for the head, `tail_dt_scaled` for the tail. Because they
/// share `speed`, they move outward at the same radial speed, so the body
/// translates while approaching, the head parks on the ring at the hit, and the
/// tail reaches the ring exactly when the hold ends. (The previous drain model
/// moved the tail at a different speed for short holds.)
///
/// Before the tail is visible it sits at the lock radius, matching the head's
/// spawn radius; it is never allowed past the head.
fn draw_hold(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    t: &NoteTiming,
    ang: f32,
    dir: Vec2,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
) {
    let lock_r = note_lock_radius(outer_r, params::tap_target_offset());
    let head_motion = note_radial_motion_hold(
        t.dt_scaled,
        t.speed,
        outer_r,
        params::tap_target_offset(),
        params::hold_spawn_time_effective(),
    )
    .unwrap_or(NoteMotion {
        radius: lock_r,
        scale: 0.0,
        progress: 0.0,
    });

    // The hold scales uniformly from 0 to full: both its length (the min body
    // above) and its width follow `head_motion.scale`, so the aspect ratio is
    // fixed during birth and it grows while centred.
    let body_w = params::hold_width() * scale * head_motion.scale;

    // Head/tail render radii with the symmetric minimum body length.
    let tail_motion =
        note_radial_motion(t.tail_dt_scaled, t.speed, outer_r, params::tap_target_offset());
    let (head_r, tail_r) = hold_render_radii(&head_motion, tail_motion, lock_r, scale);

    // Hold sprite offset (visual only; judgment / guides unaffected).
    let hx = spawn_cx.x + dir.x * head_r;
    let hy = spawn_cx.y + dir.y * head_r;
    let tx = spawn_cx.x + dir.x * tail_r;
    let ty = spawn_cx.y + dir.y * tail_r;

    let hold_tex = skin::body_or_normal(app, skin::SkinKind::Hold, skin::SkinVariant::of(note));

    let head_pos = vec2(hx, hy);
    let tail_pos = vec2(tx, ty);
    if let Some(tex) = hold_tex {
        draw_hold_9slice_segment(
            tex,
            head_pos,
            tail_pos,
            body_w,
            Color::from_rgba(255, 255, 255, 255),
            dir,
        );
        if note.is_ex {
            if let Some(ex_tex) = skin::ex(app, skin::SkinKind::Hold) {
                draw_hold_9slice_segment(
                    ex_tex,
                    head_pos,
                    tail_pos,
                    body_w,
                    Color::from_rgba(255, 255, 255, 255),
                    dir,
                );
            }
        }
    } else {
        // Fallback line + tail dot.
        draw_line(
            hx,
            hy,
            tx,
            ty,
            params::hold_width() * 0.233 * scale * head_motion.scale,
            Color::from_rgba(251, 113, 133, 200),
        );
        draw_circle(
            tx,
            ty,
            params::hold_width() * 0.167 * scale * head_motion.scale,
            Color::from_rgba(253, 164, 175, 255),
        );
    }
}

/// Radius of the rendered hold tail.
///
/// `tail_raw`/`tail_progress` come from the tail's own radial motion (0 = still
/// pinned at the lock radius, 1 = on the judgment ring). The rendered tail is
/// pushed inward by `min_body`, but that extra offset is **anchored to the tail
/// and faded by `1 - tail_progress`**:
///
/// * while the tail is pinned (spawn/early) the offset is constant, so the tail
///   stays put and only the head moves — the bar stretches instead of the whole
///   body lurching forward;
/// * as the tail nears the ring the offset fades to 0, so the tail still lands
///   exactly on the ring at the hold end.
///
/// Never crosses the centre (`>= 0`) or the head.
fn hold_tail_radius(tail_raw: f32, tail_progress: f32, head_r: f32, min_body: f32) -> f32 {
    let extra = min_body * (1.0 - tail_progress.clamp(0.0, 1.0));
    (tail_raw - extra).max(0.0).min(head_r)
}

#[cfg(test)]
mod tests {
    use super::hold_tail_radius;

    #[test]
    fn hold_tail_min_length_is_anchored_and_fades_to_the_ring() {
        let lock = 74.0;
        let target = 292.0;
        let min_body = 20.0;

        // Spawn: tail pinned (progress 0) → offset = min_body.
        let at_spawn = hold_tail_radius(lock, 0.0, lock, min_body);
        assert!((lock - at_spawn - min_body).abs() < 1e-3);

        // Head moves out but the tail is still pinned: the tail must NOT move,
        // otherwise the whole bar lurches forward.
        let later = hold_tail_radius(lock, 0.0, lock + 100.0, min_body);
        assert!((later - at_spawn).abs() < 1e-3);

        // Tail on the ring: offset is gone, so it lands exactly on the ring.
        let at_ring = hold_tail_radius(target, 1.0, target, min_body);
        assert!((at_ring - target).abs() < 1e-3);

        // Never crosses the centre or the head.
        assert!(hold_tail_radius(5.0, 0.0, 10.0, min_body) >= 0.0);
        assert!(hold_tail_radius(200.0, 0.5, 100.0, min_body) <= 100.0);
    }
}
