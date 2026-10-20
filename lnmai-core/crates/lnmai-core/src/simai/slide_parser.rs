//! Simai slide-note parsing entry points.
//!
//! Mirrors `LnmaiCore/Simai/SlideParser.lean`.

use crate::areas::{OuterSlot, SensorArea};
use crate::rat::Rat;
use crate::simai::shape::{
    detect_just_type, parse_end_area_at, parse_slide_body_from_text, parse_start_lane_at,
    solve_slide_shape,
};
use crate::simai::syntax::{
    ParseError, ParsedSlideBody, SlideNoteSemantics, SlideShape, TimingPointSemantics,
};
use crate::time::TimePoint;
use lnmai_core_verify::{
    slide_parser as kernel, slide_parser_sequence as sequence_kernel,
    slide_parser_text as text_kernel,
};

/// `sanitizeSlideText`: drop Simai flag letters that are not part of the shape.
fn sanitize_slide_text(raw_text: &str) -> String {
    let chars: Vec<char> = raw_text.chars().collect();
    text_kernel::sanitize_slide_chars(&chars)
        .into_iter()
        .collect()
}

/// `parseSlideNote`.
pub fn parse_slide_note(
    raw_text: &str,
    start_slot: OuterSlot,
    end_area: SensorArea,
) -> Result<SlideNoteSemantics, ParseError> {
    let sanitized = sanitize_slide_text(raw_text);
    let shape = parse_slide_body_from_text(&sanitized).and_then(|b| solve_slide_shape(&b))?;
    let is_just_right = detect_just_type(&sanitized)?;
    Ok(SlideNoteSemantics {
        raw_text: sanitized,
        start_slot,
        end_area,
        shape,
        is_just_right,
    })
}

/// `parseSlideNoteFromBody`.
pub fn parse_slide_note_from_body(
    raw_text: &str,
    body: &ParsedSlideBody,
    end_area: SensorArea,
) -> Result<SlideNoteSemantics, ParseError> {
    let shape = solve_slide_shape(body)?;
    let is_just_right = detect_just_type(raw_text)?;
    Ok(SlideNoteSemantics {
        raw_text: raw_text.to_string(),
        start_slot: body.start_lane,
        end_area,
        shape,
        is_just_right,
    })
}

/// `parseTerminalEndArea`.
pub fn parse_terminal_end_area(raw_text: &str) -> Result<SensorArea, ParseError> {
    let cs: Vec<char> = raw_text.chars().collect();
    let facts = text_kernel::command_facts(&cs);
    let index = kernel::terminal_end_index(facts.turn, facts.pp, facts.qq);
    parse_end_area_at(&cs, index)
}

/// `parseSlideShapeText`.
pub fn parse_slide_shape_text(raw_text: &str) -> Result<SlideShape, ParseError> {
    let sanitized = sanitize_slide_text(raw_text);
    parse_slide_body_from_text(&sanitized).and_then(|b| solve_slide_shape(&b))
}

/// `parseSlideJustText`.
pub fn parse_slide_just_text(raw_text: &str) -> Result<bool, ParseError> {
    let sanitized = sanitize_slide_text(raw_text);
    detect_just_type(&sanitized)
}

/// `parseSlideTimingPoint`.
pub fn parse_slide_timing_point(
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    raw_notes: &[String],
) -> Result<TimingPointSemantics, ParseError> {
    let notes = sequence_kernel::try_map_indexed(
        raw_notes.len(),
        |index| -> Result<SlideNoteSemantics, ParseError> {
            let raw = &raw_notes[index];
            let shape = parse_slide_body_from_text(raw).and_then(|b| solve_slide_shape(&b))?;
            let just = detect_just_type(raw)?;
            let cs: Vec<char> = raw.chars().collect();
            let start_slot = parse_start_lane_at(&cs, 0)?;
            let end_area = parse_terminal_end_area(raw)?;
            Ok(SlideNoteSemantics {
                raw_text: raw.clone(),
                start_slot,
                end_area,
                shape,
                is_just_right: just,
            })
        },
    )?;
    Ok(TimingPointSemantics {
        timing,
        bpm,
        h_speed,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_drops_flags() {
        assert_eq!(sanitize_slide_text("1-3bxf"), "1-3");
    }

    #[test]
    fn note_parse() {
        let note = parse_slide_note("1-3", OuterSlot::S1, SensorArea::A3).unwrap();
        assert_eq!(note.start_slot, OuterSlot::S1);
        assert_eq!(note.end_area, SensorArea::A3);
        assert_eq!(note.raw_text, "1-3");
    }

    #[test]
    fn terminal_end_area() {
        assert_eq!(parse_terminal_end_area("1-3").unwrap(), SensorArea::A3);
        assert_eq!(parse_terminal_end_area("1-8").unwrap(), SensorArea::A8);
    }
}
