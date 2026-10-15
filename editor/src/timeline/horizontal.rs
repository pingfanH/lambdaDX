//! Horizontal timeline renderer (macroquad): time flows to the right.
//!
//! This is the legacy ported layout, kept as a secondary orientation (the design
//! and default is vertical). It shares all note recipes with the vertical
//! renderer via [`super::notes`].

use macroquad::prelude as mq;
use macroquad::prelude::Rect as MqRect;

use crate::app::types::{
    hold_tail_time, mdur_to_secs, measure_to_secs, note_secs, secs_to_measure, slide_end_time,
    BpmChange, ChartDoc, Note, NoteType,
};
use crate::audio::WaveformPyramid;
use crate::state::EditorState;
use crate::player::state::PadPreviewState;

use super::geometry::{lane_index_of, zone_lane_index, LANE_COUNT, LANE_GUTTER, RULER_H};
use super::notes as draw;
use super::waveform;

/// Per-frame context for placing a note on the horizontal time axis.
struct NoteCtx<'a> {
    app: &'a PadPreviewState,
    bpms: &'a [BpmChange],
    plot_left: f32,
    pps: f32,
    view: f32,
    lanes_top: f32,
    lane_h: f32,
    scale: f32,
    /// Extra multiplier on note glyph sizes (F1 `editor_note_scale`).
    note_scale: f32,
    tint: mq::Color,
}

impl NoteCtx<'_> {
    fn x_of(&self, t: f32) -> f32 {
        self.plot_left + (t - self.view) * self.pps
    }
    fn lane_y(&self, lane_index: usize) -> f32 {
        self.lanes_top + (lane_index as f32 + 0.5) * self.lane_h
    }
    /// Note glyph scale: the frame scale times the live F1 size multiplier.
    fn note_s(&self) -> f32 {
        self.scale * self.note_scale
    }
    fn color(&self, color: mq::Color) -> mq::Color { draw::tint(color, self.tint) }
}

/// Timeline note-size multiplier from the F1 params (clamped to a sane range).
fn note_scale() -> f32 {
    crate::app::params::get().editor_note_scale.clamp(0.05, 8.0)
}

pub fn draw(
    app: &PadPreviewState,
    editor: &EditorState,
    chart: &ChartDoc,
    rect: MqRect,
    current_time: f32,
    waveform: &WaveformPyramid,
    scale: f32,
) {
    // `g` = geometry scale (egui points → macroquad logical), used for anything
    // positional; `s` = content scale for glyph sizes / strokes / fonts (kept at
    // logical 1.0 so notes don't shrink on HiDPI).
    let g = scale.max(0.1);
    let s = 1.0_f32;
    let bpms = &chart.bpms;
    let gutter = LANE_GUTTER * g;
    let ruler_h = RULER_H * g;
    let plot_left = rect.x + gutter;
    let plot_right = rect.x + rect.w;
    let lanes_top = rect.y + ruler_h;
    let lanes_bottom = rect.y + rect.h;
    let lanes_h = (lanes_bottom - lanes_top).max(1.0);
    let lane_h = lanes_h / LANE_COUNT as f32;
    let pps = editor.pixels_per_second * g;
    let view = editor.view_start;

    mq::draw_rectangle(rect.x, rect.y, rect.w, rect.h, draw::c(24, 24, 27));
    mq::draw_rectangle(rect.x, rect.y, rect.w, ruler_h, draw::c(42, 42, 45));
    mq::draw_rectangle(rect.x, lanes_top, gutter, lanes_h, draw::c(42, 42, 45));

    let view_end = view + (plot_right - plot_left) / pps;
    waveform::draw_horizontal(waveform, plot_left, plot_right, lanes_top, lanes_h, view, pps, s, chart.audio_offset);

    draw_measure_grid(rect, plot_left, plot_right, lanes_top, lanes_bottom, view, view_end, bpms, pps, g, s, editor.snap_division);
    draw_lanes(rect, plot_left, plot_right, lanes_top, lanes_bottom, lane_h, g, s);
    super::judgment::draw(app, chart, super::judgment::Axis {
        plot: MqRect::new(plot_left, lanes_top, plot_right - plot_left, lanes_h),
        vertical: false, origin: plot_left, reference: view, pps, lane_size: lane_h,
    });

    let mut ctx = NoteCtx {
        app,
        bpms,
        plot_left,
        pps,
        view,
        lanes_top,
        lane_h,
        scale: s,
        note_scale: note_scale(),
        tint: mq::WHITE,
    };
    let backdrop = editor.template_session.as_ref().map(|session| session.backdrop.notes.as_slice()).unwrap_or(&[]);
    for (notes, is_backdrop) in [(backdrop, true), (chart.notes.as_slice(), false)] {
    for note in super::notes::draw_order(notes) {
        ctx.tint = draw::note_tint(note, is_backdrop);
        let head = note_secs(note, bpms);
        let x = ctx.x_of(head);
        let y = ctx.lane_y(lane_index_of(note));
        let tail = match note.note_type {
            NoteType::Hold | NoteType::TouchHold => hold_tail_time(note, bpms),
            NoteType::Slide => slide_end_time(note, bpms),
            _ => head,
        };
        let x_tail = ctx.x_of(tail);
        if x_tail < plot_left - 80.0 * g || x > plot_right + 80.0 * g {
            continue;
        }
        draw_note(&ctx, note, x, x_tail, y, !is_backdrop && editor.selected.contains(&note.id));
    }
    }

    let px = ctx.x_of(current_time);
    if px >= plot_left - 2.0 * g && px <= plot_right + 2.0 * g {
        mq::draw_line(px, rect.y, px, lanes_bottom, 2.0 * s, draw::c(208, 70, 70));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_measure_grid(
    rect: MqRect,
    plot_left: f32,
    plot_right: f32,
    lanes_top: f32,
    lanes_bottom: f32,
    view: f32,
    view_end: f32,
    bpms: &[BpmChange],
    pps: f32,
    g: f32,
    s: f32,
    division: u32,
) {
    let subdiv = division.max(1);
    let start_m = secs_to_measure(view, bpms).floor();
    let end_m = secs_to_measure(view_end, bpms) + 1.0;
    let mut m = start_m;
    while m <= end_m {
        for i in 0..subdiv {
            let x = plot_left + (measure_to_secs(m + i as f32 / subdiv as f32, bpms) - view) * pps;
            if x < plot_left - 1.0 || x > plot_right + 1.0 {
                continue;
            }
            let (col, th) = if i == 0 {
                (draw::c(140, 50, 50), 2.0 * s)
            } else if i * 4 % subdiv == 0 {
                (draw::c(80, 80, 80), 1.5 * s)
            } else {
                (draw::c(40, 40, 40), 0.5 * s)
            };
            mq::draw_line(x, lanes_top, x, lanes_bottom, th, col);
        }
        let x = plot_left + (measure_to_secs(m, bpms) - view) * pps;
        if x >= plot_left - 40.0 * g && x <= plot_right {
            mq::draw_text(
                &format!("{}", m as i32),
                x + 3.0 * g,
                rect.y + 15.0 * g,
                14.0 * s,
                draw::c(140, 140, 140),
            );
        }
        m += 1.0;
    }
}

fn draw_lanes(
    rect: MqRect,
    plot_left: f32,
    plot_right: f32,
    lanes_top: f32,
    lanes_bottom: f32,
    lane_h: f32,
    g: f32,
    s: f32,
) {
    for lane in 0..=LANE_COUNT {
        let y = lanes_top + lane as f32 * lane_h;
        let (col, th) = if lane == 4 {
            (draw::c(90, 90, 90), 2.0 * s)
        } else {
            (draw::c(55, 55, 55), 1.0 * s)
        };
        mq::draw_line(plot_left, y, plot_right, y, th, col);
    }
    let _ = lanes_bottom;
    for lane in 0..LANE_COUNT {
        let y = lanes_top + (lane as f32 + 0.5) * lane_h + 5.0 * g;
        let (label, col) = if lane == LANE_COUNT - 1 {
            ("T".to_string(), draw::c(230, 149, 48))
        } else {
            ((lane + 1).to_string(), draw::c(180, 180, 180))
        };
        mq::draw_text(&label, rect.x + 10.0 * g, y, 15.0 * s, col);
    }
}

fn draw_note(ctx: &NoteCtx, note: &Note, x: f32, x_tail: f32, y: f32, selected: bool) {
    let app = ctx.app;
    let s = ctx.note_s();
    let tap = draw::TAP_SIZE * s;
    let hold_w = draw::HOLD_WIDTH * s;
    if selected {
        draw::selection_outline(note, x, y, s);
    }
    match note.note_type {
        NoteType::Tap => {
            if let Some(t) = draw::tap_tex(app, note) {
                draw::draw_tex(t, x, y, tap, tap, ctx.tint);
            } else {
                draw::fallback_tap_tinted(x, y, tap, ctx.tint);
            }
            if note.is_ex {
                if let Some(ex) = app.tap_ex_tex.as_ref() {
                    draw::draw_tex(ex, x, y, tap, tap, ctx.tint);
                }
            }
            mq::draw_circle(x, y, 2.5 * s, ctx.color(draw::ca(255, 255, 255, 200)));
        }
        NoteType::Hold | NoteType::TouchHold => {
            let (x0, x1) = (x.min(x_tail), x.max(x_tail));
            match draw::hold_tex(app, note) {
                Some(t) => {
                    draw::draw_hold_9slice_horizontal(t, x0, x1, y, hold_w, ctx.tint);
                    if note.is_ex {
                        if let Some(ex) = app.hold_ex_tex.as_ref() {
                            draw::draw_hold_9slice_horizontal(ex, x0, x1, y, hold_w, ctx.tint);
                        }
                    }
                }
                None => {
                    let top = y - hold_w * 0.2;
                    let h = hold_w * 0.4;
                    let w = (x1 - x0).max(hold_w * 0.133);
                    mq::draw_rectangle(x0, top, w, h, ctx.color(draw::ca(190, 24, 93, 130)));
                    mq::draw_rectangle_lines(x0, top, w, h, 1.0 * s, ctx.color(draw::ca(244, 114, 182, 200)));
                }
            }
            mq::draw_circle(x0, y, 2.5 * s, ctx.color(draw::ca(255, 255, 255, 200)));
            mq::draw_circle(x1, y, 2.5 * s, ctx.color(draw::ca(255, 255, 255, 180)));
        }
        NoteType::Touch => {
            draw::draw_touch_tinted(app, s, note, x, y, ctx.tint);
        }
        NoteType::Slide => {
            draw_slide(ctx, note, x, y);
        }
    }
}

fn draw_slide(ctx: &NoteCtx, note: &Note, x: f32, y: f32) {
    let app = ctx.app;
    let bpms = ctx.bpms;
    let s = ctx.note_s();
    let head = note_secs(note, bpms);
    for sl in &note.slide {
        let dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.0);
        let delay_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
            .max(0.0)
            .min(dur_s);
        let delay_x = ctx.x_of(head + delay_s);
        let tail_x = ctx.x_of(head + dur_s);

        draw_delay_dash(x, delay_x, y, delay_s, s, ctx.tint);

        let travel = (tail_x - delay_x).abs();
        if travel > 0.5 {
            let waypoints = slide_waypoints(ctx, sl, head, delay_s, dur_s, delay_x, y, tail_x);
            let texture = draw::slide_tex(app, note, sl);
            for seg in waypoints.windows(2) {
                draw_trail_segment(texture, seg[0], seg[1], s, ctx.tint);
            }
            for wp in &waypoints[1..] {
                mq::draw_circle(wp.0, wp.1, 3.0 * s, ctx.color(draw::ca(253, 224, 71, 200)));
            }
        }

        if delay_s > 0.0 || dur_s > 0.0 {
            mq::draw_circle(delay_x, y, 4.5 * s, ctx.color(draw::ca(56, 189, 248, 230)));
            mq::draw_circle_lines(delay_x, y, 4.5 * s, 1.5 * s, ctx.color(draw::ca(255, 255, 255, 220)));
        }
        if dur_s > 0.0 {
            let end_y = sl.segments.iter().flat_map(|s| &s.points).last()
                .map(|p| ctx.lane_y(zone_lane_index(p.zone.to_id()))).unwrap_or(y);
            mq::draw_circle(tail_x, end_y, 5.5 * s, ctx.color(draw::ca(250, 204, 21, 230)));
            mq::draw_circle_lines(tail_x, end_y, 5.5 * s, 1.5 * s, ctx.color(draw::ca(255, 255, 255, 220)));
        }
    }

    if !note.is_tapless {
        if let Some(tex) = draw::star_tex(app, note) {
            let ss = draw::STAR_SIZE * s;
            draw::draw_tex(tex, x, y, ss, ss, ctx.color(draw::ca(255, 255, 255, 230)));
            if note.is_ex {
                let double = note.is_star;
                let ex = if double {
                    app.star_double_ex_tex.as_ref()
                } else {
                    app.star_ex_tex.as_ref()
                }
                .or(app.star_ex_tex.as_ref());
                if let Some(ex) = ex {
                    draw::draw_tex(ex, x, y, ss, ss, ctx.color(draw::ca(255, 255, 255, 230)));
                }
            }
        } else {
            let ss = draw::STAR_SIZE * s;
            mq::draw_poly(x, y, 5, ss * 0.4, 0.0, ctx.color(draw::ca(250, 204, 21, 230)));
        }
    }
}

fn draw_delay_dash(x: f32, delay_x: f32, y: f32, delay_s: f32, s: f32, tint: mq::Color) {
    let delay_len = (delay_x - x).abs();
    if delay_s <= 0.0 || delay_len <= 0.5 {
        return;
    }
    let dash = 6.0 * s;
    let gap = 4.0 * s;
    let period = dash + gap;
    let left = x.min(delay_x);
    let n = (delay_len / period).ceil() as i32;
    let col = draw::tint(draw::ca(253, 224, 71, 220), tint);
    for k in 0..n {
        let x0 = left + k as f32 * period;
        let x1 = (x0 + dash).min(left + delay_len);
        mq::draw_line(x0, y, x1, y, 2.0 * s, col);
    }
}

#[allow(clippy::too_many_arguments)]
fn slide_waypoints(
    ctx: &NoteCtx,
    sl: &crate::app::types::Slide,
    head: f32,
    delay_s: f32,
    dur_s: f32,
    delay_x: f32,
    y: f32,
    tail_x: f32,
) -> Vec<(f32, f32)> {
    let mut waypoints = vec![(delay_x, y)];
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
    waypoints
}

fn draw_trail_segment(
    texture: Option<&macroquad::prelude::Texture2D>,
    a: (f32, f32),
    b: (f32, f32),
    s: f32,
    tint: mq::Color,
) {
    let (x0, y0) = a;
    let (x1, y1) = b;
    let Some(tex) = texture else {
        mq::draw_line(x0, y0, x1, y1, 3.0 * s, draw::tint(draw::ca(250, 204, 21, 200), tint));
        return;
    };
    let (dx, dy) = (x1 - x0, y1 - y0);
    let seg_len = (dx * dx + dy * dy).sqrt();
    let bar_w = 14.0 * s;
    let tw_nat = tex.width() * draw::SLIDE_TILE_SCALE;
    let th_nat = tex.height() * draw::SLIDE_TILE_SCALE;
    let tile_h = th_nat * (bar_w / tw_nat.max(0.001)).max(0.001);
    let spacing = draw::SLIDE_TILE_SPACING * s;
    let angle = dy.atan2(dx) + std::f32::consts::PI;
    let steps = (seg_len / spacing).ceil().max(1.0) as i32;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let px = x0 + dx * t;
        let py = y0 + dy * t;
        draw::tex_at(
            tex,
            px - bar_w * 0.5,
            py - tile_h * 0.5,
            bar_w,
            tile_h,
            angle,
            draw::tint(draw::ca(255, 255, 255, 230), tint),
        );
    }
}
