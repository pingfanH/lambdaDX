//! Editor document/tool state.
//!
//! This module holds the *data* an editing session owns: the active tool, the
//! selection, the view transform, the undo/redo stacks and the small amount of
//! pending-IO request state that the UI raises for the runtime to apply.
//!
//! Chart-mutating operations (undo, clipboard, …) live in [`super::edit_ops`]
//! so this file stays a plain description of the state.

use std::collections::HashSet;

use crate::app::types::{ChartDoc, Note, NoteType};

/// The active editing tool, selected from the palette above the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Tap,
    Hold,
    Touch,
    Slide,
    Eraser,
}

impl Tool {
    /// Every tool in palette order.
    pub const ALL: [Tool; 6] = [
        Tool::Select,
        Tool::Tap,
        Tool::Hold,
        Tool::Touch,
        Tool::Slide,
        Tool::Eraser,
    ];

    /// Short label shown in the palette tooltip / button.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Tap => "Tap",
            Tool::Hold => "Hold",
            Tool::Touch => "Touch",
            Tool::Slide => "Slide",
            Tool::Eraser => "Eraser",
        }
    }
}

/// Overall editor arrangement.
///
/// * `Vertical` — the design layout and the default: a vertical timeline (time
///   flows upward toward the judgment line at the bottom) with the pad/inspector
///   panel on the right.
/// * `Horizontal` — the legacy ported layout (time flows to the right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorLayout {
    Vertical,
    Horizontal,
}

impl EditorLayout {
    pub fn toggled(self) -> Self {
        match self {
            EditorLayout::Vertical => EditorLayout::Horizontal,
            EditorLayout::Horizontal => EditorLayout::Vertical,
        }
    }
}

/// Where a note was relative to the cursor when a drag began, so dragging keeps
/// that offset instead of snapping the note's lane/time onto the cursor (which
/// made notes jump a row on grab).
#[derive(Debug, Clone, Copy)]
pub struct DragGrab {
    pub note_time: f32,
    pub note_lane: u8,
    pub grab_measure: f32,
    pub grab_lane: i32,
}

/// Where the pointer is, resolved to timeline coordinates (for the read-out).
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    pub measure: f32,
    pub time: f32,
    pub lane: u8,
}

/// All mutable state for one editing session.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub tool: Tool,
    pub layout: EditorLayout,
    /// Notes selected by id.
    pub selected: HashSet<u64>,
    /// Note under the pointer (drawn with a hover ring).
    pub hovered: Option<u64>,
    /// Left edge of the visible time window, in seconds.
    pub view_start: f32,
    /// Horizontal/vertical zoom: pixels per second.
    pub pixels_per_second: f32,
    /// Snap grid as subdivisions per measure (e.g. 16 = 16th notes).
    pub snap_division: u32,
    /// Whether placement/drag snaps to the grid (top-bar `Grid` toggle).
    pub snap_enabled: bool,
    pub dirty: bool,
    pub undo: Vec<ChartDoc>,
    pub redo: Vec<ChartDoc>,
    /// Note currently being dragged, if any.
    pub dragging: Option<u64>,
    /// Whether the in-progress drag already pushed an undo snapshot.
    pub drag_history_pushed: bool,
    /// Grab offset captured when a note drag starts (see [`DragGrab`]).
    pub drag_grab: Option<DragGrab>,
    /// Start corner (egui points) of an in-progress box (marquee) selection.
    pub marquee_start: Option<(f32, f32)>,
    /// Notes copied with Ctrl/Cmd+C, pasted with Ctrl/Cmd+V.
    pub clipboard: Vec<Note>,
    /// Last save/IO message shown in the toolbar.
    pub status: Option<String>,
    /// Editable chart path (folder or `maidata.txt`).
    pub chart_dir: String,
    /// Available Simai `&inote_N` difficulty slots for the current chart.
    pub level_slots: Vec<u32>,
    /// Index into [`Self::level_slots`].
    pub level_index: usize,
    /// Set by the UI to ask the runtime to (re)load [`Self::chart_dir`].
    pub reload_request: bool,
    /// Set by the UI to ask the runtime to open a native file picker.
    pub open_dialog: bool,
    /// Set by the UI to ask the runtime to export Simai (unimplemented → status).
    pub export_request: bool,
    /// Set while dragging the playhead/overview; the runtime applies it to the
    /// player's clock (`timeline_view_time` / `mode_song_offset`).
    pub pending_seek: Option<f32>,
    /// Whether the current drag is scrubbing the red playhead (not moving a note).
    pub scrubbing: bool,
    /// Pointer position on the timeline this frame (`None` when outside it).
    pub cursor: Option<CursorInfo>,
    /// Preview mode: 0 = autoplay without judgment, 1 = manual with judgment,
    /// 2 = autoplay with judgment. `mode_dirty` asks the runtime to apply it.
    pub autoplay_mode: u8,
    pub mode_dirty: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            layout: EditorLayout::Vertical,
            selected: HashSet::new(),
            hovered: None,
            view_start: 0.0,
            pixels_per_second: 120.0,
            snap_division: 16,
            snap_enabled: true,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            dragging: None,
            drag_history_pushed: false,
            drag_grab: None,
            marquee_start: None,
            clipboard: Vec::new(),
            status: None,
            chart_dir: String::new(),
            level_slots: Vec::new(),
            level_index: 0,
            reload_request: false,
            open_dialog: false,
            export_request: false,
            pending_seek: None,
            scrubbing: false,
            cursor: None,
            autoplay_mode: 1,
            mode_dirty: true,
        }
    }
}

impl EditorState {
    /// Note type produced by the currently selected placement tool.
    pub fn selected_type(&self) -> NoteType {
        match self.tool {
            Tool::Hold => NoteType::Hold,
            Tool::Touch => NoteType::Touch,
            Tool::Slide => NoteType::Slide,
            _ => NoteType::Tap,
        }
    }

    /// Clear the selection and select exactly `id`.
    pub fn select_only(&mut self, id: u64) {
        self.selected.clear();
        self.selected.insert(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_defaults_to_vertical_and_toggles() {
        let editor = EditorState::default();
        assert_eq!(editor.layout, EditorLayout::Vertical);
        assert_eq!(editor.layout.toggled(), EditorLayout::Horizontal);
        assert_eq!(editor.layout.toggled().toggled(), EditorLayout::Vertical);
    }
}
