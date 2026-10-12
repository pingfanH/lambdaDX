//! egui theme + font setup for the editor.

use egui_macroquad::egui;
use std::sync::Arc;

use crate::theme::{
    BG_ACTIVE, BG_PANEL, BG_TOOLBAR, BG_WIDGET, BG_WIDGET_HOVER, BORDER, BORDER_LIGHT, TEXT,
    TEXT_DIM,
};

/// Install the CJK-capable font and the dark widget styling once per context.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_style(style());
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Prefer the same CJK-capable font the F1 params panel uses so opening that
    // panel does not swap the editor's typeface. Fall back to bundled Arial.
    let (name, data) = match crate::player::font::bytes() {
        Some(bytes) => ("editor_cjk", egui::FontData::from_owned(bytes)),
        None => (
            "editor_arial",
            egui::FontData::from_static(include_bytes!("../../../assets/Arial.ttf")),
        ),
    };
    fonts.font_data.insert(name.into(), Arc::new(data));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    ctx.set_fonts(fonts);
}

fn style() -> egui::Style {
    let mut style = egui::Style::default();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);

    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG_PANEL;
    visuals.panel_fill = BG_TOOLBAR;
    visuals.extreme_bg_color = BG_TOOLBAR;
    visuals.faint_bg_color = BG_WIDGET;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = BG_ACTIVE;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, BG_ACTIVE);

    visuals.widgets.noninteractive.bg_fill = BG_WIDGET;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);

    visuals.widgets.inactive.bg_fill = BG_WIDGET;
    visuals.widgets.inactive.weak_bg_fill = BG_WIDGET;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_DIM);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_LIGHT);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

    visuals.widgets.hovered.bg_fill = BG_WIDGET_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_WIDGET_HOVER;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, BG_ACTIVE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);

    visuals.widgets.active.bg_fill = BG_ACTIVE;
    visuals.widgets.active.weak_bg_fill = BG_ACTIVE;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, BG_ACTIVE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);

    visuals.widgets.open.bg_fill = BG_WIDGET_HOVER;
    visuals.widgets.open.weak_bg_fill = BG_WIDGET_HOVER;
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_LIGHT);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(4);

    style.visuals = visuals;
    style
}
