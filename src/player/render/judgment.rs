//! Timing windows projected with the same motion/path as the visible notes.

use super::{slide, timing};
use crate::app::types::{
    NoteType, PAD_ROTATION_RAD, PadGeom, SLIDE_MIN_DURATION_S, hold_tail_time, mdur_to_secs,
    note_is_touch, note_lock_radius, note_radial_motion, note_radial_motion_continue,
    zone::PadZone,
};
use crate::app::{params, slide_render};
use crate::player::{judge_windows as windows, state::PadPreviewState};
use macroquad::prelude::*;

fn direction(zone: u8) -> Vec2 {
    let angle = -std::f32::consts::FRAC_PI_2
        + PAD_ROTATION_RAD
        + zone.saturating_sub(1) as f32 * std::f32::consts::TAU / 8.0;
    vec2(angle.cos(), angle.sin())
}

fn tick(point: Vec2, normal: Vec2, size: f32, color: Color) {
    let a = point - normal * size;
    let b = point + normal * size;
    draw_line(a.x, a.y, b.x, b.y, 2.0, color);
}

/// A time arc also represents the late portion that is beyond a clamped tail.
fn halo(
    center: Vec2,
    radius: f32,
    at: f32,
    now: f32,
    ranges: &[windows::Interval],
    first_grade: usize,
    scale: f32,
) {
    let outer = ranges[0];
    let span = (outer.late - outer.early).max(0.001);
    let fraction = |offset: f32| ((offset - outer.early) / span).clamp(0.0, 1.0);
    let point = |f: f32| {
        let a = -std::f32::consts::PI * 0.85 + f * std::f32::consts::PI * 1.7;
        center + vec2(a.cos(), a.sin()) * radius
    };
    for (index, range) in ranges.iter().enumerate() {
        let from = fraction(range.early);
        let to = fraction(range.late);
        let path: Vec<_> = (0..=48)
            .map(|i| point(from + (to - from) * i as f32 / 48.0))
            .collect();
        slide_render::draw_polyline_band(
            &path,
            (7.0 - index as f32) * scale,
            windows::color(index + first_grade, 200),
        );
    }
    let zero = point(fraction(0.0));
    tick(
        zero,
        (zero - center).normalize_or_zero(),
        5.0 * scale,
        WHITE,
    );
    if (outer.early..=outer.late).contains(&(now - at)) {
        let position = point(fraction(now - at));
        draw_circle(position.x, position.y, 3.0 * scale, WHITE);
    }
}

pub fn draw(
    app: &PadPreviewState,
    pad: &PadGeom,
    scale: f32,
    center: Vec2,
    now: f32,
    speed_scale: f32,
) {
    if !params::get().show_judge_windows {
        return;
    }
    let p = params::get();
    let [r, g, b] = windows::HOLD_COLOR;
    let hold_color = Color::from_rgba(r, g, b, 235);
    for note in &app.playback_chart().notes {
        let solo = app.edit_solo_slide;
        if solo.is_some_and(|(id, _)| id != note.id) {
            continue;
        }
        if solo.is_none() && app.hidden_notes.contains(&note.id) {
            continue;
        }
        let t = timing::compute(note, app, &app.chart.bpms, now, speed_scale);
        let head_windows = windows::head(note);
        // Diagnostic windows outlive a NoCore sprite through its late window.
        let late = if note.note_type == NoteType::Slide {
            (0..note.slide.len())
                .filter_map(|sub| app.judge_windows.slides.get(&(note.id, sub)))
                .map(|w| w.deadline)
                .fold(t.ns + head_windows[0].late, f32::max)
        } else if matches!(note.note_type, NoteType::Hold | NoteType::TouchHold) {
            let tail_late = if app
                .judge_windows
                .classic_holds
                .get(&note.id)
                .copied()
                .unwrap_or(false)
            {
                windows::classic_hold_tail().late
            } else {
                head_windows[0].late
            };
            hold_tail_time(note, &app.chart.bpms) + tail_late
        } else {
            t.ns + head_windows[0].late
        };
        let visible = solo.is_some() || t.dt_scaled <= t.lead_time && now <= late;
        if !visible {
            continue;
        }
        let touch = note_is_touch(note);
        let dir = direction(t.zone);
        let normal = vec2(-dir.y, dir.x);
        let radial = |seconds: f32, unclamped: bool| {
            let motion = if unclamped {
                note_radial_motion_continue(
                    seconds / speed_scale,
                    t.speed,
                    pad.outer_r,
                    p.tap_target_offset,
                )
            } else {
                note_radial_motion(
                    seconds / speed_scale,
                    t.speed,
                    pad.outer_r,
                    p.tap_target_offset,
                )
            };
            Some(
                center
                    + dir
                        * motion
                            .map(|m| m.radius)
                            .unwrap_or_else(|| note_lock_radius(pad.outer_r, p.tap_target_offset)),
            )
        };
        if solo.is_none()
            && (note.note_type != NoteType::Slide || !note.is_tapless)
            && t.dt >= -head_windows[0].late
            && t.dt <= t.lead_time * speed_scale
        {
            if touch {
                if let Some(point) = app
                    .pad_svg
                    .as_ref()
                    .and_then(|svg| svg.zone_screen_centroid(PadZone::from(t.zone), pad))
                {
                    halo(point, 30.0 * scale, t.ns, now, &head_windows, 0, scale);
                }
            } else {
                let offset = if note.note_type == NoteType::Hold {
                    p.judge_off_hold
                } else {
                    p.judge_off_tap
                };
                for (index, range) in head_windows.iter().enumerate() {
                    if let (Some(a), Some(b)) =
                        (radial(-range.early, true), radial(-range.late, true))
                    {
                        slide_render::draw_polyline_band(
                            &[a + dir * offset * scale, b + dir * offset * scale],
                            (14.0 - index as f32 * 3.0) * scale,
                            windows::color(index, 190),
                        );
                    }
                }
                if let Some(point) = radial(0.0, true) {
                    tick(point + dir * offset * scale, normal, 10.0 * scale, WHITE);
                }
            }
        }
        if solo.is_none() && matches!(note.note_type, NoteType::Hold | NoteType::TouchHold) {
            let tail = hold_tail_time(note, &app.chart.bpms);
            if app
                .judge_windows
                .classic_holds
                .get(&note.id)
                .copied()
                .unwrap_or(false)
            {
                if let Some(point) = radial(tail - now, false) {
                    halo(
                        point + dir * p.judge_off_hold_end * scale,
                        22.0 * scale,
                        tail,
                        now,
                        &[windows::classic_hold_tail()],
                        2,
                        scale,
                    );
                }
            } else if let Some(body) = windows::hold_requirement(t.ns, tail - t.ns, touch) {
                if touch {
                    if let Some(point) = app
                        .pad_svg
                        .as_ref()
                        .and_then(|svg| svg.zone_screen_centroid(PadZone::from(t.zone), pad))
                    {
                        let interval = windows::Interval {
                            early: body.body_start - t.ns,
                            late: body.perfect_release - t.ns,
                        };
                        // The duration arc is outside the hit-timing arc.
                        let radius = 41.0 * scale;
                        let length = (tail - t.ns).max(0.001);
                        let angle = |seconds: f32| {
                            -std::f32::consts::FRAC_PI_2 + seconds / length * std::f32::consts::TAU
                        };
                        let arc: Vec<_> = (0..=48)
                            .map(|i| {
                                let a = angle(
                                    interval.early
                                        + (interval.late - interval.early) * i as f32 / 48.0,
                                );
                                point + vec2(a.cos(), a.sin()) * radius
                            })
                            .collect();
                        slide_render::draw_polyline_band(&arc, 3.0 * scale, hold_color);
                        if let Some(end) = arc.last() {
                            tick(
                                *end,
                                (*end - point).normalize_or_zero(),
                                5.0 * scale,
                                hold_color,
                            );
                        }
                    }
                } else if let (Some(a), Some(b)) = (
                    radial(body.body_start - now, false),
                    radial(body.perfect_release - now, false),
                ) {
                    let shift = normal * (p.hold_width * 0.6 + 4.0) * scale
                        + dir * p.judge_off_hold * scale;
                    slide_render::draw_polyline_band(
                        &[a + shift, b + shift],
                        3.0 * scale,
                        hold_color,
                    );
                    tick(b + shift, normal, 6.0 * scale, hold_color);
                }
            }
        }
        if note.note_type == NoteType::Slide {
            for (sub, sl) in slide::preview_subslides(note, solo, false) {
                let Some(window) = app.judge_windows.slides.get(&(note.id, sub)) else {
                    continue;
                };
                if solo.is_none() && now > window.deadline {
                    continue;
                }
                if solo.is_none()
                    && app.use_core()
                    && app
                        .slide_progress
                        .get(&(note.id, sub))
                        .is_some_and(|progress| {
                            !progress.seg_frac.is_empty()
                                && progress.seg_frac.iter().all(|f| *f >= 1.0)
                        })
                {
                    continue;
                }
                let Some(svg) = &app.pad_svg else {
                    continue;
                };
                let rendered = slide_render::render_slide_legs(sl);
                let sl = rendered.as_ref();
                let duration = mdur_to_secs(sl.slide_duration, note.time, &app.chart.bpms)
                    .max(SLIDE_MIN_DURATION_S);
                let delay = mdur_to_secs(sl.slide_start_delay, note.time, &app.chart.bpms)
                    .max(0.0)
                    .min(duration - 0.001)
                    .max(0.001);
                let start = t.ns + delay;
                let travel = (duration - delay).max(SLIDE_MIN_DURATION_S);
                let paths = if sl
                    .segments
                    .iter()
                    .any(|s| s.shape == crate::app::types::SlideShape::Wifi)
                {
                    let origin =
                        center + direction(note.lane) * (pad.outer_r + p.tap_target_offset);
                    [3, 4, 5]
                        .iter()
                        .map(|delta| {
                            let zone = ((note.lane as i32 + delta - 1).rem_euclid(8) + 1) as u8;
                            vec![
                                origin,
                                center + direction(zone) * (pad.outer_r + p.tap_target_offset),
                            ]
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![slide_render::build_slide_path(
                        note,
                        sl,
                        pad,
                        svg,
                        scale,
                        center,
                        pad.outer_r,
                    )]
                };
                draw_slide_windows(&paths, window, start, travel, now, scale);
            }
        }
    }
}

fn draw_slide_windows(
    paths: &[Vec<Vec2>],
    window: &windows::SlideWindow,
    start: f32,
    travel: f32,
    now: f32,
    scale: f32,
) {
    for path in paths {
        for (index, range) in window.windows.iter().enumerate() {
            let portion = windows::path_interval(
                path,
                (window.judge + range.early - start) / travel,
                (window.judge + range.late - start) / travel,
            );
            slide_render::draw_polyline_band(
                &portion,
                (11.0 - index as f32 * 3.0) * scale,
                windows::color(index + 1, 210),
            );
        }
        if let Some(end) = path.last() {
            halo(
                *end,
                18.0 * scale,
                window.judge,
                now,
                &window.windows,
                1,
                scale,
            );
        }
    }
}
