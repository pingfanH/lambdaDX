use macroquad::math::{vec2, Vec2};
use crate::app::pad_svg::PadSvgDef;
use crate::app::types::{Note, PadGeom, SlideSegment, PAD_ROTATION_RAD, SLIDE_TILE_SPACING, TAP_TARGET_OFFSET};
use crate::app::types::zone::PadZone;

// ── Direction ──

enum ArcDir { CCW, CW }

// ── Helpers ──

fn a_ring_pos(zone: PadZone, outer_r: f32, spawn_cx: Vec2) -> Vec2 {
    let idx = (zone.to_id() - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let target_r = outer_r + TAP_TARGET_OFFSET;
    vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
}

fn b_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(8 + i), pad).unwrap()
}
fn a_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(i), pad).unwrap()
}

fn b_ring(svg: &PadSvgDef, pad: &PadGeom) -> (Vec2, f32) {
    let cents: Vec<Vec2> = (1..=8).map(|i| b_centroid(i, svg, pad)).collect();
    let center = cents.iter().sum::<Vec2>() / 8.0;
    let radius = cents.iter().map(|c| c.distance(center)).sum::<f32>() / 8.0;
    (center, radius)
}
fn a_ring(svg: &PadSvgDef, pad: &PadGeom) -> (Vec2, f32) {
    let cents: Vec<Vec2> = (1..=8).map(|i| a_centroid(i, svg, pad)).collect();
    let center = cents.iter().sum::<Vec2>() / 8.0;
    let radius = cents.iter().map(|c| c.distance(center)).sum::<f32>() / 8.0;
    (center, radius)
}


fn wrap(x: i32) -> u8 { ((x - 1).rem_euclid(8) + 1) as u8 }

/// 生成弧线上的采样点并推入 path
fn push_arc(path: &mut Vec<Vec2>, bp: f32, ep: f32, b_center: Vec2, b_radius: f32, spacing: f32) {
    let arc_len = b_radius * (ep - bp).abs();
    if arc_len < 1.0 { return; }
    let steps = ((arc_len / spacing).ceil() as usize).max(8);
    for i in 1..=steps {
        let ang = bp + (ep - bp) * i as f32 / steps as f32;
        path.push(b_center + vec2(ang.cos(), ang.sin()) * b_radius);
    }
}

// ── Shape builders ──

/// Q/P：起点 → 直线 → B弧 → 直线 → 终点（span 由 lane 和 target 自动算出）
fn build_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
    base_offset: i32,
    dir: ArcDir,
) {
    let sp = match seg.points.first() {
        Some(p) if seg.points.len() == 1 => p,
        _ => return,
    };
    let end = if sp.zone.to_id() <= 8 {
        a_ring_pos(sp.zone, outer_r, spawn_cx)
    } else {
        svg.zone_screen_centroid(sp.zone, pad).unwrap()
    };

    let (b_center, b_radius) = b_ring(svg, pad);
    let start_zone = base_offset + note.lane as i32;
    let span = 4 - ((note.lane as i32 - sp.zone.to_id() as i32 + 8) % 8);
    let end_zone = start_zone + span;

    let start_pos = b_centroid(wrap(start_zone), svg, pad);
    let end_pos   = b_centroid(wrap(end_zone),   svg, pad);

    let bp = (start_pos.y - b_center.y).atan2(start_pos.x - b_center.x);
    let mut ep = (end_pos.y - b_center.y).atan2(end_pos.x - b_center.x);
    match dir {
        ArcDir::CCW => { if ep <= bp { ep += std::f32::consts::TAU; } }
        ArcDir::CW  => { if ep >= bp { ep -= std::f32::consts::TAU; } }
    }

    path.push(start_pos);
    push_arc(path, bp, ep, b_center, b_radius, SLIDE_TILE_SPACING * scale);
    path.push(end);
}

/// Left/Right：起点 → 直线 → A弧 → 直线 → 终点（弧在 A 环上，span 由 lane 和 target 自动算出）
fn build_a_ring_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
    base_offset: i32,
    dir: ArcDir,
) {
    let sp = match seg.points.first() {
        Some(p) if seg.points.len() == 1 => p,
        _ => return,
    };
    // 终点：目标 zone 的 tap 圆点
    let end = a_ring_pos(sp.zone, outer_r, spawn_cx);

    // A 环圆心/半径：8 个 tap 圆点反算
    let ring_dots: Vec<Vec2> = (1..=8).map(|i| a_ring_pos(PadZone::from(i), outer_r, spawn_cx)).collect();
    let a_center = ring_dots.iter().sum::<Vec2>() / 8.0;
    let a_radius = ring_dots.iter().map(|c| c.distance(a_center)).sum::<f32>() / 8.0;

    // 弧起/止：note.lane 的 tap 圆点 → target 的 tap 圆点
    let start_pos = a_ring_pos(PadZone::from(note.lane), outer_r, spawn_cx);
    let end_pos   = end;

    let bp = (start_pos.y - a_center.y).atan2(start_pos.x - a_center.x);
    let mut ep = (end_pos.y - a_center.y).atan2(end_pos.x - a_center.x);
    match dir {
        ArcDir::CCW => { if ep <= bp { ep += std::f32::consts::TAU; } }
        ArcDir::CW  => { if ep >= bp { ep -= std::f32::consts::TAU; } }
    }

    push_arc(path, bp, ep, a_center, a_radius, SLIDE_TILE_SPACING * scale);
}

/// Caret：note1 → note2，两点间 B 环弧连接（seg.points 恰好 2 个）
fn build_caret_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
) {
   let sp = note.lane as i32;
    let ep = seg.points.first().unwrap().zone.to_id() as i32;
    if sort_cw(sp,ep,8) {
        build_a_ring_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, -1, ArcDir::CW);
    }else{
        build_a_ring_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, 1, ArcDir::CCW);
    }
    }
fn sort_cw(a: i32, b: i32, n: i32) -> bool {
    let cw = (b - a + n) % n;
    let ccw = (a - b + n) % n;

    if cw < ccw {
        false
    } else {
        true
    }
}
pub fn slide_shape_q(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, 2, ArcDir::CCW); }

pub fn slide_shape_p(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, -2, ArcDir::CW); }

pub fn slide_shape_left(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_a_ring_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, -1, ArcDir::CW); }

pub fn slide_shape_right(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_a_ring_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, 1, ArcDir::CCW); }

pub fn slide_shape_caret(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) {
    build_caret_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx); }

/// 直线连接 segment 的各个 waypoint
pub fn slide_shape_line(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, _scale: f32,
) {
    for sp in &seg.points {
        if sp.zone == note.lane && path.len() == 1 { continue; }
        let zid = sp.zone.to_id();
        let c = if zid >= 1 && zid <= 8 {
            Some(a_ring_pos(sp.zone, outer_r, spawn_cx))
        } else {
            svg.zone_screen_centroid(sp.zone, pad)
        };
        if let Some(c) = c {
            if path.last().map(|p| (*p - c).length() > 1.0).unwrap_or(true) {
                path.push(c);
            }
        }
    }
}
