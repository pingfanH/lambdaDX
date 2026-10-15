//! Note and UI sizes in logical pixels, independent of the window dimensions.
//! Pad geometry fits its viewport separately; explicit zoom still scales art.

use crate::player::state::PadPreviewState;

/// Shorter-side length the visuals were tuned at (1280×760 default window).
pub const DESIGN_MIN_SIDE: f32 = 760.0;

/// `MAI2_UI_SCALE` / the settings override wins; mobile uses larger controls.
pub fn ui_scale(app: &PadPreviewState) -> f32 {
    if let Some(v) = app.ui_scale_override {
        return v;
    }
    if app.mobile_ui {
        1.35
    } else {
        1.0
    }
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
    fn window_size_does_not_change_note_or_hud_scale() {
        let mut app = app();
        for mobile in [false, true] {
            app.mobile_ui = mobile;
            let expected = if mobile { 1.35 } else { 1.0 };
            for (width, height) in [(800.0, 600.0), (1280.0, 760.0), (2560.0, 1440.0)] {
                let layout = crate::player::layout::compute_layout_sized(&app, width, height);
                assert_eq!(layout.header.h, 76.0 * expected, "{width}x{height}");
                assert_eq!(ui_scale(&app), expected);
            }
        }
        app.ui_scale_override = Some(1.6);
        for (width, height) in [(800.0, 600.0), (2560.0, 1440.0)] {
            let layout = crate::player::layout::compute_layout_sized(&app, width, height);
            assert_eq!(layout.header.h, 76.0 * 1.6);
        }
    }
}
