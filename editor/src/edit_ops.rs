//! Chart-mutating edit operations (undo/redo, clipboard, paste, delete).
//!
//! Kept separate from [`super::state`] so the state module stays descriptive and
//! these behaviours can be read and tested as a unit.

use std::collections::HashSet;

use crate::app::types::{
    ChartDoc, Note, NoteType, SlidePoint, SlideSegment, SlideShape, validate_slide_shape,
    zone::PadZone,
};

use super::state::{EditorState, HistoryEntry};

impl HistoryEntry {
    fn apply(self, chart: &mut ChartDoc, simai: &mut crate::simai_editor::SimaiEditor) -> Self {
        match self {
            Self::Document { change, simai: previous } => {
                let current = simai.history_snapshot();
                let inverse = change.apply(chart, simai);
                simai.restore_history(*previous, chart);
                match current {
                    Some(current) => Self::Document { change: Box::new(inverse), simai: Box::new(current) },
                    None => inverse,
                }
            }
            Self::Snapshot(mut previous) => {
                std::mem::swap(chart, &mut previous);
                Self::Snapshot(previous)
            }
            Self::RemoveNote(id) => {
                let index = chart.notes.iter().position(|note| note.id == id)
                    .expect("inserted note must exist until its insertion is undone");
                Self::InsertNote { index, note: Box::new(chart.notes.remove(index)) }
            }
            Self::InsertNote { index, note } => {
                let id = note.id;
                chart.notes.insert(index, *note);
                Self::RemoveNote(id)
            }
        }
    }
}

impl EditorState {
    pub(crate) fn next_note_id(&self, chart: &ChartDoc) -> u64 {
        chart.notes.iter().map(|note| note.id)
            .chain(self.mairis.annotation.original_ids.iter().copied())
            .max().unwrap_or(0) + 1
    }

    /// Snapshot `chart` onto the undo stack before a mutation. Trims the stack to
    /// a bounded depth and drops any redo history.
    pub fn push_history(&mut self, chart: &ChartDoc) {
        self.placement_sequence.clear();
        if self.template_session.is_none() && !self.mairis.annotation.active && self.simai_editor.has_history_source() {
            self.simai_editor.sync(chart);
        }
        self.push_history_entry(chart.clone().into());
    }

    pub fn record_insertion(&mut self, id: u64) {
        self.push_history_entry(HistoryEntry::RemoveNote(id));
    }

    pub(crate) fn push_history_entry(&mut self, entry: HistoryEntry) {
        self.simai_editor.history_boundary();
        let entry = if self.template_session.is_none() && !self.mairis.annotation.active && !matches!(&entry, HistoryEntry::Document { .. }) {
            match self.simai_editor.history_snapshot() {
                Some(simai) => HistoryEntry::Document { change: Box::new(entry), simai: Box::new(simai) },
                None => entry,
            }
        } else { entry };
        self.undo.push(entry);
        if self.undo.len() > 64 {
            self.undo.drain(..self.undo.len() - 64);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self, chart: &mut ChartDoc) {
        if self.check_slide_placement_finished().is_err() {
            crate::keyboard::edit::cancel(self, chart);
            self.exit_slide_edit(chart);
            return;
        }
        self.simai_editor.history_boundary();
        if let Some(previous) = self.undo.pop() {
            if self.template_session.is_none() && !self.mairis.annotation.active { self.simai_editor.sync(chart); }
            self.placement_sequence.clear();
            self.last_edit = None;
            self.redo.push(previous.apply(chart, &mut self.simai_editor));
            crate::app::maichart::recompute_each(&mut chart.notes);
            crate::app::maichart::mark_double_stars(&mut chart.notes);
            self.reconcile_slide_target(chart);
            if self.template_session.is_none() && !self.mairis.annotation.active { self.simai_editor.sync(chart); }
            self.dirty = true;
        }
    }

    pub fn redo(&mut self, chart: &mut ChartDoc) {
        if self.check_slide_placement_finished().is_err() {
            crate::keyboard::edit::cancel(self, chart);
            self.exit_slide_edit(chart);
            return;
        }
        self.simai_editor.history_boundary();
        if let Some(next) = self.redo.pop() {
            if self.template_session.is_none() && !self.mairis.annotation.active { self.simai_editor.sync(chart); }
            self.placement_sequence.clear();
            self.last_edit = None;
            self.undo.push(next.apply(chart, &mut self.simai_editor));
            crate::app::maichart::recompute_each(&mut chart.notes);
            crate::app::maichart::mark_double_stars(&mut chart.notes);
            self.reconcile_slide_target(chart);
            if self.template_session.is_none() && !self.mairis.annotation.active { self.simai_editor.sync(chart); }
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
        self.delete_notes(chart, &self.selected.clone());
    }

    pub fn delete_note(&mut self, chart: &mut ChartDoc, id: u64) {
        self.delete_notes(chart, &HashSet::from([id]));
    }

    fn delete_notes(&mut self, chart: &mut ChartDoc, ids: &HashSet<u64>) {
        if !chart.notes.iter().any(|n| ids.contains(&n.id)) {
            return;
        }
        let mut sequence = std::mem::take(&mut self.placement_sequence);
        self.push_history(chart);
        chart.notes.retain(|n| !ids.contains(&n.id));
        self.selected.retain(|id| !ids.contains(id));
        let live_ids: HashSet<_> = chart.notes.iter().map(|n| n.id).collect();
        sequence.retain(|old| live_ids.contains(&old.id));
        self.placement_sequence = sequence;
        self.last_edit = None;
        if self.keyboard.candidate.is_some_and(|id| ids.contains(&id)) { self.keyboard.candidate = None; }
        if self.hovered.is_some_and(|id| ids.contains(&id)) { self.hovered = None; }
        self.reconcile_slide_target(chart);
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
        let mut next_id = self.next_note_id(chart) - 1;
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

    /// Duplicate in place without touching the clipboard or committing history.
    /// The modal transform owns the combined duplicate/move undo step.
    pub fn duplicate_selection(&mut self, chart: &mut ChartDoc) -> bool {
        let mut copies: Vec<_> = chart.notes.iter()
            .filter(|n| self.selected.contains(&n.id)).cloned().collect();
        if copies.is_empty() {
            return false;
        }
        let mut next_id = self.next_note_id(chart) - 1;
        for copy in &mut copies {
            next_id += 1;
            copy.id = next_id;
        }
        self.selected = copies.iter().map(|n| n.id).collect();
        chart.notes.extend(copies);
        self.editing_slide = None;
        self.pending_shape = None;
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
    /// lanes as one undoable step. Preserve group spacing at the chart start
    /// and rotate sensors/slide paths within their original rings.
    pub fn nudge_selection(&mut self, chart: &mut ChartDoc, delta_measure: f32, delta_lane: i32) {
        if self.selected.is_empty() || (delta_measure == 0.0 && delta_lane == 0) {
            return;
        }
        let earliest = chart
            .notes
            .iter()
            .filter(|n| self.selected.contains(&n.id))
            .map(|n| n.time)
            .fold(f32::INFINITY, f32::min);
        let delta_measure = delta_measure.max(1.0 - earliest);
        if !earliest.is_finite() || (delta_measure == 0.0 && delta_lane == 0) {
            return;
        }
        self.push_history(chart);
        for n in chart.notes.iter_mut() {
            if !self.selected.contains(&n.id) {
                continue;
            }
            crate::timeline::geometry::move_note(n, n.time + delta_measure, delta_lane);
        }
        self.last_edit = Some(super::state::RepeatEdit {
            kind: super::state::SelectionTransformKind::Grab,
            duplicate: false, time: delta_measure, lanes: delta_lane, factor: 1.0,
        });
    }

    /// Append a freshly placed note of the current tool's type and select it.
    pub fn place_note(&mut self, chart: &mut ChartDoc, time: f32, lane: u8) -> u64 {
        let id = self.next_note_id(chart);
        self.record_insertion(id);
        chart.notes.push(Note {
            id,
            ..crate::timeline::placement::note_for_tool(self, time, lane)
        });
        self.placement_sequence.push(chart.notes.last().unwrap().clone());
        self.last_edit = None;
        self.select_only(id);
        id
    }

    /// A pending hold is committed only after its tail is confirmed.
    pub fn place_hold_step(&mut self, chart: &mut ChartDoc, time: f32, lane: u8) {
        let Some(mut note) = self.hold_placement.take() else {
            self.hold_placement = Some(crate::timeline::placement::note_for_tool(self, time, lane));
            return;
        };
        note.hold_duration = (time - note.time).max(0.0);
        note.id = self.next_note_id(chart);
        self.record_insertion(note.id);
        self.select_only(note.id);
        self.placement_sequence.push(note.clone());
        self.last_edit = None;
        chart.notes.push(note);
    }

    /// Continue the translation between the latest two compatible placements.
    pub fn repeat_placement(&mut self, chart: &mut ChartDoc) {
        use crate::timeline::geometry::{move_note, snap};
        let [.., a, b] = self.placement_sequence.as_slice() else {
            self.status = Some("No placement pattern".into());
            return;
        };
        let tolerance =
            (a.time.abs().max(b.time.abs()) * f32::EPSILON * 4.0).max(1e-5);
        let normalized = |note: &Note| {
            let mut copy = note.clone();
            move_note(&mut copy, 1.0, 1 - note.lane as i32);
            copy.id = 0;
            copy.is_each = false;
            copy.is_each_head = false;
            copy.is_star = false;
            copy.hold_duration = 0.0;
            for slide in &mut copy.slide {
                slide.slide_duration = 0.0;
                slide.slide_start_delay = 0.0;
            }
            copy
        };
        let compatible = |a: &Note, b: &Note| {
            (a.hold_duration - b.hold_duration).abs() <= tolerance
                && a.slide.len() == b.slide.len()
                && a.slide.iter().zip(&b.slide).all(|(a, b)| {
                    (a.slide_duration - b.slide_duration).abs() <= tolerance
                        && (a.slide_start_delay - b.slide_start_delay).abs() <= tolerance
                })
                && normalized(a) == normalized(b)
        };
        let dt = b.time - a.time;
        let dl = (b.lane as i32 - a.lane as i32).rem_euclid(8);
        let valid = (dt.abs() > 1e-5 || dl != 0)
            && compatible(a, b)
            && [a, b].into_iter().all(|old| {
                chart.notes.iter().any(|n| {
                    n.id == old.id
                        && (n.time - old.time).abs() < 1e-5
                        && n.lane == old.lane
                        && compatible(n, old)
                })
            });
        let time = snap(b.time + dt, self.snap_enabled, self.snap_division);
        if !valid || !time.is_finite() || time < 1.0 {
            self.status = Some("No placement pattern".into());
            return;
        }
        let mut copy = b.clone();
        move_note(&mut copy, time, dl);
        copy.id = self.next_note_id(chart);
        copy.is_each = false;
        copy.is_each_head = false;
        self.record_insertion(copy.id);
        self.placement_sequence.push(copy.clone());
        self.select_only(copy.id);
        chart.notes.push(copy);
        self.status = Some("Repeated placement".into());
    }

    /// Three-click timing placement enters path editing. Without a default
    /// path the whole placement stays provisional until confirmation.
    pub fn place_star_step(&mut self, chart: &mut ChartDoc, time: f32, lane: u8) {
        use super::state::StarPlacement;
        match self.star_placement {
            None => {
                self.star_placement = Some(StarPlacement {
                    tool: self.tool,
                    note_id: None,
                    head: time,
                    lane: lane.clamp(1, 8),
                    delay_end: None,
                })
            }
            Some(mut pending) if pending.delay_end.is_none() => {
                if time > pending.head {
                    pending.delay_end = Some(time);
                    self.star_placement = Some(pending);
                }
            }
            Some(pending) => {
                let delay = pending.delay_end.unwrap();
                if time <= delay {
                    return;
                }
                let mut slide = default_slide();
                slide.slide_start_delay = delay - pending.head;
                slide.slide_duration = time - pending.head;
                self.place_slide_path(chart, pending.note_id, pending.head, pending.lane, slide);
                self.star_placement = None;
            }
        }
    }

    pub fn begin_same_head_placement(&mut self, chart: &ChartDoc, id: u64) -> bool {
        let Some(note) = chart
            .notes
            .iter()
            .find(|n| n.id == id && n.note_type == NoteType::Slide)
        else {
            return false;
        };
        self.star_placement = Some(super::state::StarPlacement {
            tool: self.tool,
            note_id: Some(id),
            head: note.time,
            lane: note.lane,
            delay_end: None,
        });
        self.editing_slide = None;
        self.pending_shape = None;
        self.select_only(id);
        true
    }

    /// The id of the sole selected note, if exactly one is selected.
    pub fn single_selected(&self) -> Option<u64> {
        if self.selected.len() == 1 {
            self.selected.iter().next().copied()
        } else {
            None
        }
    }

    /// Add an independent trajectory to a star and enter editing on it.
    pub fn add_sub_slide(&mut self, chart: &mut ChartDoc, id: u64) -> bool {
        self.exit_slide_edit(chart);
        let Some(idx) = chart
            .notes
            .iter()
            .position(|n| n.id == id && n.note_type == NoteType::Slide)
        else {
            return false;
        };
        let note = &chart.notes[idx];
        self.place_slide_path(chart, Some(id), note.time, note.lane, default_slide())
    }

    /// Delete the active trajectory while preserving the note head and other paths.
    pub fn remove_sub_slide(&mut self, chart: &mut ChartDoc) -> bool {
        if self.slide_draft.is_some() { self.exit_slide_edit(chart); return true; }
        let Some((idx, sub)) = self.editing_target(chart) else {
            return false;
        };
        if chart.notes[idx].slide.is_empty() {
            return false;
        }
        self.push_history(chart);
        chart.notes[idx].slide.remove(sub);
        if chart.notes[idx].slide.is_empty() {
            chart.notes[idx].note_type = NoteType::Tap;
            chart.notes[idx].is_star = false;
        }
        crate::app::maichart::mark_double_stars(&mut chart.notes);
        self.pending_shape = None;
        self.reconcile_slide_target(chart);
        true
    }

    fn reconcile_slide_target(&mut self, chart: &ChartDoc) {
        if let Some((_, sub)) = self.editing_target(chart) {
            self.editing_slide_idx = sub;
        } else {
            self.editing_slide = None;
            self.editing_slide_idx = 0;
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
    /// Free-form segments are inferred from their waypoints. Explicit shapes
    /// keep their geometry; further waypoints start a new connected segment.
    pub fn append_slide_waypoint(&mut self, chart: &mut ChartDoc, zone: PadZone) -> bool {
        let Some((note, sub)) = self.editing_slide_target(chart) else {
            return false;
        };
        // Ignore a repeat of the current tail (matches the original's dedup).
        let is_dup = note.slide.get(sub)
            .and_then(|slide| slide.segments.last())
            .and_then(|segment| segment.points.last())
            .is_some_and(|point| point.zone == zone);
        if is_dup {
            return false;
        }
        self.slide_path_history(chart);
        self.with_editing_slide_mut(chart, |note, sub| {
            let start_lane = note.lane;
            let sl = slide_mut(note, sub);
            let starts_new_segment = sl.segments.last().is_some_and(|segment| {
                !matches!(segment.shape, SlideShape::Line | SlideShape::VShape)
                    && !segment.points.is_empty()
            });
            if starts_new_segment {
                sl.segments.push(SlideSegment {
                    points: Vec::new(),
                    shape: SlideShape::Line,
                });
            }
            let segment_start = sl.segments.iter().rev().nth(1)
                .and_then(|segment| segment.points.last())
                .map(|point| point.zone.to_id())
                .unwrap_or(start_lane);
            let seg = ensure_last_segment(sl);
            seg.points.push(SlidePoint { zone, beat_offset: 0.0 });
            seg.shape = infer_segment_shape(segment_start, &seg.points);
            true
        })
    }

    /// Remove the last waypoint of the editing slide's last segment (Backspace),
    /// dropping the segment once it becomes empty. Returns whether anything
    /// changed. One undoable step.
    pub fn pop_slide_waypoint(&mut self, chart: &mut ChartDoc) -> bool {
        let Some((note, sub)) = self.editing_slide_target(chart) else {
            return false;
        };
        if !note.slide.get(sub).and_then(|slide| slide.segments.last())
            .is_some_and(|segment| !segment.points.is_empty())
        {
            return false;
        }
        self.slide_path_history(chart);
        self.with_editing_slide_mut(chart, |note, sub| {
            let start_lane = note.lane;
            let sl = note.slide.get_mut(sub).unwrap();
            let segment_start = sl
                .segments
                .iter()
                .rev()
                .nth(1)
                .and_then(|seg| seg.points.last())
                .map(|p| p.zone.to_id())
                .unwrap_or(start_lane);
            if let Some(seg) = sl.segments.last_mut() {
                seg.points.pop();
                if seg.points.is_empty() {
                    sl.segments.pop();
                } else if matches!(seg.shape, SlideShape::Line | SlideShape::VShape) {
                    seg.shape = infer_segment_shape(segment_start, &seg.points);
                }
            }
            true
        })
    }

    /// Append `shape` (ending at `end_lane`, 1..=8) as a new segment on the
    /// editing sub-slide, chaining it after any existing arcs (mirroring the
    /// original editor). Rejected when invalid for the segment's start lane. One
    /// undoable step.
    pub fn apply_slide_shape(
        &mut self,
        chart: &mut ChartDoc,
        shape: SlideShape,
        end_lane: u8,
    ) -> bool {
        let Some((note, sub)) = self.editing_slide_target(chart) else {
            return false;
        };
        let start_lane = note.slide.get(sub)
            .and_then(|slide| slide.segments.last())
            .and_then(|segment| segment.points.last())
            .map(|point| point.zone.to_id())
            .unwrap_or(note.lane);
        if end_lane < 1
            || end_lane > 8
            || validate_slide_shape(shape, start_lane, end_lane).is_err()
        {
            return false;
        }
        self.slide_path_history(chart);
        self.with_editing_slide_mut(chart, |note, sub| {
            slide_mut(note, sub).segments.push(SlideSegment {
                points: shape_waypoints(shape, end_lane),
                shape,
            });
            true
        })
    }
}

/// Infer a segment's canonical shape from its waypoint list, mirroring the
/// Only a fold through C is a canonical `v`. Other waypoint lists remain
/// free-form lines so the renderer cannot substitute a centre-V prefab.
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
        [a, 17, b] if a != b && *a != 17 && *b != 17 => SlideShape::VShape,
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

    #[test]
    fn placement_history_is_a_delta_and_redo_keeps_the_final_note() {
        use super::HistoryEntry;
        let mut chart = chart(Vec::new());
        let original = chart.notes.clone();
        let mut editor = EditorState::default();
        let id = editor.place_note(&mut chart, 4.0, 3);
        assert!(matches!(editor.undo.last(), Some(HistoryEntry::RemoveNote(saved)) if *saved == id));
        chart.notes.last_mut().unwrap().hold_duration = 2.0;
        let placed = chart.notes.last().unwrap().clone();
        editor.undo(&mut chart);
        assert_eq!(chart.notes.len(), original.len());
        editor.redo(&mut chart);
        assert_eq!(chart.notes.last(), Some(&placed));
    }

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
            autoplay_overrides: Vec::new(),
        }
    }

    #[test]
    fn hold_placement_confirms_tail_once_and_keeps_head_lane() {
        for tool in [crate::state::Tool::Hold, crate::state::Tool::TouchHold] {
            let mut editor = EditorState { tool, ..Default::default() };
            let mut chart = chart(Vec::new());
            editor.place_hold_step(&mut chart, 2.0, 3);
            assert!(chart.notes.is_empty());
            assert!(editor.undo.is_empty());
            assert_eq!(editor.hold_placement.as_ref().unwrap().hold_duration, 0.0);
            editor.place_hold_step(&mut chart, 2.75, 8);
            assert!(editor.hold_placement.is_none());
            assert_eq!(chart.notes.len(), 1);
            assert_eq!(chart.notes[0].time, 2.0);
            assert_eq!(chart.notes[0].lane, 3);
            assert_eq!(chart.notes[0].hold_duration, 0.75);
            assert_eq!(editor.undo.len(), 1);
            assert_eq!(editor.placement_sequence[0], chart.notes[0]);
        }
    }

    #[test]
    fn hold_placement_can_confirm_zero_duration_or_cancel_without_edit() {
        let mut editor = EditorState { tool: crate::state::Tool::Hold, ..Default::default() };
        let mut chart = chart(Vec::new());
        editor.place_hold_step(&mut chart, 2.0, 3);
        editor.hold_placement = None;
        assert!(chart.notes.is_empty());
        assert!(editor.undo.is_empty());
        editor.place_hold_step(&mut chart, 2.0, 3);
        editor.place_hold_step(&mut chart, 1.0, 8);
        assert_eq!(chart.notes[0].hold_duration, 0.0);
    }

    #[test]
    fn repeat_placement_requires_two_compatible_notes_and_wraps_lanes() {
        let mut editor = EditorState { tool: super::super::state::Tool::Tap, ..Default::default() };
        let mut chart = chart(Vec::new());
        editor.place_note(&mut chart, 2.0, 7);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 1);
        editor.place_note(&mut chart, 2.25, 8);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (2.5, 1));
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!((chart.notes[3].time, chart.notes[3].lane), (2.75, 2));
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!((chart.notes[4].time, chart.notes[4].lane), (3.0, 3));
        assert_eq!(editor.single_selected(), Some(chart.notes[4].id));
        assert_eq!(editor.undo.len(), 5);
        editor.undo(&mut chart);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 4, "undo invalidates placement detection");
    }

    #[test]
    fn repeat_placement_keeps_triplet_grid_at_large_measures() {
        let mut editor = EditorState {
            tool: super::super::state::Tool::Tap, snap_division: 12, ..Default::default()
        };
        let mut chart = chart(Vec::new());
        for tick in 12000..12003 {
            editor.place_note(&mut chart, tick as f32 / 12.0, 3);
        }
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 4);
        assert_eq!(chart.notes[3].time, 12003.0 / 12.0);
    }

    #[test]
    fn repeat_placement_rejects_incompatible_changed_or_stationary_notes() {
        for case in 0..6 {
            let mut editor = EditorState { tool: super::super::state::Tool::Tap, ..Default::default() };
            let mut chart = chart(Vec::new());
            editor.place_note(&mut chart, 2.0, 1);
            if case == 0 { editor.tool = crate::state::Tool::Hold; }
            if case == 1 { editor.tool = crate::state::Tool::Touch; }
            editor.place_note(&mut chart, if case == 2 { 2.0 } else { 2.25 }, if case == 2 { 1 } else { 2 });
            if case == 3 { chart.notes[1].is_break = true; }
            if case == 4 { chart.notes[1].time = 4.0; }
            if case == 5 { chart.notes[0].note_type = NoteType::Hold; }
            crate::timeline::transform::repeat(&mut editor, &mut chart);
            assert_eq!(chart.notes.len(), 2, "case {case}");
            assert_eq!(editor.undo.len(), 2);
        }
    }

    #[test]
    fn deleting_placements_rebuilds_pattern_from_latest_two_survivors() {
        let mut editor = EditorState { tool: crate::state::Tool::Tap, ..Default::default() };
        let mut chart = chart(Vec::new());
        let mut ids = Vec::new();
        for (time, lane) in [(2.0, 1), (2.25, 2), (3.0, 3), (4.0, 4), (5.0, 5)] {
            ids.push(editor.place_note(&mut chart, time, lane));
        }
        editor.selected = ids[2..].iter().copied().collect();
        editor.delete_selection(&mut chart);
        assert_eq!(editor.placement_sequence.iter().map(|n| n.id).collect::<Vec<_>>(), ids[..2]);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 3);
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (2.5, 3));
        editor.delete_selection(&mut chart);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (2.5, 3));
        editor.selected = [ids[0], ids[1]].into_iter().collect();
        editor.delete_selection(&mut chart);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 1, "one surviving placement is insufficient");
    }

    #[test]
    fn placement_pattern_uses_latest_pair_instead_of_older_intervals() {
        let mut editor = EditorState { tool: crate::state::Tool::Tap, ..Default::default() };
        let mut chart = chart(Vec::new());
        for (time, lane) in [(2.0, 1), (4.0, 4), (4.5, 5)] { editor.place_note(&mut chart, time, lane); }
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 4);
        assert_eq!((chart.notes[3].time, chart.notes[3].lane), (5.0, 6));
    }

    #[test]
    fn deleting_one_note_preserves_other_selection_and_surviving_placement_references() {
        let mut editor = EditorState { tool: crate::state::Tool::Tap, ..Default::default() };
        let mut chart = chart(Vec::new());
        let a = editor.place_note(&mut chart, 2.0, 1);
        let b = editor.place_note(&mut chart, 2.25, 2);
        let c = editor.place_note(&mut chart, 3.0, 5);
        editor.select_only(a);
        editor.keyboard.candidate = Some(c); editor.hovered = Some(c);
        editor.delete_note(&mut chart, c);
        assert_eq!(editor.single_selected(), Some(a));
        assert_eq!(editor.keyboard.candidate, None);
        assert_eq!(editor.hovered, None);
        assert_eq!(editor.placement_sequence.iter().map(|n| n.id).collect::<Vec<_>>(), vec![a, b]);
        editor.delete_note(&mut chart, 999);
        assert_eq!(editor.undo.len(), 4, "deleting a missing note is not an edit");
        editor.repeat_placement(&mut chart);
        assert_eq!((chart.notes[2].time, chart.notes[2].lane), (2.5, 3));
    }

    #[test]
    fn repeat_completed_star_placement_preserves_timing() {
        let mut editor = EditorState { tool: super::super::state::Tool::Slide, ..Default::default() };
        let mut chart = chart(Vec::new());
        for (time, lane) in [(2.0, 1), (3.0, 2), (4.0, 3)] {
            editor.place_star_step(&mut chart, time, lane);
            editor.place_star_step(&mut chart, time + 0.25, lane);
            editor.place_star_step(&mut chart, time + 0.75, lane);
            assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(lane + 4)));
            assert!(editor.confirm_slide_edit(&mut chart));
        }
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 4);
        assert_eq!((chart.notes[3].time, chart.notes[3].lane), (5.0, 4));
        assert_eq!(chart.notes[3].slide[0].slide_start_delay, 0.25);
        assert_eq!(chart.notes[3].slide[0].slide_duration, 0.75);
        assert_eq!(editor.undo.len(), 4);
    }

    #[test]
    fn hold_placement_starts_with_zero_duration() {
        for tool in [super::super::state::Tool::Hold, super::super::state::Tool::TouchHold] {
            let mut editor = EditorState { tool, ..Default::default() };
            let mut chart = chart(Vec::new());
            let id = editor.place_note(&mut chart, 2.0, 3);
            assert_eq!(chart.notes[0].hold_duration, 0.0);
            assert_eq!(editor.single_selected(), Some(id));
            editor.undo(&mut chart);
            assert!(chart.notes.is_empty());
        }
    }

    #[test]
    fn pending_same_head_placement_does_not_edit_and_rejects_deleted_parent() {
        let mut editor = EditorState::default();
        let mut star = note(1, 2.0, 3);
        star.note_type = NoteType::Slide;
        let mut chart = chart(vec![star]);
        assert!(editor.begin_same_head_placement(&chart, 1));
        editor.place_star_step(&mut chart, 2.25, 8);
        assert!(chart.notes[0].slide.is_empty());
        assert!(editor.undo.is_empty());
        chart.notes.clear();
        editor.place_star_step(&mut chart, 3.0, 8);
        assert!(chart.notes.is_empty());
        assert!(editor.star_placement.is_none());
        assert!(editor.undo.is_empty());
    }

    #[test]
    fn switching_tools_cancels_slide_timing_placement() {
        let mut editor = EditorState { tool: crate::state::Tool::Slide, ..Default::default() };
        let mut chart = chart(Vec::new());
        editor.place_star_step(&mut chart, 2.0, 3);
        editor.tool = crate::state::Tool::Tap;
        editor.reconcile_slide_edit(&mut chart);
        assert!(editor.star_placement.is_none());
        assert!(chart.notes.is_empty());
        assert!(editor.undo.is_empty());
        assert!(!editor.dirty);
    }

    #[test]
    fn unfinished_star_path_is_provisional() {
        let mut editor = EditorState { tool: super::super::state::Tool::Slide, ..Default::default() };
        let mut chart = chart(Vec::new());
        for time in [2.0, 2.25, 3.0] {
            editor.place_star_step(&mut chart, time, 3);
        }
        assert!(chart.notes.is_empty());
        assert!(editor.editing_slide.is_some());
        assert!(editor.slide_draft.is_some());
        assert!(editor.undo.is_empty(), "timing alone must not commit a pathless Slide");
        assert!(!editor.dirty);
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(7)));
        assert!(chart.notes.is_empty());
        assert!(editor.undo.is_empty(), "draft path edits belong to one placement");
        assert!(!editor.dirty);
    }

    #[test]
    fn star_placement_requires_increasing_times_and_commits_one_history_step() {
        let mut editor = EditorState::default();
        editor.tool = super::super::state::Tool::Slide;
        let mut chart = chart(Vec::new());
        editor.place_star_step(&mut chart, 2.0, 3);
        assert!(chart.notes.is_empty());
        assert!(editor.undo.is_empty());
        editor.place_star_step(&mut chart, 1.5, 7);
        assert_eq!(editor.star_placement.unwrap().delay_end, None);
        editor.place_star_step(&mut chart, 2.25, 7);
        assert!(chart.notes.is_empty());
        editor.place_star_step(&mut chart, 2.25, 8);
        assert!(chart.notes.is_empty());
        editor.place_star_step(&mut chart, 3.0, 8);
        assert!(editor.star_placement.is_none());
        assert!(chart.notes.is_empty());
        let preview = editor.slide_preview_note(&chart).unwrap();
        assert_eq!((preview.time, preview.lane), (2.0, 3));
        assert_eq!(preview.slide[0].slide_start_delay, 0.25);
        assert_eq!(preview.slide[0].slide_duration, 1.0);
        assert_eq!(editor.editing_slide, Some(preview.id));
        assert!(editor.undo.is_empty());
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(7)));
        assert!(editor.confirm_slide_edit(&mut chart));
        assert_eq!(chart.notes.len(), 1);
        assert_eq!(editor.editing_slide, None);
        assert_eq!(editor.undo.len(), 1);
        editor.undo(&mut chart);
        assert!(chart.notes.is_empty());
        editor.redo(&mut chart);
        assert_eq!(chart.notes[0].slide[0].slide_start_delay, 0.25);
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
    fn nudge_moves_selected_by_grid_and_wraps_lanes() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![note(1, 4.0, 1), note(2, 4.5, 8)]);
        editor.selected.insert(1);
        editor.selected.insert(2);
        editor.nudge_selection(&mut chart, 0.25, 1);
        assert!((chart.notes[0].time - 4.25).abs() < 1e-4);
        assert_eq!(chart.notes[0].lane, 2);
        assert_eq!(chart.notes[1].lane, 1, "outer lanes wrap around");
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
    fn edited_conslide_render_preserves_every_waypoint() {
        use crate::app::pad_svg::PadSvgDef;
        use crate::app::slide_render::build_slide_path;
        use crate::app::types::{PAD_ROTATION_RAD, PadGeom};
        use macroquad::math::vec2;

        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 3)]);
        editor.editing_slide = Some(1);
        let svg = PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")).unwrap();
        let pad = PadGeom {
            cx: 0.0,
            cy: 0.0,
            outer_r: 480.0,
        };
        let assert_path = |chart: &ChartDoc, lanes: &[u8]| {
            let n = &chart.notes[0];
            let path =
                build_slide_path(n, &n.slide[0], &pad, &svg, 1.0, vec2(0.0, 0.0), pad.outer_r);
            let mut remaining = path.as_slice();
            for lane in lanes {
                let angle = -std::f32::consts::FRAC_PI_2
                    + PAD_ROTATION_RAD
                    + (*lane - 1) as f32 * std::f32::consts::TAU / 8.0;
                let expected = vec2(angle.cos(), angle.sin())
                    * (pad.outer_r + crate::app::params::tap_target_offset());
                let index = remaining.iter().position(|point| (*point - expected).length() < 0.01)
                    .unwrap_or_else(|| panic!("missing ordered lane {lane}"));
                remaining = &remaining[index + 1..];
            }
        };
        editor.append_slide_waypoint(&mut chart, PadZone::from(5u8));
        editor.append_slide_waypoint(&mut chart, PadZone::from(8u8));
        assert_path(&chart, &[3, 5, 8]);
        editor.append_slide_waypoint(&mut chart, PadZone::from(5u8));
        assert_path(&chart, &[3, 5, 8, 5]);
        editor.pop_slide_waypoint(&mut chart);
        assert_path(&chart, &[3, 5, 8]);
        editor.undo(&mut chart);
        assert_path(&chart, &[3, 5, 8, 5]);
        editor.redo(&mut chart);
        assert_path(&chart, &[3, 5, 8]);
    }

    #[test]
    fn center_v_shape_is_restored_after_removing_extra_waypoint() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 3)]);
        editor.editing_slide = Some(1);
        editor.append_slide_waypoint(&mut chart, PadZone::from(17u8));
        editor.append_slide_waypoint(&mut chart, PadZone::from(8u8));
        assert_eq!(
            chart.notes[0].slide[0].segments[0].shape,
            SlideShape::VShape
        );
        editor.append_slide_waypoint(&mut chart, PadZone::from(5u8));
        assert_eq!(chart.notes[0].slide[0].segments[0].shape, SlideShape::Line);
        editor.pop_slide_waypoint(&mut chart);
        assert_eq!(
            chart.notes[0].slide[0].segments[0].shape,
            SlideShape::VShape
        );
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
        // A ring waypoint is not a centre-V; preserve the free-form line.
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(5u8)));
        let seg = &chart.notes[0].slide[0].segments[0];
        assert_eq!(seg.points.len(), 2);
        assert_eq!(seg.shape, SlideShape::Line);
    }

    #[test]
    fn appending_during_a_later_edit_preserves_explicit_shape_segments() {
        use crate::app::types::{Slide, SlidePoint, SlideSegment};

        let mut editor = EditorState::default();
        let mut note = slide_note(1, 4.0, 1);
        note.slide = vec![Slide {
            slide_start_delay: 0.25,
            slide_duration: 1.0,
            segments: vec![SlideSegment {
                shape: SlideShape::Right,
                points: vec![SlidePoint::from(PadZone::from(3u8))],
            }],
            slide_is_break: false,
            runtime_parts: 1,
        }];
        let mut chart = chart(vec![note]);
        editor.editing_slide = Some(1);

        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(5u8)));

        let segments = &chart.notes[0].slide[0].segments;
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].shape, SlideShape::Right);
        assert_eq!(segments[0].points[0].zone, PadZone::from(3u8));
        assert_eq!(segments[1].shape, SlideShape::Line);
        assert_eq!(segments[1].points[0].zone, PadZone::from(5u8));

        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(7u8)));
        assert_eq!(chart.notes[0].slide[0].segments[0].shape, SlideShape::Right);
        assert_eq!(chart.notes[0].slide[0].segments[1].shape, SlideShape::Line);
        assert_eq!(chart.notes[0].slide[0].segments[1].points.len(), 2);

        assert!(editor.pop_slide_waypoint(&mut chart));
        assert!(editor.pop_slide_waypoint(&mut chart));
        assert_eq!(chart.notes[0].slide[0].segments.len(), 1);
        assert_eq!(chart.notes[0].slide[0].segments[0].shape, SlideShape::Right);
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
        // A chained shape starts at the preceding endpoint (3), not the head (1).
        assert!(!editor.apply_slide_shape(&mut chart, SlideShape::S, 5));
        assert!(editor.apply_slide_shape(&mut chart, SlideShape::S, 7));
        assert_eq!(chart.notes[0].slide[0].segments.len(), 3);
        assert_eq!(chart.notes[0].slide[0].segments[2].shape, SlideShape::S);
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
        assert!(
            !editor.pop_slide_waypoint(&mut chart),
            "nothing left to pop"
        );
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

    #[test]
    fn add_and_remove_sub_slide_preserve_existing_path_and_undo() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1), note(2, 5.0, 2)]);
        editor.editing_slide = Some(1);
        editor.append_slide_waypoint(&mut chart, PadZone::from(3u8));
        chart.notes[0].slide[0].slide_start_delay = 0.25;
        chart.notes[0].slide[0].slide_duration = 2.0;
        let original = serde_json::to_value(&chart.notes[0].slide[0]).unwrap();
        let history = editor.undo.len();
        assert!(!editor.add_sub_slide(&mut chart, 2));
        assert!(!editor.add_sub_slide(&mut chart, 99));
        assert_eq!(editor.undo.len(), history);
        assert!(editor.add_sub_slide(&mut chart, 1));
        assert_eq!(editor.single_selected(), Some(1));
        assert_eq!(editor.editing_slide_idx, 1);
        assert_eq!(chart.notes[0].slide.len(), 1);
        assert_eq!(editor.slide_preview_note(&chart).unwrap().slide.len(), 2);
        editor.append_slide_waypoint(&mut chart, PadZone::from(6u8));
        assert!(editor.confirm_slide_edit(&mut chart));
        editor.editing_slide = Some(1);
        editor.editing_slide_idx = 1;
        assert!(chart.notes[0].is_star, "same-head trajectories use a double star");
        assert_eq!(
            serde_json::to_value(&chart.notes[0].slide[0]).unwrap(),
            original
        );
        assert!(editor.remove_sub_slide(&mut chart));
        assert_eq!(chart.notes[0].slide.len(), 1);
        assert_eq!(editor.editing_slide_idx, 0);
        assert!(!chart.notes[0].is_star);
        editor.undo(&mut chart);
        assert_eq!(chart.notes[0].slide.len(), 2);
        assert_eq!(
            chart.notes[0].slide[1].segments[0].points[0].zone,
            PadZone::from(6u8)
        );
        editor.redo(&mut chart);
        assert_eq!(chart.notes[0].slide.len(), 1);
        assert_eq!(
            serde_json::to_value(&chart.notes[0].slide[0]).unwrap(),
            original
        );
        editor.editing_slide = Some(1);
        editor.editing_slide_idx = 0;
        assert!(editor.remove_sub_slide(&mut chart));
        assert!(!editor.remove_sub_slide(&mut chart));
        assert_eq!(chart.notes.len(), 2, "deleting a trajectory keeps the note");
        assert_eq!(chart.notes[0].note_type, NoteType::Tap);
        assert_eq!(editor.editing_slide, None);
    }

    #[test]
    fn undo_new_sub_slide_keeps_editing_target_in_range() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.add_sub_slide(&mut chart, 1);
        editor.append_slide_waypoint(&mut chart, PadZone::from(5));
        assert!(editor.confirm_slide_edit(&mut chart));
        editor.add_sub_slide(&mut chart, 1);
        editor.append_slide_waypoint(&mut chart, PadZone::from(6));
        assert!(editor.confirm_slide_edit(&mut chart));
        editor.editing_slide = Some(1);
        editor.editing_slide_idx = 1;
        editor.undo(&mut chart);
        assert_eq!(editor.editing_slide_idx, 0);
        assert_eq!(chart.notes[0].slide.len(), 1);
        editor.undo(&mut chart);
        assert!(chart.notes[0].slide.is_empty());
        editor.redo(&mut chart);
        assert_eq!(editor.editing_slide_idx, 0);
        editor.editing_slide = Some(99);
        editor.undo(&mut chart);
        assert_eq!(editor.editing_slide, None);
    }

    #[test]
    fn bounded_history_keeps_the_latest_edits() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        for _ in 0..70 {
            assert!(editor.add_sub_slide(&mut chart, 1));
            editor.append_slide_waypoint(&mut chart, PadZone::from(5));
            assert!(editor.confirm_slide_edit(&mut chart));
        }
        assert_eq!(editor.undo.len(), 64);
        editor.undo(&mut chart);
        assert_eq!(chart.notes[0].slide.len(), 69);
        editor.redo(&mut chart);
        assert_eq!(chart.notes[0].slide.len(), 70);
        for _ in 0..64 {
            editor.undo(&mut chart);
        }
        assert_eq!(chart.notes[0].slide.len(), 6);
        assert!(editor.undo.is_empty());
    }
}
