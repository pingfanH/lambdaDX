use egui_macroquad::egui::{self, Vec2, CornerRadius, Stroke};
use super::button::*;
use crate::ui_prototype::style::*;

/// Top toolbar matching Bevy Editor SVG layout
pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_TOOLBAR)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SPACING;

                // App icon (yellow circle with "M")
                let icon_size = ICON_SIZE * 0.8;
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(icon_size), egui::Sense::hover());
                ui.painter().rect_filled(icon_rect, CornerRadius::same(4), ACCENT_YELLOW);
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "M",
                    egui::FontId::proportional(FONT_ICON),
                    BG_DARK,
                );
                ui.add_space(SPACING * 1.5);

                // File section
                let btn_size = Vec2::new(ICON_SIZE * 2.5, BUTTON_HEIGHT);
                for label in &["Save", "Load", "Export"] {
                    Button::new(label, ButtonKind::Normal, btn_size).show(ui);
                }
                separator(ui);

                // Edit tools section
                let tool_size = Vec2::new(ICON_SIZE * 2.2, BUTTON_HEIGHT);
                for label in &["Select", "Tap", "Hold", "Slide", "Touch"] {
                    Button::new(label, ButtonKind::Normal, tool_size).show(ui);
                }

                // Spacer to push right-aligned items
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Zoom controls
                    let small_btn = Vec2::new(ICON_SIZE * 1.2, BUTTON_HEIGHT);
                    Button::new("-", ButtonKind::Normal, small_btn).show(ui);
                    Button::new("+", ButtonKind::Normal, small_btn).show(ui);
                    separator(ui);

                    // Zoom label
                    ui.label(egui::RichText::new("1.0x").color(TEXT_SECONDARY).size(FONT_BODY));
                    separator(ui);

                    // BPM display
                    ui.label(egui::RichText::new("BPM: 180").color(TEXT_SECONDARY).size(FONT_BODY));
                    separator(ui);

                    // Transport controls
                    let transport_size = Vec2::new(ICON_SIZE * 1.4, BUTTON_HEIGHT);
                    for label in &["▶", "●", "■"] {
                        Button::new(label, ButtonKind::Normal, transport_size).show(ui);
                    }
                });
            });
        });
}