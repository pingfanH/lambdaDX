use egui_macroquad::egui::{self, Vec2, Color32, CornerRadius, Stroke, StrokeKind};
use super::button::*;
use crate::ui_prototype::style::*;

/// Right properties panel matching Bevy Editor SVG
pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_DARK)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING;

                // ── Note Properties ──
                section_header(ui, "Note Properties");
                ui.add_space(SPACING * 1.5);

                value_row(ui, "Type", "Tap");
                value_row(ui, "Time", "m4.000");
                value_row(ui, "Lane", "3");
                value_row(ui, "Duration", "m2.000");

                // Flags row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACING * 0.5;
                    for (i, flag) in ["Break", "Ex", "Star", "Tapless"].iter().enumerate() {
                        let btn = Button::new(flag, ButtonKind::Toggle(i == 0), Vec2::new(ICON_SIZE * 3.0, BUTTON_HEIGHT * 0.9));
                        btn.show(ui);
                    }
                });

                ui.add_space(SPACING * 1.5);
                separator(ui);

                // ── Chart Info ──
                section_header(ui, "Chart Info");
                ui.add_space(SPACING * 1.5);

                value_row(ui, "Title", "Demo Song");
                value_row(ui, "Artist", "Unknown");
                value_row(ui, "BPM", "180.0");
                value_row(ui, "Offset", "0.000s");

                ui.add_space(SPACING * 1.5);
                separator(ui);

                // ── Templates ──
                section_header(ui, "Templates");
                ui.add_space(SPACING * 1.5);

                // Template items
                template_item(ui, "Slide Pattern A", 3, true);
                template_item(ui, "Hold Sequence", 1, false);

                ui.add_space(SPACING);
                Button::new("+ New Template", ButtonKind::Normal, Vec2::new(ui.available_width(), BUTTON_HEIGHT)).show(ui);
            });
        });
}

fn template_item(ui: &mut egui::Ui, name: &str, instance_count: usize, selected: bool) {
    let bg = if selected { Color32::from_rgb(46, 71, 115) } else { BG_PANEL };
    let item_h = BUTTON_HEIGHT * 1.3;

    let btn = ui.allocate_response(Vec2::new(ui.available_width(), item_h), egui::Sense::click());
    ui.painter().rect_filled(btn.rect, CornerRadius::same(6), bg);
    ui.painter().rect_stroke(btn.rect, CornerRadius::same(6), Stroke::new(1.0_f32, BORDER_LIGHT), StrokeKind::Outside);

    // Name
    ui.painter().text(
        egui::pos2(btn.rect.left() + PADDING, btn.rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(FONT_BODY),
        TEXT_PRIMARY,
    );

    // Instance count
    let count_text = format!("×{}", instance_count);
    ui.painter().text(
        egui::pos2(btn.rect.right() - PADDING, btn.rect.center().y),
        egui::Align2::RIGHT_CENTER,
        &count_text,
        egui::FontId::proportional(FONT_BODY),
        TEXT_DIM,
    );
}