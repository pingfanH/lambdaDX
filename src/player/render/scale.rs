//! UI controls and playfield art have independent scale sources.

use crate::app::types::PadGeom;
use crate::player::state::PadPreviewState;
use macroquad::prelude::{screen_height, screen_width};

/// Shorter-side length the visuals were tuned at (1280×760 default window).
pub const DESIGN_MIN_SIDE: f32 = 760.0;
pub const UI_SCALE_MIN: f32 = 0.7;
pub const UI_SCALE_MAX: f32 = 4.0;

/// Geometry already includes Pad zoom and the actual render viewport.
pub fn pad_scale(pad: PadGeom) -> f32 {
    (pad.outer_r / (DESIGN_MIN_SIDE * 0.42)).max(0.01)
}

/// `MAI2_UI_SCALE` / the settings override wins; mobile uses larger controls.
pub fn ui_scale(app: &PadPreviewState) -> f32 {
    ui_scale_for(app, screen_width(), screen_height())
}

/// Scale for an explicit viewport. Used by the offscreen exporter, whose
/// layout is computed at the target resolution rather than the live window.
pub fn ui_scale_for(app: &PadPreviewState, width: f32, height: f32) -> f32 {
    if let Some(v) = app.ui_scale_override {
        if v.is_finite() { return v.clamp(UI_SCALE_MIN, UI_SCALE_MAX); }
    }
    let base = (width.min(height) / DESIGN_MIN_SIDE).max(0.1);
    if app.mobile_ui { base.max(1.35) } else { base }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::types::ChartDoc;

    pub(crate) fn app() -> PadPreviewState {
        let chart: ChartDoc = serde_json::from_str(
            r#"{"version":"1","title":"","bpm":120,"notes":[]}"#,
        )
        .unwrap();
        let mut app = PadPreviewState::new_at(chart, None, None, 0.0, None);
        app.ui_scale_override = None;
        app.mobile_ui = false;
        app
    }

    #[test]
    fn window_size_scales_notes_and_hud_proportionally() {
        let mut app = app();
        for mobile in [false, true] {
            app.mobile_ui = mobile;
            for (width, height) in [(800.0, 600.0), (1280.0, 760.0), (2560.0, 1440.0)] {
                let layout = crate::player::layout::compute_layout_sized(&app, width, height);
                let base = (width.min(height) / DESIGN_MIN_SIDE).max(0.1);
                let expected = if mobile { base.max(1.35) } else { base };
                assert_eq!(layout.header.h, 76.0 * expected, "{width}x{height}");
                assert_eq!(ui_scale_for(&app, width, height), expected);
            }
        }
        app.ui_scale_override = Some(1.6);
        for (width, height) in [(800.0, 600.0), (2560.0, 1440.0)] {
            let layout = crate::player::layout::compute_layout_sized(&app, width, height);
            assert_eq!(layout.header.h, 76.0 * 1.6);
            assert_eq!(ui_scale_for(&app, width, height), 1.6);
        }
    }

    #[test]
    fn ui_override_and_mobile_layout_do_not_resize_playfield_art() {
        let mut app = app();
        for (w, h) in [(800.0, 600.0), (1280.0, 760.0), (2560.0, 1440.0)] {
            let mut expected = None;
            for mobile in [false, true] {
                app.mobile_ui = mobile;
                for ui in [None, Some(0.7), Some(1.6), Some(4.0)] {
                    app.ui_scale_override = ui;
                    let panel = crate::player::layout::compute_layout_sized(&app, w, h).pad;
                    let geom = crate::player::layout::compute_pad_geom(panel);
                    let scale = pad_scale(geom);
                    assert_eq!(*expected.get_or_insert(scale), scale);
                }
            }
        }
    }

    #[test]
    fn playfield_art_tracks_geometry_including_zoom_and_offscreen_viewports() {
        for zoom in [0.3, 1.0, 2.0] {
            for height in [360.0, 760.0, 1440.0] {
                let pad = PadGeom { cx: 0.0, cy: 0.0, outer_r: height * 0.42 * zoom };
                assert!((pad_scale(pad) - height / DESIGN_MIN_SIDE * zoom).abs() < 0.0001);
            }
        }
    }
}
