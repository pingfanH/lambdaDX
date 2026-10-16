//! Small reusable egui widgets shared by the toolbar, inspector and tools strip.

use egui_macroquad::egui;

use crate::theme::{
    BG_ACTIVE, FONT_BODY, FONT_HEADER, PANEL_SEP, SEPARATOR, TEXT,
    TEXT_DIM, TEXT_FAINT, TEXT_ON_ACTIVE, TEXT_VALUE,
};

/// A flat toolbar button. The design's top bar is plain text; `active` tints it
/// with the accent fill. `tip` is an optional hover tooltip (empty = none).
pub fn toolbar_button(ui: &mut egui::Ui, label: &str, active: bool, tip: &str) -> bool {
    let (fill, color) = if active {
        (BG_ACTIVE, TEXT_ON_ACTIVE)
    } else {
        (egui::Color32::TRANSPARENT, TEXT)
    };
    let text = egui::RichText::new(label).size(FONT_BODY).color(color);
    let response = ui.add(egui::Button::new(text).fill(fill).frame(active));
    if tip.is_empty() {
        response.clicked()
    } else {
        response.on_hover_text(tip).clicked()
    }
}

/// A flat toolbar button that can be greyed out when `enabled` is false (used by
/// Undo/Redo). A disabled button is non-interactive and dimmed.
pub fn toolbar_button_enabled(ui: &mut egui::Ui, label: &str, enabled: bool, tip: &str) -> bool {
    let color = if enabled { TEXT } else { TEXT_FAINT };
    let text = egui::RichText::new(label).size(FONT_BODY).color(color);
    let response = ui.add_enabled(enabled, egui::Button::new(text).frame(false));
    if tip.is_empty() {
        response.clicked()
    } else {
        response.on_hover_text(tip).clicked()
    }
}

/// A vertical divider between toolbar groups.
pub fn toolbar_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 18.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top() + 2.0),
            egui::pos2(rect.center().x, rect.bottom() - 2.0),
        ],
        egui::Stroke::new(1.0_f32, SEPARATOR),
    );
}

/// A collapsible section heading.
pub fn section_header(ui: &mut egui::Ui, label: &str, open: bool) -> bool {
    ui.horizontal(|ui| {
        let arrow = if open { "▾" } else { "▸" };
        let text = egui::RichText::new(format!("{arrow}  {label}"))
            .size(FONT_HEADER)
            .color(TEXT);
        ui.add(egui::Button::new(text).frame(false)).clicked()
    })
    .inner
}

/// A label/value row: dim caption on the left, value right-aligned.
pub fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(FONT_BODY).color(TEXT_DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(FONT_BODY).color(TEXT_VALUE));
        });
    });
}

/// A label followed by an editable single-line text field (inspector metadata).
pub fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(FONT_BODY).color(TEXT_DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed |= ui
                .add(
                    egui::TextEdit::singleline(value)
                        .desired_width(ui.available_width().min(180.0))
                        .font(egui::FontId::proportional(FONT_BODY))
                        .text_color(TEXT_VALUE),
                )
                .changed();
        });
    });
    changed
}

/// A label followed by an editable drag value (used by the inspector).
pub fn drag_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    speed: f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(FONT_BODY).color(TEXT_DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed |= ui
                .add(egui::DragValue::new(value).speed(speed).range(range).max_decimals(3))
                .changed();
        });
    });
    changed
}

/// A toggle chip (Break / Ex / Each / …).
pub fn toggle_chip(ui: &mut egui::Ui, label: &str, on: &mut bool) -> bool {
    ui.checkbox(on, label).changed()
}

/// A thin horizontal separator across the current layout width.
pub fn hsep(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, PANEL_SEP);
}
