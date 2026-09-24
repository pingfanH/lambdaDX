//! Runtime-tunable visual parameters (note sizes, slide trail, touch motion).
//!
//! These used to be `const`s in `types.rs`. They are now a serializable
//! [`Params`] value stored in a thread-local so the deep render code (slide
//! path builders, segmentation, `slide_render`) can read it without threading a
//! parameter through every signature.
//!
//! Startup precedence:
//!   1. the writable override `output/note_params.json` (what the panel saves),
//!   2. the bundled `assets/note_params.json` (the shipped defaults),
//!   3. the built-in `Params::default()`.

use std::cell::RefCell;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::platform;
use super::types as t;

/// Bundled default file (under `assets/`).
pub const PARAMS_ASSET: &str = "note_params.json";
/// Override file written by the panel (under the writable output dir).
pub const PARAMS_OVERRIDE: &str = "note_params.json";

/// All tunable visual parameters. Missing JSON fields fall back to
/// [`Params::default`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    // ── Note sizes ───────────────────────────────────────────────────
    pub tap_size: f32,
    pub hold_width: f32,
    pub star_size: f32,
    /// Radius added to `outer_r` where tap/slide land and the A-ring dots sit.
    pub tap_target_offset: f32,
    pub tap_ring_offset: f32,

    // ── Slide trail ──────────────────────────────────────────────────
    pub slide_tile_spacing: f32,
    pub slide_tile_scale: f32,
    pub slide_tile_size: f32,
    /// Slide trail tiles opacity (0..255). 255 = fully opaque.
    pub slide_trail_alpha: f32,
    /// Slide trail fade-in seconds (`app.slide_fade_in` default).
    pub slide_fade_in: f32,
    /// G1 join fillet radius = `arc_radius * frac`, clamped to [min, max].
    pub slide_join_fillet_frac: f32,
    pub slide_join_fillet_min: f32,
    pub slide_join_fillet_max: f32,
    /// How strongly the join fillet pulls the **arc** back (`1.0` = current).
    pub slide_join_arc_influence: f32,
    /// How strongly the join fillet pulls the **straight line** back
    /// (`1.0` = current).
    pub slide_join_line_influence: f32,
    /// Empty gap at the slide head / tail (screen px, scaled by ui scale).
    /// The trail samples between `[head_gap, total - tail_gap]`.
    pub slide_head_gap: f32,
    pub slide_tail_gap: f32,
    /// Star pop-in: end size = `star_size * (1 + gain)`.
    pub star_spawn_scale_gain: f32,
    /// Star pop-in: start alpha (0..1), ending at 1.
    pub star_spawn_alpha_start: f32,
    /// Note pass draw order. `false` = later notes drawn on top (default),
    /// `true` = earlier notes drawn on top (reverse the note pass).
    pub note_earlier_on_top: bool,
    /// Within a single slide, draw the trail tiles in reverse order.
    pub slide_tile_reverse: bool,
    /// Within one note, draw its `note.slide` sub-slides in reverse order.
    pub slide_sub_reverse: bool,

    // ── Touch / touch-hold ───────────────────────────────────────────
    pub touch_cross_size: f32,
    pub touch_start_dist: f32,
    pub touch_end_dist: f32,
    pub touch_scale: f32,
    pub touch_grow_frac: f32,
    /// Inward speed ramp half-range: 0.5 → speed 0.5x..1.5x.
    pub touch_move_ramp: f32,
    /// Touch note total visible duration multiplier (基础总时长倍率).
    pub touch_duration_scale: f32,
    /// Touch fade-in ("birth") fraction of the total duration (出生时长占比).
    pub touch_spawn_frac: f32,
    pub touchhold_cross_base: f32,
    pub touchhold_border_base: f32,
    pub touchhold_start_dist: f32,
    pub touchhold_end_dist: f32,
    pub touchhold_scale: f32,
    pub touchhold_rot_offset: f32,

    // ── Gameplay / playfield ─────────────────────────────────────────
    /// Overall pad zoom: scales the pad radius *and* everything drawn on it
    /// (notes, zone strokes, labels). 1.0 = design size.
    pub pad_zoom: f32,
    /// Sensor-zone cluster zoom about the pad centre. Independent of the pad
    /// radius, so the zone ring can be grown/shrunk on its own. 1.0 = SVG size.
    pub pad_zone_scale: f32,
    /// Default note flight speed (流速) applied to a fresh session.
    pub note_speed_default: f32,
    /// Default touch flight speed applied to a fresh session.
    pub touch_speed_default: f32,
    /// Where notes spawn (locked) as a fraction of the judge radius.
    pub note_spawn_frac: f32,
    /// Tap "birth" time (seconds): how long a tap scales up before it starts
    /// flying to the ring. `0` = follow the note speed (original behavior).
    pub tap_spawn_time: f32,
    /// Ring-note flight acceleration. `0` = constant speed; `1` = the note
    /// starts slow and gradually speeds up as it approaches the ring.
    pub note_accel: f32,
    /// Draw a guide texture under each tap, aligned with its flight direction.
    pub tap_guide: bool,
    /// Tap-guide base size as a multiple of the tap size (aspect preserved).
    pub tap_guide_size: f32,
    /// Tap-guide offset perpendicular to the flight direction (design px).
    pub tap_guide_off_x: f32,
    /// Tap-guide offset along the flight direction, + = outward (design px).
    pub tap_guide_off_y: f32,
    /// Tap-guide scale anchor along its length: `0` = top edge, `0.5` = centre,
    /// `1` = bottom edge. Scaling keeps this point fixed on the note centre.
    pub tap_guide_anchor: f32,
    /// Extra tap-guide scale proportional to travel progress: the further out,
    /// the bigger the guide (`0` = constant size).
    pub tap_guide_grow: f32,
    /// Tap-guide opacity (0..255).
    pub tap_guide_alpha: f32,
    /// Tap-guide rotation offset (radians) added to the flight direction.
    pub tap_guide_rot: f32,
    /// White on-hit ring opacity (0..255) (判定区透明度).
    pub judge_ring_alpha: f32,
    /// Pad panel background opacity (0..255) (背景透明度).
    pub pad_bg_alpha: f32,
    /// Big circle drawn just outside the sensor zones, as a multiple of the
    /// sensor outer radius (圆的大小).
    pub pad_circle_scale: f32,
    /// Background (disc + cover) radius as a multiple of the sensor outer
    /// radius. Defaults to match the occlusion circle (`pad_circle_scale`).
    pub pad_bg_scale: f32,
    /// Opacity (0..255) of the outside-the-circle occluding background.
    pub pad_outside_alpha: f32,

    // ── Judgment ─────────────────────────────────────────────────────
    /// Draw a black dot at each note's judgment point (topmost layer).
    pub judge_dot: bool,
    /// Judge-dot radius (design px).
    pub judge_dot_size: f32,
    /// Tap judgment-point offset along the flight direction (design px, + out).
    pub judge_off_tap: f32,
    /// Hold-head judgment-point offset along the flight direction.
    pub judge_off_hold: f32,
    /// Hold-tail judgment-point offset along the flight direction.
    pub judge_off_hold_end: f32,
    /// Hide the note layer (notes, slide trails, judgment text) — e.g. to see
    /// only the background video.
    pub hide_notes: bool,
    /// Hide the sensor-zone layer (zone polygons, spawn dot, A-ring dots).
    pub hide_zones: bool,
    /// Default playback speed applied to a fresh session (1.0 = normal).
    pub play_speed_default: f32,
    /// Whether playback speed also scales the visuals (note flight, star spin,
    /// touch motion). `false` = audio-only speed change; `true` = everything
    /// scales with the song (整体速度).
    pub speed_scales_visuals: bool,

    // ── Background video (pad preview) ───────────────────────────────
    /// Draw a background video behind the pad. Pad preview only.
    pub bg_video: bool,
    /// Video file path. Empty = `<assets>/bg.mp4`.
    pub bg_video_path: String,
    /// Video time (seconds) shown when the song clock is at `t = 0`.
    pub bg_video_start: f32,
    /// Horizontal offset of the video centre (design px, scaled by UI scale).
    pub bg_video_x: f32,
    /// Vertical offset of the video centre (design px, scaled by UI scale).
    pub bg_video_y: f32,
    /// Uniform zoom applied to the video.
    pub bg_video_scale: f32,
    /// Video opacity (0..255).
    pub bg_video_alpha: f32,
    /// Frames per second used when extracting frames. `0` = follow the source.
    pub bg_video_fps: f32,
    /// Output height in pixels (aspect is preserved).
    pub bg_video_height: f32,
    /// Loop the video when it reaches the end.
    pub bg_video_loop: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            tap_size: t::TAP_SIZE,
            hold_width: t::HOLD_WIDTH,
            star_size: t::STAR_SIZE,
            tap_target_offset: t::TAP_TARGET_OFFSET,
            tap_ring_offset: t::TAP_RING_OFFSET,

            slide_tile_spacing: t::SLIDE_TILE_SPACING,
            slide_tile_scale: t::SLIDE_TILE_SCALE,
            slide_tile_size: t::SLIDE_TILE_SIZE,
            slide_trail_alpha: 255.0,
            slide_fade_in: t::SLIDE_STAR_FADE_IN,
            slide_join_fillet_frac: 0.3,
            slide_join_fillet_min: 8.0,
            slide_join_fillet_max: 40.0,
            slide_join_arc_influence: 1.0,
            slide_join_line_influence: 1.0,
            slide_head_gap: 8.0,
            slide_tail_gap: 26.0,
            star_spawn_scale_gain: 0.5,
            star_spawn_alpha_start: 0.5,
            note_earlier_on_top: false,
            slide_tile_reverse: false,
            slide_sub_reverse: false,

            touch_cross_size: t::TOUCH_CROSS_SIZE,
            touch_start_dist: t::TOUCH_START_DIST,
            touch_end_dist: t::TOUCH_END_DIST,
            touch_scale: t::TOUCH_SCALE,
            touch_grow_frac: t::TOUCH_GROW_FRAC,
            touch_move_ramp: 0.5,
            touch_duration_scale: 1.0,
            touch_spawn_frac: 0.19,
            touchhold_cross_base: t::TOUCHHOLD_CROSS_BASE,
            touchhold_border_base: t::TOUCHHOLD_BORDER_BASE,
            touchhold_start_dist: t::TOUCHHOLD_START_DIST,
            touchhold_end_dist: t::TOUCHHOLD_END_DIST,
            touchhold_scale: t::TOUCHHOLD_SCALE,
            touchhold_rot_offset: t::TOUCHHOLD_ROT_OFFSET,

            note_speed_default: t::NOTE_SPEED,
            touch_speed_default: t::NOTE_SPEED * 0.7,
            pad_zoom: 1.0,
            pad_zone_scale: 1.0,            note_spawn_frac: 1.225 / 4.8,
            tap_spawn_time: 0.0,
            note_accel: 0.0,
            tap_guide: true,
            tap_guide_size: 1.0,
            tap_guide_off_x: 0.0,
            tap_guide_off_y: 0.0,
            tap_guide_anchor: 0.5,
            tap_guide_grow: 0.5,
            tap_guide_alpha: 255.0,
            tap_guide_rot: 0.0,
            judge_ring_alpha: 220.0,
            pad_bg_alpha: 255.0,
            pad_circle_scale: 1.06,
            pad_bg_scale: 1.06,
            pad_outside_alpha: 255.0,
            judge_dot: true,
            judge_dot_size: 3.0,
            judge_off_tap: 0.0,
            judge_off_hold: 0.0,
            judge_off_hold_end: 0.0,
            hide_notes: false,
            hide_zones: false,
            play_speed_default: 1.0,
            speed_scales_visuals: false,

            bg_video: false,
            bg_video_path: String::new(),
            bg_video_start: 0.0,
            bg_video_x: 0.0,
            bg_video_y: 0.0,
            bg_video_scale: 1.0,
            bg_video_alpha: 255.0,
            bg_video_fps: 0.0,
            bg_video_height: 1080.0,
            bg_video_loop: true,
        }
    }
}

thread_local! {
    static PARAMS: RefCell<Params> = RefCell::new(Params::default());
}

thread_local! {
    /// Pad/UI scale for the current frame, set by the renderer so the px-valued
    /// offsets below track the pad instead of the framebuffer.
    static PAD_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// Set the current pad/UI scale (called once per frame by the pad renderer).
pub fn set_pad_scale(s: f32) {
    PAD_SCALE.with(|c| c.set(s));
}

pub fn pad_scale() -> f32 {
    PAD_SCALE.with(|c| c.get())
}

/// Judge-ring offset in design px, scaled to the current pad.
pub fn tap_target_offset() -> f32 {
    get().tap_target_offset * pad_scale()
}

/// A-ring dot offset in design px, scaled to the current pad.
pub fn tap_ring_offset() -> f32 {
    get().tap_ring_offset * pad_scale()
}

/// A snapshot of the current parameters (cheap; all fields are `Copy`).
pub fn get() -> Params {
    PARAMS.with(|p| p.borrow().clone())
}

/// Note pass draw order: `true` = earlier notes on top.
pub fn note_earlier_on_top() -> bool {
    PARAMS.with(|p| p.borrow().note_earlier_on_top)
}

/// Within one slide, draw the trail tiles reversed.
pub fn slide_tile_reverse() -> bool {
    PARAMS.with(|p| p.borrow().slide_tile_reverse)
}

/// Within one note, draw its sub-slides reversed.
pub fn slide_sub_reverse() -> bool {
    PARAMS.with(|p| p.borrow().slide_sub_reverse)
}

/// Replace the current parameters globally.
pub fn set(p: Params) {
    PARAMS.with(|c| *c.borrow_mut() = p);
}

/// Per-field accessors so the render code can read a single value without
/// importing the whole [`Params`] struct. Generated from the field list.
macro_rules! param_accessors {
    ($($field:ident),* $(,)?) => {
        $(
            #[inline]
            pub fn $field() -> f32 {
                get().$field
            }
        )*
    };
}
param_accessors!(
    tap_size,
    hold_width,
    star_size,
    slide_tile_spacing,
    slide_tile_scale,
    slide_tile_size,
    slide_trail_alpha,
    slide_fade_in,
    slide_join_fillet_frac,
    slide_join_fillet_min,
    slide_join_fillet_max,
    slide_join_arc_influence,
    slide_join_line_influence,
    slide_head_gap,
    slide_tail_gap,
    star_spawn_scale_gain,
    star_spawn_alpha_start,
    touch_cross_size,
    touch_start_dist,
    touch_end_dist,
    touch_scale,
    touch_grow_frac,
    touch_move_ramp,
    touch_duration_scale,
    touch_spawn_frac,
    touchhold_cross_base,
    touchhold_border_base,
    touchhold_start_dist,
    touchhold_end_dist,
    touchhold_scale,
    touchhold_rot_offset,
    note_speed_default,
    touch_speed_default,
    pad_zoom,
    pad_zone_scale,
    note_spawn_frac,
    tap_spawn_time,
    note_accel,
    tap_guide_size,
    tap_guide_off_x,
    tap_guide_off_y,
    tap_guide_anchor,
    tap_guide_grow,
    tap_guide_alpha,
    tap_guide_rot,
    judge_ring_alpha,
    pad_bg_alpha,
    pad_circle_scale,
    pad_bg_scale,
    pad_outside_alpha,
    judge_dot_size,
    judge_off_tap,
    judge_off_hold,
    judge_off_hold_end,
    play_speed_default,
    bg_video_start,
    bg_video_x,
    bg_video_y,
    bg_video_scale,
    bg_video_alpha,
    bg_video_fps,
    bg_video_height,
);

/// Whether the background video is enabled.
pub fn bg_video() -> bool {
    PARAMS.with(|p| p.borrow().bg_video)
}

/// Whether the background video should loop.
pub fn bg_video_loop() -> bool {
    PARAMS.with(|p| p.borrow().bg_video_loop)
}

/// Whether the note layer is hidden.
pub fn hide_notes() -> bool {
    PARAMS.with(|p| p.borrow().hide_notes)
}

/// Whether the sensor-zone layer is hidden.
pub fn hide_zones() -> bool {
    PARAMS.with(|p| p.borrow().hide_zones)
}

/// Whether playback speed also scales the note visuals (整体速度).
pub fn speed_scales_visuals() -> bool {
    PARAMS.with(|p| p.borrow().speed_scales_visuals)
}

/// Whether the tap guide texture is enabled.
pub fn tap_guide() -> bool {
    PARAMS.with(|p| p.borrow().tap_guide)
}

/// Whether the judgment-point black dot is enabled.
pub fn judge_dot() -> bool {
    PARAMS.with(|p| p.borrow().judge_dot)
}

/// Explicit background-video path (empty = `<assets>/bg.mp4`).
pub fn bg_video_path() -> String {
    PARAMS.with(|p| p.borrow().bg_video_path.clone())
}

/// Load parameters: override file, then bundled asset, then defaults.
pub fn load() -> Params {
    if let Ok(s) = platform::read_output_text(PARAMS_OVERRIDE) {
        if let Ok(p) = serde_json::from_str::<Params>(&s) {
            return p;
        }
    }
    let asset = platform::asset_dir().join(PARAMS_ASSET);
    if let Ok(s) = std::fs::read_to_string(&asset) {
        if let Ok(p) = serde_json::from_str::<Params>(&s) {
            return p;
        }
    }
    Params::default()
}

/// Save parameters as pretty JSON to the writable override path.
pub fn save(p: &Params) -> Result<PathBuf, String> {
    let json = serde_json::to_string_pretty(p).map_err(|e| e.to_string())?;
    platform::write_output_text(PARAMS_OVERRIDE, &json)
}

#[cfg(test)]
mod tests {
    use super::Params;

    #[test]
    fn partial_json_falls_back_to_defaults() {
        let p: Params = serde_json::from_str(r#"{ "star_size": 99.0 }"#).unwrap();
        assert_eq!(p.star_size, 99.0);
        assert_eq!(p.tap_size, Params::default().tap_size);
    }

    #[test]
    fn round_trips_through_json() {
        let p = Params::default();
        let s = serde_json::to_string_pretty(&p).unwrap();
        let q: Params = serde_json::from_str(&s).unwrap();
        assert_eq!(p, q);
    }

    /// The background (disc + cover) defaults to the same size as the
    /// occlusion circle. Both stay independently editable.
    #[test]
    fn bg_scale_defaults_to_the_mask_scale() {
        let p = Params::default();
        assert_eq!(p.pad_bg_scale, p.pad_circle_scale);
    }
}
