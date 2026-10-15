//! Compile the live document for the core's native Simai loader. Editor IDs
//! remain stable even when the parser renumbers notes after insertion or moves.

use std::collections::HashMap;

use crate::app::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlideShape, mdur_to_secs, note_secs,
};

pub struct CoreChart {
    pub text: String,
    pub note_ids: HashMap<u64, u64>,
    pub slide_positions: HashMap<u64, (u64, usize, usize, usize)>,
}

/// Encode one note without changing the document's measure/BPM clock.
pub fn note_text(note: &Note, bpms: &[BpmChange]) -> Result<String, String> {
    let mut out = CoreChart {
        text: String::new(),
        note_ids: HashMap::new(),
        slide_positions: HashMap::new(),
    };
    emit_note(&mut out, note, bpms, &mut 1)?;
    Ok(out.text)
}

pub fn compile(chart: &ChartDoc, level: u32) -> Result<CoreChart, String> {
    let mut out = CoreChart {
        text: format!("&inote_{level}="),
        note_ids: HashMap::new(),
        slide_positions: HashMap::new(),
    };
    let mut entries = Vec::new();
    for note in &chart.notes {
        let time = note_secs(note, &chart.bpms);
        if !time.is_finite() {
            return Err(format!("note {} has invalid timing", note.id));
        }
        entries.push(((time.max(0.0) * 1e6).round() as i64, note));
    }
    entries.sort_by_key(|(time, _)| *time);
    // Each entry advances to the next absolute microsecond boundary. A local
    // tempo avoids expanding off-grid edits into millions of empty commas;
    // explicit second durations keep holds/slides independent of that tempo.
    if let Some((first, _)) = entries.first().filter(|(time, _)| *time > 0) {
        out.text
            .push_str(&format!("({:.12}){{1}},", 240_000_000.0 / *first as f64));
    }
    let mut next_index = 1;
    let mut cursor = 0;
    while cursor < entries.len() {
        let time = entries[cursor].0;
        let end = cursor + entries[cursor..].partition_point(|(t, _)| *t == time);
        let gap = entries
            .get(end)
            .map(|(t, _)| *t - time)
            .unwrap_or(2_000_000);
        out.text
            .push_str(&format!("({:.12}){{1}}", 240_000_000.0 / gap as f64));
        for (offset, (_, note)) in entries[cursor..end].iter().enumerate() {
            if offset != 0 {
                out.text.push('/');
            }
            emit_note(&mut out, note, &chart.bpms, &mut next_index)?;
        }
        out.text.push(',');
        cursor = end;
    }
    out.text.push_str("E\n");
    Ok(out)
}

fn flags(note: &Note) -> String {
    format!(
        "{}{}{}",
        if note.is_break { "b" } else { "" },
        if note.is_ex { "x" } else { "" },
        if note.is_hanabi { "f" } else { "" }
    )
}

fn touch_zone(lane: u8) -> Result<String, String> {
    Ok(match lane {
        1..=8 => format!("A{lane}"),
        9..=16 => format!("B{}", lane - 8),
        17 => "C".into(),
        18..=25 => format!("D{}", lane - 17),
        26..=33 => format!("E{}", lane - 25),
        _ => return Err(format!("invalid touch zone {lane}")),
    })
}

fn seconds(duration: f32) -> Result<String, String> {
    if !duration.is_finite() || duration < 0.0 {
        return Err("invalid note duration".into());
    }
    Ok(format!("{duration:.6}"))
}

fn emit_note(
    out: &mut CoreChart,
    note: &Note,
    bpms: &[BpmChange],
    index: &mut u64,
) -> Result<(), String> {
    let touch = matches!(note.note_type, NoteType::Touch | NoteType::TouchHold);
    let head = if touch {
        touch_zone(note.lane)?
    } else if (1..=8).contains(&note.lane) {
        note.lane.to_string()
    } else {
        return Err(format!(
            "note {} has invalid ring lane {}",
            note.id, note.lane
        ));
    };
    if note.note_type != NoteType::Slide {
        out.text.push_str(&head);
        out.text.push_str(&flags(note));
        if matches!(note.note_type, NoteType::Hold | NoteType::TouchHold) {
            out.text.push_str(&format!(
                "h[{}]",
                seconds(mdur_to_secs(note.hold_duration, note.time, bpms))?
            ));
        } else if note.is_star {
            out.text.push_str(if note.is_fake_rotate { "$$" } else { "$" });
        }
        out.note_ids.insert(*index, note.id);
        *index += 1;
        return Ok(());
    }
    let mut emitted = false;
    for (sub, slide) in note.slide.iter().enumerate() {
        let (body, parts) = slide_body(slide)?;
        if parts == 0 {
            continue;
        }
        if emitted {
            out.text.push('/');
        }
        out.text.push_str(&head);
        out.text.push_str(&flags(note));
        if note.is_fake_rotate { out.text.push_str("$$"); }
        if note.is_tapless || emitted {
            out.text.push('!');
        }
        out.text.push_str(&body);
        let delay = mdur_to_secs(slide.slide_start_delay, note.time, bpms);
        let travel = mdur_to_secs(slide.slide_duration, note.time, bpms) - delay;
        out.text.push_str(&format!(
            "[{}##{}]{}",
            seconds(delay)?,
            seconds(travel.max(0.000001))?,
            if slide.slide_is_break { "b" } else { "" }
        ));
        for part in 0..parts {
            out.note_ids.insert(*index, note.id);
            out.slide_positions
                .insert(*index, (note.id, sub, part, parts));
            *index += 1;
        }
        emitted = true;
    }
    if !emitted && !note.is_tapless {
        out.text.push_str(&format!("{head}{}{}", flags(note), if note.is_fake_rotate { "$$" } else { "$" }));
        out.note_ids.insert(*index, note.id);
        *index += 1;
    }
    Ok(())
}

fn slide_body(slide: &Slide) -> Result<(String, usize), String> {
    let mut body = String::new();
    let mut parts = 0;
    for segment in &slide.segments {
        let zones: Vec<u8> = segment.points.iter().map(|p| p.zone.to_id()).collect();
        if zones.is_empty() {
            continue;
        }
        let endpoint = *zones.last().unwrap();
        if !(1..=8).contains(&endpoint) {
            // A pending center waypoint has no judgeable trail yet.
            if zones == [17] {
                continue;
            }
            return Err("slide must end at a ring lane".into());
        }
        if matches!(
            segment.shape,
            SlideShape::Line | SlideShape::VShape | SlideShape::BigV
        ) {
            if zones.len() == 2 && zones[0] == 17 {
                body.push_str(&format!("v{endpoint}"));
                parts += 1;
            } else if segment.shape == SlideShape::BigV && zones.len() == 2 {
                body.push_str(&format!("V{}{endpoint}", zones[0]));
                parts += 1;
            } else {
                for zone in zones {
                    if !(1..=8).contains(&zone) {
                        return Err("unfinished center slide waypoint".into());
                    }
                    body.push_str(&format!("-{zone}"));
                    parts += 1;
                }
            }
        } else {
            let symbol = match segment.shape {
                SlideShape::Caret => "^",
                SlideShape::Left => "<",
                SlideShape::Right => ">",
                SlideShape::P => "p",
                SlideShape::Q => "q",
                SlideShape::S => "s",
                SlideShape::Z => "z",
                SlideShape::PP => "pp",
                SlideShape::QQ => "qq",
                SlideShape::Wifi => "w",
                _ => unreachable!(),
            };
            body.push_str(&format!("{symbol}{endpoint}"));
            parts += 1;
        }
    }
    Ok((body, parts))
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;
    use crate::player::engine::JudgeEngine;

    fn load(source: &str) -> (ChartDoc, JudgeEngine) {
        let mut chart =
            crate::app::maidata::from_maidata_level(&format!("&inote_6={source}\n"), 6).unwrap();
        for (i, note) in chart.notes.iter_mut().enumerate() {
            note.id = 100 + i as u64;
        }
        let compiled = compile(&chart, 6).unwrap();
        let mut engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        engine
            .bind_editor_chart(compiled.note_ids, compiled.slide_positions)
            .unwrap();
        (chart, engine)
    }

    #[test]
    fn continuous_slide_keeps_every_arc_and_editor_identity() {
        let (chart, engine) = load("(120){4}3-5-8[4:2],E");
        let spec = engine.chart_spec().unwrap();
        assert_eq!(spec.slides.len(), 2);
        assert_eq!(spec.slide_heads.len(), 1);
        assert_eq!(engine.slide_position(&chart, 0), Some((100, 0, 0, 2)));
        assert_eq!(engine.slide_position(&chart, 1), Some((100, 0, 1, 2)));
    }

    #[test]
    fn zero_duration_holds_remain_holds_in_native_core() {
        let (mut chart, _) = load("(120){4}1h[4:1]/Ch[4:1],E");
        for note in &mut chart.notes {
            note.hold_duration = 0.0;
        }
        let compiled = compile(&chart, 6).unwrap();
        let mut engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        engine
            .bind_editor_chart(compiled.note_ids, compiled.slide_positions)
            .unwrap();
        let spec = engine.chart_spec().unwrap();
        assert_eq!(spec.holds.len(), 1);
        assert_eq!(spec.touch_holds.len(), 1);
        assert_eq!(spec.holds[0].length, 0);
        assert_eq!(spec.touch_holds[0].length, 0);
    }

    #[test]
    fn native_compilation_preserves_note_kinds_and_second_durations() {
        let (chart, engine) = load("(120){4}1/2h[4:2]/B3/Ch[4:1]/4-6[4:2],E");
        let spec = engine.chart_spec().unwrap();
        assert_eq!(spec.taps.len(), 1);
        assert_eq!(spec.holds.len(), 1);
        assert_eq!(spec.touches.len(), 1);
        assert_eq!(spec.touch_holds.len(), 1);
        assert_eq!(spec.slides.len(), 1);
        assert_eq!(spec.holds[0].length, 1_000_000);
        assert_eq!(spec.touch_holds[0].length, 500_000);
        assert_eq!(
            engine.slide_position(&chart, 0).unwrap().0,
            chart.notes[4].id
        );
    }

    #[test]
    fn off_grid_moves_and_bpm_changes_keep_absolute_timings() {
        let mut chart =
            crate::app::maidata::from_maidata_level("&inote_6=(120){4}1,2h[4:2],E", 6).unwrap();
        chart.notes[0].time = 1.12345;
        chart.notes[1].time = 2.34567;
        chart.bpms.push(BpmChange {
            measure: 2.0,
            bpm: 180.0,
        });
        let compiled = compile(&chart, 6).unwrap();
        let engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        let spec = engine.chart_spec().unwrap();
        assert!(
            (spec.taps[0].timing - (note_secs(&chart.notes[0], &chart.bpms) * 1e6).round() as i64)
                .abs()
                <= 1
        );
        assert!(
            (spec.holds[0].timing - (note_secs(&chart.notes[1], &chart.bpms) * 1e6).round() as i64)
                .abs()
                <= 1
        );
        assert!(
            (spec.holds[0].length
                - (mdur_to_secs(
                    chart.notes[1].hold_duration,
                    chart.notes[1].time,
                    &chart.bpms
                ) * 1e6)
                    .round() as i64)
                .abs()
                <= 1
        );
    }

    #[test]
    fn incomplete_native_chart_is_not_installed_as_current() {
        let chart =
            crate::app::maidata::from_maidata_level("&inote_6=(120){4}3-5[4:2],E", 6).unwrap();
        let mut compiled = compile(&chart, 6).unwrap();
        compiled.note_ids.insert(999, 1000);
        let mut engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        assert!(
            engine
                .bind_editor_chart(compiled.note_ids, compiled.slide_positions)
                .is_err()
        );
    }

    #[test]
    fn same_head_subslides_and_tapless_trails_keep_distinct_bindings() {
        let (mut chart, _) = load("(120){4}1-3[4:1]*-5[4:1]/6!-8[4:1],E");
        let second = chart.notes.remove(1);
        chart.notes[0].slide.extend(second.slide);
        let compiled = compile(&chart, 6).unwrap();
        let mut engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        engine
            .bind_editor_chart(compiled.note_ids, compiled.slide_positions)
            .unwrap();
        let spec = engine.chart_spec().unwrap();
        assert_eq!(spec.slides.len(), 3);
        assert_eq!(spec.slide_heads.len(), 1);
        for sub in 0..2 {
            assert_eq!(engine.slide_position(&chart, sub), Some((100, sub, 0, 1)));
        }
        assert_eq!(engine.slide_position(&chart, 2), Some((102, 0, 0, 1)));
    }

    #[test]
    fn pending_star_and_empty_chart_can_be_compiled() {
        let (mut chart, _) = load("(120){4}1-3[4:1],E");
        chart.notes[0].slide.clear();
        let compiled = compile(&chart, 6).unwrap();
        let mut engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        engine
            .bind_editor_chart(compiled.note_ids, compiled.slide_positions)
            .unwrap();
        assert_eq!(engine.chart_spec().unwrap().taps.len(), 1);
        chart.notes.clear();
        let compiled = compile(&chart, 6).unwrap();
        let mut engine = JudgeEngine::load(&compiled.text, 6).unwrap();
        engine
            .bind_editor_chart(compiled.note_ids, compiled.slide_positions)
            .unwrap();
        assert!(engine.chart_spec().unwrap().taps.is_empty());
    }
}
