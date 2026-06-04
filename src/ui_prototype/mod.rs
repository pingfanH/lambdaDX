pub mod style;
pub mod components;

use egui_macroquad::egui;
use style::*;
use components::*;

/// Main entry point for the egui UI prototype matching Bevy Editor SVG layout
pub fn draw_editor(egui_ctx: &egui::Context) {
    // Configure dark visuals matching Bevy Editor
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG_DARK;
    visuals.panel_fill = BG_DARK;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.inactive.bg_fill = BG_BUTTON;
    visuals.widgets.hovered.bg_fill = BG_BUTTON_HOVER;
    visuals.widgets.active.bg_fill = ACCENT_BLUE;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
    egui_ctx.set_visuals(visuals);

    // Top panel: toolbar (matches SVG top bar)
    egui::TopBottomPanel::top("toolbar")
        .resizable(false)
        .exact_height(TOOLBAR_HEIGHT + PADDING)
        .show(egui_ctx, |ui| {
            toolbar::draw(ui);
        });

    // Left panel: sidebar (matches SVG left sidebar)
    egui::SidePanel::left("sidebar")
        .resizable(false)
        .exact_width(SIDEBAR_WIDTH + PADDING)
        .show(egui_ctx, |ui| {
            sidebar::draw(ui);
        });

    // Left panel: timeline (vertical, next to sidebar)
    egui::SidePanel::left("timeline")
        .resizable(false)
        .exact_width(TIMELINE_WIDTH)
        .show(egui_ctx, |ui| {
            timeline::draw_vertical(ui);
        });

    // Right panel: properties (matches SVG right panel)
    egui::SidePanel::right("properties")
        .resizable(false)
        .exact_width(RIGHT_PANEL_WIDTH + PADDING * 2.0)
        .show(egui_ctx, |ui| {
            panel::draw(ui);
        });

    // Central panel: viewport (matches SVG center area)
    egui::CentralPanel::default()
        .show(egui_ctx, |ui| {
            viewport::draw(ui);
        });
}