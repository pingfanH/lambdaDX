use macroquad::color::{Color, WHITE};
use macroquad::math::{vec2, Vec2};
use macroquad::miniquad::FilterMode;
use macroquad::prelude::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_text, draw_texture_ex, load_texture, measure_text, DrawTextureParams};
use lambda_dx::{pad_svg, slide_render};
use lambda_dx::slide::path::{slide_shape_caret, slide_shape_left, slide_shape_line, slide_shape_p, slide_shape_pp, slide_shape_q, slide_shape_qq, slide_shape_right, slide_shape_s, slide_shape_z};
use lambda_dx::state::AppState;
use lambda_dx::types::{hold_tail_time, mdur_to_secs, note_secs, sanitize_note_zone, slide_end_time, Mode, NoteType, PadGeom, RectF, SlideShape, HIT_WINDOW, HOLD_FLY_TIME, HOLD_LENGTH_FRAC, HOLD_SPAWN_FRAC, HOLD_TAIL_FLY_TIME, HOLD_TARGET_OFFSET, HOLD_TRAVEL_TIME, HOLD_WIDTH, PAD_ROTATION_RAD, SLIDE_TRAVEL_TIME, TAP_GROW_FRAC, TAP_RING_OFFSET, TAP_SIZE, TAP_SPAWN_FRAC, TAP_TARGET_OFFSET, TAP_TRAVEL_TIME, TOUCHHOLD_BORDER_BASE, TOUCHHOLD_CROSS_BASE, TOUCHHOLD_END_DIST, TOUCHHOLD_ROT_OFFSET, TOUCHHOLD_SCALE, TOUCHHOLD_START_DIST, TOUCH_CROSS_SIZE, TOUCH_DISAPPEAR_TIME, TOUCH_END_DIST, TOUCH_GROW_FRAC, TOUCH_SCALE, TOUCH_START_DIST, TOUCH_TRAVEL_TIME};
use lambda_dx::types::zone::PadZone;
use lambda_dx::ui::draw_hold_9slice_segment;
use crate::state::PlayerState;
use crate::player_layout::*;
pub fn draw_pad_panel(app: &PlayerState, rect: RectF, pad: PadGeom) {
    let scale = ui_scale(app);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::from_rgba(38, 38, 38, 255),
    );
    draw_text(
        "Pad View",
        rect.x + 12.0 * scale,
        rect.y + 24.0 * scale,
        24.0 * scale,
        Color::from_rgba(180, 180, 180, 255),
    );

    let cx = pad.cx;
    let cy = pad.cy;
    let outer_r = pad.outer_r;
    // Tap spawn center: C zone centroid for alignment
    let spawn_cx = app.pad_svg.as_ref()
        .and_then(|svg| svg.pad_visual_center(&pad))
        .unwrap_or(vec2(cx, cy));

    draw_circle(cx, cy, outer_r, Color::from_rgba(35, 35, 35, 255));

    // Tap spawn point indicator
    draw_circle(spawn_cx.x, spawn_cx.y, 3.0 * scale, Color::from_rgba(255, 255, 255, 180));

    let active_zones: Vec<PadZone> = app.active_pointer_zones.values().copied().collect();
    let feedback_zones: Vec<PadZone> = app.pad_feedback.iter().map(|fb| fb.zone).collect();

    if let Some(ref pad_svg) = app.pad_svg {
        for def in &pad_svg.zones {
            let screen_verts = pad_svg.def_screen_verts(def, &pad);
            let centroid = pad_svg.def_screen_centroid(def, &pad);

            let is_active = active_zones.contains(&def.zone);
            let is_feedback = feedback_zones.contains(&def.zone);

            let (fill_color, stroke_color) = if is_active {
                (
                    Color::from_rgba(74, 125, 170, 180),
                    Color::from_rgba(120, 170, 220, 255),
                )
            } else if is_feedback {
                (
                    Color::from_rgba(230, 149, 48, 160),
                    Color::from_rgba(240, 180, 80, 255),
                )
            } else {
                (
                    Color::from_rgba(50, 50, 50, 255),
                    Color::from_rgba(70, 70, 70, 255),
                )
            };

            pad_svg::draw_polygon_fill(&screen_verts, fill_color);
            pad_svg::draw_polygon_lines(&screen_verts, 2.0 * scale, stroke_color);

            let text_color = if is_active || is_feedback {
                WHITE
            } else {
                Color::from_rgba(160, 160, 160, 255)
            };
            let text_size = 17.0 * scale;
            let text_dims = measure_text(&def.label, None, text_size as _, 1.0);
            draw_text(
                &def.label,
                centroid.x - text_dims.width * 0.5,
                centroid.y + text_dims.height * 0.35,
                text_size,
                text_color,
            );
        }

        // Draw A-zone tap indicators with connecting octagon
        // Draw A-zone tap indicators as a perfect circle centered on spawn_cx
        let dot_r = outer_r + TAP_RING_OFFSET * scale;
        let mut a_dots: Vec<Vec2> = Vec::new();
        for i in 0..8 {
            let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + i as f32 * std::f32::consts::TAU / 8.0;
            a_dots.push(vec2(spawn_cx.x + ang.cos() * dot_r, spawn_cx.y + ang.sin() * dot_r));
        }
        // 圆弧连接 8 个 tap 圆点
        let arc_steps = 8;
        for i in 0..8 {
            let a0 = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + i as f32 * std::f32::consts::TAU / 8.0;
            let a1 = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + (i + 1) as f32 * std::f32::consts::TAU / 8.0;
            for j in 0..arc_steps {
                let t0 = j as f32 / arc_steps as f32;
                let t1 = (j + 1) as f32 / arc_steps as f32;
                let ang0 = a0 + (a1 - a0) * t0;
                let ang1 = a0 + (a1 - a0) * t1;
                draw_line(
                    spawn_cx.x + ang0.cos() * dot_r,
                    spawn_cx.y + ang0.sin() * dot_r,
                    spawn_cx.x + ang1.cos() * dot_r,
                    spawn_cx.y + ang1.sin() * dot_r,
                    2.0 * scale,
                    Color::from_rgba(255, 255, 255, 120),
                );
            }
        }
        for dot in &a_dots {
            draw_circle(dot.x, dot.y, 5.0 * scale, Color::from_rgba(255, 255, 255, 220));
        }
    }

    let current_t = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    let speed_scale = app.play_speed.max(0.1);
    // 编辑时显示 slide 路径的折线和路点圆点
    if let (Some(i), Some(svg)) = (app.editing_slide_path, app.pad_svg.as_ref()) {
        if let Some(note) = app.chart.notes.get(i) {
            if matches!(note.note_type, NoteType::Slide) && !note.slide.is_empty() {
                let spawn_cx = svg.pad_visual_center(&pad).unwrap_or(vec2(pad.cx, pad.cy));
                // 构建路径
                let mut path: Vec<Vec2> = Vec::new();
                if let Some(c) = svg.zone_screen_centroid(PadZone::from(note.lane), &pad) {
                    path.push(c);
                }
                let mut curr_note = note.clone();
                for sl in &note.slide {
                    for seg in &sl.segments {
                        match seg.shape {
                            SlideShape::Q => slide_shape_q(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::QQ => slide_shape_qq(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::P => slide_shape_p(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::PP => slide_shape_pp(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Left => slide_shape_left(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Right => slide_shape_right(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Caret => slide_shape_caret(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Z => slide_shape_z(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::S => slide_shape_s(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Wifi => {
                                // Build three separate Wifi lines for editor preview
                                // Calculate start position
                                let start_pos = {
                                    let idx = (note.lane - 1) as f32;
                                    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                    let target_r = outer_r + TAP_TARGET_OFFSET;
                                    vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                };

                                // Calculate target positions (1-8环形排列)
                                let lane_i = note.lane as i32;
                                let targets = vec![
                                    {
                                        let z = ((lane_i + 3 - 1).rem_euclid(8) + 1) as u8;
                                        let idx = (z - 1) as f32;
                                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                        let target_r = outer_r + TAP_TARGET_OFFSET;
                                        vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                    },
                                    {
                                        let z = ((lane_i + 4 - 1).rem_euclid(8) + 1) as u8;
                                        let idx = (z - 1) as f32;
                                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                        let target_r = outer_r + TAP_TARGET_OFFSET;
                                        vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                    },
                                    {
                                        let z = ((lane_i + 5 - 1).rem_euclid(8) + 1) as u8;
                                        let idx = (z - 1) as f32;
                                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                        let target_r = outer_r + TAP_TARGET_OFFSET;
                                        vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                    },
                                ];

                                // Add all path points for editor preview rendering
                                for target in targets {
                                    path.push(start_pos);
                                    path.push(target);
                                }
                            },

                            _ => slide_shape_line(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                        }
                        if let Some(last_sp) = seg.points.last() {
                            curr_note.lane = last_sp.zone.to_id();
                        }
                    }
                }
                // Polyline
                let line_col = Color::from_rgba(250, 204, 21, 200);
                let line_w = 5. * scale;
                for w in path.windows(2) {
                    draw_line(w[0].x, w[0].y, w[1].x, w[1].y, line_w, line_col);
                }
                // Waypoint dots
                for (k, pt) in path.iter().enumerate() {
                    let is_endpoint = k == 0 || k == path.len() - 1;
                    let r = if is_endpoint { 5.0 * scale } else { 3.5 * scale };
                    draw_circle(pt.x, pt.y, r, Color::from_rgba(255, 220, 50, 200));
                    if is_endpoint {
                        draw_circle_lines(pt.x, pt.y, r, 1.2 * scale, Color::from_rgba(255, 255, 255, 150));
                    }
                }
                let edit_idx = app.editing_slide_idx.unwrap_or(0);
                let banner = format!(
                    "Trajectory edit  #{}:{}  [click=add  Bksp=undo  Esc/E=exit]",
                    i, edit_idx
                );
                draw_rectangle(
                    rect.x + 8.0 * scale, rect.y + 36.0 * scale,
                    (rect.w - 16.0 * scale).max(10.0), 24.0 * scale,
                    Color::from_rgba(250, 204, 21, 60),
                );
                draw_text(&banner, rect.x + 14.0 * scale, rect.y + 53.0 * scale,
                          16.0 * scale, Color::from_rgba(250, 204, 21, 255));
            }
        }
    }

    let bpms = &app.chart.bpms;
    for (_p_idx, note) in app.chart.notes.iter().enumerate() {
        if app.hidden_notes.contains(&note.id) { continue; }
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let ns = note_secs(note, bpms);
        let dt = ns - current_t;
        let dt_scaled = dt / speed_scale;
        let tail_dt = if matches!(note.note_type, NoteType::Hold) {
            hold_tail_time(note, bpms) - current_t
        } else {
            dt
        };
        let tail_dt_scaled = tail_dt / speed_scale;

        let lead_time = if zone <= 8 {
            if matches!(note.note_type, NoteType::Hold) { HOLD_FLY_TIME }
            else if matches!(note.note_type, NoteType::Slide) { SLIDE_TRAVEL_TIME }
            else { TAP_TRAVEL_TIME }
        } else {
            match note.note_type {
                NoteType::Hold => HOLD_TRAVEL_TIME,
                NoteType::Slide => SLIDE_TRAVEL_TIME,
                _ => TOUCH_TRAVEL_TIME,
            }
        };
        let slide_tail_dt = if matches!(note.note_type, NoteType::Slide) {
            slide_end_time(note, bpms) - current_t
        } else {
            tail_dt
        };
        let disappear_time = if matches!(note.note_type, NoteType::Touch) { TOUCH_DISAPPEAR_TIME }
        else if matches!(note.note_type, NoteType::Slide) { 0.3 }
        else { 0.18 };
        if slide_tail_dt < -disappear_time || dt > lead_time {
            continue;
        }

        // ── Slide rendering (shared via slide_render module) ──
        if matches!(note.note_type, NoteType::Slide) && !note.slide.is_empty() {
            let spawn_center = app.pad_svg.as_ref()
                .and_then(|svg| svg.pad_visual_center(&pad))
                .unwrap_or(vec2(cx, cy));
            if let Some(ref svg) = app.pad_svg {
                for sl in &note.slide {
                    let slide_dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.3);
                    let fade_in_s = if let Some(t) = lambda_dx::types::FIXED_SLIDE_FADE_IN {
                        t
                    } else {
                        mdur_to_secs(sl.slide_start_delay, note.time, bpms)
                    }
                        .max(0.0).min(slide_dur_s - 0.001).max(0.001);

                    let dbl = note.is_star;
                    let trail_tex = if sl.slide_is_break { app.slide_break_tex.as_ref() }
                    else if note.is_each { app.slide_each_tex.as_ref() }
                    else { app.slide_tex.as_ref() };
                    let star_variant = if note.is_break {
                        if dbl { app.star_double_break_tex.as_ref() } else { app.star_break_tex.as_ref() }
                    } else if note.is_each {
                        if dbl { app.star_double_each_tex.as_ref() } else { app.star_each_tex.as_ref() }
                    } else {
                        if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() }
                    };
                    let star_fb = if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() };
                    let ex_variant = if note.is_ex {
                        if dbl { app.star_double_ex_tex.as_ref() } else { app.star_ex_tex.as_ref() }
                    } else { None };

                    let tex = slide_render::SlideTextures {
                        trail: trail_tex,
                        star: star_variant.or(star_fb),
                        star_fallback: app.star_tex.as_ref(),
                        star_ex: ex_variant,
                        star_ex_fallback: app.star_ex_tex.as_ref(),
                        wifi: std::array::from_fn(|i| app.wifi_tex[i].as_ref()),
                    };

                    slide_render::draw_slide(
                        note, sl,
                        current_t, ns, slide_dur_s, fade_in_s,
                        &pad, svg, scale, spawn_center, outer_r,
                        &tex, false, speed_scale,
                    );
                }
            }
        }

        if zone <= 8 {
            let idx = (zone - 1) as f32;
            let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
            let dir = vec2(ang.cos(), ang.sin());
            let head_travel = if matches!(note.note_type, NoteType::Slide) { SLIDE_TRAVEL_TIME } else { TAP_TRAVEL_TIME };
            let progress = ((head_travel - dt_scaled) / head_travel).clamp(0.0, 1.0);
            // Phase 1: grow from 0 to 1 at spawn point. Phase 2: fly at full size.
            let size_scale = if progress < TAP_GROW_FRAC {
                progress / TAP_GROW_FRAC
            } else {
                1.0
            };
            let fly_progress = if progress < TAP_GROW_FRAC {
                0.0
            } else {
                (progress - TAP_GROW_FRAC) / (1.0 - TAP_GROW_FRAC)
            };
            let spawn_r = outer_r * TAP_SPAWN_FRAC;
            let target_r = outer_r + TAP_TARGET_OFFSET;
            // r = midpoint (grow: fixed at spawn, fly: moves to target)
            let r = spawn_r + (target_r - spawn_r) * fly_progress;
            let px = spawn_cx.x + dir.x * r;
            let py = spawn_cx.y + dir.y * r;

            if matches!(note.note_type, NoteType::Hold) {
                let h_spawn_r = outer_r * HOLD_SPAWN_FRAC;
                let h_target_r = outer_r + HOLD_TARGET_OFFSET;
                let h_progress = ((HOLD_FLY_TIME - dt_scaled) / HOLD_FLY_TIME).clamp(0.0, 1.0);
                let h_size_scale = if h_progress < TAP_GROW_FRAC { h_progress / TAP_GROW_FRAC } else { 1.0 };
                let h_fly_progress = if h_progress < TAP_GROW_FRAC { 0.0 } else { (h_progress - TAP_GROW_FRAC) / (1.0 - TAP_GROW_FRAC) };
                let full_hold_len = (h_target_r - h_spawn_r) * HOLD_LENGTH_FRAC;
                // Uniform scale: length and width both scale 0→1 during grow
                let hold_half = (full_hold_len * h_size_scale * 0.5).max(2.0);
                // Head flies during fly phase using hold's own fly time
                let head_fly_r = h_spawn_r + (h_target_r - h_spawn_r) * h_fly_progress;
                let head_r = (head_fly_r + hold_half).min(h_target_r);
                // Tail lags at spawn, flies to target in last HOLD_TAIL_FLY_TIME seconds
                let tail_dt = hold_tail_time(note, bpms) - current_t;
                let tail_fly = if tail_dt_scaled <= HOLD_TAIL_FLY_TIME {
                    (1.0 - tail_dt_scaled / HOLD_TAIL_FLY_TIME).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let tail_base = h_spawn_r + (h_target_r - h_spawn_r) * tail_fly;
                let tail_r = (tail_base - hold_half).max(h_spawn_r * 0.1);
                let hx = spawn_cx.x + dir.x * head_r;
                let hy = spawn_cx.y + dir.y * head_r;
                let tx = spawn_cx.x + dir.x * tail_r;
                let ty = spawn_cx.y + dir.y * tail_r;
                // Width scales 0→1 during grow, body length stays full
                let hold_w = HOLD_WIDTH * scale * h_size_scale;
                let hold_tex = if note.is_break {
                    app.hold_break_tex.as_ref()
                } else if note.is_each {
                    app.hold_each_tex.as_ref()
                } else {
                    app.hold_texture.as_ref()
                };
                if let Some(tex) = hold_tex.or(app.hold_texture.as_ref()) {
                    draw_hold_9slice_segment(tex, vec2(hx, hy), vec2(tx, ty), hold_w.max(1.0), Color::from_rgba(255, 255, 255, 255));
                    if note.is_ex {
                        if let Some(ex_tex) = app.hold_ex_tex.as_ref() {
                            draw_hold_9slice_segment(ex_tex, vec2(hx, hy), vec2(tx, ty), hold_w.max(1.0), Color::from_rgba(255, 255, 255, 255));
                        }
                    }
                } else {
                    draw_line(hx, hy, tx, ty, HOLD_WIDTH * 0.233 * scale * h_size_scale, Color::from_rgba(251, 113, 133, 200));
                    draw_circle(tx, ty, HOLD_WIDTH * 0.167 * scale * h_size_scale, Color::from_rgba(253, 164, 175, 255));
                }
            }

            // Slide head star is now drawn inside slide_render::draw_slide
            if !matches!(note.note_type, NoteType::Hold | NoteType::Slide) {
                let ts = TAP_SIZE * scale * size_scale;
                let tap_tex = if note.is_break {
                    app.tap_break_tex.as_ref()
                } else if note.is_each {
                    app.tap_each_tex.as_ref()
                } else {
                    app.tap_texture.as_ref()
                };
                if let Some(tex) = tap_tex.or(app.tap_texture.as_ref()) {
                    draw_texture_ex(tex, px - ts * 0.5, py - ts * 0.5, WHITE, DrawTextureParams {
                        dest_size: Some(vec2(ts, ts)),
                        ..Default::default()
                    });
                    // Ex overlay on top, same size
                    if note.is_ex {
                        if let Some(ex_tex) = app.tap_ex_tex.as_ref() {
                            draw_texture_ex(ex_tex, px - ts * 0.5, py - ts * 0.5, WHITE, DrawTextureParams {
                                dest_size: Some(vec2(ts, ts)),
                                ..Default::default()
                            });
                        }
                    }
                } else {
                    let tr = TAP_SIZE * 0.375 * scale * size_scale;
                    draw_circle(px, py, tr, Color::from_rgba(17, 24, 39, 255));
                    draw_circle_lines(px, py, tr, tr * 0.25, Color::from_rgba(244, 114, 182, 255));
                    draw_circle(px, py, tr * 0.317, Color::from_rgba(249, 168, 212, 255));
                }
            }

            if dt.abs() <= HIT_WINDOW {
                draw_circle_lines(px, py, TAP_SIZE * 0.53 * scale, 2.0 * scale, Color::from_rgba(255, 255, 255, 220));
            }
        } else {
            let Some(center) = app
                .pad_svg
                .as_ref()
                .and_then(|svg| svg.zone_screen_centroid(PadZone::from(zone), &pad))
            else {
                continue;
            };
            let travel = match note.note_type {
                NoteType::Hold => HOLD_TRAVEL_TIME,
                _ => TOUCH_TRAVEL_TIME,
            };
            let raw = (travel - dt_scaled) / travel;
            let progress = smoothstep(raw.clamp(0.0, 1.0));
            // Phase 1: fade in 0→255. Phase 2: animate movement.
            let alpha = if progress < TOUCH_GROW_FRAC {
                (progress / TOUCH_GROW_FRAC * 255.0) as u8
            } else {
                255
            };
            let move_progress = if progress < TOUCH_GROW_FRAC {
                0.0
            } else {
                (progress - TOUCH_GROW_FRAC) / (1.0 - TOUCH_GROW_FRAC)
            };
            let dist = (TOUCH_START_DIST + (TOUCH_END_DIST - TOUCH_START_DIST) * move_progress) * TOUCH_SCALE * scale;
            let ts = TOUCH_CROSS_SIZE * TOUCH_SCALE * scale;

            // Regular touch cross (skip for hold)
            if !matches!(note.note_type, NoteType::Hold) {
                let tri_tex = if note.is_each { app.touch_tri_each_tex.as_ref() } else { app.touch_tri_tex.as_ref() };
                if let Some(tex) = tri_tex {
                    let ratio = tex.width() / tex.height();
                    let tw = ts;
                    let th = ts / ratio;
                    let draw_tri = |cx: f32, cy: f32, rot: f32| {
                        draw_texture_ex(tex, cx - tw * 0.5, cy - th * 0.5,
                                        Color::from_rgba(255, 255, 255, alpha),
                                        DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: rot, ..Default::default() });
                    };
                    draw_tri(center.x, center.y + dist, 0.0);
                    draw_tri(center.x, center.y - dist, std::f32::consts::PI);
                    draw_tri(center.x - dist, center.y, std::f32::consts::FRAC_PI_2);
                    draw_tri(center.x + dist, center.y, -std::f32::consts::FRAC_PI_2);
                }
            }
            // Center dot (for non-hold; hold draws it later on top)
            if !matches!(note.note_type, NoteType::Hold) {
                let pt_tex = if note.is_each { app.touch_point_each_tex.as_ref() } else { app.touch_point_tex.as_ref() };
                if let Some(tex) = pt_tex {
                    let ps = ts * 0.4;
                    draw_texture_ex(tex, center.x - ps * 0.5, center.y - ps * 0.5,
                                    Color::from_rgba(255, 255, 255, alpha),
                                    DrawTextureParams { dest_size: Some(vec2(ps, ps)), ..Default::default() });
                }
            }

            if matches!(note.note_type, NoteType::Hold) {
                // Touch hold: 4-texture cross rotated 45°, with progress border
                let hold_progress = ((current_t - ns) / (hold_tail_time(note, bpms) - ns).max(0.01)).clamp(0.0, 1.0);
                let hold_dist = (TOUCHHOLD_START_DIST + (TOUCHHOLD_END_DIST - TOUCHHOLD_START_DIST) * move_progress) * TOUCHHOLD_SCALE * scale;
                let d = hold_dist * 0.707; // √2/2 for diagonal
                // Cross rotated 45° CW from regular touch, starting top-right
                let hts = TOUCHHOLD_CROSS_BASE * TOUCHHOLD_SCALE * scale;
                let ro = TOUCHHOLD_ROT_OFFSET;
                let positions = [
                    (center.x + d, center.y - d, -3.0 * std::f32::consts::FRAC_PI_4 + ro), // top-right (0)
                    (center.x + d, center.y + d, -std::f32::consts::FRAC_PI_4 + ro),        // bottom-right (1)
                    (center.x - d, center.y + d, std::f32::consts::FRAC_PI_4 + ro),         // bottom-left (2)
                    (center.x - d, center.y - d, 3.0 * std::f32::consts::FRAC_PI_4 + ro),   // top-left (3)
                ];
                for (i, (px, py, rot)) in positions.iter().enumerate() {
                    if let Some(tex) = &app.touchhold_tex[i] {
                        let ratio = tex.width() / tex.height();
                        let tw = hts;
                        let th = hts / ratio;
                        draw_texture_ex(tex, px - tw * 0.5, py - th * 0.5,
                                        Color::from_rgba(255, 255, 255, alpha),
                                        DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: *rot, ..Default::default() });
                    }
                }
                // Progress border: shader-based clockwise sweep
                if let Some(border) = &app.touchhold_border_tex {
                    let bs = TOUCHHOLD_BORDER_BASE * TOUCHHOLD_SCALE * scale;
                    // Ghost ring
                    draw_texture_ex(border, center.x - bs * 0.5, center.y - bs * 0.5,
                                    Color::from_rgba(255, 255, 255, 0),
                                    DrawTextureParams { dest_size: Some(vec2(bs, bs)), ..Default::default() });
                    // Shader sweep
                    if let Some(ref mat) = app.mask_material {
                        macroquad::material::gl_use_material(mat);
                        mat.set_uniform("progress", hold_progress);
                    }
                    draw_texture_ex(border, center.x - bs * 0.5, center.y - bs * 0.5,
                                    WHITE,
                                    DrawTextureParams { dest_size: Some(vec2(bs, bs)), ..Default::default() });
                    if app.mask_material.is_some() {
                        macroquad::material::gl_use_default_material();
                    }
                }
                // Center dot on top for hold
                let pt_tex = if note.is_each { app.touch_point_each_tex.as_ref() } else { app.touch_point_tex.as_ref() };
                if let Some(tex) = pt_tex {
                    let ps = hts * 0.4;
                    draw_texture_ex(tex, center.x - ps * 0.5, center.y - ps * 0.5,
                                    Color::from_rgba(255, 255, 255, alpha),
                                    DrawTextureParams { dest_size: Some(vec2(ps, ps)), ..Default::default() });
                }
            }
        }
    }


    // draw_text(
    //     "Pad zones: A1~A8(Outer) + B1~B8(Inner) + C1(Center) + D1~8(Left) + E1~8(Right)",
    //     rect.x + 12.0 * scale,
    //     rect.y + rect.h - 30.0 * scale,
    //     18.0 * scale,
    //     Color::from_rgba(165, 180, 252, 255),
    // );
}

pub async fn load_note_textures(app: &mut PlayerState) {
    let tap_candidates = [
        "tap.png",
        "Skins/classic/tap.png",
        "skins/classic/tap.png",
    ];
    for path in tap_candidates {
        match load_texture(path).await {
            Ok(tex)=>{
                tex.set_filter(FilterMode::Linear);
                app.tap_texture = Some(tex);
                break;
            },
            Err(e)=>{
                println!("e:{}",e);
            }
        }
    }

    let hold_candidates = [
        "hold.png",
        "Skins/classic/hold.png",
        "skins/classic/hold.png",
    ];
    for path in hold_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_texture = Some(tex);
            break;
        }
    }

    let touch_tri_candidates = ["Skins/classic/touch.png", "touch.png"];
    for path in touch_tri_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_tri_tex = Some(tex);
            break;
        }
    }
    let touch_pt_candidates = ["Skins/classic/touch_point.png", "touch_point.png"];
    for path in touch_pt_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_point_tex = Some(tex);
            break;
        }
    }
    // Each variant textures
    for path in ["Skins/classic/tap_each.png", "tap_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/hold_each.png", "hold_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/touch_each.png", "touch_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_tri_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/touch_point_each.png", "touch_point_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_point_each_tex = Some(tex);
            break;
        }
    }
    // Touch hold textures (4-frame cross, rotated 45°)
    for (i, name) in ["touchhold_0", "touchhold_1", "touchhold_2", "touchhold_3"].iter().enumerate() {
        for path in [format!("Skins/classic/{name}.png"), format!("{name}.png")] {
            if let Ok(tex) = load_texture(&path).await {
                tex.set_filter(FilterMode::Linear);
                app.touchhold_tex[i] = Some(tex);
                break;
            }
        }
    }
    for path in ["Skins/classic/touchhold_border.png", "touchhold_border.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touchhold_border_tex = Some(tex);
            break;
        }
    }
    // Slide textures
    for path in ["Skins/classic/slide.png", "slide.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.slide_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/slide_each.png", "slide_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.slide_each_tex = Some(tex);
            break;
        }
    }
    for i in 0..11 {
        for path in [format!("Skins/classic/wifi_{i}.png"), format!("wifi_{i}.png")] {
            if let Ok(tex) = load_texture(&path).await {
                tex.set_filter(FilterMode::Linear);
                app.wifi_tex[i as usize] = Some(tex);
                break;
            }
        }
    }

    // Star textures
    for path in ["Skins/classic/star.png", "star.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_each.png", "star_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_break.png", "star_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double.png", "star_double.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double_each.png", "star_double_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_each_tex = Some(tex);
            break;
        }
    }

    // Break textures
    for path in ["Skins/classic/tap_break.png", "tap_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/hold_break.png", "hold_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/slide_break.png", "slide_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.slide_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double_break.png", "star_double_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_break_tex = Some(tex);
            break;
        }
    }
    // Ex overlay textures
    for path in ["Skins/classic/tap_ex.png", "tap_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_ex_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/hold_ex.png", "hold_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_ex_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_ex.png", "star_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_ex_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double_ex.png", "star_double_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_ex_tex = Some(tex);
            break;
        }
    }

    if app.tap_texture.is_none() {
        app.set_status("tap texture not found (tried tap.png / Skins/classic/tap.png)".to_string());
    } else if app.hold_texture.is_none() {
        app.set_status("hold texture not found (tried hold.png / Skins/classic/hold.png)".to_string());
    }
}

pub fn compute_pad_geom(panel: RectF) -> PadGeom {
    let cx = panel.x + panel.w * 0.5;
    let cy = panel.y + panel.h * 0.5;
    let outer_r = panel.w.min(panel.h) * 0.42;
    PadGeom { cx, cy, outer_r }
}

fn smoothstep(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
