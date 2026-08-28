use crate::ui_prototype_bevy::style::*;
use egui_macroquad::egui::{self, Align2, Color32, CornerRadius, FontId, Stroke, StrokeKind, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Normal,
    Primary,
    Toggle(bool),
    Icon,
    Separator,
    Menu,
}

pub struct Button<'a> {
    pub label: &'a str,
    pub kind: ButtonKind,
    pub size: Vec2,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str, kind: ButtonKind, size: Vec2) -> Self {
        Self { label, kind, size }
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        if self.kind == ButtonKind::Separator {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(1.0, self.size.y), egui::Sense::hover());
            ui.painter().line_segment(
                [
                    egui::pos2(rect.center().x, rect.top() + 4.0),
                    egui::pos2(rect.center().x, rect.bottom() - 4.0),
                ],
                Stroke::new(1.0_f32, SEPARATOR),
            );
            return false;
        }

        let bg = match self.kind {
            ButtonKind::Primary => ACCENT_BLUE,
            ButtonKind::Toggle(on) if on => BG_BUTTON_HOVER,
            ButtonKind::Menu => Color32::TRANSPARENT,
            _ => BG_BUTTON,
        };

        let text_color = match self.kind {
            ButtonKind::Primary => Color32::WHITE,
            ButtonKind::Toggle(false) => TEXT_DIM,
            ButtonKind::Menu => TEXT_PRIMARY,
            _ => TEXT_PRIMARY,
        };

        let btn = ui.allocate_response(self.size, egui::Sense::click());
        let rect = btn.rect;

        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(1.0_f32, BUTTON_BORDER),
                StrokeKind::Outside,
            );
        }

        if btn.hovered() {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(4),
                Color32::from_rgba_premultiplied(255, 255, 255, 10),
            );
        }

        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            self.label,
            FontId::proportional(13.0),
            text_color,
        );

        btn.clicked()
    }
}

pub fn icon_button(ui: &mut egui::Ui, icon: &str, active: bool) -> bool {
    let size = Vec2::splat(ICON_SIZE);
    let bg = if active { ACCENT_BLUE } else { BG_BUTTON };

    let btn = ui.allocate_response(size, egui::Sense::click());
    let rect = btn.rect;

    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, BUTTON_BORDER),
        StrokeKind::Outside,
    );

    if btn.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(5),
            Color32::from_rgba_premultiplied(255, 255, 255, 10),
        );
    }

    let text_color = if active {
        Color32::WHITE
    } else {
        TEXT_SECONDARY
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(14.0),
        text_color,
    );

    btn.clicked()
}

pub fn small_icon_button(ui: &mut egui::Ui, icon: &str, active: bool) -> bool {
    let size = Vec2::new(26.0, 18.0);
    let bg = if active { ACCENT_BLUE } else { BG_BUTTON };

    let btn = ui.allocate_response(size, egui::Sense::click());
    let rect = btn.rect;

    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, BUTTON_BORDER),
        StrokeKind::Outside,
    );

    if btn.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(5),
            Color32::from_rgba_premultiplied(255, 255, 255, 10),
        );
    }

    let text_color = if active {
        Color32::WHITE
    } else {
        TEXT_SECONDARY
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(12.0),
        text_color,
    );

    btn.clicked()
}

pub fn section_header(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_PRIMARY).size(14.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("\u{25BC}").color(TEXT_DIM).size(12.0));
        });
    });
}

pub fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_SECONDARY).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).color(TEXT_PRIMARY).size(13.0));
        });
    });
}

pub fn separator_line(ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top() + ui.spacing().item_spacing.y * 0.5;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        Stroke::new(1.0_f32, SEPARATOR),
    );
    ui.add_space(6.0);
}

pub fn draw_dot_matrix(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let spacing = 16.0;
    let dot_r = 1.5;
    let mut y = rect.top() + spacing * 0.5;
    while y < rect.bottom() {
        let mut x = rect.left() + spacing * 0.5;
        while x < rect.right() {
            painter.circle_filled(egui::pos2(x, y), dot_r, color);
            x += spacing;
        }
        y += spacing;
    }
}
