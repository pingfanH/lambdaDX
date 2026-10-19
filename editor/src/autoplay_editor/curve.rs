//! Arc-length motion and sensor baking, independent of the edit gesture.
use super::*;
use crate::app::{
    autoplay_edit::{AutoplayCurve, CurveHandles, CurveTiming},
    pad_svg::PadSvgDef,
    types::{PadGeom, SlideShape},
};
use crate::core::types::sort_timed_input_events;

const RADIUS: f32 = 400.0;
const MAX_POINTS: usize = 2048;

pub(super) fn ensure_handles(curve: &mut AutoplayCurve) {
    if curve.handles.len() == curve.points.len() {
        return;
    }
    curve.handles = curve
        .points
        .iter()
        .enumerate()
        .map(|(i, point)| {
            let p = Pos2::from(*point);
            let before = Pos2::from(curve.points[i.saturating_sub(1)]);
            let after = Pos2::from(curve.points[(i + 1).min(curve.points.len() - 1)]);
            let tangent = (after - before) / 6.0;
            CurveHandles {
                incoming: (if curve.smooth {
                    p - tangent
                } else {
                    p.lerp(before, 1.0 / 3.0)
                })
                .into(),
                outgoing: (if curve.smooth {
                    p + tangent
                } else {
                    p.lerp(after, 1.0 / 3.0)
                })
                .into(),
                aligned: curve.smooth,
            }
        })
        .collect();
}

pub(super) fn move_point(curve: &mut AutoplayCurve, index: usize, point: [f32; 2]) {
    ensure_handles(curve);
    let delta = Pos2::from(point) - Pos2::from(curve.points[index]);
    curve.points[index] = point;
    let handles = &mut curve.handles[index];
    handles.incoming = (Pos2::from(handles.incoming) + delta).into();
    handles.outgoing = (Pos2::from(handles.outgoing) + delta).into();
}

pub(super) fn move_handle(
    curve: &mut AutoplayCurve,
    index: usize,
    incoming: bool,
    point: [f32; 2],
    free: bool,
) {
    ensure_handles(curve);
    let p = Pos2::from(curve.points[index]);
    let handles = &mut curve.handles[index];
    if free {
        handles.aligned = false;
    }
    let (active, opposite) = if incoming {
        (&mut handles.incoming, &mut handles.outgoing)
    } else {
        (&mut handles.outgoing, &mut handles.incoming)
    };
    *active = point;
    let delta = Pos2::from(point) - p;
    if handles.aligned && delta.length_sq() > 1e-12 {
        *opposite = (p - delta.normalized() * p.distance(Pos2::from(*opposite))).into();
    }
}

pub(super) fn remove_point(curve: &mut AutoplayCurve, index: usize) {
    ensure_handles(curve);
    curve.points.remove(index);
    curve.handles.remove(index);
}

pub(super) fn append_point(curve: &mut AutoplayCurve, point: [f32; 2]) {
    ensure_handles(curve);
    let p = Pos2::from(point);
    let before = curve.points.last().copied().map(Pos2::from).unwrap_or(p);
    if let Some(handle) = curve.handles.last_mut() {
        handle.outgoing = before.lerp(p, 1.0 / 3.0).into();
        handle.aligned = false;
    }
    curve.points.push(point);
    curve.handles.push(CurveHandles {
        incoming: p.lerp(before, 1.0 / 3.0).into(),
        outgoing: point,
        aligned: false,
    });
}

fn segments(curve: &AutoplayCurve) -> Vec<[Pos2; 4]> {
    let mut curve = curve.clone();
    ensure_handles(&mut curve);
    curve
        .points
        .windows(2)
        .enumerate()
        .map(|(i, p)| {
            [
                Pos2::from(p[0]),
                Pos2::from(curve.handles[i].outgoing),
                Pos2::from(curve.handles[i + 1].incoming),
                Pos2::from(p[1]),
            ]
        })
        .collect()
}

fn evaluate(p: [Pos2; 4], t: f32) -> Pos2 {
    let a = p[0].lerp(p[1], t);
    let b = p[1].lerp(p[2], t);
    let c = p[2].lerp(p[3], t);
    a.lerp(b, t).lerp(b.lerp(c, t), t)
}

pub(super) fn insert_nearest(
    curve: &mut AutoplayCurve,
    point: Pos2,
    tolerance: f32,
) -> Option<usize> {
    let segments = segments(curve);
    let (index, sample, _) = segments
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            (0..=32).map(move |step| (i, step, point.distance_sq(evaluate(*p, step as f32 / 32.0))))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))?;
    let p = segments[index];
    let mut lo = (sample as f32 - 1.0).max(0.0) / 32.0;
    let mut hi = (sample as f32 + 1.0).min(32.0) / 32.0;
    for _ in 0..16 {
        let a = lo + (hi - lo) / 3.0;
        let b = hi - (hi - lo) / 3.0;
        if point.distance_sq(evaluate(p, a)) < point.distance_sq(evaluate(p, b)) {
            hi = b;
        } else {
            lo = a;
        }
    }
    let t = (lo + hi) * 0.5;
    if point.distance(evaluate(p, t)) > tolerance || !(0.001..=0.999).contains(&t) {
        return None;
    }
    // De Casteljau subdivision inserts a node without changing the path.
    let a = p[0].lerp(p[1], t);
    let b = p[1].lerp(p[2], t);
    let c = p[2].lerp(p[3], t);
    let d = a.lerp(b, t);
    let e = b.lerp(c, t);
    let node = d.lerp(e, t);
    ensure_handles(curve);
    curve.handles[index].outgoing = a.into();
    curve.handles[index + 1].incoming = c.into();
    curve.points.insert(index + 1, node.into());
    curve.handles.insert(
        index + 1,
        CurveHandles {
            incoming: d.into(),
            outgoing: e.into(),
            aligned: true,
        },
    );
    Some(index + 1)
}

pub(super) fn canonical_pad() -> PadGeom {
    PadGeom {
        cx: 0.0,
        cy: 0.0,
        outer_r: RADIUS,
    }
}

pub(super) fn segment_distance(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let delta = b - a;
    let t = ((point - a).dot(delta) / delta.length_sq().max(1e-12)).clamp(0.0, 1.0);
    point.distance(a + t * delta)
}

pub(super) struct Path {
    pub points: Vec<Pos2>,
    distances: Vec<f32>,
    pub length: f32,
}

impl Path {
    pub fn new(curve: &AutoplayCurve) -> Self {
        let mut points = Vec::new();
        if valid_geometry(curve) {
            for (i, nodes) in segments(curve).into_iter().enumerate() {
                let shape = egui::epaint::CubicBezierShape::from_points_stroke(
                    nodes,
                    false,
                    Color32::TRANSPARENT,
                    Stroke::NONE,
                );
                let flattened = shape.flatten(Some(0.001));
                points.extend(flattened.into_iter().skip(usize::from(i != 0)));
            }
            if points.is_empty() {
                points.extend(curve.points.first().copied().map(Pos2::from));
            }
        }
        points.dedup_by(|a, b| a.distance_sq(*b) < 1e-12);
        let mut distances = vec![0.0; points.len()];
        for i in 1..points.len() {
            distances[i] = distances[i - 1] + points[i - 1].distance(points[i]);
        }
        let length = distances.last().copied().unwrap_or(0.0);
        Self {
            points,
            distances,
            length,
        }
    }

    pub fn at(&self, fraction: f32) -> Pos2 {
        if self.points.len() < 2 {
            return self.points.first().copied().unwrap_or(Pos2::ZERO);
        }
        let d = fraction.clamp(0.0, 1.0) * self.length;
        let i = self
            .distances
            .partition_point(|v| *v <= d)
            .clamp(1, self.points.len() - 1);
        let t = ((d - self.distances[i - 1])
            / (self.distances[i] - self.distances[i - 1]).max(1e-9))
        .clamp(0.0, 1.0);
        self.points[i - 1].lerp(self.points[i], t)
    }

    pub fn duration(&self, curve: &AutoplayCurve) -> f64 {
        match curve.timing {
            CurveTiming::ConstantSpeed => self.length as f64 / curve.speed.max(0.001),
            CurveTiming::FixedDuration => curve.duration,
        }
    }
}

fn valid_geometry(curve: &AutoplayCurve) -> bool {
    let valid = |p: &[f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 4.0);
    curve.points.len() <= MAX_POINTS
        && curve.points.iter().all(valid)
        && (curve.handles.is_empty()
            || curve.handles.len() == curve.points.len()
                && curve
                    .handles
                    .iter()
                    .all(|h| valid(&h.incoming) && valid(&h.outgoing)))
}

pub(super) fn bake(curve: &AutoplayCurve, svg: &PadSvgDef) -> Result<Vec<TimedInputEvent>, String> {
    if curve.points.len() < 2
        || !valid_geometry(curve)
        || !curve.start.is_finite()
        || curve.start.abs() > 86400.0
        || !curve.speed.is_finite()
        || curve.speed <= 0.0
        || !curve.radius.is_finite()
        || !(0.0..=0.25).contains(&curve.radius)
    {
        return Err("Invalid curve coordinates or timing".into());
    }
    let path = Path::new(curve);
    let duration = path.duration(curve);
    if path.length < 1e-6 || !duration.is_finite() || !(0.001..=86400.0).contains(&duration) {
        return Err("Curve needs a non-zero path and a duration between 0.001 and 86400 s".into());
    }
    // Spatial sampling stays independent of speed. Refine each crossing to
    // avoid timing shifts when only the duration is changed.
    let steps = (path.length / 0.002).ceil() as usize;
    if steps > 100_000 {
        return Err("Curve is too long; split it into several clips".into());
    }
    let pad = canonical_pad();
    let zones = |fraction: f32| -> std::collections::BTreeSet<String> {
        let p = path.at(fraction);
        svg.hit_test_range(
            macroquad::prelude::vec2(p.x, p.y) * RADIUS,
            &pad,
            curve.radius * RADIUS,
        )
        .iter()
        .map(ToString::to_string)
        .collect()
    };
    let mut events = Vec::new();
    let mut previous = zones(0.0);
    let tp = |f: f32| ((curve.start + duration * f as f64) * 1e6).round() as i64;
    for zone in &previous {
        events.push(make_event(zone, 1, tp(0.0)));
    }
    for i in 1..=steps {
        let f = i as f32 / steps as f32;
        let next = zones(f);
        for zone in previous.symmetric_difference(&next) {
            let entering = next.contains(zone);
            let mut lo = (i - 1) as f32 / steps as f32;
            let mut hi = f;
            for _ in 0..10 {
                let mid = (lo + hi) * 0.5;
                if zones(mid).contains(zone) == entering {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            events.push(make_event(zone, if entering { 1 } else { 2 }, tp(hi)));
        }
        previous = next;
    }
    let end = tp(1.0);
    for zone in previous {
        events.push(make_event(&zone, 2, end));
    }
    sort_timed_input_events(&mut events);
    // Quantized crossings can occupy one microsecond. Drop only zero-length
    // intervals, never leave an unmatched press in the core input stream.
    let spans = clips::intervals(&events);
    let mut remove = HashSet::new();
    for span in spans {
        if let Some(release) = span.release {
            if timed_input_tp(&events[span.press]) >= timed_input_tp(&events[release]) {
                remove.insert(span.press);
                remove.insert(release);
            }
        }
    }
    events = events
        .into_iter()
        .enumerate()
        .filter_map(|(i, e)| (!remove.contains(&i)).then_some(e))
        .collect();
    if events.is_empty() {
        return Err("Curve does not cross a sensor area".into());
    }
    Ok(events)
}

/// Ring point of a lane, in the same space as `build_slide_path` output.
fn wifi_ring(center: macroquad::prelude::Vec2, lane: u8) -> macroquad::prelude::Vec2 {
    let angle = -std::f32::consts::FRAC_PI_2
        + crate::app::types::PAD_ROTATION_RAD
        + lane.saturating_sub(1) as f32 * std::f32::consts::TAU / 8.0;
    center
        + macroquad::prelude::vec2(angle.cos(), angle.sin())
            * (RADIUS + crate::app::params::tap_target_offset())
}

/// The Wi-Fi fan spreads across three lanes starting three steps ahead of the
/// head; the middle one is the endpoint the format declares.
fn wifi_branch_lane(start: u8, branch: u8) -> u8 {
    ((start as i32 - 1 + 3 + branch as i32).rem_euclid(8) + 1) as u8
}

/// Append the straight Wi-Fi legs and return the lane the fan continues from.
fn append_wifi_legs(
    points: &mut Vec<macroquad::prelude::Vec2>,
    start: u8,
    branch: Option<u8>,
    center: macroquad::prelude::Vec2,
) -> u8 {
    let head = wifi_ring(center, start);
    if let Some(branch) = branch {
        points.push(head);
        points.push(wifi_ring(center, wifi_branch_lane(start, branch)));
    } else {
        for branch in 0..3 {
            points.push(head);
            points.push(wifi_ring(center, wifi_branch_lane(start, branch)));
        }
    }
    wifi_branch_lane(start, 1)
}

fn append_points(points: &mut Vec<macroquad::prelude::Vec2>, part: Vec<macroquad::prelude::Vec2>) {
    for point in part {
        if points.last().is_none_or(|last| last.distance(point) > 1e-4) {
            points.push(point);
        }
    }
}

pub(super) fn from_slide(
    key: &AutoplayKey,
    app: &PadPreviewState,
) -> Result<AutoplayCurve, String> {
    let chart = app.playback_chart();
    let note = chart
        .notes
        .iter()
        .find(|n| n.id == key.note_id)
        .ok_or("Slide note no longer exists")?;
    let slide = note.slide.get(key.slide).ok_or("Slide no longer exists")?;
    let svg = app.pad_svg.as_ref().ok_or("Pad geometry is not loaded")?;
    let engine = app.judge_engine.as_ref().ok_or("Core is updating")?;
    let spec = engine.chart_spec()?;
    // The core reports one runtime slide per judged part. Counting them tells
    // us whether this owner is a piece of a conslide, which decides if the
    // curve is built from one segment or from the whole chain.
    let mut parts = 0usize;
    let mut runtime = None;
    for (index, candidate) in spec.slides.iter().enumerate() {
        if let Some((id, slide_index, part, _)) = engine.slide_position(chart, index) {
            if id == key.note_id && slide_index == key.slide {
                parts = parts.max(part + 1);
                if part == key.segment {
                    runtime = Some(candidate);
                }
            }
        }
    }
    let runtime = runtime.ok_or("Slide timing is not ready")?;
    let pad = canonical_pad();
    let center = svg
        .pad_visual_center(&pad)
        .unwrap_or(macroquad::prelude::Vec2::ZERO);
    let mut source = note.clone();
    let mut shape = slide.clone();
    if parts > 1 {
        let starts = crate::app::slide_render::segment_start_lanes(note, slide);
        source.lane = *starts
            .get(key.segment)
            .ok_or("Slide segment no longer exists")?;
        shape.segments = vec![
            slide
                .segments
                .get(key.segment)
                .ok_or("Slide segment no longer exists")?
                .clone(),
        ];
        shape.runtime_parts = 1;
    }
    // Wi-Fi is straight legs from the head's ring point. A per-track owner
    // keeps its own branch; a combined owner (the Lean core reports all three
    // branches in one body) exposes the whole fan so the group stays editable.
    let branch = key.track.filter(|b| *b < 3).map(|b| b as u8);
    let points = if shape.segments.iter().any(|s| s.shape == SlideShape::Wifi) {
        let mut points = Vec::new();
        let mut lane = source.lane;
        for segment in &shape.segments {
            if segment.shape == SlideShape::Wifi {
                lane = append_wifi_legs(&mut points, lane, branch, center);
                continue;
            }
            let mut one = source.clone();
            one.lane = lane;
            let mut single = shape.clone();
            single.segments = vec![segment.clone()];
            single.runtime_parts = 1;
            append_points(
                &mut points,
                crate::app::slide_render::build_slide_path(
                    &one, &single, &pad, svg, 1.0, center, RADIUS,
                ),
            );
            lane = segment
                .points
                .last()
                .map(|p| p.zone.to_id())
                .unwrap_or(lane);
        }
        points
    } else {
        crate::app::slide_render::build_slide_path(&source, &shape, &pad, svg, 1.0, center, RADIUS)
    };
    let mut curve = AutoplayCurve {
        smooth: false,
        start: (runtime.start_timing - (note_secs(note, &chart.bpms) * 1e6).round() as i64) as f64
            / 1e6,
        duration: runtime.length as f64 / 1e6,
        timing: CurveTiming::FixedDuration,
        points: points
            .into_iter()
            .map(|p| [p.x / RADIUS, p.y / RADIUS])
            .collect(),
        ..Default::default()
    };
    // Remove redundant straight-line samples without moving corners/endpoints.
    let mut reduced: Vec<[f32; 2]> = Vec::new();
    for point in curve.points {
        while reduced.len() >= 2 {
            let a = Pos2::from(reduced[reduced.len() - 2]);
            let b = Pos2::from(reduced[reduced.len() - 1]);
            let c = Pos2::from(point);
            if segment_distance(b, a, c) > 0.0001 {
                break;
            }
            reduced.pop();
        }
        reduced.push(point);
    }
    curve.points = reduced;
    let path = Path::new(&curve);
    curve.speed = path.length as f64 / curve.duration.max(0.001);
    if curve.points.len() < 2 {
        return Err("Slide has no editable path".into());
    }
    Ok(curve)
}
