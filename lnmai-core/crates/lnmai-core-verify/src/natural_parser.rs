//! Decimal syntax shared with `Natural::from_lean_str`.
//! The accumulator stays generic so the runtime retains unbounded BigUint
//! arithmetic. UTF-8 decoding and allocation are outside this character kernel.

/// Lean `Char.isDigit` accepts precisely the ten ASCII decimal digits.
pub fn decimal_digit(c: char) -> Option<u8> {
    // Explicit comparisons also avoid a char-valued switch in Charon/Aeneas.
    if c == '0' {
        Some(0)
    } else if c == '1' {
        Some(1)
    } else if c == '2' {
        Some(2)
    } else if c == '3' {
        Some(3)
    } else if c == '4' {
        Some(4)
    } else if c == '5' {
        Some(5)
    } else if c == '6' {
        Some(6)
    } else if c == '7' {
        Some(7)
    } else if c == '8' {
        Some(8)
    } else if c == '9' {
        Some(9)
    } else {
        None
    }
}

/// Parse decimal digits with a single underscore allowed only between digits.
/// `zero` constructs zero; `append(value, digit)` computes value * 10 + digit.
/// The scan itself rejects empty input, signs, whitespace, non-ASCII digits,
/// and leading, trailing, or repeated underscores.
#[allow(clippy::question_mark)]
pub fn parse_decimal<T, Z, F>(chars: &[char], zero: Z, append: F) -> Option<T>
where
    Z: Fn() -> T,
    F: Fn(T, u8) -> T,
{
    let mut value = zero();
    let mut last_was_digit = false;
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        if c == '_' {
            if !last_was_digit {
                return None;
            }
            last_was_digit = false;
        } else {
            let digit = match decimal_digit(c) {
                Some(digit) => digit,
                None => return None,
            };
            value = append(value, digit);
            last_was_digit = true;
        }
        index += 1;
    }
    if last_was_digit {
        Some(value)
    } else {
        None
    }
}
