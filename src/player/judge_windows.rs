//! Read-only visualization of LnmaiCore/Judge.lean (microsecond thresholds).

use crate::app::types::{Note, note_is_touch};
use std::collections::HashMap;

pub const FRAME: i64 = 16_667;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub early: f32,
    pub late: f32,
}

impl Interval {
    pub fn micros(early: i64, late: i64) -> Self {
        Self {
            early: early as f32 / 1e6,
            late: late as f32 / 1e6,
        }
    }
}

/// Outer to inner: Good, Great, Perfect, Critical Perfect.
pub type Windows = [Interval; 4];
/// Slide Good has no finite early grade boundary; show terminal grade windows.
pub type SlideWindows = [Interval; 3];

pub const COLORS: [[u8; 3]; 4] = [
    [100, 174, 255],
    [241, 124, 182],
    [255, 205, 77],
    [255, 244, 175],
];
pub const HOLD_COLOR: [u8; 3] = [94, 224, 173];

pub fn color(index: usize, alpha: u8) -> macroquad::color::Color {
    let [r, g, b] = COLORS[index];
    macroquad::color::Color::from_rgba(r, g, b, alpha)
}

pub fn classic_hold_tail() -> Interval {
    // Classic release is binary: Perfect inside this strict interval, Good
    // outside. It has no independent Great or subdivided Perfect windows.
    Interval::micros(-9 * FRAME + 1, 12 * FRAME - 1)
}

pub fn head(note: &Note) -> Windows {
    let touch = note_is_touch(note);
    if touch {
        // Input eligibility starts at -150 ms (three microseconds later than 9f).
        [
            Interval::micros(-150_000, 18 * FRAME),
            Interval::micros(-150_000, 15 * FRAME),
            Interval::micros(-150_000, 12 * FRAME),
            Interval::micros(-150_000, 9 * FRAME),
        ]
    } else if note.is_ex {
        [Interval::micros(-150_000, 150_000); 4]
    } else {
        [
            Interval::micros(-150_000, 150_000),
            Interval::micros(-6 * FRAME, 6 * FRAME),
            Interval::micros(-3 * FRAME, 3 * FRAME),
            Interval::micros(-FRAME, FRAME),
        ]
    }
}

pub fn slide(stay: i64, classic: bool) -> SlideWindows {
    let perfect = if classic {
        12 * FRAME
    } else {
        14 * FRAME + (stay.max(0) / 4).min(22 * FRAME)
    };
    let great = (if classic { 24 * FRAME } else { 29 * FRAME }).max(perfect);
    [
        Interval::micros(-great, great),
        // The default core corrects all slide Perfect subgrades to Critical Perfect.
        Interval::micros(-perfect, perfect),
        Interval::micros(-perfect, perfect),
    ]
}

/// Clip by arc length, matching the flying star rather than vertex count.
pub fn path_interval(
    path: &[macroquad::math::Vec2],
    from: f32,
    to: f32,
) -> Vec<macroquad::math::Vec2> {
    if path.len() < 2 || !from.is_finite() || !to.is_finite() {
        return Vec::new();
    }
    let total: f32 = path.windows(2).map(|p| p[0].distance(p[1])).sum();
    if total <= f32::EPSILON {
        return Vec::new();
    }
    let lo = from.clamp(0.0, 1.0) * total;
    let hi = to.clamp(0.0, 1.0) * total;
    if hi <= lo {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut at = 0.0;
    for pair in path.windows(2) {
        let length = pair[0].distance(pair[1]);
        if length > f32::EPSILON && at + length > lo && at < hi {
            let a = pair[0].lerp(pair[1], ((lo - at) / length).clamp(0.0, 1.0));
            let b = pair[0].lerp(pair[1], ((hi - at) / length).clamp(0.0, 1.0));
            if out.last().is_none_or(|last| *last != a) {
                out.push(a);
            }
            out.push(b);
        }
        at += length;
        if at >= hi {
            break;
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldRequirement {
    pub body_start: f32,
    pub body_end: f32,
    /// Earliest release for a continuous on-time head press to retain Perfect.
    pub perfect_release: f32,
}

pub fn hold_requirement(head: f32, length: f32, touch: bool) -> Option<HoldRequirement> {
    let length = (length.max(0.0) as f64 * 1e6).round() as i64;
    let head_ignore = if touch { 15 * FRAME } else { 6 * FRAME };
    let ignore = head_ignore + 12 * FRAME;
    if length <= ignore {
        return None;
    }
    let reality = (length - ignore).min(length - 300_000).max(0);
    if reality == 0 {
        return None;
    }
    let end = length - 12 * FRAME;
    // Release grace cannot always be deducted: the core charges it once the
    // grace expires. Integer rounding preserves its >=67% threshold.
    let release = end - reality * 33 / 100;
    Some(HoldRequirement {
        body_start: head + head_ignore as f32 / 1e6,
        body_end: head + end as f32 / 1e6,
        perfect_release: head + release as f32 / 1e6,
    })
}

#[derive(Debug, Clone)]
pub struct SlideWindow {
    pub note_id: u64,
    pub sub: usize,
    pub start: f32,
    pub end: f32,
    pub judge: f32,
    pub deadline: f32,
    pub windows: SlideWindows,
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub slides: HashMap<(u64, usize), SlideWindow>,
    pub classic_holds: HashMap<u64, bool>,
}

impl Metadata {
    pub fn from_spec(
        spec: &crate::core::types::ChartSpec,
        note_id: impl Fn(u64) -> Option<u64>,
        slide_position: impl Fn(u64) -> Option<(u64, usize)>,
    ) -> Self {
        let mut metadata = Self::default();
        for hold in &spec.holds {
            if let Some(id) = note_id(hold.note_index) {
                metadata
                    .classic_holds
                    .insert(id, hold.is_classic.unwrap_or(false));
            }
        }
        let mut starts: HashMap<(u64, usize), i64> = HashMap::new();
        for part in &spec.slides {
            if let Some(key) = slide_position(part.note_index) {
                starts
                    .entry(key)
                    .and_modify(|t| *t = (*t).min(part.start_timing))
                    .or_insert(part.start_timing);
            }
        }
        for part in &spec.slides {
            if part.is_conn_slide && !part.is_group_end {
                continue;
            }
            let Some((id, sub)) = slide_position(part.note_index) else {
                continue;
            };
            let end = part.start_timing + part.length;
            let judge = part.judge_at.unwrap_or(end);
            let mut windows = slide(end - judge, part.is_classic);
            for range in &mut windows {
                range.early = range
                    .early
                    .max((part.head_timing - 50_000 - judge) as f32 / 1e6);
            }
            metadata.slides.insert(
                (id, sub),
                SlideWindow {
                    note_id: id,
                    sub,
                    start: starts[&(id, sub)] as f32 / 1e6,
                    end: end as f32 / 1e6,
                    judge: judge as f32 / 1e6,
                    deadline: (end + 36 * FRAME) as f32 / 1e6,
                    windows,
                },
            );
        }
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_windows_use_distance_and_clip_to_motion() {
        use macroquad::math::vec2;
        let path = [vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(10.0, 0.0)];
        assert_eq!(
            path_interval(&path, 0.2, 0.8),
            vec![vec2(2.0, 0.0), vec2(8.0, 0.0)]
        );
        assert_eq!(path_interval(&path, -1.0, 2.0), path);
        assert!(path_interval(&[], 0.0, 1.0).is_empty());
        assert!(path_interval(&[path[0]; 2], 0.0, 1.0).is_empty());
        assert!(path_interval(&path, 0.8, 0.2).is_empty());
        assert!(path_interval(&path, f32::NAN, 1.0).is_empty());
    }
    #[test]
    fn touch_windows_are_asymmetric() {
        let chart = crate::app::maidata::from_maidata("&inote_1=(120){4}B1,E", None).unwrap();
        let w = head(&chart.notes[0]);
        assert_eq!(w[0], Interval::micros(-150_000, 18 * FRAME));
        assert_eq!(w[3], Interval::micros(-150_000, 9 * FRAME));
    }
    #[test]
    fn ex_promotes_ring_heads_but_not_touch_heads() {
        let mut chart = crate::app::maidata::from_maidata("&inote_1=(120){4}1/B1,E", None).unwrap();
        let ring = &mut chart.notes[0];
        assert_eq!(head(ring)[3], Interval::micros(-FRAME, FRAME));
        ring.is_ex = true;
        assert_eq!(head(ring), [Interval::micros(-150_000, 150_000); 4]);
        let touch = &mut chart.notes[1];
        touch.is_ex = true;
        assert_eq!(head(touch)[3], Interval::micros(-150_000, 9 * FRAME));
    }
    #[test]
    fn short_holds_do_not_invent_a_body_requirement() {
        assert!(hold_requirement(1.0, 0.3, false).is_none());
        assert!(hold_requirement(1.0, 0.4, true).is_none());
    }
    #[test]
    fn perfect_hold_release_uses_charged_body_and_67_percent() {
        let h = hold_requirement(1.0, 2.0, false).unwrap();
        assert!((h.body_start - 1.100002).abs() < 1e-6);
        let reality = 2_000_000 - 18 * FRAME;
        let release = (2_000_000 - 12 * FRAME) - reality * 33 / 100;
        assert!((h.perfect_release - (1.0 + release as f32 / 1e6)).abs() < 1e-6);
    }
    #[test]
    fn slide_extension_is_bounded_and_classic_is_fixed() {
        assert_eq!(
            slide(0, false)[1],
            Interval::micros(-14 * FRAME, 14 * FRAME)
        );
        assert_eq!(
            slide(10_000_000, false)[1],
            Interval::micros(-36 * FRAME, 36 * FRAME)
        );
        assert_eq!(
            slide(10_000_000, true)[1],
            Interval::micros(-12 * FRAME, 12 * FRAME)
        );
        let extended = slide(10_000_000, false);
        assert_eq!(extended[0], extended[1]);
    }

    #[test]
    fn classic_hold_release_has_strict_boundaries() {
        assert_eq!(
            classic_hold_tail(),
            Interval::micros(-9 * FRAME + 1, 12 * FRAME - 1)
        );
    }

    #[test]
    fn touch_hold_has_a_longer_head_ignore_window() {
        let body = hold_requirement(0.0, 2.0, true).unwrap();
        assert_eq!(body.body_start, (15 * FRAME) as f32 / 1e6);
        assert_eq!(body.body_end, (2_000_000 - 12 * FRAME) as f32 / 1e6);
        let reality = 2_000_000 - 27 * FRAME;
        let released = (2_000_000 - 12 * FRAME) - reality * 33 / 100;
        assert_eq!(body.perfect_release, released as f32 / 1e6);
    }

    #[test]
    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    fn metadata_uses_only_chain_terminals_and_editor_ids() {
        let source = "&inote_1=(120){4}1-5-8[4:2]*-7[4:1]/3h[4:2]/B2h[4:2],E";
        let mut chart = crate::app::maidata::from_maidata(source, None).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        let mut engine = crate::player::engine::JudgeEngine::load(source, 1).unwrap();
        let mut spec = engine.chart_spec().unwrap();
        assert_eq!(spec.slides.len(), 3);
        let positions: HashMap<_, _> = spec
            .slides
            .iter()
            .map(|part| {
                let runtime = engine.runtime_slide_index(part.note_index).unwrap();
                let (id, sub, part_index, count) = engine.slide_position(&chart, runtime).unwrap();
                (part.note_index, (id + 1000, sub, part_index, count))
            })
            .collect();
        let mut ids: HashMap<_, _> = spec
            .taps
            .iter()
            .map(|n| n.note_index)
            .chain(spec.holds.iter().map(|n| n.note_index))
            .chain(spec.touches.iter().map(|n| n.note_index))
            .chain(spec.touch_holds.iter().map(|n| n.note_index))
            .map(|index| (index, index + 1000))
            .collect();
        ids.extend(positions.iter().map(|(&index, &(id, _, _, _))| (index, id)));
        for n in &mut chart.notes {
            n.id += 1000;
        }
        engine.bind_editor_chart(ids, positions).unwrap();
        let metadata = engine.judgment_metadata(&chart);
        assert_eq!(
            metadata.slides.len(),
            2,
            "one window per complete sub-slide"
        );
        for note in chart.notes.iter().filter(|n| !n.slide.is_empty()) {
            for sub in 0..note.slide.len() {
                let arcs: Vec<_> = spec
                    .slides
                    .iter()
                    .filter(|arc| {
                        engine
                            .runtime_slide_index(arc.note_index)
                            .and_then(|i| engine.slide_position(&chart, i))
                            .is_some_and(|(id, s, _, _)| id == note.id && s == sub)
                    })
                    .collect();
                let tail = arcs
                    .iter()
                    .find(|a| !a.is_conn_slide || a.is_group_end)
                    .unwrap();
                let w = &metadata.slides[&(note.id, sub)];
                assert_eq!(
                    w.start,
                    arcs.iter().map(|a| a.start_timing).min().unwrap() as f32 / 1e6
                );
                assert_eq!(w.end, (tail.start_timing + tail.length) as f32 / 1e6);
                assert_eq!(w.judge, tail.judge_at.unwrap() as f32 / 1e6);
                assert_eq!(
                    w.deadline,
                    (tail.start_timing + tail.length + 36 * FRAME) as f32 / 1e6
                );
                assert!(w.windows.iter().all(|range| {
                    w.judge + range.early >= tail.head_timing as f32 / 1e6 - 0.050001
                }));
                let fraction = (w.judge - w.start) / (w.end - w.start);
                assert!((0.0..=1.0).contains(&fraction));
                let path = [
                    macroquad::math::vec2(0.0, 0.0),
                    macroquad::math::vec2(100.0, 0.0),
                ];
                let portion = path_interval(&path, fraction, 1.0);
                assert!((portion[0].x - fraction * 100.0).abs() < 1e-4);
            }
        }
        let hold = &mut spec.holds[0];
        hold.is_classic = Some(true);
        let index = hold.note_index;
        let classic = Metadata::from_spec(&spec, |i| Some(i + 1000), |_| None);
        assert_eq!(classic.classic_holds.get(&(index + 1000)), Some(&true));
    }
}
