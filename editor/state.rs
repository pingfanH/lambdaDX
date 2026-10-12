use std::collections::HashSet;

use crate::app::types::{ChartDoc, Note, NoteType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Tap,
    Hold,
    Touch,
    Slide,
    Eraser,
}

/// Overall editor arrangement.
///
/// * `Horizontal` — the ported layout: pad/inspector sidebar on the left, a
///   horizontal timeline (time flows to the right) in the centre.
/// * `Vertical` — the original `macroquad_sim` layout: a vertical timeline
///   (time flows downward toward the judgment line) on the left, pad/inspector
///   info panel on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorLayout {
    Horizontal,
    Vertical,
}

impl EditorLayout {
    pub fn toggled(self) -> Self {
        match self {
            EditorLayout::Horizontal => EditorLayout::Vertical,
            EditorLayout::Vertical => EditorLayout::Horizontal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub tool: Tool,
    pub layout: EditorLayout,
    pub selected: HashSet<u64>,
    pub hovered: Option<u64>,
    pub view_start: f32,
    pub pixels_per_second: f32,
    pub snap_division: u32,
    pub dirty: bool,
    pub undo: Vec<ChartDoc>,
    pub redo: Vec<ChartDoc>,
    pub dragging: Option<u64>,
    pub drag_history_pushed: bool,
    /// Start corner (egui points) of an in-progress box (marquee) selection.
    pub marquee_start: Option<(f32, f32)>,
    /// Notes copied with Ctrl/Cmd+C, pasted with Ctrl/Cmd+V.
    pub clipboard: Vec<Note>,
    /// Last save/IO message shown in the header.
    pub status: Option<String>,
    /// Set while dragging the playhead/overview; `editor_main` applies it to the
    /// player's time (`timeline_view_time` / `mode_song_offset`).
    pub pending_seek: Option<f32>,
    /// Whether the current drag is scrubbing the red playhead (not moving a note).
    pub scrubbing: bool,
    /// Preview mode: 0 = autoplay without judgment, 1 = manual with judgment,
    /// 2 = autoplay with judgment. `mode_dirty` asks `editor_main` to apply it.
    pub autoplay_mode: u8,
    pub mode_dirty: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            layout: EditorLayout::Horizontal,
            selected: HashSet::new(),
            hovered: None,
            view_start: 0.0,
            pixels_per_second: 120.0,
            snap_division: 16,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            dragging: None,
            drag_history_pushed: false,
            marquee_start: None,
            clipboard: Vec::new(),
            status: None,
            pending_seek: None,
            scrubbing: false,
            autoplay_mode: 1,
            mode_dirty: true,
        }
    }
}

impl EditorState {
    pub fn selected_type(&self) -> NoteType {
        match self.tool {
            Tool::Hold => NoteType::Hold,
            Tool::Touch => NoteType::Touch,
            Tool::Slide => NoteType::Slide,
            _ => NoteType::Tap,
        }
    }

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

    pub fn copy_selection(&mut self, chart: &ChartDoc) {
        self.clipboard = chart
            .notes
            .iter()
            .filter(|n| self.selected.contains(&n.id))
            .cloned()
            .collect();
    }

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
            let mut c = n.clone();
            c.id = next_id;
            c.time = measure + (n.time - base);
            selected.insert(c.id);
            added.push(c);
        }
        chart.notes.extend(added);
        self.selected = selected;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_defaults_to_horizontal_and_toggles() {
        let editor = EditorState::default();
        assert_eq!(editor.layout, EditorLayout::Horizontal);
        assert_eq!(editor.layout.toggled(), EditorLayout::Vertical);
        assert_eq!(editor.layout.toggled().toggled(), EditorLayout::Horizontal);
    }

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
        // Pasted notes: earliest on measure 16, the other +0.5, same lanes.
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
