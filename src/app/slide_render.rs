use super::pad_svg::PadSvgDef;
use super::slide::segmentation;
use super::types::{
    Note, NOTE_LOCK_DISTANCE, NOTE_OUTER_DISTANCE, PAD_ROTATION_RAD, PadGeom, SLIDE_MIN_DURATION_S,
    SLIDE_TILE_SCALE, SLIDE_TILE_SIZE, SLIDE_TILE_SPACING, STAR_SIZE, Slide, SlideShape,
    TAP_TARGET_OFFSET,
};
use crate::app::slide::path::{
    slide_shape_caret, slide_shape_left, slide_shape_line, slide_shape_p, slide_shape_pp,
    slide_shape_q, slide_shape_qq, slide_shape_right, slide_shape_s, slide_shape_z,
};
use crate::app::types::zone::PadZone;
use macroquad::prelude::*;
use macroquad::texture::{DrawTextureParams, Texture2D};

/// Resolved textures for a single draw_slide call.
/// The caller picks the appropriate variant; the function just uses what's given.
pub struct SlideTextures<'a> {
    pub trail: Option<&'a Texture2D>,
    pub star: Option<&'a Texture2D>,
    pub star_fallback: Option<&'a Texture2D>,
    pub star_ex: Option<&'a Texture2D>,
    pub star_ex_fallback: Option<&'a Texture2D>,
    pub wifi: [Option<&'a Texture2D>; 11],
}

/// Build the standard Slide polyline used by both rendering and judgment.
/// Wifi uses a separate three-track renderer and is intentionally omitted.
pub fn build_slide_path(
    note: &Note,
    slide: &Slide,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
) -> Vec<Vec2> {
    let mut path = Vec::new();
    let start = if note.lane <= 8 {
        let idx = (note.lane - 1) as f32;
        let angle =
            -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
        let radius = outer_r + TAP_TARGET_OFFSET;
        Some(spawn_cx + vec2(angle.cos(), angle.sin()) * radius)
    } else {
        svg.zone_screen_centroid(PadZone::from(note.lane), pad)
    };
    if let Some(start) = start {
        path.push(start);
    }

    let mut current = note.clone();
    for segment in &slide.segments {
        match segment.shape {
            SlideShape::Q => slide_shape_q(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::QQ => slide_shape_qq(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::P => slide_shape_p(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::PP => slide_shape_pp(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Left => slide_shape_left(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Right => slide_shape_right(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Caret => slide_shape_caret(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Z => slide_shape_z(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::S => slide_shape_s(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Wifi => {}
            SlideShape::Line | SlideShape::VShape | SlideShape::BigV => slide_shape_line(
                &mut path, &current, segment, outer_r, spawn_cx, pad, svg, scale,
            ),
        }
        if let Some(last) = segment.points.last() {
            current.lane = last.zone.to_id();
        }
    }
    path
}

/// Draw a single slide on the pad surface: path tiles + head star + flying star.
///
/// `note` — parent note (provides lane, flags)
/// `slide` — the sub-slide to render
/// `current_t` — current playback time, in seconds
/// `ns` — note head time, in seconds
/// `slide_dur_s` — total slide span from the head to the tail, in seconds
/// `start_delay_s` — delay from the note head to Slide movement, in seconds
/// `pad` — pad geometry
/// `svg` — parsed SVG zone definitions
/// `spawn_cx` — screen-space tap spawn center (C-zone centroid)
/// `outer_r` — pad outer radius in screen space
/// `show_full` — true to render the entire trail at full alpha (static view)
/// `completed_areas` — number of leading Sensor areas already completed
pub fn draw_slide(
    note: &Note,
    slide: &Slide,
    current_t: f32,
    ns: f32,
    slide_dur_s: f32,
    start_delay_s: f32,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
    tex: &SlideTextures,
    show_full: bool,
    speed_scale: f32,
    completed_areas: usize,
) {
    // `slide_dur_s` is the total span from the head (tail = ns + slide_dur_s).
    // The star motion fills the `[start_delay, total]` window; the travel time
    // is therefore `total - start_delay`.
    let slide_start_s = ns + start_delay_s;
    let slide_end_s = ns + slide_dur_s;
    let travel_dur_s = (slide_dur_s - start_delay_s).max(SLIDE_MIN_DURATION_S);
    let fade_duration_s = 0.2_f32.min(travel_dur_s).max(0.001);
    let dt = ns - current_t;
    // Scale the approach time by the play speed so the head star flies and
    // the trail culls in musical time (matching taps and MajdataView, where
    // `AudioTime` advances faster at higher playback speed).
    let dt_scaled = dt / speed_scale.max(0.1);
    // The slide head star uses the same radial flight as a Tap note.
    let head_speed = super::types::note_flight_speed(note);
    let head_lead = super::types::note_lead_time(head_speed);

    // ── Time culling (skip when not show_full) ──
    if !show_full {
        if !(dt_scaled <= head_lead && current_t <= slide_end_s + 0.2) {
            return;
        }
    }

    // ── 构建路径：起点 + 各 segment ──
    let mut path: Vec<Vec2> = Vec::new();

    // 起点：A 区用外环 tap 圆点位置
    let start_pt = if note.lane <= 8 {
        let idx = (note.lane - 1) as f32;
        let ang =
            -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
        let target_r = outer_r + TAP_TARGET_OFFSET;
        Some(vec2(
            spawn_cx.x + ang.cos() * target_r,
            spawn_cx.y + ang.sin() * target_r,
        ))
    } else {
        svg.zone_screen_centroid(PadZone::from(note.lane), pad)
    };
    if let Some(c) = start_pt {
        path.push(c);
    }

    let mut curr_note = note.clone();
    for seg in &slide.segments {
        match seg.shape {
            SlideShape::Q => slide_shape_q(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::QQ => slide_shape_qq(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::P => slide_shape_p(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::PP => slide_shape_pp(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Left => slide_shape_left(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Right => slide_shape_right(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Caret => slide_shape_caret(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
            SlideShape::Z => slide_shape_z(
                &mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale,
            ),
            SlideShape::S => slide_shape_s(
                &mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale,
            ),
            SlideShape::Wifi => {
                let start_pos = {
                    let idx = (note.lane - 1) as f32;
                    let ang = -std::f32::consts::FRAC_PI_2
                        + PAD_ROTATION_RAD
                        + idx * std::f32::consts::TAU / 8.0;
                    let target_r = outer_r + TAP_TARGET_OFFSET;
                    vec2(
                        spawn_cx.x + ang.cos() * target_r,
                        spawn_cx.y + ang.sin() * target_r,
                    )
                };

                let lane_i = note.lane as i32;
                let targets = [
                    {
                        let z = ((lane_i + 3 - 1).rem_euclid(8) + 1) as u8;
                        let idx = (z - 1) as f32;
                        let ang = -std::f32::consts::FRAC_PI_2
                            + PAD_ROTATION_RAD
                            + idx * std::f32::consts::TAU / 8.0;
                        let target_r = outer_r + TAP_TARGET_OFFSET;
                        vec2(
                            spawn_cx.x + ang.cos() * target_r,
                            spawn_cx.y + ang.sin() * target_r,
                        )
                    },
                    {
                        let z = ((lane_i + 4 - 1).rem_euclid(8) + 1) as u8;
                        let idx = (z - 1) as f32;
                        let ang = -std::f32::consts::FRAC_PI_2
                            + PAD_ROTATION_RAD
                            + idx * std::f32::consts::TAU / 8.0;
                        let target_r = outer_r + TAP_TARGET_OFFSET;
                        vec2(
                            spawn_cx.x + ang.cos() * target_r,
                            spawn_cx.y + ang.sin() * target_r,
                        )
                    },
                    {
                        let z = ((lane_i + 5 - 1).rem_euclid(8) + 1) as u8;
                        let idx = (z - 1) as f32;
                        let ang = -std::f32::consts::FRAC_PI_2
                            + PAD_ROTATION_RAD
                            + idx * std::f32::consts::TAU / 8.0;
                        let target_r = outer_r + TAP_TARGET_OFFSET;
                        vec2(
                            spawn_cx.x + ang.cos() * target_r,
                            spawn_cx.y + ang.sin() * target_r,
                        )
                    },
                ];

                // ── Head star (pre-judge flying in from center) ──
                if show_full {
                    let head_pt = path[0];
                    let ss = STAR_SIZE * scale;
                    let star_used = tex.star.or(tex.star_fallback);
                    if let Some(st) = star_used {
                        draw_texture_ex(
                            st,
                            head_pt.x - ss * 0.5,
                            head_pt.y - ss * 0.5,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(ss, ss)),
                                ..Default::default()
                            },
                        );
                    }
                } else if current_t < ns && !note.is_tapless {
                    // Same radial flight as a Tap: grow at the inner lock
                    // radius, then fly out to the target ring.
                    let head_motion = super::types::note_radial_motion(
                        dt_scaled,
                        head_speed,
                        outer_r,
                        TAP_TARGET_OFFSET,
                    );
                    let size_scale = head_motion.map(|m| m.scale).unwrap_or(0.0);
                    let fly_progress = head_motion.map(|m| m.progress).unwrap_or(0.0);

                    let idx = (note.lane - 1) as f32;
                    let ang = -std::f32::consts::FRAC_PI_2
                        + PAD_ROTATION_RAD
                        + idx * std::f32::consts::TAU / 8.0;
                    let lock_r = super::types::note_lock_radius(outer_r, TAP_TARGET_OFFSET);
                    let r = head_motion.map(|m| m.radius).unwrap_or(lock_r);
                    let px = spawn_cx.x + ang.cos() * r;
                    let py = spawn_cx.y + ang.sin() * r;
                    let ss = STAR_SIZE * scale * size_scale;
                    let star_rot = fly_progress * std::f32::consts::TAU;
                    let star_used = tex.star.or(tex.star_fallback);
                    if let Some(st) = star_used {
                        draw_texture_ex(
                            st,
                            px - ss * 0.5,
                            py - ss * 0.5,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(ss, ss)),
                                rotation: star_rot,
                                ..Default::default()
                            },
                        );
                        if let Some(ex_tex) = tex.star_ex.or(tex.star_ex_fallback) {
                            draw_texture_ex(
                                ex_tex,
                                px - ss * 0.5,
                                py - ss * 0.5,
                                WHITE,
                                DrawTextureParams {
                                    dest_size: Some(vec2(ss, ss)),
                                    rotation: star_rot,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }

                // ── Tile alpha ──
                let path_alpha = if show_full || current_t >= ns {
                    220u8
                } else {
                    ((220.0 * (current_t - (ns - fade_duration_s)) / fade_duration_s)
                        .clamp(0.0, 220.0)) as u8
                };

                // ── Flying star progress (0..1) ──
                let star_t = if !show_full && current_t >= slide_start_s {
                    ((current_t - slide_start_s) / travel_dur_s.max(0.001)).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let sprite_count = 11;
                let wifi_boundaries = [0usize, 1, 4, 6, 11];
                let area_hidden_until = wifi_boundaries[completed_areas.min(4)];

                for (j, target) in targets.iter().enumerate() {
                    let dir = (*target - start_pos).normalize_or_zero();
                    let seg_len = (*target - start_pos).length().max(0.001);
                    let angle = dir.y.atan2(dir.x) + std::f32::consts::PI + 112.0_f32.to_radians();
                    let star_dist = star_t * seg_len;
                    let star_pos = start_pos + dir * star_dist;
                    let step_size = seg_len / (sprite_count - 1) as f32 * 0.83;
                    // Wifi has three independent straight tracks, so its
                    // bars do not go through the shared path segmentation.
                    // Hide each bar once the star has crossed its position.
                    let star_hidden_until = if show_full {
                        0
                    } else if star_t >= 1.0 - 0.001 {
                        sprite_count
                    } else {
                        (0..sprite_count)
                            .take_while(|index| *index as f32 * step_size < star_dist)
                            .count()
                    };
                    let hidden_until = area_hidden_until.max(star_hidden_until);

                    let is_middle = j == 1;

                    // ── Tiles (only middle line gets wifi textures) ──
                    for i in 0..sprite_count {
                        if i < hidden_until {
                            continue;
                        }
                        let dist = i as f32 * step_size;
                        let sprite_pos = start_pos + dir * dist;

                        if is_middle {
                            if let Some(t) = tex.wifi[i] {
                                let tw = t.width() * scale * SLIDE_TILE_SCALE;
                                let th = t.height() * scale * SLIDE_TILE_SCALE;
                                draw_texture_ex(
                                    t,
                                    sprite_pos.x - tw * 0.5,
                                    sprite_pos.y - th * 0.5,
                                    Color::from_rgba(255, 255, 255, path_alpha),
                                    DrawTextureParams {
                                        dest_size: Some(vec2(tw, th)),
                                        rotation: angle,
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    }

                    // ── Flying star (post-judge, along this line) ──
                    if !show_full && current_t >= ns && current_t <= slide_end_s {
                        let intro = if current_t < slide_start_s {
                            ((current_t - ns) / (slide_start_s - ns).max(0.001)).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        let ss = STAR_SIZE * scale * (0.5 + intro);
                        let star_alpha = (intro * 255.0) as u8;
                        let star_used = tex.star.or(tex.star_fallback);
                        if let Some(st) = star_used {
                            draw_texture_ex(
                                st,
                                star_pos.x - ss * 0.5,
                                star_pos.y - ss * 0.5,
                                Color::from_rgba(255, 255, 255, star_alpha),
                                DrawTextureParams {
                                    dest_size: Some(vec2(ss, ss)),
                                    rotation: angle,
                                    ..Default::default()
                                },
                            );
                            if let Some(ex_tex) = tex.star_ex.or(tex.star_ex_fallback) {
                                draw_texture_ex(
                                    ex_tex,
                                    star_pos.x - ss * 0.5,
                                    star_pos.y - ss * 0.5,
                                    Color::from_rgba(255, 255, 255, star_alpha),
                                    DrawTextureParams {
                                        dest_size: Some(vec2(ss, ss)),
                                        rotation: angle,
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    }
                }
            }

            _ => slide_shape_line(
                &mut path, &curr_note, seg, outer_r, spawn_cx, pad, svg, scale,
            ),
        }
        // 下一段从上段的终点 lane 开始
        if let Some(last_sp) = seg.points.last() {
            curr_note.lane = last_sp.zone.to_id();
        }
    }

    if path.len() < 2 {
        return;
    }

    // ── Segment lengths ──
    let seg_lens: Vec<f32> = path
        .windows(2)
        .map(|w| (w[1] - w[0]).length().max(0.001))
        .collect();
    let total_len: f32 = seg_lens.iter().sum();

    // ── Alpha & star position ──
    let (path_alpha, star_dist_along) = if show_full {
        (220u8, -1.0_f32) // all tiles visible, star at start
    } else {
        let alpha = if current_t >= ns {
            220
        } else {
            ((220.0 * (current_t - (ns - fade_duration_s)) / fade_duration_s)
                .clamp(0.0, 220.0)) as u8
        };
        let star_t = if current_t < slide_start_s {
            0.0
        } else {
            ((current_t - slide_start_s) / travel_dur_s.max(0.001)).clamp(0.0, 1.0)
        };
        (alpha, star_t * total_len)
    };

    // ── point_at helper ──
    let point_at = |d: f32| -> (Vec2, f32) {
        let mut acc = 0.0;
        for (i, w) in path.windows(2).enumerate() {
            let len = seg_lens[i];
            if d <= acc + len {
                let local = (d - acc) / len;
                let p = w[0] + (w[1] - w[0]) * local;
                let dir = (w[1] - w[0]).normalize_or_zero();
                return (p, dir.y.atan2(dir.x));
            }
            acc += len;
        }
        let last = path.windows(2).last().unwrap();
        let dir = (last[1] - last[0]).normalize_or_zero();
        (*path.last().unwrap(), dir.y.atan2(dir.x))
    };

    // ── Path tiles ──
    let (tw, th) = if let Some(t) = tex.trail {
        (
            t.width() * scale * SLIDE_TILE_SCALE,
            t.height() * scale * SLIDE_TILE_SCALE,
        )
    } else {
        (SLIDE_TILE_SIZE * scale, SLIDE_TILE_SIZE * scale)
    };
    let spacing = SLIDE_TILE_SPACING * scale;

    let segmentation = segmentation::build(&path, spacing, svg, pad);
    // Derive the visual completion from the same path distance as the moving
    // star. Autoplay removes a complete segment, not individual bars.
    let mut bar_distances = vec![0.0; segmentation.bars.len()];
    for index in 1..segmentation.bars.len() {
        bar_distances[index] = bar_distances[index - 1]
            + segmentation.bars[index - 1]
                .position
                .distance(segmentation.bars[index].position);
    }
    let star_completed_areas = if show_full || star_dist_along < 0.0 {
        0
    } else if star_dist_along >= total_len - 0.001 {
        segmentation.judge_segments.len()
    } else {
        segmentation
            .judge_segments
            .iter()
            .take_while(|segment| {
                let last_bar = segment.end_bar.saturating_sub(1);
                bar_distances.get(last_bar).copied().unwrap_or(f32::MAX) <= star_dist_along
            })
            .count()
    };
    let completed_areas = completed_areas.max(star_completed_areas);
    let hidden_until = if completed_areas == 0 {
        0
    } else {
        segmentation
            .judge_segments
            .get(completed_areas - 1)
            .map(|segment| segment.end_bar)
            .unwrap_or(segmentation.bars.len())
    };
    for (bar_index, bar) in segmentation.bars.iter().enumerate() {
        if bar_index < hidden_until {
            continue;
        }
        if let Some(t) = tex.trail {
            draw_texture_ex(
                t,
                bar.position.x - tw * 0.5,
                bar.position.y - th * 0.5,
                Color::from_rgba(255, 255, 255, path_alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(tw, th)),
                    rotation: bar.rotation,
                    ..Default::default()
                },
            );
        }
    }

    // ── Original polyline on top of tiles ──
    // let line_alpha: u8 = if show_full { 200 } else { path_alpha.saturating_add(80).min(255) };
    // let line_color = Color::from_rgba(250, 204, 21, line_alpha);
    // let line_w = 5. * scale;
    // for w in path.windows(2) {
    //     draw_line(w[0].x, w[0].y, w[1].x, w[1].y, line_w, line_color);
    // }

    // ── Waypoint dots ──
    // for (i, pt) in path.iter().enumerate() {
    //     let is_endpoint = i == 0 || i == path.len() - 1;
    //     let r = if is_endpoint { 5.0 * scale } else { 3.5 * scale };
    //     draw_circle(pt.x, pt.y, r, Color::from_rgba(255, 220, 50, 200));
    //     if is_endpoint {
    //         draw_circle_lines(pt.x, pt.y, r, 1.2 * scale, Color::from_rgba(255, 255, 255, 150));
    //     }
    // }

    // ── Head star ──
    if show_full {
        // Static star at start position
        let head_pt = path[0];
        let ss = STAR_SIZE * scale;
        let star_used = tex.star.or(tex.star_fallback);
        if let Some(st) = star_used {
            draw_texture_ex(
                st,
                head_pt.x - ss * 0.5,
                head_pt.y - ss * 0.5,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(ss, ss)),
                    ..Default::default()
                },
            );
        }
    } else if dt_scaled > 0.0 && dt_scaled < head_lead && !note.is_tapless {
        // Pre-judge flying-in head star (A-zone and touch-zone). Same radial
        // flight as a Tap note.
        let head_motion = super::types::note_radial_motion(
            dt_scaled,
            head_speed,
            outer_r,
            TAP_TARGET_OFFSET,
        );
        let size_scale = head_motion.map(|m| m.scale).unwrap_or(0.0);
        let fly_progress = head_motion.map(|m| m.progress).unwrap_or(0.0);
        let lock_r = super::types::note_lock_radius(outer_r, TAP_TARGET_OFFSET);

        if note.lane <= 8 {
            // A-zone: grow at the inner lock radius, then fly to the target.
            let idx = (note.lane - 1) as f32;
            let ang =
                -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
            let r = head_motion
                .map(|m| m.radius)
                .unwrap_or(lock_r);
            let px = spawn_cx.x + ang.cos() * r;
            let py = spawn_cx.y + ang.sin() * r;

            let ss = STAR_SIZE * scale * size_scale;
            let star_rot = fly_progress * std::f32::consts::TAU;
            let star_used = tex.star.or(tex.star_fallback);
            if let Some(st) = star_used {
                draw_texture_ex(
                    st,
                    px - ss * 0.5,
                    py - ss * 0.5,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(ss, ss)),
                        rotation: star_rot,
                        ..Default::default()
                    },
                );
                if let Some(ex_tex) = tex.star_ex.or(tex.star_ex_fallback) {
                    draw_texture_ex(
                        ex_tex,
                        px - ss * 0.5,
                        py - ss * 0.5,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(ss, ss)),
                            rotation: star_rot,
                            ..Default::default()
                        },
                    );
                }
            }
        } else {
            // Touch zone: fade in at centroid
            let head_rot = fly_progress * std::f32::consts::TAU;
            let ss = STAR_SIZE * scale * size_scale;
            let star_used = tex.star.or(tex.star_fallback);
            if let Some(st) = star_used {
                draw_texture_ex(
                    st,
                    path[0].x - ss * 0.5,
                    path[0].y - ss * 0.5,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(ss, ss)),
                        rotation: head_rot,
                        ..Default::default()
                    },
                );
                if let Some(ex_tex) = tex.star_ex.or(tex.star_ex_fallback) {
                    draw_texture_ex(
                        ex_tex,
                        path[0].x - ss * 0.5,
                        path[0].y - ss * 0.5,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(ss, ss)),
                            rotation: head_rot,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    // ── Flying star (post-judge, moves along path) ──
    if !show_full && current_t >= ns && current_t <= slide_end_s {
        let (star_pos, angle) = point_at(star_dist_along);
        let ss = STAR_SIZE * scale;
        let star_used = tex.star.or(tex.star_fallback);
        if let Some(st) = star_used {
            draw_texture_ex(
                st,
                star_pos.x - ss * 0.5,
                star_pos.y - ss * 0.5,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(ss, ss)),
                    rotation: angle,
                    ..Default::default()
                },
            );
            if let Some(ex_tex) = tex.star_ex.or(tex.star_ex_fallback) {
                draw_texture_ex(
                    ex_tex,
                    star_pos.x - ss * 0.5,
                    star_pos.y - ss * 0.5,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(ss, ss)),
                        rotation: angle,
                        ..Default::default()
                    },
                );
            }
        }
    }
}
