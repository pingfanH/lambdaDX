use super::pad_svg::PadSvgDef;
use super::slide::segmentation;
use super::slide_svg;
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

/// Place a sub-slide's `slideok` overlay from the baked prefab pose.
///
/// Returns `(centre, rotation, width, height)` in screen space (before the
/// caller's per-variant `adj.rot`/flips), or `None` when the slide has no
/// reference prefab (wifi and off-ring locations fall back to the procedural
/// placement).
fn prefab_ok(
    note: &Note,
    slide: &Slide,
    spawn_cx: Vec2,
    outer_r: f32,
    adj: crate::app::params::SlideJustParams,
) -> Option<(Vec2, f32, f32, f32)> {
    let seg = slide.segments.last()?;
    if matches!(seg.shape, SlideShape::Wifi) {
        return None;
    }
    let starts = segment_start_lanes(note, slide);
    let start = *starts.last()?;
    if !(1..=8).contains(&start) {
        return None;
    }
    let end = seg.points.last().map(|p| p.zone.to_id()).unwrap_or(start);
    let turn = seg.points.first().map(|p| p.zone.to_id()).unwrap_or(0);
    let key = slide_svg::prefab_key(seg.shape, start, end, turn)?;
    let def = slide_svg::defs().get(&key.name)?;
    // Reference `SlideDrop.LoadSkin`: the one baked straight pose is flipped
    // when its side does not match the mirror state. Curv poses never flip.
    let fix_up = match seg.shape {
        SlideShape::Caret | SlideShape::Left | SlideShape::Right => false,
        _ => slide_svg::is_just_right(seg.shape, start, end, turn) == key.mirror,
    };
    let unit = prefab_unit(outer_r);
    let ok = slide_svg::ok_screen(def, start, key.mirror, spawn_cx, unit, fix_up)?;
    let offset = vec2(adj.off_x, adj.off_y) * params::pad_scale();
    let s = adj.scale.max(0.01);
    Some((
        ok.pos + offset,
        ok.rot,
        ok.size.x * unit * s,
        ok.size.y * unit * s,
    ))
}

/// Draw the MajdataView `slideok` judgment overlay for one sub-slide.
///
/// Registration differs by shape:
/// * normal slides — the sprite's edge sits on the end of the curve and trails
///   back along the tail direction;
/// * wifi — the sprite's **up axis** follows the middle track's tail direction
///   (start lane → opposite lane), with its top edge attached to the tail.
///
/// `adj` (per shape family) scales the sprite, nudges it in screen px and can
/// mirror it on either axis.
#[allow(clippy::too_many_arguments)]
pub fn draw_slide_just(
    note: &Note,
    slide: &Slide,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
    spawn_cx: Vec2,
    outer_r: f32,
    tex: &Texture2D,
    tint: Color,
    adj: crate::app::params::SlideJustParams,
) {
    // Baked prefab pose (reference registration) — non-wifi only for now.
    if let Some((center, rotation, w, h)) = prefab_ok(note, slide, spawn_cx, outer_r, adj) {
        draw_texture_ex(
            tex,
            center.x - w * 0.5,
            center.y - h * 0.5,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                rotation: rotation + adj.rot,
                flip_x: adj.flip_x,
                flip_y: adj.flip_y,
                pivot: Some(center),
                ..Default::default()
            },
        );
        return;
    }
    let aspect = tex.height() / tex.width().max(1.0);
    let size = adj.scale.max(0.01);
    let offset = vec2(adj.off_x, adj.off_y) * params::pad_scale();

    let (w, h, center, rotation) = if slide
        .segments
        .iter()
        .any(|s| matches!(s.shape, SlideShape::Wifi))
    {
        // A-ring point of a lane (same geometry as the wifi renderer).
        let ring = |lane: u8| -> Vec2 {
            let idx = lane.saturating_sub(1) as f32;
            let ang = -std::f32::consts::FRAC_PI_2
                + PAD_ROTATION_RAD
                + idx * std::f32::consts::TAU / 8.0;
            let r = outer_r + params::tap_target_offset();
            spawn_cx + vec2(ang.cos(), ang.sin()) * r
        };
        let lane = note.lane;
        let start = ring(lane);
        let mid_lane = ((lane as i32 + 4 - 1).rem_euclid(8) + 1) as u8;
        let mid = ring(mid_lane);
        let dir = (mid - start).normalize_or_zero();
        if dir == Vec2::ZERO {
            return;
        }
        let w = (2.0 * outer_r).max(1.0) * size;
        let h = (w * aspect).max(1.0);
        // Up axis (local -y) follows the middle track; top edge attached to the
        // tail (opposite lane), body hanging back toward the start lane.
        let rotation = dir.y.atan2(dir.x) + std::f32::consts::FRAC_PI_2;
        (w, h, mid - dir * (h * 0.5) + offset, rotation)
    } else {
        // Normal slide: align the sprite's axis to the path **chord** (which,
        // for a circular arc, is parallel to the tangent at the arc midpoint, so
        // the curved sprite matches the curve), with its edge on the curve end.
        let path = build_slide_path(note, slide, pad, svg, scale, spawn_cx, outer_r);
        if path.len() < 2 {
            return;
        }
        let first = path[0];
        let tip = *path.last().unwrap();
        let chord = tip - first;
        let dir = if chord.length_squared() > 1e-6 {
            chord.normalize()
        } else {
            (tip - path[path.len() - 2]).normalize_or_zero()
        };
        if dir == Vec2::ZERO {
            return;
        }
        let w = path
            .windows(2)
            .map(|seg| (seg[1] - seg[0]).length())
            .sum::<f32>()
            .max(1.0)
            * size;
        let h = (w * aspect).max(1.0);
        // Only the left turn (`<`, shapely `Left`) is 180° out; `>` is not.
        let left = slide
            .segments
            .last()
            .is_some_and(|s| matches!(s.shape, SlideShape::Left));
        let rotation = dir.y.atan2(dir.x)
            + if left {
                std::f32::consts::PI
            } else {
                0.0
            };
        // Sit **inside** the curve: put the sprite's outer edge on the alignment
        // line and the body toward the pad centre.
        let line = tip - dir * (w * 0.5);
        let normal = vec2(-dir.y, dir.x);
        let inward = if normal.dot(spawn_cx - line) >= 0.0 {
            normal
        } else {
            -normal
        };
        (w, h, line + inward * (h * 0.5) + offset, rotation)
    };

    draw_texture_ex(
        tex,
        center.x - w * 0.5,
        center.y - h * 0.5,
        tint,
        DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            rotation: rotation + adj.rot,
            flip_x: adj.flip_x,
            flip_y: adj.flip_y,
            pivot: Some(center),
            ..Default::default()
        },
    );
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
    // Prefer the baked MajdataPlay prefab curve; fall back to the procedural
    // geometry for chains/paths the reference has no prefab for.
    if let Some(path) = prefab_slide_path(note, slide, spawn_cx, outer_r) {
        return path;
    }
    let mut path = Vec::new();
    if let Some(start) = slide_start_point(note, svg, pad, spawn_cx, outer_r) {
        path.push(start);
    }
    append_segments(&mut path, note, slide, outer_r, spawn_cx, pad, svg, scale);
    path
}

/// Screen length of one prefab unit: the on-screen A-ring radius over 4.8.
pub fn prefab_unit(outer_r: f32) -> f32 {
    (outer_r + params::tap_target_offset()) / slide_svg::PREFAB_UNIT
}

/// The start button of each segment (the note's lane, then the previous end).
pub fn segment_start_lanes(note: &Note, slide: &Slide) -> Vec<u8> {
    let mut starts = Vec::with_capacity(slide.segments.len());
    let mut start = note.lane;
    for seg in &slide.segments {
        starts.push(start);
        if let Some(last) = seg.points.last() {
            start = last.zone.to_id();
        }
    }
    starts
}

/// The reference prefab key for a segment (shape + start + end + turn).
pub fn segment_key(seg: &crate::app::types::SlideSegment, start: u8) -> Option<slide_svg::PrefabKey> {
    let end = seg.points.last().map(|p| p.zone.to_id()).unwrap_or(start);
    let turn = seg.points.first().map(|p| p.zone.to_id()).unwrap_or(0);
    slide_svg::prefab_key(seg.shape, start, end, turn)
}

/// Build the slide curve from the baked prefab polylines (one per segment).
///
/// Returns `None` when any segment has no prefab (wifi, off-ring zones, or an
/// out-of-range geometry), so the caller can fall back to procedural drawing.
pub fn prefab_slide_path(
    note: &Note,
    slide: &Slide,
    spawn_cx: Vec2,
    outer_r: f32,
) -> Option<Vec<Vec2>> {
    prefab_slide_path_bounded(note, slide, spawn_cx, outer_r).map(|(p, _)| p)
}

/// Like [`prefab_slide_path`] but also returns the per-segment path indices
/// (same meaning as the procedural builder's `seg_boundaries`).
pub fn prefab_slide_path_bounded(
    note: &Note,
    slide: &Slide,
    spawn_cx: Vec2,
    outer_r: f32,
) -> Option<(Vec<Vec2>, Vec<usize>)> {
    // Wifi keeps its dedicated three-track renderer.
    if slide
        .segments
        .iter()
        .any(|s| matches!(s.shape, SlideShape::Wifi))
    {
        return None;
    }
    // Chains: the prefab type is derived per arc, but the reference's
    // upper/lower-half mirror rule mis-maps arcs that cross the half (e.g.
    // `3>4` → a 304° mirrored `circle8`). The procedural builder already joins
    // chained arcs smoothly, so only single-segment slides use the prefab
    // geometry here.
    if slide.segments.len() > 1 {
        return None;
    }
    let unit = prefab_unit(outer_r);
    let defs = slide_svg::defs();
    let starts = segment_start_lanes(note, slide);
    let mut out: Vec<Vec2> = Vec::new();
    let mut bounds: Vec<usize> = Vec::with_capacity(slide.segments.len() + 1);
    let mut push = |out: &mut Vec<Vec2>, p: Vec2| {
        if out
            .last()
            .map(|q: &Vec2| (*q - p).length_squared() > 1e-6)
            .unwrap_or(true)
        {
            out.push(p);
        }
    };
    // The prefab's tile row is inset from its start/end buttons, so a chained
    // slide (`1>2>3`) would leave an ~11° straight gap at each junction. Mirror
    // the reference (`StarPositions = [ring(start), bars…, ring(end)]`) by
    // pinning the exact A-ring point at *every* segment's start and end; the
    // shared junction point then bridges consecutive arcs smoothly.
    for (seg, &start) in slide.segments.iter().zip(&starts) {
        if !(1..=8).contains(&start) {
            return None;
        }
        let key = segment_key(seg, start)?;
        let def = defs.get(&key.name)?;
        if let Some(p) = ring_point(start, spawn_cx, outer_r) {
            push(&mut out, p);
        }
        bounds.push(out.len());
        let placed = slide_svg::place(def, start, key.mirror, spawn_cx, unit);
        for p in placed.points {
            push(&mut out, p);
        }
        if let Some(end) = seg.points.last().and_then(|p| ring_point(p.zone.to_id(), spawn_cx, outer_r)) {
            push(&mut out, end);
        }
    }
    bounds.push(out.len());
    (out.len() >= 2).then_some((out, bounds))
}

/// Screen point of an A-ring button's tap target (`outer_r + tap offset`).
fn ring_point(lane: u8, spawn_cx: Vec2, outer_r: f32) -> Option<Vec2> {
    if !(1..=8).contains(&lane) {
        return None;
    }
    let idx = (lane - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let r = outer_r + params::tap_target_offset();
    Some(spawn_cx + vec2(ang.cos(), ang.sin()) * r)
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
/// `core_driven` — true when `lnmai-core` owns the slide's lifetime, so the
///                 post-tail time cull is disabled and the slide is only
///                 removed by core's hide commands
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
    seg_frac: &[f32],
    // Wifi per-track explicit trail-bar cutoffs (`HideSlideTrackBars`); takes
    // precedence over the fraction-derived cutoff.
    track_bars: [Option<usize>; 3],
    core_driven: bool,
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
        // Approach culling is always local: don't draw before the head flies in.
        let before_head = dt_scaled <= head_lead.max(fade_in_s);
        // Post-tail culling only applies when no engine owns the slide. When
        // `core_driven`, the slide stays until lnmai-core hides its bars.
        let after_end_cull = !core_driven && current_t > slide_end_s + 0.2;
        if !before_head || after_end_cull {
            return;
        }
    }

    // ── 构建路径：优先使用烘焙的 prefab 折线，否则程序化生成。──
    let is_wifi = slide
        .segments
        .iter()
        .any(|s| matches!(s.shape, SlideShape::Wifi));
    let prefab_built = if is_wifi {
        None
    } else {
        prefab_slide_path_bounded(note, slide, spawn_cx, outer_r)
    };
    let (path, seg_boundaries): (Vec<Vec2>, Vec<usize>) = if let Some(built) = prefab_built {
        built
    } else {
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
    // Path index at the start of each segment (plus one past the end), used to
    // convert per-segment core progress into trail-bar ranges.
    let mut seg_boundaries: Vec<usize> = Vec::with_capacity(slide.segments.len() + 1);
    for seg in &slide.segments {
        seg_boundaries.push(path.len());
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
                // lnmai-core's wifi cutoff is in arrow units (0..=7), while
                // the renderer's visual cutoff is assigned by judge area like
                // an ordinary slide. Convert arrows to completed areas first.
                const WIFI_ARROWS: f32 = 8.0;
                // Cumulative consumed fraction across the sub-slide's segments.
                let overall_frac = if seg_frac.is_empty() {
                    0.0
                } else {
                    seg_frac.iter().sum::<f32>() / seg_frac.len() as f32
                };
                // Each wifi track consumes on its own: sample that track and
                // split its sprites across the sensor areas it crosses
                // (first/last fixed, middle even), matching straight slides.
                let track_cutoffs: [usize; 3] = std::array::from_fn(|j| {
                    // Core's explicit per-track bar cutoff wins (pure's signal);
                    // otherwise derive one from the track's consumed fraction.
                    if let Some(bar) = track_bars[j] {
                        let track_path = [start_pos, targets[j]];
                        let track_seg = segmentation::build(
                            &track_path,
                            params::slide_tile_spacing() * scale,
                            params::slide_head_gap() * scale,
                            params::slide_tail_gap() * scale,
                            svg,
                            pad,
                        );
                        let areas = track_seg.judge_segments.len().max(1);
                        let arrow_frac = ((bar as f32 + 1.0) / WIFI_ARROWS).clamp(0.0, 1.0);
                        let passed = (arrow_frac * areas as f32).ceil() as usize;
                        return area_bar_boundary(sprite_count, areas, passed);
                    }
                    let frac = seg_frac.get(j).copied().unwrap_or(overall_frac);
                    if frac <= 0.0 {
                        return 0;
                    }
                    let track_path = [start_pos, targets[j]];
                    let track_seg = segmentation::build(
                        &track_path,
                        params::slide_tile_spacing() * scale,
                        params::slide_head_gap() * scale,
                        params::slide_tail_gap() * scale,
                        svg,
                        pad,
                    );
                    let areas = track_seg.judge_segments.len().max(1);
                    let passed = (frac.clamp(0.0, 1.0) * areas as f32).round() as usize;
                    area_bar_boundary(sprite_count, areas, passed)
                });
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
                        if i < track_cutoffs[j] {
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
                if !show_full
                    && current_t >= ns
                    && (core_driven || current_t <= slide_end_s)
                {
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
    seg_boundaries.push(path.len());
        (path, seg_boundaries)
    };

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
    // Core progress is per chart segment; map each segment's consumed fraction
    // onto its path-distance range to get the trail-bar frontier.
    let hidden_until = if core_driven && seg_frac.len() == 1 {
        // Straight-line (single segment) slides: the star crosses one sensor
        // area per judge step, so distribute the bars evenly across those areas
        // instead of by raw path distance (the first/last area get a fixed
        // share, the middle split the rest).
        let areas = segmentation.judge_segments.len().max(1);
        let passed = (seg_frac[0].clamp(0.0, 1.0) * areas as f32).round() as usize;
        area_bar_boundary(segmentation.bars.len(), areas, passed)
    } else if core_driven && !seg_frac.is_empty() && seg_boundaries.len() >= 2 {
        let mut cum: Vec<f32> = Vec::with_capacity(path.len());
        let mut acc = 0.0_f32;
        cum.push(0.0);
        for w in path.windows(2) {
            acc += (w[1] - w[0]).length();
            cum.push(acc);
        }
        let last = cum.len() - 1;
        let seg_count = seg_frac.len().min(seg_boundaries.len() - 1);
        // A boundary stores `path.len()` before the segment's first point is
        // appended, so map it to the previous point's distance (`-1`).
        let dist_at = |b: usize| cum[b.saturating_sub(1).min(last)];
        // Segments are consumed in order (state normalises earlier arcs to 1.0),
        // so the frontier is just the furthest segment with any progress: earlier
        // ones are fully hidden up to their end, later ones contribute nothing.
        let frontier = match seg_frac[..seg_count].iter().rposition(|f| *f > 0.0) {
            Some(j) => {
                let d0 = dist_at(seg_boundaries[j]);
                let d1 = dist_at(seg_boundaries[j + 1]);
                d0 + seg_frac[j].clamp(0.0, 1.0) * (d1 - d0)
            }
            None => 0.0,
        };
        segmentation
            .bars
            .iter()
            .rposition(|bar| bar.distance_along <= frontier)
            .map(|index| index + 1)
            .unwrap_or(0)
    } else {
        0
    };
    let hidden_until = hidden_until.min(segmentation.bars.len());
    if matches!(layer, SlideLayer::Trail) && crate::player::engine::debug_slide_enabled() {
        // Tag by note **and** segment: a continuous chain renders one trail per
        // arc, and a shared per-note tag made the dedup flap (print every frame).
        let seg_sig: String = slide
            .segments
            .iter()
            .map(|s| {
                format!(
                    "{:?}:{}",
                    s.shape,
                    s.points.last().map(|p| p.zone.to_id()).unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let key = format!("{hidden_until}|{seg_frac:?}");
        if crate::player::engine::debug_dedup(
            format!("player_render/{}/{seg_sig}", note.id),
            &key,
        ) {
            eprintln!(
                "[slide/player t={current_t:.3}] star note={} traveled={star_dist_along:.2}/{total_len:.2} \
                 bars={hidden_until}/{} seg_frac={:?} star_t={:.3} core_driven={core_driven}",
                note.id,
                segmentation.bars.len(),
                seg_frac,
                if total_len > 0.0 { star_dist_along / total_len } else { 0.0 },
            );
        }
    }
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
    if !show_full && current_t >= ns && (core_driven || current_t <= slide_end_s) {
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

/// Trail-bar boundary after `passed` of `areas` sensor areas have been crossed.
///
/// Used for single-segment (straight-line) slides: the first and last area hide
/// a fixed small share, the middle areas split the remaining bars evenly, so the
/// trail consumes in even per-area steps rather than by raw path distance.
fn area_bar_boundary(bars: usize, areas: usize, passed: usize) -> usize {
    if passed == 0 || areas == 0 || bars == 0 {
        return 0;
    }
    let first = 3.min(bars);
    let last = if areas > 1 {
        3.min(bars.saturating_sub(first))
    } else {
        0
    };
    let middle_total = bars.saturating_sub(first + last);
    let middle_n = areas.saturating_sub(2);
    let mut acc = 0usize;
    for i in 0..areas {
        let count = if i == 0 {
            first
        } else if i == areas - 1 {
            last
        } else if middle_n > 0 {
            middle_total / middle_n + usize::from((i - 1) < middle_total % middle_n)
        } else {
            0
        };
        acc += count;
        if i + 1 == passed {
            return acc.min(bars);
        }
    }
    acc.min(bars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::zone::PadZone;
    use crate::app::types::{SlidePoint, SlideSegment};

    fn line_slide(start: u8, end: u8) -> (Note, Slide) {
        let note = Note {
            lane: start,
            ..Default::default()
        };
        let slide = Slide {
            segments: vec![SlideSegment {
                points: vec![SlidePoint {
                    zone: PadZone::from(end),
                    beat_offset: 0.0,
                }],
                shape: SlideShape::Line,
            }],
            slide_duration: 1.0,
            slide_start_delay: 0.0,
            slide_is_break: false,
            runtime_parts: 1,
        };
        (note, slide)
    }

    /// `line3` (start 3 → end 5) resolves to the baked prefab polyline, rotated
    /// onto the start button, with bar centres on the A-ring.
    #[test]
    fn prefab_path_builds_line3_from_the_asset() {
        let (note, slide) = line_slide(3, 5);
        let spawn = vec2(0.0, 0.0);
        let outer_r = 480.0;
        let path = prefab_slide_path(&note, &slide, spawn, outer_r).expect("line3 prefab path");
        // The tap-ring start point plus the densified prefab polyline.
        assert!(path.len() > 20, "n={}", path.len());
        let r = path[0].length() / prefab_unit(outer_r);
        assert!((r - slide_svg::PREFAB_UNIT).abs() < 0.05, "start radius {r}");
        // Some later point is the A-ring tile (just inside the ring).
        let inner = path[3].length() / prefab_unit(outer_r);
        assert!(inner < slide_svg::PREFAB_UNIT, "first tile radius {inner}");
    }

    /// A wifi slide keeps its dedicated renderer: no prefab path.
    #[test]
    fn wifi_has_no_prefab_path() {        let note = Note {
            lane: 1,
            ..Default::default()
        };
        let slide = Slide {
            segments: vec![SlideSegment {
                points: vec![SlidePoint {
                    zone: PadZone::from(5),
                    beat_offset: 0.0,
                }],
                shape: SlideShape::Wifi,
            }],
            slide_duration: 1.0,
            slide_start_delay: 0.0,
            slide_is_break: false,
            runtime_parts: 1,
        };
        assert!(prefab_slide_path(&note, &slide, vec2(0.0, 0.0), 480.0).is_none());
    }

    /// A chained slide must still join smoothly. Chains fall back to the
    /// procedural builder, so this guards the whole `draw_slide` path really is
    /// smooth for a `1>2>3<4<5<6` zig-zag.
    #[test]
    fn chained_slide_junctions_are_smooth() {
        let note = Note {
            lane: 1,
            ..Default::default()
        };
        let chain = [
            (2u8, SlideShape::Right),
            (3, SlideShape::Right),
            (4, SlideShape::Left),
            (5, SlideShape::Left),
            (6, SlideShape::Left),
        ];
        let segments: Vec<SlideSegment> = chain
            .iter()
            .map(|&(e, sh)| SlideSegment {
                points: vec![SlidePoint {
                    zone: PadZone::from(e),
                    beat_offset: 0.0,
                }],
                shape: sh,
            })
            .collect();
        let slide = Slide {
            segments,
            slide_duration: 1.0,
            slide_start_delay: 0.0,
            slide_is_break: false,
            runtime_parts: 5,
        };
        let pad_svg = PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")).expect("pad svg");
        let pad = PadGeom {
            cx: 0.0,
            cy: 0.0,
            outer_r: 326.57,
        };
        let path = build_slide_path(&note, &slide, &pad, &pad_svg, 1.0, vec2(0.0, 0.0), 326.57);
        assert!(path.len() > 5);
        let mut worst = 0.0_f32;
        for i in 1..path.len() - 1 {
            let a = path[i] - path[i - 1];
            let b = path[i + 1] - path[i];
            if a.length() < 1e-6 || b.length() < 1e-6 {
                continue;
            }
            worst = worst.max(
                (a.dot(b) / (a.length() * b.length()))
                    .clamp(-1.0, 1.0)
                    .acos()
                    .to_degrees(),
            );
        }
        assert!(worst < 15.0, "junction corner {worst}deg");
    }
}
