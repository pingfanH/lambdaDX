use macroquad::prelude::*;
use crate::ui_prototype::style::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Normal,
    Primary,
    Toggle(bool),
    Icon,
    Separator,
}

pub struct Button<'a> {
    pub label: &'a str,
    pub kind: ButtonKind,
    pub rect: UIRect,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str, kind: ButtonKind, rect: UIRect) -> Self {
        Self { label, kind, rect }
    }

    pub fn draw(&self) -> bool {
        if self.kind == ButtonKind::Separator {
            draw_line(
                self.rect.x + self.rect.w * 0.5,
                self.rect.y + 4.0,
                self.rect.x + self.rect.w * 0.5,
                self.rect.y + self.rect.h - 4.0,
                1.0,
                SEPARATOR,
            );
            return false;
        }

        let (mx, my) = mouse_position();
        let hovered = self.rect.contains(mx, my);
        let pressed = hovered && is_mouse_button_down(MouseButton::Left);
        let clicked = hovered && is_mouse_button_released(MouseButton::Left);

        let bg = match self.kind {
            ButtonKind::Primary => ACCENT_BLUE,
            ButtonKind::Toggle(on) if on => BG_BUTTON_HOVER,
            _ => {
                if pressed { Color::new(0.20, 0.20, 0.22, 1.0) }
                else if hovered { Color::new(0.24, 0.24, 0.27, 1.0) }
                else { BG_BUTTON }
            }
        };

        draw_rectangle(
            self.rect.x + 0.5,
            self.rect.y + 0.5,
            self.rect.w - 1.0,
            self.rect.h - 1.0,
            bg,
        );
        draw_rectangle_lines(self.rect.x, self.rect.y, self.rect.w, self.rect.h, 1.0, BUTTON_BORDER);

        let text_color = match self.kind {
            ButtonKind::Primary => WHITE,
            ButtonKind::Toggle(false) => TEXT_DIM,
            _ => TEXT_PRIMARY,
        };

        let font_size = 11.0;
        let text_dims = measure_text(self.label, None, font_size as u16, 1.0);
        let tx = self.rect.x + (self.rect.w - text_dims.width) * 0.5;
        let ty = self.rect.y + (self.rect.h + text_dims.height) * 0.5 - 2.0;
        draw_text(self.label, tx, ty, font_size, text_color);

        clicked
    }
}

pub fn draw_icon_button(rect: UIRect, icon: &str, active: bool) -> bool {
    let (mx, my) = mouse_position();
    let hovered = rect.contains(mx, my);
    let clicked = hovered && is_mouse_button_released(MouseButton::Left);

    let bg = if active { ACCENT_BLUE } else if hovered { BG_BUTTON_HOVER } else { BG_BUTTON };
    draw_rectangle(rect.x + 0.5, rect.y + 0.5, rect.w - 1.0, rect.h - 1.0, bg);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BUTTON_BORDER);

    let font_size = 12.0;
    let text_dims = measure_text(icon, None, font_size as u16, 1.0);
    let tx = rect.x + (rect.w - text_dims.width) * 0.5;
    let ty = rect.y + (rect.h + text_dims.height) * 0.5 - 1.0;
    draw_text(icon, tx, ty, font_size, if active { WHITE } else { TEXT_SECONDARY });

    clicked
}

pub fn draw_section_label(rect: UIRect, label: &str) {
    draw_text(label, rect.x + 4.0, rect.y + rect.h * 0.5 + 4.0, 11.0, TEXT_DIM);
}

pub fn draw_value_row(rect: UIRect, label: &str, value: &str) {
    let font_size = 11.0;
    draw_text(label, rect.x + 4.0, rect.y + rect.h * 0.5 + 4.0, font_size, TEXT_SECONDARY);
    let value_dims = measure_text(value, None, font_size as u16, 1.0);
    draw_text(value, rect.x + rect.w - value_dims.width - 4.0, rect.y + rect.h * 0.5 + 4.0, font_size, TEXT_PRIMARY);
}

pub fn draw_text_input(rect: UIRect, text: &str, focused: bool) {
    let border = if focused { ACCENT_BLUE } else { BUTTON_BORDER };
    draw_rectangle(rect.x + 0.5, rect.y + 0.5, rect.w - 1.0, rect.h - 1.0, BG_INPUT);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    draw_text(text, rect.x + 6.0, rect.y + rect.h * 0.5 + 4.0, 11.0, TEXT_PRIMARY);
}

pub fn draw_separator(x: f32, y: f32, w: f32) {
    draw_line(x, y, x + w, y, 1.0, SEPARATOR);
}
