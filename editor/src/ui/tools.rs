//! Compact floating note palette over the chart canvas.

use egui_macroquad::egui;

use crate::state::{EditorState, Tool};
use crate::theme::{ACCENT_BLUE, floating_surface, SEPARATOR, TEXT};
use lucide_icons::Icon;

pub const PANEL_W: f32 = 32.0;
const BUTTON: f32 = 24.0;
const INSET: f32 = 4.0;
const STEP: f32 = 26.0;
/// Extra space a group separator takes (design ≈5px block around the 1px line).
const SEP_EXTRA: f32 = 3.0;
/// Design offsets of the panel from the timeline's top-left corner.
pub const PANEL_X: f32 = 8.0;
pub const PANEL_Y: f32 = 38.0;

/// Tool indices after which a separator is drawn (0-based): after the 4th and
/// 6th buttons, giving the design's 4 / 2 / 2 grouping.
const SEPARATORS: [usize; 2] = [3, 5];

/// Height of the floating panel for `n` buttons.
pub fn panel_height(n: usize) -> f32 {
    let gaps = n.saturating_sub(1) as f32;
    INSET * 2.0 + n as f32 * BUTTON + gaps * (STEP - BUTTON) + SEPARATORS.len() as f32 * SEP_EXTRA
}

/// Hidden palettes reserve neither space nor a pointer exclusion area.
pub fn panel_rect(timeline: egui::Rect, hidden: bool) -> egui::Rect {
    if hidden {
        egui::Rect::NOTHING
    } else {
        egui::Rect::from_min_size(
            timeline.min + egui::vec2(PANEL_X, PANEL_Y),
            egui::vec2(PANEL_W, panel_height(Tool::ALL.len())),
        )
    }
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
/// any modifier is held (modified keys belong to other commands).
pub fn handle_shortcuts(ui: &egui::Ui, editor: &mut EditorState) {
    if ui.ctx().wants_keyboard_input() {
        return;
    }
    if ui.input(|i| i.modifiers.any()) {
        return;
    }
    for tool in Tool::ALL {
        let (key, _) = shortcut(tool);
        if ui.input(|i| i.key_pressed(key)) {
            editor.tool = tool;
            editor.star_placement = None;
            editor.hold_placement = None;
        }
    }
}

/// Draw the floating palette into `rect` (computed by the layout from the
/// design offsets). Returns `true` if the pointer is over the panel, so the
/// caller can suppress timeline interaction under it.
pub fn draw(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    textures: &[Option<egui::TextureHandle>],
    interactive: bool,
) -> bool {
    if editor.hide_chart_tools {
        return false;
    }
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
        painter.rect_filled(rect, radius, floating_surface());
    }

    let mut y = rect.top() + INSET;
    let hovered = ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)));
    for (i, tool) in Tool::ALL.iter().enumerate() {
        let btn = egui::Rect::from_min_size(
            egui::pos2(rect.left() + INSET, y),
            egui::vec2(BUTTON, BUTTON),
        );
        if tool_button(ui, btn, *tool, editor.tool == *tool, textures, interactive) {
            editor.tool = *tool;
            editor.star_placement = None;
            editor.hold_placement = None;
        }
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
fn tool_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    tool: Tool,
    active: bool,
    textures: &[Option<egui::TextureHandle>],
    interactive: bool,
) -> bool {
    let response = ui.interact(
        rect,
        ui.id().with(("tool", tool as u8)),
        if interactive { egui::Sense::click() } else { egui::Sense::hover() },
    );
    let radius = egui::CornerRadius::same(3);
    ui.painter().rect_filled(rect, radius, egui::Color32::TRANSPARENT);
    if active {
        ui.painter().vline(rect.left(), rect.y_range().shrink(4.0), egui::Stroke::new(2.0_f32, ACCENT_BLUE));
    }
    if response.hovered() && !active {
        ui.painter().rect_filled(
            rect,
            radius,
            egui::Color32::from_white_alpha(14),
        );
    }
    let texture = match tool {
        Tool::Tap => textures.first().and_then(Option::as_ref),
        Tool::Hold => textures.get(1).and_then(Option::as_ref),
        Tool::Slide => textures.get(2).and_then(Option::as_ref),
        Tool::Touch => textures.get(3).and_then(Option::as_ref),
        Tool::TouchHold => textures.get(4).and_then(Option::as_ref),
        Tool::Break => textures.get(5).and_then(Option::as_ref),
        _ => None,
    };
    if let Some(texture) = texture {
        let natural = texture.size_vec2();
        let factor = (19.0 / natural.x).min(19.0 / natural.y);
        let image_rect = egui::Rect::from_center_size(rect.center(), natural * factor);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        draw_icon(ui.painter(), rect.center(), tool, active);
    }
    let (_, key) = shortcut(tool);
    interactive && response
        .on_hover_text(format!("{} ({key})", tool.label()))
        .clicked()
}

/// Vector icon per tool (14 px footprint, matching the design glyphs).
fn draw_icon(painter: &egui::Painter, c: egui::Pos2, tool: Tool, active: bool) {
    let symbol = match tool {
        Tool::Select => Icon::MousePointer2,
        Tool::Tap => Icon::CircleDot,
        Tool::Hold => Icon::RectangleVertical,
        Tool::Touch => Icon::Scan,
        Tool::TouchHold => Icon::ScanLine,
        Tool::Slide => Icon::Star,
        Tool::Break => Icon::Zap,
        Tool::Eraser => Icon::Eraser,
    };
    painter.text(c, egui::Align2::CENTER_CENTER, char::from(symbol),
        egui::FontId::new(14.0, egui::FontFamily::Name("lucide".into())),
        if active { ACCENT_BLUE } else { TEXT });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_chart_tools_do_not_paint_or_capture_and_keep_shortcuts() {
        let ctx = egui::Context::default();
        let mut editor = EditorState { hide_chart_tools: true, ..Default::default() };
        let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let panel = panel_rect(canvas, false);
        assert_eq!(panel_rect(canvas, true), egui::Rect::NOTHING);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(canvas),
                events: vec![
                    egui::Event::PointerMoved(panel.center()),
                    egui::Event::Key {
                        key: egui::Key::T, physical_key: None, pressed: true,
                        repeat: false, modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(!draw(ui, panel, &mut editor, &[], true));
                    handle_shortcuts(ui, &mut editor);
                });
            },
        );
        assert_eq!(output.shapes.len(), 1, "only the panel background should be painted");
        assert_eq!(editor.tool, Tool::Tap);
    }

    #[test]
    fn clicking_each_palette_button_selects_tool_and_preserves_selection() {
        let ctx = egui::Context::default();
        super::super::style::apply(&ctx);
        let mut editor = EditorState::default();
        editor.selected.insert(42);
        let panel = egui::Rect::from_min_size(
            egui::pos2(20.0, 40.0),
            egui::vec2(PANEL_W, panel_height(Tool::ALL.len())),
        );
        let mut time = 0.0;
        let mut y = panel.top() + INSET + BUTTON * 0.5;
        for (i, tool) in Tool::ALL.iter().enumerate() {
            let position = egui::pos2(panel.center().x, y);
            for pressed in [None, Some(true), Some(false)] {
                time += 0.1;
                let mut events = vec![egui::Event::PointerMoved(position)];
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            ui.interact(
                                ui.max_rect(),
                                ui.id().with("timeline"),
                                egui::Sense::click_and_drag(),
                            );
                            assert!(draw(ui, panel, &mut editor, &[None, None, None], true));
                        });
                    },
                );
            }
            assert_eq!(editor.tool, *tool);
            assert_eq!(editor.selected, [42].into_iter().collect());
            y += STEP;
            if SEPARATORS.contains(&i) {
                y += SEP_EXTRA;
            }
        }
    }

    #[test]
    fn panel_height_matches_the_design() {
        assert!((PANEL_W - 32.0).abs() < 1e-6);
        assert!(
            (panel_height(Tool::ALL.len()) - 220.0).abs() < 0.01,
            "height = {}",
            panel_height(Tool::ALL.len())
        );
    }
}
