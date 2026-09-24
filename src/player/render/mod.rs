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
pub mod progress;
pub mod ring;
pub mod scale;
pub mod skin;
pub mod slide;
pub mod textures;
pub mod timing;
pub mod touch;

pub use scale::ui_scale;
pub use textures::load_note_textures;

use macroquad::math::vec2;
use macroquad::prelude::Color;

use crate::app::params;
use crate::app::types::{Mode, PadGeom, RectF};
use crate::player::state::PadPreviewState;

/// Surface styling for [`draw_pad_panel`], so both front-ends share one pad
/// composition while choosing their own backdrop.
#[derive(Debug, Clone, Copy, Default)]
pub struct PadSurface {
    /// Panel fill; `None` leaves whatever the caller drew underneath.
    pub fill: Option<Color>,
    /// Sparse dot-grid color drawn over the fill.
    pub dots: Option<Color>,
    /// 1px panel border color.
    pub border: Option<Color>,
}

/// Shared dark surface palette (also re-exported by the player UI theme, so the
/// pad backdrop is literally the same in both binaries).
pub const PANEL: Color = Color::from_rgba(0x20, 0x20, 0x22, 255);
pub const GRID: Color = Color::from_rgba(0x2e, 0x2e, 0x33, 255);
pub const BORDER_SOFT: Color = Color::from_rgba(0x2b, 0x2b, 0x2f, 255);

impl PadSurface {
    /// Nothing at all (the caller draws its own backdrop).
    pub fn transparent() -> Self {
        Self::default()
    }

    /// The shared themed surface: `PANEL` fill (alpha `fill_alpha`), dot grid
    /// and a soft border. Used by both binaries so the pad area is identical.
    pub fn themed(fill_alpha: f32) -> Self {
        Self {
            fill: Some(Color::new(
                PANEL.r,
                PANEL.g,
                PANEL.b,
                fill_alpha.clamp(0.0, 1.0),
            )),
            dots: Some(Color::new(GRID.r, GRID.g, GRID.b, 0.35)),
            border: Some(BORDER_SOFT),
        }
    }
}

/// Draw the whole pad view: surface, cover, notes, occlusion ring, judgment.
///
/// Both binaries funnel through here (the preview and the player UI), so the
/// pad composition only exists once. `surface` selects the backdrop style.
pub fn draw_pad_panel(
    app: &PadPreviewState,
    rect: RectF,
    pad: PadGeom,
    surface: PadSurface,
) {
    // Overall pad zoom scales the sprites/labels with the pad radius.
    let scale = ui_scale(app) * params::pad_zoom();
    // Publish the pad scale so px-valued offsets (judge ring, A-dots) track the
    // pad across window resizes / DPI changes.
    params::set_pad_scale(scale);

    // A background video, when present, *replaces* the whole backdrop: the
    // styled surface, the disc and the outside occlusion are all skipped so the
    // video stays visible behind the zones/notes.
    let video = app.video_bg.is_ready();
    if video {
        pad::draw_video_background(app, rect, scale);
    } else {
        pad::draw_surface(rect, surface, scale);
    }
    // Background (disc + circular cover) sized like the occlusion circle.
    let bg_r = pad.outer_r * params::pad_bg_scale();
    if !video {
        pad::draw_pad_disc(pad.cx, pad.cy, bg_r);
    }
    pad::draw_cover(app, pad.cx, pad.cy, bg_r);

    // Tap spawn centre = SVG C-zone centroid (falls back to the disc centre).
    let spawn_cx = app
        .pad_svg
        .as_ref()
        .and_then(|svg| svg.pad_visual_center(&pad))
        .unwrap_or(vec2(pad.cx, pad.cy));
    // Sensor-zone layer (zone polygons, spawn dot, A-ring dots). Can be hidden
    // to leave only the background video.
    if !params::hide_zones() {
        pad::draw_spawn_dot(spawn_cx, scale);
        pad::draw_zones(app, &pad, scale);
        pad::draw_ring_indicators(spawn_cx, pad.outer_r, scale);
    }

    // Note layer (notes, slide trails, judgment text). Can be hidden too.
    let current_t = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    // `speed_scale` normalizes note flight to wall-clock time. When the user
    // opts into "整体速度" we pass 1.0 instead, so the visuals scale with the
    // playback speed (flight, star spin, touch motion all speed up).
    let speed_scale = if params::speed_scales_visuals() {
        1.0
    } else {
        app.play_speed.max(0.1)
    };
    if !params::hide_notes() {
        notes::draw_notes(app, &pad, scale, spawn_cx, current_t, speed_scale);
    }

    // Big sensor circle + occluding ring on top of the notes. The outside
    // occlusion stays on even with a video; only the inner disc is skipped.
    pad::draw_outer_circle(&pad, scale);

    // Overlay.
    if !params::hide_notes() {
        feedback::draw(app, &pad, pad.outer_r, spawn_cx, scale);
        // Judgment-point dots on the very top.
        notes::draw_judge_dots(app, &pad, scale, spawn_cx, current_t, speed_scale);
    }

    // lnmai-core score parameters down the bottom-left of the pad panel.
    crate::player::hud::draw_score_block(app, rect, scale);
}
