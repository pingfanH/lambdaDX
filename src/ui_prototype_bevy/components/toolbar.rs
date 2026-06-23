use egui_macroquad::egui::{self, Vec2, Stroke};
use super::button::*;
use crate::ui_prototype_bevy::style::*;

pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_TOOLBAR)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // Tool icons
                let tools = [
                    ("V", true),
                    ("T", false),
                    ("H", false),
                    ("S", false),
                    ("C", false),
                ];

                for (icon, active) in &tools {
                    small_icon_button(ui, icon, *active);
                }

                // Separator
                let sep_size = Vec2::new(1.0, 20.0);
                Button::new("", ButtonKind::Separator, sep_size).show(ui);

                // File operations
                for label in &["Save", "Load"] {
                    Button::new(label, ButtonKind::Normal, Vec2::new(50.0, 22.0)).show(ui);
                }

                // Separator
                Button::new("", ButtonKind::Separator, sep_size).show(ui);

                // Edit tools
                for label in &["Undo", "Redo"] {
                    Button::new(label, ButtonKind::Normal, Vec2::new(50.0, 22.0)).show(ui);
                }

                // Spacer
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Zoom
                    Button::new("-", ButtonKind::Normal, Vec2::new(24.0, 22.0)).show(ui);
                    ui.label(egui::RichText::new("100%").color(TEXT_SECONDARY).size(12.0));
                    Button::new("+", ButtonKind::Normal, Vec2::new(24.0, 22.0)).show(ui);

                    // Separator
                    Button::new("", ButtonKind::Separator, sep_size).show(ui);

                    // Snap
                    ui.label(egui::RichText::new("Snap").color(TEXT_DIM).size(12.0));
                    Button::new("1/4", ButtonKind::Normal, Vec2::new(36.0, 22.0)).show(ui);

                    // Separator
                    Button::new("", ButtonKind::Separator, sep_size).show(ui);

                    // BPM
                    ui.label(egui::RichText::new("BPM: 180").color(TEXT_SECONDARY).size(12.0));
                });
            });
        });
}
