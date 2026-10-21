//! Character-level decimal timing numbers, shared with the runtime parser.
//! Trimming/UTF-8 decoding and unbounded arithmetic remain in the caller.

use crate::natural_parser::decimal_digit;

/// Only the exact natural-number primitives used by the scan.
pub trait DecimalOps<T> {
    fn zero(&self) -> T;
    fn one(&self) -> T;
    fn append(&self, value: T, digit: u8) -> T;
}

pub struct Parts<T> {
    pub negative: bool,
    pub whole: T,
    pub fraction: Option<(T, T)>,
}

/// Scan the characters after the first decimal point. Empty fractions denote
/// zero over one; underscores and all non-ASCII digits are rejected.
#[allow(clippy::question_mark)]
pub fn fraction<T, O: DecimalOps<T>>(chars: &[char], mut index: usize, ops: &O) -> Option<(T, T)> {
    if index > chars.len() {
        return None;
    }
    let mut value = ops.zero();
    let mut denominator = ops.one();
    while index < chars.len() {
        let digit = match decimal_digit(chars[index]) {
            Some(digit) => digit,
            None => return None,
        };
        value = ops.append(value, digit);
        denominator = ops.append(denominator, 0);
        index += 1;
    }
    Some((value, denominator))
}

/// Parse a trimmed timing number. The whole part follows Lean `toNat?`,
/// including single internal underscores; the fractional part is digits only.
/// A single leading minus is accepted, while plus signs and empty wholes fail.
#[allow(clippy::question_mark)]
pub fn parse<T, O: DecimalOps<T>>(chars: &[char], ops: &O) -> Option<Parts<T>> {
    if chars.is_empty() {
        return None;
    }
    let negative = chars[0] == '-';
    let mut index = if negative { 1 } else { 0 };
    let mut whole = ops.zero();
    let mut last_was_digit = false;
    while index < chars.len() {
        let c = chars[index];
        if c == '.' {
            if !last_was_digit {
                return None;
            }
            index += 1;
            return match fraction(chars, index, ops) {
                Some(fraction) => Some(Parts {
                    negative,
                    whole,
                    fraction: Some(fraction),
                }),
                None => None,
            };
        } else if c == '_' {
            if !last_was_digit {
                return None;
            }
            last_was_digit = false;
        } else {
            let digit = match decimal_digit(c) {
                Some(digit) => digit,
                None => return None,
            };
            whole = ops.append(whole, digit);
            last_was_digit = true;
        }
        index += 1;
    }
    if last_was_digit {
        Some(Parts {
            negative,
            whole,
            fraction: None,
        })
    } else {
        None
    }
}
