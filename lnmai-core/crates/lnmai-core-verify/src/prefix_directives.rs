//! One Simai prefix directive, using Unicode scalar indices only.
//!
//! Mirrors stripPrefixDirectives: ASCII trim, inspect the first character,
//! discard through the FIRST corresponding closer, then ASCII trim once more.
//! An unclosed directive yields empty text; a second directive is not parsed.
//! All scanning, branch decisions and output copying live in this module.
//! String decoding/encoding stays at the runtime boundary.
//!
//! Source draft: not compiled or extracted. Helpers taking a range are private;
//! their caller maintains begin <= end <= chars.len().

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixRange {
    pub begin: usize,
    pub end: usize,
}

/// Exactly Lean Char.isWhitespace, not Rust char::is_whitespace or
/// is_ascii_whitespace. In particular VT, FF, NBSP and U+3000 are retained.
pub fn is_prefix_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\r' || c == '\n'
}

/// If-comparisons avoid a char-valued switch in the extractor.
pub fn directive_closer(c: char) -> Option<char> {
    if c == '{' {
        Some('}')
    } else if c == '(' {
        Some(')')
    } else if c == '<' {
        Some('>')
    } else {
        None
    }
}

fn trim_range(chars: &[char], mut begin: usize, mut end: usize) -> PrefixRange {
    while begin < end {
        if !is_prefix_whitespace(chars[begin]) {
            break;
        }
        begin += 1;
    }
    while begin < end {
        let previous = end - 1;
        if !is_prefix_whitespace(chars[previous]) {
            break;
        }
        end = previous;
    }
    PrefixRange { begin, end }
}

/// Return end when the closer is absent, so the next trim/copy returns empty.
/// Starts at the opener, just as splitOn in the original Lean definition.
fn after_first_closer(chars: &[char], mut cursor: usize, end: usize, closer: char) -> usize {
    while cursor < end {
        let c = chars[cursor];
        cursor += 1;
        if c == closer {
            return cursor;
        }
    }
    end
}

/// Total public entry for a borrowed Unicode-scalar slice. Both empty and
/// all-whitespace inputs take the empty branch before indexing the head.
pub fn strip_prefix_range(chars: &[char]) -> PrefixRange {
    let trimmed = trim_range(chars, 0, chars.len());
    if trimmed.begin == trimmed.end {
        return trimmed;
    }
    match directive_closer(chars[trimmed.begin]) {
        None => trimmed,
        Some(closer) => {
            let begin = after_first_closer(chars, trimmed.begin, trimmed.end, closer);
            trim_range(chars, begin, trimmed.end)
        }
    }
}

/// Copy the selected range in order. No byte offsets, lossy decoding, nested
/// prefix removal, flag filtering, or whole-prefix callback is involved.
pub fn strip_prefix_chars(chars: &[char]) -> Vec<char> {
    let selected = strip_prefix_range(chars);
    let mut result = Vec::new();
    let mut cursor = selected.begin;
    while cursor < selected.end {
        result.push(chars[cursor]);
        cursor += 1;
    }
    result
}
