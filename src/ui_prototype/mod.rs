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
    //visuals.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
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

    // Central area: timeline (30%) + viewport (70%)
    egui::CentralPanel::default()
        .show(egui_ctx, |ui| {
            let screen_rect = ui.max_rect();
            let available = ui.available_size();
            let timeline_w = available.x * 0.7;
            let viewport_w = available.x - timeline_w;

            let mut saved_timeline_rect = egui::Rect::NOTHING;

            ui.horizontal_top(|ui| {
                // Timeline (30%)
                let (_, timeline_rect) = ui.allocate_space(egui::Vec2::new(timeline_w, available.y));
                saved_timeline_rect = timeline_rect;
                let mut timeline_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(timeline_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                timeline::draw_vertical(&mut timeline_ui);
                panel::draw_scene_toggle_button(&mut timeline_ui, timeline_rect);

                // Viewport (70%)
                let (_, viewport_rect) = ui.allocate_space(egui::Vec2::new(viewport_w, available.y));
                let mut vp_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(viewport_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                viewport::draw(&mut vp_ui);

                // Toggle button on right edge of viewport
                panel::draw_toggle_button(&mut vp_ui, viewport_rect);
            });

            // Floating drawer overlays
            panel::draw_drawer(egui_ctx, screen_rect);
            panel::draw_scene_drawer(egui_ctx, saved_timeline_rect);
        });
}