pub mod style;
pub mod components;

use egui_macroquad::egui;
use style::*;
use components::*;

pub fn draw_editor(egui_ctx: &egui::Context) {
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

    // Title bar
    egui::TopBottomPanel::top("title_bar")
        .resizable(false)
        .exact_height(TITLE_BAR_HEIGHT)
        .show(egui_ctx, |ui| {
            title_bar::draw(ui);
        });

    // Toolbar
    egui::TopBottomPanel::top("toolbar")
        .resizable(false)
        .exact_height(TOOLBAR_HEIGHT)
        .show(egui_ctx, |ui| {
            toolbar::draw(ui);
        });

    // Timeline (bottom)
    egui::TopBottomPanel::bottom("timeline")
        .resizable(false)
        .exact_height(TIMELINE_HEIGHT)
        .show(egui_ctx, |ui| {
            timeline::draw(ui);
        });

    // Right panel (properties + scene hierarchy)
    egui::SidePanel::right("properties")
        .resizable(false)
        .exact_width(RIGHT_PANEL_WIDTH)
        .show(egui_ctx, |ui| {
            properties_panel::draw(ui);
        });

    // Left sidebar
    egui::SidePanel::left("sidebar")
        .resizable(false)
        .exact_width(SIDEBAR_WIDTH)
        .show(egui_ctx, |ui| {
            sidebar::draw(ui);
        });

    // Central viewport
    egui::CentralPanel::default()
        .show(egui_ctx, |ui| {
            viewport::draw(ui);
        });
}
