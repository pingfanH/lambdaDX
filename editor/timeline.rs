//! Horizontal editor timeline.
//!
//! Rendering is done with macroquad (so note textures from `PadPreviewState`
//! can be drawn directly), behind a transparent egui central panel. egui only
//! hosts the interaction layer plus the bottom overview minimap.
//!
//! Note metrics and draw recipes follow `Mai2Chart/demo/macroquad_sim`'s
//! `app/egui_ui/draw_timeline.rs`, with the vertical (time-down) axis mapped to
//! the horizontal (time-right) editor axis.

use egui_macroquad::egui;
use macroquad::prelude as mq;
use macroquad::prelude::{Color, Rect as MqRect, Texture2D};
use macroquad::texture::DrawTextureParams;

use crate::app::types::{
    hold_tail_time, mdur_to_secs, measure_to_secs, note_secs, secs_to_measure, snap_measure,
    BpmChange, ChartDoc, Note, NoteType,
};
use crate::editor_audio::WaveformPyramid;
use crate::editor_state::{EditorState, Tool};
use crate::player::state::PadPreviewState;

pub const OVERVIEW_HEIGHT: f32 = 28.0;
/// Left gutter that holds the lane labels (`1..8`, `T`).
pub const LANE_GUTTER: f32 = 30.0;
pub const LANE_COUNT: usize = 9;
const RULER_H: f32 = 22.0;
/// Gap between the judgment line and the bottom edge in the vertical layout.
const JUDGE_MARGIN: f32 = 38.0;

// Reference note metrics (macroquad_sim, desktop profile, `scale = 1.0`).
const TAP_SIZE: f32 = 40.0;
const HOLD_WIDTH: f32 = 40.0;
const TOUCH_TRI: f32 = 30.0;
const TOUCH_POINT: f32 = 12.0;
const STAR_SIZE: f32 = 45.0;
const SLIDE_TILE_SPACING: f32 = 20.0;
const SLIDE_TILE_SCALE: f32 = 0.4;

fn c(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

/// Plot (time) area of the timeline: everything right of the lane gutter.
fn plot_span(rect: egui::Rect) -> (f32, f32) {
    (rect.left() + LANE_GUTTER, rect.right())
}

fn lane_index_of(note: &Note) -> usize {
    if note.is_touch || matches!(note.note_type, NoteType::Touch | NoteType::TouchHold) {
        LANE_COUNT - 1
    } else {
        (note.lane.saturating_sub(1) as usize).min(LANE_COUNT - 2)
    }
}

fn zone_lane_index(zone_id: u8) -> usize {
    (zone_id.saturating_sub(1) as usize).min(LANE_COUNT - 2)
}

fn note_tick_color(kind: NoteType) -> egui::Color32 {
    match kind {
        NoteType::Hold => egui::Color32::from_rgb(255, 107, 138),
        NoteType::Touch | NoteType::TouchHold => egui::Color32::from_rgb(179, 136, 255),
        NoteType::Slide => egui::Color32::from_rgb(255, 200, 87),
        NoteType::Tap => egui::Color32::from_rgb(76, 201, 240),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// egui interaction (transparent overlay over the macroquad-drawn timeline)
// ─────────────────────────────────────────────────────────────────────────────

pub fn handle_input(
    response: &egui::Response,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
    ui: &egui::Ui,
) {
    let (plot_left, plot_right) = plot_span(rect);
    let lanes_top = rect.top() + RULER_H;
    let lane_h = (rect.bottom() - lanes_top) / LANE_COUNT as f32;

    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let time = editor.view_start + (pos.x - plot_left) / editor.pixels_per_second;
    let measure = snap_measure(secs_to_measure(time, &chart.bpms)).max(1.0);
    let lane = (((pos.y - lanes_top) / lane_h).floor() as i64).clamp(1, LANE_COUNT as i64) as u8;

    // The red playhead is directly grabbable (drag it to scrub the player time).
    let play_x = plot_left + (current_time - editor.view_start) * editor.pixels_per_second;
    let on_playhead = (pos.x - play_x).abs() <= 6.0;

    let hit = chart.notes.iter().find_map(|n| {
        let x = plot_left + (note_secs(n, &chart.bpms) - editor.view_start) * editor.pixels_per_second;
        let y = lanes_top + (lane_index_of(n) as f32 + 0.5) * lane_h;
        ((x - pos.x).abs() < 12.0 && (y - pos.y).abs() < lane_h * 0.6).then_some(n.id)
    });

    if response.drag_started() {
        editor.scrubbing = on_playhead;
        editor.dragging = if on_playhead { None } else { hit };
        editor.drag_history_pushed = false;
        if let Some(id) = editor.dragging {
            editor.selected.clear();
            editor.selected.insert(id);
        }
    }
    if response.dragged() && editor.scrubbing {
        editor.pending_seek = Some(time.max(0.0));
        return;
    }
    if response.dragged() {
        if let Some(id) = editor.dragging {
            if !editor.drag_history_pushed {
                editor.push_history(chart);
                editor.drag_history_pushed = true;
            }
            if let Some(note) = chart.notes.iter_mut().find(|n| n.id == id) {
                note.time = measure;
                note.lane = lane.clamp(1, 8);
            }
        }
    }
    if response.drag_stopped() {
        editor.dragging = None;
        editor.drag_history_pushed = false;
        editor.scrubbing = false;
    }

    if response.clicked() {
        if on_playhead {
            editor.pending_seek = Some(time.max(0.0));
            return;
        }
        if let Some(id) = hit {
            editor.selected.clear();
            editor.selected.insert(id);
            if editor.tool == Tool::Eraser {
                editor.push_history(chart);
                chart.notes.retain(|n| n.id != id);
                editor.selected.clear();
            }
            return;
        }
        if matches!(
            editor.tool,
            Tool::Tap | Tool::Hold | Tool::Touch | Tool::Slide
        ) {
            editor.push_history(chart);
            let id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
            chart.notes.push(crate::app::types::Note {
                id,
                time: measure,
                lane: lane.min(8),
                note_type: editor.selected_type(),
                hold_duration: if editor.tool == Tool::Hold { 1.0 } else { 0.0 },
                is_touch: editor.tool == Tool::Touch,
                ..Default::default()
            });
            editor.selected.clear();
            editor.selected.insert(id);
        } else {
            // Select tool on empty space: move the red playhead there.
            editor.pending_seek = Some(time.max(0.0));
        }
    }

    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        && !editor.selected.is_empty()
    {
        editor.push_history(chart);
        chart.notes.retain(|n| !editor.selected.contains(&n.id));
        editor.selected.clear();
    }

    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        let old = editor.pixels_per_second;
        let new = (old * (1.0 + scroll * 0.001)).clamp(30.0, 1600.0);
        if let Some(pos) = response.hover_pos() {
            let anchor = editor.view_start + (pos.x - plot_left) / old;
            editor.view_start = anchor - (pos.x - plot_left) / new;
        }
        editor.pixels_per_second = new;
    }
    let _ = plot_right;
}

/// Vertical-timeline interaction: lanes are columns, time flows upward from the
/// judgment line (future above, past below), mirroring the original editor.
pub fn handle_input_vertical(
    response: &egui::Response,
    rect: egui::Rect,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    current_time: f32,
    ui: &egui::Ui,
) {
    let plot_left = rect.left() + LANE_GUTTER;
    let plot_right = rect.right();
    let judge_y = rect.bottom() - JUDGE_MARGIN;
    let lane_w = (plot_right - plot_left).max(1.0) / LANE_COUNT as f32;
    let pps = editor.pixels_per_second;
    let now = current_time;

    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    let time = now + (judge_y - pos.y) / pps;
    let measure = snap_measure(secs_to_measure(time, &chart.bpms)).max(1.0);
    let lane = (((pos.x - plot_left) / lane_w).floor() as i64).clamp(1, LANE_COUNT as i64) as u8;

    // The judgment line doubles as the scrub handle.
    let on_playhead = (pos.y - judge_y).abs() <= 6.0;

    let hit = chart.notes.iter().find_map(|n| {
        let y = judge_y - (note_secs(n, &chart.bpms) - now) * pps;
        let x = plot_left + (lane_index_of(n) as f32 + 0.5) * lane_w;
        ((y - pos.y).abs() < 12.0 && (x - pos.x).abs() < lane_w * 0.6).then_some(n.id)
    });

    if response.drag_started() {
        editor.scrubbing = on_playhead;
        editor.dragging = if on_playhead { None } else { hit };
        editor.drag_history_pushed = false;
        if let Some(id) = editor.dragging {
            editor.selected.clear();
            editor.selected.insert(id);
        }
    }
    if response.dragged() && editor.scrubbing {
        editor.pending_seek = Some(time.max(0.0));
        return;
    }
    if response.dragged() {
        if let Some(id) = editor.dragging {
            if !editor.drag_history_pushed {
                editor.push_history(chart);
                editor.drag_history_pushed = true;
            }
            if let Some(note) = chart.notes.iter_mut().find(|n| n.id == id) {
                note.time = measure;
                note.lane = lane.clamp(1, 8);
            }
        }
    }
    if response.drag_stopped() {
        editor.dragging = None;
        editor.drag_history_pushed = false;
        editor.scrubbing = false;
    }

    if response.clicked() {
        if on_playhead {
            editor.pending_seek = Some(time.max(0.0));
            return;
        }
        if let Some(id) = hit {
            editor.selected.clear();
            editor.selected.insert(id);
            if editor.tool == Tool::Eraser {
                editor.push_history(chart);
                chart.notes.retain(|n| n.id != id);
                editor.selected.clear();
            }
            return;
        }
        if matches!(
            editor.tool,
            Tool::Tap | Tool::Hold | Tool::Touch | Tool::Slide
        ) {
            editor.push_history(chart);
            let id = chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
            chart.notes.push(crate::app::types::Note {
                id,
                time: measure,
                lane: lane.min(8),
                note_type: editor.selected_type(),
                hold_duration: if editor.tool == Tool::Hold { 1.0 } else { 0.0 },
                is_touch: editor.tool == Tool::Touch,
                ..Default::default()
            });
            editor.selected.clear();
            editor.selected.insert(id);
        } else {
            editor.pending_seek = Some(time.max(0.0));
        }
    }

    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        && !editor.selected.is_empty()
    {
        editor.push_history(chart);
        chart.notes.retain(|n| !editor.selected.contains(&n.id));
        editor.selected.clear();
    }

    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        let old = editor.pixels_per_second;
        editor.pixels_per_second = (old * (1.0 + scroll * 0.001)).clamp(30.0, 1600.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// macroquad rendering
// ─────────────────────────────────────────────────────────────────────────────

struct NoteCtx<'a> {
    app: &'a PadPreviewState,
    bpms: &'a [BpmChange],
    plot_left: f32,
    pps: f32,
    view: f32,
    lanes_top: f32,
    lane_h: f32,
    /// Pixels-per-point, so the fixed reference metrics scale with the display.
    scale: f32,
}

impl NoteCtx<'_> {
    fn x_of(&self, t: f32) -> f32 {
        self.plot_left + (t - self.view) * self.pps
    }
    fn lane_y(&self, lane_index: usize) -> f32 {
        self.lanes_top + (lane_index as f32 + 0.5) * self.lane_h
    }
}

pub fn draw_macroquad(
    app: &PadPreviewState,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: MqRect,
    current_time: f32,
    waveform: &WaveformPyramid,
    scale: f32,
) {
    let s = scale.max(0.1);
    let bpms = &chart.bpms;
    let gutter = LANE_GUTTER * s;
    let ruler_h = RULER_H * s;
    let plot_left = rect.x + gutter;
    let plot_right = rect.x + rect.w;
    let lanes_top = rect.y + ruler_h;
    let lanes_bottom = rect.y + rect.h;
    let lanes_h = (lanes_bottom - lanes_top).max(1.0);
    let lane_h = lanes_h / LANE_COUNT as f32;
    let pps = editor.pixels_per_second;
    let view = editor.view_start;

    // Backgrounds.
    mq::draw_rectangle(rect.x, rect.y, rect.w, rect.h, c(24, 24, 27));
    mq::draw_rectangle(rect.x, rect.y, rect.w, ruler_h, c(42, 42, 45));
    mq::draw_rectangle(rect.x, lanes_top, gutter, lanes_h, c(42, 42, 45));

    let view_end = view + (plot_right - plot_left) / pps;

    // Waveform behind everything.
    draw_waveform(waveform, plot_left, plot_right, lanes_top, lanes_h, view, pps, s);

    // Measure grid: 64 subdivisions per measure (bar / beat / sub).
    const SUBDIV: i32 = 64;
    let start_m = secs_to_measure(view, bpms).floor();
    let end_m = secs_to_measure(view_end, bpms) + 1.0;
    let mut m = start_m;
    while m <= end_m {
        for i in 0..SUBDIV {
            let x = plot_left + (measure_to_secs(m + i as f32 / SUBDIV as f32, bpms) - view) * pps;
            if x < plot_left - 1.0 || x > plot_right + 1.0 {
                continue;
            }
            let (col, th) = if i == 0 {
                (c(140, 50, 50), 2.0 * s)
            } else if i % 16 == 0 {
                (c(80, 80, 80), 1.5 * s)
            } else {
                (c(40, 40, 40), 0.5 * s)
            };
            mq::draw_line(x, lanes_top, x, lanes_bottom, th, col);
        }
        let x = plot_left + (measure_to_secs(m, bpms) - view) * pps;
        if x >= plot_left - 40.0 * s && x <= plot_right {
            mq::draw_text(
                &format!("{}", m as i32),
                x + 3.0 * s,
                rect.y + 15.0 * s,
                14.0 * s,
                c(140, 140, 140),
            );
        }
        m += 1.0;
    }

    // Lane separators + labels.
    for lane in 0..=LANE_COUNT {
        let y = lanes_top + lane as f32 * lane_h;
        let (col, th) = if lane == 4 {
            (c(90, 90, 90), 2.0 * s)
        } else {
            (c(55, 55, 55), 1.0 * s)
        };
        mq::draw_line(plot_left, y, plot_right, y, th, col);
    }
    for lane in 0..LANE_COUNT {
        let y = lanes_top + (lane as f32 + 0.5) * lane_h + 5.0 * s;
        let (label, col) = if lane == LANE_COUNT - 1 {
            ("T".to_string(), c(230, 149, 48))
        } else {
            ((lane + 1).to_string(), c(180, 180, 180))
        };
        mq::draw_text(&label, rect.x + 10.0 * s, y, 15.0 * s, col);
    }

    // Notes.
    let ctx = NoteCtx {
        app,
        bpms,
        plot_left,
        pps,
        view,
        lanes_top,
        lane_h,
        scale: s,
    };
    for note in &chart.notes {
        let head = note_secs(note, bpms);
        let x = ctx.x_of(head);
        let y = ctx.lane_y(lane_index_of(note));
        let tail = match note.note_type {
            NoteType::Hold | NoteType::TouchHold => hold_tail_time(note, bpms),
            NoteType::Slide => crate::app::types::slide_end_time(note, bpms),
            _ => head,
        };
        let x_tail = ctx.x_of(tail);
        if x_tail < plot_left - 80.0 * s || x > plot_right + 80.0 * s {
            continue;
        }
        let selected = editor.selected.contains(&note.id);
        draw_note(&ctx, note, x, x_tail, y, selected);
    }

    // Playhead.
    let px = ctx.x_of(current_time);
    if px >= plot_left - 2.0 * s && px <= plot_right + 2.0 * s {
        mq::draw_line(px, rect.y, px, lanes_bottom, 2.0 * s, c(208, 70, 70));
    }
}

fn draw_note(ctx: &NoteCtx, note: &Note, x: f32, x_tail: f32, y: f32, selected: bool) {
    let app = ctx.app;
    let s = ctx.scale;
    let tap = TAP_SIZE * s;
    let hold_w = HOLD_WIDTH * s;
    if selected {
        mq::draw_circle(x, y, tap * 0.8, ca(56, 189, 248, 100));
    }
    match note.note_type {
        NoteType::Tap => {
            let tex = if note.is_break {
                app.tap_break_tex.as_ref()
            } else if note.is_each {
                app.tap_each_tex.as_ref()
            } else {
                app.tap_texture.as_ref()
            }
            .or(app.tap_texture.as_ref());
            if let Some(t) = tex {
                draw_tex(t, x, y, tap, tap, mq::WHITE);
            } else {
                fallback_tap(x, y, tap);
            }
            if note.is_ex {
                if let Some(ex) = app.tap_ex_tex.as_ref() {
                    draw_tex(ex, x, y, tap, tap, mq::WHITE);
                }
            }
            // Judgment center dot.
            mq::draw_circle(x, y, 2.5 * s, ca(255, 255, 255, 200));
        }
        NoteType::Hold | NoteType::TouchHold => {
            let hw = hold_w;
            let (x0, x1) = (x.min(x_tail), x.max(x_tail));
            let hold_tex = if note.is_break {
                app.hold_break_tex.as_ref()
            } else if note.is_each {
                app.hold_each_tex.as_ref()
            } else {
                app.hold_texture.as_ref()
            }
            .or(app.hold_texture.as_ref());
            match hold_tex {
                Some(t) => {
                    draw_hold_9slice_horizontal(t, x0, x1, y, hw, mq::WHITE);
                    if note.is_ex {
                        if let Some(ex) = app.hold_ex_tex.as_ref() {
                            draw_hold_9slice_horizontal(ex, x0, x1, y, hw, mq::WHITE);
                        }
                    }
                }
                None => {
                    let top = y - hw * 0.2;
                    let h = hw * 0.4;
                    mq::draw_rectangle(x0, top, (x1 - x0).max(hw * 0.133), h, ca(190, 24, 93, 130));
                    mq::draw_rectangle_lines(
                        x0,
                        top,
                        (x1 - x0).max(hw * 0.133),
                        h,
                        1.0 * s,
                        ca(244, 114, 182, 200),
                    );
                }
            }
            // Judgment center dots (head & tail).
            mq::draw_circle(x0, y, 2.5 * s, ca(255, 255, 255, 200));
            mq::draw_circle(x1, y, 2.5 * s, ca(255, 255, 255, 180));
        }
        NoteType::Touch => {
            draw_touch(ctx, note, x, y);
        }
        NoteType::Slide => {
            draw_slide(ctx, note, x, y);
        }
    }
}

fn draw_touch(ctx: &NoteCtx, note: &Note, x: f32, y: f32) {
    draw_touch_at(ctx.app, ctx.scale, note, x, y);
}

fn draw_touch_at(app: &PadPreviewState, s: f32, note: &Note, x: f32, y: f32) {
    let tri = if note.is_each {
        app.touch_tri_each_tex.as_ref()
    } else {
        app.touch_tri_tex.as_ref()
    }
    .or(app.touch_tri_tex.as_ref());
    let point = if note.is_each {
        app.touch_point_each_tex.as_ref()
    } else {
        app.touch_point_tex.as_ref()
    }
    .or(app.touch_point_tex.as_ref());

    if let Some(t) = tri {
        let ratio = t.width() / t.height().max(1.0);
        let ts = TOUCH_TRI * s;
        let tw = ts;
        let th = ts / ratio.max(0.001);
        let color = ca(255, 255, 255, 200);
        // Four orientations (the reference builds a cross of triangles).
        tex_at(t, x - tw * 0.5, y + ts * 0.3 - th * 0.5, tw, th, 0.0, color);
        tex_at(t, x - tw * 0.5, y - ts * 0.3 - th * 0.5, tw, th, std::f32::consts::PI, color);
        tex_at(
            t,
            x - ts * 0.3 - tw * 0.5,
            y - th * 0.5,
            tw,
            th,
            std::f32::consts::FRAC_PI_2,
            color,
        );
        tex_at(
            t,
            x + ts * 0.3 - tw * 0.5,
            y - th * 0.5,
            tw,
            th,
            -std::f32::consts::FRAC_PI_2,
            color,
        );
    }
    if let Some(p) = point {
        let ps = TOUCH_POINT * s;
        draw_tex(p, x, y, ps, ps, ca(255, 255, 255, 200));
    } else {
        mq::draw_circle(x, y, 3.0 * s, ca(103, 232, 249, 200));
    }
}

fn draw_slide(ctx: &NoteCtx, note: &Note, x: f32, y: f32) {
    let app = ctx.app;
    let bpms = ctx.bpms;
    let s = ctx.scale;
    let head = note_secs(note, bpms);
    for (si, sl) in note.slide.iter().enumerate() {
        let _ = si;
        let dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.0);
        let delay_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
            .max(0.0)
            .min(dur_s);
        let delay_x = ctx.x_of(head + delay_s);
        let tail_x = ctx.x_of(head + dur_s);

        // Dashed delay segment (head -> slide motion start).
        let delay_len = (delay_x - x).abs();
        if delay_s > 0.0 && delay_len > 0.5 {
            let dash = 6.0_f32 * s;
            let gap = 4.0_f32 * s;
            let period = dash + gap;
            let left = x.min(delay_x);
            let n = (delay_len / period).ceil() as i32;
            let col = ca(253, 224, 71, 220);
            for k in 0..n {
                let x0 = left + k as f32 * period;
                let x1 = (x0 + dash).min(left + delay_len);
                mq::draw_line(x0, y, x1, y, 2.0 * s, col);
            }
        }

        // Tiled trail through the A-zone waypoints.
        let travel = (tail_x - delay_x).abs();
        if travel > 0.5 {
            let mut waypoints: Vec<(f32, f32)> = vec![(delay_x, y)];
            let mut a_points: Vec<u8> = Vec::new();
            for seg in &sl.segments {
                for sp in &seg.points {
                    let id = sp.zone.to_id();
                    if (1..=8).contains(&id) {
                        a_points.push(id);
                    }
                }
            }
            if a_points.is_empty() {
                waypoints.push((tail_x, y));
            } else {
                let n = a_points.len();
                for (pi, id) in a_points.iter().enumerate() {
                    let frac = (pi + 1) as f32 / n as f32;
                    let wx = ctx.x_of(head + delay_s + (dur_s - delay_s) * frac);
                    let wy = ctx.lane_y(zone_lane_index(*id));
                    waypoints.push((wx, wy));
                }
            }
            let slide_tex = if sl.slide_is_break {
                app.slide_break_tex.as_ref()
            } else if note.is_each {
                app.slide_each_tex.as_ref()
            } else {
                app.slide_tex.as_ref()
            };
            for seg in waypoints.windows(2) {
                let (x0, y0) = seg[0];
                let (x1, y1) = seg[1];
                if let Some(tex) = slide_tex {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let seg_len = (dx * dx + dy * dy).sqrt();
                    let bar_w = 14.0 * s;
                    let tw_nat = tex.width() * SLIDE_TILE_SCALE;
                    let th_nat = tex.height() * SLIDE_TILE_SCALE;
                    let tile_h = th_nat * (bar_w / tw_nat.max(0.001)).max(0.001);
                    let spacing = SLIDE_TILE_SPACING;
                    let angle = dy.atan2(dx) + std::f32::consts::PI;
                    let steps = (seg_len / spacing).ceil().max(1.0) as i32;
                    for k in 0..=steps {
                        let t = k as f32 / steps as f32;
                        let px = x0 + dx * t;
                        let py = y0 + dy * t;
                        tex_at(tex, px - bar_w * 0.5, py - tile_h * 0.5, bar_w, tile_h, angle, ca(255, 255, 255, 230));
                    }
                } else {
                    mq::draw_line(x0, y0, x1, y1, 3.0 * s, ca(250, 204, 21, 200));
                }
            }
            for wp in &waypoints[1..] {
                mq::draw_circle(wp.0, wp.1, 3.0 * s, ca(253, 224, 71, 200));
            }
        }

        if delay_s > 0.0 || dur_s > 0.0 {
            mq::draw_circle(delay_x, y, 4.5 * s, ca(56, 189, 248, 230));
            mq::draw_circle_lines(delay_x, y, 4.5 * s, 1.5 * s, ca(255, 255, 255, 220));
        }
        if dur_s > 0.0 {
            mq::draw_circle(tail_x, y, 5.5 * s, ca(250, 204, 21, 230));
            mq::draw_circle_lines(tail_x, y, 5.5 * s, 1.5 * s, ca(255, 255, 255, 220));
        }
    }

    // Star head at note.time (same size as Tap); skipped for tapless slides.
    if !note.is_tapless {
        let is_double = note.is_star;
        let star_tex = if note.is_break {
            if is_double {
                app.star_double_break_tex.as_ref()
            } else {
                app.star_break_tex.as_ref()
            }
        } else if note.is_each {
            if is_double {
                app.star_double_each_tex.as_ref()
            } else {
                app.star_each_tex.as_ref()
            }
        } else if is_double {
            app.star_double_tex.as_ref()
        } else {
            app.star_tex.as_ref()
        };
        let fallback = if is_double {
            app.star_double_tex.as_ref()
        } else {
            app.star_tex.as_ref()
        };
        let ss = STAR_SIZE * s;
        if let Some(tex) = star_tex.or(fallback).or(app.star_tex.as_ref()) {
            draw_tex(tex, x, y, ss, ss, ca(255, 255, 255, 230));
            if note.is_ex {
                let ex = if is_double {
                    app.star_double_ex_tex.as_ref()
                } else {
                    app.star_ex_tex.as_ref()
                }
                .or(app.star_ex_tex.as_ref());
                if let Some(ex) = ex {
                    draw_tex(ex, x, y, ss, ss, ca(255, 255, 255, 230));
                }
            }
        } else {
            mq::draw_poly(x, y, 5, ss * 0.4, 0.0, ca(250, 204, 21, 230));
        }
    }
}

fn fallback_tap(x: f32, y: f32, size: f32) {
    let r = size * 0.3125;
    mq::draw_circle(x, y, r, c(17, 24, 39));
    mq::draw_circle_lines(x, y, r, r * 0.3, c(244, 114, 182));
    mq::draw_circle(x, y, r * 0.3, c(249, 168, 212));
}

fn draw_tex(tex: &Texture2D, cx: f32, cy: f32, w: f32, h: f32, color: Color) {
    mq::draw_texture_ex(
        tex,
        cx - w * 0.5,
        cy - h * 0.5,
        color,
        DrawTextureParams {
            dest_size: Some(mq::vec2(w, h)),
            ..Default::default()
        },
    );
}

/// Draw a texture at a top-left position with a rotation (no custom pivot),
/// matching the reference's triangle placement.
fn tex_at(tex: &Texture2D, x: f32, y: f32, w: f32, h: f32, rot: f32, color: Color) {
    mq::draw_texture_ex(
        tex,
        x,
        y,
        color,
        DrawTextureParams {
            dest_size: Some(mq::vec2(w, h)),
            rotation: rot,
            ..Default::default()
        },
    );
}

/// Horizontal 9-slice of the (vertical) hold texture: fixed left/right caps,
/// stretched middle. `x0..x1` are the head/tail judgment centers.
fn draw_hold_9slice_horizontal(tex: &Texture2D, x0: f32, x1: f32, y: f32, width: f32, tint: Color) {
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_len = (cap_h * (width / tex_w)).max(1.0);

    let left = x0.min(x1);
    let right = x0.max(x1);
    let center_len = (right - left).max(1.0);
    let (head_len, tail_len) = if center_len < cap_len * 2.0 {
        let squeezed = (center_len * 0.5).max(1.0);
        (squeezed, squeezed)
    } else {
        (cap_len, cap_len)
    };

    let head_center = left;
    let tail_center = right;
    let body_start = head_center + head_len * 0.5;
    let body_end = tail_center - tail_len * 0.5;
    let body_len = (body_end - body_start).max(0.0);

    hold_piece(tex, 0.0, cap_h, head_center, y, width, head_len, tint);
    if body_len > 0.0 {
        hold_piece(
            tex,
            cap_h,
            body_src_h,
            (body_start + body_end) * 0.5,
            y,
            width,
            body_len,
            tint,
        );
    }
    hold_piece(tex, tex_h - cap_h, cap_h, tail_center, y, width, tail_len, tint);
}

fn hold_piece(tex: &Texture2D, src_y: f32, src_h: f32, cx: f32, cy: f32, width: f32, len: f32, tint: Color) {
    mq::draw_texture_ex(
        tex,
        cx - width * 0.5,
        cy - len * 0.5,
        tint,
        DrawTextureParams {
            source: Some(MqRect {
                x: 0.0,
                y: src_y,
                w: tex.width(),
                h: src_h,
            }),
            dest_size: Some(mq::vec2(width, len)),
            rotation: std::f32::consts::FRAC_PI_2,
            pivot: Some(mq::vec2(width * 0.5, len * 0.5)),
            ..Default::default()
        },
    );
}

fn draw_waveform(
    waveform: &WaveformPyramid,
    plot_left: f32,
    plot_right: f32,
    lanes_top: f32,
    lanes_h: f32,
    view: f32,
    pps: f32,
    scale: f32,
) {
    if waveform.levels.is_empty() {
        return;
    }
    let level = waveform
        .levels
        .iter()
        .rev()
        .find(|l| waveform.duration / l.min_max.len().max(1) as f32 * pps < 2.0 * scale)
        .or_else(|| waveform.levels.first())
        .unwrap();
    let bins = level.min_max.len().max(1);
    let bin_duration = waveform.duration / bins as f32;
    if bin_duration <= 0.0 {
        return;
    }
    let center = lanes_top + lanes_h * 0.5;
    let first = ((view / bin_duration).floor() as isize - 1).max(0) as usize;
    let last =
        (((view + (plot_right - plot_left) / pps) / bin_duration).ceil() as usize + 1).min(bins);
    for index in first..last {
        let x = plot_left + (index as f32 * bin_duration - view) * pps;
        if x < plot_left - 1.0 || x > plot_right + 1.0 {
            continue;
        }
        let [lo, hi] = level.min_max[index];
        mq::draw_line(
            x,
            center - hi * lanes_h * 0.45,
            x,
            center - lo * lanes_h * 0.45,
            1.0 * scale,
            ca(90, 141, 186, 130),
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vertical macroquad rendering (time flows upward from the judgment line)
// ─────────────────────────────────────────────────────────────────────────────

struct VNoteCtx<'a> {
    app: &'a PadPreviewState,
    bpms: &'a [BpmChange],
    judge_y: f32,
    pps: f32,
    now: f32,
    plot_left: f32,
    lane_w: f32,
    scale: f32,
}

impl VNoteCtx<'_> {
    fn y_of(&self, t: f32) -> f32 {
        self.judge_y - (t - self.now) * self.pps
    }
    fn lane_x(&self, lane_index: usize) -> f32 {
        self.plot_left + (lane_index as f32 + 0.5) * self.lane_w
    }
}

pub fn draw_macroquad_vertical(
    app: &PadPreviewState,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: MqRect,
    current_time: f32,
    waveform: &WaveformPyramid,
    scale: f32,
) {
    let s = scale.max(0.1);
    let bpms = &chart.bpms;
    let header_h = RULER_H * s;
    let left_margin = LANE_GUTTER * s;
    let plot_left = rect.x + left_margin;
    let plot_right = rect.x + rect.w;
    let plot_top = rect.y + header_h;
    let plot_bottom = rect.y + rect.h;
    let judge_y = plot_bottom - JUDGE_MARGIN * s;
    let lane_w = (plot_right - plot_left).max(1.0) / LANE_COUNT as f32;
    let pps = editor.pixels_per_second;
    let now = current_time;
    let y_of = |t: f32| judge_y - (t - now) * pps;
    let lane_x = |i: usize| plot_left + (i as f32 + 0.5) * lane_w;

    // Backgrounds.
    mq::draw_rectangle(rect.x, rect.y, rect.w, rect.h, c(24, 24, 27));
    mq::draw_rectangle(rect.x, rect.y, rect.w, header_h, c(42, 42, 45));
    mq::draw_rectangle(
        rect.x,
        plot_top,
        left_margin,
        (plot_bottom - plot_top).max(0.0),
        c(42, 42, 45),
    );

    // Waveform behind everything.
    draw_waveform_vertical(
        waveform, plot_left, plot_right, plot_top, plot_bottom, judge_y, now, pps, s,
    );

    // Measure grid: 64 subdivisions per measure (bar / beat / sub).
    const SUBDIV: i32 = 64;
    let t_top = now + (judge_y - plot_top) / pps;
    let t_bottom = now + (judge_y - plot_bottom) / pps;
    let start_m = secs_to_measure(t_bottom.min(t_top), bpms).floor();
    let end_m = secs_to_measure(t_bottom.max(t_top), bpms) + 1.0;
    let mut m = start_m;
    while m <= end_m {
        for i in 0..SUBDIV {
            let y = y_of(measure_to_secs(m + i as f32 / SUBDIV as f32, bpms));
            if y < plot_top - 1.0 || y > plot_bottom + 1.0 {
                continue;
            }
            let (col, th) = if i == 0 {
                (c(140, 50, 50), 2.0 * s)
            } else if i % 16 == 0 {
                (c(80, 80, 80), 1.5 * s)
            } else {
                (c(40, 40, 40), 0.5 * s)
            };
            mq::draw_line(plot_left, y, plot_right, y, th, col);
        }
        let y = y_of(measure_to_secs(m, bpms));
        if y >= plot_top && y <= plot_bottom + 40.0 * s {
            mq::draw_text(
                &format!("{}", m as i32),
                rect.x + 4.0 * s,
                y + 4.0 * s,
                14.0 * s,
                c(140, 140, 140),
            );
        }
        m += 1.0;
    }

    // Lane separators (vertical lines) + header labels.
    for lane in 0..=LANE_COUNT {
        let x = plot_left + lane as f32 * lane_w;
        let (col, th) = if lane == 4 {
            (c(90, 90, 90), 2.0 * s)
        } else {
            (c(55, 55, 55), 1.0 * s)
        };
        mq::draw_line(x, plot_top, x, plot_bottom, th, col);
    }
    for lane in 0..LANE_COUNT {
        let (label, col) = if lane == LANE_COUNT - 1 {
            ("T".to_string(), c(230, 149, 48))
        } else {
            ((lane + 1).to_string(), c(180, 180, 180))
        };
        let tw = mq::measure_text(&label, None, (15.0 * s) as u16, 1.0).width;
        mq::draw_text(&label, lane_x(lane) - tw * 0.5, rect.y + 16.0 * s, 15.0 * s, col);
    }

    // Notes.
    let ctx = VNoteCtx {
        app,
        bpms,
        judge_y,
        pps,
        now,
        plot_left,
        lane_w,
        scale: s,
    };
    for note in &chart.notes {
        let head = note_secs(note, bpms);
        let y = ctx.y_of(head);
        let x = ctx.lane_x(lane_index_of(note));
        let tail = match note.note_type {
            NoteType::Hold | NoteType::TouchHold => hold_tail_time(note, bpms),
            NoteType::Slide => crate::app::types::slide_end_time(note, bpms),
            _ => head,
        };
        let y_tail = ctx.y_of(tail);
        let (ya, yb) = (y.min(y_tail), y.max(y_tail));
        if yb < plot_top - 80.0 * s || ya > plot_bottom + 80.0 * s {
            continue;
        }
        draw_note_vertical(&ctx, note, x, y, y_tail, editor.selected.contains(&note.id));
    }

    // Judgment line = "now" playhead.
    mq::draw_line(plot_left, judge_y, plot_right, judge_y, 2.0 * s, c(200, 70, 70));
}

fn draw_note_vertical(ctx: &VNoteCtx, note: &Note, x: f32, y: f32, y_tail: f32, selected: bool) {
    let app = ctx.app;
    let s = ctx.scale;
    let tap = TAP_SIZE * s;
    let hold_w = HOLD_WIDTH * s;
    if selected {
        mq::draw_circle(x, y, tap * 0.8, ca(56, 189, 248, 100));
    }
    match note.note_type {
        NoteType::Tap => {
            let tex = if note.is_break {
                app.tap_break_tex.as_ref()
            } else if note.is_each {
                app.tap_each_tex.as_ref()
            } else {
                app.tap_texture.as_ref()
            }
            .or(app.tap_texture.as_ref());
            if let Some(t) = tex {
                draw_tex(t, x, y, tap, tap, mq::WHITE);
            } else {
                fallback_tap(x, y, tap);
            }
            if note.is_ex {
                if let Some(ex) = app.tap_ex_tex.as_ref() {
                    draw_tex(ex, x, y, tap, tap, mq::WHITE);
                }
            }
            mq::draw_circle(x, y, 2.5 * s, ca(255, 255, 255, 200));
        }
        NoteType::Hold | NoteType::TouchHold => {
            let (y0, y1) = (y, y_tail);
            let hold_tex = if note.is_break {
                app.hold_break_tex.as_ref()
            } else if note.is_each {
                app.hold_each_tex.as_ref()
            } else {
                app.hold_texture.as_ref()
            }
            .or(app.hold_texture.as_ref());
            match hold_tex {
                Some(t) => {
                    draw_hold_9slice_vertical(t, x, y0, y1, hold_w, mq::WHITE);
                    if note.is_ex {
                        if let Some(ex) = app.hold_ex_tex.as_ref() {
                            draw_hold_9slice_vertical(ex, x, y0, y1, hold_w, mq::WHITE);
                        }
                    }
                }
                None => {
                    let top = y0.min(y1);
                    let len = (y0 - y1).abs().max(hold_w * 0.133);
                    mq::draw_rectangle(
                        x - hold_w * 0.2,
                        top,
                        hold_w * 0.4,
                        len,
                        ca(190, 24, 93, 130),
                    );
                    mq::draw_rectangle_lines(
                        x - hold_w * 0.2,
                        top,
                        hold_w * 0.4,
                        len,
                        1.0 * s,
                        ca(244, 114, 182, 200),
                    );
                }
            }
            mq::draw_circle(x, y0, 2.5 * s, ca(255, 255, 255, 200));
            mq::draw_circle(x, y1, 2.5 * s, ca(255, 255, 255, 180));
        }
        NoteType::Touch => {
            draw_touch_at(app, s, note, x, y);
        }
        NoteType::Slide => {
            draw_slide_vertical(ctx, note, x, y);
        }
    }
}

fn draw_slide_vertical(ctx: &VNoteCtx, note: &Note, x: f32, y: f32) {
    let app = ctx.app;
    let bpms = ctx.bpms;
    let s = ctx.scale;
    let head = note_secs(note, bpms);
    for sl in &note.slide {
        let dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.0);
        let delay_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
            .max(0.0)
            .min(dur_s);
        let delay_y = ctx.y_of(head + delay_s);
        let tail_y = ctx.y_of(head + dur_s);

        // Dashed delay segment (head -> slide motion start).
        let delay_len = (delay_y - y).abs();
        if delay_s > 0.0 && delay_len > 0.5 {
            let dash = 6.0_f32 * s;
            let gap = 4.0_f32 * s;
            let period = dash + gap;
            let top = y.min(delay_y);
            let n = (delay_len / period).ceil() as i32;
            let col = ca(253, 224, 71, 220);
            for k in 0..n {
                let y0 = top + k as f32 * period;
                let y1 = (y0 + dash).min(top + delay_len);
                mq::draw_line(x, y0, x, y1, 2.0 * s, col);
            }
        }

        // Tiled trail through the A-zone waypoints.
        let travel = (tail_y - delay_y).abs();
        if travel > 0.5 {
            let mut waypoints: Vec<(f32, f32)> = vec![(x, delay_y)];
            let mut a_points: Vec<u8> = Vec::new();
            for seg in &sl.segments {
                for sp in &seg.points {
                    let id = sp.zone.to_id();
                    if (1..=8).contains(&id) {
                        a_points.push(id);
                    }
                }
            }
            if a_points.is_empty() {
                waypoints.push((x, tail_y));
            } else {
                let n = a_points.len();
                for (pi, id) in a_points.iter().enumerate() {
                    let frac = (pi + 1) as f32 / n as f32;
                    let wy = ctx.y_of(head + delay_s + (dur_s - delay_s) * frac);
                    let wx = ctx.lane_x(zone_lane_index(*id));
                    waypoints.push((wx, wy));
                }
            }
            let slide_tex = if sl.slide_is_break {
                app.slide_break_tex.as_ref()
            } else if note.is_each {
                app.slide_each_tex.as_ref()
            } else {
                app.slide_tex.as_ref()
            };
            for seg in waypoints.windows(2) {
                let (x0, y0) = seg[0];
                let (x1, y1) = seg[1];
                if let Some(tex) = slide_tex {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let seg_len = (dx * dx + dy * dy).sqrt();
                    let bar_w = 14.0 * s;
                    let tw_nat = tex.width() * SLIDE_TILE_SCALE;
                    let th_nat = tex.height() * SLIDE_TILE_SCALE;
                    let tile_h = th_nat * (bar_w / tw_nat.max(0.001)).max(0.001);
                    let spacing = SLIDE_TILE_SPACING;
                    let angle = dy.atan2(dx) + std::f32::consts::PI;
                    let steps = (seg_len / spacing).ceil().max(1.0) as i32;
                    for k in 0..=steps {
                        let t = k as f32 / steps as f32;
                        let px = x0 + dx * t;
                        let py = y0 + dy * t;
                        tex_at(
                            tex,
                            px - bar_w * 0.5,
                            py - tile_h * 0.5,
                            bar_w,
                            tile_h,
                            angle,
                            ca(255, 255, 255, 230),
                        );
                    }
                } else {
                    mq::draw_line(x0, y0, x1, y1, 3.0 * s, ca(250, 204, 21, 200));
                }
            }
            for wp in &waypoints[1..] {
                mq::draw_circle(wp.0, wp.1, 3.0 * s, ca(253, 224, 71, 200));
            }
        }

        if delay_s > 0.0 || dur_s > 0.0 {
            mq::draw_circle(x, delay_y, 4.5 * s, ca(56, 189, 248, 230));
            mq::draw_circle_lines(x, delay_y, 4.5 * s, 1.5 * s, ca(255, 255, 255, 220));
        }
        if dur_s > 0.0 {
            mq::draw_circle(x, tail_y, 5.5 * s, ca(250, 204, 21, 230));
            mq::draw_circle_lines(x, tail_y, 5.5 * s, 1.5 * s, ca(255, 255, 255, 220));
        }
    }

    if !note.is_tapless {
        let is_double = note.is_star;
        let star_tex = if note.is_break {
            if is_double {
                app.star_double_break_tex.as_ref()
            } else {
                app.star_break_tex.as_ref()
            }
        } else if note.is_each {
            if is_double {
                app.star_double_each_tex.as_ref()
            } else {
                app.star_each_tex.as_ref()
            }
        } else if is_double {
            app.star_double_tex.as_ref()
        } else {
            app.star_tex.as_ref()
        };
        let fallback = if is_double {
            app.star_double_tex.as_ref()
        } else {
            app.star_tex.as_ref()
        };
        let ss = STAR_SIZE * s;
        if let Some(tex) = star_tex.or(fallback).or(app.star_tex.as_ref()) {
            draw_tex(tex, x, y, ss, ss, ca(255, 255, 255, 230));
            if note.is_ex {
                let ex = if is_double {
                    app.star_double_ex_tex.as_ref()
                } else {
                    app.star_ex_tex.as_ref()
                }
                .or(app.star_ex_tex.as_ref());
                if let Some(ex) = ex {
                    draw_tex(ex, x, y, ss, ss, ca(255, 255, 255, 230));
                }
            }
        } else {
            mq::draw_poly(x, y, 5, ss * 0.4, 0.0, ca(250, 204, 21, 230));
        }
    }
}

fn draw_waveform_vertical(
    waveform: &WaveformPyramid,
    plot_left: f32,
    plot_right: f32,
    plot_top: f32,
    plot_bottom: f32,
    judge_y: f32,
    now: f32,
    pps: f32,
    scale: f32,
) {
    if waveform.levels.is_empty() {
        return;
    }
    let level = waveform
        .levels
        .iter()
        .rev()
        .find(|l| waveform.duration / l.min_max.len().max(1) as f32 * pps < 2.0 * scale)
        .or_else(|| waveform.levels.first())
        .unwrap();
    let bins = level.min_max.len().max(1);
    let bin_duration = waveform.duration / bins as f32;
    if bin_duration <= 0.0 {
        return;
    }
    let center = (plot_left + plot_right) * 0.5;
    let half = (plot_right - plot_left) * 0.5 * 0.45;
    let t_top = now + (judge_y - plot_top) / pps;
    let t_bottom = now + (judge_y - plot_bottom) / pps;
    let first = ((t_bottom.min(t_top) / bin_duration).floor() as isize - 1).max(0) as usize;
    let last =
        (((t_bottom.max(t_top)) / bin_duration).ceil() as usize + 1).min(bins);
    for index in first..last {
        let y = judge_y - (index as f32 * bin_duration - now) * pps;
        if y < plot_top - 1.0 || y > plot_bottom + 1.0 {
            continue;
        }
        let [lo, hi] = level.min_max[index];
        mq::draw_line(
            center - hi * half,
            y,
            center - lo * half,
            y,
            1.0 * scale,
            ca(90, 141, 186, 130),
        );
    }
}

/// Vertical 9-slice of the (vertical) hold texture: fixed top/bottom caps,
/// stretched middle. `y0..y1` are the head/tail judgment centers.
fn draw_hold_9slice_vertical(tex: &Texture2D, cx: f32, y0: f32, y1: f32, width: f32, tint: Color) {
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_dest = (cap_h * (width / tex_w)).max(1.0);

    let top_center = y0.min(y1);
    let bottom_center = y0.max(y1);
    let center_len = (bottom_center - top_center).max(1.0);
    let (top_h, bottom_h) = if center_len < cap_dest * 2.0 {
        let squeezed = (center_len * 0.5).max(1.0);
        (squeezed, squeezed)
    } else {
        (cap_dest, cap_dest)
    };
    let top_y = top_center - top_h * 0.5;
    let bottom_y = bottom_center - bottom_h * 0.5;
    let body_y = top_y + top_h;
    let body_h = (bottom_y - body_y).max(0.0);
    let x = cx - width * 0.5;

    mq::draw_texture_ex(
        tex,
        x,
        top_y,
        tint,
        DrawTextureParams {
            source: Some(MqRect {
                x: 0.0,
                y: 0.0,
                w: tex_w,
                h: cap_h,
            }),
            dest_size: Some(mq::vec2(width, top_h)),
            ..Default::default()
        },
    );
    if body_h > 0.0 {
        mq::draw_texture_ex(
            tex,
            x,
            body_y,
            tint,
            DrawTextureParams {
                source: Some(MqRect {
                    x: 0.0,
                    y: cap_h,
                    w: tex_w,
                    h: body_src_h,
                }),
                dest_size: Some(mq::vec2(width, body_h)),
                ..Default::default()
            },
        );
    }
    mq::draw_texture_ex(
        tex,
        x,
        bottom_y,
        tint,
        DrawTextureParams {
            source: Some(MqRect {
                x: 0.0,
                y: tex_h - cap_h,
                w: tex_w,
                h: cap_h,
            }),
            dest_size: Some(mq::vec2(width, bottom_h)),
            ..Default::default()
        },
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Overview minimap (egui)
// ─────────────────────────────────────────────────────────────────────────────

pub fn draw_overview(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    chart: &ChartDoc,
    current_time: f32,
) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), OVERVIEW_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(43, 43, 45));

    // Align with the timeline plot: the timeline sits right of the tool column.
    let plot_left = rect.left() + crate::editor_egui::TOOLBAR_W + LANE_GUTTER;
    let plot_right = rect.right();
    let width = (plot_right - plot_left).max(1.0);
    painter.line_segment(
        [
            egui::pos2(plot_left, rect.top()),
            egui::pos2(plot_left, rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(70, 70, 74)),
    );

    let duration = chart
        .notes
        .iter()
        .map(|n| hold_tail_time(n, &chart.bpms).max(note_secs(n, &chart.bpms)))
        .fold(8.0_f32, f32::max);

    let x_of = |t: f32| plot_left + t.clamp(0.0, duration) / duration * width;

    for note in &chart.notes {
        let x = x_of(note_secs(note, &chart.bpms));
        painter.line_segment(
            [
                egui::pos2(x, rect.top() + 5.0),
                egui::pos2(x, rect.bottom() - 5.0),
            ],
            egui::Stroke::new(1.0_f32, note_tick_color(note.note_type)),
        );
    }

    // The red playhead is the only handle: drag it (anywhere on the bar) to
    // seek the player, instead of moving a view block.
    let play_x = x_of(current_time);
    painter.vline(
        play_x,
        rect.y_range(),
        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 59, 80)),
    );

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let frac = ((pos.x - plot_left) / width).clamp(0.0, 1.0);
            editor.pending_seek = Some(frac * duration);
            editor.scrubbing = true;
        }
    }
    if response.drag_stopped() {
        editor.scrubbing = false;
    }
}
