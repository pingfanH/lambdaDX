use egui_macroquad::egui::{self, Stroke};
use super::button::*;
use crate::ui_prototype::style::*;

/// Left sidebar with tool icons matching Bevy Editor SVG
pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_SIDEBAR)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING;

                // Tool icons (matching SVG layout)
                let tools = [
                    ("V", "Select", true),
                    ("T", "Tap", false),
                    ("H", "Hold", false),
                    ("S", "Slide", false),
                    ("C", "Touch", false),
                    ("★", "Star", false),
                ];

                for (icon, _tooltip, active) in &tools {
                    icon_button(ui, icon, *active);
                }

                // Separator line
                ui.add_space(SPACING);
                let rect = ui.available_rect_before_wrap();
                let y = rect.top();
                ui.painter().line_segment(
                    [egui::pos2(rect.left() + SPACING, y), egui::pos2(rect.right() - SPACING, y)],
                    Stroke::new(1.0_f32, SEPARATOR),
                );
                ui.add_space(SPACING * 1.5);

                // Utility icons (matching SVG)
                let utils = [
                    ("⚡", "Snap"),
                    ("📏", "Grid"),
                    ("🔊", "Audio"),
                ];

                for (icon, _tooltip) in &utils {
                    icon_button(ui, icon, false);
                }

                // Spacer to push settings to bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    icon_button(ui, "⚙", false);
                });
            });
        });
}