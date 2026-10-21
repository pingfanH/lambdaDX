//! Public Simai front-end entry points.
//!
//! Mirrors `LnmaiCore/Simai/Frontend.lean`.

use crate::chart_loader::ChartSpec;
use crate::natural::Natural;
use crate::simai::ir::{
    FrontendChartInspection, FrontendChartResult, FrontendSemanticChart, NormalizedChart,
    NormalizedSlide,
};
use crate::simai::maidata::{
    lower_source_chart_block, lower_source_chart_by_level, parse_and_lower_source_maidata,
    parse_source_maidata,
};
use crate::simai::syntax::{
    MaidataChartBlock, MaidataFile, ParseError, ParseErrorKind, RawNoteToken, SlideNoteSemantics,
};
use crate::simai::timing::trim;

fn single_note_maidata(note_text: &str) -> String {
    format!("&first=0\n&inote_1=\n(120)\n{},\n", note_text)
}

fn exactly_one_error(kind: &str, raw_text: &str, count: usize) -> ParseError {
    ParseError {
        kind: ParseErrorKind::InvalidSyntax,
        raw_text: raw_text.to_string(),
        message: format!("expected exactly one {}, found {}", kind, count),
        span: None,
    }
}

/// `parseFrontendMaidata`.
pub fn parse_frontend_maidata(content: &str) -> Result<MaidataFile, ParseError> {
    parse_source_maidata(content)
}

/// `frontendChartResultOfBlock`.
pub fn frontend_chart_result_of_block(
    file: &MaidataFile,
    block: &MaidataChartBlock,
) -> Result<FrontendChartResult, ParseError> {
    lower_source_chart_block(file, block)
}

/// `frontendChartResultOfLevel`.
pub fn frontend_chart_result_of_level(
    file: &MaidataFile,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    lower_source_chart_by_level(file, level_index)
}

/// `parseFrontendChartResult`.
pub fn parse_frontend_chart_result(
    content: &str,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    parse_and_lower_source_maidata(content, level_index)
}

/// `parseFrontendSemanticChart`.
pub fn parse_frontend_semantic_chart(
    content: &str,
    level_index: Natural,
) -> Result<FrontendSemanticChart, ParseError> {
    Ok(parse_frontend_chart_result(content, level_index)?.semantic)
}

/// `parseFrontendInspectionChart`.
pub fn parse_frontend_inspection_chart(
    content: &str,
    level_index: Natural,
) -> Result<FrontendChartInspection, ParseError> {
    Ok(parse_frontend_chart_result(content, level_index)?.inspection)
}

/// `frontendNormalizedChart`.
pub fn frontend_normalized_chart(
    content: &str,
    level_index: Natural,
) -> Result<NormalizedChart, ParseError> {
    Ok(parse_frontend_semantic_chart(content, level_index)?.normalized)
}

/// `frontendLoweredChart`.
pub fn frontend_lowered_chart(
    content: &str,
    level_index: Natural,
) -> Result<ChartSpec, ParseError> {
    Ok(parse_frontend_semantic_chart(content, level_index)?.lowered)
}

/// `parseFrontendSingleToken`.
pub fn parse_frontend_single_token(note_text: &str) -> Result<RawNoteToken, ParseError> {
    let result = parse_frontend_chart_result(&single_note_maidata(note_text), (1).into())?;
    match result.inspection.tokens.as_slice() {
        [token] => Ok(token.clone()),
        tokens => Err(exactly_one_error("note token", note_text, tokens.len())),
    }
}

/// `parseFrontendSingleSlideNote`.
pub fn parse_frontend_single_slide_note(note_text: &str) -> Result<SlideNoteSemantics, ParseError> {
    let result = parse_frontend_chart_result(&single_note_maidata(note_text), (1).into())?;
    match result.inspection.slide_notes.as_slice() {
        [note] => Ok(note.clone()),
        notes => Err(exactly_one_error("slide note", note_text, notes.len())),
    }
}

/// `parseFrontendSingleNormalizedSlide`.
pub fn parse_frontend_single_normalized_slide(
    note_text: &str,
) -> Result<NormalizedSlide, ParseError> {
    let result = parse_frontend_chart_result(&single_note_maidata(note_text), (1).into())?;
    match result.semantic.normalized.slides.as_slice() {
        [slide] => Ok(slide.clone()),
        slides => Err(exactly_one_error(
            "normalized slide",
            note_text,
            slides.len(),
        )),
    }
}

/// `lowerFrontendChartBlock`.
pub fn lower_frontend_chart_block(
    file: &MaidataFile,
    block: &MaidataChartBlock,
) -> Result<FrontendChartResult, ParseError> {
    frontend_chart_result_of_block(file, block)
}

/// `lowerFrontendChartByLevel`.
pub fn lower_frontend_chart_by_level(
    file: &MaidataFile,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    frontend_chart_result_of_level(file, level_index)
}

/// `parseFrontendChartByLevel`.
pub fn parse_frontend_chart_by_level(
    content: &str,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    parse_frontend_chart_result(content, level_index)
}

/// `trim` is re-exported for callers that build literals.
pub fn frontend_trim(s: &str) -> String {
    trim(s)
}

pub fn parse_frontend_chart_result_with_mode(
    content: &str,
    level_index: Natural,
    is_classic: bool,
) -> Result<FrontendChartResult, ParseError> {
    super::maidata::parse_and_lower_source_maidata_with_mode(content, level_index, is_classic)
}

pub fn frontend_lowered_chart_with_mode(
    content: &str,
    level_index: Natural,
    is_classic: bool,
) -> Result<ChartSpec, ParseError> {
    Ok(
        parse_frontend_chart_result_with_mode(content, level_index, is_classic)?
            .semantic
            .lowered,
    )
}

/// Lean's `panic!` reports an invalid literal and returns `Inhabited.default`.
/// In particular, invalid literal helpers do not unwind the runtime session.
pub(crate) fn literal_or_default<T: Default>(result: Result<T, ParseError>, kind: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid simai {kind} literal: {}", error.message);
            T::default()
        }
    }
}

pub fn frontend_chart_literal(content: &str, level_index: Natural) -> FrontendChartResult {
    literal_or_default(parse_frontend_chart_result(content, level_index), "chart")
}

pub fn frontend_semantic_chart_literal(
    content: &str,
    level_index: Natural,
) -> FrontendSemanticChart {
    frontend_chart_literal(content, level_index).semantic
}

pub fn frontend_inspection_chart_literal(
    content: &str,
    level_index: Natural,
) -> FrontendChartInspection {
    frontend_chart_literal(content, level_index).inspection
}

pub fn frontend_normalized_chart_literal(content: &str, level_index: Natural) -> NormalizedChart {
    frontend_chart_literal(content, level_index)
        .semantic
        .normalized
}

pub fn frontend_lowered_chart_literal(content: &str, level_index: Natural) -> ChartSpec {
    frontend_chart_literal(content, level_index)
        .semantic
        .lowered
}

pub fn frontend_note_literal(note_text: &str) -> RawNoteToken {
    literal_or_default(parse_frontend_single_token(note_text), "note")
}

pub fn frontend_slide_note_literal(note_text: &str) -> SlideNoteSemantics {
    literal_or_default(parse_frontend_single_slide_note(note_text), "slide")
}

pub fn frontend_normalized_slide_literal(note_text: &str) -> NormalizedSlide {
    literal_or_default(
        parse_frontend_single_normalized_slide(note_text),
        "normalized slide",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_token() {
        let token = parse_frontend_single_token("1-3").unwrap();
        assert_eq!(
            crate::simai::shape::shape_kind(
                &parse_frontend_single_slide_note("1-3").unwrap().shape
            ),
            crate::simai::syntax::SlideKind::Line
        );
        assert_eq!(token.raw_text, "1-3");
    }

    #[test]
    fn normalized_slide_single() {
        let slide = parse_frontend_single_normalized_slide("1-3").unwrap();
        assert_eq!(slide.judge_queues.len(), 1);
        assert!(slide.has_head_note);
    }
}
