//! Note pass: iterate the chart, derive timing, cull, and dispatch each note to
//! the appropriate kind-specific renderer (`ring`, `touch`, `slide`).

use macroquad::color::Color;
use macroquad::math::Vec2;
use macroquad::prelude::draw_circle;

use crate::app::params;
use crate::app::slide_render::SlideLayer;
use crate::app::types::{Note, NoteType, PadGeom, note_is_touch};
use crate::player::render::timing::{self, NoteTiming};
use crate::player::render::{ring, slide, touch};
use crate::player::state::PadPreviewState;

/// How long after a slide's local tail a core-driven slide stays on screen
/// before the fallback cull removes it (covers missed conslides that lnmai-core
/// never reports as hidden).
const CORE_SLIDE_FALLBACK_GRACE: f32 = 0.35;

/// Timing snapshots shared by all note layers in one rendered frame.
pub struct NoteFrame<'a> {
    visible: Vec<(&'a Note, NoteTiming)>,
}

pub fn is_visible(app: &PadPreviewState, note: &Note, t: &NoteTiming) -> bool {
    if let Some((solo, _)) = app.edit_solo_slide { return note.id == solo; }
    if app.hidden_notes.contains(&note.id) { return false; }
    if note.note_type == NoteType::Slide && app.use_core() {
        t.dt_scaled <= t.lead_time && t.slide_tail_dt >= -CORE_SLIDE_FALLBACK_GRACE
    } else {
        t.visible()
    }
}

/// Draw all visible notes for `current_t` (seconds).
///
/// Two passes so the stacking is type-aware:
/// 1. every slide note — trail + flying star, plus the on-hit ring (behind);
/// 2. every non-slide note (tap / hold / touch) — ring or touch art (in front).
///
/// Notes therefore always sit **above** slides. `note_earlier_on_top` only
/// orders items *within* each pass (slide vs slide, note vs note); it no longer
/// interleaves the two kinds.
pub fn draw_notes<'a>(
    app: &'a PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    current_t: f32,
    speed_scale: f32,
) -> NoteFrame<'a> {
    let frame = prepare(app, current_t, speed_scale);
    // Guides first, so every guide sits under every note sprite.
    draw_guide_pass(app, pad, scale, spawn_cx, &frame);
    draw_pass(app, pad, scale, spawn_cx, current_t, &frame, true);
    draw_pass(app, pad, scale, spawn_cx, current_t, &frame, false);
    frame
}

fn prepare<'a>(app: &'a PadPreviewState, current_t: f32, speed_scale: f32) -> NoteFrame<'a> {
    let bpms = &app.chart.bpms;
    let chart = if app.edit_solo_slide.is_some() { &app.chart } else { app.playback_chart() };
    let mut visible = Vec::new();
    for note in &chart.notes {
        if app.edit_solo_slide.is_none() && app.hidden_notes.contains(&note.id) {
            continue;
        }
        let t = timing::compute(note, app, bpms, current_t, speed_scale);
        if is_visible(app, note, &t) {
            visible.push((note, t));
        }
    }
    let earlier_on_top = params::note_earlier_on_top();
    visible.sort_by(|(a, _), (b, _)| crate::app::types::note_draw_order(a, b, earlier_on_top));
    NoteFrame { visible }
}

/// Draw only the tap / hold guides (slides draw theirs in the trail layer).
fn draw_guide_pass(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    frame: &NoteFrame<'_>,
) {
    if app.edit_solo_slide.is_some() {
        return;
    }
    for (note, t) in &frame.visible {
        if matches!(note.note_type, NoteType::Slide) || note_is_touch(note) {
            continue;
        }
        ring::draw_guides(app, note, t, scale, spawn_cx, pad.outer_r);
    }
}

/// Draw a black dot at every note's judgment point, on the topmost layer.
pub fn draw_judge_dots(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    spawn_cx: Vec2,
    frame: &NoteFrame<'_>,
) {
    if !params::judge_dot() {
        return;
    }
    let r = params::judge_dot_size() * scale;
    if r <= 0.0 {
        return;
    }
    let color = Color::from_rgba(0, 0, 0, 255);
    for (note, t) in &frame.visible {
        for p in ring::judge_points(note, t, scale, spawn_cx, pad.outer_r) {
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
    frame: &NoteFrame<'_>,
    slides: bool,
) {
    let visible = frame
        .visible
        .iter()
        .filter(|(note, _)| matches!(note.note_type, NoteType::Slide) == slides);

    if slides {
        // Trails (plus each slide's on-hit ring) first...
        for (note, t) in visible {
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
        for (note, t) in frame
            .visible
            .iter()
            .filter(|(note, _)| matches!(note.note_type, NoteType::Slide))
        {
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
        for (note, t) in frame
            .visible
            .iter()
            .filter(|(note, _)| !matches!(note.note_type, NoteType::Slide))
        {
            // Touch / touch-hold render on the screen zones (A-zone touch notes
            // share zone 1..=8 with the ring but are still touch notes).
            if note_is_touch(note) {
                touch::draw(app, note, t, pad, scale, current_t);
            } else {
                ring::draw(app, note, t, scale, spawn_cx, pad.outer_r);
            }
        }
    }
}
