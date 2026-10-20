//! maidata.txt front-end and chart lowering.
//!
//! Mirrors `LnmaiCore/Simai/Source/Maidata.lean`.

use crate::natural::Natural;
use crate::rat::Rat;
use crate::simai::ir::{FrontendChartInspection, FrontendChartResult, FrontendSemanticChart};
use crate::simai::normalize::{lower_raw_tokens_with_mode, to_chart_spec};
use crate::simai::syntax::{
    MaidataChartBlock, MaidataFile, MaidataMetadata, ParseError, ParseErrorKind, RawNoteKind,
    RawNoteToken, SourceChart, SourceEvent, SourceNote,
};
use crate::simai::timing::{parse_rat_def, trim};
use crate::simai::tokenize::{parse_segments, strip_comments};
use crate::simai::typecheck::typecheck_slides;
use crate::time::{self, TimePoint};
use lnmai_core_verify::{maidata_line_adapter, maidata_scan};

// Compatibility names exported by `LnmaiCore.Simai.Maidata`.
pub use self::{
    lower_source_chart_block as lower_chart_block,
    lower_source_chart_by_level as lower_chart_by_level,
    parse_and_lower_source_maidata as parse_and_lower_maidata,
    parse_source_maidata as parse_maidata,
};

fn same_event_key(
    token: &RawNoteToken,
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
) -> bool {
    token.timing == timing
        && token.bpm == bpm
        && token.h_speed == h_speed
        && token.divisor == divisor
}

struct TouchGroupOps<'a> {
    tokens: &'a [RawNoteToken],
}

impl maidata_scan::TouchRead for TouchGroupOps<'_> {
    fn same_event(&self, head: usize, index: usize) -> bool {
        let first = &self.tokens[head];
        let token = &self.tokens[index];
        token.timing == first.timing
            && token.bpm == first.bpm
            && token.h_speed == first.h_speed
            && token.divisor == first.divisor
    }

    fn kind(&self, index: usize) -> maidata_scan::TouchKind {
        match self.tokens[index].kind {
            RawNoteKind::Slide => maidata_scan::TouchKind::Slide,
            RawNoteKind::Touch => maidata_scan::TouchKind::Touch,
            RawNoteKind::TouchHold => maidata_scan::TouchKind::TouchHold,
            _ => maidata_scan::TouchKind::Other,
        }
    }

    fn source_group_index_nonzero(&self, index: usize) -> bool {
        matches!(self.tokens[index].source_group_index.as_ref(), Some(i) if *i != 0)
    }

    fn is_slide_no_head(&self, index: usize) -> bool {
        self.tokens[index].is_slide_no_head
    }
}

impl maidata_scan::TouchWrite<Natural, Vec<RawNoteToken>> for TouchGroupOps<'_> {
    fn append_unchanged(&self, mut output: Vec<RawNoteToken>, source: usize) -> Vec<RawNoteToken> {
        output.push(self.tokens[source].clone());
        output
    }

    fn append_tagged(
        &self,
        mut output: Vec<RawNoteToken>,
        source: usize,
        group_id: &Natural,
        group_index: usize,
        group_size: usize,
    ) -> Vec<RawNoteToken> {
        output.push(RawNoteToken {
            source_group_id: Some(group_id.clone()),
            source_group_index: Some(group_index.into()),
            source_group_size: Some(group_size.into()),
            ..self.tokens[source].clone()
        });
        output
    }

    fn next_group(&self, group_id: Natural) -> Natural {
        group_id + 1
    }
}

fn annotate_touch_each_groups_from(
    group_id: Natural,
    tokens: &[RawNoteToken],
) -> Vec<RawNoteToken> {
    maidata_scan::annotate_touch_each(
        tokens.len(),
        group_id,
        Vec::with_capacity(tokens.len()),
        TouchGroupOps { tokens },
    )
}

fn annotate_touch_each_groups(tokens: &[RawNoteToken]) -> Vec<RawNoteToken> {
    annotate_touch_each_groups_from((0).into(), tokens)
}

fn source_chart_from_tokens(tokens: &[RawNoteToken]) -> SourceChart {
    let mut events: Vec<SourceEvent> = Vec::new();
    let mut current: Option<SourceEvent> = None;
    for token in tokens {
        let note = SourceNote {
            token: token.clone(),
            source_pos: token.source_pos.clone(),
        };
        match &mut current {
            None => {
                current = Some(SourceEvent {
                    timing: token.timing.clone(),
                    bpm: token.bpm.clone(),
                    h_speed: token.h_speed.clone(),
                    divisor: token.divisor.clone(),
                    notes: vec![note],
                    source_pos: None,
                });
            }
            Some(event)
                if same_event_key(
                    token,
                    event.timing.clone(),
                    event.bpm.clone(),
                    event.h_speed.clone(),
                    event.divisor.clone(),
                ) =>
            {
                event.notes.push(note);
            }
            Some(event) => {
                events.push(event.clone());
                current = Some(SourceEvent {
                    timing: token.timing.clone(),
                    bpm: token.bpm.clone(),
                    h_speed: token.h_speed.clone(),
                    divisor: token.divisor.clone(),
                    notes: vec![note],
                    source_pos: None,
                });
            }
        }
    }
    if let Some(event) = current {
        events.push(event);
    }
    SourceChart { events }
}

fn trimmed_line(s: &str) -> &str {
    // The same four characters as Timing.trim / Lean Char.isWhitespace.
    s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

fn parse_key_value_line(line: &str) -> Option<(String, String)> {
    let key = match line.find('=') {
        Some(i) => trim(&line[..i]),
        None => trim(line),
    };
    let value = match line.find('=') {
        Some(i) => trim(&line[i + 1..]),
        None => String::new(),
    };
    Some((key, value))
}

struct MaidataTextOps;

impl maidata_line_adapter::LinePrimitives<String> for MaidataTextOps {
    type View = str;
    type Level = Natural;

    fn borrow<'a>(&self, text: &'a String) -> &'a str {
        text.as_str()
    }

    fn trim<'a>(&self, view: &'a str) -> &'a str {
        trimmed_line(view)
    }

    fn is_empty(&self, view: &str) -> bool {
        view.is_empty()
    }

    fn starts_with_amp(&self, view: &str) -> bool {
        view.starts_with('&')
    }

    fn starts_with_inote(&self, view: &str) -> bool {
        view.starts_with("&inote_")
    }

    fn header(&self, view: &str) -> Option<(String, String)> {
        parse_key_value_line(view)
    }

    fn level(&self, view: &str) -> Natural {
        view.strip_prefix("&inote_")
            .and_then(Natural::from_lean_str)
            .unwrap_or_default()
    }

    fn push_newline(&self, text: &mut String) {
        text.push('\n');
    }

    fn push_str(&self, text: &mut String, view: &str) {
        text.push_str(view);
    }
}

fn parse_maidata_lines(
    lines: &[String],
    fields: Vec<(String, String)>,
    charts: Vec<MaidataChartBlock>,
) -> MaidataFile {
    let (fields, charts) = maidata_scan::collect_lines(
        lines.len(),
        fields,
        charts,
        maidata_line_adapter::MaidataLineOps {
            lines,
            primitives: MaidataTextOps,
        },
    );
    MaidataFile {
        metadata: MaidataMetadata { fields },
        charts,
    }
}

fn metadata_field(md: &MaidataMetadata, key: &str) -> Option<String> {
    md.fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

/// `parseSourceMaidata`.
pub fn parse_source_maidata(content: &str) -> Result<MaidataFile, ParseError> {
    let lines: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
    Ok(parse_maidata_lines(&lines, Vec::new(), Vec::new()))
}

/// `lowerSourceChartBlock`.
pub fn lower_source_chart_block_with_mode(
    is_classic: bool,
    file: &MaidataFile,
    block: &MaidataChartBlock,
) -> Result<FrontendChartResult, ParseError> {
    let base_bpm = parse_rat_def(
        &metadata_field(&file.metadata, "&wholebpm").unwrap_or_else(|| "120".to_string()),
        Rat::from_int(120),
    );
    let first_offset = match time::parse_seconds_point_string(
        &metadata_field(&file.metadata, "&first").unwrap_or_else(|| "0".to_string()),
    ) {
        Some(value) => value,
        None => TimePoint::zero(),
    };
    let cleaned_body = strip_comments(&block.raw_body);
    let segments: Vec<String> = cleaned_body.split(',').map(|s| s.to_string()).collect();
    let raw_tokens = parse_segments(
        &segments,
        first_offset,
        base_bpm,
        Rat::one(),
        (4).into(),
        &[],
    )?;
    let tokens = annotate_touch_each_groups(&raw_tokens);
    let _typed = typecheck_slides(&tokens)?;
    let source = source_chart_from_tokens(&tokens);
    let measure_dur_sec =
        |bpm: Rat| time::duration_from_rat_micros(time::bpm_measure_micros_rat(bpm));
    let (normalized, slide_notes) =
        lower_raw_tokens_with_mode(is_classic, &measure_dur_sec, &tokens);
    let lowered = to_chart_spec(&normalized);
    Ok(FrontendChartResult {
        semantic: FrontendSemanticChart {
            normalized,
            lowered,
        },
        inspection: FrontendChartInspection {
            metadata: file.metadata.clone(),
            chart: block.clone(),
            source,
            tokens,
            slide_notes,
        },
    })
}

/// `lowerSourceChartByLevel`.
pub fn lower_source_chart_by_level_with_mode(
    is_classic: bool,
    file: &MaidataFile,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    match file
        .charts
        .iter()
        .find(|block| block.level_index == level_index)
    {
        Some(block) => lower_source_chart_block_with_mode(is_classic, file, block),
        None => Err(ParseError {
            kind: ParseErrorKind::InvalidSyntax,
            raw_text: String::new(),
            message: format!("missing inote block {}", level_index),
            span: None,
        }),
    }
}

/// `parseAndLowerSourceMaidata`.
pub fn parse_and_lower_source_maidata_with_mode(
    content: &str,
    level_index: Natural,
    is_classic: bool,
) -> Result<FrontendChartResult, ParseError> {
    let file = parse_source_maidata(content)?;
    lower_source_chart_by_level_with_mode(is_classic, &file, level_index)
}

pub fn lower_source_chart_block(
    file: &MaidataFile,
    block: &MaidataChartBlock,
) -> Result<FrontendChartResult, ParseError> {
    lower_source_chart_block_with_mode(false, file, block)
}

pub fn lower_source_chart_by_level(
    file: &MaidataFile,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    lower_source_chart_by_level_with_mode(false, file, level_index)
}

pub fn parse_and_lower_source_maidata(
    content: &str,
    level_index: Natural,
) -> Result<FrontendChartResult, ParseError> {
    parse_and_lower_source_maidata_with_mode(content, level_index, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::OuterSlot;

    const MAIDATA: &str = "&title=Test\n&wholebpm=120\n&first=0\n&inote_1=\n(120)\n1,2,3,\n";

    #[test]
    fn parse_and_lower() {
        let file = parse_source_maidata(MAIDATA).unwrap();
        assert_eq!(file.charts.len(), 1);
        assert_eq!(file.charts[0].level_index, 1);
        let result = parse_and_lower_source_maidata(MAIDATA, (1).into()).unwrap();
        assert_eq!(result.semantic.lowered.taps.len(), 3);
        assert_eq!(result.semantic.normalized.taps.len(), 3);
    }

    #[test]
    fn key_value_line() {
        assert_eq!(
            parse_key_value_line("&title=A=B"),
            Some(("&title".to_string(), "A=B".to_string()))
        );
        assert_eq!(
            parse_key_value_line("bare"),
            Some(("bare".to_string(), "".to_string()))
        );
    }

    #[test]
    fn indented_directives_and_invalid_level_suffixes() {
        let file = parse_source_maidata(
            " \t&title=Indented\n&inote_1=1,\n\r&\r\n2,\n &inote_+1=3,\n&inote_1_0=4,\n",
        )
        .unwrap();
        assert_eq!(
            file.metadata.fields[0],
            ("&title".into(), "Indented".into())
        );
        assert_eq!(file.charts[0].raw_body, "1,");
        assert_eq!(file.charts[1].level_index, 0);
        assert_eq!(file.charts[2].level_index, 10);
    }

    #[test]
    fn multi_measure_body_keeps_source_order() {
        // Regression: `collect_chart_body` used to reverse the body lines, so
        // later measures were lowered before earlier ones. Expected timings are
        // the ones Lean (`compileRuntimeChartSection`) produces for this body.
        let content = "&first=0\n&inote_1=\n(210){4},{8},,,,,,,,6,\n5,4,3,4,5,\n";
        let result = parse_and_lower_source_maidata(content, (1).into()).unwrap();
        let taps = &result.semantic.lowered.taps;
        assert_eq!(taps.len(), 6);
        let slots: Vec<_> = taps.iter().map(|t| t.slot).collect();
        assert_eq!(
            slots,
            vec![
                OuterSlot::S6,
                OuterSlot::S5,
                OuterSlot::S4,
                OuterSlot::S3,
                OuterSlot::S4,
                OuterSlot::S5
            ]
        );
        let times: Vec<_> = taps.iter().map(|t| t.timing.to_micros()).collect();
        assert_eq!(
            times,
            vec![1_428_571, 1_571_429, 1_714_286, 1_857_143, 2_000_000, 2_142_857]
        );
    }
}
