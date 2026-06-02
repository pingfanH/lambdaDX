use macroquad::prelude::*;
use crate::ui_prototype::style::*;

pub struct Timeline;

impl Timeline {
    pub fn draw(rect: UIRect) {
        // Background
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG_DARK);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER_LIGHT);

        let inner = rect.inset(PADDING);

        // ── Top toolbar row ──
        let toolbar_h = 24.0;
        Self::draw_toolbar(UIRect::new(inner.x, inner.y, inner.w, toolbar_h));

        // ── Ruler area ──
        let ruler_y = inner.y + toolbar_h + 4.0;
        let ruler_h = 20.0;
        Self::draw_ruler(UIRect::new(inner.x, ruler_y, inner.w, ruler_h));

        // ── Note lanes ──
        let lanes_y = ruler_y + ruler_h + 2.0;
        let lanes_h = inner.h - toolbar_h - ruler_h - 8.0;
        Self::draw_lanes(UIRect::new(inner.x, lanes_y, inner.w, lanes_h));

        // ── Playhead ──
        let playhead_x = inner.x + inner.w * 0.3;
        draw_line(playhead_x, ruler_y, playhead_x, inner.y + inner.h, 2.0, ACCENT_YELLOW);
    }

    fn draw_toolbar(rect: UIRect) {
        // Transport controls
        let btn_w = 24.0;
        let gap = 4.0;
        let mut x = rect.x;

        let controls = ["◀◀", "▶", "■", "⏺", "▶▶"];
        for label in &controls {
            draw_rectangle(x, rect.y, btn_w, rect.h, BG_BUTTON);
            let font_size = 10.0;
            let text_dims = measure_text(label, None, font_size as u16, 1.0);
            draw_text(label, x + (btn_w - text_dims.width) * 0.5, rect.y + rect.h * 0.5 + 4.0, font_size, TEXT_PRIMARY);
            x += btn_w + gap;
        }

        // Separator
        x += 4.0;
        draw_line(x, rect.y + 4.0, x, rect.y + rect.h - 4.0, 1.0, SEPARATOR);
        x += 8.0;

        // Time display
        let time_text = "0:04.230 / 1:23.456";
        draw_text(time_text, x, rect.y + rect.h * 0.5 + 4.0, 11.0, TEXT_SECONDARY);

        // Right side: snap/grid controls
        let right_x = rect.x + rect.w;
        let snap_text = "Snap: 1/4";
        let snap_dims = measure_text(snap_text, None, 11, 1.0);
        draw_text(snap_text, right_x - snap_dims.width - 4.0, rect.y + rect.h * 0.5 + 4.0, 11.0, TEXT_DIM);
    }

    fn draw_ruler(rect: UIRect) {
        // Ruler background
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG_PANEL);

        // Measure markers
        let measure_count = 16;
        let measure_w = rect.w / measure_count as f32;
        for i in 0..measure_count {
            let x = rect.x + measure_w * i as f32;
            // Tick mark
            draw_line(x, rect.y + rect.h - 6.0, x, rect.y + rect.h, 1.0, TEXT_DIM);
            // Measure number
            if i % 2 == 0 {
                let label = format!("{}", i + 1);
                draw_text(&label, x + 2.0, rect.y + 12.0, 9.0, TEXT_DIM);
            }
            // Sub-ticks
            for sub in 1..4 {
                let sx = x + measure_w * sub as f32 / 4.0;
                draw_line(sx, rect.y + rect.h - 3.0, sx, rect.y + rect.h, 1.0, Color::new(0.3, 0.3, 0.3, 1.0));
            }
        }
    }

    fn draw_lanes(rect: UIRect) {
        // Background
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, Color::new(0.11, 0.11, 0.13, 1.0));

        // Lane labels (8 lanes for A1-A8)
        let lane_h = rect.h / 8.0;
        let label_w = 20.0;
        for i in 0..8 {
            let y = rect.y + lane_h * i as f32;
            // Lane separator
            draw_line(rect.x, y, rect.x + rect.w, y, 0.5, Color::new(0.2, 0.2, 0.2, 1.0));
            // Lane label
            let label = format!("A{}", i + 1);
            draw_text(&label, rect.x + 2.0, y + lane_h * 0.5 + 4.0, 9.0, TEXT_DIM);
        }

        // Placeholder notes (some sample taps)
        let notes = [
            (0.15, 0, ACCENT_BLUE),
            (0.25, 2, ACCENT_BLUE),
            (0.30, 4, ACCENT_YELLOW), // break
            (0.45, 1, ACCENT_BLUE),
            (0.55, 6, ACCENT_BLUE),
            (0.70, 3, ACCENT_BLUE),
        ];

        for (frac, lane, color) in &notes {
            let nx = rect.x + label_w + (rect.w - label_w) * frac;
            let ny = rect.y + lane_h * (*lane as f32) + lane_h * 0.5;
            let r = lane_h * 0.35;
            draw_circle(nx, ny, r, *color);
            draw_circle_lines(nx, ny, r, 1.5, WHITE);
        }

        // Hold note placeholder
        let hold_x = rect.x + label_w + (rect.w - label_w) * 0.35;
        let hold_y = rect.y + lane_h * 5.0 + lane_h * 0.5;
        let hold_w = (rect.w - label_w) * 0.15;
        draw_rectangle(hold_x, hold_y - 4.0, hold_w, 8.0, ACCENT_BLUE);
        draw_circle(hold_x, hold_y, 6.0, ACCENT_BLUE);
        draw_circle(hold_x + hold_w, hold_y, 6.0, ACCENT_BLUE);

        // Bottom lane separator
        draw_line(rect.x, rect.y + rect.h, rect.x + rect.w, rect.y + rect.h, 0.5, Color::new(0.2, 0.2, 0.2, 1.0));
    }
}
