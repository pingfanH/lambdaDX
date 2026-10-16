//! Design tokens for the editor chrome.
//!
//! Neutral workbench surfaces with color reserved for content and state.

use egui_macroquad::egui;

/// Small helper so the palette below stays a plain list of hex-ish triples.
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// Convert an egui colour to a macroquad colour (for the macroquad layers).
pub fn to_mq(c: egui::Color32) -> macroquad::prelude::Color {
    let [r, g, b, a] = c.to_array();
    macroquad::prelude::Color::from_rgba(r, g, b, a)
}

// ── Surfaces ───────────────────────────────────────────────────────────────
pub const BG_WINDOW: egui::Color32 = rgb(23, 23, 23);
pub const BG_TOOLBAR: egui::Color32 = rgb(31, 31, 31);
pub fn floating_surface() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(43, 43, 43, 232)
}
pub const BG_CANVAS: egui::Color32 = rgb(39, 39, 39);
pub const BG_PANEL: egui::Color32 = rgb(40, 40, 40);
pub const BG_OVERVIEW: egui::Color32 = rgb(34, 34, 34);
pub const BG_WIDGET: egui::Color32 = rgb(52, 52, 52);
/// Widget background on hover.
pub const BG_WIDGET_HOVER: egui::Color32 = rgb(56, 56, 56);
pub const BG_HANDLE: egui::Color32 = rgb(61, 61, 61);
pub const BG_ACTIVE: egui::Color32 = rgb(121, 184, 244);

// ── Text ───────────────────────────────────────────────────────────────────
pub const TEXT: egui::Color32 = rgb(220, 220, 220);
pub const TEXT_DIM: egui::Color32 = rgb(160, 160, 160);
pub const TEXT_FAINT: egui::Color32 = rgb(115, 115, 115);
pub const TEXT_ON_ACTIVE: egui::Color32 = egui::Color32::WHITE;
pub const TEXT_VALUE: egui::Color32 = rgb(205, 205, 205);
pub const TEXT_RULER: egui::Color32 = rgb(165, 165, 165);

// ── Lines ──────────────────────────────────────────────────────────────────
pub const BORDER: egui::Color32 = rgb(61, 61, 61);
pub const BORDER_LIGHT: egui::Color32 = rgb(61, 61, 61);
pub const PANEL_SEP: egui::Color32 = rgb(48, 48, 48);
pub const SEPARATOR: egui::Color32 = rgb(61, 61, 61);

// ── Semantic accents ───────────────────────────────────────────────────────
pub const ACCENT_RED: egui::Color32 = rgb(241, 109, 122);
pub const ACCENT_GREEN: egui::Color32 = rgb(107, 200, 175);
pub const ACCENT_YELLOW: egui::Color32 = rgb(229, 195, 122);
pub const ACCENT_BLUE: egui::Color32 = rgb(121, 184, 244);

// ── Timeline grid ──────────────────────────────────────────────────────────
pub const GRID_MINOR: egui::Color32 = rgb(45, 45, 45);
pub const GRID_LINE: egui::Color32 = rgb(53, 53, 53);
pub const GRID_BEAT: egui::Color32 = rgb(72, 72, 72);
pub const GRID_LANE: egui::Color32 = rgb(53, 53, 53);
pub const GRID_LANE_CENTER: egui::Color32 = rgb(80, 80, 80);
pub const JUDGE_LINE: egui::Color32 = rgb(241, 109, 122);

// ── Note-type tick colours (overview minimap) ──────────────────────────────
pub const TICK_TAP: egui::Color32 = rgb(76, 201, 240);
pub const TICK_HOLD: egui::Color32 = rgb(255, 107, 138);
pub const TICK_TOUCH: egui::Color32 = rgb(179, 136, 255);
pub const TICK_SLIDE: egui::Color32 = rgb(255, 200, 87);

// ── Metrics ────────────────────────────────────────────────────────────────
/// Height of the top application toolbar.
pub const TOOLBAR_H: f32 = 28.0;

// ── Font sizes ─────────────────────────────────────────────────────────────
pub const FONT_SMALL: f32 = 11.0;
pub const FONT_BODY: f32 = 12.0;
pub const FONT_HEADER: f32 = 13.0;
