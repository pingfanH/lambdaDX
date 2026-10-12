//! Tool palette strip, docked above the timeline.
//!
//! The design places the palette as a floating column on the left; per the
//! brief it lives here as a horizontal strip so it does not consume a layout
//! column of its own.

use egui_macroquad::egui;

use crate::state::{EditorState, Tool};
use crate::theme::{BG_ACTIVE, BG_WIDGET, BORDER_LIGHT, FONT_SMALL, TEXT, TEXT_DIM};

/// Palettes's keyboard shortcut + its display glyph.
fn shortcut(tool: Tool) -> (egui::Key, &'static str) {
    match tool {
        Tool::Select => (egui::Key::V, "V"),
        Tool::Tap => (egui::Key::T, "T"),
        Tool::Hold => (egui::Key::H, "H"),
        Tool::Touch => (egui::Key::C, "C"),
        Tool::Slide => (egui::Key::S, "S"),
        Tool::Eraser => (egui::Key::E, "E"),
    }
}

/// Plain-key tool shortcuts (V/T/H/C/S/E). Skipped while a text field has focus
/// and whenever Ctrl/Cmd is held (those are the cmd shortcuts).
pub fn handle_shortcuts(ui: &egui::Ui, editor: &mut EditorState) {
    if ui.ctx().wants_keyboard_input() {
        return;
    }
    if ui.input(|i| i.modifiers.command) {
        return;
    }
    for tool in Tool::ALL {
        let (key, _) = shortcut(tool);
        if ui.input(|i| i.key_pressed(key)) {
            editor.tool = tool;
        }
    }
}

/// Draw the strip contents inside `rect` (already reserved by the layout).
pub fn draw(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(8.0, 7.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for tool in Tool::ALL {
            if tool_button(ui, tool, editor.tool == tool) {
                editor.tool = tool;
            }
        }
    });
}

/// A square tool button with a small vector icon and a caption underneath.
fn tool_button(ui: &mut egui::Ui, tool: Tool, active: bool) -> bool {
    let size = egui::vec2(52.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if active { BG_ACTIVE } else { BG_WIDGET };
    ui.painter().rect_filled(rect, 4.0, fill);
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0_f32, BORDER_LIGHT),
        egui::StrokeKind::Inside,
    );
    if response.hovered() && !active {
        ui.painter()
            .rect_filled(rect, 4.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 10));
    }

    let icon_center = rect.center() - egui::vec2(14.0, 0.0);
    draw_icon(ui.painter(), icon_center, tool, active);
    ui.painter().text(
        egui::pos2(rect.center().x + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        tool.label(),
        egui::FontId::proportional(FONT_SMALL),
        if active { crate::theme::TEXT_ON_ACTIVE } else { TEXT },
    );
    let (_, key) = shortcut(tool);
    response
        .on_hover_text(format!("{} ({key})", tool.label()))
        .clicked()
}

/// Vector icon per tool (kept simple: shape + colour convey the note type).
fn draw_icon(painter: &egui::Painter, center: egui::Pos2, tool: Tool, active: bool) {
    let dim = if active { egui::Color32::WHITE } else { TEXT_DIM };
    let stroke = egui::Stroke::new(1.6_f32, dim);
    match tool {
        Tool::Select => {
            painter.line_segment(
                [center + egui::vec2(-5.0, -5.0), center + egui::vec2(5.0, 5.0)],
                stroke,
            );
            painter.line_segment(
                [center + egui::vec2(-5.0, 5.0), center + egui::vec2(5.0, -5.0)],
                stroke,
            );
        }
        Tool::Tap => {
            painter.circle_stroke(center, 5.0, stroke);
        }
        Tool::Hold => {
            painter.rect_stroke(
                egui::Rect::from_center_size(center, egui::vec2(6.0, 12.0)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Tool::Touch => {
            painter.rect_stroke(
                egui::Rect::from_center_size(center, egui::vec2(11.0, 11.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Tool::Slide => {
            let pts: Vec<egui::Pos2> = (0..5)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                    egui::pos2(center.x + a.cos() * 6.0, center.y + a.sin() * 6.0)
                })
                .collect();
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Tool::Eraser => {
            painter.circle_stroke(center, 5.5, stroke);
            painter.line_segment(
                [center + egui::vec2(-4.0, 4.0), center + egui::vec2(4.0, -4.0)],
                stroke,
            );
        }
    }
}
