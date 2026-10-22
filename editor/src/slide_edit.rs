//! A new Slide path is a single placement, not a series of partial edits.

use std::collections::HashSet;

use crate::app::types::{ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape};
use crate::state::{EditorState, Tool, Workspace};

#[derive(Debug, Clone)]
pub struct Placement {
    before: ChartDoc,
    selected: HashSet<u64>,
    was_dirty: bool,
    note: Note,
    id: u64,
    sub: usize,
    new_note: bool,
    tool: Tool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{maidata, params};
    use crate::app::types::zone::PadZone;

    fn fixture() -> (EditorState, ChartDoc) {
        (EditorState { tool: Tool::Slide, ..Default::default() },
            maidata::from_maidata("&inote_1=(120){4}1,,E", None).unwrap())
    }

    fn place(editor: &mut EditorState, chart: &mut ChartDoc) {
        for time in [2.0, 2.25, 3.0] { editor.place_star_step(chart, time, 1); }
    }

    #[test]
    fn empty_path_stays_pending_until_one_complete_placement_is_confirmed() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        place(&mut editor, &mut chart);
        assert!(editor.editing_slide.is_some());
        assert!(!editor.confirm_slide_edit(&mut chart));
        assert!(editor.check_slide_placement_finished().is_err());
        assert!(editor.undo.is_empty());
        assert!(!editor.dirty);
        editor.append_slide_waypoint(&mut chart, PadZone::from(5));
        assert!(editor.confirm_slide_edit(&mut chart));
        let placed = chart.clone();
        assert!(editor.check_slide_placement_finished().is_ok());
        assert_eq!(editor.undo.len(), 1);
        assert!(editor.dirty);
        editor.undo(&mut chart);
        assert_eq!(chart, original);
        editor.redo(&mut chart);
        assert_eq!(chart, placed);
    }

    #[test]
    fn cancelling_a_same_head_path_preserves_the_existing_note_and_history() {
        let (mut editor, mut chart) = fixture();
        place(&mut editor, &mut chart);
        editor.append_slide_waypoint(&mut chart, PadZone::from(5));
        assert!(editor.confirm_slide_edit(&mut chart));
        let id = chart.notes.last().unwrap().id;
        let original = chart.clone();
        let history = editor.undo.len();
        assert!(editor.add_sub_slide(&mut chart, id));
        editor.append_slide_waypoint(&mut chart, PadZone::from(7));
        editor.exit_slide_edit(&mut chart);
        assert_eq!(chart, original);
        assert_eq!(editor.undo.len(), history);
        assert!(editor.dirty);
        editor.editing_slide = Some(id);
        editor.exit_slide_edit(&mut chart);
        assert_eq!(chart, original, "exiting existing path editing must not delete it");
    }

    #[test]
    fn pending_path_undo_and_redo_cancel_without_consuming_history() {
        for redo in [false, true] {
            let (mut editor, mut chart) = fixture();
            editor.tool = Tool::Tap;
            editor.place_note(&mut chart, 1.5, 3);
            if redo { editor.undo(&mut chart); }
            let original = chart.clone();
            let history = (editor.undo.len(), editor.redo.len());
            editor.tool = Tool::Slide;
            place(&mut editor, &mut chart);
            editor.append_slide_waypoint(&mut chart, PadZone::from(5));
            if redo { editor.redo(&mut chart); } else { editor.undo(&mut chart); }
            assert_eq!(chart, original);
            assert_eq!((editor.undo.len(), editor.redo.len()), history);
            assert!(editor.check_slide_placement_finished().is_ok());
            assert!(editor.editing_slide.is_none());
        }
    }

    #[test]
    fn leaving_chart_workspace_discards_only_the_unconfirmed_placement() {
        let (mut editor, mut chart) = fixture();
        let original = chart.clone();
        place(&mut editor, &mut chart);
        editor.workspace = Workspace::Autoplay;
        editor.reconcile_slide_edit(&mut chart);
        assert_eq!(chart, original);
        assert!(editor.check_slide_placement_finished().is_ok());
        assert!(editor.undo.is_empty());
        assert!(!editor.dirty);
    }

    #[test]
    fn default_opposite_previews_all_eight_lanes_until_confirmed() {
        struct Restore(params::Params);
        impl Drop for Restore { fn drop(&mut self) { params::set(self.0.clone()); } }
        let _restore = Restore(params::get());
        let mut configured = params::get();
        configured.editor_slide_default_opposite = true;
        let saved = serde_json::to_string(&configured).unwrap();
        params::set(serde_json::from_str(&saved).unwrap());
        assert!(!serde_json::from_str::<params::Params>("{}").unwrap().editor_slide_default_opposite);
        for (lane, opposite) in [(1, 5), (2, 6), (3, 7), (4, 8), (5, 1), (6, 2), (7, 3), (8, 4)] {
            let (mut editor, mut chart) = fixture();
            let before = chart.clone();
            for time in [2.0, 2.25, 3.0] { editor.place_star_step(&mut chart, time, lane); }
            assert_eq!(chart, before);
            let note = editor.slide_preview_note(&chart).unwrap();
            assert_eq!(note.slide[0].segments[0].shape, SlideShape::Line);
            assert_eq!(note.slide[0].segments[0].points[0].zone, PadZone::from(opposite));
            assert_eq!(editor.editing_slide, Some(note.id));
            assert!(editor.undo.is_empty());
            assert!(editor.check_slide_placement_finished().is_err());
            assert!(editor.confirm_slide_edit(&mut chart));
            assert_eq!(chart.notes.last(), Some(&note));
            assert_eq!(editor.undo.len(), 1);
            assert!(editor.check_slide_placement_finished().is_ok());
            editor.undo(&mut chart);
            assert_eq!(chart, before);
        }
    }

    #[test]
    fn slide_head_preview_tracks_pending_timing_and_path_then_clears() {
        let (mut editor, mut chart) = fixture();
        editor.place_star_step(&mut chart, 2.0, 3);
        let head = editor.slide_head_preview_note(&chart).unwrap();
        assert_eq!(head.note_type, NoteType::Slide);
        assert_eq!(head.lane, 3);

        editor.place_star_step(&mut chart, 2.25, 3);
        editor.place_star_step(&mut chart, 3.0, 3);
        assert!(editor.slide_draft.is_some());
        assert_eq!(chart.notes.len(), 1);
        let preview = editor.slide_head_preview_note(&chart).unwrap();
        assert_eq!((preview.time, preview.lane), (2.0, 3));
        assert_eq!(preview.slide[0].slide_start_delay, 0.25);
        assert_eq!(preview.slide[0].slide_duration, 1.0);

        editor.exit_slide_edit(&mut chart);
        assert!(editor.slide_head_preview_note(&chart).is_none());
        assert_eq!(chart.notes.len(), 1);
    }
}

impl EditorState {
    pub(crate) fn place_slide_path(
        &mut self,
        chart: &mut ChartDoc,
        parent: Option<u64>,
        head: f32,
        lane: u8,
        mut slide: Slide,
    ) -> bool {
        self.exit_slide_edit(chart);
        if parent.is_some_and(|id| {
            !chart.notes.iter().any(|n| n.id == id && n.note_type == NoteType::Slide)
        }) {
            return false;
        }
        let default_path = crate::app::params::get().editor_slide_default_opposite;
        let id = parent.unwrap_or_else(|| chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1);
        let mut note = parent
            .and_then(|id| chart.notes.iter().find(|n| n.id == id).cloned())
            .unwrap_or_else(|| Note {
                id, time: head, lane, note_type: NoteType::Slide, ..Default::default()
            });
        let sub = note.slide.len();
        if default_path {
            let opposite = (lane.clamp(1, 8) - 1 + 4) % 8 + 1;
            slide.segments = vec![SlideSegment {
                shape: SlideShape::Line,
                points: vec![SlidePoint::from(crate::app::types::zone::PadZone::from(opposite))],
            }];
        }
        note.slide.push(slide);
        crate::app::maichart::mark_double_stars(std::slice::from_mut(&mut note));
        self.slide_draft = Some(Placement {
            before: chart.clone(),
            selected: self.selected.clone(),
            was_dirty: self.dirty,
            note,
            id,
            sub,
            new_note: parent.is_none(),
            tool: self.tool,
        });
        self.editing_slide = Some(id);
        self.editing_slide_idx = sub;
        self.pending_shape = None;
        self.keyboard.segment = 0;
        self.select_only(id);
        self.status = Some(if default_path { "Slide ready to confirm" } else { "Slide placement pending" }.into());
        true
    }

    fn record_slide_placement(&mut self, chart: &ChartDoc, id: u64) {
        if let Some(note) = chart.notes.iter().find(|n| n.id == id) {
            self.placement_sequence.push(note.clone());
            self.last_edit = None;
        }
    }

    /// Accept the entire provisional path as one history entry.
    pub fn confirm_slide_edit(&mut self, chart: &mut ChartDoc) -> bool {
        if let Some(draft) = &self.slide_draft {
            let valid = draft.note.slide.get(draft.sub)
                .is_some_and(|s| s.slide_duration > s.slide_start_delay
                    && !s.segments.is_empty()
                    && s.segments.iter().all(|seg| !seg.points.is_empty()));
            if !valid {
                self.status = Some("Slide needs a path before confirming".into());
                return false;
            }
            let draft = self.slide_draft.take().unwrap();
            if draft.new_note {
                if chart.notes.iter().any(|note| note.id == draft.id) {
                    self.status = Some("Slide ID is already in use".into());
                    self.slide_draft = Some(draft);
                    return false;
                }
                chart.notes.push(draft.note);
                self.record_insertion(draft.id);
                self.record_slide_placement(chart, draft.id);
            } else {
                self.push_history(&draft.before);
                let Some(note) = chart.notes.iter_mut().find(|note| note.id == draft.id) else {
                    self.slide_draft = Some(draft);
                    self.status = Some("Slide target no longer exists".into());
                    return false;
                };
                *note = draft.note;
            }
            crate::app::maichart::recompute_each(&mut chart.notes);
            crate::app::maichart::mark_double_stars(&mut chart.notes);
            self.dirty = true;
        }
        self.editing_slide = None;
        self.pending_shape = None;
        self.status = Some("Slide edit confirmed".into());
        true
    }

    /// Exiting discards only unfinished placement, never an existing Slide.
    pub fn exit_slide_edit(&mut self, chart: &mut ChartDoc) {
        self.star_placement = None;
        if let Some(draft) = self.slide_draft.take() {
            self.selected = draft.selected;
            self.dirty = draft.was_dirty;
            self.keyboard.candidate = self.single_selected();
            self.hovered = None;
            self.status = Some("Slide placement cancelled".into());
        }
        self.editing_slide = None;
        self.pending_shape = None;
        self.editing_slide_idx = 0;
    }

    fn slide_head_preview_note(&self, chart: &ChartDoc) -> Option<Note> {
        if let Some(pending) = self.star_placement {
            Some(
                pending
                    .note_id
                    .and_then(|id| chart.notes.iter().find(|note| note.id == id))
                    .cloned()
                    .unwrap_or_else(|| Note {
                        lane: pending.lane,
                        note_type: NoteType::Slide,
                        ..Default::default()
                    }),
            )
        } else {
            self.slide_draft.as_ref().map(|draft| draft.note.clone())
        }
    }

    pub(crate) fn slide_preview_note(&self, chart: &ChartDoc) -> Option<Note> {
        self.slide_head_preview_note(chart)
    }

    pub(crate) fn with_editing_slide_mut(
        &mut self,
        chart: &mut ChartDoc,
        edit: impl FnOnce(&mut Note, usize) -> bool,
    ) -> bool {
        if let Some(draft) = self.slide_draft.as_mut() {
            return edit(&mut draft.note, draft.sub);
        }
        let Some(id) = self.editing_slide else { return false; };
        let Some(note) = chart.notes.iter_mut().find(|note| note.id == id && note.note_type == NoteType::Slide) else {
            return false;
        };
        let sub = self.editing_slide_idx.min(note.slide.len().saturating_sub(1));
        edit(note, sub)
    }

    pub(crate) fn editing_slide_target<'a>(
        &'a self,
        chart: &'a ChartDoc,
    ) -> Option<(&'a Note, usize)> {
        if let Some(draft) = &self.slide_draft {
            if self.editing_slide != Some(draft.id) || self.editing_slide_idx != draft.sub {
                return None;
            }
            return Some((&draft.note, draft.sub));
        }
        let id = self.editing_slide?;
        let note = chart.notes.iter().find(|note| note.id == id && note.note_type == NoteType::Slide)?;
        let sub = self.editing_slide_idx.min(note.slide.len().saturating_sub(1));
        Some((note, sub))
    }

    pub(crate) fn draw_slide_head_preview(
        &self,
        app: &crate::player::state::PadPreviewState,
        pad: &crate::app::types::PadGeom,
        scale: f32,
    ) {
        use crate::app::slide_render;
        use crate::player::render::skin;
        let Some(svg) = &app.pad_svg else { return; };
        let Some(note) = self.slide_head_preview_note(&app.chart) else {
            return;
        };
        let center = svg.pad_visual_center(pad).unwrap_or(macroquad::math::vec2(pad.cx, pad.cy));
        if let Some(head) = slide_render::slide_start_point(&note, svg, pad, center, pad.outer_r) {
            slide_render::draw_static_star(head, scale, skin::star_body(app, &note).or(app.star_tex.as_ref()), skin::star_ex(app, &note));
        }
    }

    pub fn reconcile_slide_edit(&mut self, chart: &mut ChartDoc) {
        if self.star_placement.is_some_and(|pending| {
            self.workspace != Workspace::Chart || self.tool != pending.tool
        }) {
            self.exit_slide_edit(chart);
        }
        let cancel_keyboard = self.keyboard.transaction.as_ref().is_some_and(|txn| {
            if let crate::keyboard::edit::Draft::Star { id, sub, .. } = &txn.kind {
                self.workspace != Workspace::Chart || self.tool != txn.tool
                    || self.editing_slide != Some(*id) || self.editing_slide_idx != *sub
                    || self.single_selected() != Some(*id)
            } else {
                false
            }
        });
        if cancel_keyboard {
            crate::keyboard::edit::cancel(self, chart);
            self.exit_slide_edit(chart);
        }
        let cancelled = self.slide_draft.as_ref().is_some_and(|draft| {
            self.workspace != Workspace::Chart || self.tool != draft.tool
                || self.editing_slide != Some(draft.id) || self.editing_slide_idx != draft.sub
                || self.single_selected() != Some(draft.id)
        });
        if cancelled {
            self.exit_slide_edit(chart);
        }
    }

    pub fn check_slide_placement_finished(&self) -> Result<(), String> {
        if self.slide_draft.is_some() || self.star_placement.is_some()
            || self.keyboard.transaction.as_ref().is_some_and(|txn| {
                matches!(txn.kind, crate::keyboard::edit::Draft::Star { .. })
            })
        {
            Err("Finish or cancel the Slide placement before saving".into())
        } else {
            Ok(())
        }
    }

    pub(crate) fn slide_path_history(&mut self, chart: &ChartDoc) {
        if self.slide_draft.is_none() {
            self.push_history(chart);
        }
    }
}
