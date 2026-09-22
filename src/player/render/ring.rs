//! A/B-ring notes: tap and hold. These use the radial flight model described in
//! `render/mod.rs` — fly inward-locked, grow, then travel to the outer ring.

use macroquad::color::{Color, WHITE};
use macroquad::math::{Vec2, vec2};
use macroquad::prelude::{DrawTextureParams, draw_circle, draw_circle_lines, draw_line, draw_texture_ex};

use crate::app::types::{
    HIT_WINDOW, HOLD_SPAWN_BODY_WIDTH_FRAC, HOLD_WIDTH, NoteMotion, NoteType, PAD_ROTATION_RAD,
    TAP_SIZE, TAP_TARGET_OFFSET, note_lock_radius, note_radial_motion,
    note_radial_motion_continue,
};
use crate::app::ui::draw_hold_9slice_segment;
use crate::player::render::timing::NoteTiming;
use crate::player::state::PadPreviewState;

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
    // version (the head must stay on the ring while the body is held).
    let motion = if matches!(note.note_type, NoteType::Tap) {
        note_radial_motion_continue(t.dt_scaled, t.speed, outer_r, TAP_TARGET_OFFSET)
    } else {
        note_radial_motion(t.dt_scaled, t.speed, outer_r, TAP_TARGET_OFFSET)
    };
    let Some(motion) = motion else {
        return;
    };
    let px = spawn_cx.x + dir.x * motion.radius;
    let py = spawn_cx.y + dir.y * motion.radius;

    if matches!(note.note_type, NoteType::Hold) {
        draw_hold(app, note, t, dir, scale, spawn_cx, outer_r);
    }

    // Slide heads are drawn by the slide renderer; everything else draws a tap.
    if !matches!(note.note_type, NoteType::Hold | NoteType::Slide) {
        draw_tap(app, note, motion.scale, px, py, scale);
    }

    // A brief white ring when the note is exactly on its hit instant.
    if t.dt.abs() <= HIT_WINDOW {
        draw_circle_lines(
            px,
            py,
            TAP_SIZE * 0.53 * scale,
            2.0 * scale,
            Color::from_rgba(255, 255, 255, 220),
        );
    }
}

/// Tap body: a skin texture (plus an Ex overlay) or a fallback pink circle.
fn draw_tap(
    app: &PadPreviewState,
    note: &crate::app::types::Note,
    motion_scale: f32,
    px: f32,
    py: f32,
    scale: f32,
) {
    let ts = TAP_SIZE * scale * motion_scale;
    let tap_tex = if note.is_break {
        app.tap_break_tex.as_ref()
    } else if note.is_each {
        app.tap_each_tex.as_ref()
    } else {
        app.tap_texture.as_ref()
    };
    if let Some(tex) = tap_tex.or(app.tap_texture.as_ref()) {
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
            if let Some(ex_tex) = app.tap_ex_tex.as_ref() {
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
        let tr = TAP_SIZE * 0.375 * scale * motion_scale;
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
    dir: Vec2,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
) {
    let lock_r = note_lock_radius(outer_r, TAP_TARGET_OFFSET);
    let head_motion = note_radial_motion(t.dt_scaled, t.speed, outer_r, TAP_TARGET_OFFSET)
        .unwrap_or(NoteMotion {
            radius: lock_r,
            scale: 0.0,
            progress: 0.0,
        });

    // Width of the 9-slice body (also its minimum length at spawn). Grows with
    // the head while the hold is spawning.
    let body_w = (HOLD_WIDTH * scale * head_motion.scale).max(1.0);

    // Tail flies on the same radial model at the hold-end time.
    let tail_motion = note_radial_motion(t.tail_dt_scaled, t.speed, outer_r, TAP_TARGET_OFFSET);
    let (tail_raw, tail_progress) = tail_motion
        .map(|m| (m.radius, m.progress))
        .unwrap_or((lock_r, 0.0));

    // At spawn head and tail share the lock radius, so the raw body length is 0.
    // Add a short extra length toward the centre. The offset is anchored to the
    // tail (not the head) so the bar does not lurch forward once the head starts
    // moving, and it fades out as the tail nears the ring so the tail still
    // lands exactly on the ring at the hold end.
    let tail_r = hold_tail_radius(
        tail_raw,
        tail_progress,
        head_motion.radius,
        body_w * HOLD_SPAWN_BODY_WIDTH_FRAC,
    );

    let hx = spawn_cx.x + dir.x * head_motion.radius;
    let hy = spawn_cx.y + dir.y * head_motion.radius;
    let tx = spawn_cx.x + dir.x * tail_r;
    let ty = spawn_cx.y + dir.y * tail_r;

    let hold_tex = if note.is_break {
        app.hold_break_tex.as_ref()
    } else if note.is_each {
        app.hold_each_tex.as_ref()
    } else {
        app.hold_texture.as_ref()
    };

    let head_pos = vec2(hx, hy);
    let tail_pos = vec2(tx, ty);
    if let Some(tex) = hold_tex.or(app.hold_texture.as_ref()) {
        draw_hold_9slice_segment(
            tex,
            head_pos,
            tail_pos,
            body_w,
            Color::from_rgba(255, 255, 255, 255),
            dir,
        );
        if note.is_ex {
            if let Some(ex_tex) = app.hold_ex_tex.as_ref() {
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
            HOLD_WIDTH * 0.233 * scale * head_motion.scale,
            Color::from_rgba(251, 113, 133, 200),
        );
        draw_circle(
            tx,
            ty,
            HOLD_WIDTH * 0.167 * scale * head_motion.scale,
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
