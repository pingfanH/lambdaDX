//! Character-level Simai tokenization shared with the runtime parser.
//! UTF-8 decoding and String construction remain in the runtime adapter.

pub fn is_touch_area_char(c: char) -> bool {
    c == 'A' || c == 'B' || c == 'C' || c == 'D' || c == 'E'
}

pub fn is_slide_mark_char(c: char) -> bool {
    c == '-'
        || c == '^'
        || c == 'v'
        || c == '<'
        || c == '>'
        || c == 'V'
        || c == 'p'
        || c == 'q'
        || c == 's'
        || c == 'z'
        || c == 'w'
        || c == 'K'
}

/// Preserve empty pieces and treat separators inside brackets as literal text.
/// Brackets themselves have priority even when the separator is '[' or ']'.
pub fn split_top_level(sep: char, chars: &[char]) -> Vec<Vec<char>> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut current = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        if c == '[' {
            depth += 1;
            current.push(c);
        } else if c == ']' {
            depth = depth.saturating_sub(1);
            current.push(c);
        } else if c == sep && depth == 0 {
            result.push(current);
            current = Vec::new();
        } else {
            current.push(c);
        }
        index += 1;
    }
    result.push(current);
    result
}
