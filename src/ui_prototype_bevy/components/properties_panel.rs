use egui_macroquad::egui::{self, Vec2, Color32, CornerRadius, Stroke, StrokeKind};
use super::button::*;
use crate::ui_prototype_bevy::style::*;

pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_RIGHT_UPPER)
        .stroke(Stroke::new(1.0_f32, BORDER_PANEL))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // ── Upper panel: Scene Hierarchy ──
                draw_scene_hierarchy(ui);

                ui.add_space(2.0);

                // ── Lower panel: Properties/Inspector ──
                draw_properties(ui);
            });
        });
}

fn draw_scene_hierarchy(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_SCENE_HIERARCHY)
        .stroke(Stroke::new(1.0_f32, BORDER_PANEL))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // Header bar
                egui::Frame::new()
                    .fill(BG_BUTTON)
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Scene").color(TEXT_PRIMARY).size(13.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Search icon
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
                                ui.painter().circle_stroke(rect.center(), 6.0, Stroke::new(1.5_f32, TEXT_DIM));
                                ui.painter().line_segment(
                                    [egui::pos2(rect.center().x + 4.0, rect.center().y + 4.0),
                                     egui::pos2(rect.center().x + 8.0, rect.center().y + 8.0)],
                                    Stroke::new(1.5_f32, TEXT_DIM),
                                );
                            });
                        });
                    });

                ui.add_space(6.0);

                // Scene tree items
                draw_tree_item(ui, "\u{25BC}", "Cube", true, 0);
                draw_tree_item(ui, "  \u{25B6}", "Transform", false, 1);
                draw_tree_item(ui, "  \u{25B6}", "Mesh", false, 1);
                draw_tree_item(ui, "  \u{25B6}", "Material", false, 1);
                draw_tree_item(ui, "\u{25B6}", "Camera", false, 0);
                draw_tree_item(ui, "\u{25B6}", "Light", false, 0);
            });
        });
}

fn draw_tree_item(ui: &mut egui::Ui, icon: &str, name: &str, selected: bool, indent: usize) {
    let bg = if selected { BG_HIGHLIGHT } else { Color32::TRANSPARENT };
    let h = 24.0;
    let indent_w = indent as f32 * 16.0;

    let btn = ui.allocate_response(Vec2::new(ui.available_width(), h), egui::Sense::click());
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(btn.rect, CornerRadius::same(3), bg);
    }

    if btn.hovered() && !selected {
        ui.painter().rect_filled(btn.rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(255, 255, 255, 8));
    }

    ui.painter().text(
        egui::pos2(btn.rect.left() + 8.0 + indent_w, btn.rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} {}", icon, name),
        egui::FontId::proportional(12.0),
        TEXT_PRIMARY,
    );
}

fn draw_properties(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_PANEL)
        .stroke(Stroke::new(1.0_f32, BORDER_PANEL))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // Header
                egui::Frame::new()
                    .fill(BG_BUTTON)
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Inspector").color(TEXT_PRIMARY).size(13.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Dots menu
                                for i in 0..3 {
                                    let x = ui.cursor().right() - 4.0 - i as f32 * 8.0;
                                    let y = ui.cursor().center().y;
                                    ui.painter().circle_filled(egui::pos2(x, y), 2.0, TEXT_DIM);
                                }
                            });
                        });
                    });

                ui.add_space(6.0);

                // Transform section
                draw_section_with_icon(ui, "\u{25BC}", "Transform");
                ui.add_space(4.0);

                draw_property_row(ui, "Position", "X: 0.0  Y: 0.0  Z: 0.0");
                draw_property_row(ui, "Rotation", "X: 0.0  Y: 0.0  Z: 0.0");
                draw_property_row(ui, "Scale", "X: 1.0  Y: 1.0  Z: 1.0");

                ui.add_space(8.0);
                separator_line(ui);

                // Mesh section
                draw_section_with_icon(ui, "\u{25BC}", "Mesh");
                ui.add_space(4.0);

                draw_property_row(ui, "Vertices", "24");
                draw_property_row(ui, "Triangles", "12");

                ui.add_space(8.0);
                separator_line(ui);

                // Material section
                draw_section_with_icon(ui, "\u{25BC}", "Material");
                ui.add_space(4.0);

                // Color swatch
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Color").color(TEXT_SECONDARY).size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let swatch_rect = ui.allocate_response(Vec2::new(60.0, 18.0), egui::Sense::hover());
                        ui.painter().rect_filled(swatch_rect.rect, CornerRadius::same(3), Color32::from_rgb(100, 149, 237));
                        ui.painter().rect_stroke(swatch_rect.rect, CornerRadius::same(3), Stroke::new(1.0_f32, BORDER), StrokeKind::Outside);
                    });
                });

                draw_property_row(ui, "Metallic", "0.0");
                draw_property_row(ui, "Roughness", "0.5");
            });
        });
}

fn draw_section_with_icon(ui: &mut egui::Ui, icon: &str, name: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new(icon).color(TEXT_DIM).size(10.0));
        ui.label(egui::RichText::new(name).color(TEXT_PRIMARY).size(13.0));
    });
}

fn draw_property_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_SECONDARY).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Value box
            let val_rect = ui.allocate_response(Vec2::new(140.0, 20.0), egui::Sense::hover());
            ui.painter().rect_filled(val_rect.rect, CornerRadius::same(3), BG_INPUT);
            ui.painter().rect_stroke(val_rect.rect, CornerRadius::same(3), Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 8)), StrokeKind::Outside);
            ui.painter().text(
                egui::pos2(val_rect.rect.left() + 6.0, val_rect.rect.center().y),
                egui::Align2::LEFT_CENTER,
                value,
                egui::FontId::proportional(11.0),
                TEXT_PRIMARY,
            );
        });
    });
}
