//! Chart-mutating edit operations (undo/redo, clipboard, paste, delete).
//!
//! Kept separate from [`super::state`] so the state module stays descriptive and
//! these behaviours can be read and tested as a unit.

use std::collections::HashSet;

use crate::app::types::{ChartDoc, Note};

use super::state::EditorState;

impl EditorState {
    /// Snapshot `chart` onto the undo stack before a mutation. Trims the stack to
    /// a bounded depth and drops any redo history.
    pub fn push_history(&mut self, chart: &ChartDoc) {
        self.undo.push(chart.clone());
        self.undo.truncate(64);
        self.redo.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self, chart: &mut ChartDoc) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(chart.clone());
            *chart = previous;
            self.dirty = true;
        }
    }

    pub fn redo(&mut self, chart: &mut ChartDoc) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(chart.clone());
            *chart = next;
            self.dirty = true;
        }
    }

    /// Copy the selected notes onto the clipboard.
    pub fn copy_selection(&mut self, chart: &ChartDoc) {
        self.clipboard = chart
            .notes
            .iter()
            .filter(|n| self.selected.contains(&n.id))
            .cloned()
            .collect();
    }

    /// Delete the selected notes as one undoable step.
    pub fn delete_selection(&mut self, chart: &mut ChartDoc) {
        if self.selected.is_empty() {
            return;
        }
        self.push_history(chart);
        chart.notes.retain(|n| !self.selected.contains(&n.id));
        self.selected.clear();
    }

    /// Paste the clipboard so its earliest note lands on `measure`; preserves
    /// relative timing/lanes and selects the pasted notes. Returns whether
    /// anything was pasted.
    pub fn paste_at(&mut self, chart: &mut ChartDoc, measure: f32) -> bool {
        if self.clipboard.is_empty() {
            return false;
        }
        self.push_history(chart);
        let base = self
            .clipboard
            .iter()
            .map(|n| n.time)
            .fold(f32::INFINITY, f32::min);
        let mut next_id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0);
        let mut selected = HashSet::new();
        let mut added = Vec::with_capacity(self.clipboard.len());
        for n in &self.clipboard {
            next_id += 1;
            let mut copy = n.clone();
            copy.id = next_id;
            copy.time = measure + (n.time - base);
            selected.insert(copy.id);
            added.push(copy);
        }
        chart.notes.extend(added);
        self.selected = selected;
        true
    }

    /// Apply `f` to every selected note as one undoable step.
    pub fn edit_selected(&mut self, chart: &mut ChartDoc, f: impl Fn(&mut Note)) {
        self.push_history(chart);
        for n in chart.notes.iter_mut() {
            if self.selected.contains(&n.id) {
                f(n);
            }
        }
    }

    /// Append a freshly placed note of the current tool's type and select it.
    pub fn place_note(&mut self, chart: &mut ChartDoc, time: f32, lane: u8) -> u64 {
        self.push_history(chart);
        let id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        chart.notes.push(Note {
            id,
            time,
            lane: lane.min(8),
            note_type: self.selected_type(),
            hold_duration: if self.tool == super::state::Tool::Hold {
                1.0
            } else {
                0.0
            },
            is_touch: self.tool == super::state::Tool::Touch,
            ..Default::default()
        });
        self.select_only(id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::EditorState;
    use crate::app::types::{ChartDoc, Note};

    fn note(id: u64, time: f32, lane: u8) -> Note {
        Note {
            id,
            time,
            lane,
            ..Default::default()
        }
    }

    fn chart(notes: Vec<Note>) -> ChartDoc {
        ChartDoc {
            version: String::new(),
            title: String::new(),
            artist: String::new(),
            simai_level: 0,
            bpm: 120.0,
            bpms: Vec::new(),
            audio_offset: 0.0,
            notes,
            templates: Vec::new(),
            template_instances: Vec::new(),
        }
    }

    #[test]
    fn copy_paste_preserves_relative_timing_and_lanes() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 4.0, 1), note(2, 4.5, 3), note(3, 9.0, 5)]);
        editor.selected.insert(1);
        editor.selected.insert(2);
        editor.copy_selection(&chart);
        assert_eq!(editor.clipboard.len(), 2);

        assert!(editor.paste_at(&mut chart, 16.0));
        let pasted: Vec<&Note> = chart
            .notes
            .iter()
            .filter(|n| editor.selected.contains(&n.id))
            .collect();
        assert_eq!(pasted.len(), 2);
        let mut times: Vec<f32> = pasted.iter().map(|n| n.time).collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(times, vec![16.0, 16.5]);
        assert!(pasted.iter().any(|n| n.lane == 1));
        assert!(pasted.iter().any(|n| n.lane == 3));
    }

    #[test]
    fn undo_redo_round_trips_a_paste() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 4.0, 1)]);
        editor.selected.insert(1);
        editor.copy_selection(&chart);
        editor.paste_at(&mut chart, 16.0);
        assert_eq!(chart.notes.len(), 2);
        editor.undo(&mut chart);
        assert_eq!(chart.notes.len(), 1);
        editor.redo(&mut chart);
        assert_eq!(chart.notes.len(), 2);
    }

    #[test]
    fn delete_selection_removes_only_selected() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 1.0, 1), note(2, 2.0, 2)]);
        editor.selected.insert(2);
        editor.delete_selection(&mut chart);
        assert_eq!(chart.notes.len(), 1);
        assert_eq!(chart.notes[0].id, 1);
    }
}
