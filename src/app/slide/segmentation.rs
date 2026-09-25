use crate::app::pad_svg::PadSvgDef;
use crate::app::types::PadGeom;
use crate::app::types::zone::PadZone;
use macroquad::prelude::Vec2;

/// A single drawable trail bar sampled from the slide path.
#[derive(Debug, Clone, Copy)]
pub struct SlideBar {
    pub position: Vec2,
    pub rotation: f32,
    pub zone: Option<PadZone>,
    /// Distance from the path start. Used to map the star's progress to a bar.
    pub distance_along: f32,
}

/// A consecutive run of trail bars that belongs to one sensor area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideJudgeSegment {
    pub zone: PadZone,
    pub start_bar: usize,
    pub end_bar: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SlideSegmentation {
    pub bars: Vec<SlideBar>,
    pub judge_segments: Vec<SlideJudgeSegment>,
}

/// Sample the same polyline used for rendering into discrete trail bars and
/// merge consecutive bars that occupy the same playable sensor area.
///
/// `head_gap` / `tail_gap` are the empty gaps (already scaled) at the two ends;
/// bars are sampled from `total - tail_gap` backwards every `spacing`, down to
/// `head_gap`. Anchoring to the tail keeps two slides that share a tail aligned.
pub fn build(
    path: &[Vec2],
    spacing: f32,
    head_gap: f32,
    tail_gap: f32,
    svg: &PadSvgDef,
    pad: &PadGeom,
) -> SlideSegmentation {
    let spacing = spacing.max(1.0);

    let lengths: Vec<f32> = path
        .windows(2)
        .map(|window| (window[1] - window[0]).length())
        .collect();
    let total_length: f32 = lengths.iter().sum();
    if total_length <= f32::EPSILON {
        return SlideSegmentation {
            bars: Vec::new(),
            judge_segments: Vec::new(),
        };
    }

    // Sample by cumulative path distance, phase anchored to the tail.
    let first = head_gap.max(0.0);
    let last = (total_length - tail_gap).max(0.0);
    let mut distances: Vec<f32> = Vec::new();
    let mut d = last;
    while d >= first - f32::EPSILON {
        distances.push(d);
        d -= spacing;
    }
    distances.reverse(); // ascending along the path (bar 0 = closest to head)

    let mut bars = Vec::with_capacity(distances.len());
    for distance in distances {
        let mut accumulated = 0.0;
        let mut sample = None;
        for (index, &length) in lengths.iter().enumerate() {
            if length <= f32::EPSILON {
                continue;
            }
            if distance <= accumulated + length || index == lengths.len() - 1 {
                let local = ((distance - accumulated) / length).clamp(0.0, 1.0);
                let start = path[index];
                let delta = path[index + 1] - start;
                let position = start + delta * local;
                sample = Some((position, delta.y.atan2(delta.x) + std::f32::consts::PI));
                break;
            }
            accumulated += length;
        }

        let Some((position, rotation)) = sample else {
            break;
        };
        bars.push(SlideBar {
            position,
            rotation,
            zone: svg.hit_test(position, pad),
            distance_along: distance,
        });
    }

    let mut judge_segments = Vec::new();
    let mut current: Option<(PadZone, usize)> = None;
    for (bar_index, bar) in bars.iter().enumerate() {
        let Some(zone) = bar.zone else {
            if let Some((current_zone, start_bar)) = current.take() {
                judge_segments.push(SlideJudgeSegment {
                    zone: current_zone,
                    start_bar,
                    end_bar: bar_index,
                });
            }
            continue;
        };

        match current {
            Some((current_zone, start_bar)) if current_zone == zone => {
                current = Some((current_zone, start_bar));
            }
            Some((current_zone, start_bar)) => {
                judge_segments.push(SlideJudgeSegment {
                    zone: current_zone,
                    start_bar,
                    end_bar: bar_index,
                });
                current = Some((zone, bar_index));
            }
            None => current = Some((zone, bar_index)),
        }
    }
    if let Some((zone, start_bar)) = current {
        judge_segments.push(SlideJudgeSegment {
            zone,
            start_bar,
            end_bar: bars.len(),
        });
    }

    SlideSegmentation {
        bars,
        judge_segments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::pad_svg::PadSvgDef;
    use crate::app::types::PadGeom;

    // Zone coordinates are relative to the pad origin (`SVG_BG_CX/CY`), so the
    // fixture sits at the origin and the bars pass just above it.
    const SVG: &str = r#"
        <svg><g id="touch">
          <rect id="A1" x="420" y="395" width="10" height="10"/>
          <rect id="A2" x="430" y="395" width="10" height="10"/>
        </g></svg>
    "#;

    #[test]
    fn adjacent_bars_in_one_zone_form_one_judge_segment() {
        let svg = PadSvgDef::from_svg_str(SVG).expect("test SVG");
        let pad = PadGeom {
            cx: 0.0,
            cy: 0.0,
            outer_r: 326.57,
        };
        let result = build(
            &[Vec2::new(-1.0, -7.0), Vec2::new(5.0, -7.0)],
            2.0,
            0.0,
            0.0,
            &svg,
            &pad,
        );

        assert_eq!(result.judge_segments.len(), 1);
        assert_eq!(result.judge_segments[0].zone, PadZone::A1);
        assert_eq!(result.judge_segments[0].start_bar, 0);
        assert_eq!(result.judge_segments[0].end_bar, result.bars.len());
    }
}
