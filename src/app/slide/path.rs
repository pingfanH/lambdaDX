use crate::app::pad_svg::PadSvgDef;
use crate::app::types::zone::PadZone;
use crate::app::types::{
    Note, PAD_ROTATION_RAD, PadGeom, SLIDE_TILE_SPACING, SlideSegment, SlideShape,
    TAP_TARGET_OFFSET,
};
use macroquad::math::{Vec2, vec2};

// ── Direction ──

enum ArcDir {
    CCW,
    CW,
}

// ── Helpers ──

fn a_ring_pos(zone: PadZone, outer_r: f32, spawn_cx: Vec2) -> Vec2 {
    let idx = (zone.to_id() - 1) as f32;
    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
    let target_r = outer_r + TAP_TARGET_OFFSET;
    vec2(
        spawn_cx.x + ang.cos() * target_r,
        spawn_cx.y + ang.sin() * target_r,
    )
}

fn b_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(8 + i), pad).unwrap()
}
fn a_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(i), pad).unwrap()
}
fn d_centroid(i: u8, svg: &PadSvgDef, pad: &PadGeom) -> Vec2 {
    svg.zone_screen_centroid(PadZone::from(17 + i), pad)
        .unwrap()
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

fn wrap(x: i32) -> u8 {
    ((x - 1).rem_euclid(8) + 1) as u8
}

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

fn ad_ring_pos(
    zone: PadZone,
    svg: &PadSvgDef,
    pad: &PadGeom,
    outer_r: f32,
    spawn_cx: Vec2,
) -> Option<Vec2> {
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
    if arc_len < 1.0 {
        return;
    }
    let steps = ((arc_len / spacing).ceil() as usize).max(8);
    for i in 1..=steps {
        let ang = bp + (ep - bp) * i as f32 / steps as f32;
        path.push(b_center + vec2(ang.cos(), ang.sin()) * b_radius);
    }
}
fn push_corner_bezier(path: &mut Vec<Vec2>, corner: Vec2, next: Vec2, radius: f32, spacing: f32) {
    if path.is_empty() {
        path.push(corner);

        path.push(next);

        return;
    }

    let prev = *path.last().unwrap();

    let v1 = corner - prev;

    let v2 = next - corner;

    let len1 = v1.length();

    let len2 = v2.length();

    if len1 < 1e-3 || len2 < 1e-3 {
        path.push(corner);

        path.push(next);

        return;
    }

    let d1 = v1 / len1;

    let d2 = v2 / len2;

    let r = radius.min(len1 * 0.5).min(len2 * 0.5);

    // 贝塞尔起点和终点

    let p0 = corner - d1 * r;

    let p2 = corner + d2 * r;

    // 用 corner 作为控制点

    let ctrl = corner;

    // 先接到圆角起点

    path.push(p0);

    let approx_len = p0.distance(ctrl) + ctrl.distance(p2);

    let steps = ((approx_len / spacing).ceil() as usize).max(6);

    for i in 1..=steps {
        let t = i as f32 / steps as f32;

        let u = 1.0 - t;

        let p = p0 * (u * u) + ctrl * (2.0 * u * t) + p2 * (t * t);

        path.push(p);
    }

    path.push(next);
}

/// Signed smallest angle difference `a - b`, in `(-π, π]`.
fn signed_delta(a: f32, b: f32) -> f32 {
    (a - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Unit tangent of a circle at angle `a` in the travel direction (`sign = +1`
/// when the angle increases, `-1` when it decreases).
fn arc_tangent(a: f32, sign: f32) -> Vec2 {
    vec2(-a.sin(), a.cos()) * sign
}

/// Append cubic Bézier samples from `p0` to `p3` (excludes `p0`, includes `p3`)
/// with unit tangents `t0` at `p0` and `t1` at `p3`, so the join is G1.
fn push_cubic(path: &mut Vec<Vec2>, p0: Vec2, t0: Vec2, p3: Vec2, t1: Vec2, spacing: f32) {
    let d = (p3 - p0).length();
    if d < 0.5 {
        path.push(p3);
        return;
    }
    let k = d / 3.0;
    let p1 = p0 + t0 * k;
    let p2 = p3 - t1 * k;
    let steps = ((d / spacing.max(1.0)).ceil() as usize).max(6);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        path.push(p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t));
    }
}

/// Join a straight segment into the start of a circular arc with a G1 cubic.
///
/// `path[idx]` is the arc's first point (angle `a0`), preceded by the straight.
/// The straight and the arc are pulled back by `fillet_r`, the arc samples in
/// between are dropped, and a cubic Bézier that is tangent to the straight at
/// one end and to the arc at the other is inserted. This removes the visible
/// seam that a small quadratic fillet left behind.
#[allow(clippy::too_many_arguments)]
fn blend_line_to_arc(
    path: &mut Vec<Vec2>,
    idx: usize,
    center: Vec2,
    radius: f32,
    a0: f32,
    sign: f32,
    fillet_r: f32,
    spacing: f32,
) {
    if idx == 0 || radius < 1.0 {
        return;
    }
    let corner = path[idx];
    let prev = path[idx - 1];
    let lin = corner - prev;
    if lin.length() < 1e-3 {
        return;
    }
    let t_in = lin.normalize();

    let da = (fillet_r / radius).clamp(0.0, std::f32::consts::FRAC_PI_2);
    let a1 = a0 + sign * da;
    let p_arc = center + vec2(a1.cos(), a1.sin()) * radius;
    let t_arc = arc_tangent(a1, sign);

    let r = fillet_r.min(lin.length());
    let p0 = corner - t_in * r;

    // Drop the arc samples that lie before `a1` (they are replaced by the blend).
    let mut end = idx + 1;
    while end < path.len() {
        let p = path[end];
        let ang = (p.y - center.y).atan2(p.x - center.x);
        if signed_delta(ang, a0) * sign >= da {
            break;
        }
        end += 1;
    }

    let mut repl = vec![p0];
    push_cubic(&mut repl, p0, t_in, p_arc, t_arc, spacing);
    path.splice(idx..end, repl);
}

/// Join the end of a circular arc into a straight segment with a G1 cubic.
///
/// `path[idx]` is the arc's last point (angle `a1`) and `path[idx+1]` is the
/// straight's far end. Symmetric to [`blend_line_to_arc`].
#[allow(clippy::too_many_arguments)]
fn blend_arc_to_line(
    path: &mut Vec<Vec2>,
    idx: usize,
    center: Vec2,
    radius: f32,
    a1: f32,
    sign: f32,
    fillet_r: f32,
    spacing: f32,
) {
    if idx == 0 || idx + 1 >= path.len() || radius < 1.0 {
        return;
    }
    let corner = path[idx];
    let next = path[idx + 1];
    let lout = next - corner;
    if lout.length() < 1e-3 {
        return;
    }
    let t_out = lout.normalize();

    let da = (fillet_r / radius).clamp(0.0, std::f32::consts::FRAC_PI_2);
    let a0 = a1 - sign * da;
    let p_arc = center + vec2(a0.cos(), a0.sin()) * radius;
    let t_arc = arc_tangent(a0, sign);

    let r = fillet_r.min(lout.length());
    let p1 = corner + t_out * r;

    // Drop the arc samples that lie after `a0` (they are replaced by the blend).
    let mut start = idx;
    while start > 0 {
        let p = path[start - 1];
        let ang = (p.y - center.y).atan2(p.x - center.x);
        if signed_delta(a1, ang) * sign >= da {
            break;
        }
        start -= 1;
    }

    let mut repl = vec![p_arc];
    push_cubic(&mut repl, p_arc, t_arc, p1, t_out, spacing);
    path.splice(start..=idx, repl);
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
    let end_pos = b_centroid(wrap(end_zone), svg, pad);

    let bp = (start_pos.y - b_center.y).atan2(start_pos.x - b_center.x);
    let mut ep = (end_pos.y - b_center.y).atan2(end_pos.x - b_center.x);
    match dir {
        ArcDir::CCW => {
            if ep <= bp {
                ep += std::f32::consts::TAU;
            }
        }
        ArcDir::CW => {
            if ep >= bp {
                ep -= std::f32::consts::TAU;
            }
        }
    }

    if start_zone == end_zone {
        push_corner_bezier(
            path,
            start_pos,
            end,
            20.0 * scale,
            SLIDE_TILE_SPACING * scale,
        );
        // path.push(start_pos);
        // path.push(end);
    } else {
        // Start → B-ring entry (straight), B-arc, B-exit → end (straight).
        // Smooth both line↔arc junctions with G1 cubics (arc side first so the
        // earlier index stays valid).
        let sign = if ep >= bp { 1.0 } else { -1.0 };
        let start_idx = path.len();
        path.push(start_pos);
        push_arc(path, bp, ep, b_center, b_radius, SLIDE_TILE_SPACING * scale);
        let end_idx = path.len() - 1;
        path.push(end);
        let fillet_r = (b_radius * 0.3).clamp(8.0 * scale, 40.0 * scale);
        blend_arc_to_line(
            path, end_idx, b_center, b_radius, ep, sign, fillet_r, SLIDE_TILE_SPACING * scale,
        );
        blend_line_to_arc(
            path, start_idx, b_center, b_radius, bp, sign, fillet_r, SLIDE_TILE_SPACING * scale,
        );
    }
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
    let ideal = (note.lane as i32 + 2) % 8 + 1; // lane+3, opposite of QQ's lane+5
    let dist = (target - ideal + 8) % 8;
    let arc_fraction = 1.0 - 0.1 * dist as f32;
    let arc_span = std::f32::consts::TAU * arc_fraction;

    // C 的角度 → CW（与 QQ 的 CCW 相反）
    let bp = (c_pos.y - arc_center.y).atan2(c_pos.x - arc_center.x);
    let ep = bp - arc_span;

    // C → arc → target, with G1 cubics at both line↔arc junctions (arc side
    // first so the earlier index stays valid). PP runs clockwise: sign = -1.
    let start_idx = path.len();
    path.push(c_pos);
    push_arc(
        path,
        bp,
        ep,
        arc_center,
        arc_radius,
        SLIDE_TILE_SPACING * scale,
    );
    let end_idx = path.len() - 1;
    path.push(target_end);
    let fillet_r = (arc_radius * 0.3).clamp(8.0 * scale, 40.0 * scale);
    blend_arc_to_line(
        path, end_idx, arc_center, arc_radius, ep, -1.0, fillet_r, SLIDE_TILE_SPACING * scale,
    );
    blend_line_to_arc(
        path, start_idx, arc_center, arc_radius, bp, -1.0, fillet_r, SLIDE_TILE_SPACING * scale,
    );
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
    let ideal = (note.lane as i32 + 4) % 8 + 1; // lane+5 wrapped to 1..8
    let dist = (ideal - target + 8) % 8; // 0=ideal, 1..7 递减
    let arc_fraction = 1.0 - 0.1 * dist as f32;
    let arc_span = std::f32::consts::TAU * arc_fraction;

    // C 在固定圆上的角度 → CCW 走 arc_span
    let bp = (c_pos.y - arc_center.y).atan2(c_pos.x - arc_center.x);
    let ep = bp + arc_span;

    // note.lane → C（直线），C → 圆弧，圆弧 → 目标终点（直线）。两处接缝用
    // G1 三次贝塞尔过渡（先处理靠后的接缝，索引不失效）。QQ 逆时针：sign = +1。
    let start_idx = path.len();
    path.push(c_pos);
    push_arc(
        path,
        bp,
        ep,
        arc_center,
        arc_radius,
        SLIDE_TILE_SPACING * scale,
    );
    let end_idx = path.len() - 1;
    path.push(target_end);
    let fillet_r = (arc_radius * 0.3).clamp(8.0 * scale, 40.0 * scale);
    blend_arc_to_line(
        path, end_idx, arc_center, arc_radius, ep, 1.0, fillet_r, SLIDE_TILE_SPACING * scale,
    );
    blend_line_to_arc(
        path, start_idx, arc_center, arc_radius, bp, 1.0, fillet_r, SLIDE_TILE_SPACING * scale,
    );
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
    let ring_dots: Vec<Vec2> = (1..=8)
        .map(|i| a_ring_pos(PadZone::from(i), outer_r, spawn_cx))
        .collect();
    let a_center = ring_dots.iter().sum::<Vec2>() / 8.0;
    let a_radius = ring_dots.iter().map(|c| c.distance(a_center)).sum::<f32>() / 8.0;

    // 弧起/止：note.lane 的 tap 圆点 → target 的 tap 圆点
    let start_pos = a_ring_pos(PadZone::from(note.lane), outer_r, spawn_cx);
    let end_pos = end;

    let bp = (start_pos.y - a_center.y).atan2(start_pos.x - a_center.x);
    let mut ep = (end_pos.y - a_center.y).atan2(end_pos.x - a_center.x);
    match dir {
        ArcDir::CCW => {
            if ep <= bp {
                ep += std::f32::consts::TAU;
            }
        }
        ArcDir::CW => {
            if ep >= bp {
                ep -= std::f32::consts::TAU;
            }
        }
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
    if sort_cw(sp, ep, 8) {
        build_a_ring_arc(
            path,
            note,
            seg,
            svg,
            pad,
            scale,
            outer_r,
            spawn_cx,
            -1,
            ArcDir::CW,
        );
    } else {
        build_a_ring_arc(
            path,
            note,
            seg,
            svg,
            pad,
            scale,
            outer_r,
            spawn_cx,
            1,
            ArcDir::CCW,
        );
    }
}
fn sort_cw(a: i32, b: i32, n: i32) -> bool {
    let cw = (b - a + n) % n;
    let ccw = (a - b + n) % n;

    if cw < ccw { false } else { true }
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
    slide_type: SlideShape,
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

    let b1 = svg
        .zone_screen_centroid(PadZone::num_to_b(note.lane as i8 + 2), pad)
        .unwrap();
    let b2 = svg
        .zone_screen_centroid(PadZone::num_to_b(note.lane as i8 - 2), pad)
        .unwrap();

    if matches!(slide_type, SlideShape::Z) {
        path.push(b1);
        path.push(b2);
    } else if matches!(slide_type, SlideShape::S) {
        path.push(b2);
        path.push(b1);
    }

    // 目标终点（直线）
    path.push(target_end);
}

pub fn slide_shape_q(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        2,
        ArcDir::CCW,
    );
}
pub fn slide_shape_qq(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_edge_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        1,
        ArcDir::CCW,
    );
}

pub fn slide_shape_p(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        -2,
        ArcDir::CW,
    );
}

pub fn slide_shape_pp(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_pp_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx);
}

pub fn slide_shape_left(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_a_ring_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        -1,
        if note.lane < 7 && note.lane > 2 {
            ArcDir::CCW
        } else {
            ArcDir::CW
        },
    );
}

pub fn slide_shape_right(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_a_ring_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        1,
        if note.lane < 7 && note.lane > 2 {
            ArcDir::CW
        } else {
            ArcDir::CCW
        },
    );
}

pub fn slide_shape_caret(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_caret_arc(path, note, seg, svg, pad, scale, outer_r, spawn_cx);
}

pub fn slide_shape_z(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_z_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        SlideShape::Z,
    );
}
pub fn slide_shape_s(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    scale: f32,
) {
    build_z_arc(
        path,
        note,
        seg,
        svg,
        pad,
        scale,
        outer_r,
        spawn_cx,
        SlideShape::S,
    );
}

/// 直线连接 segment 的各个 waypoint
pub fn slide_shape_line(
    path: &mut Vec<Vec2>,
    note: &Note,
    seg: &SlideSegment,
    outer_r: f32,
    spawn_cx: Vec2,
    pad: &PadGeom,
    svg: &PadSvgDef,
    _scale: f32,
) {
    for sp in &seg.points {
        if sp.zone == note.lane && path.len() == 1 {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::{blend_arc_to_line, blend_line_to_arc};
    use macroquad::math::{Vec2, vec2};

    /// Largest direction change (radians) between consecutive path segments.
    fn max_turn(points: &[Vec2]) -> f32 {
        let mut m = 0.0_f32;
        for w in points.windows(3) {
            let a = (w[1] - w[0]).normalize_or_zero();
            let b = (w[2] - w[1]).normalize_or_zero();
            if a.length_squared() > 0.0 && b.length_squared() > 0.0 {
                m = m.max(a.dot(b).clamp(-1.0, 1.0).acos());
            }
        }
        m
    }

    fn arc_pt(c: Vec2, r: f32, a: f32) -> Vec2 {
        c + vec2(a.cos(), a.sin()) * r
    }
    fn arc_pts(c: Vec2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Vec2> {
        (1..=n)
            .map(|i| arc_pt(c, r, a0 + (a1 - a0) * i as f32 / n as f32))
            .collect()
    }

    // 90° corner: straight along +x meets a circle whose tangent there is +y.
    #[test]
    fn blend_line_to_arc_removes_the_seam() {
        let c = vec2(0.0, 0.0);
        let r = 100.0;
        let (a0, a1) = (0.0_f32, 1.0_f32);
        let mut path = vec![vec2(40.0, 0.0), arc_pt(c, r, a0)];
        path.extend(arc_pts(c, r, a0, a1, 12));
        assert!(max_turn(&path) > 1.0, "expected a sharp corner");

        blend_line_to_arc(&mut path, 1, c, r, a0, 1.0, 30.0, 10.0);

        assert_eq!(path.first().copied(), Some(vec2(40.0, 0.0)));
        assert!(
            (max_turn(&path) - 0.0).abs() < 0.6,
            "junction still bent: {}",
            max_turn(&path)
        );
    }

    #[test]
    fn blend_arc_to_line_removes_the_seam() {
        let c = vec2(0.0, 0.0);
        let r = 100.0;
        let (a0, a1) = (0.0_f32, 1.0_f32);
        let mut path = vec![arc_pt(c, r, a0)];
        path.extend(arc_pts(c, r, a0, a1, 12));
        let end = *path.last().unwrap();
        path.push(end + vec2(50.0, 0.0));
        let idx = path.len() - 2;
        assert!(max_turn(&path) > 1.0, "expected a sharp corner");

        blend_arc_to_line(&mut path, idx, c, r, a1, 1.0, 30.0, 10.0);

        assert_eq!(path.last().copied(), Some(end + vec2(50.0, 0.0)));
        assert!(
            (max_turn(&path) - 0.0).abs() < 0.6,
            "junction still bent: {}",
            max_turn(&path)
        );
    }

    #[test]
    fn blend_is_a_noop_on_degenerate_input() {
        let mut path = vec![vec2(0.0, 0.0), vec2(1.0, 0.0)];
        let before = path.clone();
        // No arc point / end point to blend.
        blend_line_to_arc(&mut path, 0, vec2(0.0, 0.0), 100.0, 0.0, 1.0, 30.0, 10.0);
        blend_arc_to_line(&mut path, 1, vec2(0.0, 0.0), 100.0, 0.0, 1.0, 30.0, 10.0);
        assert_eq!(path, before);
    }

    #[test]
    fn real_q_and_qq_paths_have_no_sharp_junction() {
        use crate::app::pad_svg::PadSvgDef;
        use crate::app::types::zone::PadZone;
        use crate::app::types::{Note, NoteType, PadGeom, SlidePoint, SlideSegment, SlideShape};

        let svg = PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg")).unwrap();
        let pad = PadGeom {
            cx: 400.0,
            cy: 400.0,
            outer_r: 300.0,
        };
        let spawn = vec2(400.0, 400.0);

        for (shape, end, f) in [
            (
                SlideShape::Q,
                5u8,
                super::slide_shape_q
                    as fn(&mut Vec<Vec2>, &Note, &SlideSegment, f32, Vec2, &PadGeom, &PadSvgDef, f32),
            ),
            (
                SlideShape::QQ,
                5,
                super::slide_shape_qq
                    as fn(&mut Vec<Vec2>, &Note, &SlideSegment, f32, Vec2, &PadGeom, &PadSvgDef, f32),
            ),
        ] {
            let note = Note {
                time: 1.0,
                lane: 1,
                note_type: NoteType::Slide,
                ..Default::default()
            };
            let seg = SlideSegment {
                points: vec![SlidePoint {
                    zone: PadZone::from(end),
                    beat_offset: 0.0,
                }],
                shape,
            };
            // The caller (draw_slide) always seeds the path with the head.
            let mut path = vec![super::a_ring_pos(PadZone::from(1), 300.0, spawn)];
            f(&mut path, &note, &seg, 300.0, spawn, &pad, &svg, 1.0);
            assert!(path.len() > 4, "{shape:?} path too short: {}", path.len());
            assert!(
                max_turn(&path) < 0.7,
                "{shape:?} still has a sharp junction: {}",
                max_turn(&path)
            );
        }
    }
}
