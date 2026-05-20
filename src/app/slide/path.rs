use macroquad::math::{vec2, Vec2};
use crate::app::pad_svg::PadSvgDef;
use crate::app::types::{Note, PadGeom, SlideSegment, SlideShape, PAD_ROTATION_RAD, SLIDE_TILE_SPACING, TAP_TARGET_OFFSET};
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
fn d_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(17 + i), pad).unwrap()
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

fn b_perm_idx(zone: PadZone) -> Option<i32> {
    let zid = zone.to_id();
    if (9..=16).contains(&zid) {
        Some((zid - 9) as i32)
    } else {
        None
    }
}

/// A/D 边圈顺序：D1-A1-D2-A2-...-D8-A8（共 16 格）
fn ad_perm_idx(zone: PadZone) -> Option<i32> {
    let zid = zone.to_id();
    if (1..=8).contains(&zid) {
        Some(((zid - 1) as i32) * 2 + 1)
    } else if (18..=25).contains(&zid) {
        Some(((zid - 18) as i32) * 2)
    } else {
        None
    }
}

fn ad_ring_pos(zone: PadZone, svg: &PadSvgDef, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2) -> Option<Vec2> {
    let zid = zone.to_id();
    if (1..=8).contains(&zid) {
        Some(a_ring_pos(zone, outer_r, spawn_cx))
    } else if (18..=25).contains(&zid) {
        Some(svg.zone_screen_centroid(zone, pad).unwrap())
    } else {
        None
    }
}

fn ad_ring(svg: &PadSvgDef, pad: &PadGeom, outer_r: f32, spawn_cx: Vec2) -> (Vec2, f32) {
    let mut cents = Vec::with_capacity(16);
    for i in 1..=8 {
        cents.push(d_centroid(i, svg, pad));
        cents.push(a_ring_pos(PadZone::from(i), outer_r, spawn_cx));
    }
    let center = cents.iter().sum::<Vec2>() / 16.0;
    let radius = cents.iter().map(|c| c.distance(center)).sum::<f32>() / 16.0;
    (center, radius)
}

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
/// PP：与 QQ 相反（CW 弧），同用 AD 环固定圆
fn build_pp_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
) {
    let sp = match seg.points.first() {
        Some(p) if seg.points.len() == 1 => p,
        _ => return,
    };
    let target = sp.zone.to_id() as i32;
    let target_end = if sp.zone.to_id() <= 8 {
        a_ring_pos(sp.zone, outer_r, spawn_cx)
    } else {
        svg.zone_screen_centroid(sp.zone, pad).unwrap()
    };
    let c_pos = svg.zone_screen_centroid(PadZone::C, pad).unwrap();

    // 固定圆：圆心 = C 与 (lane+3 的 AD 环位置) 的中点（与 QQ 的 lane-3 相反）
    let lane_ad_idx = ad_perm_idx(PadZone::from(note.lane)).unwrap_or(0);
    let base_ad_idx = (lane_ad_idx + 3).rem_euclid(16);
    let base_zone = ad_idx_to_zone(base_ad_idx);
    let base_pos = ad_ring_pos(base_zone, svg, pad, outer_r, spawn_cx)
        .unwrap_or_else(|| svg.zone_screen_centroid(base_zone, pad).unwrap());
    let arc_center = (c_pos + base_pos) / 2.0;
    let arc_radius = c_pos.distance(base_pos) / 2.0;

    // 弧长：与 QQ 公式相反，从 lane-3 递减（QQ 从 lane+5 递减）
    let ideal = ((note.lane as i32 + 2) % 8 + 1); // lane+3, opposite of QQ's lane+5
    let dist = (target - ideal + 8) % 8;
    let arc_fraction = 1.0 - 0.1 * dist as f32;
    let arc_span = std::f32::consts::TAU * arc_fraction;

    // C 的角度 → CW（与 QQ 的 CCW 相反）
    let bp = (c_pos.y - arc_center.y).atan2(c_pos.x - arc_center.x);
    let ep = bp - arc_span;

    path.push(c_pos);
    push_arc(path, bp, ep, arc_center, arc_radius, SLIDE_TILE_SPACING * scale);
    path.push(target_end);
}

/// 将 AD 环上的 0..15 索引转回 PadZone
fn ad_idx_to_zone(idx: i32) -> PadZone {
    let i = idx.rem_euclid(16);
    if i % 2 == 0 {
        // 偶数 → D 区: D1@18..D8@25
        PadZone::from(18u8 + (i / 2) as u8)
    } else {
        // 奇数 → A 区: A1@1..A8@8
        PadZone::from(((i - 1) / 2 + 1) as u8)
    }
}

/// Edge：起点始终为 C 区，终点在 AD 环上
/// ad_pos = 3 + (lane - target)
/// AD 环索引 = 5 - ad_pos（wrapped to 0..15）
fn build_edge_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
    _base_offset: i32,
    _dir: ArcDir,
) {
    let sp = match seg.points.first() {
        Some(p) if seg.points.len() == 1 => p,
        _ => return,
    };
    // 目标终点：seg 指定的 zone
    let target_end = if sp.zone.to_id() <= 8 {
        a_ring_pos(sp.zone, outer_r, spawn_cx)
    } else {
        svg.zone_screen_centroid(sp.zone, pad).unwrap()
    };

    // C 区中心
    let c_pos = svg.zone_screen_centroid(PadZone::C, pad).unwrap();

    let target = sp.zone.to_id() as i32;

    // 固定圆：圆心 = C 与 (lane-3 的 AD 环位置) 的中点，半径 = 距离的一半
    let lane_ad_idx = ad_perm_idx(PadZone::from(note.lane)).unwrap_or(0);
    let base_ad_idx = (lane_ad_idx - 3).rem_euclid(16);
    let base_zone = ad_idx_to_zone(base_ad_idx);
    let base_pos = ad_ring_pos(base_zone, svg, pad, outer_r, spawn_cx)
        .unwrap_or_else(|| svg.zone_screen_centroid(base_zone, pad).unwrap());
    let arc_center = (c_pos + base_pos) / 2.0;
    let arc_radius = c_pos.distance(base_pos) / 2.0;

    // 弧长：target=lane+5 → 100%，+4→90%，+3→80%... 单向递减
    let ideal = ((note.lane as i32 + 4) % 8 + 1); // lane+5 wrapped to 1..8
    let dist = (ideal - target + 8) % 8;           // 0=ideal, 1..7 递减
    let arc_fraction = 1.0 - 0.1 * dist as f32;
    let arc_span = std::f32::consts::TAU * arc_fraction;

    // C 在固定圆上的角度 → CCW 走 arc_span
    let bp = (c_pos.y - arc_center.y).atan2(c_pos.x - arc_center.x);
    let ep = bp + arc_span;

    // note.lane → C（直线，note 起点已在 path 中）
    path.push(c_pos);
    // C → 圆弧
    push_arc(path, bp, ep, arc_center, arc_radius, SLIDE_TILE_SPACING * scale);
    // AD 环 → 目标终点（直线）
    path.push(target_end);
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

fn build_z_arc(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    svg: &PadSvgDef,
    pad: &PadGeom,
    scale: f32,
    outer_r: f32,
    spawn_cx: Vec2,
    slide_type:SlideShape
) {
    let sp = match seg.points.first() {
        Some(p) if seg.points.len() == 1 => p,
        _ => return,
    };
    // 目标终点：seg 指定的 zone
    let target_end = if sp.zone.to_id() <= 8 {
        a_ring_pos(sp.zone, outer_r, spawn_cx)
    } else {
        svg.zone_screen_centroid(sp.zone, pad).unwrap()
    };

    // C 区中心
    let c_pos = svg.zone_screen_centroid(PadZone::C, pad).unwrap();

    let b1 =    svg.zone_screen_centroid(PadZone::num_to_b(note.lane as i8+2),pad).unwrap();
    let b2 =    svg.zone_screen_centroid(PadZone::num_to_b(note.lane as i8-2),pad).unwrap();

    if matches!(slide_type,SlideShape::Z) {
        path.push(b1);
        path.push(b2);
    }else if  matches!(slide_type,SlideShape::S) {
        path.push(b2);
        path.push(b1);
    }

    // 目标终点（直线）
    path.push(target_end);
}

pub fn slide_shape_q(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, 2, ArcDir::CCW); }
pub fn slide_shape_qq(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_edge_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, 1, ArcDir::CCW); }

pub fn slide_shape_p(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, -2, ArcDir::CW); }

pub fn slide_shape_pp(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) { build_pp_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx); }

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

pub fn slide_shape_z(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) {
    build_z_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, SlideShape::Z);
}
pub fn slide_shape_s(
    path: &mut Vec<Vec2>, note: &Note, seg: &SlideSegment,
    outer_r: f32, spawn_cx: Vec2, pad: &PadGeom, svg: &PadSvgDef, scale: f32,
) {
    build_z_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx, SlideShape::S);
}

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
