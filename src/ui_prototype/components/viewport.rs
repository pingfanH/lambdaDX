use macroquad::prelude::*;
use crate::ui_prototype::style::*;

pub struct Viewport;

impl Viewport {
    pub fn draw(rect: UIRect) {
        // Background
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, Color::new(0.10, 0.10, 0.12, 1.0));
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER_LIGHT);

        // Center: Pad placeholder
        let pad_size = rect.w.min(rect.h) * 0.65;
        let cx = rect.x + rect.w * 0.5;
        let cy = rect.y + rect.h * 0.45;
        let pad_r = pad_size * 0.5;

        // Outer ring
        draw_circle_lines(cx, cy, pad_r, 2.0, Color::new(0.3, 0.3, 0.35, 1.0));
        // Inner ring
        draw_circle_lines(cx, cy, pad_r * 0.55, 1.5, Color::new(0.25, 0.25, 0.3, 1.0));
        // Center circle
        draw_circle_lines(cx, cy, pad_r * 0.15, 1.5, Color::new(0.3, 0.3, 0.35, 1.0));

        // Button positions (A1-A8 around outer ring)
        for i in 0..8 {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 8.0;
            let bx = cx + angle.cos() * pad_r;
            let by = cy + angle.sin() * pad_r;
            let btn_r = pad_r * 0.12;
            draw_circle(bx, by, btn_r, BG_BUTTON);
            draw_circle_lines(bx, by, btn_r, 1.5, Color::new(0.4, 0.4, 0.45, 1.0));

            // Button label
            let label = format!("A{}", i + 1);
            let font_size = 9.0;
            let text_dims = measure_text(&label, None, font_size as u16, 1.0);
            let lx = cx + angle.cos() * (pad_r + 14.0);
            let ly = cy + angle.sin() * (pad_r + 14.0);
            draw_text(&label, lx - text_dims.width * 0.5, ly + text_dims.height * 0.3, font_size, TEXT_DIM);
        }

        // D1-D8 (inner ring)
        for i in 0..8 {
            let angle = -std::f32::consts::FRAC_PI_2 + (i as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            let bx = cx + angle.cos() * pad_r * 0.55;
            let by = cy + angle.sin() * pad_r * 0.55;
            let btn_r = pad_r * 0.08;
            draw_circle(bx, by, btn_r, BG_BUTTON);
            draw_circle_lines(bx, by, btn_r, 1.0, Color::new(0.35, 0.35, 0.4, 1.0));
        }

        // Center zone (C)
        draw_circle(cx, cy, pad_r * 0.15, BG_BUTTON);
        draw_circle_lines(cx, cy, pad_r * 0.15, 1.5, Color::new(0.35, 0.35, 0.4, 1.0));
        let c_dims = measure_text("C", None, 10, 1.0);
        draw_text("C", cx - c_dims.width * 0.5, cy + c_dims.height * 0.3, 10.0, TEXT_DIM);

        // Slide path placeholder (drawn on pad)
        Self::draw_slide_preview(cx, cy, pad_r);

        // Status bar at bottom of viewport
        let status_h = 20.0;
        let status_y = rect.y + rect.h - status_h;
        draw_rectangle(rect.x, status_y, rect.w, status_h, BG_DARK);
        draw_line(rect.x, status_y, rect.x + rect.w, status_y, 1.0, BORDER_LIGHT);
        draw_text("Ready | 8 notes | BPM: 180 | 1.0x", rect.x + 8.0, status_y + 14.0, 10.0, TEXT_DIM);
    }

    fn draw_slide_preview(cx: f32, cy: f32, pad_r: f32) {
        // Draw a sample slide path (arc from A1 to A4)
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
            draw_line(x0, y0, x1, y1, 3.0, Color::new(0.9, 0.8, 0.2, 0.6));
        }

        // Slide tile dots
        for i in 0..=5 {
            let t = i as f32 / 5.0;
            let angle = start_angle + (end_angle - start_angle) * t;
            let x = cx + angle.cos() * r;
            let y = cy + angle.sin() * r;
            draw_circle(x, y, 4.0, Color::new(1.0, 1.0, 1.0, 0.7));
        }
    }
}
