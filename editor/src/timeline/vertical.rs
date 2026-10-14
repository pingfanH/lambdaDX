//! Vertical timeline renderer (macroquad).
//!
//! Lanes are columns, time flows upward from the judgment line near the bottom
//! (future above, past below), mirroring the authored design. Rendering is done
//! with macroquad so note textures from [`PadPreviewState`] can be drawn
//! directly; the transparent egui central panel only hosts interaction.

use macroquad::prelude as mq;
use macroquad::prelude::Rect as MqRect;

use crate::app::types::{
    hold_tail_time, mdur_to_secs, measure_to_secs, note_secs, secs_to_measure, slide_end_time,
    BpmChange, ChartDoc, Note, NoteType,
};
use crate::audio::WaveformPyramid;
use crate::state::EditorState;
use crate::theme;
use crate::player::state::PadPreviewState;

use super::geometry::{lane_index_of, zone_lane_index, LANE_COUNT, LANE_GUTTER, RULER_H, JUDGE_MARGIN};
use super::notes as draw;
use super::waveform;

/// Per-frame context for placing a note on the vertical time axis.
struct VNoteCtx<'a> {
    app: &'a PadPreviewState,
    bpms: &'a [BpmChange],
    judge_y: f32,
    pps: f32,
    now: f32,
    plot_left: f32,
    lane_w: f32,
    scale: f32,
    /// Extra multiplier on note glyph sizes (F1 `editor_note_scale`).
    note_scale: f32,
}

impl VNoteCtx<'_> {
    /// Screen y for a time: future above the judge line, past below.
    fn y_of(&self, t: f32) -> f32 {
        self.judge_y - (t - self.now) * self.pps
    }
    fn lane_x(&self, lane_index: usize) -> f32 {
        self.plot_left + (lane_index as f32 + 0.5) * self.lane_w
    }
    /// Note glyph scale: the frame scale times the live F1 size multiplier.
    fn note_s(&self) -> f32 {
        self.scale * self.note_scale
    }
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
    // Two scales: `g` converts egui points to macroquad logical units and MUST
    // be used for anything positional (gutter, ruler, judge line, px/sec) so the
    // drawing lines up with the egui-space input hit-testing. `s` is the content
    // scale for glyph sizes / strokes / fonts, kept at 1.0 logical so notes do
    // not shrink on a HiDPI display (`to_mq` would be 0.5 there).
    let g = scale.max(0.1);
    let s = 1.0_f32;
    let bpms = &chart.bpms;
    let header_h = RULER_H * g;
    let left_margin = LANE_GUTTER * g;
    let plot_left = rect.x + left_margin;
    let plot_right = rect.x + rect.w;
    let plot_top = rect.y + header_h;
    let plot_bottom = rect.y + rect.h;
    let judge_y = plot_bottom - JUDGE_MARGIN * g;
    let lane_w = (plot_right - plot_left).max(1.0) / LANE_COUNT as f32;
    // The rect is in macroquad logical units (egui points × `g`), so the time
    // axis must be scaled by the same `g` — otherwise notes drift from where the
    // egui input math (which works in egui points) expects them.
    let pps = editor.pixels_per_second * g;
    let now = current_time;
    let y_of = |t: f32| judge_y - (t - now) * pps;
    let lane_x = |i: usize| plot_left + (i as f32 + 0.5) * lane_w;

    draw_background(rect, header_h, plot_top, plot_bottom, left_margin);
    waveform::draw_vertical(
        waveform, plot_left, plot_right, plot_top, plot_bottom, judge_y, now, pps, s,
        chart.audio_offset,
    );
    // Time at the plot's top/bottom edges: time flows up, so the bottom edge is
    // earlier. `draw_measure_grid` needs these in *seconds* (not pixels).
    let t_top = now + (judge_y - plot_top) / pps;
    let t_bottom = now + (judge_y - plot_bottom) / pps;
    draw_measure_grid(
        rect,
        plot_left,
        plot_right,
        plot_top,
        plot_bottom,
        t_bottom.min(t_top),
        t_bottom.max(t_top),
        bpms,
        y_of,
        g,
        s,
        editor.snap_division,
    );
    draw_lanes(plot_left, plot_top, plot_bottom, lane_w, rect.y + 16.0 * g, lane_x, s);

    let ctx = VNoteCtx {
        app,
        bpms,
        judge_y,
        pps,
        now,
        plot_left,
        lane_w,
        scale: s,
        note_scale: note_scale(),
    };
    for note in super::notes::draw_order(&chart.notes) {
        let head = note_secs(note, bpms);
        let y = ctx.y_of(head);
        let x = ctx.lane_x(lane_index_of(note));
        let tail = match note.note_type {
            NoteType::Hold | NoteType::TouchHold => hold_tail_time(note, bpms),
            NoteType::Slide => slide_end_time(note, bpms),
            _ => head,
        };
        let y_tail = ctx.y_of(tail);
        let (ya, yb) = (y.min(y_tail), y.max(y_tail));
        if yb < plot_top - 80.0 * g || ya > plot_bottom + 80.0 * g {
            continue;
        }
        draw_note(&ctx, note, x, y, y_tail, editor.selected.contains(&note.id));
    }

    // Judgment line doubles as the "now" playhead (design: 1px `#FF3B50`).
    mq::draw_line(plot_left, judge_y, plot_right, judge_y, 1.0 * s, theme::to_mq(theme::JUDGE_LINE));
}

fn draw_background(rect: MqRect, header_h: f32, plot_top: f32, plot_bottom: f32, left_margin: f32) {
    // Design: the whole timeline shares the workspace background (`#1D1D1E`),
    // with a hairline under the ruler and a left edge line.
    let bg = theme::to_mq(theme::BG_WINDOW);
    mq::draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    mq::draw_rectangle(rect.x, plot_top, left_margin, (plot_bottom - plot_top).max(0.0), bg);
    mq::draw_line(
        rect.x,
        rect.y + header_h,
        rect.x + rect.w,
        rect.y + header_h,
        1.0,
        theme::to_mq(theme::BORDER_LIGHT),
    );
}

/// Measure grid uses the placement/drag subdivision (bar / beat / sub). `g` is the
/// geometry scale (label offsets), `s` the content scale (strokes / font).
#[allow(clippy::too_many_arguments)]
fn draw_measure_grid(
    rect: MqRect,
    plot_left: f32,
    plot_right: f32,
    plot_top: f32,
    plot_bottom: f32,
    t_lo: f32,
    t_hi: f32,
    bpms: &[BpmChange],
    y_of: impl Fn(f32) -> f32,
    g: f32,
    s: f32,
    division: u32,
) {
    let subdiv = division.max(1);
    let start_m = secs_to_measure(t_lo, bpms).floor();
    let end_m = secs_to_measure(t_hi, bpms) + 1.0;
    let mut m = start_m;
    while m <= end_m {
        for i in 0..subdiv {
            let y = y_of(measure_to_secs(m + i as f32 / subdiv as f32, bpms));
            if y < plot_top - 1.0 || y > plot_bottom + 1.0 {
                continue;
            }
            let (col, th) = if i == 0 {
                (theme::to_mq(theme::GRID_BEAT), 1.2 * s)
            } else if i * 4 % subdiv == 0 {
                (theme::to_mq(theme::GRID_LINE), 1.0 * s)
            } else {
                (theme::to_mq(theme::GRID_MINOR), 0.6 * s)
            };
            mq::draw_line(plot_left, y, plot_right, y, th, col);
        }
        let y = y_of(measure_to_secs(m, bpms));
        if y >= plot_top && y <= plot_bottom + 40.0 * g {
            mq::draw_text(
                &format!("{}", m as i32),
                rect.x + 4.0 * g,
                y + 4.0 * g,
                14.0 * s,
                theme::to_mq(theme::TEXT_RULER),
            );
        }
        m += 1.0;
    }
}

/// Lane separators as vertical lines plus the header lane labels (`1..8`, `T`).
fn draw_lanes(
    plot_left: f32,
    plot_top: f32,
    plot_bottom: f32,
    lane_w: f32,
    label_y: f32,
    lane_x: impl Fn(usize) -> f32,
    s: f32,
) {
    for lane in 0..=LANE_COUNT {
        let x = plot_left + lane as f32 * lane_w;
        let (col, th) = if lane == LANE_COUNT / 2 {
            (theme::to_mq(theme::GRID_LANE_CENTER), 1.0 * s)
        } else {
            (theme::to_mq(theme::GRID_LANE), 1.0 * s)
        };
        mq::draw_line(x, plot_top, x, plot_bottom, th, col);
    }
    for lane in 0..LANE_COUNT {
        let (label, col) = if lane == LANE_COUNT - 1 {
            ("T".to_string(), draw::c(230, 149, 48))
        } else {
            ((lane + 1).to_string(), theme::to_mq(theme::TEXT_RULER))
        };
        let tw = mq::measure_text(&label, None, (15.0 * s) as u16, 1.0).width;
        mq::draw_text(&label, lane_x(lane) - tw * 0.5, label_y, 15.0 * s, col);
    }
}

fn draw_note(ctx: &VNoteCtx, note: &Note, x: f32, y: f32, y_tail: f32, selected: bool) {
    let app = ctx.app;
    let s = ctx.note_s();
    let tap = draw::TAP_SIZE * s;
    let hold_w = draw::HOLD_WIDTH * s;
    if selected {
        draw::selection_outline(note, x, y, s);
    }
    match note.note_type {
        NoteType::Tap => {
            let tex = draw::tap_tex(app, note);
            if let Some(t) = tex {
                draw::draw_tex(t, x, y, tap, tap, mq::WHITE);
            } else {
                draw::fallback_tap(x, y, tap);
            }
            if note.is_ex {
                if let Some(ex) = app.tap_ex_tex.as_ref() {
                    draw::draw_tex(ex, x, y, tap, tap, mq::WHITE);
                }
            }
            mq::draw_circle(x, y, 2.5 * s, draw::ca(255, 255, 255, 200));
        }
        NoteType::Hold | NoteType::TouchHold => {
            draw_hold(ctx, note, x, y, y_tail, hold_w);
        }
        NoteType::Touch => {
            draw::draw_touch(app, s, note, x, y);
        }
        NoteType::Slide => {
            draw_slide(ctx, note, x, y);
        }
    }
}

fn draw_hold(ctx: &VNoteCtx, note: &Note, x: f32, y0: f32, y1: f32, hold_w: f32) {
    let app = ctx.app;
    let s = ctx.note_s();
    match draw::hold_tex(app, note) {
        Some(t) => {
            draw::draw_hold_9slice_vertical(t, x, y0, y1, hold_w, mq::WHITE);
            if note.is_ex {
                if let Some(ex) = app.hold_ex_tex.as_ref() {
                    draw::draw_hold_9slice_vertical(ex, x, y0, y1, hold_w, mq::WHITE);
                }
            }
        }
        None => {
            let top = y0.min(y1);
            let len = (y0 - y1).abs().max(hold_w * 0.133);
            mq::draw_rectangle(x - hold_w * 0.2, top, hold_w * 0.4, len, draw::ca(190, 24, 93, 130));
            mq::draw_rectangle_lines(
                x - hold_w * 0.2,
                top,
                hold_w * 0.4,
                len,
                1.0 * s,
                draw::ca(244, 114, 182, 200),
            );
        }
    }
    mq::draw_circle(x, y0, 2.5 * s, draw::ca(255, 255, 255, 200));
    mq::draw_circle(x, y1, 2.5 * s, draw::ca(255, 255, 255, 180));
}

/// Draw one slide: dashed delay segment, tiled trail through the A-zone
/// waypoints, and the star head.
fn draw_slide(ctx: &VNoteCtx, note: &Note, x: f32, y: f32) {
    let app = ctx.app;
    let bpms = ctx.bpms;
    let s = ctx.note_s();
    let head = note_secs(note, bpms);
    for sl in &note.slide {
        let dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.0);
        let delay_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
            .max(0.0)
            .min(dur_s);
        let delay_y = ctx.y_of(head + delay_s);
        let tail_y = ctx.y_of(head + dur_s);

        draw_delay_dash(x, y, delay_y, delay_s, s);

        let travel = (tail_y - delay_y).abs();
        if travel > 0.5 {
            let waypoints = slide_waypoints(ctx, sl, head, delay_s, dur_s, x, delay_y, tail_y);
            let texture = draw::slide_tex(app, note, sl);
            for seg in waypoints.windows(2) {
                draw_trail_segment(texture, seg[0], seg[1], s);
            }
            for wp in &waypoints[1..] {
                mq::draw_circle(wp.0, wp.1, 3.0 * s, draw::ca(253, 224, 71, 200));
            }
        }

        if delay_s > 0.0 || dur_s > 0.0 {
            mq::draw_circle(x, delay_y, 4.5 * s, draw::ca(56, 189, 248, 230));
            mq::draw_circle_lines(x, delay_y, 4.5 * s, 1.5 * s, draw::ca(255, 255, 255, 220));
        }
        if dur_s > 0.0 {
            mq::draw_circle(x, tail_y, 5.5 * s, draw::ca(250, 204, 21, 230));
            mq::draw_circle_lines(x, tail_y, 5.5 * s, 1.5 * s, draw::ca(255, 255, 255, 220));
        }
    }

    if !note.is_tapless {
        if let Some(tex) = draw::star_tex(app, note) {
            let ss = draw::STAR_SIZE * s;
            draw::draw_tex(tex, x, y, ss, ss, draw::ca(255, 255, 255, 230));
            if note.is_ex {
                if let Some(ex) = draw_ex_star(app, note) {
                    draw::draw_tex(ex, x, y, ss, ss, draw::ca(255, 255, 255, 230));
                }
            }
        } else {
            let ss = draw::STAR_SIZE * s;
            mq::draw_poly(x, y, 5, ss * 0.4, 0.0, draw::ca(250, 204, 21, 230));
        }
    }
}

fn draw_ex_star<'a>(app: &'a PadPreviewState, note: &Note) -> Option<&'a macroquad::prelude::Texture2D> {
    let double = note.is_star;
    if double {
        app.star_double_ex_tex.as_ref()
    } else {
        app.star_ex_tex.as_ref()
    }
    .or(app.star_ex_tex.as_ref())
}

fn draw_delay_dash(x: f32, y: f32, delay_y: f32, delay_s: f32, s: f32) {
    let delay_len = (delay_y - y).abs();
    if delay_s <= 0.0 || delay_len <= 0.5 {
        return;
    }
    let dash = 6.0 * s;
    let gap = 4.0 * s;
    let period = dash + gap;
    let top = y.min(delay_y);
    let n = (delay_len / period).ceil() as i32;
    let col = draw::ca(253, 224, 71, 220);
    for k in 0..n {
        let y0 = top + k as f32 * period;
        let y1 = (y0 + dash).min(top + delay_len);
        mq::draw_line(x, y0, x, y1, 2.0 * s, col);
    }
}

/// Waypoints of one slide leg in screen space, starting at the motion start and
/// ending at the tail; intermediate points follow the A-zone ids.
fn slide_waypoints(
    ctx: &VNoteCtx,
    sl: &crate::app::types::Slide,
    head: f32,
    delay_s: f32,
    dur_s: f32,
    x: f32,
    delay_y: f32,
    tail_y: f32,
) -> Vec<(f32, f32)> {
    let mut waypoints = vec![(x, delay_y)];
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
    waypoints
}

fn draw_trail_segment(
    texture: Option<&macroquad::prelude::Texture2D>,
    a: (f32, f32),
    b: (f32, f32),
    s: f32,
) {
    let (x0, y0) = a;
    let (x1, y1) = b;
    let Some(tex) = texture else {
        mq::draw_line(x0, y0, x1, y1, 3.0 * s, draw::ca(250, 204, 21, 200));
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
            draw::ca(255, 255, 255, 230),
        );
    }
}
