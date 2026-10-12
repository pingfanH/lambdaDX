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
/// Window / workspace background (`#1D1D1E`).
pub const BG_WINDOW: egui::Color32 = rgb(29, 29, 30);
/// Top application toolbar strip (`#202020`).
pub const BG_TOOLBAR: egui::Color32 = rgb(32, 32, 32);
/// Floating tool palette panel (design `#252529`).
pub const BG_TOOLS_PANEL: egui::Color32 = rgb(37, 37, 41);
/// Right-hand info panel (pad preview + inspector) (`#222222`).
pub const BG_PANEL: egui::Color32 = rgb(34, 34, 34);
/// Bottom progress / overview bar (design `#2B2B2D`).
pub const BG_OVERVIEW: egui::Color32 = rgb(43, 43, 45);
/// Raised widget / tool button background (design `#303035`).
pub const BG_WIDGET: egui::Color32 = rgb(48, 48, 53);
/// Widget background on hover.
pub const BG_WIDGET_HOVER: egui::Color32 = rgb(58, 58, 63);
/// Panel collapse handle tab (design `#45454A`).
pub const BG_HANDLE: egui::Color32 = rgb(69, 69, 74);
/// Selected / active control background (design `#477DAD`).
pub const BG_ACTIVE: egui::Color32 = rgb(71, 125, 173);

// ── Text ───────────────────────────────────────────────────────────────────
pub const TEXT: egui::Color32 = rgb(211, 211, 214);
pub const TEXT_DIM: egui::Color32 = rgb(141, 141, 148);
pub const TEXT_FAINT: egui::Color32 = rgb(110, 110, 116);
pub const TEXT_ON_ACTIVE: egui::Color32 = egui::Color32::WHITE;
/// Inspector value text (design `#BDBDC2`).
pub const TEXT_VALUE: egui::Color32 = rgb(189, 189, 194);
/// Timeline ruler / lane label text (design `#B6B6BB`).
pub const TEXT_RULER: egui::Color32 = rgb(182, 182, 187);

// ── Lines ──────────────────────────────────────────────────────────────────
pub const BORDER: egui::Color32 = rgb(40, 40, 43);
pub const BORDER_LIGHT: egui::Color32 = rgb(57, 57, 61);
/// Inspector row separator (design `#2A2A2D`).
pub const PANEL_SEP: egui::Color32 = rgb(42, 42, 45);
pub const SEPARATOR: egui::Color32 = rgb(81, 81, 88);

// ── Semantic accents ───────────────────────────────────────────────────────
pub const ACCENT_RED: egui::Color32 = rgb(255, 59, 80);
pub const ACCENT_GREEN: egui::Color32 = rgb(140, 205, 140);
pub const ACCENT_YELLOW: egui::Color32 = rgb(255, 202, 57);
pub const ACCENT_BLUE: egui::Color32 = rgb(150, 200, 255);

// ── Timeline grid ──────────────────────────────────────────────────────────
/// Sub-beat grid line (design `#28282B`).
pub const GRID_MINOR: egui::Color32 = rgb(40, 40, 43);
/// Beat line (design lane-separator grey `#39393D`).
pub const GRID_LINE: egui::Color32 = rgb(57, 57, 61);
/// Measure line — the design's brightest grid grey (`#5A5A5F`).
pub const GRID_BEAT: egui::Color32 = rgb(90, 90, 95);
/// Lane separator between columns (design `#39393D`).
pub const GRID_LANE: egui::Color32 = rgb(57, 57, 61);
/// Center lane separator (design `#77777D`).
pub const GRID_LANE_CENTER: egui::Color32 = rgb(119, 119, 125);
/// Judgment line / playhead (design `#FF3B50`).
pub const JUDGE_LINE: egui::Color32 = rgb(255, 59, 80);

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
