use egui_macroquad::egui::{self, Vec2, CornerRadius, Stroke, StrokeKind};
use super::button::*;
use crate::ui_prototype_bevy::style::*;

pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_TITLE_BAR)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // App icon - yellow circle
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::hover());
                ui.painter().circle_filled(icon_rect.center(), 10.0, ACCENT_YELLOW);
                ui.add_space(8.0);

                // Menu items
                for label in &["Edit", "Assets", "Game", "Scene"] {
                    Button::new(label, ButtonKind::Menu, Vec2::new(50.0, 24.0)).show(ui);
                }

                // Spacer
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Window controls (dots)
                    for _ in 0..3 {
                        let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot_rect.center(), 4.0, TEXT_DOT);
                        ui.painter().circle_stroke(dot_rect.center(), 4.0, Stroke::new(0.5_f32, TEXT_DOT));
                    }

                    ui.add_space(8.0);

                    // Layout button
                    Button::new("Layout", ButtonKind::Normal, Vec2::new(60.0, 22.0)).show(ui);

                    // Search box
                    let search_rect = ui.allocate_response(Vec2::new(130.0, 22.0), egui::Sense::hover());
                    ui.painter().rect_filled(search_rect.rect, CornerRadius::same(4), BG_SEARCH);
                    ui.painter().rect_stroke(search_rect.rect, CornerRadius::same(4), Stroke::new(1.0_f32, BORDER_PANEL), StrokeKind::Outside);
                    ui.painter().text(
                        egui::pos2(search_rect.rect.left() + 8.0, search_rect.rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "Search",
                        egui::FontId::proportional(12.0),
                        TEXT_DIM,
                    );
                });
            });
        });
}
