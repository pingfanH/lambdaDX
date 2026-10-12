//! Chart-mutating edit operations (undo/redo, clipboard, paste, delete).
//!
//! Kept separate from [`super::state`] so the state module stays descriptive and
//! these behaviours can be read and tested as a unit.

use std::collections::HashSet;

use crate::app::types::{
    zone::PadZone, validate_slide_shape, ChartDoc, Note, NoteType, SlidePoint, SlideSegment, SlideShape,
};

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

    /// Nudge every selected note by `delta_measure` measures and `delta_lane`
    /// lanes as one undoable step. Time is clamped to measure ≥ 1 and lanes to
    /// 1..=8, so a nudge at a boundary is a no-op for that axis.
    pub fn nudge_selection(&mut self, chart: &mut ChartDoc, delta_measure: f32, delta_lane: i32) {
        if self.selected.is_empty() || (delta_measure == 0.0 && delta_lane == 0) {
            return;
        }
        self.push_history(chart);
        for n in chart.notes.iter_mut() {
            if !self.selected.contains(&n.id) {
                continue;
            }
            if delta_measure != 0.0 {
                n.time = (n.time + delta_measure).max(1.0);
            }
            if delta_lane != 0 {
                n.lane = (n.lane as i32 + delta_lane).clamp(1, 8) as u8;
            }
        }
    }

    /// Append a freshly placed note of the current tool's type and select it.
    pub fn place_note(&mut self, chart: &mut ChartDoc, time: f32, lane: u8) -> u64 {
        use super::state::Tool;
        self.push_history(chart);
        let id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let hold = matches!(self.tool, Tool::Hold | Tool::TouchHold);
        chart.notes.push(Note {
            id,
            time,
            lane: lane.min(8),
            note_type: self.selected_type(),
            hold_duration: if hold { 1.0 } else { 0.0 },
            is_touch: matches!(self.tool, Tool::Touch | Tool::TouchHold),
            is_break: self.tool == Tool::Break,
            ..Default::default()
        });
        self.select_only(id);
        id
    }

    /// The id of the sole selected note, if exactly one is selected.
    pub fn single_selected(&self) -> Option<u64> {
        if self.selected.len() == 1 {
            self.selected.iter().next().copied()
        } else {
            None
        }
    }

    /// `(note index, sub-slide index)` of the slide being edited: the note in
    /// [`EditorState::editing_slide`], paired with
    /// [`EditorState::editing_slide_idx`] clamped into range.
    fn editing_target(&self, chart: &ChartDoc) -> Option<(usize, usize)> {
        let id = self.editing_slide?;
        let idx = chart
            .notes
            .iter()
            .position(|n| n.id == id && n.note_type == NoteType::Slide)?;
        let sub = self
            .editing_slide_idx
            .min(chart.notes[idx].slide.len().saturating_sub(1));
        Some((idx, sub))
    }

    /// Append a waypoint `zone` to the editing slide's last segment, creating
    /// the slide/segment as needed (a freshly placed slide has neither). Returns
    /// whether anything changed. One undoable step.
    ///
    /// Mirrors the original editor: the segment is kept free-form (its shape is
    /// re-inferred from the whole waypoint list) so every waypoint stays on the
    /// visible path instead of collapsing to a start→end arc.
    pub fn append_slide_waypoint(&mut self, chart: &mut ChartDoc, zone: PadZone) -> bool {
        let Some((idx, sub)) = self.editing_target(chart) else {
            return false;
        };
        let start_lane = chart.notes[idx].lane;
        // Ignore a repeat of the current tail (matches the original's dedup).
        let is_dup = chart.notes[idx]
            .slide
            .get(sub)
            .and_then(|sl| sl.segments.last())
            .and_then(|seg| seg.points.last())
            .map(|p| p.zone == zone)
            .unwrap_or(false);
        if is_dup {
            return false;
        }
        self.push_history(chart);
        let sl = slide_mut(&mut chart.notes[idx], sub);
        let seg = ensure_last_segment(sl);
        seg.points.push(SlidePoint {
            zone,
            beat_offset: 0.0,
        });
        seg.shape = infer_segment_shape(start_lane, &seg.points);
        true
    }

    /// Remove the last waypoint of the editing slide's last segment (Backspace),
    /// dropping the segment once it becomes empty. Returns whether anything
    /// changed. One undoable step.
    pub fn pop_slide_waypoint(&mut self, chart: &mut ChartDoc) -> bool {
        let Some((idx, sub)) = self.editing_target(chart) else {
            return false;
        };
        let has_point = chart.notes[idx]
            .slide
            .get(sub)
            .and_then(|sl| sl.segments.last())
            .map(|seg| !seg.points.is_empty())
            .unwrap_or(false);
        if !has_point {
            return false;
        }
        self.push_history(chart);
        if let Some(sl) = chart.notes[idx].slide.get_mut(sub) {
            if let Some(seg) = sl.segments.last_mut() {
                seg.points.pop();
                if seg.points.is_empty() {
                    sl.segments.pop();
                }
            }
        }
        true
    }

    /// Append `shape` (ending at `end_lane`, 1..=8) as a new segment on the
    /// editing sub-slide, chaining it after any existing arcs (mirroring the
    /// original editor). Rejected when invalid for the slide's start lane. One
    /// undoable step.
    pub fn apply_slide_shape(
        &mut self,
        chart: &mut ChartDoc,
        shape: SlideShape,
        end_lane: u8,
    ) -> bool {
        let Some((idx, sub)) = self.editing_target(chart) else {
            return false;
        };
        let start_lane = chart.notes[idx].lane;
        if end_lane < 1 || end_lane > 8 || validate_slide_shape(shape, start_lane, end_lane).is_err() {
            return false;
        }
        self.push_history(chart);
        let seg = SlideSegment {
            points: shape_waypoints(shape, end_lane),
            shape,
        };
        let note = &mut chart.notes[idx];
        if note.slide.is_empty() {
            let mut sl = default_slide();
            sl.segments.push(seg);
            note.slide.push(sl);
        } else {
            let sub = sub.min(note.slide.len() - 1);
            note.slide[sub].segments.push(seg);
        }
        true
    }
}

/// Infer a segment's canonical shape from its waypoint list, mirroring the
/// original editor's `match_slide_shape`: a 2-point path is a `Line`, a 3-point
/// folded path is a `VShape`, and anything else stays free-form (`Line`). `Line`
/// and `VShape` both render as a polyline through every waypoint.
fn infer_segment_shape(start_lane: u8, points: &[SlidePoint]) -> SlideShape {
    let mut visited = vec![start_lane];
    for sp in points {
        let z = sp.zone.to_id();
        if visited.last() != Some(&z) {
            visited.push(z);
        }
    }
    match visited.as_slice() {
        [a, b] if a != b => SlideShape::Line,
        [a, m, b] if a != m && m != b && a != b => SlideShape::VShape,
        _ => SlideShape::Line,
    }
}

/// Waypoint zones for `shape` ending at `end_lane`, excluding the start (mirrors
/// the Simai parser: `v` passes through the centre, everything else is a direct
/// endpoint that the renderer arcs between).
fn shape_waypoints(shape: SlideShape, end_lane: u8) -> Vec<SlidePoint> {
    let sp = |z: u8| SlidePoint {
        zone: PadZone::from(z),
        beat_offset: 0.0,
    };
    match shape {
        SlideShape::VShape => vec![sp(17), sp(end_lane)],
        _ => vec![sp(end_lane)],
    }
}

/// A default (empty, 1-measure) sub-slide.
fn default_slide() -> crate::app::types::Slide {
    crate::app::types::Slide {
        segments: Vec::new(),
        slide_duration: 1.0,
        slide_start_delay: 0.0,
        slide_is_break: false,
        runtime_parts: 1,
    }
}

/// The `sub`-th sub-slide of a note, pushing a default one when the note has
/// none. `sub` is clamped into range.
fn slide_mut(note: &mut Note, sub: usize) -> &mut crate::app::types::Slide {
    if note.slide.is_empty() {
        note.slide.push(default_slide());
    }
    let i = sub.min(note.slide.len() - 1);
    &mut note.slide[i]
}

/// The last segment of a slide, pushing a default (line) one when absent.
fn ensure_last_segment(sl: &mut crate::app::types::Slide) -> &mut SlideSegment {
    if sl.segments.is_empty() {
        sl.segments.push(SlideSegment {
            points: Vec::new(),
            shape: SlideShape::Line,
        });
        sl.runtime_parts = sl.runtime_parts.max(1);
    }
    sl.segments.last_mut().unwrap()
}

#[cfg(test)]
mod tests {
    use super::super::state::EditorState;
    use crate::app::types::zone::PadZone;
    use crate::app::types::{ChartDoc, Note, NoteType, SlideShape};

    fn note(id: u64, time: f32, lane: u8) -> Note {
        Note {
            id,
            time,
            lane,
            ..Default::default()
        }
    }

    fn slide_note(id: u64, time: f32, lane: u8) -> Note {
        Note {
            note_type: NoteType::Slide,
            ..note(id, time, lane)
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

    #[test]
    fn nudge_moves_selected_by_grid_and_clamps() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 4.0, 1), note(2, 4.5, 8)]);
        editor.selected.insert(1);
        editor.selected.insert(2);
        editor.nudge_selection(&mut chart, 0.25, 1);
        assert!((chart.notes[0].time - 4.25).abs() < 1e-4);
        assert_eq!(chart.notes[0].lane, 2);
        assert_eq!(chart.notes[1].lane, 8, "lane clamps at 8");
        // A single undo restores both notes.
        editor.undo(&mut chart);
        assert!((chart.notes[0].time - 4.0).abs() < 1e-4);
        assert_eq!(chart.notes[0].lane, 1);
    }

    #[test]
    fn nudge_is_noop_without_selection_or_delta() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 4.0, 1)]);
        editor.nudge_selection(&mut chart, 1.0, 0);
        assert!(!editor.dirty && editor.undo.is_empty());
        editor.selected.insert(1);
        editor.nudge_selection(&mut chart, 0.0, 0);
        assert!(!editor.dirty && editor.undo.is_empty());
    }

    #[test]
    fn append_slide_waypoint_creates_slide_and_segment() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.editing_slide = Some(1);
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(4u8)));
        let n = &chart.notes[0];
        assert_eq!(n.slide.len(), 1);
        assert_eq!(n.slide[0].segments.len(), 1);
        assert_eq!(n.slide[0].segments[0].points.len(), 1);
        assert_eq!(n.slide[0].segments[0].points[0].zone, PadZone::from(4u8));
        // One undo removes the whole appended structure.
        editor.undo(&mut chart);
        assert!(chart.notes[0].slide.is_empty());
    }

    #[test]
    fn append_dedups_and_infers_a_free_form_shape() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.editing_slide = Some(1);
        // A single waypoint is a plain Line.
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(3u8)));
        assert_eq!(chart.notes[0].slide[0].segments[0].shape, SlideShape::Line);
        // A repeated tail is ignored and adds no undo step.
        let undo_before = editor.undo.len();
        assert!(!editor.append_slide_waypoint(&mut chart, PadZone::from(3u8)));
        assert_eq!(editor.undo.len(), undo_before);
        assert_eq!(chart.notes[0].slide[0].segments[0].points.len(), 1);
        // A second distinct waypoint folds into a V (both render as a polyline).
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(5u8)));
        let seg = &chart.notes[0].slide[0].segments[0];
        assert_eq!(seg.points.len(), 2);
        assert_eq!(seg.shape, SlideShape::VShape);
    }

    #[test]
    fn apply_slide_shape_sets_waypoints_and_rejects_invalid() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.editing_slide = Some(1);
        assert!(editor.apply_slide_shape(&mut chart, SlideShape::Line, 5));
        assert_eq!(chart.notes[0].slide[0].segments[0].shape, SlideShape::Line);
        assert_eq!(chart.notes[0].slide[0].segments[0].points.len(), 1);
        // `S` must end opposite the start (lane 1 → 5); lane 3 is invalid.
        assert!(!editor.apply_slide_shape(&mut chart, SlideShape::S, 3));
        assert_eq!(
            chart.notes[0].slide[0].segments.len(),
            1,
            "a rejected shape adds nothing"
        );
        // `v` routes through the centre zone (17) and chains as a new segment.
        assert!(editor.apply_slide_shape(&mut chart, SlideShape::VShape, 3));
        let segs = &chart.notes[0].slide[0].segments;
        assert_eq!(segs.len(), 2, "the shape is chained as a new segment");
        assert_eq!(segs[1].shape, SlideShape::VShape);
        assert_eq!(segs[1].points.len(), 2);
        assert_eq!(segs[1].points[0].zone, PadZone::from(17u8));
    }

    #[test]
    fn slide_ops_require_an_editing_slide_target() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 4.0, 1), slide_note(2, 8.0, 2)]);
        // No target: nothing to edit.
        assert!(!editor.append_slide_waypoint(&mut chart, PadZone::from(4u8)));
        // The target must be a slide (a tap id is rejected).
        editor.editing_slide = Some(1);
        assert!(!editor.append_slide_waypoint(&mut chart, PadZone::from(4u8)));
        // A non-existent id is rejected too.
        editor.editing_slide = Some(99);
        assert!(!editor.apply_slide_shape(&mut chart, SlideShape::Line, 4));
    }

    #[test]
    fn pop_slide_waypoint_undoes_last_and_drops_empty_segment() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.editing_slide = Some(1);
        editor.append_slide_waypoint(&mut chart, PadZone::from(4u8));
        editor.append_slide_waypoint(&mut chart, PadZone::from(5u8));
        assert!(editor.pop_slide_waypoint(&mut chart));
        assert_eq!(chart.notes[0].slide[0].segments[0].points.len(), 1);
        assert!(editor.pop_slide_waypoint(&mut chart));
        assert!(
            chart.notes[0].slide[0].segments.is_empty(),
            "the emptied segment is dropped"
        );
        assert!(!editor.pop_slide_waypoint(&mut chart), "nothing left to pop");
    }

    #[test]
    fn slide_ops_target_the_editing_sub_slide() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.editing_slide = Some(1);
        editor.append_slide_waypoint(&mut chart, PadZone::from(2u8));
        // Add a second sub-slide and target it.
        chart.notes[0].slide.push(crate::app::types::Slide {
            segments: vec![crate::app::types::SlideSegment {
                points: Vec::new(),
                shape: SlideShape::Line,
            }],
            slide_duration: 1.0,
            slide_start_delay: 0.0,
            slide_is_break: false,
            runtime_parts: 1,
        });
        editor.editing_slide_idx = 1;
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(6u8)));
        assert_eq!(
            chart.notes[0].slide[0].segments[0].points.len(),
            1,
            "the first sub-slide is untouched"
        );
        assert_eq!(
            chart.notes[0].slide[1].segments[0].points[0].zone,
            PadZone::from(6u8)
        );
    }
}
