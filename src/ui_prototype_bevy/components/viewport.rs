use egui_macroquad::egui::{self, Color32, Stroke, Pos2};
use crate::ui_prototype_bevy::style::*;

pub fn draw(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BG_VIEWPORT)
        .stroke(Stroke::new(1.0_f32, BORDER_LIGHT))
        .show(ui, |ui| {
            let available = ui.available_size();

            // Main viewport area
            let viewport_rect = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().left_top(), available),
                egui::Sense::hover(),
            );

            let rect = viewport_rect.rect;
            let painter = ui.painter();

            // Draw dot grid background
            draw_dot_grid(painter, rect);

            // Draw 3D grid plane
            draw_3d_grid(painter, rect);

            // Draw axis indicators
            draw_axes(painter, rect);

            // Draw camera icon hint
            draw_camera_hint(painter, rect);
        });
}

fn draw_dot_grid(painter: &egui::Painter, rect: egui::Rect) {
    let spacing = 32.0;
    let dot_r = 1.0;
    let color = Color32::from_rgba_premultiplied(60, 60, 65, 120);
    let mut y = rect.top() + spacing * 0.5;
    while y < rect.bottom() {
        let mut x = rect.left() + spacing * 0.5;
        while x < rect.right() {
            painter.circle_filled(Pos2::new(x, y), dot_r, color);
            x += spacing;
        }
        y += spacing;
    }
}

fn draw_3d_grid(painter: &egui::Painter, rect: egui::Rect) {
    let cx = rect.center().x;
    let cy = rect.center().y + rect.height() * 0.15;
    let grid_w = rect.width() * 0.5;
    let grid_h = rect.height() * 0.3;
    let lines = 12;
    let grid_color = Color32::from_rgba_premultiplied(200, 200, 200, 40);

    // Horizontal lines with perspective
    for i in 0..=lines {
        let t = i as f32 / lines as f32;
        let y = cy - grid_h * 0.5 + grid_h * t;
        let shrink = 1.0 - (t - 0.5).abs() * 0.6;
        let half_w = grid_w * 0.5 * shrink;
        painter.line_segment(
            [Pos2::new(cx - half_w, y), Pos2::new(cx + half_w, y)],
            Stroke::new(1.0_f32, grid_color),
        );
    }

    // Vertical lines with perspective
    for i in 0..=lines {
        let t = i as f32 / lines as f32;
        let x = cx - grid_w * 0.5 + grid_w * t;
        let top_offset = (t - 0.5).abs() * grid_h * 0.3;
        painter.line_segment(
            [Pos2::new(x, cy - grid_h * 0.5 + top_offset),
             Pos2::new(x, cy + grid_h * 0.5 - top_offset)],
            Stroke::new(1.0_f32, grid_color),
        );
    }
}

fn draw_axes(painter: &egui::Painter, rect: egui::Rect) {
    let cx = rect.center().x;
    let cy = rect.center().y + rect.height() * 0.15;
    let axis_len = 60.0;

    // X axis (red)
    painter.line_segment(
        [Pos2::new(cx, cy), Pos2::new(cx + axis_len, cy)],
        Stroke::new(2.0_f32, Color32::from_rgb(220, 60, 60)),
    );
    painter.text(
        Pos2::new(cx + axis_len + 8.0, cy),
        egui::Align2::LEFT_CENTER,
        "X",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(220, 60, 60),
    );

    // Y axis (green)
    painter.line_segment(
        [Pos2::new(cx, cy), Pos2::new(cx, cy - axis_len)],
        Stroke::new(2.0_f32, Color32::from_rgb(60, 180, 60)),
    );
    painter.text(
        Pos2::new(cx, cy - axis_len - 8.0),
        egui::Align2::CENTER_BOTTOM,
        "Y",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(60, 180, 60),
    );

    // Z axis (blue) - diagonal for perspective
    painter.line_segment(
        [Pos2::new(cx, cy), Pos2::new(cx - axis_len * 0.7, cy - axis_len * 0.5)],
        Stroke::new(2.0_f32, Color32::from_rgb(60, 100, 220)),
    );
    painter.text(
        Pos2::new(cx - axis_len * 0.7 - 8.0, cy - axis_len * 0.5 - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        "Z",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(60, 100, 220),
    );
}

fn draw_camera_hint(painter: &egui::Painter, rect: egui::Rect) {
    // Camera frustum wireframe in top-right
    let cx = rect.right() - 80.0;
    let cy = rect.top() + 60.0;
    let w = 40.0;
    let h = 30.0;
    let d = 20.0;
    let color = Color32::from_rgba_premultiplied(200, 200, 200, 60);

    // Near plane
    painter.line_segment([Pos2::new(cx - w * 0.3, cy - h * 0.3), Pos2::new(cx + w * 0.3, cy - h * 0.3)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx + w * 0.3, cy - h * 0.3), Pos2::new(cx + w * 0.3, cy + h * 0.3)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx + w * 0.3, cy + h * 0.3), Pos2::new(cx - w * 0.3, cy + h * 0.3)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx - w * 0.3, cy + h * 0.3), Pos2::new(cx - w * 0.3, cy - h * 0.3)], Stroke::new(1.0_f32, color));

    // Far plane
    painter.line_segment([Pos2::new(cx - w * 0.5 - d, cy - h * 0.5 - d * 0.5), Pos2::new(cx + w * 0.5 + d, cy - h * 0.5 - d * 0.5)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx + w * 0.5 + d, cy - h * 0.5 - d * 0.5), Pos2::new(cx + w * 0.5 + d, cy + h * 0.5 + d * 0.5)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx + w * 0.5 + d, cy + h * 0.5 + d * 0.5), Pos2::new(cx - w * 0.5 - d, cy + h * 0.5 + d * 0.5)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx - w * 0.5 - d, cy + h * 0.5 + d * 0.5), Pos2::new(cx - w * 0.5 - d, cy - h * 0.5 - d * 0.5)], Stroke::new(1.0_f32, color));

    // Connecting lines
    painter.line_segment([Pos2::new(cx - w * 0.3, cy - h * 0.3), Pos2::new(cx - w * 0.5 - d, cy - h * 0.5 - d * 0.5)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx + w * 0.3, cy - h * 0.3), Pos2::new(cx + w * 0.5 + d, cy - h * 0.5 - d * 0.5)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx + w * 0.3, cy + h * 0.3), Pos2::new(cx + w * 0.5 + d, cy + h * 0.5 + d * 0.5)], Stroke::new(1.0_f32, color));
    painter.line_segment([Pos2::new(cx - w * 0.3, cy + h * 0.3), Pos2::new(cx - w * 0.5 - d, cy + h * 0.5 + d * 0.5)], Stroke::new(1.0_f32, color));
}
