//! Simai slide-shape classification.
//!
//! Mirrors `LnmaiCore/Simai/Shape.lean`.

use crate::areas::{OuterSlot, SensorArea};
use crate::natural::Natural;
use crate::simai::syntax::{
    CanonicalSlideShape, ParseError, ParseErrorKind, ParsedSlideBody, SlideKind, SlideShape,
};
use crate::symmetry;
use lnmai_core_verify::{
    shape as kernel, slide_parser as parser_kernel, slide_parser_frontend as frontend,
    slide_parser_text as text_kernel,
};

pub use kernel::digit_to_nat;

pub fn shape_kind(shape: &SlideShape) -> SlideKind {
    match &shape.canonical {
        CanonicalSlideShape::Line(_) => SlideKind::Line,
        CanonicalSlideShape::Circle(_) => SlideKind::Circle,
        CanonicalSlideShape::V(_) => SlideKind::V,
        CanonicalSlideShape::Turn(_) => SlideKind::Turn,
        CanonicalSlideShape::Pq(_) => SlideKind::Pq,
        CanonicalSlideShape::Ppqq(_) => SlideKind::Ppqq,
        CanonicalSlideShape::S => SlideKind::S,
        CanonicalSlideShape::Wifi => SlideKind::Wifi,
    }
}

pub fn shape_rel_end(shape: &SlideShape) -> Option<Natural> {
    match &shape.canonical {
        CanonicalSlideShape::Line(n)
        | CanonicalSlideShape::Circle(n)
        | CanonicalSlideShape::V(n)
        | CanonicalSlideShape::Turn(n)
        | CanonicalSlideShape::Pq(n)
        | CanonicalSlideShape::Ppqq(n) => Some(n.clone()),
        CanonicalSlideShape::S | CanonicalSlideShape::Wifi => None,
    }
}

pub fn shape_mirrored(shape: &SlideShape) -> bool {
    symmetry::is_mirrored(shape.symmetry)
}

pub fn mk_canonical_slide_shape_opt(
    kind: SlideKind,
    rel_end: Option<Natural>,
) -> Option<CanonicalSlideShape> {
    match rel_end {
        None => kernel::mk_canonical_slide_shape_opt(kind, None).map(Into::into),
        Some(n) => match n.to_usize() {
            Some(small) => kernel::mk_canonical_slide_shape_opt(kind, Some(small)).map(Into::into),
            // Only these constructors accept endpoints beyond the machine range.
            None => match kind {
                SlideKind::Circle => Some(CanonicalSlideShape::Circle(n)),
                SlideKind::V => Some(CanonicalSlideShape::V(n)),
                SlideKind::Pq => Some(CanonicalSlideShape::Pq(n)),
                SlideKind::Ppqq => Some(CanonicalSlideShape::Ppqq(n)),
                _ => None,
            },
        },
    }
}

pub fn mk_canonical_slide_shape_unchecked(
    kind: SlideKind,
    rel_end: Option<Natural>,
) -> CanonicalSlideShape {
    match rel_end {
        None => kernel::mk_canonical_slide_shape_unchecked(kind, None).into(),
        Some(n) => match n.to_usize() {
            Some(small) => kernel::mk_canonical_slide_shape_unchecked(kind, Some(small)).into(),
            None => match kind {
                SlideKind::Line => CanonicalSlideShape::Line(n),
                SlideKind::Circle => CanonicalSlideShape::Circle(n),
                SlideKind::V => CanonicalSlideShape::V(n),
                SlideKind::Turn => CanonicalSlideShape::Turn(n),
                SlideKind::Pq => CanonicalSlideShape::Pq(n),
                SlideKind::Ppqq => CanonicalSlideShape::Ppqq(n),
                SlideKind::S => CanonicalSlideShape::S,
                SlideKind::Wifi => CanonicalSlideShape::Wifi,
            },
        },
    }
}

pub fn mk_slide_shape(
    canonical: CanonicalSlideShape,
    symmetry: symmetry::SlideSymmetry,
) -> SlideShape {
    SlideShape {
        canonical,
        symmetry,
    }
}

pub fn canonical_rel_end(sym: symmetry::SlideSymmetry, rel_end: impl Into<Natural>) -> Natural {
    let rel_end = rel_end.into();
    match rel_end.to_usize() {
        Some(n) => kernel::canonical_rel_end(sym, n).into(),
        // Lean's mirrorKey is the identity outside 1..=8.
        None => rel_end,
    }
}

pub fn base_rel_end(sym: symmetry::SlideSymmetry, rel_end: impl Into<Natural>) -> Natural {
    canonical_rel_end(sym, rel_end)
}

fn shape_error(raw_text: &str, failure: kernel::ShapeError) -> ParseError {
    match failure {
        kernel::ShapeError::EndNotOuter => error(
            ParseErrorKind::InvalidEndPosition,
            "",
            "slide end must be on outer A-ring",
        ),
        kernel::ShapeError::InvalidEnd => {
            error(ParseErrorKind::InvalidEndPosition, raw_text, "invalid end")
        }
        kernel::ShapeError::InvalidTurnEnd => {
            error(ParseErrorKind::InvalidTurnPosition, raw_text, "invalid end")
        }
        kernel::ShapeError::InvalidTurn => error(
            ParseErrorKind::InvalidTurnPosition,
            raw_text,
            "invalid turn",
        ),
    }
}

/// solveSlideShape: use the same pure implementation that is extracted for Lean.
pub fn solve_slide_shape(body: &ParsedSlideBody) -> Result<SlideShape, ParseError> {
    kernel::solve_shape(body.kind, body.start_lane, body.end_area, body.turn_area)
        .map(Into::into)
        .map_err(|err| shape_error(&body.raw_text, err))
}

fn error(kind: ParseErrorKind, raw_text: &str, message: &str) -> ParseError {
    ParseError {
        kind,
        raw_text: raw_text.to_string(),
        message: message.to_string(),
        span: None,
    }
}

/// `mkCanonicalSlideShape`.
pub fn mk_canonical_slide_shape(
    kind: SlideKind,
    rel_end: Option<Natural>,
    raw_text: &str,
    message: &str,
) -> Result<CanonicalSlideShape, ParseError> {
    match mk_canonical_slide_shape_opt(kind, rel_end) {
        Some(shape) => Ok(shape),
        None => Err(error(ParseErrorKind::InvalidEndPosition, raw_text, message)),
    }
}

/// `getAt?`.
pub fn get_at(xs: &[char], n: impl Into<Natural>) -> Option<char> {
    frontend::get_at(xs, n.into().to_usize()?)
}

/// `canonicalShapeKey`.
pub fn canonical_shape_key(shape: &SlideShape) -> String {
    let sym = shape.symmetry;
    match &shape.canonical {
        CanonicalSlideShape::Line(n) => format!("line{}", n),
        CanonicalSlideShape::Circle(n) => format!("circle{}", base_rel_end(sym, n)),
        CanonicalSlideShape::V(n) => format!("v{}", n),
        CanonicalSlideShape::Turn(n) => format!("L{}", base_rel_end(sym, n)),
        CanonicalSlideShape::Pq(n) => format!("pq{}", base_rel_end(sym, n)),
        CanonicalSlideShape::Ppqq(n) => format!("ppqq{}", base_rel_end(sym, n)),
        CanonicalSlideShape::S => "s".to_string(),
        CanonicalSlideShape::Wifi => "wifi".to_string(),
    }
}

/// `displayShapeKey`.
pub fn display_shape_key(shape: &SlideShape) -> String {
    let base = canonical_shape_key(shape);
    if base == "wifi" || base.is_empty() || !symmetry::is_mirrored(shape.symmetry) {
        base
    } else {
        format!("-{}", base)
    }
}

/// `shapeKey`.
pub fn shape_key(shape: &SlideShape) -> String {
    display_shape_key(shape)
}

fn read_error(content: &[char], index: &Natural, failure: frontend::ReadError) -> ParseError {
    let message = match failure {
        frontend::ReadError::MissingDigit => format!("missing digit at {}", index),
        frontend::ReadError::ExpectedDigit => format!("expected digit at {}", index),
        frontend::ReadError::InvalidStartLane => format!("invalid start lane at {}", index),
        frontend::ReadError::InvalidEndArea => format!("invalid end area at {}", index),
    };
    error(
        ParseErrorKind::InvalidSyntax,
        &content.iter().collect::<String>(),
        &message,
    )
}

fn read_start_lane_at(
    content: &[char],
    index: impl Into<Natural>,
) -> Result<OuterSlot, ParseError> {
    let index = index.into();
    frontend::read_start_lane(get_at(content, &index))
        .map_err(|failure| read_error(content, &index, failure))
}

fn read_end_area_at(content: &[char], index: impl Into<Natural>) -> Result<SensorArea, ParseError> {
    let index = index.into();
    frontend::read_end_area(get_at(content, &index))
        .map_err(|failure| read_error(content, &index, failure))
}

/// `parseSlideBodyFromText`, with command control in the extracted kernel.
/// Start-lane errors precede unknown commands; turn errors precede end errors.
pub fn parse_slide_body_from_text(content: &str) -> Result<ParsedSlideBody, ParseError> {
    let cs: Vec<char> = content.chars().collect();
    let start_lane = read_start_lane_at(&cs, 0)?;
    let kind = parser_kernel::select_command(text_kernel::command_facts(&cs)).ok_or_else(|| {
        error(
            ParseErrorKind::InvalidShape,
            content,
            "unrecognized Simai slide shape",
        )
    })?;
    let turn_area = match parser_kernel::command_turn_index(kind) {
        Some(index) => Some(read_end_area_at(&cs, index)?),
        None => None,
    };
    let end_area = read_end_area_at(&cs, parser_kernel::command_end_index(kind))?;
    Ok(ParsedSlideBody {
        raw_text: content.to_string(),
        start_lane,
        kind,
        end_area: Some(end_area),
        turn_area,
    })
}

/// `detectShapeFromText`.
pub fn detect_shape_from_text(content: &str) -> Result<SlideShape, ParseError> {
    let body = parse_slide_body_from_text(content)?;
    solve_slide_shape(&body)
}

/// `detectJustType`.
pub fn detect_just_type(content: &str) -> Result<bool, ParseError> {
    let cs: Vec<char> = content.chars().collect();
    frontend::detect_just_type(&cs).map_err(|failure| match failure {
        frontend::JustError::At(issue, index) => read_error(&cs, &index.into(), issue),
        frontend::JustError::Shape(issue) => shape_error("", issue),
        frontend::JustError::EndNotOuter => error(
            ParseErrorKind::InvalidEndPosition,
            content,
            "slide end must be on outer A-ring",
        ),
    })
}

/// `parseStartLaneAt`.
pub fn parse_start_lane_at(
    content: &[char],
    index: impl Into<Natural>,
) -> Result<OuterSlot, ParseError> {
    read_start_lane_at(content, index)
}

/// `parseEndAreaAt`.
pub fn parse_end_area_at(
    content: &[char],
    index: impl Into<Natural>,
) -> Result<SensorArea, ParseError> {
    read_end_area_at(content, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_line() {
        let shape = detect_shape_from_text("1-3").unwrap();
        assert_eq!(shape.canonical, CanonicalSlideShape::Line(3.into()));
        assert_eq!(canonical_shape_key(&shape), "line3");
    }

    #[test]
    fn invalid_v_at_5() {
        assert!(detect_shape_from_text("1v5").is_err());
        assert!(detect_shape_from_text("1v3").is_ok());
    }

    #[test]
    fn wifi_requires_center() {
        let shape = detect_shape_from_text("1w5").unwrap();
        assert_eq!(shape.canonical, CanonicalSlideShape::Wifi);
        assert!(detect_shape_from_text("1w3").is_err());
    }

    #[test]
    fn just_type_ishape() {
        // "1>3": circle right from S1 (upper half) → isJustRight true.
        assert!(detect_just_type("1>3").unwrap());
    }
}
