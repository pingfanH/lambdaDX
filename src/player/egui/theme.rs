use std::sync::{Arc, Once};

use egui_macroquad::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    Vec2,
};

pub const BG_VOID: Color32 = Color32::from_rgb(8, 11, 16);
pub const BG_PANEL: Color32 = Color32::from_rgb(17, 23, 32);
pub const BG_RAISED: Color32 = Color32::from_rgb(24, 33, 44);
pub const BG_HOVER: Color32 = Color32::from_rgb(34, 48, 61);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(244, 247, 250);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(168, 180, 194);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(104, 117, 133);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(53, 215, 232);
pub const ACCENT_CYAN_HOVER: Color32 = Color32::from_rgb(116, 231, 241);
pub const ACCENT_CORAL: Color32 = Color32::from_rgb(255, 107, 95);
pub const STATUS_SUCCESS: Color32 = Color32::from_rgb(105, 211, 145);
pub const BORDER: Color32 = Color32::from_rgb(43, 57, 72);

pub const RADIUS_CONTROL: CornerRadius = CornerRadius::same(6);
pub const RADIUS_PANEL: CornerRadius = CornerRadius::same(8);

static FONT_ONCE: Once = Once::new();

fn load_system_font(ctx: &egui::Context) {
    FONT_ONCE.call_once(|| {
        let candidates = [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "assets/Arial.ttf",
        ];

        for path in candidates {
            let Ok(data) = std::fs::read(path) else {
                continue;
            };
            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert(
                "player_system".to_owned(),
                Arc::new(FontData::from_owned(data)),
            );
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "player_system".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    });
}

pub fn apply(ctx: &egui::Context) {
    load_system_font(ctx);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(16.0, 10.0);
    style.spacing.window_margin = Margin::same(20);
    style.spacing.interact_size = Vec2::new(48.0, 48.0);
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            FontId::new(32.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_VOID;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_corner_radius = RADIUS_PANEL;
    visuals.extreme_bg_color = BG_VOID;
    visuals.faint_bg_color = BG_PANEL;
    visuals.selection.bg_fill = ACCENT_CYAN;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_CYAN_HOVER);
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.corner_radius = RADIUS_CONTROL;
    visuals.widgets.inactive.bg_fill = BG_RAISED;
    visuals.widgets.inactive.weak_bg_fill = BG_RAISED;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.corner_radius = RADIUS_CONTROL;
    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_CYAN_HOVER);
    visuals.widgets.hovered.corner_radius = RADIUS_CONTROL;
    visuals.widgets.active.bg_fill = ACCENT_CYAN;
    visuals.widgets.active.weak_bg_fill = ACCENT_CYAN;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, BG_VOID);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_CYAN);
    visuals.widgets.active.corner_radius = RADIUS_CONTROL;
    visuals.widgets.open = visuals.widgets.hovered;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    style.visuals = visuals;
    ctx.set_style(style);
}
