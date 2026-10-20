//! Character scans and field routing shared with the runtime timing parser.
//! String conversion, exact numeric parsing, and quantization stay in runtime.

/// Preserve the reference's front insertion for a nested opening bracket.
/// Explicit copying avoids relying on the extractor's Vec::insert model.
fn prepend_open(current: Vec<char>) -> Vec<char> {
    let mut result = Vec::new();
    result.push('[');
    let mut index = 0usize;
    while index < current.len() {
        result.push(current[index]);
        index += 1;
    }
    result
}

/// Scan complete brackets, including the reference's malformed-input behavior.
/// Nested '[' is prepended; unmatched ']' and an unfinished tail are ignored.
pub fn extract_brackets(chars: &[char]) -> Vec<Vec<char>> {
    let mut result = Vec::new();
    let mut inside = false;
    let mut current = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        if c == '[' {
            if inside {
                current = prepend_open(current);
            } else {
                inside = true;
                current = Vec::new();
            }
        } else if c == ']' {
            if inside {
                inside = false;
                result.push(current);
                current = Vec::new();
            }
        } else if inside {
            current.push(c);
        }
        index += 1;
    }
    result
}

#[derive(Clone, Copy)]
pub enum DurationRoute {
    Reject,
    CurrentRatioOrSeconds { timing: usize },
    SecondsOnly { timing: usize },
    CustomRatioOrSeconds { bpm: usize, timing: usize },
    CustomRatioOnly { bpm: usize, timing: usize },
}

/// Classify fields after splitting on '#', before trimming or numeric parsing.
/// Only a strictly empty first field selects seconds-only for two fields.
// Vec::len has an extraction model; Vec::is_empty is an opaque external call.
#[allow(clippy::len_zero)]
pub fn duration_route(fields: &[Vec<char>]) -> DurationRoute {
    if fields.len() == 1 {
        DurationRoute::CurrentRatioOrSeconds { timing: 0 }
    } else if fields.len() == 2 {
        if fields[0].len() == 0 {
            DurationRoute::SecondsOnly { timing: 1 }
        } else {
            DurationRoute::CustomRatioOrSeconds { bpm: 0, timing: 1 }
        }
    } else if fields.len() == 3 {
        DurationRoute::CurrentRatioOrSeconds { timing: 2 }
    } else if fields.len() == 4 {
        DurationRoute::CustomRatioOnly { bpm: 2, timing: 3 }
    } else {
        DurationRoute::Reject
    }
}
