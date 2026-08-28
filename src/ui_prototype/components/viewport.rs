use super::button::*;
use crate::ui_prototype::style::*;
use egui_macroquad::egui::{self, Color32, CornerRadius, Pos2, Stroke, Vec2};

/// Center viewport with pad visualization matching Bevy Editor SVG
pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_VIEWPORT)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .show(ui, |ui| {
            let available = ui.available_size();
            let status_h = BUTTON_HEIGHT;
            let props_h = available.y * 0.4;
            let pad_area_h = available.y - status_h - props_h;

            // ── Pad area ──
            let pad_rect = ui.allocate_rect(
                egui::Rect::from_min_size(
                    ui.cursor().left_top(),
                    Vec2::new(available.x, pad_area_h),
                ),
                egui::Sense::hover(),
            );

            let cx = pad_rect.rect.center().x;
            let cy = pad_rect.rect.center().y;
            let pad_r =
                (pad_rect.rect.width().min(pad_rect.rect.height()) * 0.35).min(ICON_SIZE * 8.0);

            // Outer ring
            ui.painter()
                .circle_stroke(Pos2::new(cx, cy), pad_r, Stroke::new(UI_SCALE, RING_OUTER));

            // Inner ring
            ui.painter().circle_stroke(
                Pos2::new(cx, cy),
                pad_r * 0.55,
                Stroke::new(UI_SCALE * 0.8, RING_INNER),
            );

            // Center circle
            ui.painter().circle_stroke(
                Pos2::new(cx, cy),
                pad_r * 0.15,
                Stroke::new(UI_SCALE * 0.8, RING_OUTER),
            );

            // Button positions (A1-A8) - outer ring
            for i in 0..8 {
                let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 8.0;
                let bx = cx + angle.cos() * pad_r;
                let by = cy + angle.sin() * pad_r;
                let btn_r = pad_r * 0.13;
                ui.painter()
                    .circle_filled(Pos2::new(bx, by), btn_r, BG_BUTTON);
                ui.painter().circle_stroke(
                    Pos2::new(bx, by),
                    btn_r,
                    Stroke::new(UI_SCALE * 0.8, Color32::from_rgb(100, 100, 110)),
                );

                // Label
                let label = format!("A{}", i + 1);
                let lx = cx + angle.cos() * (pad_r + SPACING * 2.5);
                let ly = cy + angle.sin() * (pad_r + SPACING * 2.5);
                ui.painter().text(
                    Pos2::new(lx, ly),
                    egui::Align2::CENTER_CENTER,
                    &label,
                    egui::FontId::proportional(FONT_SMALL),
                    TEXT_DIM,
                );
            }

            // D1-D8 (inner ring)
            for i in 0..8 {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + (i as f32 + 0.5) * std::f32::consts::TAU / 8.0;
                let bx = cx + angle.cos() * pad_r * 0.55;
                let by = cy + angle.sin() * pad_r * 0.55;
                let btn_r = pad_r * 0.09;
                ui.painter()
                    .circle_filled(Pos2::new(bx, by), btn_r, BG_BUTTON);
                ui.painter().circle_stroke(
                    Pos2::new(bx, by),
                    btn_r,
                    Stroke::new(UI_SCALE * 0.5, Color32::from_rgb(90, 90, 100)),
                );
            }

            // Center zone (C)
            ui.painter()
                .circle_filled(Pos2::new(cx, cy), pad_r * 0.15, BG_BUTTON);
            ui.painter().circle_stroke(
                Pos2::new(cx, cy),
                pad_r * 0.15,
                Stroke::new(UI_SCALE * 0.8, Color32::from_rgb(90, 90, 100)),
            );
            ui.painter().text(
                Pos2::new(cx, cy),
                egui::Align2::CENTER_CENTER,
                "C",
                egui::FontId::proportional(FONT_BODY),
                TEXT_DIM,
            );

            // Slide path placeholder (arc)
            let start_angle = -std::f32::consts::FRAC_PI_2;
            let end_angle = -std::f32::consts::FRAC_PI_2 + 3.0 * std::f32::consts::TAU / 8.0;
            let r = pad_r * 0.85;
            let segments = 20;
            for i in 0..segments {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = start_angle + (end_angle - start_angle) * t0;
                let a1 = start_angle + (end_angle - start_angle) * t1;
                let x0 = cx + a0.cos() * r;
                let y0 = cy + a0.sin() * r;
                let x1 = cx + a1.cos() * r;
                let y1 = cy + a1.sin() * r;
                ui.painter().line_segment(
                    [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                    Stroke::new(UI_SCALE * 1.5, SLIDE_COLOR),
                );
            }

            // Slide tile dots
            for i in 0..=5 {
                let t = i as f32 / 5.0;
                let angle = start_angle + (end_angle - start_angle) * t;
                let x = cx + angle.cos() * r;
                let y = cy + angle.sin() * r;
                ui.painter().circle_filled(
                    Pos2::new(x, y),
                    UI_SCALE * 2.5,
                    Color32::from_rgba_premultiplied(255, 255, 255, 180),
                );
            }

            // ── Note Properties & Chart Info area ──
            let props_rect = egui::Rect::from_min_size(
                Pos2::new(pad_rect.rect.left(), pad_rect.rect.bottom()),
                Vec2::new(available.x, props_h),
            );
            let props_outer = ui.allocate_rect(props_rect, egui::Sense::hover());
            let props_ui_rect = props_outer.rect;

            // Draw border between pad and props
            ui.painter().line_segment(
                [
                    Pos2::new(props_ui_rect.left(), props_ui_rect.top()),
                    Pos2::new(props_ui_rect.right(), props_ui_rect.top()),
                ],
                Stroke::new(1.0_f32, BORDER_LIGHT),
            );

            // Fill background
            ui.painter()
                .rect_filled(props_ui_rect, CornerRadius::ZERO, BG_DARK);

            // Draw content using a child ui
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(props_ui_rect.shrink(PADDING))
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );

            // Note Properties
            child_ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING * 0.8;

                section_header(ui, "Note Properties");
                ui.add_space(SPACING);

                value_row(ui, "Type", "Tap");
                value_row(ui, "Time", "m4.000");
                value_row(ui, "Lane", "3");
                value_row(ui, "Duration", "m2.000");

                // Flags row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACING * 0.5;
                    for (i, flag) in ["Break", "Ex", "Star", "Tapless"].iter().enumerate() {
                        let btn = Button::new(
                            flag,
                            ButtonKind::Toggle(i == 0),
                            Vec2::new(ICON_SIZE * 3.0, BUTTON_HEIGHT * 0.9),
                        );
                        btn.show(ui);
                    }
                });
            });

            child_ui.add_space(SPACING);
            // Horizontal separator
            let sep_rect = child_ui.available_rect_before_wrap();
            child_ui.painter().line_segment(
                [
                    Pos2::new(sep_rect.left(), sep_rect.top()),
                    Pos2::new(sep_rect.right(), sep_rect.top()),
                ],
                Stroke::new(1.0_f32, SEPARATOR),
            );
            child_ui.add_space(SPACING);

            // Chart Info
            child_ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING * 0.8;

                section_header(ui, "Chart Info");
                ui.add_space(SPACING);

                value_row(ui, "Title", "Demo Song");
                value_row(ui, "Artist", "Unknown");
                value_row(ui, "BPM", "180.0");
                value_row(ui, "Offset", "0.000s");
            });

            // ── Status bar ──
            let status_rect = egui::Rect::from_min_size(
                Pos2::new(props_ui_rect.left(), props_ui_rect.bottom()),
                Vec2::new(available.x, status_h),
            );
            ui.painter()
                .rect_filled(status_rect, CornerRadius::ZERO, BG_DARK);
            ui.painter().line_segment(
                [
                    Pos2::new(status_rect.left(), status_rect.top()),
                    Pos2::new(status_rect.right(), status_rect.top()),
                ],
                Stroke::new(1.0_f32, BORDER_LIGHT),
            );

            ui.painter().text(
                Pos2::new(status_rect.left() + PADDING, status_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "Ready | 8 notes | BPM: 180 | 1.0x",
                egui::FontId::proportional(FONT_SMALL),
                TEXT_DIM,
            );
        });
}
