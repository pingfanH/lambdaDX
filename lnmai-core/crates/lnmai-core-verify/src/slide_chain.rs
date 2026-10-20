//! Continuous-slide grammar shared with the runtime tokenizer.
//!
//! Scans use a borrowed character slice and increasing cursors. Errors are
//! finite; successful segments are ranges in the input. No char-valued switch,
//! recursion, suffix cloning, timing parser, or whole-chain callback occurs here.
//! The only generic operation encodes ONE already-built Vec<char> as text.
//!
//! Source-only delivery: not compiled or extracted. See OPEN-OBLIGATIONS.md.

use crate::tokenize::is_slide_mark_char;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainError {
    InvalidSyntax,
    FreshNumericHead,
    UnterminatedTiming,
}

/// head is synthetic; [begin, end) contains the original shape and optional
/// bracket suffix, including '[' and the FIRST closing ']'. Bounds are character
/// positions, never UTF-8 byte positions. Consecutive plans partition the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentPlan {
    pub head: char,
    pub begin: usize,
    pub end: usize,
    pub has_timing: bool,
}

/// The runtime uses Segment<String>; the encoder cannot change has_timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment<Text> {
    pub raw_text: Text,
    pub has_timing: bool,
}

/// Deliberately accepts ASCII 0 and 9 as well as 1..8, matching Char.isDigit.
/// Equality tests avoid the extractor's unsupported char-valued switch.
pub fn is_chain_digit(c: char) -> bool {
    c == '0'
        || c == '1'
        || c == '2'
        || c == '3'
        || c == '4'
        || c == '5'
        || c == '6'
        || c == '7'
        || c == '8'
        || c == '9'
}

/// Success consumes exactly one character. An out-of-range cursor returns the
/// original generic syntax error, not a bounds panic.
pub fn read_digit(chars: &[char], cursor: usize) -> Result<(char, usize), ChainError> {
    if cursor < chars.len() {
        let c = chars[cursor];
        if is_chain_digit(c) {
            return Ok((c, cursor + 1));
        }
    }
    Err(ChainError::InvalidSyntax)
}

/// cursor is immediately after op; parse_plans checks the slide-mark predicate.
/// Return the endpoint and next cursor. The original-input range preserves the
/// spelling of the shape without reconstruction.
pub fn read_shape(
    chars: &[char],
    op: char,
    mut cursor: usize,
) -> Result<(char, usize), ChainError> {
    if op == 'V' {
        cursor = match read_digit(chars, cursor) {
            Ok((_middle, next)) => next,
            Err(error) => return Err(error),
        };
        read_digit(chars, cursor)
    } else {
        if (op == 'p' || op == 'q') && cursor < chars.len() && chars[cursor] == op {
            cursor += 1;
        }
        read_digit(chars, cursor)
    }
}

/// Exact readBracketSuffixFuel order: empty first, read a character, accept ']'
/// EVEN WITH ZERO FUEL, then check/decrement fuel and continue. The opening '['
/// has already been consumed. Return the position after ']'.
pub fn timing_end_with_fuel(
    chars: &[char],
    mut cursor: usize,
    mut fuel: usize,
) -> Result<usize, ChainError> {
    while cursor < chars.len() {
        let c = chars[cursor];
        cursor += 1;
        if c == ']' {
            return Ok(cursor);
        }
        if fuel == 0 {
            return Err(ChainError::UnterminatedTiming);
        }
        fuel -= 1;
    }
    Err(ChainError::UnterminatedTiming)
}

/// Uses remaining characters without +1. Equality with the original length+1
/// wrapper needs the explicit fuel-stability proof in the Lean draft.
pub fn timing_end(chars: &[char], cursor: usize) -> Result<usize, ChainError> {
    if cursor > chars.len() {
        return Err(ChainError::UnterminatedTiming);
    }
    timing_end_with_fuel(chars, cursor, chars.len() - cursor)
}

/// Outer fuel: empty succeeds before the fuel guard; nonempty/fuel=0 returns
/// InvalidSyntax before digit/mark inspection. A later error discards all plans,
/// as with the original Except recursion.
#[allow(clippy::question_mark)]
pub fn parse_plans_with_fuel(
    chars: &[char],
    mut current_start: char,
    mut fuel: usize,
) -> Result<Vec<SegmentPlan>, ChainError> {
    let mut plans = Vec::new();
    let mut cursor = 0usize;
    while cursor < chars.len() {
        if fuel == 0 {
            return Err(ChainError::InvalidSyntax);
        }
        fuel -= 1;
        let begin = cursor;
        let op = chars[cursor];
        cursor += 1;
        if is_chain_digit(op) {
            return Err(ChainError::FreshNumericHead);
        }
        if !is_slide_mark_char(op) {
            return Err(ChainError::InvalidSyntax);
        }
        let (end_char, next) = match read_shape(chars, op, cursor) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        cursor = next;
        let mut has_timing = false;
        if cursor < chars.len() && chars[cursor] == '[' {
            has_timing = true;
            cursor += 1;
            cursor = match timing_end(chars, cursor) {
                Ok(next) => next,
                Err(error) => return Err(error),
            };
        }
        plans.push(SegmentPlan {
            head: current_start,
            begin,
            end: cursor,
            has_timing,
        });
        current_start = end_char;
    }
    Ok(plans)
}

/// Runtime uses remaining length, not remaining length+1. Each complete segment
/// consumes at least its operator and one digit. Fuel sufficiency and equality
/// with the original wrapper remain explicit proof obligations.
pub fn parse_plans(chars: &[char], current_start: char) -> Result<Vec<SegmentPlan>, ChainError> {
    parse_plans_with_fuel(chars, current_start, chars.len())
}

/// Only indexing/push. For scanner-produced plans: begin < end <= chars.len().
/// Prove those bounds and room for the synthetic head before calling this helper;
/// arbitrary hand-built plans are not checked here. No UTF-8 slicing occurs.
pub fn render_plan(chars: &[char], plan: SegmentPlan) -> Vec<char> {
    let mut text = Vec::new();
    text.push(plan.head);
    let mut cursor = plan.begin;
    while cursor < plan.end {
        text.push(chars[cursor]);
        cursor += 1;
    }
    text
}

/// Point encoder law: decode(encode(v)) = String.ofList(v). Both traversal and
/// segment construction are in the kernel, not hidden inside that callback.
pub fn encode_plans<Text, Encode>(
    chars: &[char],
    plans: &[SegmentPlan],
    encode: Encode,
) -> Vec<Segment<Text>>
where
    Encode: Fn(Vec<char>) -> Text,
{
    let mut segments = Vec::new();
    let mut index = 0usize;
    while index < plans.len() {
        let plan = plans[index];
        let text = render_plan(chars, plan);
        segments.push(Segment {
            raw_text: encode(text),
            has_timing: plan.has_timing,
        });
        index += 1;
    }
    segments
}

/// Runtime entry: scanning, output order, range copies and record construction
/// are actual Rust kernel code. String encoding is the only callback.
pub fn parse_segments<Text, Encode>(
    chars: &[char],
    current_start: char,
    encode: Encode,
) -> Result<Vec<Segment<Text>>, ChainError>
where
    Encode: Fn(Vec<char>) -> Text,
{
    match parse_plans(chars, current_start) {
        Ok(plans) => Ok(encode_plans(chars, &plans, encode)),
        Err(error) => Err(error),
    }
}
