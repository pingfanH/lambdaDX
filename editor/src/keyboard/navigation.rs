//! Shared timeline-space cursor navigation and selection.

use egui_macroquad::egui;
use crate::app::types::{ChartDoc, Note, measure_to_secs, secs_to_measure};
use crate::state::{EditorLayout, EditorState, Tool};
use crate::timeline::geometry::{JUDGE_MARGIN, LANE_COUNT, LANE_GUTTER, RULER_H, snap};
use super::EditCursor;

pub fn note_cursor(note: &Note) -> EditCursor {
    EditCursor { measure: note.time, zone: note.lane, touch: note.is_touch || matches!(note.note_type, crate::app::types::NoteType::Touch | crate::app::types::NoteType::TouchHold) }
}

pub fn pointer_anchor(editor: &mut EditorState, chart: &ChartDoc, time: f32, lane: u8, hit: Option<u64>) {
    editor.keyboard.initialized = true;
    editor.keyboard.candidate = hit;
    editor.keyboard.cursor = if let Some(note) = hit.and_then(|id| chart.notes.iter().find(|n| n.id == id)) {
        note_cursor(note)
    } else {
        let touch = lane > 8;
        let zone = if !touch { lane.clamp(1, 8) }
            else if editor.keyboard.cursor.touch { editor.keyboard.cursor.zone }
            else { match editor.keyboard.touch_ring { 0 => 1, 1 => 9, 2 => 17, 3 => 18, _ => 26 } };
        EditCursor {
            measure: snap(secs_to_measure(time, &chart.bpms), editor.snap_enabled, editor.snap_division).max(1.0),
            zone,
            touch,
        }
    };
}

pub fn reference_time(editor: &EditorState, now: f32) -> f32 {
    if editor.keyboard.detached { editor.view_start } else { now }
}

pub fn position(editor: &EditorState, chart: &ChartDoc, now: f32, cursor: EditCursor) -> egui::Pos2 {
    let rect = editor.keyboard.rect;
    let seconds = measure_to_secs(cursor.measure, &chart.bpms);
    let lane = if cursor.touch { 8.5 } else { cursor.zone.clamp(1, 8) as f32 - 0.5 };
    match editor.layout {
        EditorLayout::Vertical => egui::pos2(
            rect.left() + LANE_GUTTER + lane * (rect.width() - LANE_GUTTER) / LANE_COUNT as f32,
            rect.bottom() - JUDGE_MARGIN - (seconds - reference_time(editor, now)) * editor.pixels_per_second,
        ),
        EditorLayout::Horizontal => egui::pos2(
            rect.left() + LANE_GUTTER + (seconds - editor.view_start) * editor.pixels_per_second,
            rect.top() + RULER_H + lane * (rect.height() - RULER_H) / LANE_COUNT as f32,
        ),
    }
}

pub fn page_seconds(editor: &EditorState) -> f32 {
    let pixels = match editor.layout {
        EditorLayout::Vertical => editor.keyboard.rect.height() - RULER_H - JUDGE_MARGIN,
        EditorLayout::Horizontal => editor.keyboard.rect.width() - LANE_GUTTER,
    };
    pixels.max(1.0) / editor.pixels_per_second.max(1.0)
}

pub fn arrow(editor: &EditorState, key: egui::Key, shift: bool) -> (f32, i32) {
    use egui::Key;
    let step = if shift { 1.0 } else { 1.0 / editor.snap_division.max(1) as f32 };
    let (earlier, later, prev, next) = match editor.layout {
        EditorLayout::Vertical => (Key::ArrowDown, Key::ArrowUp, Key::ArrowLeft, Key::ArrowRight),
        EditorLayout::Horizontal => (Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown),
    };
    if key == earlier { (-step, 0) } else if key == later { (step, 0) }
    else if key == prev { (0.0, -1) } else if key == next { (0.0, 1) } else { (0.0, 0) }
}

pub fn set_lane(editor: &mut EditorState, lane: u8) {
    let cursor = &mut editor.keyboard.cursor;
    cursor.touch = matches!(editor.tool, Tool::Touch | Tool::TouchHold);
    cursor.zone = if cursor.touch {
        match editor.keyboard.touch_ring { 0 => lane, 1 => lane + 8, 2 => 17, 3 => lane + 17, _ => lane + 25 }
    } else { lane };
    editor.keyboard.candidate = None;
}

pub fn follow_cursor(editor: &mut EditorState, chart: &ChartDoc) {
    let time = measure_to_secs(editor.keyboard.cursor.measure, &chart.bpms);
    // A diagonal or several queued key events need only one native seek.
    if let Some(super::Transport::Cursor(pending)) = editor.keyboard.transport.last_mut() {
        *pending = time;
    } else {
        editor.keyboard.transport.push(super::Transport::Cursor(time));
    }
}

pub fn cursor_mode(editor: &EditorState) -> bool {
    editor.keyboard.active && editor.keyboard.focus == super::Focus::Timeline
}

pub fn target(editor: &EditorState, chart: &ChartDoc) -> Option<u64> {
    if cursor_mode(editor) { candidate(editor, chart, 0.0) }
    else { editor.single_selected() }
}

pub fn navigate(editor: &mut EditorState, chart: &ChartDoc, now: f32, key: egui::Key, shift: bool) {
    let (dt, dl) = arrow(editor, key, shift);
    let current = editor.keyboard.cursor;
    let measure = match key {
        egui::Key::Home => 1.0,
        egui::Key::End => chart.notes.iter().map(|n| n.time + n.hold_duration.max(n.slide.iter().map(|s| s.slide_duration).fold(0.0, f32::max))).fold(1.0, f32::max),
        egui::Key::PageUp | egui::Key::PageDown => {
            let sign = if key == egui::Key::PageUp { 1.0 } else { -1.0 };
            secs_to_measure(measure_to_secs(current.measure, &chart.bpms) + sign * page_seconds(editor), &chart.bpms)
        }
        _ => current.measure + dt,
    };
    editor.keyboard.cursor.measure = snap(measure, editor.snap_enabled, editor.snap_division).max(1.0);
    if dl != 0 {
        editor.keyboard.cursor.zone = crate::timeline::geometry::rotated_zone(current.zone, dl * if shift { 4 } else { 1 });
    }
    editor.keyboard.candidate = None;
    follow_cursor(editor, chart);
    ensure_visible(editor, chart, now);
}

pub fn repeat_held(editor: &mut EditorState, chart: &ChartDoc, held: &[egui::Key], shift: bool) {
    use egui::Key;
    for (negative, positive) in [(Key::ArrowLeft, Key::ArrowRight), (Key::ArrowDown, Key::ArrowUp)] {
        match (held.contains(&negative), held.contains(&positive)) {
            (true, false) => navigate(editor, chart, 0.0, negative, shift),
            (false, true) => navigate(editor, chart, 0.0, positive, shift),
            _ => {}
        }
    }
}

pub fn ensure_visible(editor: &mut EditorState, chart: &ChartDoc, now: f32) {
    if !editor.keyboard.rect.is_positive() { return; }
    let t = measure_to_secs(editor.keyboard.cursor.measure, &chart.bpms);
    let pps = editor.pixels_per_second.max(1.0);
    let start = match editor.layout { EditorLayout::Vertical => reference_time(editor, now), EditorLayout::Horizontal => editor.view_start };
    // The vertical plot includes the strip below the judgment line.
    let first = start - if editor.layout == EditorLayout::Vertical { JUDGE_MARGIN / pps } else { 0.0 };
    let last = start + page_seconds(editor);
    let tolerance = 0.5 / pps;
    if t < first - tolerance { editor.view_start = (start + t - first).max(0.0); }
    else if t > last + tolerance { editor.view_start = (start + t - last).max(0.0); }
}

fn markers(editor: &EditorState, chart: &ChartDoc, now: f32) -> Vec<(u64, egui::Rect)> {
    chart.notes.iter().map(|note| {
        let center = position(editor, chart, now, note_cursor(note));
        (note.id, crate::timeline::input::head_rect(editor, note, center))
    }).collect()
}

pub fn candidate(editor: &EditorState, chart: &ChartDoc, now: f32) -> Option<u64> {
    if let Some(id) = editor.keyboard.candidate.filter(|id| chart.notes.iter().any(|n| n.id == *id)) { return Some(id); }
    let at = position(editor, chart, now, editor.keyboard.cursor);
    markers(editor, chart, now).into_iter().filter(|(_, rect)| rect.contains(at))
        .min_by(|(a, ar), (b, br)| ar.center().distance_sq(at).total_cmp(&br.center().distance_sq(at)).then_with(|| a.cmp(b)))
        .map(|(id, _)| id)
}

pub fn select(editor: &mut EditorState, chart: &ChartDoc, now: f32, additive: bool) {
    if let Some(id) = candidate(editor, chart, now) {
        editor.keyboard.candidate = Some(id);
        if additive { if !editor.selected.remove(&id) { editor.selected.insert(id); } }
        else { editor.select_only(id); }
    } else { editor.status = Some("No note at edit cursor".into()); }
}

pub fn cycle(editor: &mut EditorState, chart: &ChartDoc, reverse: bool) {
    let mut notes: Vec<_> = chart.notes.iter().collect();
    notes.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.lane.cmp(&b.lane)).then(a.id.cmp(&b.id)));
    if notes.is_empty() { return; }
    let cursor = editor.keyboard.cursor;
    let current = editor.keyboard.candidate.and_then(|id| notes.iter().position(|n| n.id == id && note_cursor(n) == cursor))
        .or_else(|| notes.iter().position(|n| note_cursor(n) == cursor));
    let index = match current {
        Some(i) if reverse => (i + notes.len() - 1) % notes.len(), Some(i) => (i + 1) % notes.len(),
        None if reverse => notes.iter().rposition(|n| n.time < cursor.measure || (n.time == cursor.measure && n.lane <= cursor.zone)).unwrap_or(notes.len() - 1),
        None => notes.iter().position(|n| n.time > cursor.measure || (n.time == cursor.measure && n.lane >= cursor.zone)).unwrap_or(0),
    };
    editor.keyboard.cursor = note_cursor(notes[index]);
    editor.keyboard.candidate = Some(notes[index].id);
    follow_cursor(editor, chart);
}

pub fn range(editor: &mut EditorState, chart: &ChartDoc, now: f32, additive: bool) {
    if let Some((anchor, add)) = editor.keyboard.range.take() {
        let rect = egui::Rect::from_two_pos(position(editor, chart, now, anchor), position(editor, chart, now, editor.keyboard.cursor));
        if !add && !additive { editor.selected.clear(); }
        for (id, bounds) in markers(editor, chart, now) { if rect.intersects(bounds) { editor.selected.insert(id); } }
    } else { editor.keyboard.range = Some((editor.keyboard.cursor, additive)); }
}

pub fn division(editor: &mut EditorState, next: bool) {
    const PRESETS: &[u32] = &[1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256];
    editor.snap_division = if next { PRESETS.iter().copied().find(|n| *n > editor.snap_division).unwrap_or(256) }
        else { PRESETS.iter().copied().rev().find(|n| *n < editor.snap_division).unwrap_or(1) };
}

pub fn draw(ui: &egui::Ui, editor: &EditorState, chart: &ChartDoc, now: f32) {
    if !editor.keyboard.active || editor.keyboard.focus != super::Focus::Timeline { return; }
    let painter = ui.painter().with_clip_rect(editor.keyboard.rect);
    let p = position(editor, chart, now, editor.keyboard.cursor);
    let color = egui::Color32::from_rgb(110, 229, 174);
    painter.line_segment([p - egui::vec2(8.0, 0.0), p + egui::vec2(8.0, 0.0)], egui::Stroke::new(1.5_f32, color));
    painter.line_segment([p - egui::vec2(0.0, 8.0), p + egui::vec2(0.0, 8.0)], egui::Stroke::new(1.5_f32, color));
    if let Some(id) = candidate(editor, chart, now) {
        if let Some((_, rect)) = markers(editor, chart, now).into_iter().find(|(n, _)| *n == id) {
            painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.5_f32, color), egui::StrokeKind::Inside);
        }
    }
    if let Some((anchor, _)) = editor.keyboard.range {
        let rect = egui::Rect::from_two_pos(position(editor, chart, now, anchor), p);
        crate::timeline::geometry::paint_marquee(ui, rect.intersect(editor.keyboard.rect));
    }
}
