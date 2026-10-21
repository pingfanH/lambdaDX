//! Pad note / HUD sizes follow the window's shorter side, expressed in logical
//! pixels so a HiDPI window does not scale twice. Pad geometry fits its
//! viewport separately; explicit zoom still scales art on top of this.

use crate::player::state::PadPreviewState;
use macroquad::prelude::{screen_height, screen_width};

/// Shorter-side length the visuals were tuned at (1280×760 default window).
pub const DESIGN_MIN_SIDE: f32 = 760.0;

/// `MAI2_UI_SCALE` / the settings override wins; mobile uses larger controls.
pub fn ui_scale(app: &PadPreviewState) -> f32 {
    ui_scale_for(app, screen_width(), screen_height())
}

/// Scale for an explicit viewport. Used by the offscreen exporter, whose
/// layout is computed at the target resolution rather than the live window.
pub fn ui_scale_for(app: &PadPreviewState, width: f32, height: f32) -> f32 {
    if let Some(v) = app.ui_scale_override {
        return v;
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
}
