//! New drafts record an insertion; existing-note edits keep one snapshot.

use egui_macroquad::egui;
use crate::app::types::{ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape, validate_slide_shape};
use crate::app::types::zone::PadZone;
use crate::state::{EditorState, SelectionTransformKind, Tool};
use crate::timeline::geometry::{move_note, snap};
use super::{EditCursor, navigation};

#[derive(Debug, Clone)]
pub struct Transaction {
    pub before: Option<ChartDoc>,
    pub was_dirty: bool,
    pub selection: std::collections::HashSet<u64>,
    pub kind: Draft,
    pub valid: bool,
}

#[derive(Debug, Clone)]
pub enum Draft {
    Hold { id: u64 },
    Star { id: u64, sub: usize, delay: bool, accepted: Vec<SlideSegment>, new_note: bool },
    Segment { id: u64, sub: usize, index: usize, shape: SlideShape },
    Handle { id: u64, sub: usize, kind: Handle },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle { HoldTail, Delay, Tail, Lane }

fn start(editor: &mut EditorState, chart: &ChartDoc, kind: Draft) {
    let placement = matches!(kind, Draft::Hold { .. } | Draft::Star { new_note: true, .. });
    editor.keyboard.transaction = Some(Transaction {
        before: (!placement).then(|| chart.clone()), was_dirty: editor.dirty, selection: editor.selected.clone(), kind, valid: true,
    });
    editor.keyboard.suppress_pointer = true;
}

fn blank_slide() -> Slide {
    Slide { segments: Vec::new(), slide_start_delay: 0.0, slide_duration: 0.0, slide_is_break: false, runtime_parts: 1 }
}

pub fn place(editor: &mut EditorState, chart: &mut ChartDoc) {
    let cursor = editor.keyboard.cursor;
    if editor.tool == Tool::Select { navigation::select(editor, chart, 0.0, false); return; }
    if matches!(editor.tool, Tool::Break | Tool::Eraser) {
        if let Some(id) = navigation::candidate(editor, chart, 0.0) {
            if editor.tool == Tool::Eraser { editor.delete_note(chart, id); }
            else {
                editor.push_history(chart);
                if let Some(note) = chart.notes.iter_mut().find(|n| n.id == id) { note.is_break = !note.is_break; }
            }
        }
        return;
    }
    let id = editor.next_note_id(chart);
    let note = Note { id, time: cursor.measure, lane: cursor.zone, is_touch: matches!(editor.tool, Tool::Touch | Tool::TouchHold), note_type: editor.selected_type(), ..Default::default() };
    match editor.tool {
        Tool::Hold | Tool::TouchHold => {
            start(editor, chart, Draft::Hold { id }); chart.notes.push(note); editor.select_only(id);
        }
        Tool::Slide => {
            start(editor, chart, Draft::Star { id, sub: 0, delay: true, accepted: Vec::new(), new_note: true });
            chart.notes.push(Note { slide: vec![blank_slide()], ..note });
            editor.select_only(id); editor.pending_shape = Some(SlideShape::Line);
        }
        _ => {
            editor.record_insertion(id); chart.notes.push(note); editor.select_only(id);
            record_placement(editor, chart, id);
        }
    }
    editor.keyboard.candidate = Some(id);
}

fn record_placement(editor: &mut EditorState, chart: &ChartDoc, id: u64) {
    if let Some(note) = chart.notes.iter().find(|n| n.id == id) {
        editor.placement_sequence.push(note.clone());
        editor.last_edit = None;
    }
}

pub fn cancel(editor: &mut EditorState, chart: &mut ChartDoc) {
    if let Some(txn) = editor.keyboard.transaction.take() {
        if let Some(before) = txn.before { *chart = before; }
        else {
            let id = match txn.kind { Draft::Hold { id } | Draft::Star { id, .. } => id, _ => unreachable!() };
            chart.notes.retain(|note| note.id != id);
        }
        editor.dirty = txn.was_dirty; editor.selected = txn.selection;
        editor.pending_shape = None; editor.keyboard.suppress_pointer = true;
    }
}

fn commit(editor: &mut EditorState, chart: &mut ChartDoc) {
    let Some(txn) = editor.keyboard.transaction.take() else { return; };
    let placement = match txn.kind { Draft::Hold { id } | Draft::Star { id, new_note: true, .. } => Some(id), _ => None };
    let sequence = std::mem::take(&mut editor.placement_sequence);
    if let Some(before) = txn.before {
        if chart.notes != before.notes { editor.push_history(&before); } else { editor.dirty = txn.was_dirty; }
    } else if let Some(id) = placement { editor.record_insertion(id); }
    if let Some(id) = placement { editor.placement_sequence = sequence; record_placement(editor, chart, id); }
    editor.pending_shape = None; editor.keyboard.suppress_pointer = true;
    editor.status = Some("Edit confirmed".into());
}

fn endpoint(slide: &Slide, head: u8) -> u8 {
    slide.segments.iter().flat_map(|s| &s.points).last().map(|p| p.zone.to_id()).unwrap_or(head)
}

fn segment(shape: SlideShape, lane: u8) -> SlideSegment {
    let mut points = Vec::new();
    if shape == SlideShape::VShape { points.push(SlidePoint::from(PadZone::from(17))); }
    points.push(SlidePoint::from(PadZone::from(lane)));
    SlideSegment { points, shape }
}

fn valid_segment(shape: SlideShape, start: u8, end: u8) -> Result<(), String> {
    if shape == SlideShape::Line && start == end { return Err("A straight slide must end in another lane".into()); }
    validate_slide_shape(shape, start, end)
}

pub fn refresh(editor: &mut EditorState, chart: &mut ChartDoc) {
    let Some(txn) = editor.keyboard.transaction.as_mut() else { return; };
    let cursor = editor.keyboard.cursor;
    txn.valid = true;
    let mut error = None;
    match &txn.kind {
        Draft::Hold { id } => if let Some(n) = chart.notes.iter_mut().find(|n| n.id == *id) {
            n.hold_duration = (cursor.measure - n.time).max(0.0); editor.keyboard.cursor.zone = n.lane;
        },
        Draft::Star { id, sub, delay, accepted, .. } => if let Some(n) = chart.notes.iter_mut().find(|n| n.id == *id) {
            let sl = &mut n.slide[*sub];
            if *delay {
                sl.slide_start_delay = (cursor.measure - n.time).max(0.0);
                sl.slide_duration = sl.slide_start_delay;
                sl.segments.clear(); editor.keyboard.cursor.zone = n.lane;
            } else {
                let shape = editor.pending_shape.unwrap_or(SlideShape::Line);
                let start_lane = accepted.iter().flat_map(|s| &s.points).last().map(|p| p.zone.to_id()).unwrap_or(n.lane);
                error = valid_segment(shape, start_lane, cursor.zone).err();
                sl.segments = accepted.clone(); sl.segments.push(segment(shape, cursor.zone));
                sl.slide_duration = cursor.measure - n.time;
                if sl.slide_duration <= sl.slide_start_delay { error = Some("Slide tail must be after motion start".into()); }
            }
        },
        Draft::Segment { id, sub, index, shape } => if let Some(n) = chart.notes.iter_mut().find(|n| n.id == *id) {
            let sl = &mut n.slide[*sub];
            let start_lane = sl.segments[..*index].iter().flat_map(|s| &s.points).last().map(|p| p.zone.to_id()).unwrap_or(n.lane);
            error = valid_segment(*shape, start_lane, cursor.zone).err();
            let seg = segment(*shape, cursor.zone);
            if *index < sl.segments.len() { sl.segments[*index] = seg; } else { sl.segments.push(seg); }
        },
        Draft::Handle { id, sub, kind } => if let Some(n) = chart.notes.iter_mut().find(|n| n.id == *id) {
            match kind {
                Handle::HoldTail => { n.hold_duration = (cursor.measure - n.time).max(0.0); editor.keyboard.cursor.zone = n.lane; }
                Handle::Delay => {
                    let sl = &mut n.slide[*sub];
                    sl.slide_start_delay = (cursor.measure - n.time).clamp(0.0, sl.slide_duration); editor.keyboard.cursor.zone = n.lane;
                }
                Handle::Tail => {
                    let sl = &mut n.slide[*sub]; sl.slide_duration = cursor.measure - n.time;
                    if sl.slide_duration <= sl.slide_start_delay { error = Some("Slide tail must be after motion start".into()); }
                    editor.keyboard.cursor.zone = endpoint(sl, n.lane);
                }
                Handle::Lane => {
                    let sl = &mut n.slide[*sub]; let last = sl.segments.len().saturating_sub(1);
                    let start_lane = sl.segments[..last].iter().flat_map(|s| &s.points).last().map(|p| p.zone.to_id()).unwrap_or(n.lane);
                    if let Some(seg) = sl.segments.last_mut() {
                        error = valid_segment(seg.shape, start_lane, cursor.zone).err();
                        if let Some(p) = seg.points.last_mut() { p.zone = PadZone::from(cursor.zone); }
                    } else { error = Some("Slide has no endpoint".into()); }
                }
            }
        },
    }
    if let Some(error) = error { txn.valid = false; editor.status = Some(error); }
}

pub fn transaction_key(editor: &mut EditorState, chart: &mut ChartDoc, key: egui::Key, mods: egui::Modifiers, _repeat: bool) {
    use egui::Key;
    if mods.command || mods.ctrl || mods.alt { return; }
    let Some(kind) = editor.keyboard.transaction.as_ref().map(|t| t.kind.clone()) else { return; };
    if key == Key::Escape {
        if let Draft::Star { id, sub, delay: false, mut accepted, new_note } = kind {
            let delay = accepted.pop().is_none();
            editor.keyboard.transaction.as_mut().unwrap().kind = Draft::Star { id, sub, delay, accepted, new_note };
            refresh(editor, chart);
        } else { cancel(editor, chart); }
        return;
    }
    if key == Key::Enter {
        refresh(editor, chart);
        if !editor.keyboard.transaction.as_ref().is_some_and(|t| t.valid) { return; }
        if let Draft::Star { id, sub, delay, accepted: _, new_note } = kind {
            if delay {
                editor.keyboard.transaction.as_mut().unwrap().kind = Draft::Star { id, sub, delay: false, accepted: Vec::new(), new_note };
                editor.pending_shape = Some(SlideShape::Line);
                editor.keyboard.cursor.measure += 1.0 / editor.snap_division.max(1) as f32;
                refresh(editor, chart); return;
            }
            if mods.shift {
                let accepted = chart.notes.iter().find(|n| n.id == id).unwrap().slide[sub].segments.clone();
                editor.keyboard.transaction.as_mut().unwrap().kind = Draft::Star { id, sub, delay: false, accepted, new_note };
                editor.pending_shape = Some(SlideShape::Line);
                editor.keyboard.cursor.measure += 1.0 / editor.snap_division.max(1) as f32;
                refresh(editor, chart); return;
            }
        }
        let continue_segment = matches!(kind, Draft::Segment { .. }) && mods.shift;
        commit(editor, chart);
        if continue_segment { begin_segment(editor, chart, true); }
        return;
    }
    let (dt, dl) = navigation::arrow(editor, key, mods.shift);
    if dt != 0.0 { editor.keyboard.cursor.measure = snap(editor.keyboard.cursor.measure + dt, editor.snap_enabled, editor.snap_division).max(1.0); }
    let endpoint_mode = matches!(kind, Draft::Star { delay: false, .. } | Draft::Segment { .. } | Draft::Handle { kind: Handle::Lane, .. });
    if endpoint_mode {
        if dl != 0 { editor.keyboard.cursor.zone = crate::timeline::geometry::rotated_zone(editor.keyboard.cursor.zone, dl); }
        if let Some(lane) = super::lane_key(key) { editor.keyboard.cursor.zone = lane; }
        if let Some(shape) = super::shape_key(key, mods.shift) {
            editor.pending_shape = Some(shape);
            if let Some(Transaction { kind: Draft::Segment { shape: s, .. }, .. }) = &mut editor.keyboard.transaction { *s = shape; }
        }
    }
    refresh(editor, chart);
}

pub fn begin_slide(editor: &mut EditorState, chart: &ChartDoc) -> bool {
    let Some(id) = editor.single_selected().filter(|id| chart.notes.iter().any(|n| n.id == *id && n.note_type == NoteType::Slide)) else { return false; };
    if editor.editing_slide == Some(id) { editor.editing_slide = None; return true; }
    editor.editing_slide = Some(id); editor.editing_slide_idx = 0; editor.keyboard.segment = 0;
    editor.pending_shape = None;
    true
}

fn begin_segment(editor: &mut EditorState, chart: &ChartDoc, append: bool) {
    let Some(n) = chart.notes.iter().find(|n| Some(n.id) == editor.editing_slide) else { return; };
    let sub = editor.editing_slide_idx; let Some(sl) = n.slide.get(sub) else { return; };
    let index = if append { sl.segments.len() } else { editor.keyboard.segment.min(sl.segments.len().saturating_sub(1)) };
    let shape = sl.segments.get(index).map(|s| s.shape).unwrap_or(SlideShape::Line);
    let lane = sl.segments.get(index).and_then(|s| s.points.last()).map(|p| p.zone.to_id()).unwrap_or(endpoint(sl, n.lane));
    let cursor = EditCursor { measure: n.time + sl.slide_duration, zone: lane, touch: false };
    start(editor, chart, Draft::Segment { id: n.id, sub, index, shape });
    editor.keyboard.cursor = cursor; editor.pending_shape = Some(shape); editor.keyboard.segment = index;
}

pub fn same_head(editor: &mut EditorState, chart: &mut ChartDoc) {
    let Some(n) = chart.notes.iter().find(|n| Some(n.id) == editor.editing_slide).cloned() else { return; };
    let sub = n.slide.len();
    start(editor, chart, Draft::Star { id: n.id, sub, delay: true, accepted: Vec::new(), new_note: false });
    chart.notes.iter_mut().find(|x| x.id == n.id).unwrap().slide.push(blank_slide());
    editor.editing_slide_idx = sub; editor.keyboard.cursor = navigation::note_cursor(&n); editor.pending_shape = Some(SlideShape::Line);
}

pub fn slide_key(editor: &mut EditorState, chart: &mut ChartDoc, key: egui::Key, mods: egui::Modifiers, repeat: bool) -> bool {
    use egui::Key;
    if mods.command || mods.ctrl {
        if mods.shift && key == Key::D && !repeat { same_head(editor, chart); return true; }
        if matches!(key, Key::ArrowLeft | Key::ArrowRight) {
            let count = chart.notes.iter().find(|n| Some(n.id) == editor.editing_slide).and_then(|n| n.slide.get(editor.editing_slide_idx)).map(|s| s.segments.len()).unwrap_or(0);
            editor.keyboard.segment = if key == Key::ArrowLeft { editor.keyboard.segment.saturating_sub(1) } else { (editor.keyboard.segment + 1).min(count.saturating_sub(1)) };
            return true;
        }
        return false;
    }
    if mods.alt { return false; }
    if !editor.keyboard.active {
        if let Some(shape) = super::shape_key(key, mods.shift) {
            editor.pending_shape = Some(shape);
            return true;
        }
        if let Some(lane) = super::lane_key(key) {
            if let Some(shape) = editor.pending_shape.take() { editor.apply_slide_shape(chart, shape, lane); }
            else { editor.append_slide_waypoint(chart, PadZone::from(lane)); }
            return true;
        }
    }
    if key == Key::Tab {
        let count = chart.notes.iter().find(|n| Some(n.id) == editor.editing_slide).map(|n| n.slide.len()).unwrap_or(0);
        if count > 0 { editor.editing_slide_idx = (editor.editing_slide_idx + if mods.shift { count - 1 } else { 1 }) % count; editor.keyboard.segment = 0; }
        return true;
    }
    if matches!(key, Key::Escape | Key::E | Key::Enter) { editor.editing_slide = None; editor.pending_shape = None; return true; }
    if key == Key::Delete { super::panels::open(editor, chart, super::panels::DialogKind::DeleteSlide); return true; }
    if key == Key::Backspace {
        editor.pop_slide_waypoint(chart);
        return true;
    }
    if super::shape_key(key, mods.shift).is_some() || super::lane_key(key).is_some() {
        begin_segment(editor, chart, false); transaction_key(editor, chart, key, mods, repeat); return true;
    }
    false
}

pub fn begin_handle(editor: &mut EditorState, chart: &ChartDoc, handle: Handle) {
    let target = navigation::target(editor, chart);
    let Some(n) = chart.notes.iter().find(|n| Some(n.id) == target) else { editor.status = Some("Point to one Hold or Star".into()); return; };
    let sub = editor.editing_slide_idx.min(n.slide.len().saturating_sub(1));
    let cursor = if matches!(n.note_type, NoteType::Hold | NoteType::TouchHold) {
        if handle != Handle::HoldTail { return; }
        EditCursor { measure: n.time + n.hold_duration, ..navigation::note_cursor(n) }
    } else if let Some(sl) = n.slide.get(sub) {
        EditCursor { measure: n.time + if handle == Handle::Delay { sl.slide_start_delay } else { sl.slide_duration }, zone: if handle == Handle::Delay { n.lane } else { endpoint(sl, n.lane) }, touch: false }
    } else { editor.status = Some("Star has no slide".into()); return; };
    if editor.keyboard.transaction.is_none() { start(editor, chart, Draft::Handle { id: n.id, sub, kind: handle }); }
    editor.keyboard.cursor = cursor; editor.keyboard.active = true;
    editor.keyboard.detached = true; editor.keyboard.focus = super::Focus::Timeline;
}

pub fn apply_transform(editor: &mut EditorState, chart: &mut ChartDoc, dt: f32, dl: i32, factor: f32) -> bool {
    let Some(t) = &editor.selection_transform else { return false; };
    let originals: Vec<_> = t.before.notes.iter().filter(|n| editor.selected.contains(&n.id)).collect();
    if originals.is_empty() || !dt.is_finite() || !factor.is_finite() || factor <= 0.0 { editor.status = Some("Invalid transform".into()); return false; }
    let min = originals.iter().map(|n| n.time).fold(f32::INFINITY, f32::min);
    let max = originals.iter().map(|n| n.time).fold(f32::NEG_INFINITY, f32::max);
    let center = (min + max) * 0.5;
    let mut candidates = Vec::new();
    for original in originals {
        let mut n = original.clone();
        let time = if t.kind == SelectionTransformKind::Scale { center + (original.time - center) * factor } else { original.time + dt };
        if !time.is_finite() || time < 1.0 { editor.status = Some("Entire transform rejected: before chart start".into()); return false; }
        let time = if t.kind == SelectionTransformKind::Scale { snap(time, editor.snap_enabled, editor.snap_division) } else { time };
        move_note(&mut n, time, dl);
        if t.kind == SelectionTransformKind::Scale { n.hold_duration *= factor; for sl in &mut n.slide { sl.slide_duration *= factor; sl.slide_start_delay *= factor; } }
        if !n.hold_duration.is_finite() || n.slide.iter().any(|s| !s.slide_duration.is_finite() || !s.slide_start_delay.is_finite()) { editor.status = Some("Entire transform rejected: invalid span".into()); return false; }
        candidates.push(n);
    }
    for n in candidates { if let Some(dst) = chart.notes.iter_mut().find(|x| x.id == n.id) { *dst = n; } }
    let t = editor.selection_transform.as_mut().unwrap();
    t.keyboard_driven = true; t.operation.time = dt; t.operation.lanes = dl; t.operation.factor = factor;
    editor.dirty = t.was_dirty || chart.notes != t.before.notes;
    true
}

pub fn numeric_transform(editor: &mut EditorState, chart: &mut ChartDoc) -> bool {
    let Some(t) = &editor.selection_transform else { return false; };
    let value = if editor.keyboard.numeric.is_empty() { 1.0 } else if let Ok(n) = editor.keyboard.numeric.parse::<f32>() { n } else { editor.status = Some("Enter a valid number".into()); return false; };
    match t.kind {
        SelectionTransformKind::Scale => apply_transform(editor, chart, 0.0, 0, value),
        SelectionTransformKind::Rotate => apply_transform(editor, chart, 0.0, value.round().clamp(-10000.0, 10000.0) as i32, 1.0),
        SelectionTransformKind::Grab => apply_transform(editor, chart, value, t.operation.lanes, 1.0),
    }
}

pub fn transform_key(editor: &mut EditorState, chart: &mut ChartDoc, key: egui::Key, mods: egui::Modifiers, _repeat: bool) {
    use egui::Key;
    if key == Key::Escape { crate::timeline::transform::finish(editor, chart, false); editor.keyboard.suppress_pointer = true; return; }
    if mods.any() && !mods.shift { return; }
    if key == Key::Enter {
        if !editor.keyboard.numeric.is_empty() && !numeric_transform(editor, chart) { return; }
        crate::timeline::transform::finish(editor, chart, true); editor.keyboard.suppress_pointer = true; return;
    }
    if key == Key::Backspace { editor.keyboard.numeric.pop(); numeric_transform(editor, chart); return; }
    let (dt, dl) = navigation::arrow(editor, key, mods.shift);
    if dt == 0.0 && dl == 0 { return; }
    editor.keyboard.numeric.clear();
    let t = editor.selection_transform.as_ref().unwrap(); let op = t.operation;
    match t.kind {
        SelectionTransformKind::Grab => { apply_transform(editor, chart, op.time + dt, op.lanes + dl, 1.0); }
        SelectionTransformKind::Rotate => { apply_transform(editor, chart, 0.0, op.lanes + dl, 1.0); }
        SelectionTransformKind::Scale => if dt != 0.0 { apply_transform(editor, chart, 0.0, 0, op.factor + dt.signum() * 0.1); },
    }
}

pub fn repeat(editor: &mut EditorState, chart: &mut ChartDoc) {
    if let Some(op) = editor.last_edit {
        let before = chart.clone(); let selection = editor.selected.clone();
        if op.duplicate && !editor.duplicate_selection(chart) { return; }
        crate::timeline::transform::begin(editor, chart, op.kind, egui::Pos2::ZERO);
        if !apply_transform(editor, chart, op.time, op.lanes, op.factor) {
            crate::timeline::transform::finish(editor, chart, false);
            if op.duplicate { *chart = before; editor.selected = selection; }
            return;
        }
        if op.duplicate { let t = editor.selection_transform.as_mut().unwrap(); t.duplicate_before = Some(before); t.operation.duplicate = true; }
        crate::timeline::transform::finish(editor, chart, true);
    } else { editor.repeat_placement(chart); }
}

pub fn paste(editor: &mut EditorState, chart: &mut ChartDoc) {
    let Some(first) = editor.clipboard.iter().min_by(|a,b| a.time.total_cmp(&b.time)).cloned() else { return; };
    let cursor = editor.keyboard.cursor;
    if navigation::note_cursor(&first).touch != cursor.touch || (cursor.touch && ring(first.lane) != ring(cursor.zone)) { editor.status = Some("Paste requires the same lane / Touch ring".into()); return; }
    let delta = cursor.measure - first.time; let dl = cursor.zone as i32 - first.lane as i32;
    let mut notes = editor.clipboard.clone(); let mut id = editor.next_note_id(chart);
    for n in &mut notes { let time = n.time + delta; if time < 1.0 { return; } move_note(n, time, dl); n.id = id; id += 1; }
    editor.push_history(chart); editor.selected = notes.iter().map(|n| n.id).collect(); chart.notes.extend(notes);
}

fn ring(zone: u8) -> u8 { match zone { 1..=8 => 0, 9..=16 => 1, 17 => 2, 18..=25 => 3, _ => 4 } }
