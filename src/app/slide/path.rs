use macroquad::math::{vec2, Vec2};
use crate::app::pad_svg::PadSvgDef;
use crate::app::types::{Note, PadGeom, SlideSegment, PAD_ROTATION_RAD, SLIDE_TILE_SPACING, TAP_TARGET_OFFSET};
use crate::app::types::zone::PadZone;

// ── Direction ──

enum ArcDir { CCW, CW }

// ── Helpers ──

/// A-zone (1~8) → 外环八边形上坐标
fn a_ring_pos(zone: PadZone, outer_r: f32, spawn_cx: Vec2) -> Vec2 {
    let idx = (zone.to_id() - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let target_r = outer_r + TAP_TARGET_OFFSET;
    vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
}

/// B-zone centroid lookup (B1~B8)
fn b_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(8 + i), pad).unwrap()
}

/// B-ring circle: (center, radius)
fn b_ring(svg: &PadSvgDef, pad: &PadGeom) -> (Vec2, f32) {
    let cents: Vec<Vec2> = (1..=8).map(|i| b_centroid(i, svg, pad)).collect();
    let center = cents.iter().sum::<Vec2>() / 8.0;
    let radius = cents.iter().map(|c| c.distance(center)).sum::<f32>() / 8.0;
    (center, radius)
}

/// 角度归一化到 [0, TAU)
fn norm_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    ((a % tau) + tau) % tau
}

// ── Shape builders ──

/// Q/P 共用：起点 → 直线 → B弧起点 → [圆弧] → B弧终点 → 直线 → 终点
fn build_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
    base_offset: i32,   // Q: +1, P: -3
    dir: ArcDir,        // Q: CCW, P: CW
) {
    let sp = match seg.points.first() {
        Some(p) if seg.points.len() == 1 => p,
        _ => return,
    };

    // 终点
    let end = if sp.zone.to_id() <= 8 {
        a_ring_pos(sp.zone, outer_r, spawn_cx)
    } else {
        svg.zone_screen_centroid(sp.zone, pad).unwrap()
    };

    let (b_center, b_radius) = b_ring(svg, pad);

    // 弧的起止 B-zone 编号
    let start_zone = base_offset + note.lane as i32;
    let span = 4 - ((note.lane as i32 - sp.zone.to_id() as i32 + 8) % 8); // calc 内联
    let end_zone = start_zone + span;

    // wrap to 1..=8
    let wrap = |x: i32| ((x - 1).rem_euclid(8) + 1) as u8;

    let start_pos = b_centroid(wrap(start_zone), svg, pad);
    let end_pos   = b_centroid(wrap(end_zone),   svg, pad);

    let bp = (start_pos.y - b_center.y).atan2(start_pos.x - b_center.x);
    let mut ep = (end_pos.y - b_center.y).atan2(end_pos.x - b_center.x);

    // 方向调整
    match dir {
        ArcDir::CCW => { if ep <= bp { ep += std::f32::consts::TAU; } }
        ArcDir::CW  => { if ep >= bp { ep -= std::f32::consts::TAU; } }
    }

    // 直线: 起点 → 弧起点
    path.push(start_pos);

    // 圆弧: 弧起点 → 弧终点
    if wrap(start_zone) != wrap(end_zone) {
        let arc_len = b_radius * (ep - bp).abs();
        let spacing = SLIDE_TILE_SPACING * scale;
        let steps = ((arc_len / spacing).ceil() as usize).max(4);
        for i in 1..=steps {
            let ang = bp + (ep - bp) * i as f32 / steps as f32;
            path.push(b_center + vec2(ang.cos(), ang.sin()) * b_radius);
        }
    }

    // 直线: 弧终点 → 终点
    path.push(end);
}

pub fn slide_shape_q(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, 2, ArcDir::CCW); }

pub fn slide_shape_p(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, -2, ArcDir::CW); }

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
