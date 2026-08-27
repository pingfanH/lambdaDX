use egui_macroquad::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    Vec2,
};
use std::sync::{Arc, Once};

// ── Colors from Bevy Editor SVG prototype ──
pub const BG_DARK: Color32 = Color32::from_rgb(31, 31, 36);
pub const BG_PANEL: Color32 = Color32::from_rgb(42, 42, 46);
pub const BG_BUTTON: Color32 = Color32::from_rgb(54, 55, 59);
pub const BG_BUTTON_HOVER: Color32 = Color32::from_rgb(71, 72, 77);
pub const BG_INPUT: Color32 = Color32::from_rgb(70, 71, 76);
pub const BG_VIEWPORT: Color32 = Color32::from_rgb(26, 26, 30);
pub const BG_SIDEBAR: Color32 = Color32::from_rgb(42, 42, 46);
pub const BG_ACTIVE: Color32 = Color32::from_rgb(74, 125, 170);
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(32, 110, 201);
pub const ACCENT_YELLOW: Color32 = Color32::from_rgb(255, 202, 57);
pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(230, 149, 48);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 236, 236);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(168, 168, 168);
pub const TEXT_DIM: Color32 = Color32::from_rgb(131, 131, 133);
pub const BORDER: Color32 = Color32::from_rgb(30, 30, 30);
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(48, 48, 48);
pub const BUTTON_BORDER: Color32 = Color32::from_rgb(65, 65, 66);
pub const SEPARATOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 15);
pub const RING_OUTER: Color32 = Color32::from_rgb(75, 75, 85);
pub const RING_INNER: Color32 = Color32::from_rgb(60, 60, 70);

pub const CR2: CornerRadius = CornerRadius::same(2);

// ── Global scale factor ──
pub const UI_SCALE: f32 = 1.0;

// ── Sizes (not scaled by default for editor; prototype had 3.0) ──
pub const SIDEBAR_WIDTH: f32 = 48.0;
pub const RIGHT_PANEL_WIDTH: f32 = 220.0;
pub const BUTTON_HEIGHT: f32 = 24.0;
pub const ICON_SIZE: f32 = 28.0;
pub const SPACING: f32 = 6.0;
pub const PADDING: f32 = 8.0;

pub const FONT_SMALL: f32 = 11.0;
pub const FONT_BODY: f32 = 12.0;
pub const FONT_BUTTON: f32 = 12.0;
pub const FONT_HEADER: f32 = 13.0;
pub const FONT_ICON: f32 = 12.0;

static FONT_ONCE: Once = Once::new();

fn load_system_font(ctx: &egui::Context) {
    FONT_ONCE.call_once(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let font_candidates: Vec<String> = {
            let mut v = vec![
                "assets/Arial.ttf".to_string(),
                "assets/arial.ttf".to_string(),
                "assets/font.ttf".to_string(),
            ];
            if let Some(ref dir) = exe_dir {
                v.push(dir.join("assets/Arial.ttf").to_string_lossy().to_string());
            }
            v.extend([
                "/System/Library/Fonts/Helvetica.ttc".to_string(),
                "/Library/Fonts/Arial.ttf".to_string(),
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string(),
                "C:\\Windows\\Fonts\\segoeui.ttf".to_string(),
            ]);
            v
        };

        for path in &font_candidates {
            if let Ok(data) = std::fs::read(path) {
                let mut fonts = FontDefinitions::default();
                fonts
                    .font_data
                    .insert("system".to_owned(), Arc::new(FontData::from_owned(data)));
                if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
                    proportional.insert(0, "system".to_owned());
                }
                ctx.set_fonts(fonts);
                return;
            }
        }
    });
}

pub fn apply_style(ctx: &egui::Context) {
    load_system_font(ctx);

    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = Vec2::new(4.0, 4.0);
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    style.spacing.window_margin = Margin::same(6);

    style.text_styles = [
        (
            egui::TextStyle::Heading,
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.window_fill = BG_DARK;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CR2;
    visuals.panel_fill = BG_DARK;
    visuals.faint_bg_color = Color32::from_rgb(35, 35, 35);
    visuals.extreme_bg_color = Color32::from_rgb(24, 24, 24);
    visuals.selection.bg_fill = BG_ACTIVE;
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(120, 170, 220));
    visuals.widgets.noninteractive.bg_fill = BG_DARK;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(0.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = CR2;
    visuals.widgets.inactive.bg_fill = BG_BUTTON;
    visuals.widgets.inactive.weak_bg_fill = BG_BUTTON;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_LIGHT);
    visuals.widgets.inactive.corner_radius = CR2;
    visuals.widgets.hovered.bg_fill = BG_BUTTON_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_BUTTON_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.hovered.corner_radius = CR2;
    visuals.widgets.active.bg_fill = BG_ACTIVE;
    visuals.widgets.active.weak_bg_fill = BG_ACTIVE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.active.corner_radius = CR2;
    visuals.widgets.open.bg_fill = BG_BUTTON_HOVER;
    visuals.widgets.open.weak_bg_fill = BG_BUTTON_HOVER;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
    visuals.widgets.open.corner_radius = CR2;
    visuals.override_text_color = Some(TEXT_PRIMARY);

    style.visuals = visuals;
    ctx.set_style(style);
}
