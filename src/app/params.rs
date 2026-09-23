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
    pub touch_stall_frac: f32,
    /// Inward speed ramp half-range: 0.5 → speed 0.5x..1.5x.
    pub touch_move_ramp: f32,
    pub touchhold_cross_base: f32,
    pub touchhold_border_base: f32,
    pub touchhold_start_dist: f32,
    pub touchhold_end_dist: f32,
    pub touchhold_scale: f32,
    pub touchhold_rot_offset: f32,
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
            touch_stall_frac: t::TOUCH_STALL_FRAC,
            touch_move_ramp: 0.5,
            touchhold_cross_base: t::TOUCHHOLD_CROSS_BASE,
            touchhold_border_base: t::TOUCHHOLD_BORDER_BASE,
            touchhold_start_dist: t::TOUCHHOLD_START_DIST,
            touchhold_end_dist: t::TOUCHHOLD_END_DIST,
            touchhold_scale: t::TOUCHHOLD_SCALE,
            touchhold_rot_offset: t::TOUCHHOLD_ROT_OFFSET,
        }
    }
}

thread_local! {
    static PARAMS: RefCell<Params> = RefCell::new(Params::default());
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
    tap_target_offset,
    tap_ring_offset,
    slide_tile_spacing,
    slide_tile_scale,
    slide_tile_size,
    slide_trail_alpha,
    slide_fade_in,
    slide_join_fillet_frac,
    slide_join_fillet_min,
    slide_join_fillet_max,
    slide_head_gap,
    slide_tail_gap,
    star_spawn_scale_gain,
    star_spawn_alpha_start,
    touch_cross_size,
    touch_start_dist,
    touch_end_dist,
    touch_scale,
    touch_grow_frac,
    touch_stall_frac,
    touch_move_ramp,
    touchhold_cross_base,
    touchhold_border_base,
    touchhold_start_dist,
    touchhold_end_dist,
    touchhold_scale,
    touchhold_rot_offset,
);

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
}
