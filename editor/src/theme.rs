//! Design tokens for the editor chrome.
//!
//! Values are taken from the authored design (`assets/Editor-1.svg`, reproduced
//! in the task's reference SVG). Keep every colour/size here so the whole UI
//! reads from one palette instead of scattering literals through the widgets.

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
/// Window / workspace background.
pub const BG_WINDOW: egui::Color32 = rgb(29, 29, 30);
/// Top application toolbar strip.
pub const BG_TOOLBAR: egui::Color32 = rgb(32, 32, 32);
/// Right-hand info panel (pad preview + inspector).
pub const BG_PANEL: egui::Color32 = rgb(34, 34, 34);
/// Raised widget / tool button background.
pub const BG_WIDGET: egui::Color32 = rgb(48, 48, 53);
/// Widget background on hover.
pub const BG_WIDGET_HOVER: egui::Color32 = rgb(58, 58, 63);
/// Selected / active control background.
pub const BG_ACTIVE: egui::Color32 = rgb(71, 125, 173);

// ── Text ───────────────────────────────────────────────────────────────────
pub const TEXT: egui::Color32 = rgb(208, 208, 212);
pub const TEXT_DIM: egui::Color32 = rgb(141, 141, 148);
pub const TEXT_FAINT: egui::Color32 = rgb(110, 110, 116);
pub const TEXT_ON_ACTIVE: egui::Color32 = egui::Color32::WHITE;

// ── Lines ──────────────────────────────────────────────────────────────────
pub const BORDER: egui::Color32 = rgb(40, 40, 43);
pub const BORDER_LIGHT: egui::Color32 = rgb(57, 57, 61);
pub const SEPARATOR: egui::Color32 = rgb(81, 81, 88);

// ── Semantic accents ───────────────────────────────────────────────────────
pub const ACCENT_RED: egui::Color32 = rgb(255, 59, 80);
pub const ACCENT_GREEN: egui::Color32 = rgb(140, 205, 140);
pub const ACCENT_YELLOW: egui::Color32 = rgb(255, 202, 57);
pub const ACCENT_BLUE: egui::Color32 = rgb(150, 200, 255);

// ── Timeline grid ──────────────────────────────────────────────────────────
pub const GRID_MINOR: egui::Color32 = rgb(40, 40, 43);
pub const GRID_BEAT: egui::Color32 = rgb(90, 90, 95);
pub const GRID_BAR: egui::Color32 = rgb(140, 50, 50);
pub const JUDGE_LINE: egui::Color32 = rgb(200, 70, 70);

// ── Note-type tick colours (overview minimap) ──────────────────────────────
pub const TICK_TAP: egui::Color32 = rgb(76, 201, 240);
pub const TICK_HOLD: egui::Color32 = rgb(255, 107, 138);
pub const TICK_TOUCH: egui::Color32 = rgb(179, 136, 255);
pub const TICK_SLIDE: egui::Color32 = rgb(255, 200, 87);

// ── Metrics ────────────────────────────────────────────────────────────────
/// Height of the top application toolbar.
pub const TOOLBAR_H: f32 = 42.0;
/// Height of the horizontal tool palette strip above the timeline.
pub const TOOLS_STRIP_H: f32 = 44.0;
/// Square size of a single tool button.
pub const TOOL_BUTTON: f32 = 30.0;
/// Height of the bottom progress / overview bar.
pub const OVERVIEW_H: f32 = 26.0;
/// Outer window padding used by the panels.
pub const PADDING: f32 = 8.0;

// ── Font sizes ─────────────────────────────────────────────────────────────
pub const FONT_SMALL: f32 = 11.0;
pub const FONT_BODY: f32 = 12.0;
pub const FONT_HEADER: f32 = 13.0;
