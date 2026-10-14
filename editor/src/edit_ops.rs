//! Chart-mutating edit operations (undo/redo, clipboard, paste, delete).
//!
//! Kept separate from [`super::state`] so the state module stays descriptive and
//! these behaviours can be read and tested as a unit.

use std::collections::HashSet;

use crate::app::types::{
    ChartDoc, Note, NoteType, SlidePoint, SlideSegment, SlideShape, validate_slide_shape,
    zone::PadZone,
};

use super::state::EditorState;

impl EditorState {
    /// Snapshot `chart` onto the undo stack before a mutation. Trims the stack to
    /// a bounded depth and drops any redo history.
    pub fn push_history(&mut self, chart: &ChartDoc) {
        self.placement_sequence.clear();
        self.undo.push(chart.clone());
        if self.undo.len() > 64 {
            self.undo.drain(..self.undo.len() - 64);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self, chart: &mut ChartDoc) {
        if let Some(previous) = self.undo.pop() {
            self.placement_sequence.clear();
            self.last_edit = None;
            self.redo.push(chart.clone());
            *chart = previous;
            self.reconcile_slide_target(chart);
            self.dirty = true;
        }
    }

    pub fn redo(&mut self, chart: &mut ChartDoc) {
        if let Some(next) = self.redo.pop() {
            self.placement_sequence.clear();
            self.last_edit = None;
            self.undo.push(chart.clone());
            *chart = next;
            self.reconcile_slide_target(chart);
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

    /// Duplicate in place without touching the clipboard or committing history.
    /// The modal transform owns the combined duplicate/move undo step.
    pub fn duplicate_selection(&mut self, chart: &mut ChartDoc) -> bool {
        let mut copies: Vec<_> = chart.notes.iter()
            .filter(|n| self.selected.contains(&n.id)).cloned().collect();
        if copies.is_empty() {
            return false;
        }
        let mut next_id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0);
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
        let mut sequence = std::mem::take(&mut self.placement_sequence);
        self.push_history(chart);
        let id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        chart.notes.push(Note {
            id,
            ..crate::timeline::placement::note_for_tool(self, time, lane)
        });
        sequence.push(chart.notes.last().unwrap().clone());
        if sequence.len() > 3 {
            sequence.remove(0);
        }
        self.placement_sequence = sequence;
        self.last_edit = None;
        self.select_only(id);
        id
    }

    /// Continue only a confirmed three-placement translation pattern.
    pub fn repeat_placement(&mut self, chart: &mut ChartDoc) {
        use crate::timeline::geometry::{move_note, snap};
        let [a, b, c] = self.placement_sequence.as_slice() else {
            self.status = Some("No placement pattern".into());
            return;
        };
        let tolerance =
            (a.time.abs().max(b.time.abs()).max(c.time.abs()) * f32::EPSILON * 4.0).max(1e-5);
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
        let dt = (c.time - a.time) * 0.5;
        let dl = (c.lane as i32 - b.lane as i32).rem_euclid(8);
        let valid = ((c.time - b.time) - (b.time - a.time)).abs() <= tolerance
            && dl == (b.lane as i32 - a.lane as i32).rem_euclid(8)
            && (dt.abs() > 1e-5 || dl != 0)
            && compatible(a, b)
            && compatible(b, c)
            && self.placement_sequence.iter().all(|old| {
                chart.notes.iter().any(|n| {
                    n.id == old.id
                        && (n.time - old.time).abs() < 1e-5
                        && n.lane == old.lane
                        && compatible(n, old)
                })
            });
        let time = snap(c.time + dt, self.snap_enabled, self.snap_division);
        if !valid || time < 1.0 {
            self.status = Some("No placement pattern".into());
            return;
        }
        let mut copy = c.clone();
        move_note(&mut copy, time, dl);
        copy.id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        copy.is_each = false;
        copy.is_each_head = false;
        let sequence = vec![b.clone(), c.clone(), copy.clone()];
        self.push_history(chart);
        self.select_only(copy.id);
        chart.notes.push(copy);
        self.placement_sequence = sequence;
        self.status = Some("Repeated placement".into());
    }

    /// Three-click star placement: head, motion start, then tail. Pending
    /// geometry never enters the chart or history until all three are valid.
    pub fn place_star_step(&mut self, chart: &mut ChartDoc, time: f32, lane: u8) {
        use super::state::StarPlacement;
        match self.star_placement {
            None => {
                self.star_placement = Some(StarPlacement {
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
                let id = if let Some(id) = pending.note_id {
                    if !chart
                        .notes
                        .iter()
                        .any(|n| n.id == id && n.note_type == NoteType::Slide)
                    {
                        self.star_placement = None;
                        return;
                    }
                    self.push_history(chart);
                    id
                } else {
                    self.place_note(chart, pending.head, pending.lane)
                };
                let note = chart.notes.iter_mut().find(|n| n.id == id).unwrap();
                let mut slide = default_slide();
                slide.slide_start_delay = delay - pending.head;
                slide.slide_duration = time - pending.head;
                note.slide.push(slide);
                if pending.note_id.is_some() {
                    self.editing_slide_idx = note.slide.len() - 1;
                    self.editing_slide = Some(id);
                    self.pending_shape = None;
                    self.select_only(id);
                }
                crate::app::maichart::mark_double_stars(&mut chart.notes);
                if pending.note_id.is_none() {
                    if let Some(last) = self.placement_sequence.last_mut() {
                        *last = chart.notes.iter().find(|n| n.id == id).unwrap().clone();
                    }
                }
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
        let Some(idx) = chart
            .notes
            .iter()
            .position(|n| n.id == id && n.note_type == NoteType::Slide)
        else {
            return false;
        };
        self.push_history(chart);
        let note = &mut chart.notes[idx];
        note.slide.push(default_slide());
        self.editing_slide_idx = note.slide.len() - 1;
        crate::app::maichart::mark_double_stars(&mut chart.notes);
        self.editing_slide = Some(id);
        self.pending_shape = None;
        self.select_only(id);
        true
    }

    /// Delete only the active trajectory, leaving the star and other paths intact.
    pub fn remove_sub_slide(&mut self, chart: &mut ChartDoc) -> bool {
        let Some((idx, sub)) = self.editing_target(chart) else {
            return false;
        };
        if chart.notes[idx].slide.is_empty() {
            return false;
        }
        self.push_history(chart);
        chart.notes[idx].slide.remove(sub);
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
        let start_lane = chart.notes[idx].lane;
        if let Some(sl) = chart.notes[idx].slide.get_mut(sub) {
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
        if end_lane < 1
            || end_lane > 8
            || validate_slide_shape(shape, start_lane, end_lane).is_err()
        {
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
    fn repeat_placement_requires_three_regular_compatible_notes_and_wraps_lanes() {
        let mut editor = EditorState { tool: super::super::state::Tool::Tap, ..Default::default() };
        let mut chart = chart(Vec::new());
        editor.place_note(&mut chart, 2.0, 7);
        editor.place_note(&mut chart, 2.25, 8);
        crate::timeline::transform::repeat(&mut editor, &mut chart);
        assert_eq!(chart.notes.len(), 2);
        editor.place_note(&mut chart, 2.5, 1);
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
    fn repeat_placement_rejects_irregular_or_changed_notes() {
        for case in 0..5 {
            let mut editor = EditorState { tool: super::super::state::Tool::Tap, ..Default::default() };
            let mut chart = chart(Vec::new());
            editor.place_note(&mut chart, 2.0, 1);
            editor.place_note(&mut chart, 2.25, 2);
            editor.place_note(&mut chart, if case == 0 { 2.75 } else { 2.5 }, if case == 1 { 4 } else { 3 });
            if case == 2 { chart.notes[1].is_break = true; }
            if case == 3 { chart.notes[2].time = 4.0; }
            if case == 4 { chart.notes[0].note_type = NoteType::Hold; }
            crate::timeline::transform::repeat(&mut editor, &mut chart);
            assert_eq!(chart.notes.len(), 3, "case {case}");
            assert_eq!(editor.undo.len(), 3);
        }
    }

    #[test]
    fn repeat_completed_star_placement_preserves_timing() {
        let mut editor = EditorState { tool: super::super::state::Tool::Slide, ..Default::default() };
        let mut chart = chart(Vec::new());
        for (time, lane) in [(2.0, 1), (3.0, 2), (4.0, 3)] {
            editor.place_star_step(&mut chart, time, lane);
            editor.place_star_step(&mut chart, time + 0.25, lane);
            editor.place_star_step(&mut chart, time + 0.75, lane);
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
        assert_eq!(chart.notes.len(), 1);
        assert_eq!((chart.notes[0].time, chart.notes[0].lane), (2.0, 3));
        assert_eq!(chart.notes[0].slide[0].slide_start_delay, 0.25);
        assert_eq!(chart.notes[0].slide[0].slide_duration, 1.0);
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
        assert_eq!(chart.notes[0].slide.len(), 2);
        editor.append_slide_waypoint(&mut chart, PadZone::from(6u8));
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
        assert!(editor.remove_sub_slide(&mut chart));
        assert!(!editor.remove_sub_slide(&mut chart));
        assert_eq!(chart.notes.len(), 2, "deleting a trajectory keeps the star");
        assert!(editor.append_slide_waypoint(&mut chart, PadZone::from(4u8)));
    }

    #[test]
    fn undo_new_sub_slide_keeps_editing_target_in_range() {
        let mut editor = EditorState::default();
        let mut chart = chart(vec![slide_note(1, 4.0, 1)]);
        editor.add_sub_slide(&mut chart, 1);
        editor.add_sub_slide(&mut chart, 1);
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
