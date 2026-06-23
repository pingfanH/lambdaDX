use egui_macroquad::egui::{self, Stroke};
use super::button::*;
use crate::ui_prototype_bevy::style::*;

pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_SIDEBAR)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                // Tool icons
                let tools = [
                    ("3D", "3D View", true),
                    ("A", "Select", false),
                    ("T", "Transform", false),
                    ("M", "Mesh", false),
                    ("M", "Material", false),
                    ("L", "Light", false),
                ];

                for (icon, _tooltip, active) in &tools {
                    icon_button(ui, icon, *active);
                }

                // Separator
                ui.add_space(6.0);
                let rect = ui.available_rect_before_wrap();
                let y = rect.top();
                ui.painter().line_segment(
                    [egui::pos2(rect.left() + 6.0, y), egui::pos2(rect.right() - 6.0, y)],
                    Stroke::new(1.0_f32, SEPARATOR),
                );
                ui.add_space(8.0);

                // Utility icons
                let utils = [
                    ("\u{26A1}", "Snap"),
                    ("\u{1F4CF}", "Grid"),
                    ("\u{1F50A}", "Audio"),
                ];

                for (icon, _tooltip) in &utils {
                    icon_button(ui, icon, false);
                }

                // Bottom settings
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    icon_button(ui, "\u{2699}", false);
                });
            });
        });
}
