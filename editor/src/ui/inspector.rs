//! Right-hand inspector: note properties and chart info.

use egui_macroquad::egui;

use crate::app::types::{ChartDoc, NoteType};
use crate::state::EditorState;
use crate::theme::{FONT_SMALL, TEXT_DIM};

use super::widgets::{drag_row, hsep, section_header, toggle_chip, value_row};

/// Draw both inspector sections. `open` toggles each section's collapsed state.
pub fn draw(ui: &mut egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc, open: &mut [bool; 2]) {
    note_properties(ui, editor, chart, &mut open[0]);
    ui.add_space(4.0);
    hsep(ui);
    ui.add_space(4.0);
    chart_info(ui, chart, &mut open[1]);
}

fn note_properties(ui: &mut egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc, open: &mut bool) {
    if section_header(ui, "Note Properties", *open) {
        *open = !*open;
    }
    if !*open {
        return;
    }

    // The rows are always laid out (with "-" placeholders when nothing is
    // selected) so the panel keeps the design's shape.
    let selected = editor
        .selected
        .iter()
        .next()
        .copied()
        .and_then(|id| chart.notes.iter().position(|n| n.id == id));
    let Some(idx) = selected else {
        let hint = if editor.selected.is_empty() {
            "no selection"
        } else {
            "selection not found"
        };
        ui.label(egui::RichText::new(hint).size(FONT_SMALL).color(TEXT_DIM));
        value_row(ui, "Type", "—");
        value_row(ui, "Time", "—");
        value_row(ui, "Lane", "—");
        value_row(ui, "Duration", "—");
        return;
    };

    let id = chart.notes[idx].id;
    ui.label(
        egui::RichText::new(format!("{} selected · id {id}", editor.selected.len()))
            .size(FONT_SMALL)
            .color(TEXT_DIM),
    );

    // Read the note out first so a single edit batches into one undo snapshot.
    let note = &chart.notes[idx];
    let note_type = note.note_type;
    let mut time = note.time;
    let mut lane = note.lane as i32;
    let mut dur = note.hold_duration;
    let mut break_on = note.is_break;
    let mut ex_on = note.is_ex;
    let mut each_on = note.is_each;
    let mut star_on = note.is_star;
    let mut tapless_on = note.is_tapless;

    let mut changed = false;
    value_row(ui, "Type", &format!("{note_type:?}"));
    changed |= drag_row(ui, "Time", &mut time, 0.01, 0.0..=1.0e6);
    let mut lane_f = lane as f32;
    if drag_row(ui, "Lane", &mut lane_f, 1.0, 1.0..=8.0) {
        lane = lane_f.round() as i32;
        changed = true;
    }
    if matches!(note_type, NoteType::Hold | NoteType::TouchHold | NoteType::Slide) {
        changed |= drag_row(ui, "Duration", &mut dur, 0.01, 0.0..=1.0e6);
    }

    ui.horizontal_wrapped(|ui| {
        changed |= toggle_chip(ui, "Break", &mut break_on);
        changed |= toggle_chip(ui, "Ex", &mut ex_on);
        changed |= toggle_chip(ui, "Each", &mut each_on);
        if matches!(note_type, NoteType::Slide) {
            changed |= toggle_chip(ui, "Star", &mut star_on);
            changed |= toggle_chip(ui, "Tapless", &mut tapless_on);
        }
    });

    if changed {
        editor.push_history(chart);
        let n = &mut chart.notes[idx];
        n.time = time.max(0.0);
        n.lane = lane.clamp(1, 8) as u8;
        n.hold_duration = dur.max(0.0);
        n.is_break = break_on;
        n.is_ex = ex_on;
        n.is_each = each_on;
        n.is_star = star_on;
        n.is_tapless = tapless_on;
    }
}

fn chart_info(ui: &mut egui::Ui, chart: &ChartDoc, open: &mut bool) {
    if section_header(ui, "Chart Info", *open) {
        *open = !*open;
    }
    if !*open {
        return;
    }
    let title = if chart.title.is_empty() { "—" } else { &chart.title };
    let artist = if chart.artist.is_empty() { "—" } else { &chart.artist };
    value_row(ui, "Title", title);
    value_row(ui, "Artist", artist);
    value_row(ui, "BPM", &format!("{:.1}", chart.bpm));
    value_row(ui, "Offset", &format!("{:.3}s", chart.audio_offset));
}
