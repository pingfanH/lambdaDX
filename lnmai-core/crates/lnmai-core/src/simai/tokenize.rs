//! Simai tokenization: segments → `RawNoteToken`s.
//!
//! Mirrors `LnmaiCore/Simai/Tokenize.lean`.

use crate::areas::{OuterSlot, SensorArea};
use crate::natural::Natural;
use crate::rat::Rat;
use crate::simai::shape::{detect_shape_from_text, digit_to_nat, parse_slide_body_from_text};
use crate::simai::slide_tables::slide_bar_count_for_shape;
use crate::simai::syntax::SlideKind;
use crate::simai::syntax::{ParseError, ParseErrorKind, RawNoteKind, RawNoteToken};
use crate::simai::timing::{
    parse_duration_spec, parse_nat_def, parse_rat_def, parse_star_wait_spec, trim,
};
use crate::time::{self, TimePoint};

use lnmai_core_verify::prefix_directives as prefix_kernel;
use lnmai_core_verify::slide_chain::{self, ChainError};
use lnmai_core_verify::slide_parser_text as text_kernel;
pub use lnmai_core_verify::tokenize::{is_slide_mark_char, is_touch_area_char};

fn syntax_error(raw_text: &str, message: &str) -> ParseError {
    ParseError {
        kind: ParseErrorKind::InvalidSyntax,
        raw_text: raw_text.to_string(),
        message: message.to_string(),
        span: None,
    }
}

/// `firstDigit?`.
pub fn first_digit(s: &str) -> Option<usize> {
    s.chars().find_map(digit_to_nat)
}

/// `leadingDigit?`.
pub fn leading_digit(s: &str) -> Option<usize> {
    s.chars().next().and_then(digit_to_nat)
}

/// `leadingTouchPos?`.
pub fn leading_touch_pos(s: &str) -> Option<usize> {
    let mut it = s.chars();
    match it.next() {
        Some('C') => Some(8),
        Some(_) => it.next().and_then(digit_to_nat),
        None => None,
    }
}

/// `touchAreaToSensorArea?`.
pub fn touch_area_to_sensor_area(s: &str) -> Option<SensorArea> {
    let mut it = s.chars();
    let area = it.next()?;
    if area == 'C' {
        return Some(SensorArea::C);
    }
    let d = it.next()?;
    let n = digit_to_nat(d)?;
    let pick = |a1, a2, a3, a4, a5, a6, a7, a8| match n {
        1 => a1,
        2 => a2,
        3 => a3,
        4 => a4,
        5 => a5,
        6 => a6,
        7 => a7,
        _ => a8,
    };
    match area {
        'A' => Some(pick(
            SensorArea::A1,
            SensorArea::A2,
            SensorArea::A3,
            SensorArea::A4,
            SensorArea::A5,
            SensorArea::A6,
            SensorArea::A7,
            SensorArea::A8,
        )),
        'D' => Some(pick(
            SensorArea::D1,
            SensorArea::D2,
            SensorArea::D3,
            SensorArea::D4,
            SensorArea::D5,
            SensorArea::D6,
            SensorArea::D7,
            SensorArea::D8,
        )),
        'E' => Some(pick(
            SensorArea::E1,
            SensorArea::E2,
            SensorArea::E3,
            SensorArea::E4,
            SensorArea::E5,
            SensorArea::E6,
            SensorArea::E7,
            SensorArea::E8,
        )),
        'B' => Some(pick(
            SensorArea::B1,
            SensorArea::B2,
            SensorArea::B3,
            SensorArea::B4,
            SensorArea::B5,
            SensorArea::B6,
            SensorArea::B7,
            SensorArea::B8,
        )),
        _ => None,
    }
}

/// `stripComments`.
pub fn strip_comments(s: &str) -> String {
    s.split('\n')
        .map(|line| line.split("||").next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `stripPrefixDirectives`.
pub fn strip_prefix_directives(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    prefix_kernel::strip_prefix_chars(&chars)
        .into_iter()
        .collect()
}

/// `sanitizeSlideToken`.
pub fn sanitize_slide_token(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    let stripped = prefix_kernel::strip_prefix_chars(&chars);
    text_kernel::sanitize_slide_chars(&stripped)
        .into_iter()
        .collect()
}

/// `isSlideText`.
pub fn is_slide_text(t: &str) -> bool {
    t.chars().any(is_slide_mark_char)
}

/// `inferKind`.
pub fn infer_kind(token: &str) -> RawNoteKind {
    let t = strip_prefix_directives(token);
    if t.is_empty() {
        RawNoteKind::Rest
    } else if leading_digit(&t).is_some() {
        if is_slide_text(&t) {
            RawNoteKind::Slide
        } else if t.contains('h') {
            RawNoteKind::Hold
        } else {
            RawNoteKind::Tap
        }
    } else {
        match t.chars().next() {
            Some(area) if is_touch_area_char(area) => {
                if t.contains('h') {
                    RawNoteKind::TouchHold
                } else {
                    RawNoteKind::Touch
                }
            }
            _ => RawNoteKind::Unknown,
        }
    }
}

/// `splitTopLevel`.
pub fn split_top_level(sep: char, s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    lnmai_core_verify::tokenize::split_top_level(sep, &chars)
        .into_iter()
        .map(|piece| piece.into_iter().collect())
        .collect()
}

/// `splitEntryTokens`.
pub fn split_entry_tokens(entry: &str) -> Vec<String> {
    split_top_level('/', entry)
        .into_iter()
        .map(|t| trim(&t))
        .filter(|t| !t.is_empty())
        .collect()
}

fn take_until_slide_mark(text: &str) -> String {
    let mut acc = String::new();
    for c in text.chars() {
        if is_slide_mark_char(c) {
            break;
        }
        acc.push(c);
    }
    acc
}

/// `parseHeadBreak`.
pub fn parse_head_break(token: &str) -> bool {
    let t = strip_prefix_directives(token);
    if is_slide_text(&t) {
        take_until_slide_mark(&t).contains('b')
    } else {
        t.contains('b')
    }
}

/// `parseSlideSegmentBreak`.
pub fn parse_slide_segment_break(token: &str) -> bool {
    let t = strip_prefix_directives(token);
    let chars: Vec<char> = t.chars().collect();
    let mut seen_slide = false;
    for (i, c) in chars.iter().copied().enumerate() {
        if c == 'b' && seen_slide && (i + 1 == chars.len() || chars[i + 1] == '[') {
            return true;
        }
        seen_slide |= is_slide_mark_char(c);
    }
    false
}

/// `parseHSpeedDirective`.
pub fn parse_h_speed_directive(text: &str, current: Rat) -> Rat {
    let t = trim(text);
    if !t.starts_with("<H") {
        current
    } else {
        let body = t.split('>').next().unwrap_or("");
        let body = body.chars().skip(2).collect::<String>();
        let value_text = if body.starts_with("S*") {
            body.chars().skip(2).collect::<String>()
        } else {
            body
        };
        parse_rat_def(&value_text, current)
    }
}

/// `applyInlineDirective`.
pub fn apply_inline_directive(
    bpm: Rat,
    divisor: Natural,
    h_speed: Rat,
    segment: &str,
) -> (Rat, Natural, Rat, String) {
    let mut bpm = bpm;
    let mut divisor = divisor;
    let mut h_speed = h_speed;
    let mut t = trim(segment);
    for _ in 0..=segment.chars().count() {
        t = trim(&t);
        if t.starts_with('(') {
            let (inside, rest) = t[1..].split_once(')').unwrap_or((&t[1..], ""));
            bpm = parse_rat_def(inside, bpm);
            t = rest.to_string();
        } else if t.starts_with('{') {
            let (inside, rest) = t[1..].split_once('}').unwrap_or((&t[1..], ""));
            divisor = parse_nat_def(inside, divisor);
            t = rest.to_string();
        } else if t.starts_with("<H") {
            h_speed = parse_h_speed_directive(&t, h_speed);
            t = t
                .split_once('>')
                .map(|(_, rest)| rest.to_string())
                .unwrap_or_default();
        } else if let Some(index) = t.find(['(', '{']) {
            t = t[index..].to_string();
        } else {
            break;
        }
    }
    (bpm, divisor, h_speed, t)
}

/// `mkRawToken`.
pub fn mk_raw_token(
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
    token: &str,
) -> RawNoteToken {
    let t = trim(token);
    let kind = infer_kind(&t);
    let parsed_text = if kind == RawNoteKind::Slide {
        sanitize_slide_token(&t)
    } else {
        t.clone()
    };
    let slot = leading_digit(&parsed_text).and_then(|n| OuterSlot::of_index(n.saturating_sub(1)));
    let sensor_pos = touch_area_to_sensor_area(&t);
    let slide_body = if kind == RawNoteKind::Slide {
        parse_slide_body_from_text(&parsed_text).ok()
    } else {
        None
    };
    let length = parse_duration_spec(bpm.clone(), &t);
    let star_wait = if kind == RawNoteKind::Slide {
        parse_star_wait_spec(bpm.clone(), &t)
    } else {
        None
    };
    let is_break = parse_head_break(&t);
    let is_ex = t.contains('x');
    let is_hanabi = t.contains('f');
    let is_slide_no_head = t.contains('!') || t.contains('?');
    let is_force_star = t.contains('$');
    let is_fake_rotate = t.chars().filter(|c| *c == '$').count() >= 2;
    let is_slide_break = parse_slide_segment_break(&t);
    RawNoteToken {
        raw_text: parsed_text,
        kind,
        timing,
        bpm,
        h_speed,
        divisor,
        slot,
        sensor_pos,
        slide_body,
        length,
        star_wait,
        is_break,
        is_ex,
        is_hanabi,
        is_slide_no_head,
        is_force_star,
        is_fake_rotate,
        is_slide_break,
        source_group_id: None,
        source_group_index: None,
        source_group_size: None,
        source_pos: None,
    }
}

// Segment construction and iteration live in the shared grammar kernel.
type ContinuousChainSegment = slide_chain::Segment<String>;

fn parse_continuous_slide_segments_core(
    raw_text: &str,
    current_start: char,
    chars: &[char],
) -> Result<Vec<ContinuousChainSegment>, ParseError> {
    match slide_chain::parse_segments(chars, current_start, |text: Vec<char>| {
        text.into_iter().collect::<String>()
    }) {
        Ok(segments) => Ok(segments),
        Err(error) => {
            let message = match error {
                ChainError::InvalidSyntax => "invalid connected slide syntax",
                ChainError::FreshNumericHead => {
                    "connected slide chain cannot contain a fresh numeric head"
                }
                ChainError::UnterminatedTiming => "unterminated slide timing spec",
            };
            Err(syntax_error(raw_text, message))
        }
    }
}

fn parse_continuous_slide_segments(
    token: &str,
) -> Result<Option<Vec<ContinuousChainSegment>>, ParseError> {
    let sanitized = sanitize_slide_token(token);
    let chars: Vec<char> = sanitized.chars().collect();
    match chars.split_first() {
        None => Ok(None),
        Some((start, rest)) => {
            if !start.is_ascii_digit() {
                Ok(None)
            } else {
                let segments = parse_continuous_slide_segments_core(&sanitized, *start, rest)?;
                if segments.len() <= 1 {
                    Ok(None)
                } else {
                    Ok(Some(segments))
                }
            }
        }
    }
}

fn segment_bar_count(raw_text: &str) -> Result<usize, ParseError> {
    let shape = detect_shape_from_text(raw_text)?;
    if crate::simai::shape::shape_kind(&shape) == SlideKind::Wifi {
        return Err(syntax_error(
            raw_text,
            "wifi slide cannot be part of a connection slide group",
        ));
    }
    slide_bar_count_for_shape(&shape)
        .ok_or_else(|| syntax_error(raw_text, "missing slide table for connected slide segment"))
}

fn apply_shared_slide_flags(
    base: &RawNoteToken,
    token: &RawNoteToken,
    is_headless: bool,
) -> RawNoteToken {
    RawNoteToken {
        is_break: base.is_break,
        is_ex: base.is_ex,
        is_hanabi: base.is_hanabi,
        is_slide_no_head: is_headless,
        is_force_star: base.is_force_star,
        is_fake_rotate: base.is_fake_rotate,
        is_slide_break: base.is_slide_break,
        ..token.clone()
    }
}

fn tag_connected_group(
    group_id: Natural,
    size: usize,
    tokens: Vec<RawNoteToken>,
) -> Vec<RawNoteToken> {
    tokens
        .into_iter()
        .enumerate()
        .map(|(index, tok)| RawNoteToken {
            source_group_id: Some(group_id.clone()),
            source_group_index: Some((index).into()),
            source_group_size: Some((size).into()),
            ..tok
        })
        .collect()
}

fn build_whole_duration_chain_tokens(
    group_id: Natural,
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
    base: &RawNoteToken,
    segments: &[ContinuousChainSegment],
) -> Result<Vec<RawNoteToken>, ParseError> {
    let total_length = base.length.clone().ok_or_else(|| {
        syntax_error(
            &base.raw_text,
            "connected slide chain requires an explicit timing spec",
        )
    })?;
    let mut bar_counts = Vec::with_capacity(segments.len());
    for segment in segments {
        bar_counts.push(segment_bar_count(&segment.raw_text)?);
    }
    let total_bars: Natural = bar_counts.iter().copied().map(Natural::from).sum();
    if total_bars == 0 {
        return Err(syntax_error(
            &base.raw_text,
            "connected slide chain has no measurable segments",
        ));
    }
    let base_micros = Rat::from_int(total_length.to_micros());
    let mut elapsed_bars = Natural::default();
    let tokens: Vec<RawNoteToken> = segments
        .iter()
        .zip(bar_counts.iter())
        .enumerate()
        .map(|(i, (segment, bars))| {
            let seg_tok = mk_raw_token(
                timing.clone(),
                bpm.clone(),
                h_speed.clone(),
                divisor.clone(),
                &segment.raw_text,
            );
            let start = time::duration_from_rat_micros(
                base_micros.mul(Rat::new(&elapsed_bars, &total_bars)),
            );
            elapsed_bars += *bars;
            let end = time::duration_from_rat_micros(
                base_micros.mul(Rat::new(&elapsed_bars, &total_bars)),
            );
            let seg_len = end - start;
            let seg_wait = if i == 0 { base.star_wait.clone() } else { None };
            let tok = RawNoteToken {
                length: Some(seg_len),
                star_wait: seg_wait,
                ..seg_tok
            };
            let headless = if i == 0 { base.is_slide_no_head } else { true };
            apply_shared_slide_flags(base, &tok, headless)
        })
        .collect();
    let size = tokens.len();
    Ok(tag_connected_group(group_id, size, tokens))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChainTimingLayout {
    PerSegment,
    OverallFinal,
}

fn classify_chain_timing_layout(
    raw_text: &str,
    segments: &[ContinuousChainSegment],
) -> Result<ChainTimingLayout, ParseError> {
    let flags: Vec<bool> = segments.iter().map(|s| s.has_timing).collect();
    if flags.iter().all(|f| *f) {
        Ok(ChainTimingLayout::PerSegment)
    } else {
        match flags.last() {
            Some(true) => {
                let rest_all_false = flags[..flags.len() - 1].iter().all(|f| !*f);
                if rest_all_false {
                    Ok(ChainTimingLayout::OverallFinal)
                } else {
                    Err(syntax_error(raw_text, "invalid connected slide timing layout"))
                }
            }
            Some(false) => Err(syntax_error(raw_text,
                "connected slide chain requires either per-segment timing or a final overall timing spec")),
            None => Err(syntax_error(raw_text, "invalid connected slide timing layout")),
        }
    }
}

fn expand_continuous_chain_token(
    group_id: Natural,
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
    token: &str,
) -> Result<Vec<RawNoteToken>, ParseError> {
    let base = mk_raw_token(
        timing.clone(),
        bpm.clone(),
        h_speed.clone(),
        divisor.clone(),
        token,
    );
    if base.kind != RawNoteKind::Slide {
        return Ok(vec![base]);
    }
    match parse_continuous_slide_segments(token)? {
        None => Ok(vec![base]),
        Some(segments) => {
            classify_chain_timing_layout(&base.raw_text, &segments)?;
            build_whole_duration_chain_tokens(
                group_id, timing, bpm, h_speed, divisor, &base, &segments,
            )
        }
    }
}

fn same_head_group_parts(token: &str) -> Vec<String> {
    split_top_level('*', token)
        .into_iter()
        .map(|t| trim(&t))
        .filter(|t| !t.is_empty())
        .collect()
}

fn same_head_head_prefix(token: &str) -> String {
    let t = trim(&strip_prefix_directives(token));
    let chars: Vec<char> = t.chars().collect();
    match chars.split_first() {
        None => String::new(),
        Some((first, rest)) => {
            if is_touch_area_char(*first) {
                match (first, rest.split_first()) {
                    ('C', _) => "C".to_string(),
                    (_, Some((digit, _))) if digit.is_ascii_digit() => {
                        format!("{}{}", first, digit)
                    }
                    _ => first.to_string(),
                }
            } else if first.is_ascii_digit() {
                first.to_string()
            } else {
                String::new()
            }
        }
    }
}

fn expand_same_head_group(
    group_id: Natural,
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
    token: &str,
) -> Result<Vec<RawNoteToken>, ParseError> {
    let parts = same_head_group_parts(token);
    let Some((first, rest)) = parts.split_first() else {
        return Ok(Vec::new());
    };
    let prefix = same_head_head_prefix(first);
    let mut out = expand_continuous_chain_token(
        group_id.clone(),
        timing.clone(),
        bpm.clone(),
        h_speed.clone(),
        divisor.clone(),
        first,
    )?;
    for part in rest {
        let rebuilt = format!("{}{}", prefix, part);
        let tokens = expand_continuous_chain_token(
            group_id.clone() + out.len(),
            timing.clone(),
            bpm.clone(),
            h_speed.clone(),
            divisor.clone(),
            &rebuilt,
        )?;
        out.extend(tokens.into_iter().map(|tok| RawNoteToken {
            is_slide_no_head: true,
            ..tok
        }));
    }
    Ok(out)
}

fn expand_token_list(
    base_group_id: Natural,
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
    texts: &[String],
) -> Result<Vec<RawNoteToken>, ParseError> {
    let mut out = Vec::new();
    for tok_text in texts {
        if infer_kind(tok_text) != RawNoteKind::Unknown && tok_text.contains(['K', '@', 'c', 'm']) {
            return Err(syntax_error(
                tok_text,
                "unsupported extended slide or note modifier (K, @, c, m)",
            ));
        }
        let idx = out.len();
        let current = if tok_text.contains('*') {
            expand_same_head_group(
                base_group_id.clone() + idx,
                timing.clone(),
                bpm.clone(),
                h_speed.clone(),
                divisor.clone(),
                tok_text,
            )?
        } else {
            expand_continuous_chain_token(
                base_group_id.clone() + idx,
                timing.clone(),
                bpm.clone(),
                h_speed.clone(),
                divisor.clone(),
                tok_text,
            )?
        };
        out.extend(current);
    }
    Ok(out)
}

fn expand_entry_text(
    timing: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
    entry: &str,
) -> Result<Vec<RawNoteToken>, ParseError> {
    let text = trim(entry);
    let entries = if text.chars().count() == 2 && text.chars().all(|c| digit_to_nat(c).is_some()) {
        text.chars().map(|c| c.to_string()).collect()
    } else {
        split_entry_tokens(&text)
    };
    expand_token_list((0).into(), timing, bpm, h_speed, divisor, &entries)
}

fn parse_segment_notes_exact(
    segment: &str,
    start: Rat,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
) -> Result<Vec<RawNoteToken>, ParseError> {
    let normalized: String = segment
        .chars()
        // Lean's Char.isWhitespace excludes ASCII form feed.
        .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
        .collect();
    let increment = if Rat::zero().lt(&bpm) {
        time::bpm_beat_micros_rat(bpm.clone()).div(Rat::from_int(32))
    } else {
        Rat::from_int(1000)
    };
    let mut current_time = start;
    let mut acc = Vec::new();
    for part in normalized.split(char::from(96)).filter(|p| !p.is_empty()) {
        acc.extend(expand_entry_text(
            time::point_from_rat_micros(current_time.clone()),
            bpm.clone(),
            h_speed.clone(),
            divisor.clone(),
            part,
        )?);
        current_time = current_time.add(&increment);
    }
    Ok(acc)
}

pub fn parse_segment_notes(
    segment: &str,
    time: TimePoint,
    bpm: Rat,
    h_speed: Rat,
    divisor: Natural,
) -> Result<Vec<RawNoteToken>, ParseError> {
    parse_segment_notes_exact(
        segment,
        Rat::from_int(time.to_micros()),
        bpm,
        h_speed,
        divisor,
    )
}

/// `parseSegments`: the existing accumulator is in reverse source order.
pub fn parse_segments(
    segments: &[String],
    start: TimePoint,
    mut bpm: Rat,
    mut h_speed: Rat,
    mut divisor: Natural,
    acc_rev: &[RawNoteToken],
) -> Result<Vec<RawNoteToken>, ParseError> {
    let mut acc: Vec<_> = acc_rev.iter().rev().cloned().collect();
    let mut time = Rat::from_int(start.to_micros());
    for segment in segments {
        let (new_bpm, new_divisor, new_speed, body) =
            apply_inline_directive(bpm, divisor, h_speed, segment);
        bpm = new_bpm;
        divisor = new_divisor;
        h_speed = new_speed;
        acc.extend(parse_segment_notes_exact(
            &body,
            time.clone(),
            bpm.clone(),
            h_speed.clone(),
            divisor.clone(),
        )?);
        if Rat::zero().lt(&bpm) && divisor > 0 {
            time = time
                .add(time::bpm_measure_micros_rat(bpm.clone()).div(Rat::from_int(divisor.clone())));
        }
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_inference() {
        assert_eq!(infer_kind("1"), RawNoteKind::Tap);
        assert_eq!(infer_kind("1h[4:1]"), RawNoteKind::Hold);
        assert_eq!(infer_kind("1-3"), RawNoteKind::Slide);
        assert_eq!(infer_kind("A1"), RawNoteKind::Touch);
        assert_eq!(infer_kind("Ch"), RawNoteKind::TouchHold);
        assert_eq!(infer_kind(""), RawNoteKind::Rest);
        for token in ["{4", "(120", "<-5[4:1]"] {
            assert_eq!(infer_kind(token), RawNoteKind::Rest);
        }
    }

    #[test]
    fn touch_area() {
        assert_eq!(touch_area_to_sensor_area("C"), Some(SensorArea::C));
        assert_eq!(touch_area_to_sensor_area("A3"), Some(SensorArea::A3));
        assert_eq!(touch_area_to_sensor_area("B8"), Some(SensorArea::B8));
        assert_eq!(touch_area_to_sensor_area("A9"), None);
    }

    #[test]
    fn split_entry() {
        assert_eq!(split_entry_tokens("1/2/3"), vec!["1", "2", "3"]);
        assert_eq!(split_entry_tokens("1-3[4:1]/2"), vec!["1-3[4:1]", "2"]);
    }

    #[test]
    fn segment_notes_tap() {
        let toks = parse_segment_notes(
            "1",
            TimePoint::zero(),
            Rat::from_int(120),
            Rat::one(),
            (4).into(),
        )
        .unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, RawNoteKind::Tap);
        assert_eq!(toks[0].slot, Some(OuterSlot::S1));
    }

    #[test]
    fn segments_advance_time() {
        let toks = parse_segments(
            &["1".to_string(), "2".to_string()],
            TimePoint::zero(),
            Rat::from_int(120),
            Rat::one(),
            (4).into(),
            &[],
        )
        .unwrap();
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[1].timing.to_micros(), 500_000);
    }

    #[test]
    fn inline_bpm_directive() {
        let (bpm, _, _, body) =
            apply_inline_directive(Rat::from_int(120), (4).into(), Rat::one(), "(240)1");
        assert_eq!(bpm, Rat::from_int(240));
        assert_eq!(body, "1");
    }
}
