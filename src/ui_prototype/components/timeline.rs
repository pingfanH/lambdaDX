use egui_macroquad::egui::{self, Vec2, Color32, CornerRadius, Stroke, Pos2};
use super::button::*;
use crate::ui_prototype::style::*;

/// Bottom timeline with transport, ruler, and note lanes matching Bevy Editor SVG
pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_TIMELINE)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            let available = ui.available_size();

            // ── Transport toolbar ──
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SPACING;

                let transport_size = Vec2::new(ICON_SIZE * 1.2, BUTTON_HEIGHT);
                for label in &["◀◀", "▶", "■", "⏺", "▶▶"] {
                    Button::new(label, ButtonKind::Normal, transport_size).show(ui);
                }
                separator(ui);

                ui.label(egui::RichText::new("0:04.230 / 1:23.456").color(TEXT_SECONDARY).size(FONT_BODY));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Snap: 1/4").color(TEXT_DIM).size(FONT_BODY));
                });
            });

            ui.add_space(SPACING);

            // ── Ruler ──
            let ruler_h = BUTTON_HEIGHT * 0.9;
            let ruler_rect = ui.allocate_rect(
                egui::Rect::from_min_size(
                    ui.cursor().left_top(),
                    Vec2::new(available.x, ruler_h),
                ),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(ruler_rect.rect, CornerRadius::ZERO, BG_RULER);

            // Draw ruler ticks
            let measure_count = 16;
            let measure_w = ruler_rect.rect.width() / measure_count as f32;
            for i in 0..measure_count {
                let x = ruler_rect.rect.left() + measure_w * i as f32;
                ui.painter().line_segment(
                    [egui::pos2(x, ruler_rect.rect.bottom() - SPACING),
                     egui::pos2(x, ruler_rect.rect.bottom())],
                    Stroke::new(1.0_f32, TEXT_DIM),
                );

                if i % 2 == 0 {
                    let label = format!("{}", i + 1);
                    ui.painter().text(
                        egui::pos2(x + SPACING * 0.5, ruler_rect.rect.top() + ruler_h * 0.5),
                        egui::Align2::LEFT_CENTER,
                        &label,
                        egui::FontId::proportional(FONT_SMALL),
                        TEXT_DIM,
                    );
                }

                // Sub-ticks
                for sub in 1..4 {
                    let sx = x + measure_w * sub as f32 / 4.0;
                    ui.painter().line_segment(
                        [egui::pos2(sx, ruler_rect.rect.bottom() - SPACING * 0.5),
                         egui::pos2(sx, ruler_rect.rect.bottom())],
                        Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 50)),
                    );
                }
            }

            ui.add_space(SPACING * 0.5);

            // ── Note lanes ──
            let lanes_h = available.y - ruler_h - BUTTON_HEIGHT * 2.5;
            let lanes_rect = ui.allocate_rect(
                egui::Rect::from_min_size(
                    ui.cursor().left_top(),
                    Vec2::new(available.x, lanes_h.max(BUTTON_HEIGHT * 4.0)),
                ),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(lanes_rect.rect, CornerRadius::ZERO, BG_LANE);

            // Lane labels and separators
            let lane_count = 8;
            let lane_h = lanes_rect.rect.height() / lane_count as f32;
            let label_w = ICON_SIZE * 1.2;

            for i in 0..lane_count {
                let y = lanes_rect.rect.top() + lane_h * i as f32;
                ui.painter().line_segment(
                    [egui::pos2(lanes_rect.rect.left(), y),
                     egui::pos2(lanes_rect.rect.right(), y)],
                    Stroke::new(0.5_f32, Color32::from_rgb(50, 50, 50)),
                );

                let label = format!("A{}", i + 1);
                ui.painter().text(
                    egui::pos2(lanes_rect.rect.left() + SPACING * 0.5, y + lane_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    egui::FontId::proportional(FONT_SMALL),
                    TEXT_DIM,
                );
            }

            // Sample notes
            let notes = [
                (0.15, 0, ACCENT_BLUE),
                (0.25, 2, ACCENT_BLUE),
                (0.30, 4, ACCENT_YELLOW),
                (0.45, 1, ACCENT_BLUE),
                (0.55, 6, ACCENT_BLUE),
                (0.70, 3, ACCENT_BLUE),
            ];

            for (frac, lane, color) in &notes {
                let nx = lanes_rect.rect.left() + label_w + (lanes_rect.rect.width() - label_w) * frac;
                let ny = lanes_rect.rect.top() + lane_h * (*lane as f32) + lane_h * 0.5;
                let r = lane_h * 0.38;
                ui.painter().circle_filled(Pos2::new(nx, ny), r, *color);
                ui.painter().circle_stroke(Pos2::new(nx, ny), r, Stroke::new(UI_SCALE, Color32::WHITE));
            }

            // Hold note placeholder
            let hold_x = lanes_rect.rect.left() + label_w + (lanes_rect.rect.width() - label_w) * 0.35;
            let hold_y = lanes_rect.rect.top() + lane_h * 5.0 + lane_h * 0.5;
            let hold_w = (lanes_rect.rect.width() - label_w) * 0.15;
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    Pos2::new(hold_x, hold_y - UI_SCALE * 3.0),
                    Vec2::new(hold_w, UI_SCALE * 6.0),
                ),
                CornerRadius::ZERO,
                ACCENT_BLUE,
            );
            ui.painter().circle_filled(Pos2::new(hold_x, hold_y), UI_SCALE * 4.0, ACCENT_BLUE);
            ui.painter().circle_filled(Pos2::new(hold_x + hold_w, hold_y), UI_SCALE * 4.0, ACCENT_BLUE);

            // Bottom separator
            ui.painter().line_segment(
                [egui::pos2(lanes_rect.rect.left(), lanes_rect.rect.bottom()),
                 egui::pos2(lanes_rect.rect.right(), lanes_rect.rect.bottom())],
                Stroke::new(0.5_f32, Color32::from_rgb(50, 50, 50)),
            );

            // ── Playhead ──
            let playhead_x = lanes_rect.rect.left() + lanes_rect.rect.width() * 0.3;
            ui.painter().line_segment(
                [egui::pos2(playhead_x, ruler_rect.rect.top()),
                 egui::pos2(playhead_x, lanes_rect.rect.bottom())],
                Stroke::new(UI_SCALE * 1.5, ACCENT_YELLOW),
            );
        });
}