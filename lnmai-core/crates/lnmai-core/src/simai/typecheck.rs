//! Slide type-checking / grouping.
//!
//! Mirrors `LnmaiCore/Simai/Typecheck.lean`.

use crate::natural::Natural;
use crate::simai::slide_parser::{
    parse_slide_note, parse_slide_note_from_body, parse_terminal_end_area,
};
use crate::simai::syntax::{
    CanonicalSlideShape, ParseError, ParseErrorKind, RawNoteKind, RawNoteToken, SlideNoteSemantics,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypedSlidePart {
    pub token: RawNoteToken,
    pub semantics: SlideNoteSemantics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
// Preserve the public typed syntax tree and its direct Lean correspondence.
#[allow(clippy::large_enum_variant)]
pub enum TypedSlideExpr {
    Single(TypedSlidePart),
    Conn(Vec<TypedSlidePart>),
}

impl Default for TypedSlideExpr {
    fn default() -> Self {
        Self::Single(TypedSlidePart::default())
    }
}

fn error(kind: ParseErrorKind, raw_text: &str, message: &str) -> ParseError {
    ParseError {
        kind,
        raw_text: raw_text.to_string(),
        message: message.to_string(),
        span: None,
    }
}

fn typed_slide_part(token: &RawNoteToken) -> Result<TypedSlidePart, ParseError> {
    let slot = token.slot.ok_or_else(|| {
        error(
            ParseErrorKind::InvalidSyntax,
            &token.raw_text,
            "slide token is missing a start slot",
        )
    })?;
    let end_area = parse_terminal_end_area(&token.raw_text)?;
    let semantics = match &token.slide_body {
        Some(body) => parse_slide_note_from_body(&token.raw_text, body, end_area)?,
        None => parse_slide_note(&token.raw_text, slot, end_area)?,
    };
    Ok(TypedSlidePart {
        token: token.clone(),
        semantics,
    })
}

fn wifi_conn_error(parts: &[TypedSlidePart]) -> ParseError {
    let raw = parts
        .first()
        .map(|p| p.token.raw_text.clone())
        .unwrap_or_default();
    error(
        ParseErrorKind::InvalidSyntax,
        &raw,
        "wifi slide cannot be part of a connection slide group",
    )
}

fn invalid_conn_group_error(token: &RawNoteToken) -> ParseError {
    error(
        ParseErrorKind::InvalidSyntax,
        &token.raw_text,
        "invalid connection slide group metadata",
    )
}

fn collect_conn_group(
    gid: Natural,
    expected_size: Natural,
    mut acc: Vec<TypedSlidePart>,
    remaining: &[RawNoteToken],
) -> Result<(Vec<TypedSlidePart>, Vec<RawNoteToken>), ParseError> {
    let mut i = 0usize;
    loop {
        if acc.len() == expected_size {
            return Ok((acc, remaining[i..].to_vec()));
        }
        match remaining.get(i) {
            None => {
                let raw = acc
                    .last()
                    .map(|p| p.token.raw_text.clone())
                    .unwrap_or_default();
                return Err(error(
                    ParseErrorKind::InvalidSyntax,
                    &raw,
                    "invalid connection slide group metadata",
                ));
            }
            Some(token) => match (token.kind, token.source_group_id.clone()) {
                (RawNoteKind::Slide, Some(token_gid)) if token_gid == gid => {
                    let part = typed_slide_part(token)?;
                    // Keep source order. Lean prepends to a reversed accumulator
                    // and reverses it on success; its head is our last element.
                    acc.push(part);
                    i += 1;
                }
                _ => return Err(invalid_conn_group_error(token)),
            },
        }
    }
}

fn validate_conn_group(parts: Vec<TypedSlidePart>) -> Result<TypedSlideExpr, ParseError> {
    if parts
        .iter()
        .any(|p| matches!(p.semantics.shape.canonical, CanonicalSlideShape::Wifi))
    {
        Err(wifi_conn_error(&parts))
    } else {
        Ok(TypedSlideExpr::Conn(parts))
    }
}

fn build_typed_slides(tokens: &[RawNoteToken]) -> Result<Vec<TypedSlideExpr>, ParseError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        let token = &tokens[i];
        if token.kind != RawNoteKind::Slide {
            i += 1;
            continue;
        }
        match (
            token.source_group_id.clone(),
            token.source_group_index.clone(),
            token.source_group_size.clone(),
        ) {
            (Some(gid), Some(idx), Some(size)) if idx == 0 => {
                let part = typed_slide_part(token)?;
                if size <= 1 {
                    out.push(TypedSlideExpr::Single(part));
                    i += 1;
                } else {
                    let (parts, remaining) =
                        collect_conn_group(gid, size, vec![part], &tokens[i + 1..])?;
                    let expr = validate_conn_group(parts)?;
                    out.push(expr);
                    // Advance past the tokens consumed by the group.
                    let consumed = tokens.len() - (i + 1) - remaining.len();
                    i += 1 + consumed;
                }
            }
            (Some(_), Some(idx), Some(_)) => {
                if idx == 0 {
                    return Err(invalid_conn_group_error(token));
                }
                i += 1;
            }
            _ => {
                let part = typed_slide_part(token)?;
                out.push(TypedSlideExpr::Single(part));
                i += 1;
            }
        }
    }
    Ok(out)
}

/// `typecheckSlides`.
pub fn typecheck_slides(tokens: &[RawNoteToken]) -> Result<Vec<TypedSlideExpr>, ParseError> {
    build_typed_slides(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rat::Rat;
    use crate::simai::tokenize::parse_segments;
    use crate::time::TimePoint;

    #[test]
    fn single_slide_typechecks() {
        let tokens = parse_segments(
            &["1-3".to_string()],
            TimePoint::zero(),
            Rat::from_int(120),
            Rat::one(),
            (4).into(),
            &[],
        )
        .unwrap();
        let typed = typecheck_slides(&tokens).unwrap();
        assert_eq!(typed.len(), 1);
        assert!(matches!(typed[0], TypedSlideExpr::Single(_)));
    }
}
