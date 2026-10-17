//! Direct sensor picking shares the Pad's SVG geometry and coordinate system.

use egui_macroquad::egui::{self, Color32, Pos2, Sense, Stroke};

use crate::app::types::{
    ChartDoc, PadGeom, note_is_touch,
    zone::{id_to_zone, svg_id_to_zone},
};
use crate::player::state::PadPreviewState;
use crate::state::{EditorState, Tool, Workspace};
use crate::theme::{ACCENT_BLUE, TEXT_DIM};

pub fn zone_label(zone: u8) -> String {
    id_to_zone(zone)
        .map(|z| z.to_string())
        .unwrap_or_else(|| "-".into())
}

pub fn parse_zone(text: &str) -> Option<u8> {
    svg_id_to_zone(&text.trim().to_ascii_uppercase()).map(|z| z.to_id())
}

fn touch_target(editor: &EditorState, chart: &ChartDoc) -> Option<u64> {
    let id = editor
        .pinned_note
        .or_else(|| crate::keyboard::navigation::target(editor, chart))?;
    chart
        .notes
        .iter()
        .find(|note| note.id == id && note_is_touch(note))
        .map(|note| note.id)
}

fn set_touch_zone(editor: &mut EditorState, chart: &mut ChartDoc, target: Option<u64>, zone: u8) {
    if id_to_zone(zone).is_none() {
        return;
    }
    editor.touch_zone = Some(zone);
    if editor.keyboard.active {
        editor.keyboard.cursor.zone = zone;
    }
    if let Some(index) = target.and_then(|id| {
        chart
            .notes
            .iter()
            .position(|note| note.id == id && note_is_touch(note) && note.lane != zone)
    }) {
        editor.push_history(chart);
        chart.notes[index].lane = zone;
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    mut pad: PadGeom,
    editable: bool,
) {
    // The autoplay editor's own state decides whether this Pad drives autoplay
    // editing: picking an element (or drafting a curve) switches it over.
    let autoplay = editor.autoplay_editor.pad_active();
    if autoplay && crate::autoplay_editor::curves::pad(ui, editor, app, pad, editable) { return; }
    let editable =
        editable && (!autoplay || app.has_engine() && editor.autoplay_editor.can_pick_zone());
    let target = touch_target(editor, &app.chart);
    if !autoplay
        && (editor.is_slide_edit()
            || (target.is_none() && !matches!(editor.tool, Tool::Touch | Tool::TouchHold)))
    {
        return;
    }
    let Some(svg) = &app.pad_svg else {
        return;
    };
    pad.outer_r /= crate::app::params::pad_circle_scale().max(1.0);
    let selected = if autoplay {
        editor.autoplay_editor.pad_zone()
    } else {
        zone_label(
            target
                .and_then(|id| app.chart.notes.iter().find(|n| n.id == id))
                .map(|n| n.lane)
                .or(editor.touch_zone)
                .unwrap_or(editor.keyboard.cursor.zone),
        )
    };
    let response = ui.interact(
        ui.max_rect(),
        ui.id().with("sensor_pad"),
        if editable {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let pointer = response.hover_pos();
    let mut hovered = pointer
        .and_then(|p| svg.hit_test(macroquad::prelude::vec2(p.x, p.y), &pad))
        .map(|zone| zone.to_string());
    let painter = ui.painter();
    if autoplay {
        let radius = pad.outer_r * crate::app::params::pad_circle_scale().max(1.0) * 1.06;
        let hit_radius = (radius * 0.09).clamp(6.0, 16.0);
        for index in 0..8 {
            let angle = -std::f32::consts::FRAC_PI_2
                + crate::app::types::PAD_ROTATION_RAD
                + index as f32 * std::f32::consts::TAU / 8.0;
            let center = Pos2::new(pad.cx + angle.cos() * radius, pad.cy + angle.sin() * radius);
            let zone = format!("K{}", index + 1);
            if pointer.is_some_and(|p| p.distance(center) <= hit_radius) {
                hovered = Some(zone.clone());
            }
            let active = selected == zone || hovered.as_ref() == Some(&zone);
            if active {
                painter.circle_stroke(center, hit_radius, Stroke::new(1.5_f32, ACCENT_BLUE));
            }
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                zone,
                egui::FontId::monospace(11.0),
                if active { ACCENT_BLUE } else { TEXT_DIM },
            );
        }
    }
    for def in &svg.zones {
        let zone = def.zone.to_string();
        let active = selected == zone;
        let hover = hovered.as_ref() == Some(&zone);
        let color = if active {
            ACCENT_BLUE
        } else if hover {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(45)
        };
        let vertices = svg
            .def_screen_verts(def, &pad)
            .into_iter()
            .map(|p| Pos2::new(p.x, p.y))
            .collect();
        painter.add(egui::Shape::closed_line(
            vertices,
            Stroke::new(if active || hover { 1.5_f32 } else { 0.7_f32 }, color),
        ));
        if active || hover {
            let center = svg.def_screen_centroid(def, &pad);
            painter.text(
                Pos2::new(center.x, center.y),
                egui::Align2::CENTER_CENTER,
                zone,
                egui::FontId::monospace(12.0),
                color,
            );
        }
    }
    if let Some(zone) = hovered {
        let clicked = editable && response.clicked_by(egui::PointerButton::Primary);
        response
            .on_hover_cursor(egui::CursorIcon::Crosshair)
            .on_hover_text(&zone);
        if clicked {
            if autoplay {
                let time = app.timeline_time();
                let mut state = std::mem::take(&mut editor.autoplay_editor);
                state.pick_zone(editor, &mut app.chart, &zone, time);
                editor.autoplay_editor = state;
            } else if let Some(zone) = parse_zone(&zone) {
                set_touch_zone(editor, &mut app.chart, target, zone);
            }
        }
    }
}
