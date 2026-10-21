//! Text scans used by the slide parser's command and sanitizer adapters.
//! UTF-8 decoding, String construction, and their allocation bounds stay in the
//! runtime. These kernels preserve arbitrary character sequences and scan order.

use crate::slide_parser::{keep_slide_char, CommandFacts};
use crate::slide_parser_frontend::{contains_char, contains_double};

/// Observe the whole text, including conflicting commands and malformed input.
pub fn command_facts(chars: &[char]) -> CommandFacts {
    CommandFacts {
        line: contains_char(chars, '-'),
        circle_right: contains_char(chars, '>'),
        circle_left: contains_char(chars, '<'),
        circle_up: contains_char(chars, '^'),
        v: contains_char(chars, 'v'),
        pp: contains_double(chars, 'p'),
        qq: contains_double(chars, 'q'),
        p: contains_char(chars, 'p'),
        q: contains_char(chars, 'q'),
        s: contains_char(chars, 's'),
        z: contains_char(chars, 'z'),
        turn: contains_char(chars, 'V'),
        wifi: contains_char(chars, 'w'),
    }
}

/// Remove only the six flag characters, preserving all remaining characters in
/// order. Filtering may create adjacent pp/qq pairs, so callers scan the result.
pub fn sanitize_slide_chars(chars: &[char]) -> Vec<char> {
    let mut result = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        if keep_slide_char(c) {
            result.push(c);
        }
        index += 1;
    }
    result
}
