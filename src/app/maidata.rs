//! Adapt lnmai-core's parsed Simai charts for the preview renderer.

use lnmai_core::types::{CanonicalSlideShape, FrontendChartResult, NormalizedSlide, Rational};
use lnmai_core::{api, session};

use crate::app::maichart::{mark_double_stars, recompute_each};
use crate::app::types::zone::PadZone;
use crate::app::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
    secs_to_measure,
};

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.trim().trim_start_matches('\u{feff}').strip_prefix(key))
}

pub fn levels(text: &str) -> Vec<(u32, String)> {
    let mut keys: Vec<u32> = text
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("&inote_")?
                .split_once('=')?
                .0
                .parse()
                .ok()
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter()
        .map(|key| {
            (
                key,
                field(text, &format!("&lv_{key}="))
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            )
        })
        .collect()
}

pub fn inote_key(text: &str, diff: Option<i32>) -> Option<u32> {
    let keys = levels(text);
    let index = match diff {
        Some(value) if value >= 1 => (value as usize - 1).min(keys.len().saturating_sub(1)),
        _ => keys.len().saturating_sub(1),
    };
    keys.get(index).map(|(key, _)| *key)
}

pub fn from_maidata(text: &str, diff: Option<i32>) -> Result<ChartDoc, String> {
    let key = inote_key(text, diff).ok_or("maidata has no &inote_N chart")?;
    from_maidata_key(text, key)
}

pub fn from_maidata_key(text: &str, key: u32) -> Result<ChartDoc, String> {
    session::ensure_runtime().map_err(|_| "lnmai-core initialization failed")?;
    let parsed = api::parse_frontend_chart(text, key).map_err(|error| error.json)?;
    Ok(convert(text, parsed, key))
}

fn rational(value: &Rational) -> f32 {
    value.num as f32 / value.den as f32
}

fn secs(time: i64) -> f32 {
    time as f32 / 1_000_000.0
}

fn convert(text: &str, parsed: FrontendChartResult, key: u32) -> ChartDoc {
    let metadata = &parsed.inspection.metadata.fields;
    let meta = |name: &str| {
        metadata
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.as_str())
    };
    let bpm = parsed
        .inspection
        .source
        .events
        .first()
        .map(|event| rational(&event.bpm))
        .or_else(|| meta("&wholebpm").and_then(|value| value.parse().ok()))
        .filter(|value| *value > 0.0)
        .unwrap_or(120.0);
    let mut bpms = vec![BpmChange { measure: 1.0, bpm }];
    for event in &parsed.inspection.source.events {
        let next_bpm = rational(&event.bpm);
        if next_bpm > 0.0 && (next_bpm - bpms.last().unwrap().bpm).abs() > 0.001 {
            bpms.push(BpmChange {
                measure: secs_to_measure(secs(event.timing), &bpms),
                bpm: next_bpm,
            });
        }
    }
    let measure = |time| secs_to_measure(secs(time), &bpms);
    let duration = |start, length| measure(start + length) - measure(start);
    let normalized = parsed.semantic.normalized;
    let mut notes = Vec::new();
    for tap in normalized.taps {
        notes.push(Note {
            id: tap.note_index,
            time: measure(tap.timing),
            lane: tap.slot as u8 + 1,
            note_type: NoteType::Tap,
            is_break: tap.is_break,
            is_ex: tap.is_ex,
            is_star: tap.is_force_star,
            ..Default::default()
        });
    }
    for hold in normalized.holds {
        notes.push(Note {
            id: hold.note_index,
            time: measure(hold.timing),
            lane: hold.slot as u8 + 1,
            note_type: NoteType::Hold,
            hold_duration: duration(hold.timing, hold.length),
            is_break: hold.is_break,
            is_ex: hold.is_ex,
            ..Default::default()
        });
    }
    for touch in normalized.touches {
        notes.push(Note {
            id: touch.note_index,
            time: measure(touch.timing),
            lane: touch.sensor_pos as u8 + 1,
            note_type: NoteType::Touch,
            is_break: touch.is_break,
            ..Default::default()
        });
    }
    for hold in normalized.touch_holds {
        notes.push(Note {
            id: hold.note_index,
            time: measure(hold.timing),
            lane: hold.sensor_pos as u8 + 1,
            note_type: NoteType::Hold,
            hold_duration: duration(hold.timing, hold.length),
            is_break: hold.is_break,
            is_ex: hold.is_ex,
            ..Default::default()
        });
    }
    let mut slide_heads = std::collections::HashMap::new();
    for slide in normalized.slides {
        if !slide.has_body {
            continue;
        }
        let head = slide.head_timing;
        let start = slide.start_timing;
        let end = start + slide.length;
        let segment = slide_segment(&slide);
        let part = Slide {
            segments: vec![segment],
            slide_duration: duration(head, end - head),
            slide_start_delay: duration(head, start - head),
            slide_is_break: slide.is_break || slide.is_slide_break,
            connected_from: slide.is_conn_slide.then_some(slide.slot as u8 + 1),
        };
        let head_key = (head, slide.slot as u8);
        if let Some(&index) = slide_heads.get(&head_key) {
            let note: &mut Note = &mut notes[index];
            note.slide.push(part);
            note.is_tapless &= !slide.has_head_note;
            continue;
        }
        slide_heads.insert(head_key, notes.len());
        notes.push(Note {
            id: slide.note_index,
            time: measure(head),
            lane: slide.slot as u8 + 1,
            note_type: NoteType::Slide,
            is_break: slide.is_break || slide.is_slide_break,
            is_ex: slide.is_ex,
            is_tapless: !slide.has_head_note,
            hi_speed: rational(&slide.h_speed),
            slide: vec![part],
            ..Default::default()
        });
    }
    notes.sort_by(|left, right| {
        left.time
            .total_cmp(&right.time)
            .then(left.id.cmp(&right.id))
    });
    mark_double_stars(&mut notes);
    recompute_each(&mut notes);
    ChartDoc {
        version: "simai-core-1".to_string(),
        title: meta("&title")
            .or_else(|| field(&text, "&title="))
            .unwrap_or("")
            .to_string(),
        artist: meta("&artist")
            .or_else(|| field(&text, "&artist="))
            .unwrap_or("")
            .to_string(),
        simai_level: meta(&format!("&lv_{key}"))
            .and_then(|level| level.parse::<f32>().ok())
            .unwrap_or(0.0) as u32,
        bpm,
        bpms,
        audio_offset: meta("&first")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.0),
        notes,
        templates: Vec::new(),
        template_instances: Vec::new(),
    }
}

fn slide_segment(slide: &NormalizedSlide) -> SlideSegment {
    let start = slide.slot as u8 + 1;
    let mirrored = slide.simai_shape.symmetry.mirrored;
    let (kind, relative_end) = match slide.simai_shape.canonical {
        CanonicalSlideShape::Line { rel_end } => (SlideShape::Line, rel_end),
        CanonicalSlideShape::Circle { rel_end } => (
            if mirrored != (3..=6).contains(&start) {
                SlideShape::Left
            } else {
                SlideShape::Right
            },
            rel_end,
        ),
        CanonicalSlideShape::V { rel_end } => (SlideShape::VShape, rel_end),
        CanonicalSlideShape::Turn { rel_end } => (SlideShape::BigV, rel_end),
        CanonicalSlideShape::Pq { rel_end } => (
            if mirrored {
                SlideShape::Q
            } else {
                SlideShape::P
            },
            rel_end,
        ),
        CanonicalSlideShape::Ppqq { rel_end } => (
            if mirrored {
                SlideShape::QQ
            } else {
                SlideShape::PP
            },
            rel_end,
        ),
        CanonicalSlideShape::S => (
            if mirrored {
                SlideShape::Z
            } else {
                SlideShape::S
            },
            5,
        ),
        CanonicalSlideShape::Wifi => (SlideShape::Wifi, 5),
    };
    let end = (start as u64 + relative_end - 2) % 8 + 1;
    let mut points = Vec::new();
    if matches!(kind, SlideShape::VShape) {
        points.push(SlidePoint::from(PadZone::from(17)));
    } else if matches!(kind, SlideShape::BigV) {
        let turn = if mirrored { 3 } else { 7 };
        points.push(SlidePoint::from(PadZone::from(
            ((start + turn - 2) % 8) + 1,
        )));
    }
    points.push(SlidePoint::from(PadZone::from(end as u8)));
    SlideSegment {
        points,
        shape: kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use macroquad::prelude::vec2;

    use crate::app::pad_svg::PadSvgDef;
    use crate::app::slide_render::build_slide_path;
    use crate::app::types::PadGeom;

    #[test]
    fn circle_paths_follow_core_judge_order() {
        let text = "&inote_1=(120){4}1>3[4:1],1<3[4:1],5>7[4:1],5<7[4:1],3^1[4:1],1^7[4:1],3>1[4:1],3<1[4:1],2>4[4:1],7<1[4:1],E";
        session::ensure_runtime().unwrap();
        let parsed = api::parse_frontend_chart(text, 1).expect("core chart");
        let chart = from_maidata(text, None).expect("core chart");
        let svg = PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")).unwrap();
        let pad = PadGeom {
            cx: 400.0,
            cy: 400.0,
            outer_r: 300.0,
        };
        let spawn = vec2(400.0, 400.0);
        let slides: Vec<_> = chart
            .notes
            .iter()
            .filter(|note| note.note_type == NoteType::Slide)
            .collect();
        assert_eq!(slides.len(), parsed.semantic.normalized.slides.len());
        for (index, (note, core_slide)) in slides
            .iter()
            .copied()
            .zip(&parsed.semantic.normalized.slides)
            .enumerate()
        {
            let path = build_slide_path(note, &note.slide[0], &pad, &svg, 1.0, spawn, 300.0);
            let core_areas: Vec<_> = core_slide.judge_queues[0]
                .iter()
                .filter_map(|step| step.target_areas.first().copied())
                .collect();
            let near = core_areas[1] as u8 + 1;
            let forward = note.lane % 8 + 1;
            let backward = (note.lane + 6) % 8 + 1;
            assert!(near == forward || near == backward);
            let far = if near == forward { backward } else { forward };
            let point = |lane: u8| {
                let angle = -std::f32::consts::FRAC_PI_2
                    + crate::app::types::PAD_ROTATION_RAD
                    + (lane - 1) as f32 * std::f32::consts::TAU / 8.0;
                spawn + vec2(angle.cos(), angle.sin()) * 300.0
            };
            let first_visit = |lane| {
                path.iter()
                    .position(|sample| sample.distance(point(lane)) < 30.0)
            };
            let expected = first_visit(near)
                .unwrap_or_else(|| panic!("circle {index} misses expected A{near}"));
            let opposite = first_visit(far);
            assert!(
                opposite.is_none_or(|other| expected < other),
                "circle {index} follows opposite A arc: {:?}",
                note.slide[0].segments[0].shape
            );
        }
    }

    #[test]
    fn parses_trailing_break_in_core() {
        let text = "&title=T\n&lv_5=12.7\n&inote_5=(188){4}6pp4[8:5]b,1-5[8:1]b,E\n";
        let chart = from_maidata(text, None).expect("core accepts trailing slide break");
        let slides: Vec<_> = chart
            .notes
            .iter()
            .filter(|note| note.note_type == NoteType::Slide)
            .collect();
        assert_eq!(slides.len(), 2);
        assert_eq!(chart.title, "T");
        assert_eq!(chart.simai_level, 12);
    }

    #[test]
    fn parses_official_chart_when_supplied() {
        let Ok(path) = std::env::var("MAI2_11951_CHART") else {
            return;
        };
        let text = std::fs::read_to_string(path).expect("official maidata");
        for key in [2, 3, 4, 5] {
            let chart = from_maidata_key(&text, key).expect("core parses official difficulty");
            assert_eq!(chart.title, "サイエンス");
            assert!(!chart.notes.is_empty());
        }
    }
}
