//! Pad-preview rendering, split by concern.
//!
//! [`draw_pad_panel`] is the single entry point. It composes:
//!
//! | module | responsibility |
//! |---|---|
//! | [`pad`] | background, disc, SVG zones, A-ring indicators |
//! | [`timing`] | per-note timing / visibility derivation |
//! | [`notes`] | chart iteration + per-kind dispatch |
//! | [`ring`] | tap + hold (radial flight) |
//! | [`touch`] | touch + touch-hold (whole-duration fade/move) |
//! | [`slide`] | slide adapter over `app::slide_render` |
//! | [`feedback`] | judgment text overlay |
//! | [`textures`] | skin texture loading |
//! | [`scale`] | UI scale factor |
//!
//! # Note motion model
//!
//! Chart times are **measures**, converted to **seconds** through the BPM list.
//! For every note:
//!
//! ```text
//! dt        = note_head_seconds - current_t   // > 0 still approaching
//! dt_scaled = dt / play_speed                 // musical time, speed-independent
//! ```
//!
//! ## A/B-ring notes (tap, hold, slide head — `zone <= 8`)
//!
//! Fixed direction per lane:
//! `angle = -90° + PAD_ROTATION_RAD + (zone-1) * 45°`. The radius follows
//! `distance = NOTE_OUTER_DISTANCE - dt_scaled*speed`:
//!
//! * while `distance > NOTE_LOCK_DISTANCE` the note is pinned at the inner
//!   `lock_r` and only its `scale` grows to 1.0,
//! * then it travels outward to `outer_r + TAP_TARGET_OFFSET` at `dt == 0`.
//!
//! **Taps do not stop there**: once `dt < 0` they keep flying past the judgment
//! ring at the same speed (the clamped upper bound is lifted via
//! `note_radial_motion_continue`) until the `disappear_time` cull. Holds and
//! slide heads keep the clamped radius so they stay on the ring.
//!
//! Notes are culled once `dt_scaled > lead_time` (before they would enter the
//! `NOTE_VISIBLE_DISTANCE` band). See [`timing`] and [`ring`].
//!
//! ## Hold notes
//!
//! Head and tail are both evaluated with the same radial model at their own
//! times (`dt_scaled` and `tail_dt_scaled`). Sharing `speed` makes them move at
//! the same radial speed: the body translates while approaching, the head parks
//! on the ring at the hit, and the tail reaches the ring exactly when the hold
//! ends. Before the tail is visible it sits at the lock radius. Unlike a tap,
//! a hold does not linger: it is culled the instant its tail reaches the ring
//! (its `disappear_time` is 0). See [`ring`] and [`timing`].
//!
//! ## Touch / touch-hold notes (`zone > 8`)
//!
//! No radial flight. Fade in at the zone centroid over a whole duration derived
//! from touch speed (`touch_whole_duration`), then the four arms move inward.
//! Holds add 4 diagonal sprites + a shader progress ring. See [`touch`].
//!
//! ## Slide notes
//!
//! Handled by [`slide`] / `app::slide_render`: trail tiles are sampled along the
//! path and a star flies the path from start-delay to duration.

pub mod feedback;
pub mod notes;
pub mod pad;
pub mod ring;
pub mod scale;
pub mod slide;
pub mod textures;
pub mod timing;
pub mod touch;

pub use scale::ui_scale;
pub use textures::load_note_textures;

use macroquad::math::vec2;

use crate::app::types::{Mode, PadGeom, RectF};
use crate::player::state::PadPreviewState;

/// Draw the whole pad panel: chrome, notes, then judgment text.
pub fn draw_pad_panel(app: &PadPreviewState, rect: RectF, pad: PadGeom) {
    let scale = ui_scale(app);

    // Static chrome.
    pad::draw_panel_background(rect, scale);
    pad::draw_pad_disc(pad.cx, pad.cy, pad.outer_r);

    // Tap spawn centre = SVG C-zone centroid (falls back to the disc centre).
    let spawn_cx = app
        .pad_svg
        .as_ref()
        .and_then(|svg| svg.pad_visual_center(&pad))
        .unwrap_or(vec2(pad.cx, pad.cy));
    pad::draw_spawn_dot(spawn_cx, scale);
    pad::draw_zones(app, &pad, scale);
    pad::draw_ring_indicators(spawn_cx, pad.outer_r, scale);

    // Notes.
    let current_t = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    let speed_scale = app.play_speed.max(0.1);
    notes::draw_notes(app, &pad, scale, spawn_cx, current_t, speed_scale);

    // Overlay.
    feedback::draw(app, &pad, pad.outer_r, spawn_cx, scale);
}
