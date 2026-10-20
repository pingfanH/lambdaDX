//! Shared document history: chart mutations and their exact Simai source travel together.

use super::{SimaiEditor, Source, editing::{EditHistory, Selection}};
use crate::{app::types::ChartDoc, state::{EditorState, HistoryEntry}};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Snapshot {
    text: String,
    applied: String,
    source: Option<Arc<Source>>,
    selection: Selection,
}

impl SimaiEditor {
    pub(crate) fn has_history_source(&self) -> bool { self.source.is_some() }

    pub(crate) fn history_boundary(&mut self) { self.history_typing_at = None; }

    pub(crate) fn history_snapshot(&self) -> Option<Snapshot> {
        self.source.as_ref()?;
        Some(self.snapshot_text(&self.text, self.input.selection))
    }

    fn snapshot_text(&self, text: &str, selection: Selection) -> Snapshot {
        Snapshot { text: text.into(), applied: self.applied.clone(), source: self.source.clone(), selection }
    }

    pub(crate) fn restore_history(&mut self, snapshot: Snapshot, chart: &ChartDoc) {
        self.text = snapshot.text;
        self.applied = snapshot.applied;
        self.source = snapshot.source;
        self.modified = self.text != self.applied;
        self.error = None;
        self.failed_refresh = None;
        self.revision += 1;
        self.pending = None;
        self.apply_at = self.modified.then(|| std::time::Instant::now() + std::time::Duration::from_millis(300));
        self.cursor = None;
        self.cursor_time = None;
        self.pending_cursor_seek = None;
        self.playback.invalidate();
        self.history_boundary();
        self.input.selection = snapshot.selection;
        // Compact insertion entries may predate the last text refresh. Only
        // rebase clean text; an unfinished draft must remain byte-for-byte intact.
        self.sync(chart);
        self.input.external_change(&self.text);
        self.restore_cursor = true;
        self.history_text = Some(self.text.clone());
    }
}

pub(crate) struct DocumentHistory<'a> {
    editor: &'a mut EditorState,
    chart: &'a mut ChartDoc,
}

impl EditHistory for DocumentHistory<'_> {
    fn boundary(&mut self) { self.editor.simai_editor.history_boundary(); }

    fn record(&mut self, before: &str, selection: Selection, after: &str, typing: bool, now: f64) {
        if before == after { return; }
        let join = typing && self.editor.simai_editor.history_typing_at
            .is_some_and(|at| now >= at && now - at < 0.7) && !self.editor.undo.is_empty();
        if !join {
            let snapshot = self.editor.simai_editor.snapshot_text(before, selection);
            self.editor.push_document_history(self.chart.clone().into(), snapshot);
        } else {
            self.editor.redo.clear();
            self.editor.dirty = true;
        }
        self.editor.simai_editor.history_typing_at = typing.then_some(now);
        self.editor.simai_editor.history_text = Some(after.into());
    }

    fn restore(&mut self, text: &mut String, selection: &mut Selection, redo: bool) {
        self.editor.simai_editor.text = text.clone();
        self.editor.simai_editor.modified = *text != self.editor.simai_editor.applied;
        self.editor.simai_editor.input.selection = *selection;
        if redo { self.editor.redo(self.chart); } else { self.editor.undo(self.chart); }
        *text = self.editor.simai_editor.text.clone();
        *selection = self.editor.simai_editor.input.selection;
    }
}

pub(crate) fn with_input<R>(
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    edit: impl FnOnce(&mut super::input::Input, &mut String, &super::Options, &mut DocumentHistory<'_>) -> R,
) -> R {
    let mut input = std::mem::take(&mut editor.simai_editor.input);
    let mut text = std::mem::take(&mut editor.simai_editor.text);
    let options = editor.simai_editor.options.clone();
    let result = edit(&mut input, &mut text, &options, &mut DocumentHistory { editor, chart });
    editor.simai_editor.input = input;
    editor.simai_editor.text = text;
    result
}

pub(super) fn record_application(editor: &mut EditorState, chart: &ChartDoc) {
    let simai = &editor.simai_editor;
    let snapshot = simai.snapshot_text(&simai.applied, simai.input.selection);
    editor.push_document_history(chart.clone().into(), snapshot);
}

impl EditorState {
    pub(crate) fn push_document_history(&mut self, change: HistoryEntry, simai: Snapshot) {
        self.placement_sequence.clear();
        self.push_history_entry(HistoryEntry::Document { change: Box::new(change), simai: Box::new(simai) });
    }
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;
    use crate::simai_editor::{self, PendingApply, parse_draft};
    use egui_macroquad::egui::Event;
    use std::sync::Mutex;

    fn fixture() -> (EditorState, ChartDoc) {
        let text = "(120){4}1,2,E";
        let mut chart = parse_draft(text, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        let mut editor = EditorState::default();
        editor.simai_editor.load(&chart, text.into());
        (editor, chart)
    }

    fn paste(editor: &mut EditorState, chart: &mut ChartDoc, value: &str) {
        with_input(editor, chart, |input, text, options, history| {
            input.selection = Selection { anchor: 0, head: text.chars().count() };
            input.process_with_history(&mut vec![Event::Paste(value.into())], text, options, false, 1.0, history);
        });
        editor.simai_editor.changed();
    }

    #[test]
    fn shared_history_orders_visual_and_text_edits_without_duplicate_apply_steps() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        let original_text = editor.simai_editor.text.clone();
        editor.push_history(&chart);
        chart.notes[0].lane = 4;
        editor.simai_editor.sync(&chart);
        let visual = chart.clone();
        let visual_text = editor.simai_editor.text.clone();
        paste(&mut editor, &mut chart, "(120){4}4,6,E");
        simai_editor::apply(&mut editor, &mut chart).unwrap();
        assert_eq!(editor.undo.len(), 2, "applying recorded text is not another step");
        let text_chart = chart.clone();
        let text = editor.simai_editor.text.clone();
        editor.undo(&mut chart);
        assert_eq!(chart, visual);
        assert_eq!(editor.simai_editor.text, visual_text);
        editor.undo(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.simai_editor.text, original_text);
        editor.redo(&mut chart);
        assert_eq!(chart, visual);
        assert_eq!(editor.simai_editor.text, visual_text);
        editor.redo(&mut chart);
        assert_eq!(chart, text_chart);
        assert_eq!(editor.simai_editor.text, text);
        assert!(!editor.simai_editor.modified());
    }

    #[test]
    fn shared_history_keeps_invalid_draft_across_visual_undo_redo() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        let original_text = editor.simai_editor.text.clone();
        let draft = "(120){4}1h[";
        paste(&mut editor, &mut chart, draft);
        assert!(simai_editor::apply(&mut editor, &mut chart).is_err());
        editor.push_history(&chart);
        chart.notes[0].lane = 7;
        let visual = chart.clone();
        editor.undo(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.simai_editor.text, draft);
        assert!(editor.simai_editor.modified());
        editor.redo(&mut chart);
        assert_eq!(chart, visual);
        assert_eq!(editor.simai_editor.text, draft);
        editor.undo(&mut chart);
        editor.undo(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.simai_editor.text, original_text);
        editor.redo(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.simai_editor.text, draft);
    }

    #[test]
    fn shared_history_restores_comment_only_edits() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        let original_text = editor.simai_editor.text.clone();
        let text = "(120){4}1,2,E || comment";
        paste(&mut editor, &mut chart, text);
        simai_editor::apply(&mut editor, &mut chart).unwrap();
        assert_eq!(chart, original);
        assert_eq!(editor.undo.len(), 1);
        editor.undo(&mut chart);
        assert_eq!(editor.simai_editor.text, original_text);
        editor.redo(&mut chart);
        assert_eq!(editor.simai_editor.text, text);
        assert_eq!(chart, original);
    }

    #[test]
    fn shared_history_cancels_pending_apply_and_new_edits_clear_redo() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        paste(&mut editor, &mut chart, "(120){4}4,2,E");
        let pending = Arc::new(Mutex::new(None));
        editor.simai_editor.pending = Some(PendingApply {
            revision: editor.simai_editor.revision, bpm: 120.0, level: 6, result: pending.clone(),
        });
        editor.undo(&mut chart);
        assert!(editor.simai_editor.pending.is_none());
        *pending.lock().unwrap() = Some(parse_draft("(120){4}4,2,E", 120.0, 6));
        simai_editor::tick(&mut editor, &mut chart, true);
        assert_eq!(chart, original);
        assert_eq!(editor.redo.len(), 1);
        paste(&mut editor, &mut chart, "(120){4}5,2,E");
        assert!(editor.redo.is_empty());
    }

    #[test]
    fn shared_history_live_apply_does_not_split_a_typing_group() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        let original_text = editor.simai_editor.text.clone();
        for (now, value) in [(1.0, "4"), (1.1, "5")] {
            with_input(&mut editor, &mut chart, |input, text, options, history| {
                let at = text.find('}').unwrap() + 1;
                input.selection = Selection { anchor: at, head: at + 1 };
                input.process_with_history(&mut vec![Event::Text(value.into())], text, options, false, now, history);
            });
            editor.simai_editor.changed();
            editor.simai_editor.pending = Some(PendingApply {
                revision: editor.simai_editor.revision, bpm: 120.0, level: 6,
                result: Arc::new(Mutex::new(Some(parse_draft(&editor.simai_editor.text, 120.0, 6)))),
            });
            simai_editor::tick(&mut editor, &mut chart, true);
            assert_eq!(chart.notes[0].lane.to_string(), value);
        }
        assert_eq!(editor.undo.len(), 1);
        editor.undo(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.simai_editor.text, original_text);
        editor.redo(&mut chart);
        assert_eq!(chart.notes[0].lane, 5);
        assert!(!editor.simai_editor.modified());
    }

    #[test]
    fn shared_history_rebases_stale_compact_insertion_snapshots() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        let original_text = editor.simai_editor.text.clone();
        let first = editor.place_note(&mut chart, 2.0, 3);
        editor.place_note(&mut chart, 3.0, 4);
        assert!(matches!(editor.undo.last().unwrap().chart_change(), HistoryEntry::RemoveNote(_)));
        editor.undo(&mut chart);
        assert_eq!(chart.notes.len(), 3);
        assert!(chart.notes.iter().any(|note| note.id == first));
        assert!(!editor.simai_editor.modified());
        assert!(editor.simai_editor.source.as_ref().unwrap().matches(&chart));
        editor.undo(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.simai_editor.text, original_text);
        editor.redo(&mut chart);
        editor.redo(&mut chart);
        assert_eq!(chart.notes.len(), 4);
        assert!(editor.simai_editor.source.as_ref().unwrap().matches(&chart));
    }

    #[test]
    fn shared_history_records_native_input() {
        let (mut editor, mut chart) = fixture();
        let original = editor.simai_editor.text.clone();
        with_input(&mut editor, &mut chart, |input, text, _, history| {
            text.push_str(" || input");
            input.observe_native_with_history(&original, input.selection, text, 1.0, history);
        });
        editor.simai_editor.changed();
        let edited = editor.simai_editor.text.clone();
        editor.undo(&mut chart);
        assert_eq!(editor.simai_editor.text, original);
        editor.redo(&mut chart);
        assert_eq!(editor.simai_editor.text, edited);
    }
}
