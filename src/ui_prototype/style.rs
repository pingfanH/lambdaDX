use egui_macroquad::egui::Color32;

// ── Colors from Bevy Editor SVG ──
pub const BG_DARK: Color32 = Color32::from_rgb(31, 31, 36); // #1F1F24
pub const TRANSPARENT: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0); // TRANSPARENT
pub const BG_PANEL: Color32 = Color32::from_rgb(42, 42, 46); // #2A2A2E
pub const BG_BUTTON: Color32 = Color32::from_rgb(54, 55, 59); // #36373B
pub const BG_BUTTON_HOVER: Color32 = Color32::from_rgb(71, 72, 77); // #47484D
pub const BG_INPUT: Color32 = Color32::from_rgb(70, 71, 76); // #46474C
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(32, 110, 201); // #206EC9
pub const ACCENT_YELLOW: Color32 = Color32::from_rgb(255, 202, 57); // #FFCA39
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 236, 236); // #ECECEC
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(168, 168, 168); // #A8A8A8
pub const TEXT_DIM: Color32 = Color32::from_rgb(131, 131, 133); // #838385
pub const BORDER: Color32 = Color32::from_rgb(64, 64, 64); // #404040
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(48, 48, 48); // #303030
pub const BUTTON_BORDER: Color32 = Color32::from_rgb(65, 65, 66); // #414142
pub const SEPARATOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 15); // white 6% opacity

// Additional colors from SVG
pub const BG_VIEWPORT: Color32 = Color32::from_rgb(26, 26, 30); // #1A1A1E
pub const BG_SIDEBAR: Color32 = Color32::from_rgb(42, 42, 46); // #2A2A2E
pub const BG_TOOLBAR: Color32 = Color32::from_rgb(42, 42, 46); // #2A2A2E
pub const BG_TIMELINE: Color32 = Color32::from_rgb(42, 42, 46); // #2A2A2E
pub const BG_RULER: Color32 = Color32::from_rgb(42, 42, 46); // #2A2A2E
pub const BG_LANE: Color32 = Color32::from_rgb(28, 28, 32); // #1C1C20
pub const RING_OUTER: Color32 = Color32::from_rgb(75, 75, 85); // #4B4B55
pub const RING_INNER: Color32 = Color32::from_rgb(60, 60, 70); // #3C3C46
pub const SLIDE_COLOR: Color32 = Color32::from_rgba_premultiplied(230, 204, 51, 150); // Yellow slide path

// ── Global scale factor (change this to resize everything) ──
pub const UI_SCALE: f32 = 3.0;

// ── Base sizes (will be multiplied by UI_SCALE) ──
pub const TOOLBAR_HEIGHT: f32 = 24.0 * UI_SCALE;
pub const SIDEBAR_WIDTH: f32 = 36.0 * UI_SCALE;
pub const RIGHT_PANEL_WIDTH: f32 = 180.0 * UI_SCALE;
pub const TIMELINE_WIDTH: f32 = 400.0 * UI_SCALE; // Vertical timeline width
pub const TIMELINE_HEIGHT: f32 = 140.0 * UI_SCALE; // Keep for backward compat
pub const BUTTON_HEIGHT: f32 = 18.0 * UI_SCALE;
pub const ICON_SIZE: f32 = 22.0 * UI_SCALE;
pub const SPACING: f32 = 5.0 * UI_SCALE;
pub const PADDING: f32 = 6.0 * UI_SCALE;

// ── Font sizes (will be multiplied by UI_SCALE) ──
pub const FONT_SMALL: f32 = 7.0 * UI_SCALE;
pub const FONT_BODY: f32 = 8.0 * UI_SCALE;
pub const FONT_BUTTON: f32 = 7.5 * UI_SCALE;
pub const FONT_HEADER: f32 = 8.5 * UI_SCALE;
pub const FONT_ICON: f32 = 8.0 * UI_SCALE;
