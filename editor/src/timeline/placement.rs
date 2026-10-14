//! Snapped placement ghosts and pending star geometry for both timeline layouts.

use super::geometry::{
    JUDGE_MARGIN, LANE_COUNT, LANE_GUTTER, RULER_H, lane_at, lane_index_of, snap,
};
use crate::app::types::{ChartDoc, Note, measure_to_secs, secs_to_measure};
use crate::state::{EditorLayout, EditorState, Tool};
use egui_macroquad::egui;

pub fn note_for_tool(editor: &EditorState, time: f32, lane: u8) -> Note {
    Note {
        time,
        lane: lane.clamp(1, 8),
        note_type: editor.selected_type(),
        is_touch: matches!(editor.tool, Tool::Touch | Tool::TouchHold),
        is_break: editor.tool == Tool::Break,
        ..Default::default()
    }
}

pub fn ghost(
    rect: egui::Rect,
    editor: &EditorState,
    chart: &ChartDoc,
    now: f32,
    pointer: egui::Pos2,
) -> Option<(Note, egui::Pos2)> {
    let plot = egui::Rect::from_min_max(rect.min + egui::vec2(LANE_GUTTER, RULER_H), rect.max);
    if !plot.contains(pointer)
        || editor.selection_transform.is_some()
        || editor.transform_pointer_down
        || editor.star_placement.is_some()
        || editor.dragging.is_some()
        || editor.handle_grab.is_some()
        || editor.marquee_start.is_some()
        || editor.scrubbing
        || editor.is_slide_edit()
        || matches!(editor.tool, Tool::Select | Tool::Eraser)
    {
        return None;
    }
    let (seconds, lane) = match editor.layout {
        EditorLayout::Vertical => (
            now + (rect.bottom() - JUDGE_MARGIN - pointer.y) / editor.pixels_per_second,
            lane_at(pointer.x - plot.left(), plot.width() / LANE_COUNT as f32),
        ),
        EditorLayout::Horizontal => (
            editor.view_start + (pointer.x - plot.left()) / editor.pixels_per_second,
            lane_at(pointer.y - plot.top(), plot.height() / LANE_COUNT as f32),
        ),
    };
    let measure = snap(
        secs_to_measure(seconds, &chart.bpms),
        editor.snap_enabled,
        editor.snap_division,
    )
    .max(1.0);
    let note = note_for_tool(editor, measure, lane);
    let time = measure_to_secs(measure, &chart.bpms);
    let lane = lane_index_of(&note) as f32 + 0.5;
    let position = match editor.layout {
        EditorLayout::Vertical => egui::pos2(
            plot.left() + lane * plot.width() / LANE_COUNT as f32,
            rect.bottom() - JUDGE_MARGIN - (time - now) * editor.pixels_per_second,
        ),
        EditorLayout::Horizontal => egui::pos2(
            plot.left() + (time - editor.view_start) * editor.pixels_per_second,
            plot.top() + lane * plot.height() / LANE_COUNT as f32,
        ),
    };
    Some((note, position))
}

pub fn draw(
    ui: &egui::Ui,
    rect: egui::Rect,
    editor: &EditorState,
    chart: &ChartDoc,
    now: f32,
    textures: &[Option<egui::TextureHandle>; 6],
) {
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    if let Some((note, position)) = ghost(rect, editor, chart, now, pointer) {
        let painter = ui.painter().with_clip_rect(rect);
        let size = super::notes::head_size(&note)
            * editor.glyph_to_points
            * crate::app::params::get().editor_note_scale.clamp(0.05, 8.0);
        let index = match editor.tool {
            Tool::Hold | Tool::TouchHold => 1,
            Tool::Slide => 2,
            Tool::Break => 5,
            _ => 0,
        };
        let tint = egui::Color32::from_white_alpha(120);
        if editor.tool == Tool::Touch {
            if let Some(texture) = &textures[3] {
                let width = size * super::notes::TOUCH_TRI / super::notes::head_size(&note);
                let height = width / texture.aspect_ratio();
                for angle in [
                    0.0,
                    std::f32::consts::PI,
                    std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2,
                ] {
                    let mut mesh = egui::Mesh::with_texture(texture.id());
                    mesh.add_rect_with_uv(
                        egui::Rect::from_center_size(
                            position + egui::vec2(0.0, width * 0.3),
                            egui::vec2(width, height),
                        ),
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        tint,
                    );
                    for vertex in &mut mesh.vertices {
                        vertex.pos = position
                            + egui::emath::Rot2::from_angle(angle) * (vertex.pos - position);
                    }
                    painter.add(egui::Shape::mesh(mesh));
                }
            }
            if let Some(texture) = &textures[4] {
                let width = size * super::notes::TOUCH_POINT / super::notes::head_size(&note);
                painter.image(
                    texture.id(),
                    egui::Rect::from_center_size(position, egui::vec2(width, width)),
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    tint,
                );
            } else {
                painter.circle_stroke(position, size * 0.5, egui::Stroke::new(2.0_f32, tint));
            }
        } else if let Some(texture) = textures[index].as_ref().or(textures[0].as_ref()) {
            painter.image(
                texture.id(),
                egui::Rect::from_center_size(position, egui::vec2(size, size)),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                tint,
            );
        } else {
            painter.circle_stroke(position, size * 0.5, egui::Stroke::new(2.0_f32, tint));
        }
    }
    let Some(pending) = editor.star_placement else {
        return;
    };
    if !rect.contains(pointer) {
        return;
    }
    let pointer_time = match editor.layout {
        EditorLayout::Vertical => {
            now + (rect.bottom() - JUDGE_MARGIN - pointer.y) / editor.pixels_per_second
        }
        EditorLayout::Horizontal => {
            editor.view_start + (pointer.x - rect.left() - LANE_GUTTER) / editor.pixels_per_second
        }
    };
    let cursor = snap(
        secs_to_measure(pointer_time, &chart.bpms),
        editor.snap_enabled,
        editor.snap_division,
    );
    let position = |measure| {
        let time = measure_to_secs(measure, &chart.bpms);
        match editor.layout {
            EditorLayout::Vertical => egui::pos2(
                rect.left()
                    + LANE_GUTTER
                    + (pending.lane as f32 - 0.5) * (rect.width() - LANE_GUTTER)
                        / LANE_COUNT as f32,
                rect.bottom() - JUDGE_MARGIN - (time - now) * editor.pixels_per_second,
            ),
            EditorLayout::Horizontal => egui::pos2(
                rect.left() + LANE_GUTTER + (time - editor.view_start) * editor.pixels_per_second,
                rect.top()
                    + RULER_H
                    + (pending.lane as f32 - 0.5) * (rect.height() - RULER_H) / LANE_COUNT as f32,
            ),
        }
    };
    let painter = ui.painter().with_clip_rect(rect);
    let head = position(pending.head);
    let delay = position(pending.delay_end.unwrap_or(cursor.max(pending.head)));
    let stroke = egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(93, 201, 246));
    let delta = delay - head;
    let length = delta.length();
    if length > 0.0 {
        let direction = delta / length;
        for i in 0..(length / 10.0).ceil() as usize {
            let start = i as f32 * 10.0;
            painter.line_segment(
                [
                    head + direction * start,
                    head + direction * (start + 5.0).min(length),
                ],
                stroke,
            );
        }
    }
    if let Some(delay_end) = pending.delay_end {
        let tail = position(cursor.max(delay_end));
        painter.line_segment(
            [delay, tail],
            egui::Stroke::new(
                8.0_f32,
                egui::Color32::from_rgba_unmultiplied(73, 219, 157, 180),
            ),
        );
    }
    let size = super::notes::STAR_SIZE
        * editor.glyph_to_points
        * crate::app::params::get().editor_note_scale.clamp(0.05, 8.0);
    if let Some(texture) = &textures[2] {
        painter.image(
            texture.id(),
            egui::Rect::from_center_size(head, egui::vec2(size, size)),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.circle_stroke(head, size * 0.5, stroke);
    }
    painter.circle_filled(delay, 4.0, stroke.color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_matches_snapped_placement_and_lane_center_in_both_layouts() {
        let chart = ChartDoc {
            version: String::new(),
            title: String::new(),
            artist: String::new(),
            simai_level: 0,
            bpm: 120.0,
            bpms: Vec::new(),
            audio_offset: 0.0,
            notes: Vec::new(),
            templates: Vec::new(),
            template_instances: Vec::new(),
        };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(930.0, 922.0));
        for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
            let mut editor = EditorState {
                layout,
                tool: Tool::Tap,
                pixels_per_second: 100.0,
                snap_division: 4,
                ..Default::default()
            };
            let pointer = match layout {
                EditorLayout::Vertical => egui::pos2(240.0, 771.0),
                EditorLayout::Horizontal => egui::pos2(143.0, 232.0),
            };
            let (note, center) = ghost(rect, &editor, &chart, 0.0, pointer).unwrap();
            assert_eq!(note.time, 1.5);
            assert_eq!(note.lane, 3);
            assert_eq!(
                center,
                match layout {
                    EditorLayout::Vertical => egui::pos2(280.0, 784.0),
                    EditorLayout::Horizontal => egui::pos2(130.0, 272.0),
                }
            );
            editor.snap_enabled = false;
            let (note, _) = ghost(rect, &editor, &chart, 0.0, pointer).unwrap();
            assert!((note.time - 1.565).abs() < 1e-5);
            editor.transform_pointer_down = true;
            assert!(ghost(rect, &editor, &chart, 0.0, pointer).is_none());
            editor.transform_pointer_down = false;
            editor.tool = Tool::Select;
            assert!(ghost(rect, &editor, &chart, 0.0, pointer).is_none());
            editor.tool = Tool::Touch;
            let (_, center) = ghost(rect, &editor, &chart, 0.0, pointer).unwrap();
            assert_eq!(
                match layout {
                    EditorLayout::Vertical => center.x,
                    EditorLayout::Horizontal => center.y,
                },
                match layout {
                    EditorLayout::Vertical => 880.0,
                    EditorLayout::Horizontal => 872.0,
                }
            );
        }
    }
}
