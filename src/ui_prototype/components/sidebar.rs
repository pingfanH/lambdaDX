use macroquad::prelude::*;
use crate::ui_prototype::style::*;
use super::button::*;

pub struct Sidebar;

impl Sidebar {
    pub fn draw(rect: UIRect) {
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG_DARK);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER_LIGHT);

        let inner = rect.inset(4.0);
        let icon_size = 28.0;
        let gap = 4.0;
        let mut y = inner.y;

        // Tool icons (placeholder letters)
        let tools = [
            ("V", "Select", true),
            ("T", "Tap", false),
            ("H", "Hold", false),
            ("S", "Slide", false),
            ("C", "Touch", false),
            ("★", "Star", false),
        ];

        for (icon, _tooltip, active) in &tools {
            let icon_rect = UIRect::new(inner.x + (inner.w - icon_size) * 0.5, y, icon_size, icon_size);
            let clicked = draw_icon_button(icon_rect, icon, *active);
            if clicked {
                // TODO: tool selection
            }
            y += icon_size + gap;
        }

        // Separator
        y += 4.0;
        draw_line(inner.x + 6.0, y, inner.x + inner.w - 6.0, y, 1.0, SEPARATOR);
        y += 8.0;

        // Utility buttons
        let utils = [
            ("⚡", "Snap"),
            ("📏", "Grid"),
            ("🔊", "Audio"),
        ];

        for (icon, _tooltip) in &utils {
            let icon_rect = UIRect::new(inner.x + (inner.w - icon_size) * 0.5, y, icon_size, icon_size);
            draw_icon_button(icon_rect, icon, false);
            y += icon_size + gap;
        }

        // Bottom: settings icon
        let bottom_y = inner.y + inner.h - icon_size;
        let icon_rect = UIRect::new(inner.x + (inner.w - icon_size) * 0.5, bottom_y, icon_size, icon_size);
        draw_icon_button(icon_rect, "⚙", false);
    }
}
