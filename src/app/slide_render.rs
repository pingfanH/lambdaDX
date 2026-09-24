use super::pad_svg::PadSvgDef;
use super::slide::segmentation;
use super::types::{Note, NOTE_LOCK_DISTANCE, NOTE_OUTER_DISTANCE, PAD_ROTATION_RAD, PadGeom, SLIDE_MIN_DURATION_S, Slide, SlideShape};
use crate::app::slide::path::{
    slide_shape_caret, slide_shape_left, slide_shape_line, slide_shape_p, slide_shape_pp,
    slide_shape_q, slide_shape_qq, slide_shape_right, slide_shape_s, slide_shape_z,
};
use crate::app::types::zone::PadZone;
use macroquad::prelude::*;
use macroquad::texture::{DrawTextureParams, Texture2D};
use crate::app::params;

/// Resolved textures for a single draw_slide call.
/// The caller picks the appropriate variant; the function just uses what's given.
pub struct SlideTextures<'a> {
    pub trail: Option<&'a Texture2D>,
    pub star: Option<&'a Texture2D>,
    pub star_fallback: Option<&'a Texture2D>,
    pub star_ex: Option<&'a Texture2D>,
    pub star_ex_fallback: Option<&'a Texture2D>,
    pub wifi: [Option<&'a Texture2D>; 11],
    /// Optional guide texture drawn under the stars.
    pub guide: Option<&'a Texture2D>,
}

/// Draw a filled polygon band over the slide's **last touch judge segment** as
/// the judgment background.
///
/// The slide path is sampled into trail bars and merged into per-sensor-area
/// judge segments (see [`segmentation`]). The band covers only the final such
/// block — the last piece the star can be touched/slid through — instead of the
/// whole path or shape segment. The band follows the sampled bars, so its
/// direction always matches the slide. `fill` paints the wide background band
/// and `core` paints a brighter center stripe. Returns the block direction in
/// radians and its end point so callers can align the judgment text. Wifi
/// slides are skipped because their three tracks are rendered separately.
pub fn draw_slide_judge_band(
    note: &Note,
    slide: &Slide,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
    band_width: f32,
    fill: Color,
    core: Color,
) -> Option<(f32, Vec2)> {
    if slide
        .segments
        .iter()
        .any(|s| matches!(s.shape, SlideShape::Wifi))
    {
        return None;
    }
    let path = build_slide_path(note, slide, pad, svg, scale, spawn_cx, outer_r);
    if path.len() < 2 {
        return None;
    }
    let spacing = params::slide_tile_spacing() * scale;
    let segmentation = segmentation::build(
        &path,
        spacing,
        params::slide_head_gap() * scale,
        params::slide_tail_gap() * scale,
        svg,
        pad,
    );
    let last_segment = segmentation.judge_segments.last()?;
    let start = last_segment.start_bar;
    let end = last_segment.end_bar.min(segmentation.bars.len());
    if start >= end {
        return None;
    }

    let mut block: Vec<Vec2> = segmentation.bars[start..end]
        .iter()
        .map(|bar| bar.position)
        .collect();
    if block.is_empty() {
        return None;
    }
    if block.len() == 1 {
        // A one-bar block still needs two points to form a band; extend along
        // the incoming direction of the path.
        let hint = if start > 0 {
            block[0] - segmentation.bars[start - 1].position
        } else {
            Vec2::ZERO
        };
        let dir = if hint.length_squared() > 1e-6 {
            hint.normalize()
        } else {
            vec2(1.0, 0.0)
        };
        let half = band_width.max(1.0) * 0.5;
        block = vec![block[0] - dir * half, block[0] + dir * half];
    }

    draw_polyline_band(&block, band_width, fill);
    draw_polyline_band(&block, band_width * 0.42, core);

    let first = block[0];
    let last = *block.last().unwrap();
    let dir = last - first;
    (dir.length_squared() > 1e-6).then(|| (dir.y.atan2(dir.x), last))
}

/// Fill a custom polygon band of `width` around a polyline by emitting a quad
/// per segment (two triangles) plus round joints at each vertex.
fn draw_polyline_band(path: &[Vec2], width: f32, color: Color) {
    let hw = (width * 0.5).max(0.5);
    for w in path.windows(2) {
        let delta = w[1] - w[0];
        let len = delta.length();
        if len < 1e-3 {
            continue;
        }
        let dir = delta / len;
        let normal = vec2(-dir.y, dir.x) * hw;
        let a = w[0] + normal;
        let b = w[0] - normal;
        let c = w[1] + normal;
        let d = w[1] - normal;
        draw_triangle(a, b, c, color);
        draw_triangle(b, d, c, color);
    }
    // Round the corners so the band has no notches where segments meet.
    for p in path {
        draw_circle(p.x, p.y, hw, color);
    }
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
    if let Some(start) = slide_start_point(note, svg, pad, spawn_cx, outer_r) {
        path.push(start);
    }
    append_segments(&mut path, note, slide, outer_r, spawn_cx, pad, svg, scale);
    path
}

/// Screen-space start point of a slide path: the outer tap ring for A zones,
/// or the zone centroid for screen zones.
fn slide_start_point(
    note: &Note,
    svg: &PadSvgDef,
    pad: &PadGeom,
    spawn_cx: Vec2,
    outer_r: f32,
) -> Option<Vec2> {
    if note.lane <= 8 {
        let idx = (note.lane - 1) as f32;
        let angle =
            -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
        let radius = outer_r + params::tap_target_offset();
        Some(spawn_cx + vec2(angle.cos(), angle.sin()) * radius)
    } else {
        svg.zone_screen_centroid(PadZone::from(note.lane), pad)
    }
}

fn append_segments(
    path: &mut Vec<Vec2>,
    note: &Note,
    slide: &Slide,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    let mut current = note.clone();
    for segment in &slide.segments {
        append_segment(
            path, &mut current, segment, outer_r, spawn_cx, pad, svg, scale,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_segment(
    path: &mut Vec<Vec2>,
    current: &mut Note,
    segment: &crate::app::types::SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    match segment.shape {
        SlideShape::Q => slide_shape_q(path, current, segment, outer_r, spawn_cx, pad, svg, scale),
        SlideShape::QQ => slide_shape_qq(path, current, segment, outer_r, spawn_cx, pad, svg, scale),
        SlideShape::P => slide_shape_p(path, current, segment, outer_r, spawn_cx, pad, svg, scale),
        SlideShape::PP => {
            slide_shape_pp(path, current, segment, outer_r, spawn_cx, pad, svg, scale)
        }
        SlideShape::Left => {
            slide_shape_left(path, current, segment, outer_r, spawn_cx, pad, svg, scale)
        }
        SlideShape::Right => {
            slide_shape_right(path, current, segment, outer_r, spawn_cx, pad, svg, scale)
        }
        SlideShape::Caret => {
            slide_shape_caret(path, current, segment, outer_r, spawn_cx, pad, svg, scale)
        }
        SlideShape::Z => slide_shape_z(path, current, segment, outer_r, spawn_cx, pad, svg, scale),
        SlideShape::S => slide_shape_s(path, current, segment, outer_r, spawn_cx, pad, svg, scale),
        SlideShape::Wifi => {}
        SlideShape::Line | SlideShape::VShape | SlideShape::BigV => {
            slide_shape_line(path, current, segment, outer_r, spawn_cx, pad, svg, scale)
        }
    }
    if let Some(last) = segment.points.last() {
        current.lane = last.zone.to_id();
    }
}

/// Which half of a slide to draw. Trails and stars are separate layers so the
/// caller can draw *every* slide trail before *any* slide star — otherwise an
/// overlapping slide's trail hides another slide's star (e.g. a QQ and a PP
/// playing at the same time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideLayer {
    Trail,
    Star,
}

/// Draw one layer of a single slide on the pad surface.
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
/// `hidden_until_bar` — hide trail bars with indexes lower than this value
/// `layer` — draw the trail tiles or the star(s)
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
    base_speed: f32,
    slide_fade_in: f32,
    hidden_until_bar: usize,
    layer: SlideLayer,
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
    let head_speed = super::types::note_flight_speed(note, base_speed);
    let head_lead = super::types::note_lead_time(head_speed);
    // Head-star spin, continuous from the moment the star spawns. The radial
    // `progress` stays 0 while the star grows at the lock radius, so a
    // progress-based spin only started after the fly-out; this makes it rotate
    // from birth while keeping one full turn per fly-out duration.
    let head_flight_s = (super::types::NOTE_OUTER_DISTANCE - super::types::NOTE_LOCK_DISTANCE)
        / head_speed.max(0.1);
    let head_spin = ((head_lead - dt_scaled).max(0.0) / head_flight_s.max(0.12))
        * std::f32::consts::TAU;
    // MajdataView trail fade-in: `fadeInTime = -3.926913 / noteSpeed` seconds
    // before the head, fully visible 0.2s later (in musical time).
    let fade_in_s = slide_fade_in.max(0.0);
    let full_fade_s = (fade_in_s - fade_duration_s).max(0.001);

    // Draw a star's guide texture (same rules as tap guides) if one is loaded.
    // `star_px` is the star's *current* on-screen size, so the guide scales
    // exactly with the note; `progress` adds the displacement scaling.
    let star_guide = |x: f32, y: f32, rot: f32, progress: f32, star_px: f32| {
        if let Some(g) = tex.guide {
            super::guide::draw(g, tex.star, star_px, x, y, rot, progress, scale, 1.0);
        }
    };

    // ── Time culling (skip when not show_full) ──
    if !show_full {
        if !(dt_scaled <= head_lead.max(fade_in_s) && current_t <= slide_end_s + 0.2) {
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
        let target_r = outer_r + params::tap_target_offset();
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
                    let target_r = outer_r + params::tap_target_offset();
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
                        let target_r = outer_r + params::tap_target_offset();
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
                        let target_r = outer_r + params::tap_target_offset();
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
                        let target_r = outer_r + params::tap_target_offset();
                        vec2(
                            spawn_cx.x + ang.cos() * target_r,
                            spawn_cx.y + ang.sin() * target_r,
                        )
                    },
                ];

                // ── Tile alpha ──
                let a_max = params::slide_trail_alpha();
                let path_alpha = if show_full || dt_scaled <= full_fade_s {
                    a_max as u8
                } else {
                    ((a_max * (fade_in_s - dt_scaled) / fade_duration_s).clamp(0.0, a_max)) as u8
                };

                // ── Flying star progress (0..1) ──
                let star_t = if !show_full && current_t >= slide_start_s {
                    ((current_t - slide_start_s) / travel_dur_s.max(0.001)).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let sprite_count = 11;
                let command_hidden_until = hidden_until_bar.min(sprite_count);
                // Guide orientation = the lane's flight direction (constant), so
                // the guide does NOT spin with the star.
                let guide_ang = -std::f32::consts::FRAC_PI_2
                    + PAD_ROTATION_RAD
                    + (note.lane.saturating_sub(1)) as f32 * std::f32::consts::TAU / 8.0;

                if layer == SlideLayer::Trail {
                for (j, target) in targets.iter().enumerate() {
                    let dir = (*target - start_pos).normalize_or_zero();
                    let seg_len = (*target - start_pos).length().max(0.001);
                    let angle = dir.y.atan2(dir.x) + std::f32::consts::PI + 112.0_f32.to_radians();
                    let step_size = seg_len / (sprite_count - 1) as f32 * 0.83;
                    // Wifi has three independent straight tracks, so its
                    // bars do not go through the shared path segmentation.

                    let is_middle = j == 1;

                    // ── Tiles (only middle line gets wifi textures) ──
                    for i in 0..sprite_count {
                        if i < command_hidden_until {
                            continue;
                        }
                        let dist = i as f32 * step_size;
                        let sprite_pos = start_pos + dir * dist;

                        if is_middle {
                            if let Some(t) = tex.wifi[i] {
                                let tw = t.width() * scale * params::slide_tile_scale();
                                let th = t.height() * scale * params::slide_tile_scale();
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
                }

                // Star guides (trail layer): head + one per track, all under
                // the stars.
                if tex.guide.is_some() {
                    if !show_full && current_t < ns && !note.is_tapless {
                        let head_motion = super::types::note_radial_motion_continue(
                            dt_scaled,
                            head_speed,
                            outer_r,
                            params::tap_target_offset(),
                        );
                        let size_scale = head_motion.map(|m| m.scale).unwrap_or(0.0);
                        let lock_r = super::types::note_lock_radius(outer_r, params::tap_target_offset());
                        let r = head_motion.map(|m| m.radius).unwrap_or(lock_r);
                        let px = spawn_cx.x + guide_ang.cos() * r;
                        let py = spawn_cx.y + guide_ang.sin() * r;
                        star_guide(
                            px,
                            py,
                            guide_ang,
                            head_motion.map(|m| m.progress).unwrap_or(0.0),
                            params::star_size() * scale * size_scale,
                        );
                    }
                }
                }

                if layer == SlideLayer::Star {
                // ── Head star (pre-judge flying in from center), drawn after
                // the trails so it stays on top. ──
                if show_full {
                    let head_pt = path[0];
                    let ss = params::star_size() * scale;
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
                    let head_motion = super::types::note_radial_motion_continue(
                        dt_scaled,
                        head_speed,
                        outer_r,
                        params::tap_target_offset(),
                    );
                    let size_scale = head_motion.map(|m| m.scale).unwrap_or(0.0);

                    let idx = (note.lane - 1) as f32;
                    let ang = -std::f32::consts::FRAC_PI_2
                        + PAD_ROTATION_RAD
                        + idx * std::f32::consts::TAU / 8.0;
                    let lock_r = super::types::note_lock_radius(outer_r, params::tap_target_offset());
                    let r = head_motion.map(|m| m.radius).unwrap_or(lock_r);
                    let px = spawn_cx.x + ang.cos() * r;
                    let py = spawn_cx.y + ang.sin() * r;
                    let ss = params::star_size() * scale * size_scale;
                    let star_rot = head_spin;
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

                // ── Flying stars, drawn after *all* trail tiles so no wifi
                // track's trail can cover another track's star. ──
                if !show_full && current_t >= ns && current_t <= slide_end_s {
                    let intro = if current_t < slide_start_s {
                        ((current_t - ns) / (slide_start_s - ns).max(0.001)).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let ss = params::star_size() * scale * (0.5 + intro);
                    let star_alpha = (intro * 255.0) as u8;
                    let star_used = tex.star.or(tex.star_fallback);
                    for target in targets.iter() {
                        let dir = (*target - start_pos).normalize_or_zero();
                        let seg_len = (*target - start_pos).length().max(0.001);
                        let angle =
                            dir.y.atan2(dir.x) + std::f32::consts::PI + 112.0_f32.to_radians();
                        let star_pos = start_pos + dir * (star_t * seg_len);
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
        (params::slide_trail_alpha() as u8, -1.0_f32) // all tiles visible, star at start
    } else {
        let a_max = params::slide_trail_alpha();
        let alpha = if dt_scaled <= full_fade_s {
            a_max as u8
        } else {
            ((a_max * (fade_in_s - dt_scaled) / fade_duration_s).clamp(0.0, a_max)) as u8
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
            t.width() * scale * params::slide_tile_scale(),
            t.height() * scale * params::slide_tile_scale(),
        )
    } else {
        (params::slide_tile_size() * scale, params::slide_tile_size() * scale)
    };
    let spacing = params::slide_tile_spacing() * scale;

    let segmentation = segmentation::build(
        &path,
        spacing,
        params::slide_head_gap() * scale,
        params::slide_tail_gap() * scale,
        svg,
        pad,
    );
    let hidden_until = hidden_until_bar.min(segmentation.bars.len());
    // Within one slide, the trail tiles can be drawn forward or reversed so an
    // overlapping tile's stacking can be chosen.
    let bar_order: Box<dyn Iterator<Item = usize>> = if params::slide_tile_reverse() {
        Box::new((0..segmentation.bars.len()).rev())
    } else {
        Box::new(0..segmentation.bars.len())
    };
    if layer == SlideLayer::Trail {
    for bar_index in bar_order {
        if bar_index < hidden_until {
            continue;
        }
        let bar = &segmentation.bars[bar_index];
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

    // Star guides, drawn in the trail layer so **every** guide is under
    // **every** star (a later sub-slide's guide can no longer cover an earlier
    // sub-slide's star). Orientation is the constant flight direction.
    if tex.guide.is_some() {
        let guide_dir = path[0] - spawn_cx;
        let guide_ang = guide_dir.y.atan2(guide_dir.x);
        if !show_full && dt_scaled > 0.0 && dt_scaled < head_lead && !note.is_tapless {
            let head_motion = super::types::note_radial_motion_continue(
                dt_scaled,
                head_speed,
                outer_r,
                params::tap_target_offset(),
            );
            let size_scale = head_motion.map(|m| m.scale).unwrap_or(0.0);
            let lock_r = super::types::note_lock_radius(outer_r, params::tap_target_offset());
            let r = head_motion.map(|m| m.radius).unwrap_or(lock_r);
            let (hx, hy) = if note.lane <= 8 {
                let idx = (note.lane - 1) as f32;
                let a = -std::f32::consts::FRAC_PI_2
                    + PAD_ROTATION_RAD
                    + idx * std::f32::consts::TAU / 8.0;
                (spawn_cx.x + a.cos() * r, spawn_cx.y + a.sin() * r)
            } else {
                (path[0].x, path[0].y)
            };
            star_guide(
                hx,
                hy,
                guide_ang,
                head_motion.map(|m| m.progress).unwrap_or(0.0),
                params::star_size() * scale * size_scale,
            );
        }
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
    if layer == SlideLayer::Star {
    if show_full {
        // Static star at start position
        let head_pt = path[0];
        let ss = params::star_size() * scale;
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
        let head_motion = super::types::note_radial_motion_continue(
            dt_scaled,
            head_speed,
            outer_r,
            params::tap_target_offset(),
        );
        let size_scale = head_motion.map(|m| m.scale).unwrap_or(0.0);
        let lock_r = super::types::note_lock_radius(outer_r, params::tap_target_offset());

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

            let ss = params::star_size() * scale * size_scale;
            let star_rot = head_spin;
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
            let head_rot = head_spin;
            let ss = params::star_size() * scale * size_scale;
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
    }

    // ── Flying star (post-judge, moves along path) ──
    //
    // The approaching head star vanishes at the hit; this tracing star then
    // pops in *at the hit position*: it starts at the original star-head size
    // and ~50% opacity, and over the slide's pre-trace wait (`start_delay_s`,
    // i.e. until it begins to trace) it grows to 1.5x and fades to fully
    // opaque. Then it continues along the path.
    if layer == SlideLayer::Star {
    if !show_full && current_t >= ns && current_t <= slide_end_s {
        let (star_pos, angle) = point_at(star_dist_along);
        let p = if start_delay_s > 1e-4 {
            ((current_t - ns) / start_delay_s).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let ss = params::star_size() * scale * (1.0 + params::star_spawn_scale_gain() * p);
        let a0 = params::star_spawn_alpha_start();
        let tint = Color::from_rgba(255, 255, 255, ((a0 + (1.0 - a0) * p) * 255.0) as u8);
        let star_used = tex.star.or(tex.star_fallback);
        if let Some(st) = star_used {
            draw_texture_ex(
                st,
                star_pos.x - ss * 0.5,
                star_pos.y - ss * 0.5,
                tint,
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
                    tint,
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
