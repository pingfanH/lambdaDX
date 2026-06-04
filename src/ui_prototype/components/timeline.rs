use egui_macroquad::egui::{self, Vec2, Color32, CornerRadius, Stroke, Pos2};
use super::button::*;
use crate::ui_prototype::style::*;

/// Vertical timeline on the left side
pub fn draw_vertical(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_TIMELINE)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            let available = ui.available_size();

            // ── Transport controls (top) ──
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING;

                let btn_width = available.x - PADDING * 2.0;
                let btn_size = Vec2::new(btn_width, BUTTON_HEIGHT * 0.8);

                // Play/Stop/Record buttons
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACING * 0.5;
                    let small_btn = Vec2::new(btn_width / 3.0 - SPACING * 0.3, BUTTON_HEIGHT * 0.8);
                    Button::new("◀◀", ButtonKind::Normal, small_btn).show(ui);
                    Button::new("▶", ButtonKind::Normal, small_btn).show(ui);
                    Button::new("■", ButtonKind::Normal, small_btn).show(ui);
                });

                // Time display
                ui.label(egui::RichText::new("0:04.230").color(TEXT_SECONDARY).size(FONT_SMALL));
                ui.label(egui::RichText::new("/ 1:23.456").color(TEXT_DIM).size(FONT_SMALL));

                ui.add_space(SPACING);
            });

            // ── Vertical ruler ──
            let ruler_w = BUTTON_HEIGHT * 0.7;
            let ruler_h = available.y - BUTTON_HEIGHT * 5.0;

            let ruler_rect = ui.allocate_rect(
                egui::Rect::from_min_size(
                    ui.cursor().left_top(),
                    Vec2::new(available.x, ruler_h),
                ),
                egui::Sense::hover(),
            );

            let ruler_left = ruler_rect.rect.left() + (ruler_rect.rect.width() - ruler_w) / 2.0;
            let ruler_bg = egui::Rect::from_min_size(
                Pos2::new(ruler_left, ruler_rect.rect.top()),
                Vec2::new(ruler_w, ruler_h),
            );
            ui.painter().rect_filled(ruler_bg, CornerRadius::same(3), BG_RULER);

            // Draw ruler ticks (vertical)
            let measure_count = 16;
            let measure_h = ruler_h / measure_count as f32;
            for i in 0..measure_count {
                let y = ruler_rect.rect.top() + measure_h * i as f32;
                ui.painter().line_segment(
                    [egui::pos2(ruler_left + ruler_w - SPACING, y),
                     egui::pos2(ruler_left + ruler_w, y)],
                    Stroke::new(1.0_f32, TEXT_DIM),
                );

                if i % 2 == 0 {
                    let label = format!("{}", i + 1);
                    ui.painter().text(
                        egui::pos2(ruler_left - SPACING * 0.5, y + measure_h * 0.5),
                        egui::Align2::RIGHT_CENTER,
                        &label,
                        egui::FontId::proportional(FONT_SMALL),
                        TEXT_DIM,
                    );
                }

                // Sub-ticks
                for sub in 1..4 {
                    let sy = y + measure_h * sub as f32 / 4.0;
                    ui.painter().line_segment(
                        [egui::pos2(ruler_left + ruler_w - SPACING * 0.5, sy),
                         egui::pos2(ruler_left + ruler_w, sy)],
                        Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 50)),
                    );
                }
            }

            // Draw notes on the timeline (vertical)
            let notes = [
                (0.15, 0, ACCENT_BLUE),
                (0.25, 2, ACCENT_BLUE),
                (0.30, 4, ACCENT_YELLOW),
                (0.45, 1, ACCENT_BLUE),
                (0.55, 6, ACCENT_BLUE),
                (0.70, 3, ACCENT_BLUE),
            ];

            for (frac, _lane, color) in &notes {
                let ny = ruler_rect.rect.top() + ruler_h * frac;
                let nx = ruler_left + ruler_w * 0.5;
                let r = ruler_w * 0.35;
                ui.painter().circle_filled(Pos2::new(nx, ny), r, *color);
                ui.painter().circle_stroke(Pos2::new(nx, ny), r, Stroke::new(UI_SCALE * 0.5, Color32::WHITE));
            }

            // Playhead (horizontal line)
            let playhead_y = ruler_rect.rect.top() + ruler_h * 0.3;
            ui.painter().line_segment(
                [egui::pos2(ruler_left - SPACING, playhead_y),
                 egui::pos2(ruler_left + ruler_w + SPACING, playhead_y)],
                Stroke::new(UI_SCALE * 1.5, ACCENT_YELLOW),
            );

            // ── Snap info (bottom) ──
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("Snap: 1/4").color(TEXT_DIM).size(FONT_SMALL));
            });
        });
}

/// Legacy horizontal timeline (bottom) - kept for reference
pub fn draw(ui: &mut egui::Ui) {
    draw_vertical(ui);
}