//! Source-sampled slide result poses, independent of the local trail renderer.

use std::{collections::HashMap, sync::OnceLock};

use macroquad::math::{Vec2, vec2};
use serde::Deserialize;

use crate::app::{
    slide_render, slide_svg,
    types::{Note, Slide, SlideShape},
};

pub const DURATION: f64 = 1.0 / 3.0;
const REFERENCE_JSON: &str = include_str!(
    "../../../assets/extracted/lambdadx_replay_1600_master/slide_judgment_reference.json"
);

#[derive(Debug, Deserialize, PartialEq)]
pub struct Pose {
    pub key: String,
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub corners: [f32; 8],
}

impl Pose {
    pub fn screen(&self, center: Vec2, radius: f32) -> (Vec2, Vec2, f32) {
        let unit = radius / 480.0;
        (
            center + vec2(self.x, -self.y) * unit,
            Vec2::splat(unit),
            -self.rotation.to_radians(),
        )
    }
}

#[derive(Deserialize)]
struct Reference {
    poses: HashMap<String, Pose>,
}

fn poses() -> &'static HashMap<String, Pose> {
    static REFERENCE: OnceLock<Reference> = OnceLock::new();
    &REFERENCE
        .get_or_init(|| {
            serde_json::from_str(REFERENCE_JSON).expect("bundled slide judgment poses must parse")
        })
        .poses
}

pub fn resolve(note: &Note, slide: &Slide) -> Option<&'static Pose> {
    let rendered = slide_render::render_slide_legs(slide);
    let segment = rendered.segments.last()?;
    let start = if rendered.segments.len() > 1 {
        rendered.segments[rendered.segments.len() - 2]
            .points
            .last()?
            .zone
            .to_id()
    } else {
        note.lane
    };
    let end = segment.points.last()?.zone.to_id();
    let turn = segment.points.first()?.zone.to_id();
    let prefab = slide_svg::prefab_key(segment.shape, start, end, turn)?;
    // Local </>/^ depend on the starting half; source Circle_L/R are absolute.
    let source_type = match segment.shape {
        SlideShape::Line => 1,
        SlideShape::Left | SlideShape::Right | SlideShape::Caret => {
            if prefab.mirror {
                2
            } else {
                3
            }
        }
        SlideShape::P => 4,
        SlideShape::Q => 5,
        SlideShape::S => 6,
        SlideShape::Z => 7,
        SlideShape::VShape => 8,
        SlideShape::PP => 9,
        SlideShape::QQ => 10,
        SlideShape::BigV => {
            if prefab.mirror {
                12
            } else {
                11
            }
        }
        SlideShape::Wifi => 13,
    };
    poses().get(&format!("{source_type}/{start}/{end}"))
}

pub fn animation(age: f64) -> (f32, bool) {
    if !(0.0..DURATION).contains(&age) {
        return (0.0, false);
    }
    let ms = age * 1000.0;
    let alpha = if ms < 83.33 {
        ms / 83.33
    } else if ms < 250.0 {
        1.0
    } else {
        1.0 - (ms - 250.0) / 83.33
    };
    (
        alpha.clamp(0.0, 1.0) as f32,
        (ms / 33.333).floor() as u32 % 2 == 1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::{SlidePoint, SlideSegment, zone::PadZone};

    fn slide(shape: SlideShape, end: u8) -> Slide {
        Slide {
            segments: vec![SlideSegment {
                shape,
                points: vec![SlidePoint::from(PadZone::from(end))],
            }],
            slide_duration: 1.0,
            slide_start_delay: 0.0,
            slide_is_break: false,
            runtime_parts: 1,
        }
    }

    #[test]
    fn all_576_source_poses_resolve_to_the_correct_local_shape() {
        assert_eq!(poses().len(), 576);
        for (key, expected) in poses() {
            let parts: Vec<u8> = key.split('/').map(|p| p.parse().unwrap()).collect();
            let (kind, start, end) = (parts[0], parts[1], parts[2]);
            let upper = matches!(start, 1 | 2 | 7 | 8);
            let shape = match kind {
                1 => SlideShape::Line,
                2 => {
                    if upper {
                        SlideShape::Left
                    } else {
                        SlideShape::Right
                    }
                }
                3 => {
                    if upper {
                        SlideShape::Right
                    } else {
                        SlideShape::Left
                    }
                }
                4 => SlideShape::P,
                5 => SlideShape::Q,
                6 => SlideShape::S,
                7 => SlideShape::Z,
                8 => SlideShape::VShape,
                9 => SlideShape::PP,
                10 => SlideShape::QQ,
                11 | 12 => SlideShape::BigV,
                13 => SlideShape::Wifi,
                _ => unreachable!(),
            };
            let mut sl = slide(shape, end);
            if matches!(kind, 11 | 12) {
                let turn = (start - 1 + if kind == 11 { 6 } else { 2 }) % 8 + 1;
                sl.segments[0]
                    .points
                    .insert(0, SlidePoint::from(PadZone::from(turn)));
            }
            let note = Note {
                lane: start,
                ..Default::default()
            };
            assert_eq!(resolve(&note, &sl), Some(expected), "{key}");
            let (anchor, size, rotation) = expected.screen(vec2(540.0, 540.0), 480.0);
            assert_eq!(anchor, vec2(540.0 + expected.x, 540.0 - expected.y));
            assert_eq!(size, Vec2::ONE);
            assert_eq!(rotation, -expected.rotation.to_radians());
        }
    }

    #[test]
    fn animation_matches_samples_from_the_source_draw_method() {
        let reference: serde_json::Value = serde_json::from_str(REFERENCE_JSON).unwrap();
        for sample in reference["animation"].as_array().unwrap() {
            let (alpha, additive) = animation(sample["ms"].as_f64().unwrap() / 1000.0);
            assert!(
                (alpha as f64 - sample["alpha"].as_f64().unwrap()).abs() < 0.00001,
                "{sample}"
            );
            assert_eq!(additive, sample["additive"].as_bool().unwrap(), "{sample}");
        }
        assert_eq!(animation(-0.1), (0.0, false));
        assert_eq!(animation(0.34), (0.0, false));
    }

    #[test]
    fn chains_and_freeform_lines_use_the_final_leg_start() {
        let note = Note {
            lane: 1,
            ..Default::default()
        };
        let mut chained = slide(SlideShape::Line, 5);
        chained
            .segments
            .push(slide(SlideShape::Left, 7).segments.remove(0));
        assert_eq!(resolve(&note, &chained), poses().get("3/5/7"));
        let mut freeform = slide(SlideShape::Line, 5);
        freeform.segments[0]
            .points
            .push(SlidePoint::from(PadZone::A1));
        assert_eq!(resolve(&note, &freeform), poses().get("1/5/1"));
        assert!(resolve(&note, &slide(SlideShape::Line, 2)).is_none());
        assert!(resolve(&note, &slide(SlideShape::Wifi, 3)).is_none());
    }
}
