use macroquad::prelude::*;
use crate::ui_prototype::style::*;
use super::button::*;

pub struct Toolbar;

impl Toolbar {
    pub fn draw(rect: UIRect) {
        // Background
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG_DARK);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER_LIGHT);

        let inner = rect.inset(4.0);
        let mut x = inner.x;
        let btn_h = inner.h - 2.0;

        // App icon placeholder
        draw_rectangle(x, inner.y, 20.0, btn_h, ACCENT_YELLOW);
        let font_size = 10.0;
        draw_text("M", x + 6.0, inner.y + btn_h * 0.5 + 4.0, font_size, BG_DARK);
        x += 26.0;

        // Separator
        x += 4.0;
        draw_line(x, inner.y + 4.0, x, inner.y + btn_h - 4.0, 1.0, SEPARATOR);
        x += 8.0;

        // Tool buttons (Play, Record, Stop)
        let tools = [
            ("▶", ButtonKind::Normal),
            ("●", ButtonKind::Normal),
            ("■", ButtonKind::Normal),
        ];
        for (label, kind) in &tools {
            let w = 28.0;
            let btn = Button::new(label, *kind, UIRect::new(x, inner.y, w, btn_h));
            btn.draw();
            x += w + 2.0;
        }

        // Separator
        x += 2.0;
        draw_line(x, inner.y + 4.0, x, inner.y + btn_h - 4.0, 1.0, SEPARATOR);
        x += 8.0;

        // Action buttons
        let actions = ["Save", "Load", "Export"];
        for label in &actions {
            let w = 52.0;
            let btn = Button::new(label, ButtonKind::Normal, UIRect::new(x, inner.y, w, btn_h));
            btn.draw();
            x += w + 2.0;
        }

        // Separator
        x += 2.0;
        draw_line(x, inner.y + 4.0, x, inner.y + btn_h - 4.0, 1.0, SEPARATOR);
        x += 8.0;

        // Edit tools
        let edit_tools = ["Select", "Tap", "Hold", "Slide", "Touch"];
        for label in &edit_tools {
            let w = 50.0;
            let btn = Button::new(label, ButtonKind::Normal, UIRect::new(x, inner.y, w, btn_h));
            btn.draw();
            x += w + 2.0;
        }

        // Right side: view controls
        let right_x = inner.x + inner.w;
        let mut rx = right_x;

        // Zoom label
        let zoom_text = "1.0x";
        let zoom_dims = measure_text(zoom_text, None, 11, 1.0);
        rx -= zoom_dims.width + 8.0;
        draw_text(zoom_text, rx, inner.y + btn_h * 0.5 + 4.0, 11.0, TEXT_SECONDARY);

        // Zoom buttons
        rx -= 24.0;
        let btn = Button::new("-", ButtonKind::Normal, UIRect::new(rx, inner.y, 22.0, btn_h));
        btn.draw();
        rx -= 24.0;
        let btn = Button::new("+", ButtonKind::Normal, UIRect::new(rx, inner.y, 22.0, btn_h));
        btn.draw();

        rx -= 8.0;
        draw_line(rx, inner.y + 4.0, rx, inner.y + btn_h - 4.0, 1.0, SEPARATOR);
        rx -= 8.0;

        // BPM display
        let bpm_text = "BPM: 180";
        let bpm_dims = measure_text(bpm_text, None, 11, 1.0);
        rx -= bpm_dims.width + 8.0;
        draw_text(bpm_text, rx, inner.y + btn_h * 0.5 + 4.0, 11.0, TEXT_SECONDARY);
    }
}
