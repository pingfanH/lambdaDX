//! Screen-space rectangles computed by the UI each frame.
//!
//! The runtime reads these to place the macroquad layers (pad preview, timeline)
//! exactly under their egui hosts so interaction and rendering stay aligned.

use egui_macroquad::egui;

use crate::app::types::PadGeom;

/// Reference radius used to derive the pad's render scale from the preview size
/// (matches the design stage's `DESIGN_H * 0.42`).
pub const DESIGN_PAD_REFERENCE: f32 = 856.0 * 0.42;

/// All regions of one editor frame, in egui points.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// Top application toolbar.
    pub toolbar: egui::Rect,
    /// Right-hand pad/inspector panel.
    pub sidebar: egui::Rect,
    /// Pad preview region (top of the sidebar).
    pub preview: egui::Rect,
    /// Horizontal tool palette strip (above the timeline).
    pub tools: egui::Rect,
    /// Timeline plot region (macroquad).
    pub timeline: egui::Rect,
    /// Bottom progress bar.
    pub overview: egui::Rect,
    /// Live pad geometry, filled by [`super::EditorUi::finish_layout`].
    pub pad: PadGeom,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            toolbar: egui::Rect::NOTHING,
            sidebar: egui::Rect::NOTHING,
            preview: egui::Rect::NOTHING,
            tools: egui::Rect::NOTHING,
            timeline: egui::Rect::NOTHING,
            overview: egui::Rect::NOTHING,
            pad: PadGeom {
                cx: 0.0,
                cy: 0.0,
                outer_r: 0.0,
            },
        }
    }
}

impl Layout {
    /// Update the pad geometry from the preview region at the given zoom.
    pub fn update_pad(&mut self, pad_scale: f32) {
        let radius = self.preview.width().min(self.preview.height()) * 0.42 * pad_scale;
        let center = self.preview.center();
        self.pad = PadGeom {
            cx: center.x,
            cy: center.y,
            outer_r: radius.max(1.0),
        };
    }
}

/// Width of the right-hand info panel for a given screen width.
///
/// Matches the authored design's ~47 % split (panel ≈ 468/993 of the content),
/// keeping at least ~360px of timeline and never dropping below 320px.
pub fn sidebar_width(screen_w: f32) -> f32 {
    let target = screen_w * 0.47;
    let max = (screen_w - 360.0).max(320.0);
    target.clamp(320.0, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_is_about_half_the_window() {
        let w = sidebar_width(1600.0);
        assert!(w > 700.0 && w < 760.0, "1600 → {w}");
        // Tiny windows clamp to the minimum.
        assert_eq!(sidebar_width(400.0), 320.0);
    }

    #[test]
    fn pad_stays_inside_preview() {
        let mut layout = Layout {
            preview: egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 300.0)),
            ..Default::default()
        };
        layout.update_pad(1.0);
        let p = layout.pad;
        assert!(p.cx - p.outer_r >= 100.0 && p.cx + p.outer_r <= 500.0);
        assert!(p.cy - p.outer_r >= 100.0 && p.cy + p.outer_r <= 400.0);
    }
}
