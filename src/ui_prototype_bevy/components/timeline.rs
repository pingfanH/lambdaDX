use egui_macroquad::egui::{self, Vec2, Color32, CornerRadius, Stroke, Pos2};
use super::button::*;
use crate::ui_prototype_bevy::style::*;

pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_TIMELINE)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            let available = ui.available_size();

            // ── Transport toolbar ──
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // Transport buttons
                for label in &["|<", ">", "||", "o", ">|"] {
                    Button::new(label, ButtonKind::Normal, Vec2::new(28.0, 24.0)).show(ui);
                }

                // Separator
                Button::new("", ButtonKind::Separator, Vec2::new(1.0, 20.0)).show(ui);

                // Time display
                ui.label(egui::RichText::new("0:04.230 / 1:23.456").color(TEXT_SECONDARY).size(12.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Snap: 1/4").color(TEXT_DIM).size(12.0));
                });
            });

            ui.add_space(4.0);

            // ── Ruler ──
            let ruler_h = 24.0;
            let ruler_rect = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().left_top(), Vec2::new(available.x, ruler_h)),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(ruler_rect.rect, CornerRadius::ZERO, BG_RULER);

            // Ruler ticks
            let measure_count = 16;
            let measure_w = ruler_rect.rect.width() / measure_count as f32;
            for i in 0..measure_count {
                let x = ruler_rect.rect.left() + measure_w * i as f32;
                ui.painter().line_segment(
                    [egui::pos2(x, ruler_rect.rect.bottom() - 8.0),
                     egui::pos2(x, ruler_rect.rect.bottom())],
                    Stroke::new(1.0_f32, TEXT_DIM),
                );

                if i % 2 == 0 {
                    ui.painter().text(
                        egui::pos2(x + 3.0, ruler_rect.rect.top() + 10.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", i + 1),
                        egui::FontId::proportional(10.0),
                        TEXT_DIM,
                    );
                }

                // Sub-ticks
                for sub in 1..4 {
                    let sx = x + measure_w * sub as f32 / 4.0;
                    ui.painter().line_segment(
                        [egui::pos2(sx, ruler_rect.rect.bottom() - 4.0),
                         egui::pos2(sx, ruler_rect.rect.bottom())],
                        Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 50)),
                    );
                }
            }

            ui.add_space(2.0);

            // ── Note lanes ──
            let lanes_h = available.y - ruler_h - 40.0;
            let lanes_rect = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().left_top(), Vec2::new(available.x, lanes_h.max(80.0))),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(lanes_rect.rect, CornerRadius::ZERO, BG_LANE);

            // Lane labels and separators
            let lane_count = 8;
            let lane_h = lanes_rect.rect.height() / lane_count as f32;
            let label_w = 28.0;

            for i in 0..lane_count {
                let y = lanes_rect.rect.top() + lane_h * i as f32;
                ui.painter().line_segment(
                    [egui::pos2(lanes_rect.rect.left(), y),
                     egui::pos2(lanes_rect.rect.right(), y)],
                    Stroke::new(0.5_f32, Color32::from_rgb(50, 50, 50)),
                );

                let label = format!("A{}", i + 1);
                ui.painter().text(
                    egui::pos2(lanes_rect.rect.left() + 4.0, y + lane_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    egui::FontId::proportional(10.0),
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
                let r = lane_h * 0.35;
                ui.painter().circle_filled(Pos2::new(nx, ny), r, *color);
                ui.painter().circle_stroke(Pos2::new(nx, ny), r, Stroke::new(1.5_f32, Color32::WHITE));
            }

            // Hold note
            let hold_x = lanes_rect.rect.left() + label_w + (lanes_rect.rect.width() - label_w) * 0.35;
            let hold_y = lanes_rect.rect.top() + lane_h * 5.0 + lane_h * 0.5;
            let hold_w = (lanes_rect.rect.width() - label_w) * 0.12;
            ui.painter().rect_filled(
                egui::Rect::from_min_size(Pos2::new(hold_x, hold_y - 5.0), Vec2::new(hold_w, 10.0)),
                CornerRadius::ZERO,
                ACCENT_BLUE,
            );
            ui.painter().circle_filled(Pos2::new(hold_x, hold_y), 6.0, ACCENT_BLUE);
            ui.painter().circle_filled(Pos2::new(hold_x + hold_w, hold_y), 6.0, ACCENT_BLUE);

            // ── Playhead ──
            let playhead_x = lanes_rect.rect.left() + lanes_rect.rect.width() * 0.3;
            ui.painter().line_segment(
                [egui::pos2(playhead_x, ruler_rect.rect.top()),
                 egui::pos2(playhead_x, lanes_rect.rect.bottom())],
                Stroke::new(2.0_f32, ACCENT_YELLOW),
            );

            // Playhead triangle
            let tri_size = 6.0;
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(playhead_x, ruler_rect.rect.top()),
                    Pos2::new(playhead_x - tri_size, ruler_rect.rect.top() - tri_size),
                    Pos2::new(playhead_x + tri_size, ruler_rect.rect.top() - tri_size),
                ],
                ACCENT_YELLOW,
                Stroke::NONE,
            ));
        });
}
