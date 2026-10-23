//! Orientation-independent timeline geometry + selection helpers.
//!
//! Both the horizontal and vertical timelines share the lane model (8 play
//! lanes + one touch lane) and the snapping/drag maths; only the axis mapping
//! differs, so that mapping lives in the two renderer modules.

use egui_macroquad::egui;

use crate::app::types::{Note, NoteType};
use crate::state::{DragGrab, EditorLayout, EditorState};

/// Play lanes (1..=8) plus the dedicated touch lane, rendered last.
pub const LANE_COUNT: usize = 9;
/// Gutter that holds the lane labels (`1..8`, `T`).
pub const LANE_GUTTER: f32 = 30.0;
/// Height/width of the measure-number ruler.
pub const RULER_H: f32 = 22.0;
/// Gap between the judgment line and the near edge (vertical layout only).
pub const JUDGE_MARGIN: f32 = 38.0;
/// Shared time scale for chart navigation and the Autoplay workspace.
pub const MIN_PPS: f32 = 10.0;
pub const MAX_PPS: f32 = 4800.0;
pub const DEFAULT_PPS: f32 = 360.0;

/// Keep the usual two-second lead, capped at half this pane's visible span.
pub fn default_view_reference(editor: &EditorState, playhead: f32) -> f32 {
    if editor.layout == EditorLayout::Vertical { return playhead; }
    let width = editor.keyboard.rect.width() - LANE_GUTTER;
    let lead = if width.is_finite() && width > 0.0 {
        2.0_f32.min(width * 0.5 / editor.pixels_per_second.max(1.0))
    } else {
        2.0
    };
    (playhead - lead).max(playhead.min(0.0))
}

pub fn playhead_in_view(editor: &EditorState, reference: f32, playhead: f32) -> bool {
    let rect = editor.keyboard.rect;
    if !rect.is_positive() || !rect.is_finite() { return true; }
    let offset = (playhead - reference) * editor.pixels_per_second.max(1.0);
    match editor.layout {
        EditorLayout::Vertical => {
            let y = rect.bottom() - JUDGE_MARGIN - offset;
            y >= rect.top() + RULER_H && y <= rect.bottom()
        }
        EditorLayout::Horizontal => offset >= 0.0 && offset <= rect.width() - LANE_GUTTER,
    }
}

pub fn view_reference(editor: &EditorState, playhead: f32) -> f32 {
    // Dragging the line owns the view until release; overview seeking still follows.
    if !editor.follow_playhead
        || editor.keyboard.detached
        || editor.follow_zoom_detached
        || editor.scrubbing && editor.pointer_press.is_some_and(|press| press.on_playhead)
    {
        return editor.view_start;
    }
    let playhead = editor.pending_seek.unwrap_or(playhead);
    editor.follow_view_offset.map_or_else(
        || default_view_reference(editor, playhead),
        |offset| playhead - offset,
    )
}

pub fn set_view_reference(editor: &mut EditorState, time: f32, playhead: f32) {
    editor.view_start = time;
    editor.follow_zoom_detached = false;
    if editor.follow_playhead && !editor.keyboard.detached {
        editor.follow_view_offset = Some(editor.pending_seek.unwrap_or(playhead) - time);
    }
}

/// Preserve a pointer-anchored zoom without replacing the user's follow offset.
pub fn set_zoom_view_reference(editor: &mut EditorState, time: f32) {
    editor.view_start = time;
    if editor.follow_playhead && !editor.keyboard.detached {
        editor.follow_zoom_detached = true;
    }
}

/// Preserve the playhead's screen position, including pre-roll exposed by zooming out.
pub fn set_zoom(editor: &mut EditorState, pps: f32, playhead: f32, horizontal: bool) {
    let pps = pps.clamp(MIN_PPS, MAX_PPS);
    if horizontal {
        let view = playhead - (playhead - editor.view_start) * editor.pixels_per_second / pps;
        set_zoom_view_reference(editor, view);
    }
    editor.pixels_per_second = pps;
}

/// Map a row/column offset to its one-based timeline lane (including Touch).
pub fn lane_at(offset: f32, size: f32) -> u8 {
    ((offset / size.max(f32::EPSILON)).floor() + 1.0).clamp(1.0, LANE_COUNT as f32) as u8
}

/// Lane row/column a note renders in. Touch notes share the trailing lane.
pub fn lane_index_of(note: &Note) -> usize {
    if note.is_touch || matches!(note.note_type, NoteType::Touch | NoteType::TouchHold) {
        LANE_COUNT - 1
    } else {
        (note.lane.saturating_sub(1) as usize).min(LANE_COUNT - 2)
    }
}

/// Lane row/column for a Simai sensor zone id (1..=8 → 0..=7).
pub fn zone_lane_index(zone_id: u8) -> usize {
    (zone_id.saturating_sub(1) as usize).min(LANE_COUNT - 2)
}

/// Snap a raw measure to the editor grid (`division` subdivisions per measure,
/// e.g. 16 = 16th notes) unless snapping is disabled.
pub fn snap(measure: f32, snap_enabled: bool, division: u32) -> f32 {
    if !snap_enabled || division == 0 {
        return measure;
    }
    let d = division as f32;
    (measure * d).round() / d
}

/// Rotate within a sensor ring; the centre never moves to a numbered sensor.
pub fn rotated_zone(zone: u8, delta: i32) -> u8 {
    let base = match zone {
        1..=8 => 1,
        9..=16 => 9,
        18..=25 => 18,
        26..=33 => 26,
        _ => return zone,
    };
    ((zone as i32 - base + delta).rem_euclid(8) + base) as u8
}

/// Move a head and its trajectory together, preserving all other properties.
pub fn move_note(note: &mut Note, time: f32, lane_delta: i32) {
    note.time = time;
    note.lane = rotated_zone(note.lane, lane_delta);
    for slide in &mut note.slide {
        for segment in &mut slide.segments {
            for point in &mut segment.points {
                point.zone = crate::app::types::zone::PadZone::from(rotated_zone(
                    point.zone.to_id(),
                    lane_delta,
                ));
            }
        }
    }
}

/// New (time, lane) for a dragged note, preserving the grab offset captured at
/// drag start so the note never jumps a row/time on the first frame.
pub fn dragged_position(g: &DragGrab, cur_measure: f32, cur_lane: i32, division: u32) -> (f32, u8) {
    (
        snap(g.note_time + (cur_measure - g.grab_measure), true, division).max(1.0),
        (g.note_lane as i32 + (cur_lane - g.grab_lane)).clamp(1, 8) as u8,
    )
}

/// Select a single note, or add it to the current set when `additive` (Shift).
///
/// Mirrors the original editor: a plain click replaces the selection and Shift
/// adds to it (adding an already-selected note is a no-op rather than a toggle,
/// so a Shift-clicked note can't be accidentally dropped from a group).
pub fn select_note(editor: &mut EditorState, id: u64, additive: bool) {
    if additive {
        editor.selected.insert(id);
    } else {
        editor.select_only(id);
    }
}

/// Translucent box-select rectangle drawn over the timeline (egui overlay).
pub fn paint_marquee(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(56, 189, 248, 40),
    );
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(56, 189, 248)),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_zero_playhead_stays_visible_in_both_layouts() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut editor = EditorState { layout, ..Default::default() };
            editor.keyboard.rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO, egui::vec2(640.0, 480.0),
            );
            for time in [-2.0, -1.0, -0.01, 0.0] {
                crate::runtime::follow_playhead_during_playback(&mut editor, time, true);
                assert!(playhead_in_view(&editor, editor.view_start, time), "{layout:?}: {time}");
            }
        }
    }

    #[test]
    fn requested_timeline_zoom_keeps_red_line_fixed() {
        let mut editor = EditorState::default();
        editor.view_start = 0.0;
        editor.pixels_per_second = 360.0;
        let playhead = 0.5;
        let offset = (playhead - editor.view_start) * editor.pixels_per_second;
        for pps in [120.0, MIN_PPS, MAX_PPS, 360.0] {
            set_zoom(&mut editor, pps, playhead, true);
            assert!(
                ((playhead - editor.view_start) * editor.pixels_per_second - offset).abs() < 0.01
            );
        }
        let view = editor.view_start;
        set_zoom(&mut editor, 120.0, playhead, false);
        assert_eq!(editor.view_start, view);
    }

    #[test]
    fn zoom_detaches_follow_until_playback_resumes() {
        let mut editor = EditorState {
            layout: EditorLayout::Horizontal,
            follow_playhead: true,
            ..Default::default()
        };
        crate::runtime::follow_playhead(&mut editor, 20.0);
        assert_eq!(editor.view_start, 18.0);

        set_zoom_view_reference(&mut editor, 19.25);
        assert!(editor.follow_zoom_detached);
        crate::runtime::follow_playhead(&mut editor, 22.0);
        assert_eq!(editor.view_start, 19.25, "paused zoom remains anchored");

        crate::runtime::follow_playhead_during_playback(&mut editor, 22.0, true);
        assert!(!editor.follow_zoom_detached);
        assert_eq!(
            editor.view_start, 20.0,
            "playback restores the default follow position"
        );
    }

    #[test]
    fn playback_recovers_an_offscreen_playhead_after_view_navigation() {
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            for offset in [-40.0, 40.0] {
                for zoomed in [false, true] {
                    let mut editor = EditorState {
                        layout, follow_playhead: true, pixels_per_second: 240.0,
                        follow_view_offset: Some(offset), follow_zoom_detached: zoomed,
                        view_start: 20.0 - offset, ..Default::default()
                    };
                    editor.keyboard.rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(640.0, 480.0));
                    crate::runtime::follow_playhead_during_playback(&mut editor, 20.0, false);
                    assert_eq!(editor.view_start, 20.0 - offset, "paused browsing must stay put");
                    crate::runtime::follow_playhead_during_playback(&mut editor, 20.0, true);
                    let reference = view_reference(&editor, 20.0);
                    let coordinate = if layout == EditorLayout::Vertical {
                        480.0 - JUDGE_MARGIN - (20.0 - reference) * editor.pixels_per_second
                    } else {
                        LANE_GUTTER + (20.0 - reference) * editor.pixels_per_second
                    };
                    let (lo, hi) = if layout == EditorLayout::Vertical { (RULER_H, 480.0) } else { (LANE_GUTTER, 640.0) };
                    assert!(coordinate >= lo && coordinate <= hi, "playhead must return into view: {layout:?}, offset={offset}, zoomed={zoomed}, coordinate={coordinate}");
                    assert!(!editor.follow_zoom_detached);
                }
            }
        }
    }

    #[test]
    fn playback_follow_stays_visible_at_high_zoom_in_a_small_pane() {
        let mut editor = EditorState { layout: EditorLayout::Horizontal, pixels_per_second: MAX_PPS, ..Default::default() };
        editor.keyboard.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 480.0));
        crate::runtime::follow_playhead_during_playback(&mut editor, 20.0, true);
        let x = LANE_GUTTER + (20.0 - editor.view_start) * editor.pixels_per_second;
        assert!(x >= LANE_GUTTER && x <= 320.0, "high zoom must not put the playhead outside the pane: {x}");
    }

    #[test]
    fn playback_follow_does_not_interrupt_an_active_scrub_or_disabled_follow() {
        for disabled in [false, true] {
            let mut editor = EditorState {
                layout: EditorLayout::Horizontal, view_start: 50.0, follow_playhead: !disabled,
                follow_view_offset: Some(-30.0), follow_zoom_detached: true, scrubbing: !disabled,
                ..Default::default()
            };
            crate::runtime::follow_playhead_during_playback(&mut editor, 20.0, true);
            assert_eq!(editor.view_start, 50.0);
            assert!(editor.follow_zoom_detached);
        }
    }

    #[test]
    fn pointer_lanes_match_rendered_centers_and_edges() {
        for lane in 1..=9 {
            assert_eq!(lane_at((lane as f32 - 0.5) * 40.0, 40.0), lane);
            assert_eq!(lane_at((lane as f32 - 1.0) * 40.0, 40.0), lane);
        }
        assert_eq!(lane_at(-40.0, 40.0), 1);
        assert_eq!(lane_at(400.0, 40.0), 9);
    }

    #[test]
    fn zone_rotation_wraps_every_ring_and_keeps_center() {
        for (start, end) in [(1, 8), (9, 16), (18, 25), (26, 33)] {
            assert_eq!(rotated_zone(end, 1), start);
            assert_eq!(rotated_zone(start, -1), end);
            assert_eq!(rotated_zone(start, 16), start);
        }
        assert_eq!(rotated_zone(17, 3), 17);
    }

    #[test]
    fn select_note_replaces_or_appends() {
        let mut editor = EditorState::default();
        select_note(&mut editor, 1, false);
        assert_eq!(editor.selected.len(), 1);
        select_note(&mut editor, 2, false);
        assert_eq!(editor.selected.len(), 1, "a plain click replaces");
        assert!(editor.selected.contains(&2));
        select_note(&mut editor, 3, true);
        assert_eq!(editor.selected.len(), 2, "Shift adds to the selection");
        assert!(editor.selected.contains(&2) && editor.selected.contains(&3));
        select_note(&mut editor, 3, true);
        assert_eq!(
            editor.selected.len(),
            2,
            "re-adding is a no-op, not a toggle"
        );
    }

    #[test]
    fn drag_preserves_grab_offset() {
        // Grabbed lane 3 with the cursor between lanes 3 and 4 (cursor lane 3):
        // the note stays in lane 3 until the cursor crosses a row.
        let g = DragGrab {
            note_time: 5.0,
            note_lane: 3,
            grab_measure: 5.25,
            grab_lane: 3,
        };
        assert_eq!(dragged_position(&g, 5.25, 3, 16).1, 3, "no jump on grab");
        assert_eq!(dragged_position(&g, 5.25, 1, 16).1, 1, "two rows up");
        assert_eq!(dragged_position(&g, 5.25, 4, 16).1, 4, "one row down");
        let (t, _) = dragged_position(&g, 5.75, 3, 16);
        assert!((t - 5.5).abs() < 1e-3, "delta +0.5 measure, snapped");
        assert_eq!(dragged_position(&g, 5.25, 99, 16).1, 8);
        assert_eq!(dragged_position(&g, 5.25, -99, 16).1, 1);
    }
}
