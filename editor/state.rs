use std::collections::HashSet;

use crate::app::types::{ChartDoc, NoteType};

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
}
