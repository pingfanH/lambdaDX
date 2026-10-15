//! Screen-space rectangles computed by the UI each frame.
//!
//! The runtime reads these to place the macroquad layers (pad preview, timeline)
//! exactly under their egui hosts so interaction and rendering stay aligned.

use egui_macroquad::egui;

use crate::app::types::PadGeom;

/// Content scale for notes in the embedded pad, separate from host UI sizing.
pub fn preview_note_scale(pad_radius: f32, ui_scale: f32) -> f32 {
    let reference_radius = 856.0 * 0.42;
    (pad_radius / reference_radius).max(0.1) * ui_scale
}

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

/// Width of the seam handle between the timeline and the info panel (egui
/// points). The central panel reserves this at its right edge, so the timeline
/// column is `central_width - PANEL_HANDLE_W`.
pub const PANEL_HANDLE_W: f32 = 14.0;

/// Width of the right-hand info panel for a given screen width.
///
/// The timeline column (the central panel minus the seam handle) is given the
/// same width as the panel, i.e. the two columns split the content evenly.
/// Never drops below 320px.
pub fn sidebar_width(screen_w: f32) -> f32 {
    ((screen_w - PANEL_HANDLE_W) * 0.5).max(320.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_notes_keep_player_proportions_on_small_pads() {
        let reference_radius = 856.0 * 0.42;
        for radius in [100.0, 180.0, reference_radius] {
            let scale = preview_note_scale(radius, 1.0);
            assert!(
                (scale / radius - 1.0 / reference_radius).abs() < 0.00001,
                "preview radius {radius} used full-player note scale {scale}"
            );
            assert!((preview_note_scale(radius, 1.5) - scale * 1.5).abs() < 0.00001);
        }
    }

    #[test]
    fn preview_zoom_scales_notes_once() {
        let mut layout = Layout {
            preview: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)),
            ..Default::default()
        };
        let ui_scale = 1.0;
        layout.update_pad(1.0);
        let base = preview_note_scale(layout.pad.outer_r, ui_scale);
        layout.update_pad(1.5);
        assert!((preview_note_scale(layout.pad.outer_r, ui_scale) - base * 1.5).abs() < 0.00001);
    }

    #[test]
    fn sidebar_matches_timeline_width() {
        // 1600 → panel ≈ 793, and the central panel (minus the handle) is the
        // same, so the timeline and panel columns are equal.
        let w = sidebar_width(1600.0);
        assert!((w - 793.0).abs() < 1.0, "1600 → {w}");
        let central = 1600.0 - w;
        assert!((central - PANEL_HANDLE_W - w).abs() < 0.5, "timeline != panel");
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
