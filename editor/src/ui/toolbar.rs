//! Top application toolbar.
//!
//! Mirrors the design's header group — brand, file actions, view toggles — and
//! the port's own extras (difficulty switcher, layout toggle, preview mode).
//! Native menu-bar integration is intentionally out of scope: everything lives
//! in this in-window strip.

use egui_macroquad::egui;

use crate::app::types::ChartDoc;
use crate::state::{EditorLayout, EditorState};
use crate::io::save_chart;
use crate::runtime::preview_mode_label;
use crate::theme::{ACCENT_BLUE, ACCENT_GREEN, ACCENT_RED, ACCENT_YELLOW, FONT_BODY, FONT_HEADER, TEXT, TEXT_DIM};

use super::widgets::{toolbar_button, toolbar_button_enabled, toolbar_separator};

/// Draw the toolbar. Returns `true` if audio was just turned off so the caller
/// can stop any playing track (avoids a second `&mut app` borrow here).
pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    audio_enabled: &mut bool,
) -> bool {
    let mut audio_turned_off = false;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(
            egui::RichText::new("Lambda DX")
                .size(FONT_HEADER)
                .color(TEXT)
                .strong(),
        );
        // Unsaved-changes marker (design has no explicit *; a dot reads cleanly).
        ui.label(
            egui::RichText::new(if editor.dirty { "●" } else { "·" })
                .size(FONT_HEADER)
                .color(if editor.dirty { ACCENT_RED } else { TEXT_DIM }),
        );
        toolbar_separator(ui);

        let can_undo = !editor.undo.is_empty();
        let can_redo = !editor.redo.is_empty();
        if toolbar_button_enabled(ui, "Undo", can_undo, "Undo  (⌘Z)") {
            editor.undo(chart);
        }
        if toolbar_button_enabled(ui, "Redo", can_redo, "Redo  (⇧⌘Z)") {
            editor.redo(chart);
        }
        toolbar_separator(ui);

        if toolbar_button(ui, "Save", false, "Write output/editor_chart.json  (⌘S)") {
            save_chart(editor, chart);
        }
        if toolbar_button(ui, "Load", false, "Reload the chart folder") {
            editor.reload_request = true;
        }
        if toolbar_button(ui, "Open", false, "Pick a chart folder…") {
            editor.open_dialog = true;
        }
        if toolbar_button(ui, "Export", false, "Export Simai (not implemented yet)") {
            editor.export_request = true;
        }

        toolbar_separator(ui);
        if toolbar_button(ui, "Grid", editor.snap_enabled, "Snap placement / drag to the grid") {
            editor.snap_enabled = !editor.snap_enabled;
        }
        if toolbar_button(
            ui,
            "Wheel ↕",
            editor.scroll_invert,
            "Invert the scroll / wheel direction (use when the OS \"natural scrolling\" feels backwards)",
        ) {
            editor.scroll_invert = !editor.scroll_invert;
        }
        if toolbar_button(ui, "Audio", *audio_enabled, "Toggle audio playback") {
            *audio_enabled = !*audio_enabled;
            audio_turned_off = !*audio_enabled;
        }

        if !editor.level_slots.is_empty() {
            toolbar_separator(ui);
            draw_difficulty(ui, editor);
        }

        toolbar_separator(ui);
        if toolbar_button(ui, "?", editor.show_help, "Keyboard shortcuts  (?)") {
            editor.show_help = !editor.show_help;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(status) = &editor.status {
                ui.label(
                    egui::RichText::new(status)
                        .size(FONT_BODY)
                        .color(ACCENT_GREEN),
                );
                ui.separator();
            }
            let mode = egui::RichText::new(preview_mode_label(editor.autoplay_mode))
                .size(FONT_BODY)
                .color(ACCENT_YELLOW);
            if ui.add(egui::Button::new(mode).frame(false)).clicked() {
                editor.autoplay_mode = (editor.autoplay_mode + 1) % 3;
                editor.mode_dirty = true;
            }
            ui.separator();
            let (label, color) = match editor.layout {
                EditorLayout::Vertical => ("Layout: Vertical", ACCENT_BLUE),
                EditorLayout::Horizontal => ("Layout: Horizontal", ACCENT_GREEN),
            };
            if ui
                .add(egui::Button::new(egui::RichText::new(label).size(FONT_BODY).color(color)).frame(false))
                .clicked()
            {
                editor.layout = editor.layout.toggled();
            }
        });
    });
    audio_turned_off
}

/// `◀ slot N (i/n) ▶` difficulty stepper for the active chart.
fn draw_difficulty(ui: &mut egui::Ui, editor: &mut EditorState) {
    let n = editor.level_slots.len();
    let can_step = n > 1;
    let prev = ui.add_enabled(can_step, egui::Button::new("◀"));
    let next = ui.add_enabled(can_step, egui::Button::new("▶"));
    let label = match editor.level_slots.get(editor.level_index).copied() {
        Some(slot) if n > 0 => format!("Lv.{slot}  ({}/{n})", editor.level_index + 1),
        _ => "Lv.—".to_string(),
    };
    ui.label(egui::RichText::new(label).size(FONT_BODY).color(TEXT_DIM));
    if prev.clicked() {
        editor.level_index = (editor.level_index + n - 1) % n;
        editor.reload_request = true;
    }
    if next.clicked() {
        editor.level_index = (editor.level_index + 1) % n;
        editor.reload_request = true;
    }
}
