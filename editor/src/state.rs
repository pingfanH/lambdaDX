//! Editor document/tool state.
//!
//! This module holds the *data* an editing session owns: the active tool, the
//! selection, the view transform, the undo/redo stacks and the small amount of
//! pending-IO request state that the UI raises for the runtime to apply.
//!
//! Chart-mutating operations (undo, clipboard, …) live in [`super::edit_ops`]
//! so this file stays a plain description of the state.

use std::collections::HashSet;

use crate::app::types::{ChartDoc, Note, NoteType, SlideShape};

/// The active editing tool, selected from the palette beside the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Tap,
    Hold,
    Touch,
    TouchHold,
    Slide,
    Break,
    Eraser,
}

impl Tool {
    /// Every tool in palette order (8 buttons, matching the design).
    pub const ALL: [Tool; 8] = [
        Tool::Select,
        Tool::Tap,
        Tool::Hold,
        Tool::Touch,
        Tool::TouchHold,
        Tool::Slide,
        Tool::Break,
        Tool::Eraser,
    ];

    /// Short label shown in the palette tooltip / button.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Tap => "Tap",
            Tool::Hold => "Hold",
            Tool::Touch => "Touch",
            Tool::TouchHold => "TouchHold",
            Tool::Slide => "Slide",
            Tool::Break => "Break",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EditorLayout {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Workspace {
    #[default]
    Chart,
    Autoplay,
}

impl EditorLayout {
    pub fn toggled(self) -> Self {
        match self {
            EditorLayout::Vertical => EditorLayout::Horizontal,
            EditorLayout::Horizontal => EditorLayout::Vertical,
        }
    }
}

/// A file/exit action raised by the UI that must wait for the unsaved-changes
/// prompt when [`EditorState::dirty`] is set. When nothing is unsaved the action
/// runs immediately (see [`EditorState::defer_if_dirty`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAction {
    /// Reload the current chart folder.
    Reload,
    /// Open a native file/folder picker, then reload.
    Open,
    /// Close the editor.
    Quit,
}

/// Which timing handle of a Hold / Slide note an in-progress drag is moving.
///
/// * `Delay` — the slide's motion-start point (changes `slide_start_delay`);
/// * `Tail` — the note's end (changes `hold_duration` for a hold, the slide
///   span `slide_duration` for a slide).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragHandle {
    Delay,
    Tail,
}

/// Captured state for an in-progress timing-handle drag, so the duration is
/// computed from the grab offset rather than jumping on the first frame.
#[derive(Debug, Clone, Copy)]
pub struct HandleGrab {
    pub id: u64,
    pub kind: DragHandle,
    /// Target sub-slide; holds do not have one.
    pub slide_idx: Option<usize>,
    /// Pointer measure at press, preserving the grab offset.
    pub grab_measure: f32,
    /// Note head time (measures) at grab.
    pub note_time: f32,
    /// `slide_start_delay` at grab (0 for holds).
    pub start_delay: f32,
    /// `hold_duration` / `slide_duration` at grab.
    pub duration: f32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionTransformKind {
    Grab,
    Scale,
    Rotate,
}

#[derive(Debug, Clone)]
pub struct SelectionTransform {
    pub keyboard_driven: bool,
    pub kind: SelectionTransformKind,
    pub before: ChartDoc,
    pub was_dirty: bool,
    pub anchor: (f32, f32),
    pub wheel: f32,
    pub keyboard_time: f32,
    pub keyboard_lane: i32,
    pub duplicate_before: Option<ChartDoc>,
    pub operation: RepeatEdit,
}

#[derive(Debug, Clone, Copy)]
pub struct RepeatEdit {
    pub kind: SelectionTransformKind,
    pub duplicate: bool,
    pub time: f32,
    pub lanes: i32,
    pub factor: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct StarPlacement {
    pub note_id: Option<u64>,
    pub head: f32,
    pub lane: u8,
    pub delay_end: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct TimelinePress {
    pub note: Option<u64>,
    pub position: (f32, f32),
    pub measure: f32,
    pub lane: i32,
    pub on_playhead: bool,
}

/// Where the pointer is, resolved to timeline coordinates (for the read-out).
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    pub measure: f32,
    pub time: f32,
    pub lane: u8,
}

#[derive(Debug, Clone)]
pub enum HistoryEntry {
    Snapshot(Box<ChartDoc>),
    RemoveNote(u64),
    InsertNote { index: usize, note: Box<Note> },
}

impl From<ChartDoc> for HistoryEntry {
    fn from(chart: ChartDoc) -> Self {
        Self::Snapshot(Box::new(chart))
    }
}

/// Captured ruler gesture, separate from playback seeking.
#[derive(Debug, Clone)]
pub struct LoopDrag {
    pub owner: egui_macroquad::egui::Id,
    pub anchor: f32,
    pub previous: Option<(f32, f32)>,
    pub was_enabled: bool,
    pub cancelled: bool,
}

/// All mutable state for one editing session.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub workspace: Workspace,
    pub template_panel: bool,
    pub template_name: String,
    pub selected_template: Option<String>,
    pub selected_instance: Option<String>,
    pub template_session: Option<Box<crate::templates::Session>>,
    pub template_action: Option<crate::templates::Action>,
    pub autoplay_editor: crate::autoplay_editor::AutoplayEditor,
    pub simai_editor: crate::simai_editor::SimaiEditor,
    pub keyboard: crate::keyboard::KeyboardState,
    pub keyboard_seek_on_navigation: bool,
    pub follow_playhead: bool,
    /// Playhead offset from the view reference after pointer-anchored zoom.
    pub follow_view_offset: Option<f32>,
    pub loop_range: Option<(f32, f32)>,
    pub loop_enabled: bool,
    pub loop_drag: Option<LoopDrag>,
    pub pinned_note: Option<u64>,
    pub tool: Tool,
    pub touch_zone: Option<u8>,
    pub layout: EditorLayout,
    /// Notes selected by id.
    pub selected: HashSet<u64>,
    /// Note under the pointer (drawn with a hover ring).
    pub hovered: Option<u64>,
    /// Left edge of the visible time window, in seconds.
    pub view_start: f32,
    /// Middle-button rate scrolling: fixed press coordinate and whether it moved.
    pub timeline_pan: Option<(f32, bool)>,
    /// Horizontal/vertical zoom: pixels per second.
    pub pixels_per_second: f32,
    /// Snap grid as subdivisions per measure (e.g. 16 = 16th notes).
    pub snap_division: u32,
    /// Whether placement/drag snaps to the grid (top-bar snap checkbox).
    pub snap_enabled: bool,
    pub autoplay_snap_enabled: bool,
    pub autoplay_snap_division: u32,
    /// Invert the wheel scroll direction (top-bar `Wheel ↕` toggle), for users
    /// whose OS "natural scrolling" setting feels backwards in the editor.
    pub scroll_invert: bool,
    pub dirty: bool,
    pub auto_save: bool,
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    /// Note currently being dragged, if any.
    pub dragging: Option<u64>,
    /// Whether the in-progress drag already pushed an undo snapshot.
    pub drag_history_pushed: bool,
    /// Grab offset captured when a note drag starts (see [`DragGrab`]).
    pub drag_grab: Option<DragGrab>,
    pub pointer_press: Option<TimelinePress>,
    pub selection_transform: Option<SelectionTransform>,
    pub last_edit: Option<RepeatEdit>,
    pub placement_sequence: Vec<Note>,
    /// Consume the whole pointer gesture used to confirm/cancel a transform.
    pub transform_pointer_down: bool,
    pub star_placement: Option<StarPlacement>,
    pub hold_placement: Option<Note>,
    /// Rendered logical pixels converted to egui points for head hit boxes.
    pub glyph_to_points: f32,
    /// Timing-handle gesture, separate from moving the note head.
    pub handle_grab: Option<HandleGrab>,
    /// Originals of every note in the selection at drag start, as
    /// `(id, time, lane)`. When the grabbed note is part of a multi-selection the
    /// whole group moves together (matching the original editor); empty for a
    /// pure marquee drag.
    pub drag_multi_orig: Vec<(u64, f32, u8)>,
    /// Full original notes keep each slide's own path during group rotation.
    pub drag_orig_notes: Vec<Note>,
    /// Start corner (egui points) of an in-progress box (marquee) selection.
    pub marquee_start: Option<(f32, f32)>,
    pub marquee_base: HashSet<u64>,
    pub marquee_subtract: bool,
    /// Notes copied with Ctrl/Cmd+C, pasted with Ctrl/Cmd+V.
    pub clipboard: Vec<Note>,
    /// Last save/IO message shown in the toolbar.
    pub status: Option<String>,
    /// Editable chart path (folder or `maidata.txt`).
    pub chart_dir: String,
    /// Actual loaded file/slot, independent of a pending difficulty switch.
    pub source: Option<crate::io::ChartSource>,
    /// Available Simai `&inote_N` difficulty slots for the current chart.
    pub level_slots: Vec<u32>,
    /// Index into [`Self::level_slots`].
    pub level_index: usize,
    /// Set by the UI to ask the runtime to (re)load [`Self::chart_dir`].
    pub reload_request: bool,
    /// Set by the UI to ask the runtime to open a native file picker.
    pub open_dialog: bool,
    /// Set by the UI to ask the runtime to export Simai.
    pub export_request: bool,
    pub save_as_request: bool,
    /// A native picker is running in the background; keep drawing, but freeze edits.
    pub file_dialog_active: bool,
    /// Set while dragging the playhead/overview; the runtime applies it to the
    /// player's clock (`timeline_view_time` / `mode_song_offset`).
    pub pending_seek: Option<f32>,
    /// Whether the current drag is scrubbing the red playhead (not moving a note).
    pub scrubbing: bool,
    /// Pointer position on the timeline this frame (`None` when outside it).
    pub cursor: Option<CursorInfo>,
    /// Preview mode: 0 = autoplay without judgment, 1 = manual with judgment,
    /// 2 = simulated autoplay, 3 = direct-core autoplay, both with judgment.
    /// `mode_dirty` asks the runtime to apply it.
    pub autoplay_mode: u8,
    pub mode_dirty: bool,
    /// Whether the `?` shortcut-help overlay is shown.
    pub show_help: bool,
    /// Id of the slide ("star") note currently in track-edit mode, or `None`.
    /// Entered by middle/double-clicking a slide's trail or adding a trajectory
    /// via the star head / inspector; pad / lane clicks append waypoints, shape keys
    /// `Q/P/S/Z/V/W` (plus `⇧`, `,`, `.`, `;`, `-`) pick a shape that `1-8`
    /// completes, and `⌫` undoes the last waypoint. `Esc` exits.
    pub editing_slide: Option<u64>,
    /// Shape picked with a shape key, waiting for an end lane (`1-8`) to apply.
    pub pending_shape: Option<SlideShape>,
    /// Which sub-slide ("star" part) of the editing slide new waypoints and
    /// shapes target (index into `note.slide`).
    pub editing_slide_idx: usize,
    /// File/exit action deferred pending the unsaved-changes prompt, if any.
    pub pending_action: Option<PendingAction>,
    /// Set by the confirm dialog to tell the runtime to run `pending_action`
    /// (after the user chose Save or Discard).
    pub pending_confirmed: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            workspace: Workspace::Chart,
            template_panel: false,
            template_name: "Template".into(),
            selected_template: None,
            selected_instance: None,
            template_session: None,
            template_action: None,
            autoplay_editor: Default::default(),
            simai_editor: Default::default(),
            keyboard: crate::keyboard::KeyboardState::default(),
            keyboard_seek_on_navigation: false,
            follow_playhead: true,
            follow_view_offset: None,
            loop_range: None,
            loop_enabled: false,
            loop_drag: None,
            pinned_note: None,
            tool: Tool::Select,
            touch_zone: None,
            layout: EditorLayout::Vertical,
            selected: HashSet::new(),
            hovered: None,
            view_start: 0.0,
            timeline_pan: None,
            pixels_per_second: crate::timeline::geometry::DEFAULT_PPS,
            snap_division: 16,
            snap_enabled: true,
            autoplay_snap_enabled: true,
            autoplay_snap_division: 16,
            scroll_invert: false,
            dirty: false,
            auto_save: false,
            undo: Vec::new(),
            redo: Vec::new(),
            dragging: None,
            drag_history_pushed: false,
            drag_grab: None,
            pointer_press: None,
            selection_transform: None,
            last_edit: None,
            placement_sequence: Vec::new(),
            transform_pointer_down: false,
            star_placement: None,
            hold_placement: None,
            glyph_to_points: 1.0,
            handle_grab: None,
            drag_multi_orig: Vec::new(),
            drag_orig_notes: Vec::new(),
            marquee_start: None,
            marquee_base: HashSet::new(),
            marquee_subtract: false,
            clipboard: Vec::new(),
            status: None,
            chart_dir: String::new(),
            source: None,
            level_slots: Vec::new(),
            level_index: 0,
            reload_request: false,
            open_dialog: false,
            export_request: false,
            save_as_request: false,
            file_dialog_active: false,
            pending_seek: None,
            scrubbing: false,
            cursor: None,
            autoplay_mode: 1,
            mode_dirty: true,
            show_help: false,
            editing_slide: None,
            pending_shape: None,
            editing_slide_idx: 0,
            pending_action: None,
            pending_confirmed: false,
        }
    }
}

impl EditorState {
    pub fn cancel_file_action(&mut self) {
        self.pending_action = None;
        self.pending_confirmed = false;
        if let Some(level) = self.source.as_ref().and_then(|source| source.level) {
            if let Some(index) = self.level_slots.iter().position(|slot| *slot == level) {
                self.level_index = index;
            }
        }
    }

    /// Note type produced by the currently selected placement tool.
    pub fn selected_type(&self) -> NoteType {
        match self.tool {
            Tool::Hold => NoteType::Hold,
            Tool::Touch => NoteType::Touch,
            Tool::TouchHold => NoteType::TouchHold,
            Tool::Slide => NoteType::Slide,
            _ => NoteType::Tap,
        }
    }

    /// Clear the selection and select exactly `id`.
    pub fn select_only(&mut self, id: u64) {
        self.selected.clear();
        self.selected.insert(id);
    }

    /// Whether slide ("star") track-edit mode is active.
    pub fn is_slide_edit(&self) -> bool {
        self.editing_slide.is_some()
    }

    /// Defer a file/exit action when there are unsaved changes, so the runtime
    /// shows the confirm prompt instead of running it. Returns `true` when the
    /// action was deferred (the caller should NOT run it now).
    pub fn defer_if_dirty(&mut self, action: PendingAction) -> bool {
        if self.has_unsaved_changes() {
            self.pending_action = Some(action);
            true
        } else {
            false
        }
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty || self.simai_editor.modified()
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

    #[test]
    fn defer_if_dirty_only_defers_when_unsaved() {
        let mut editor = EditorState::default();
        // Clean: the action runs now, nothing is deferred.
        assert!(!editor.defer_if_dirty(PendingAction::Reload));
        assert_eq!(editor.pending_action, None);
        // Dirty: the action is held for the confirm prompt.
        editor.dirty = true;
        assert!(editor.defer_if_dirty(PendingAction::Quit));
        assert_eq!(editor.pending_action, Some(PendingAction::Quit));
    }
}
