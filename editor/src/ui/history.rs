use crate::{
    app::types::ChartDoc,
    state::{EditorState, HistoryEntry},
    theme::TEXT_DIM,
};
use egui_macroquad::egui;

fn summary(entry: &HistoryEntry) -> String {
    match entry {
        HistoryEntry::RemoveNote(id) => format!("Note #{id}"),
        HistoryEntry::InsertNote { note, .. } => format!("Note #{}", note.id),
        HistoryEntry::Snapshot(chart) => format!(
            "{} notes / {} autoplay groups",
            chart.notes.len(),
            chart.autoplay_overrides.len()
        ),
    }
}

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc) {
    let current = editor.undo.len();
    let mut target = current;
    ui.label(
        egui::RichText::new(format!("{} / {}", current, current + editor.redo.len()))
            .color(TEXT_DIM),
    );
    if ui
        .selectable_label(current == 0, "最早可恢复状态")
        .clicked()
    {
        target = 0;
    }
    for (index, entry) in editor.undo.iter().enumerate() {
        if ui
            .selectable_label(
                current == index + 1,
                format!("{:02}  {}", index + 1, summary(entry)),
            )
            .clicked()
        {
            target = index + 1;
        }
    }
    for (index, entry) in editor.redo.iter().rev().enumerate() {
        let step = current + index + 1;
        if ui
            .selectable_label(
                false,
                egui::RichText::new(format!("{step:02}  {}", summary(entry))).color(TEXT_DIM),
            )
            .clicked()
        {
            target = step;
        }
    }
    if target < current {
        for _ in target..current {
            editor.undo(chart);
        }
    } else {
        for _ in current..target {
            editor.redo(chart);
        }
    }
}
