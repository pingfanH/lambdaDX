use macroquad::prelude::*;
use crate::ui_prototype::style::*;
use super::button::*;

pub struct RightPanel;

impl RightPanel {
    pub fn draw(rect: UIRect) {
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG_DARK);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER_LIGHT);

        let inner = rect.inset(PADDING);
        let row_h = 22.0;
        let label_w = 80.0;
        let mut y = inner.y;

        // ── Note Properties Section ──
        Self::draw_section_header(inner.x, y, inner.w, "Note Properties");
        y += 28.0;

        // Type
        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Type", "Tap");
        y += row_h + 2.0;

        // Time
        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Time", "m4.000");
        y += row_h + 2.0;

        // Lane
        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Lane", "3");
        y += row_h + 2.0;

        // Duration (for holds)
        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Duration", "m2.000");
        y += row_h + 2.0;

        // Flags
        let flags_rect = UIRect::new(inner.x, y, inner.w, row_h);
        Self::draw_flags_row(flags_rect);
        y += row_h + 12.0;

        // Separator
        draw_line(inner.x, y, inner.x + inner.w, y, 1.0, SEPARATOR);
        y += 12.0;

        // ── Chart Info Section ──
        Self::draw_section_header(inner.x, y, inner.w, "Chart Info");
        y += 28.0;

        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Title", "Demo Song");
        y += row_h + 2.0;

        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Artist", "Unknown");
        y += row_h + 2.0;

        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "BPM", "180.0");
        y += row_h + 2.0;

        draw_value_row(UIRect::new(inner.x, y, inner.w, row_h), "Offset", "0.000s");
        y += row_h + 12.0;

        // Separator
        draw_line(inner.x, y, inner.x + inner.w, y, 1.0, SEPARATOR);
        y += 12.0;

        // ── Template Section ──
        Self::draw_section_header(inner.x, y, inner.w, "Templates");
        y += 28.0;

        // Template list placeholder
        Self::draw_template_item(UIRect::new(inner.x, y, inner.w, 32.0), "Slide Pattern A", true);
        y += 36.0;
        Self::draw_template_item(UIRect::new(inner.x, y, inner.w, 32.0), "Hold Sequence", false);
        y += 36.0;

        // Add template button
        y += 4.0;
        let add_btn = Button::new("+ New Template", ButtonKind::Normal, UIRect::new(inner.x, y, inner.w, BUTTON_HEIGHT));
        add_btn.draw();
    }

    fn draw_section_header(x: f32, y: f32, w: f32, title: &str) {
        draw_text(title, x, y + 14.0, 12.0, TEXT_PRIMARY);
        // Collapse arrow placeholder
        draw_text("▼", x + w - 14.0, y + 14.0, 10.0, TEXT_DIM);
    }

    fn draw_flags_row(rect: UIRect) {
        let flags = ["Break", "Ex", "Star", "Tapless"];
        let flag_w = rect.w / flags.len() as f32;
        for (i, flag) in flags.iter().enumerate() {
            let fx = rect.x + flag_w * i as f32;
            let btn_rect = UIRect::new(fx + 1.0, rect.y, flag_w - 2.0, rect.h);
            let btn = Button::new(flag, ButtonKind::Toggle(i == 0), btn_rect);
            btn.draw();
        }
    }

    fn draw_template_item(rect: UIRect, name: &str, selected: bool) {
        let bg = if selected { Color::new(0.18, 0.28, 0.45, 1.0) } else { BG_PANEL };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER_LIGHT);

        let font_size = 11.0;
        draw_text(name, rect.x + 8.0, rect.y + rect.h * 0.5 + 4.0, font_size, TEXT_PRIMARY);

        // Instance count placeholder
        let count_text = "×3";
        let count_dims = measure_text(count_text, None, font_size as u16, 1.0);
        draw_text(count_text, rect.x + rect.w - count_dims.width - 8.0, rect.y + rect.h * 0.5 + 4.0, font_size, TEXT_DIM);
    }
}
