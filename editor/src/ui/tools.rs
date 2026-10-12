//! Tool palette, drawn as the design's floating panel over the timeline.
//!
//! The design places a rounded, shadowed panel (`#252529`, 38×268) at the
//! timeline's top-left with eight 30×30 icon buttons (`#303035`, active
//! `#477DAD`) split into 4 / 2 / 2 groups by hairline separators.

use egui_macroquad::egui;

use crate::state::{EditorState, Tool};
use crate::theme::{
    BG_ACTIVE, BG_TOOLS_PANEL, BG_WIDGET, SEPARATOR, TEXT, TEXT_ON_ACTIVE,
};

/// Panel width (design 38).
pub const PANEL_W: f32 = 38.0;
/// Button edge (design 30).
const BUTTON: f32 = 30.0;
/// Inset from the panel edge to the buttons (design 4).
const INSET: f32 = 4.0;
/// Vertical step between two buttons (design 32 = 30 + 2 gap).
const STEP: f32 = 32.0;
/// Extra space a group separator takes (design ≈5px block around the 1px line).
const SEP_EXTRA: f32 = 3.0;
/// Design offsets of the panel from the timeline's top-left corner.
pub const PANEL_X: f32 = 12.0;
pub const PANEL_Y: f32 = 72.0;

/// Tool indices after which a separator is drawn (0-based): after the 4th and
/// 6th buttons, giving the design's 4 / 2 / 2 grouping.
const SEPARATORS: [usize; 2] = [3, 5];

/// Height of the floating panel for `n` buttons.
pub fn panel_height(n: usize) -> f32 {
    let gaps = n.saturating_sub(1) as f32;
    INSET * 2.0 + n as f32 * BUTTON + gaps * (STEP - BUTTON) + SEPARATORS.len() as f32 * SEP_EXTRA
}

/// Palettes's keyboard shortcut + its display glyph.
fn shortcut(tool: Tool) -> (egui::Key, &'static str) {
    match tool {
        Tool::Select => (egui::Key::V, "V"),
        Tool::Tap => (egui::Key::T, "T"),
        Tool::Hold => (egui::Key::H, "H"),
        Tool::Touch => (egui::Key::C, "C"),
        Tool::TouchHold => (egui::Key::G, "G"),
        Tool::Slide => (egui::Key::S, "S"),
        Tool::Break => (egui::Key::K, "K"),
        Tool::Eraser => (egui::Key::E, "E"),
    }
}

/// Plain-key tool shortcuts. Skipped while a text field has focus and whenever
/// Ctrl/Cmd is held (those are the cmd shortcuts).
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

/// Draw the floating palette into `rect` (computed by the layout from the
/// design offsets). Returns `true` if the pointer is over the panel, so the
/// caller can suppress timeline interaction under it.
pub fn draw(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) -> bool {
    let radius = egui::CornerRadius::same(4);
    {
        let painter = ui.painter();
        // Drop shadow (design: dy 3, blur 4, 40 % black).
        let shadow = egui::epaint::Shadow {
            offset: [0, 3],
            blur: 4,
            spread: 0,
            color: egui::Color32::from_black_alpha(102),
        };
        painter.add(shadow.as_shape(rect, radius));
        painter.rect_filled(rect, radius, BG_TOOLS_PANEL);
    }

    let mut y = rect.top() + INSET;
    let mut hovered = false;
    for (i, tool) in Tool::ALL.iter().enumerate() {
        let btn = egui::Rect::from_min_size(
            egui::pos2(rect.left() + INSET, y),
            egui::vec2(BUTTON, BUTTON),
        );
        hovered |= tool_button(ui, btn, *tool, editor.tool == *tool);
        y += STEP;
        if SEPARATORS.contains(&i) && i + 1 < Tool::ALL.len() {
            y += SEP_EXTRA;
            let mid = y - SEP_EXTRA * 0.5;
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left() + 2.0, mid),
                    egui::pos2(rect.right() - 2.0, mid),
                ],
                egui::Stroke::new(1.0_f32, SEPARATOR),
            );
        }
    }
    hovered
}

/// A square tool button with a vector icon. Returns whether it was clicked.
fn tool_button(ui: &mut egui::Ui, rect: egui::Rect, tool: Tool, active: bool) -> bool {
    let response = ui.interact(rect, ui.id().with(("tool", tool as u8)), egui::Sense::click());
    let radius = egui::CornerRadius::same(3);
    let fill = if active { BG_ACTIVE } else { BG_WIDGET };
    ui.painter().rect_filled(rect, radius, fill);
    if response.hovered() && !active {
        ui.painter().rect_filled(
            rect,
            radius,
            egui::Color32::from_rgba_premultiplied(255, 255, 255, 14),
        );
    }
    draw_icon(ui.painter(), rect.center(), tool, active);
    let (_, key) = shortcut(tool);
    response
        .on_hover_text(format!("{} ({key})", tool.label()))
        .clicked()
}

/// Vector icon per tool (14 px footprint, matching the design glyphs).
fn draw_icon(painter: &egui::Painter, c: egui::Pos2, tool: Tool, active: bool) {
    let dim = if active { TEXT_ON_ACTIVE } else { TEXT };
    let stroke = egui::Stroke::new(1.8_f32, dim);
    match tool {
        Tool::Select => {
            // Cursor arrow.
            let pts = vec![
                c + egui::vec2(-4.5, -6.0),
                c + egui::vec2(-4.5, 5.5),
                c + egui::vec2(-0.5, 1.5),
                c + egui::vec2(2.0, 6.0),
                c + egui::vec2(4.0, 5.0),
                c + egui::vec2(1.5, 0.7),
                c + egui::vec2(5.0, 0.2),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Tool::Tap => {
            painter.circle_stroke(c, 5.0, stroke);
            painter.circle_filled(c, 1.6, dim);
        }
        Tool::Hold => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(6.0, 12.0)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Tool::Touch => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(12.0, 12.0)),
                3.5,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Tool::TouchHold => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(12.0, 12.0)),
                3.5,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [c + egui::vec2(0.0, -3.5), c + egui::vec2(0.0, 3.5)],
                egui::Stroke::new(1.6_f32, dim),
            );
        }
        Tool::Slide => {
            let pts: Vec<egui::Pos2> = (0..5)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                    egui::pos2(c.x + a.cos() * 6.0, c.y + a.sin() * 6.0)
                })
                .collect();
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Tool::Break => {
            // Break note marker: a filled diamond.
            let pts = vec![
                c + egui::vec2(0.0, -6.0),
                c + egui::vec2(6.0, 0.0),
                c + egui::vec2(0.0, 6.0),
                c + egui::vec2(-6.0, 0.0),
            ];
            painter.add(egui::Shape::convex_polygon(pts, dim, egui::Stroke::NONE));
        }
        Tool::Eraser => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(12.0, 8.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [c + egui::vec2(-4.0, 5.0), c + egui::vec2(4.0, -5.0)],
                stroke,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_height_matches_the_design() {
        // Design: 8 buttons, 4 / 2 / 2 groups, panel is 38×268.
        assert!((PANEL_W - 38.0).abs() < 1e-6);
        assert!(
            (panel_height(Tool::ALL.len()) - 268.0).abs() < 0.01,
            "height = {}",
            panel_height(Tool::ALL.len())
        );
    }
}
