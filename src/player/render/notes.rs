//! Note pass: iterate the chart, derive timing, cull, and dispatch each note to
//! the appropriate kind-specific renderer (`ring`, `touch`, `slide`).

use macroquad::color::Color;
use macroquad::math::Vec2;
use macroquad::prelude::draw_circle;

use crate::app::params;
use crate::app::slide_render::SlideLayer;
use crate::app::types::{Note, NoteType, PadGeom};
use crate::player::render::timing::{self, NoteTiming};
use crate::player::render::{ring, slide, touch};
use crate::player::state::PadPreviewState;

/// Draw all visible notes for `current_t` (seconds).
///
/// Two passes so the stacking is type-aware:
/// 1. every slide note — trail + flying star, plus the on-hit ring (behind);
/// 2. every non-slide note (tap / hold / touch) — ring or touch art (in front).
///
/// Notes therefore always sit **above** slides. `note_earlier_on_top` only
/// orders items *within* each pass (slide vs slide, note vs note); it no longer
/// interleaves the two kinds.
pub fn draw_notes(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    current_t: f32,
    speed_scale: f32,
) {
    // Guides first, so every guide sits under every note sprite.
    draw_guide_pass(app, pad, scale, spawn_cx, current_t, speed_scale);
    draw_pass(app, pad, scale, spawn_cx, current_t, speed_scale, true);
    draw_pass(app, pad, scale, spawn_cx, current_t, speed_scale, false);
}

/// Draw only the tap / hold guides (slides draw theirs in the trail layer).
fn draw_guide_pass(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    current_t: f32,
    speed_scale: f32,
) {
    let bpms = &app.chart.bpms;
    for note in app.chart.notes.iter() {
        if matches!(note.note_type, NoteType::Slide) {
            continue;
        }
        if app.hidden_notes.contains(&note.id) {
            continue;
        }
        let t: NoteTiming = timing::compute(note, app, bpms, current_t, speed_scale);
        if !t.visible() {
            continue;
        }
        ring::draw_guides(app, note, &t, scale, spawn_cx, pad.outer_r);
    }
}

/// Draw a black dot at every note's judgment point, on the topmost layer.
pub fn draw_judge_dots(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    current_t: f32,
    speed_scale: f32,
) {
    if !params::judge_dot() {
        return;
    }
    let bpms = &app.chart.bpms;
    let r = params::judge_dot_size() * scale;
    if r <= 0.0 {
        return;
    }
    let color = Color::from_rgba(0, 0, 0, 255);
    for note in app.chart.notes.iter() {
        if app.hidden_notes.contains(&note.id) {
            continue;
        }
        let t: NoteTiming = timing::compute(note, app, bpms, current_t, speed_scale);
        if !t.visible() {
            continue;
        }
        for p in ring::judge_points(note, &t, scale, spawn_cx, pad.outer_r) {
            draw_circle(p.x, p.y, r, color);
        }
    }
}

/// One stacking pass. `slides == true` draws slide notes first (behind);
/// `slides == false` draws non-slide notes on top.
///
/// Within the slide pass the **trails are drawn before any star** (two loops),
/// so an overlapping slide's trail can never hide another slide's star — e.g. a
/// QQ and a PP at the same time, or any two crossing slides.
fn draw_pass(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    current_t: f32,
    speed_scale: f32,
    slides: bool,
) {
    let bpms = &app.chart.bpms;
    // Forward: later notes on top. Reverse: earlier notes on top. Applied
    // within the pass only.
    let notes: Box<dyn Iterator<Item = &Note>> = if params::note_earlier_on_top() {
        Box::new(app.chart.notes.iter().rev())
    } else {
        Box::new(app.chart.notes.iter())
    };

    // Resolve visibility once, then draw per layer.
    let mut visible: Vec<(&Note, NoteTiming)> = Vec::new();
    for note in notes {
        let is_slide = matches!(note.note_type, NoteType::Slide);
        if is_slide != slides {
            continue;
        }
        if app.hidden_notes.contains(&note.id) {
            continue;
        }
        let t: NoteTiming = timing::compute(note, app, bpms, current_t, speed_scale);
        if !t.visible() {
            continue;
        }
        visible.push((note, t));
    }

    if slides {
        // Trails (plus each slide's on-hit ring) first...
        for (note, t) in &visible {
            slide::draw(
                app,
                note,
                pad,
                scale,
                spawn_cx,
                pad.outer_r,
                current_t,
                t,
                SlideLayer::Trail,
            );
            if t.zone <= 8 {
                ring::draw(app, note, t, scale, spawn_cx, pad.outer_r);
            }
        }
        // ...then every star, so no trail covers another slide's star.
        for (note, t) in &visible {
            slide::draw(
                app,
                note,
                pad,
                scale,
                spawn_cx,
                pad.outer_r,
                current_t,
                t,
                SlideLayer::Star,
            );
        }
    } else {
        for (note, t) in &visible {
            if t.zone <= 8 {
                ring::draw(app, note, t, scale, spawn_cx, pad.outer_r);
            } else {
                touch::draw(app, note, t, pad, scale, current_t);
            }
        }
    }
}
