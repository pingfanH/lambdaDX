use macroquad::prelude::*;
use macroquad::texture::DrawTextureParams;

use crate::app::pad_svg;
use crate::app::slide::path::*;
use crate::app::state::AppState;
use crate::app::template;
use crate::app::types::zone::PadZone;
use crate::app::types::{
    FIXED_SLIDE_FADE_IN, GRID_DIVISION, HIT_WINDOW, HOLD_DISAPPEAR_FRAC, HOLD_FLY_TIME,
    HOLD_LENGTH_FRAC, HOLD_SPAWN_FRAC, HOLD_TAIL_FLY_TIME, HOLD_TRAVEL_TIME,
    HOLD_WIDTH, LANE_COUNT, LANE_LABELS, Layout, Mode, NoteType, PAD_C_ZONE, PAD_ROTATION_RAD,
    PREVIEW_LEAD_TIME, PadGeom, RectF, SCROLL_SPEED, SLIDE_TILE_SCALE, SLIDE_TILE_SIZE,
    SLIDE_TILE_SPACING, SLIDE_TRAVEL_TIME, STAR_SIZE, SlideShape, TAP_DISAPPEAR_FRAC,
    TAP_GROW_FRAC, TAP_RING_OFFSET, TAP_SIZE, TAP_SPAWN_FRAC, TAP_TARGET_OFFSET, TAP_TRAVEL_TIME,
    TOUCH_CROSS_SIZE, TOUCH_DISAPPEAR_TIME, TOUCH_END_DIST, TOUCH_GROW_FRAC, TOUCH_SCALE,
    TOUCH_SIZE, TOUCH_START_DIST, TOUCH_TRAVEL_TIME, TOUCHHOLD_BORDER_BASE, TOUCHHOLD_CROSS_BASE,
    TOUCHHOLD_END_DIST, TOUCHHOLD_ROT_OFFSET, TOUCHHOLD_SCALE, TOUCHHOLD_START_DIST, bpm_at,
    hold_tail_time, is_touch_zone, mdur_to_secs, measure_to_secs, note_secs, sanitize_note_zone,
    secs_to_measure, slide_end_time, snap_measure,
};

fn ui_scale(app: &AppState) -> f32 {
    if let Some(v) = app.ui_scale_override {
        return v;
    }
    if app.mobile_ui {
        let base = screen_width().min(screen_height()) / 760.0;
        base.max(1.35)
    } else {
        1.0
    }
}

pub fn draw_timeline_panel(app: &AppState, rect: RectF) {
    let scale = ui_scale(app);
    // draw_rectangle(rect.x, rect.y, rect.w, rect.h, Color::from_rgba(17, 24, 39, 255));
    let title = if template::is_in_isolation(app) {
        if let Some(name) = template::current_template_name(app) {
            format!("Template: {} (editing)", name)
        } else {
            "Template (editing)".to_string()
        }
    } else {
        "Timeline (Vertical) : 1~8 Tap/Hold + T Touch".to_string()
    };
    let title_color = if template::is_in_isolation(app) {
        Color::from_rgba(230, 149, 48, 255) // orange for isolation
    } else {
        Color::from_rgba(180, 180, 180, 255)
    };
    // draw_text(
    //     &title,
    //     rect.x + 12.0 * scale,
    //     rect.y + 24.0 * scale,
    //     24.0 * scale,
    //     title_color,
    // );

    // Tool selection moved to egui sidebar; timeline fills entire area
    let sidebar_w = 0.0;
    let track_x = rect.x + 4.0 * scale;
    let track_y = rect.y + 10.0 * scale;
    let track_w = rect.w - 8.0 * scale;
    let progress_bar_h = 20.0 * scale;
    let track_h = rect.h - 20.0 * scale - progress_bar_h;

    // ── Tool sidebar (Tap / Hold / Star) — moved to egui sidebar ──
    // ...
    let ruler_w = 0.0;
    let lanes_w = track_w;
    let lane_w = lanes_w / LANE_COUNT as f32;

    for (i, label) in LANE_LABELS.iter().enumerate() {
        let lx = track_x + lane_w * i as f32 + lane_w * 0.45;
        let color = if *label == "T" {
            Color::from_rgba(230, 149, 48, 255)
        } else {
            Color::from_rgba(180, 180, 180, 255)
        };
        draw_text(label, lx, track_y + 18.0 * scale, 20.0 * scale, color);
    }

    for i in 0..=LANE_COUNT {
        let lx = track_x + lane_w * i as f32;
        let c = if i == 4 {
            Color::from_rgba(90, 90, 90, 255)
        } else {
            Color::from_rgba(55, 55, 55, 255)
        };
        draw_line(
            lx,
            track_y,
            lx,
            track_y + track_h,
            if i == 4 { 2.0 * scale } else { 1.0 * scale },
            c,
        );
    }

    let judge_y = track_y + track_h - 38.0 * scale;
    let now = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    let bpms = &app.chart.bpms;
    let scroll_speed = SCROLL_SPEED * app.timeline_zoom / app.play_speed.max(0.1);

    // BPM-based grid lines (cover full track height)
    let beat_s = 60.0 / app.chart.bpm;
    let grid_s = beat_s / (GRID_DIVISION as f32 / 4.0);
    let margin_s = track_h / scroll_speed;
    let view_start = now - margin_s;
    let view_end = now + margin_s;

    let mut t = (view_start / grid_s).floor() * grid_s;
    while t <= view_end {
        let yy = judge_y - (t - now) * scroll_speed;
        let bar_s = beat_s * 4.0;
        let dist_to_bar = ((t % bar_s) + bar_s) % bar_s;
        let is_bar = dist_to_bar < grid_s * 0.5 || (bar_s - dist_to_bar) < grid_s * 0.5;
        let dist_to_beat = ((t % beat_s) + beat_s) % beat_s;
        let is_beat = dist_to_beat < grid_s * 0.5 || (beat_s - dist_to_beat) < grid_s * 0.5;
        let color = if is_bar {
            Color::from_rgba(140, 50, 50, 255)
        } else if is_beat {
            Color::from_rgba(80, 80, 80, 255)
        } else {
            Color::from_rgba(40, 40, 40, 255)
        };
        let thickness = if is_bar {
            2.0
        } else if is_beat {
            1.5
        } else {
            0.5
        } * scale;
        draw_line(track_x, yy, track_x + track_w, yy, thickness, color);
        if is_bar {
            let bar_num = (t / (beat_s * 4.0)) as i32;
            draw_text(
                &format!("{bar_num}"),
                track_x + 10.0 * scale,
                yy + 4.0 * scale,
                16.0 * scale,
                Color::from_rgba(140, 140, 140, 255),
            );
        }
        t += grid_s;
    }

    // Beat-focused vertical spectrum (low freqs = wider, brighter)
    if app.waveform_freq_bins > 0 && !app.waveform_data.is_empty() {
        let fb = app.waveform_freq_bins as usize;
        let num_tb = app.waveform_data.len() / fb;
        let dt = app.waveform_time_res;
        let max_val = app.waveform_max_val;
        // Only use low frequencies (0-500Hz for kick/snare/beat detection)
        let sr = app
            .audio_wav_pcm
            .as_ref()
            .map(|p| p.sample_rate as f32)
            .unwrap_or(44100.0);
        let max_hz = 600.0;
        let beat_bins = ((fb as f32 * max_hz / (sr * 0.5)) as usize).min(fb).max(8);
        let bin_step = (beat_bins as f32 / 40.0).max(1.0) as usize;
        let disp_bins = beat_bins / bin_step;
        let half_w = lanes_w * 0.45;
        // Only iterate visible time range
        let t_top = now + (judge_y - track_y) / scroll_speed;
        let t_bot = now + (judge_y - (track_y + track_h)) / scroll_speed;
        let t_min = t_bot.min(t_top);
        let t_max = t_bot.max(t_top);
        let ti_start = ((t_min / dt) as usize).saturating_sub(1).min(num_tb);
        let ti_end = ((t_max / dt) as usize + 2).min(num_tb);
        for ti in ti_start..ti_end {
            let t = ti as f32 * dt;
            let cy = judge_y - (t - now) * scroll_speed;
            // Compute total low-freq energy for this time bin
            let mut total = 0.0;
            let mut peak = 0.0;
            for fi in 0..beat_bins {
                let v = app.waveform_data[ti * fb + fi];
                total += v;
                if v > peak {
                    peak = v;
                }
            }
            let avg_norm = (total / beat_bins as f32 / max_val).min(1.0);
            let peak_norm = (peak / max_val).min(1.0);
            if avg_norm < 0.005 {
                continue;
            }
            // Draw a prominent beat bar
            let bar_w = half_w * peak_norm * 1.5;
            let color = if peak_norm > app.waveform_threshold {
                Color::from_rgba(255, 180, 50, (peak_norm * 255.0) as u8)
            } else {
                Color::from_rgba(60, 120, (avg_norm * 200.0) as u8, (avg_norm * 180.0) as u8)
            };
            draw_rectangle(track_x + half_w - bar_w, cy - 2.0, bar_w * 2.0, 4.0, color);
            // Per-frequency thin bars overlaid
            for di in 0..disp_bins {
                let fi = di * bin_step;
                let mag = app.waveform_data[ti * fb + fi];
                let norm = (mag / max_val).min(1.0);
                if norm < 0.03 {
                    continue;
                }
                let fw = half_w * norm * 0.6 / disp_bins as f32;
                let fcolor = if norm > app.waveform_threshold {
                    Color::from_rgba(255, 200, 80, (norm * 200.0) as u8)
                } else {
                    Color::from_rgba(60, 140, 255, (norm * 150.0) as u8)
                };
                let lx = track_x + half_w - fw * di as f32 * 0.3;
                let rx = track_x + half_w + fw * di as f32 * 0.3 - fw;
                draw_rectangle(lx, cy - 1.0, fw, 2.0, fcolor);
                draw_rectangle(rx, cy - 1.0, fw, 2.0, fcolor);
            }
        }
    }

    // Scrubber triangle on ruler at now position
    let scrub_y = judge_y;
    if scrub_y >= track_y && scrub_y <= track_y + track_h {
        draw_triangle(
            vec2(track_x - 4.0 * scale, scrub_y),
            vec2(track_x - 14.0 * scale, scrub_y - 6.0 * scale),
            vec2(track_x - 14.0 * scale, scrub_y + 6.0 * scale),
            Color::from_rgba(200, 70, 70, 255),
        );
    }

    // ── Template instance blocks (rendered behind notes) ──
    if !template::is_in_isolation(app) {
        let lane_w = lanes_w / LANE_COUNT as f32;
        for (inst_idx, inst) in app.chart.template_instances.iter().enumerate() {
            let (i_start, i_end) = template::instance_time_range(app, inst);
            let start_secs = crate::app::types::measure_to_secs(i_start, bpms);
            let end_secs = crate::app::types::measure_to_secs(i_end, bpms);
            let dt_start = start_secs - now;
            let dt_end = end_secs - now;

            // Skip if entirely off-screen.
            if dt_end < -margin_s || dt_start > margin_s {
                continue;
            }

            let y_top = judge_y - dt_start * scroll_speed;
            let y_bot = judge_y - dt_end * scroll_speed;
            let block_x = track_x;
            let block_w = lanes_w;
            let block_y = y_top.min(y_bot);
            let block_h = (y_top - y_bot).abs();

            // Dark translucent fill.
            draw_rectangle(
                block_x,
                block_y,
                block_w,
                block_h,
                Color::from_rgba(20, 20, 40, 120),
            );
            // Top edge line.
            draw_rectangle(
                block_x,
                y_top,
                block_w,
                2.0 * scale,
                Color::from_rgba(100, 140, 200, 200),
            );
            // Bottom edge line.
            draw_rectangle(
                block_x,
                y_bot,
                block_w,
                2.0 * scale,
                Color::from_rgba(100, 140, 200, 200),
            );
            // Left edge line.
            draw_rectangle(
                block_x,
                block_y,
                2.0 * scale,
                block_h,
                Color::from_rgba(100, 140, 200, 140),
            );
            // Right edge line.
            draw_rectangle(
                block_x + block_w - 2.0 * scale,
                block_y,
                2.0 * scale,
                block_h,
                Color::from_rgba(100, 140, 200, 140),
            );

            // Resize handles: small squares at corners.
            let handle_size = 6.0 * scale;
            let handle_color = Color::from_rgba(140, 180, 240, 220);
            draw_rectangle(
                block_x - handle_size * 0.5,
                y_top - handle_size * 0.5,
                handle_size,
                handle_size,
                handle_color,
            );
            draw_rectangle(
                block_x + block_w - handle_size * 0.5,
                y_top - handle_size * 0.5,
                handle_size,
                handle_size,
                handle_color,
            );
            draw_rectangle(
                block_x - handle_size * 0.5,
                y_bot - handle_size * 0.5,
                handle_size,
                handle_size,
                handle_color,
            );
            draw_rectangle(
                block_x + block_w - handle_size * 0.5,
                y_bot - handle_size * 0.5,
                handle_size,
                handle_size,
                handle_color,
            );

            // Label: template name above the block.
            let tpl = app
                .chart
                .templates
                .iter()
                .find(|t| t.id == inst.template_id);
            if let Some(tpl) = tpl {
                let label = format!("[{}]", tpl.name);
                let label_y = y_top - 6.0 * scale;
                draw_text(
                    &label,
                    block_x + 4.0 * scale,
                    label_y,
                    14.0 * scale,
                    Color::from_rgba(140, 180, 240, 200),
                );

                // Render expanded template notes inside the block using proper textures.
                let expanded = template::expand_instance(inst, tpl);
                for enote in &expanded {
                    let en_zone = sanitize_note_zone(enote.note_type, enote.lane);
                    let en_secs = note_secs(enote, bpms);
                    let en_dt = en_secs - now;
                    if en_dt < -margin_s || en_dt > margin_s {
                        continue;
                    }
                    let en_lane_idx = if is_touch_zone(en_zone) {
                        LANE_COUNT - 1
                    } else {
                        (en_zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
                    };
                    let en_cx = track_x + lane_w * en_lane_idx as f32 + lane_w * 0.5;
                    let en_ny = judge_y - en_dt * scroll_speed;

                    // Render with proper textures, using a slight blue tint to indicate template.
                    let tpl_color = Color::from_rgba(180, 200, 255, 220);
                    match enote.note_type {
                        NoteType::Tap => {
                            let tap_tex = if enote.is_break {
                                app.tap_break_tex.as_ref()
                            } else if enote.is_each {
                                app.tap_each_tex.as_ref()
                            } else {
                                app.tap_texture.as_ref()
                            }
                            .or(app.tap_texture.as_ref());
                            let ts = TAP_SIZE * scale;
                            if let Some(tex) = tap_tex {
                                draw_tap_sprite_c(tex, en_cx, en_ny, ts, tpl_color);
                            } else {
                                let tr = TAP_SIZE * 0.3125 * scale;
                                draw_circle(en_cx, en_ny, tr, Color::from_rgba(17, 24, 39, 255));
                                draw_circle_lines(
                                    en_cx,
                                    en_ny,
                                    tr,
                                    tr * 0.3,
                                    Color::from_rgba(100, 140, 200, 255),
                                );
                                draw_circle(
                                    en_cx,
                                    en_ny,
                                    tr * 0.3,
                                    Color::from_rgba(140, 180, 240, 255),
                                );
                            }
                        }
                        NoteType::Touch => {
                            let tri_tex = if enote.is_each {
                                app.touch_tri_each_tex.as_ref()
                            } else {
                                app.touch_tri_tex.as_ref()
                            }
                            .or(app.touch_tri_tex.as_ref());
                            let pt_tex = if enote.is_each {
                                app.touch_point_each_tex.as_ref()
                            } else {
                                app.touch_point_tex.as_ref()
                            }
                            .or(app.touch_point_tex.as_ref());
                            if let Some(tex) = tri_tex {
                                let ratio = tex.width() / tex.height();
                                let ts = 30.0 * scale;
                                let tw = ts;
                                let th = ts / ratio;
                                draw_texture_ex(
                                    tex,
                                    en_cx - tw * 0.5,
                                    en_ny + ts * 0.3 - th * 0.5,
                                    tpl_color,
                                    DrawTextureParams {
                                        dest_size: Some(vec2(tw, th)),
                                        ..Default::default()
                                    },
                                );
                                draw_texture_ex(
                                    tex,
                                    en_cx - tw * 0.5,
                                    en_ny - ts * 0.3 - th * 0.5,
                                    tpl_color,
                                    DrawTextureParams {
                                        dest_size: Some(vec2(tw, th)),
                                        rotation: std::f32::consts::PI,
                                        ..Default::default()
                                    },
                                );
                                draw_texture_ex(
                                    tex,
                                    en_cx - ts * 0.3 - tw * 0.5,
                                    en_ny - th * 0.5,
                                    tpl_color,
                                    DrawTextureParams {
                                        dest_size: Some(vec2(tw, th)),
                                        rotation: std::f32::consts::FRAC_PI_2,
                                        ..Default::default()
                                    },
                                );
                                draw_texture_ex(
                                    tex,
                                    en_cx + ts * 0.3 - tw * 0.5,
                                    en_ny - th * 0.5,
                                    tpl_color,
                                    DrawTextureParams {
                                        dest_size: Some(vec2(tw, th)),
                                        rotation: -std::f32::consts::FRAC_PI_2,
                                        ..Default::default()
                                    },
                                );
                            } else {
                                draw_circle(
                                    en_cx,
                                    en_ny,
                                    3.0 * scale,
                                    Color::from_rgba(100, 140, 200, 200),
                                );
                            }
                        }
                        NoteType::Slide => {
                            if !enote.is_tapless {
                                let is_double = enote.is_star;
                                let star_tex = if enote.is_break {
                                    if is_double {
                                        app.star_double_break_tex.as_ref()
                                    } else {
                                        app.star_break_tex.as_ref()
                                    }
                                } else if enote.is_each {
                                    if is_double {
                                        app.star_double_each_tex.as_ref()
                                    } else {
                                        app.star_each_tex.as_ref()
                                    }
                                } else {
                                    if is_double {
                                        app.star_double_tex.as_ref()
                                    } else {
                                        app.star_tex.as_ref()
                                    }
                                };
                                let fallback = if is_double {
                                    app.star_double_tex.as_ref()
                                } else {
                                    app.star_tex.as_ref()
                                };
                                let ss = TAP_SIZE * scale;
                                if let Some(tex) = star_tex.or(fallback).or(app.star_tex.as_ref()) {
                                    draw_texture_ex(
                                        tex,
                                        en_cx - ss * 0.5,
                                        en_ny - ss * 0.5,
                                        tpl_color,
                                        DrawTextureParams {
                                            dest_size: Some(vec2(ss, ss)),
                                            ..Default::default()
                                        },
                                    );
                                } else {
                                    draw_poly(
                                        en_cx,
                                        en_ny,
                                        5,
                                        ss * 0.4,
                                        0.0,
                                        Color::from_rgba(100, 140, 200, 230),
                                    );
                                }
                            }
                            // Slide trail rendering
                            for sl in &enote.slide {
                                let dur_s =
                                    mdur_to_secs(sl.slide_duration, enote.time, bpms).max(0.0);
                                let delay_s = mdur_to_secs(sl.slide_start_delay, enote.time, bpms)
                                    .max(0.0)
                                    .min(dur_s);
                                let delay_y = judge_y - (en_dt + delay_s) * scroll_speed;
                                let tail_y = judge_y - (en_dt + dur_s) * scroll_speed;
                                // Dashed delay line
                                let delay_h = (en_ny - delay_y).abs();
                                if delay_s > 0.0 && delay_h > 0.5 {
                                    let dash_len = 6.0 * scale;
                                    let gap = 4.0 * scale;
                                    let period = dash_len + gap;
                                    let top = en_ny.min(delay_y);
                                    let n_dashes = (delay_h / period).ceil() as i32;
                                    for k in 0..n_dashes {
                                        let y0 = top + (k as f32) * period;
                                        let y1 = (y0 + dash_len).min(top + delay_h);
                                        draw_line(
                                            en_cx,
                                            y0,
                                            en_cx,
                                            y1,
                                            2.0 * scale,
                                            Color::from_rgba(180, 200, 255, 200),
                                        );
                                    }
                                }
                                // Slide trail waypoints
                                let travel_h = (delay_y - tail_y).abs();
                                if travel_h > 0.5 {
                                    let zone_to_cx_a = |z: PadZone| -> f32 {
                                        let li = (z.to_id().saturating_sub(1) as usize)
                                            .min(LANE_COUNT - 2);
                                        track_x + lane_w * li as f32 + lane_w * 0.5
                                    };
                                    let mut a_points: Vec<&crate::app::types::SlidePoint> =
                                        Vec::new();
                                    for seg in &sl.segments {
                                        for sp in &seg.points {
                                            if sp.zone >= 1 && sp.zone <= 8 {
                                                a_points.push(sp);
                                            }
                                        }
                                    }
                                    let mut waypoints: Vec<(f32, f32)> = Vec::new();
                                    waypoints.push((en_cx, delay_y));
                                    let n_pts = a_points.len();
                                    if n_pts > 0 {
                                        for (pi, sp) in a_points.iter().enumerate() {
                                            let frac = (pi + 1) as f32 / (n_pts) as f32;
                                            let wy = delay_y + (tail_y - delay_y) * frac;
                                            let wx = zone_to_cx_a(sp.zone);
                                            waypoints.push((wx, wy));
                                        }
                                    } else {
                                        waypoints.push((en_cx, tail_y));
                                    }
                                    let line_w = 3.0 * scale;
                                    for seg_i in 0..waypoints.len() - 1 {
                                        let (x0, y0) = waypoints[seg_i];
                                        let (x1, y1) = waypoints[seg_i + 1];
                                        if let Some(tex) = app.slide_tex.as_ref() {
                                            let dx = x1 - x0;
                                            let dy = y1 - y0;
                                            let seg_len = (dx * dx + dy * dy).sqrt();
                                            let bar_w = 14.0 * scale;
                                            let tw_nat = tex.width() * scale * SLIDE_TILE_SCALE;
                                            let th_nat = tex.height() * scale * SLIDE_TILE_SCALE;
                                            let tile_h = th_nat * (bar_w / tw_nat).max(0.001);
                                            let spacing = SLIDE_TILE_SPACING * scale;
                                            let angle = dy.atan2(dx) + std::f32::consts::PI;
                                            let steps = (seg_len / spacing).ceil().max(1.0) as i32;
                                            for k in 0..=steps {
                                                let t = k as f32 / steps as f32;
                                                let px = x0 + dx * t;
                                                let py = y0 + dy * t;
                                                draw_texture_ex(
                                                    tex,
                                                    px - bar_w * 0.5,
                                                    py - tile_h * 0.5,
                                                    Color::from_rgba(200, 210, 255, 230),
                                                    DrawTextureParams {
                                                        dest_size: Some(vec2(bar_w, tile_h)),
                                                        rotation: angle,
                                                        ..Default::default()
                                                    },
                                                );
                                            }
                                        } else {
                                            draw_line(
                                                x0,
                                                y0,
                                                x1,
                                                y1,
                                                line_w,
                                                Color::from_rgba(180, 200, 255, 200),
                                            );
                                        }
                                    }
                                    for &(wx, wy) in &waypoints[1..] {
                                        draw_circle(
                                            wx,
                                            wy,
                                            3.0 * scale,
                                            Color::from_rgba(180, 200, 255, 200),
                                        );
                                    }
                                }
                                // Delay handle and tail circles
                                if delay_s > 0.0 || dur_s > 0.0 {
                                    draw_circle(
                                        en_cx,
                                        delay_y,
                                        4.5 * scale,
                                        Color::from_rgba(140, 180, 240, 230),
                                    );
                                    draw_circle_lines(
                                        en_cx,
                                        delay_y,
                                        4.5 * scale,
                                        1.5 * scale,
                                        Color::from_rgba(200, 210, 255, 220),
                                    );
                                }
                                if dur_s > 0.0 {
                                    let tail_cx = crate::app::input::slide_tail_cx_for(
                                        enote, 0, track_x, ruler_w, lane_w,
                                    );
                                    draw_circle(
                                        tail_cx,
                                        tail_y,
                                        5.5 * scale,
                                        Color::from_rgba(180, 200, 255, 230),
                                    );
                                    draw_circle_lines(
                                        tail_cx,
                                        tail_y,
                                        5.5 * scale,
                                        1.5 * scale,
                                        Color::from_rgba(200, 210, 255, 220),
                                    );
                                }
                            }
                        }
                        NoteType::Hold => {
                            let tail_time = hold_tail_time(enote, bpms);
                            let tail_dt_s = tail_time - now;
                            let tail_y = judge_y - tail_dt_s * scroll_speed;
                            let hold_tex = if enote.is_break {
                                app.hold_break_tex.as_ref()
                            } else if enote.is_each {
                                app.hold_each_tex.as_ref()
                            } else {
                                app.hold_texture.as_ref()
                            }
                            .or(app.hold_texture.as_ref());
                            let hw = HOLD_WIDTH * scale;
                            if let Some(tex) = hold_tex {
                                draw_hold_9slice_vertical(tex, en_cx, en_ny, tail_y, hw);
                            } else {
                                let top = en_ny.min(tail_y);
                                let h = (en_ny - tail_y).abs().max(hw * 0.133);
                                draw_rectangle(
                                    en_cx - hw * 0.2,
                                    top,
                                    hw * 0.4,
                                    h,
                                    Color::from_rgba(100, 140, 200, 130),
                                );
                                draw_rectangle_lines(
                                    en_cx - hw * 0.2,
                                    top,
                                    hw * 0.4,
                                    h,
                                    1.0 * scale,
                                    Color::from_rgba(140, 180, 240, 200),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Isolation mode: dim areas outside template range ──
    if template::is_in_isolation(app) {
        let block_x = track_x;
        let block_w = lanes_w;
        // Determine the template's time range.
        // In isolation with instance_anchor, notes are already offset.
        // Find min/max time of the current notes (which are the template's notes).
        if let (Some(min_t), Some(max_t)) = (
            app.chart.notes.iter().map(|n| n.time).reduce(f32::min),
            app.chart
                .notes
                .iter()
                .map(|n| {
                    let dur = n.hold_duration.max(
                        n.slide
                            .iter()
                            .map(|s| s.slide_duration)
                            .fold(0.0_f32, f32::max),
                    );
                    n.time + dur
                })
                .reduce(f32::max),
        ) {
            let start_secs = crate::app::types::measure_to_secs(min_t, bpms);
            let end_secs = crate::app::types::measure_to_secs(max_t, bpms);
            let y_top = judge_y - (start_secs - now) * scroll_speed;
            let y_bot = judge_y - (end_secs - now) * scroll_speed;
            let region_top = y_top.min(y_bot);
            let region_bot = y_top.max(y_bot);

            // Dim above the template region.
            if region_top > track_y {
                draw_rectangle(
                    block_x,
                    track_y,
                    block_w,
                    region_top - track_y,
                    Color::from_rgba(10, 10, 20, 160),
                );
            }
            // Dim below the template region.
            if region_bot < track_y + track_h {
                draw_rectangle(
                    block_x,
                    region_bot,
                    block_w,
                    track_y + track_h - region_bot,
                    Color::from_rgba(10, 10, 20, 160),
                );
            }
            // Highlight border lines at template edges.
            draw_rectangle(
                block_x,
                region_top,
                block_w,
                2.0 * scale,
                Color::from_rgba(230, 149, 48, 200),
            );
            draw_rectangle(
                block_x,
                region_bot,
                block_w,
                2.0 * scale,
                Color::from_rgba(230, 149, 48, 200),
            );
        }
    }

    for (idx, note) in app.chart.notes.iter().enumerate() {
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let ns = note_secs(note, bpms);
        let dt = ns - now;
        let tail_dt = match note.note_type {
            NoteType::Hold => hold_tail_time(note, bpms) - now,
            NoteType::Slide => {
                let max_d = note
                    .slide
                    .iter()
                    .map(|s| s.slide_duration)
                    .fold(0.0_f32, f32::max);
                ns + mdur_to_secs(max_d, note.time, bpms) - now
            }
            _ => dt,
        };
        // Keep the note visible while either its head OR its tail is on-screen.
        // (For Tap/Touch tail_dt == dt, so the check collapses to the original.)
        if tail_dt < -margin_s || dt.min(tail_dt) > margin_s {
            continue;
        }
        let hidden = app.hidden_notes.contains(&note.id);
        let ca = |r: u8, g: u8, b: u8, a: u8| -> Color {
            if hidden {
                Color::from_rgba(r, g, b, (a as f32 * 0.3) as u8)
            } else {
                Color::from_rgba(r, g, b, a)
            }
        };
        let lane_index = if is_touch_zone(zone) {
            LANE_COUNT - 1
        } else {
            (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
        };
        let cx = track_x + lane_w * lane_index as f32 + lane_w * 0.5;
        let scroll = scroll_speed;
        let ny = judge_y - dt * scroll;

        // Selection highlight（隐藏 note 不选中）
        if !hidden && app.selected_note == Some(note.id) {
            draw_circle(cx, ny, 16.0, ca(56, 189, 248, 100));
        }

        match note.note_type {
            NoteType::Tap => {
                let tap_tex = if note.is_break {
                    app.tap_break_tex.as_ref()
                } else if note.is_each {
                    app.tap_each_tex.as_ref()
                } else {
                    app.tap_texture.as_ref()
                }
                .or(app.tap_texture.as_ref());
                let ts = TAP_SIZE * scale;
                if let Some(tex) = tap_tex {
                    draw_tap_sprite_c(tex, cx, ny, ts, ca(255, 255, 255, 255));
                    if note.is_ex {
                        if let Some(ex) = app.tap_ex_tex.as_ref() {
                            draw_tap_sprite_c(ex, cx, ny, ts, ca(255, 255, 255, 255));
                        }
                    }
                } else {
                    let tr = TAP_SIZE * 0.3125 * scale;
                    draw_circle(cx, ny, tr, ca(17, 24, 39, 255));
                    draw_circle_lines(cx, ny, tr, tr * 0.3, ca(244, 114, 182, 255));
                    draw_circle(cx, ny, tr * 0.3, ca(249, 168, 212, 255));
                }
                // Judgment center dot
                draw_circle(cx, ny, 2.5 * scale, ca(255, 255, 255, 200));
            }
            NoteType::Touch => {
                let tri_tex = if note.is_each {
                    app.touch_tri_each_tex.as_ref()
                } else {
                    app.touch_tri_tex.as_ref()
                }
                .or(app.touch_tri_tex.as_ref());
                let pt_tex = if note.is_each {
                    app.touch_point_each_tex.as_ref()
                } else {
                    app.touch_point_tex.as_ref()
                }
                .or(app.touch_point_tex.as_ref());
                if let Some(tex) = tri_tex {
                    let ratio = tex.width() / tex.height();
                    let ts = 30.0 * scale;
                    let tw = ts;
                    let th = ts / ratio;
                    let color = ca(255, 255, 255, 200);
                    draw_texture_ex(
                        tex,
                        cx - tw * 0.5,
                        ny + ts * 0.3 - th * 0.5,
                        color,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        cx - tw * 0.5,
                        ny - ts * 0.3 - th * 0.5,
                        color,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: std::f32::consts::PI,
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        cx - ts * 0.3 - tw * 0.5,
                        ny - th * 0.5,
                        color,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: std::f32::consts::FRAC_PI_2,
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        cx + ts * 0.3 - tw * 0.5,
                        ny - th * 0.5,
                        color,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: -std::f32::consts::FRAC_PI_2,
                            ..Default::default()
                        },
                    );
                }
                if let Some(pt) = pt_tex {
                    draw_texture_ex(
                        pt,
                        cx - 6.0 * scale,
                        ny - 6.0 * scale,
                        ca(255, 255, 255, 200),
                        DrawTextureParams {
                            dest_size: Some(vec2(12.0 * scale, 12.0 * scale)),
                            ..Default::default()
                        },
                    );
                } else {
                    draw_circle(cx, ny, 3.0 * scale, ca(103, 232, 249, 200));
                }
            }
            NoteType::Slide => {
                for (si, sl) in note.slide.iter().enumerate() {
                    let dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.0);
                    let delay_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
                        .max(0.0)
                        .min(dur_s);
                    let delay_y = judge_y - (dt + delay_s) * scroll;
                    let tail_y = judge_y - (dt + dur_s) * scroll;

                    let delay_h = (ny - delay_y).abs();
                    if delay_s > 0.0 && delay_h > 0.5 {
                        let dash_len = 6.0 * scale;
                        let gap = 4.0 * scale;
                        let period = dash_len + gap;
                        let top = ny.min(delay_y);
                        let n_dashes = (delay_h / period).ceil() as i32;
                        let col = ca(253, 224, 71, 220);
                        for k in 0..n_dashes {
                            let y0 = top + (k as f32) * period;
                            let y1 = (y0 + dash_len).min(top + delay_h);
                            draw_line(cx, y0, cx, y1, 2.0 * scale, col);
                        }
                    }

                    let slide_tex = if sl.slide_is_break {
                        app.slide_break_tex.as_ref()
                    } else if note.is_each {
                        app.slide_each_tex.as_ref()
                    } else {
                        app.slide_tex.as_ref()
                    };
                    let travel_h = (delay_y - tail_y).abs();
                    if travel_h > 0.5 {
                        let zone_to_cx_a = |z: PadZone| -> f32 {
                            let li = (z.to_id().saturating_sub(1) as usize).min(LANE_COUNT - 2);
                            track_x + lane_w * li as f32 + lane_w * 0.5
                        };
                        let mut a_points: Vec<&crate::app::types::SlidePoint> = Vec::new();
                        for seg in &sl.segments {
                            for sp in &seg.points {
                                if sp.zone >= 1 && sp.zone <= 8 {
                                    a_points.push(sp);
                                }
                            }
                        }
                        let mut waypoints: Vec<(f32, f32)> = Vec::new();
                        waypoints.push((cx, delay_y));
                        let n_pts = a_points.len();
                        if n_pts > 0 {
                            for (pi, sp) in a_points.iter().enumerate() {
                                let frac = (pi + 1) as f32 / (n_pts) as f32;
                                let wy = delay_y + (tail_y - delay_y) * frac;
                                let wx = zone_to_cx_a(sp.zone);
                                waypoints.push((wx, wy));
                            }
                        } else {
                            waypoints.push((cx, tail_y));
                        }

                        let line_w = 3.0 * scale;
                        let col = ca(250, 204, 21, 200);
                        let tile_col = ca(255, 255, 255, 230);
                        for seg in 0..waypoints.len() - 1 {
                            let (x0, y0) = waypoints[seg];
                            let (x1, y1) = waypoints[seg + 1];
                            if let Some(tex) = slide_tex {
                                let dx = x1 - x0;
                                let dy = y1 - y0;
                                let seg_len = (dx * dx + dy * dy).sqrt();
                                let bar_w = 14.0 * scale;
                                let tw_nat = tex.width() * scale * SLIDE_TILE_SCALE;
                                let th_nat = tex.height() * scale * SLIDE_TILE_SCALE;
                                let tile_h = th_nat * (bar_w / tw_nat).max(0.001);
                                let spacing = SLIDE_TILE_SPACING * scale;
                                let angle = dy.atan2(dx) + std::f32::consts::PI;
                                let steps = (seg_len / spacing).ceil().max(1.0) as i32;
                                for k in 0..=steps {
                                    let t = k as f32 / steps as f32;
                                    let px = x0 + dx * t;
                                    let py = y0 + dy * t;
                                    draw_texture_ex(
                                        tex,
                                        px - bar_w * 0.5,
                                        py - tile_h * 0.5,
                                        tile_col,
                                        DrawTextureParams {
                                            dest_size: Some(vec2(bar_w, tile_h)),
                                            rotation: angle,
                                            ..Default::default()
                                        },
                                    );
                                }
                            } else {
                                draw_line(x0, y0, x1, y1, line_w, col);
                            }
                        }
                        for &(wx, wy) in &waypoints[1..] {
                            draw_circle(wx, wy, 3.0 * scale, ca(253, 224, 71, 200));
                        }
                    }

                    if delay_s > 0.0 || dur_s > 0.0 {
                        draw_circle(cx, delay_y, 4.5 * scale, ca(56, 189, 248, 230));
                        draw_circle_lines(
                            cx,
                            delay_y,
                            4.5 * scale,
                            1.5 * scale,
                            ca(255, 255, 255, 220),
                        );
                    }
                    if dur_s > 0.0 {
                        let tail_cx = crate::app::input::slide_tail_cx_for(
                            note, si, track_x, ruler_w, lane_w,
                        );
                        draw_circle(tail_cx, tail_y, 5.5 * scale, ca(250, 204, 21, 230));
                        draw_circle_lines(
                            tail_cx,
                            tail_y,
                            5.5 * scale,
                            1.5 * scale,
                            ca(255, 255, 255, 220),
                        );
                    }
                }

                // Star head at note.time (same size as Tap)
                // Skip for tapless slides (they share the parent star's head)
                if !note.is_tapless {
                    // Use double-star textures when is_star is set (multiple slides)
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
                    } else {
                        if is_double {
                            app.star_double_tex.as_ref()
                        } else {
                            app.star_tex.as_ref()
                        }
                    };
                    let fallback = if is_double {
                        app.star_double_tex.as_ref()
                    } else {
                        app.star_tex.as_ref()
                    };
                    let ss = TAP_SIZE * scale;
                    if let Some(tex) = star_tex.or(fallback).or(app.star_tex.as_ref()) {
                        draw_texture_ex(
                            tex,
                            cx - ss * 0.5,
                            ny - ss * 0.5,
                            ca(255, 255, 255, 230),
                            DrawTextureParams {
                                dest_size: Some(vec2(ss, ss)),
                                ..Default::default()
                            },
                        );
                        if note.is_ex {
                            let ex_tex = if is_double {
                                app.star_double_ex_tex.as_ref()
                            } else {
                                app.star_ex_tex.as_ref()
                            };
                            if let Some(ex) = ex_tex.or(app.star_ex_tex.as_ref()) {
                                draw_texture_ex(
                                    ex,
                                    cx - ss * 0.5,
                                    ny - ss * 0.5,
                                    ca(255, 255, 255, 230),
                                    DrawTextureParams {
                                        dest_size: Some(vec2(ss, ss)),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    } else {
                        draw_poly(cx, ny, 5, ss * 0.4, 0.0, ca(250, 204, 21, 230));
                    }
                }
            }
            NoteType::Hold => {
                let tail_time = hold_tail_time(note, bpms);
                let tail_dt = tail_time - now;
                let scroll = if is_touch_zone(zone) {
                    scroll_speed * app.touch_speed
                } else {
                    scroll_speed
                };
                let tail_y = judge_y - tail_dt * scroll;
                let hold_tex = if note.is_break {
                    app.hold_break_tex.as_ref()
                } else if note.is_each {
                    app.hold_each_tex.as_ref()
                } else {
                    app.hold_texture.as_ref()
                }
                .or(app.hold_texture.as_ref());
                let hw = HOLD_WIDTH * scale;
                if let Some(tex) = hold_tex {
                    draw_hold_9slice_vertical(tex, cx, ny, tail_y, hw);
                    if note.is_ex {
                        if let Some(ex) = app.hold_ex_tex.as_ref() {
                            draw_hold_9slice_vertical(ex, cx, ny, tail_y, hw);
                        }
                    }
                } else {
                    let hw = HOLD_WIDTH * scale;
                    let top = ny.min(tail_y);
                    let h = (ny - tail_y).abs().max(hw * 0.133);
                    draw_rectangle(cx - hw * 0.2, top, hw * 0.4, h, ca(190, 24, 93, 130));
                    draw_rectangle_lines(
                        cx - hw * 0.2,
                        top,
                        hw * 0.4,
                        h,
                        1.0 * scale,
                        ca(244, 114, 182, 200),
                    );
                    let hr = hw * 0.367;
                    draw_circle(cx, ny, hr, ca(17, 24, 39, 255));
                    draw_circle_lines(cx, ny, hr, hw * 0.1, ca(251, 113, 133, 255));
                    draw_circle(cx, ny, hw * 0.107, ca(253, 164, 175, 255));
                    draw_circle(cx, tail_y, hw * 0.133, ca(251, 113, 133, 220));
                }
                // Judgment center dots (head & tail)
                draw_circle(cx, ny, 2.5 * scale, ca(255, 255, 255, 200));
                draw_circle(cx, tail_y, 2.5 * scale, ca(255, 255, 255, 180));
            }
        }
    }
    // Ghost note at mouse position (hover indicator)
    let (mx, my) = mouse_position();
    //println!("mouse_position {} {}",mx,my);
    if mx >= track_x && mx <= track_x + track_w && my >= track_y && my <= track_y + track_h {
        let dt = (judge_y - my) / scroll_speed;
        let gt = (now + dt).max(0.0);
        let cur_bpm = bpm_at(secs_to_measure(gt, bpms), bpms);
        let beat_s = 60.0 / cur_bpm;
        let grid_s = beat_s / (GRID_DIVISION as f32 / 4.0);
        let gt = (gt / grid_s).round() * grid_s;
        let gy = judge_y - (gt - now) * scroll_speed;
        let glx = mx - (track_x);
        if glx >= 0.0 {
            let glane_i = ((glx / lane_w) as i32).clamp(0, LANE_COUNT as i32 - 1);
            let glane = if glane_i == LANE_COUNT as i32 - 1 {
                9
            } else {
                (glane_i + 1) as u8
            };
            let gcx = track_x + lane_w * glane_i as f32 + lane_w * 0.5;
            let gzone = sanitize_note_zone(crate::app::types::NoteType::Tap, glane);
            if is_touch_zone(gzone) {
                if let Some(tex) = &app.touch_tri_tex {
                    let ratio = tex.width() / tex.height();
                    let ts = 30.0 * scale;
                    let tw = ts;
                    let th = ts / ratio;
                    draw_texture_ex(
                        tex,
                        gcx - tw * 0.5,
                        gy - th * 0.5 + ts * 0.3,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        gcx - tw * 0.5,
                        gy - th * 0.5 - ts * 0.3,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: std::f32::consts::PI,
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        gcx - ts * 0.3 - tw * 0.5,
                        gy - th * 0.5,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: std::f32::consts::FRAC_PI_2,
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        gcx + ts * 0.3 - tw * 0.5,
                        gy - th * 0.5,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: -std::f32::consts::FRAC_PI_2,
                            ..Default::default()
                        },
                    );
                }
                if let Some(pt) = &app.touch_point_tex {
                    draw_texture_ex(
                        pt,
                        gcx - 6.0 * scale,
                        gy - 6.0 * scale,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(12.0 * scale, 12.0 * scale)),
                            ..Default::default()
                        },
                    );
                }
            } else {
                let ghost_alpha = Color::from_rgba(255, 255, 255, 120);
                match app.place_tool {
                    crate::app::types::PlaceTool::Hold => {
                        // Show hold texture ghost
                        let hold_tex = app.hold_texture.as_ref();
                        if let Some(tex) = hold_tex {
                            let hw = HOLD_WIDTH * scale;
                            let tail_y = gy - hw * 1.25;
                            draw_hold_9slice_vertical(tex, gcx, gy, tail_y, hw);
                        } else {
                            let hw = HOLD_WIDTH * 0.4 * scale;
                            draw_rectangle(
                                gcx - hw * 0.5,
                                gy - HOLD_WIDTH * 1.25 * scale,
                                hw,
                                HOLD_WIDTH * 1.25 * scale,
                                Color::from_rgba(244, 114, 182, 80),
                            );
                        }
                    }
                    crate::app::types::PlaceTool::Star => {
                        // Show star texture ghost (same size as tap)
                        if let Some(tex) = app.star_tex.as_ref() {
                            let ss = TAP_SIZE * scale;
                            draw_texture_ex(
                                tex,
                                gcx - ss * 0.5,
                                gy - ss * 0.5,
                                ghost_alpha,
                                DrawTextureParams {
                                    dest_size: Some(vec2(ss, ss)),
                                    ..Default::default()
                                },
                            );
                        } else {
                            draw_poly(
                                gcx,
                                gy,
                                5,
                                11.0 * scale,
                                0.0,
                                Color::from_rgba(250, 204, 21, 100),
                            );
                        }
                    }
                    crate::app::types::PlaceTool::Tap => {
                        if let Some(tex) = &app.tap_texture {
                            draw_texture_ex(
                                tex,
                                gcx - TAP_SIZE * 0.5 * scale,
                                gy - TAP_SIZE * 0.5 * scale,
                                ghost_alpha,
                                DrawTextureParams {
                                    dest_size: Some(vec2(TAP_SIZE * scale, TAP_SIZE * scale)),
                                    ..Default::default()
                                },
                            );
                        } else {
                            draw_circle(
                                gcx,
                                gy,
                                11.0 * scale,
                                Color::from_rgba(244, 114, 182, 100),
                            );
                            draw_circle_lines(
                                gcx,
                                gy,
                                11.0 * scale,
                                2.5 * scale,
                                Color::from_rgba(244, 114, 182, 180),
                            );
                        }
                    }
                }
            }
        }
    }

    // Placement preview for Hold / Star multi-step tools.
    {
        use crate::app::types::{PlaceTool, PlacementState};
        let (mx, my) = mouse_position();
        let inside =
            mx >= track_x && mx <= track_x + track_w && my >= track_y && my <= track_y + track_h;
        // Cursor's snapped chart time and lane (best-effort; only used when inside).
        let cursor_t = if inside {
            let raw = (now + (judge_y - my) / scroll_speed).max(0.0);
            Some(snap_measure(secs_to_measure(raw, bpms)))
        } else {
            None
        };
        // Helper to compute lane center x.
        let lane_cx = |lane: u8| -> f32 {
            let li = if is_touch_zone(sanitize_note_zone(crate::app::types::NoteType::Tap, lane)) {
                LANE_COUNT - 1
            } else {
                (lane.saturating_sub(1) as usize).min(LANE_COUNT - 1)
            };
            track_x + lane_w * li as f32 + lane_w * 0.5
        };
        let t_to_y = |t: f32| -> f32 { judge_y - (measure_to_secs(t, bpms) - now) * scroll_speed };
        // Dashed-line helper.
        let dashed = |cx: f32, y0: f32, y1: f32, col: Color| {
            let h = (y1 - y0).abs();
            if h < 0.5 {
                return;
            }
            let dash = 6.0 * scale;
            let gap = 4.0 * scale;
            let period = dash + gap;
            let top = y0.min(y1);
            let n = (h / period).ceil() as i32;
            for k in 0..n {
                let a = top + (k as f32) * period;
                let b = (a + dash).min(top + h);
                draw_line(cx, a, cx, b, 2.0 * scale, col);
            }
        };

        match app.placement {
            PlacementState::HoldPending { anchor_t, lane } => {
                let cx = lane_cx(lane);
                let ay = t_to_y(anchor_t);
                // Anchor dot
                draw_circle(cx, ay, 6.0 * scale, Color::from_rgba(244, 114, 182, 230));
                draw_circle_lines(
                    cx,
                    ay,
                    6.0 * scale,
                    1.5 * scale,
                    Color::from_rgba(255, 255, 255, 220),
                );
                if let Some(t2) = cursor_t {
                    let by = t_to_y(t2);
                    let hw = HOLD_WIDTH * scale;
                    let (head_y, tail_y) = if ay > by { (ay, by) } else { (by, ay) };
                    if let Some(tex) = app.hold_texture.as_ref() {
                        draw_hold_9slice_vertical(tex, cx, head_y, tail_y, hw);
                    } else {
                        let bar_w = HOLD_WIDTH * 0.4 * scale;
                        let h = (head_y - tail_y).abs();
                        if h > 0.5 {
                            draw_rectangle(
                                cx - bar_w * 0.5,
                                tail_y,
                                bar_w,
                                h,
                                Color::from_rgba(244, 114, 182, 90),
                            );
                            draw_rectangle_lines(
                                cx - bar_w * 0.5,
                                tail_y,
                                bar_w,
                                h,
                                1.0 * scale,
                                Color::from_rgba(244, 114, 182, 180),
                            );
                        }
                    }
                    draw_circle(cx, by, 4.0 * scale, Color::from_rgba(244, 114, 182, 180));
                }
            }
            PlacementState::StarHead { head_t, lane } => {
                let cx = lane_cx(lane);
                let hy = t_to_y(head_t);
                // Star head marker
                let ss = 18.0 * scale;
                if let Some(tex) = &app.star_tex {
                    draw_texture_ex(
                        tex,
                        cx - ss * 0.5,
                        hy - ss * 0.5,
                        Color::from_rgba(255, 255, 255, 230),
                        DrawTextureParams {
                            dest_size: Some(vec2(ss, ss)),
                            ..Default::default()
                        },
                    );
                } else {
                    draw_poly(
                        cx,
                        hy,
                        5,
                        ss * 0.4,
                        0.0,
                        Color::from_rgba(250, 204, 21, 230),
                    );
                }
                if let Some(t2) = cursor_t {
                    if t2 > head_t {
                        let cy = t_to_y(t2);
                        dashed(cx, hy, cy, Color::from_rgba(253, 224, 71, 220));
                        draw_circle(cx, cy, 4.5 * scale, Color::from_rgba(56, 189, 248, 180));
                    }
                }
            }
            PlacementState::StarDelay {
                head_t,
                lane,
                delay_end_t,
            } => {
                let cx = lane_cx(lane);
                let hy = t_to_y(head_t);
                let dy = t_to_y(delay_end_t);
                // Star head
                let ss = 18.0 * scale;
                if let Some(tex) = &app.star_tex {
                    draw_texture_ex(
                        tex,
                        cx - ss * 0.5,
                        hy - ss * 0.5,
                        Color::from_rgba(255, 255, 255, 230),
                        DrawTextureParams {
                            dest_size: Some(vec2(ss, ss)),
                            ..Default::default()
                        },
                    );
                } else {
                    draw_poly(
                        cx,
                        hy,
                        5,
                        ss * 0.4,
                        0.0,
                        Color::from_rgba(250, 204, 21, 230),
                    );
                }
                // Dashed delay segment
                dashed(cx, hy, dy, Color::from_rgba(253, 224, 71, 220));
                // Delay-end handle
                draw_circle(cx, dy, 4.5 * scale, Color::from_rgba(56, 189, 248, 230));
                draw_circle_lines(
                    cx,
                    dy,
                    4.5 * scale,
                    1.5 * scale,
                    Color::from_rgba(255, 255, 255, 220),
                );
                // Travel preview to cursor
                if let Some(t2) = cursor_t {
                    if t2 > delay_end_t {
                        let cy = t_to_y(t2);
                        let bar_w = 6.0 * scale;
                        let top = dy.min(cy);
                        let h = (dy - cy).abs();
                        if h > 0.5 {
                            draw_rectangle(
                                cx - bar_w * 0.5,
                                top,
                                bar_w,
                                h,
                                Color::from_rgba(250, 204, 21, 110),
                            );
                            draw_rectangle_lines(
                                cx - bar_w * 0.5,
                                top,
                                bar_w,
                                h,
                                1.0 * scale,
                                Color::from_rgba(253, 224, 71, 200),
                            );
                        }
                        draw_circle(cx, cy, 5.0 * scale, Color::from_rgba(250, 204, 21, 180));
                    }
                }
                let _ = PlaceTool::Star; // silence unused-warning if tool path changes later
            }
            PlacementState::Idle => {}
        }
    }

    for hit in &app.recording_hits {
        let zone = if hit.lane == 9 { PAD_C_ZONE } else { hit.lane };
        let dt = hit.time - now;
        if dt < -margin_s || dt > margin_s {
            continue;
        }
        let lane_index = if is_touch_zone(zone) {
            LANE_COUNT - 1
        } else {
            (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
        };
        let cx = track_x + lane_w * lane_index as f32 + lane_w * 0.5;
        let ny = judge_y - dt * scroll_speed;
        draw_circle(cx, ny, 4.0 * scale, Color::from_rgba(56, 189, 248, 220));
    }
    // Box selection preview
    if let (Some(start), Some(end)) = (app.box_start, app.box_end) {
        if start != end {
            let x1 = start.x.min(end.x);
            let x2 = start.x.max(end.x);
            let y1 = start.y.min(end.y);
            let y2 = start.y.max(end.y);
            draw_rectangle(x1, y1, x2 - x1, y2 - y1, Color::from_rgba(74, 125, 170, 30));
            draw_rectangle_lines(
                x1,
                y1,
                x2 - x1,
                y2 - y1,
                1.5 * scale,
                Color::from_rgba(74, 125, 170, 180),
            );
        }
    }
    // Multi-select highlight
    for &id in &app.selected_notes {
        if let Some(i) = app.find_note_index(id) {
            if let Some(note) = app.chart.notes.get(i) {
                let zone = sanitize_note_zone(note.note_type, note.lane);
                let dt = note_secs(note, bpms) - now;
                if dt < -margin_s || dt > margin_s {
                    continue;
                }
                let li = if is_touch_zone(zone) {
                    LANE_COUNT - 1
                } else {
                    (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
                };
                let cx = track_x + lane_w * li as f32 + lane_w * 0.5;
                let ny = judge_y - dt * scroll_speed;
                draw_circle(cx, ny, 15.0, Color::from_rgba(250, 204, 21, 80));
            }
        }
    }

    // Paste ghost
    if app.pasting && !app.clipboard.is_empty() {
        let (mx, my) = mouse_position();
        let dt = (judge_y - my) / scroll_speed;
        let raw_secs = (now + dt).max(0.0);
        let grid_step = 1.0 / crate::app::types::GRID_DIVISION as f32;
        let raw_m = secs_to_measure(raw_secs, bpms);
        let target_m = (raw_m / grid_step).round() * grid_step;
        let min_t = app
            .clipboard
            .iter()
            .map(|n| n.time)
            .fold(f32::MAX, f32::min);
        let t_off = target_m - min_t;
        let lx = mx - (track_x);
        let tgt_lane = if lx >= 0.0 {
            let l = (lx / lane_w) as i32;
            let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
            if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
        } else {
            1
        };
        let a_lane = app.clipboard.first().map(|n| n.lane).unwrap_or(1);
        let l_off = tgt_lane as i32 - a_lane as i32;
        for n in &app.clipboard {
            let t_m = n.time + t_off;
            let lane =
                (n.lane as i32 + l_off).clamp(1, crate::app::types::PAD_ZONE_MAX as i32) as u8;
            let zone = sanitize_note_zone(n.note_type, lane);
            let dt2 = measure_to_secs(t_m, bpms) - now;
            if dt2 < -margin_s || dt2 > margin_s {
                continue;
            }
            let li = if is_touch_zone(zone) {
                LANE_COUNT - 1
            } else {
                (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
            };
            let cx = track_x + lane_w * li as f32 + lane_w * 0.5;
            let ny = judge_y - dt2 * scroll_speed;
            if is_touch_zone(zone) || matches!(n.note_type, crate::app::types::NoteType::Touch) {
                if let Some(tex) = &app.touch_tri_tex {
                    let ratio = tex.width() / tex.height();
                    let ts = 20.0 * scale;
                    let tw = ts;
                    let th = ts / ratio;
                    let c = Color::from_rgba(255, 255, 255, 120);
                    draw_texture_ex(
                        tex,
                        cx - tw * 0.5,
                        ny + ts * 0.15 - th * 0.5,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        cx - tw * 0.5,
                        ny - ts * 0.15 - th * 0.5,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: std::f32::consts::PI,
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        cx - ts * 0.15 - tw * 0.5,
                        ny - th * 0.5,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: std::f32::consts::FRAC_PI_2,
                            ..Default::default()
                        },
                    );
                    draw_texture_ex(
                        tex,
                        cx + ts * 0.15 - tw * 0.5,
                        ny - th * 0.5,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(tw, th)),
                            rotation: -std::f32::consts::FRAC_PI_2,
                            ..Default::default()
                        },
                    );
                }
                if let Some(pt) = &app.touch_point_tex {
                    draw_texture_ex(
                        pt,
                        cx - 5.0 * scale,
                        ny - 5.0 * scale,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(10.0 * scale, 10.0 * scale)),
                            ..Default::default()
                        },
                    );
                }
            } else {
                if let Some(tex) = &app.tap_texture {
                    draw_texture_ex(
                        tex,
                        cx - TAP_SIZE * 0.4 * scale,
                        ny - TAP_SIZE * 0.4 * scale,
                        Color::from_rgba(255, 255, 255, 120),
                        DrawTextureParams {
                            dest_size: Some(vec2(TAP_SIZE * 0.8 * scale, TAP_SIZE * 0.8 * scale)),
                            ..Default::default()
                        },
                    );
                } else {
                    draw_circle(cx, ny, 11.0 * scale, Color::from_rgba(244, 114, 182, 80));
                    draw_circle_lines(
                        cx,
                        ny,
                        11.0 * scale,
                        2.0 * scale,
                        Color::from_rgba(244, 114, 182, 160),
                    );
                }
            }
        }
    }

    // Judge line on top
    draw_line(
        track_x,
        judge_y,
        track_x + track_w,
        judge_y,
        2.0 * scale,
        Color::from_rgba(200, 70, 70, 255),
    );

    // ── Progress bar at bottom of timeline ──
    {
        let bar_x = track_x;
        let bar_y = track_y + track_h + 4.0 * scale;
        let bar_w = track_w;
        let bar_h = progress_bar_h - 4.0 * scale;

        // Total song duration: use audio length if available, otherwise fallback to note range.
        let total_dur = if let Some(ref wav) = app.audio_wav_pcm {
            let audio_dur =
                wav.samples.len() as f32 / (wav.sample_rate as f32 * wav.channels as f32).max(1.0);
            audio_dur.max(1.0)
        } else {
            let last_note_end = app
                .chart
                .notes
                .iter()
                .map(|n| {
                    let ns = note_secs(n, bpms);
                    match n.note_type {
                        NoteType::Hold => hold_tail_time(n, bpms),
                        NoteType::Slide => {
                            let max_d = n
                                .slide
                                .iter()
                                .map(|s| s.slide_duration)
                                .fold(0.0_f32, f32::max);
                            ns + mdur_to_secs(max_d, n.time, bpms)
                        }
                        _ => ns,
                    }
                })
                .fold(0.0_f32, f32::max);
            last_note_end.max(1.0)
        };

        // Background
        draw_rectangle(
            bar_x,
            bar_y,
            bar_w,
            bar_h,
            Color::from_rgba(40, 40, 40, 255),
        );
        draw_rectangle_lines(
            bar_x,
            bar_y,
            bar_w,
            bar_h,
            1.0,
            Color::from_rgba(55, 55, 55, 255),
        );

        // Filled portion
        let frac = (now / total_dur).clamp(0.0, 1.0);
        let fill_w = bar_w * frac;
        draw_rectangle(
            bar_x,
            bar_y,
            fill_w,
            bar_h,
            Color::from_rgba(74, 125, 170, 180),
        );

        // Cursor indicator
        let cursor_x = bar_x + fill_w;
        draw_rectangle(
            cursor_x - 1.5 * scale,
            bar_y,
            3.0 * scale,
            bar_h,
            Color::from_rgba(200, 70, 70, 255),
        );

        // Time label
        let time_str = format!(
            "{:.1}s / {:.1}s  (x{:.1})",
            now, total_dur, app.timeline_zoom
        );
        let tw = measure_text(&time_str, None, (12.0 * scale) as u16, 1.0).width;
        draw_text(
            &time_str,
            bar_x + bar_w - tw - 4.0 * scale,
            bar_y + bar_h - 3.0 * scale,
            12.0 * scale,
            Color::from_rgba(148, 163, 184, 255),
        );
    }
}
fn draw_tap_sprite(tex: &Texture2D, cx: f32, cy: f32, size: f32) {
    draw_tap_sprite_c(tex, cx, cy, size, WHITE);
}
fn draw_tap_sprite_c(tex: &Texture2D, cx: f32, cy: f32, size: f32, color: Color) {
    draw_texture_ex(
        tex,
        cx - size * 0.5,
        cy - size * 0.5,
        color,
        DrawTextureParams {
            dest_size: Some(vec2(size, size)),
            ..Default::default()
        },
    );
}

/// Draw hold texture with vertical 9-slice behavior:
/// top cap fixed + middle stretched + bottom cap fixed.
fn draw_hold_9slice_vertical(tex: &Texture2D, cx: f32, y0: f32, y1: f32, width: f32) {
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);

    // Keep cap aspect by converting source-pixel cap height into screen height using width ratio.
    let cap_dest_h = (cap_h * (width / tex_w)).max(1.0);

    // y0/y1 are judgment centers at the center of each fixed cap.
    let top_center = y0.min(y1);
    let bottom_center = y0.max(y1);

    let center_len = (bottom_center - top_center).max(1.0);
    let natural_cap_h = cap_dest_h;
    let (top_h, bottom_h) = if center_len < natural_cap_h {
        let squeezed = (center_len * 0.5).max(1.0);
        (squeezed, squeezed)
    } else {
        (natural_cap_h, natural_cap_h)
    };
    let top_y = top_center - top_h * 0.5;
    let bottom_y = bottom_center - bottom_h * 0.5;
    let body_y = top_y + top_h;
    let body_h = (bottom_y - body_y).max(0.0);
    let x = cx - width * 0.5;

    if top_h > 0.0 {
        draw_texture_ex(
            tex,
            x,
            top_y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: tex_w,
                    h: cap_h,
                }),
                dest_size: Some(vec2(width, top_h)),
                ..Default::default()
            },
        );
    }
    if body_h > 0.0 {
        draw_texture_ex(
            tex,
            x,
            body_y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: cap_h,
                    w: tex_w,
                    h: body_src_h,
                }),
                dest_size: Some(vec2(width, body_h)),
                ..Default::default()
            },
        );
    }
    if bottom_h > 0.0 {
        draw_texture_ex(
            tex,
            x,
            bottom_y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: tex_h - cap_h,
                    w: tex_w,
                    h: cap_h,
                }),
                dest_size: Some(vec2(width, bottom_h)),
                ..Default::default()
            },
        );
    }
}

pub fn draw_hold_9slice_segment(tex: &Texture2D, from: Vec2, to: Vec2, width: f32, tint: Color) {
    let delta = to - from;
    let center_len = delta.length().max(1.0);
    let dir = delta / center_len;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_len = (cap_h * (width / tex_w)).max(1.0);

    let min_cap = 4.0;
    let natural_cap_len = cap_len.max(min_cap);
    let (head_len, tail_len) = if center_len < natural_cap_len {
        let squeezed = (center_len * 0.5).max(1.0);
        (squeezed, squeezed)
    } else {
        (natural_cap_len, natural_cap_len)
    };
    let head_start = from - dir * (head_len * 0.5);
    let tail_start = to - dir * (tail_len * 0.5);
    let body_start = head_start + dir * head_len;
    let body_delta = tail_start - body_start;
    let body_len = body_delta.dot(dir).max(0.0);

    let draw_part = |start: Vec2, part_len: f32, src_y: f32, src_h: f32| {
        if part_len <= 0.0 {
            return;
        }
        let center = start + dir * (part_len * 0.5);
        draw_texture_ex(
            tex,
            center.x - width * 0.5,
            center.y - part_len * 0.5,
            tint,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: src_y,
                    w: tex_w,
                    h: src_h,
                }),
                dest_size: Some(vec2(width, part_len)),
                rotation: angle,
                pivot: Some(center),
                ..Default::default()
            },
        );
    };

    draw_part(head_start, head_len, 0.0, cap_h);
    draw_part(body_start, body_len, cap_h, body_src_h);
    draw_part(tail_start, tail_len, tex_h - cap_h, cap_h);
}
