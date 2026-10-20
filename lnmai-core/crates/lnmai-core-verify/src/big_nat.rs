//! Exact nonnegative integers as little-endian 64-bit limb vectors.
//!
//! This is the shared implementation of the runtime Natural type: the same
//! code is extracted by Charon/Aeneas, so every operation the runtime performs
//! is a kernel function rather than a third-party library call. The canonical
//! form stores the value as sum limbs[i] * 2^(64*i) with no trailing zero
//! limbs (the empty vector is zero), and every operation preserves that form.
//!
//! Subtraction, division and remainder are total, matching Lean Nat: a - b
//! floors at zero, a / 0 is zero and a % 0 is a.

#![allow(clippy::len_zero)]

/// Little-endian 64-bit limbs in canonical form; empty means zero.
#[derive(Clone)]
pub struct BigNat {
    pub limbs: Vec<u64>,
}

/// Remove trailing zero limbs, restoring the canonical form.
pub fn normalize(limbs: Vec<u64>) -> BigNat {
    let mut length = limbs.len();
    while length > 0 && limbs[length - 1] == 0 {
        length -= 1;
    }
    let mut normalized: Vec<u64> = Vec::new();
    let mut index = 0usize;
    while index < length {
        normalized.push(limbs[index]);
        index += 1;
    }
    BigNat { limbs: normalized }
}

/// Zero.
pub fn zero() -> BigNat {
    BigNat { limbs: Vec::new() }
}

/// One.
#[allow(clippy::vec_init_then_push)]
pub fn one() -> BigNat {
    let mut limbs: Vec<u64> = Vec::new();
    limbs.push(1u64);
    BigNat { limbs }
}

/// The value of a single machine word.
pub fn from_u64(value: u64) -> BigNat {
    let mut limbs = Vec::new();
    if value != 0 {
        limbs.push(value);
    }
    BigNat { limbs }
}

/// Two limbs, most significant first, canonicalized.
#[allow(clippy::vec_init_then_push)]
pub fn from_u64_pair(high: u64, low: u64) -> BigNat {
    let mut limbs = Vec::new();
    limbs.push(low);
    limbs.push(high);
    normalize(limbs)
}

pub fn is_zero(value: &BigNat) -> bool {
    value.limbs.len() == 0
}

pub fn is_one(value: &BigNat) -> bool {
    value.limbs.len() == 1 && value.limbs[0] == 1
}

pub fn equal(left: &BigNat, right: &BigNat) -> bool {
    if left.limbs.len() != right.limbs.len() {
        return false;
    }
    let mut index = 0usize;
    let mut same = true;
    while index < left.limbs.len() && same {
        same = left.limbs[index] == right.limbs[index];
        index += 1;
    }
    same
}

/// Comparison of canonical limb vectors: -1, 0 or 1.
pub fn compare(left: &BigNat, right: &BigNat) -> i8 {
    let left_len = left.limbs.len();
    let right_len = right.limbs.len();
    if left_len < right_len {
        return -1;
    }
    if left_len > right_len {
        return 1;
    }
    let mut index = left_len;
    let mut order = 0i8;
    while index > 0 && order == 0 {
        index -= 1;
        if left.limbs[index] < right.limbs[index] {
            order = -1;
        } else if left.limbs[index] > right.limbs[index] {
            order = 1;
        }
    }
    order
}

/// Sum with a propagating carry; the condition covers the final carry.
pub fn add(left: &BigNat, right: &BigNat) -> BigNat {
    let left_len = left.limbs.len();
    let right_len = right.limbs.len();
    let mut out: Vec<u64> = Vec::new();
    let mut carry = 0u64;
    let mut index = 0usize;
    while index < left_len || index < right_len || carry != 0 {
        let x = if index < left_len {
            left.limbs[index]
        } else {
            0
        };
        let y = if index < right_len {
            right.limbs[index]
        } else {
            0
        };
        let (partial, first_carry) = x.overflowing_add(y);
        let (digit, second_carry) = partial.overflowing_add(carry);
        out.push(digit);
        if first_carry || second_carry {
            carry = 1;
        } else {
            carry = 0;
        }
        index += 1;
    }
    normalize(out)
}

/// Truncated difference: left - right floors at zero, as in Lean Nat.
pub fn sub(left: &BigNat, right: &BigNat) -> BigNat {
    if compare(left, right) <= 0 {
        return zero();
    }
    let left_len = left.limbs.len();
    let right_len = right.limbs.len();
    let mut out: Vec<u64> = Vec::new();
    let mut borrow = 0u64;
    let mut index = 0usize;
    while index < left_len {
        let x = left.limbs[index];
        let y = if index < right_len {
            right.limbs[index]
        } else {
            0
        };
        let (partial, first_borrow) = x.overflowing_sub(y);
        let (digit, second_borrow) = partial.overflowing_sub(borrow);
        out.push(digit);
        if first_borrow || second_borrow {
            borrow = 1;
        } else {
            borrow = 0;
        }
        index += 1;
    }
    normalize(out)
}

/// Schoolbook product: out[i + j] accumulates left[i] * right[j].
pub fn mul(left: &BigNat, right: &BigNat) -> BigNat {
    let left_len = left.limbs.len();
    let right_len = right.limbs.len();
    if left_len == 0 || right_len == 0 {
        return zero();
    }
    let mut out: Vec<u64> = Vec::new();
    let mut index = 0usize;
    while index < left_len + right_len {
        out.push(0u64);
        index += 1;
    }
    let mut i = 0usize;
    while i < left_len {
        let mut carry = 0u64;
        let mut j = 0usize;
        while j < right_len {
            let product = (left.limbs[i] as u128) * (right.limbs[j] as u128)
                + (out[i + j] as u128)
                + (carry as u128);
            out[i + j] = product as u64;
            carry = (product >> 64) as u64;
            j += 1;
        }
        // The next limb may already contain the contribution from a prior
        // row. Add this row's carry to it and propagate any overflow instead
        // of overwriting that contribution.
        let mut position = i + right_len;
        let (sum, mut overflow) = out[position].overflowing_add(carry);
        out[position] = sum;
        position += 1;
        while overflow && position < out.len() {
            let (next, next_overflow) = out[position].overflowing_add(1u64);
            out[position] = next;
            overflow = next_overflow;
            position += 1;
        }
        debug_assert!(!overflow);
        i += 1;
    }
    normalize(out)
}

/// value * mul + addend for a machine word multiplier and addend.
pub fn mul_add_small(value: &BigNat, mul: u64, addend: u64) -> BigNat {
    let len = value.limbs.len();
    let mut out: Vec<u64> = Vec::new();
    let mut carry = addend as u128;
    let mut index = 0usize;
    while index < len {
        let product = (value.limbs[index] as u128) * (mul as u128) + carry;
        out.push(product as u64);
        carry = product >> 64;
        index += 1;
    }
    let mut tail = carry;
    while tail != 0 {
        out.push(tail as u64);
        tail >>= 64;
    }
    normalize(out)
}

/// Division and remainder by a nonzero machine word, with the remainder first.
pub fn divmod_small(value: &BigNat, divisor: u64) -> (u64, BigNat) {
    let len = value.limbs.len();
    if divisor == 0 {
        let quotient = value.clone();
        return (0u64, quotient);
    }
    let mut quotient: Vec<u64> = Vec::new();
    let mut index = 0usize;
    while index < len {
        quotient.push(0u64);
        index += 1;
    }
    let mut remainder = 0u128;
    let mut position = len;
    while position > 0 {
        position -= 1;
        let current = (remainder << 64) | (value.limbs[position] as u128);
        quotient[position] = (current / (divisor as u128)) as u64;
        remainder = current % (divisor as u128);
    }
    (remainder as u64, normalize(quotient))
}

/// Number of significant bits; zero has none.
pub fn bit_length(value: &BigNat) -> usize {
    if value.limbs.len() == 0 {
        return 0;
    }
    let top = value.limbs[value.limbs.len() - 1];
    let mut bits = (value.limbs.len() - 1) * 64;
    let mut probe = top;
    while probe != 0 {
        bits += 1;
        probe >>= 1;
    }
    bits
}

/// Bit at position index, zero when out of range.
pub fn bit_at(value: &BigNat, index: usize) -> u64 {
    let limb = index / 64;
    if limb >= value.limbs.len() {
        return 0;
    }
    (value.limbs[limb] >> (index % 64)) & 1
}

/// value * 2.
pub fn shl_one(value: &BigNat) -> BigNat {
    let len = value.limbs.len();
    let mut out: Vec<u64> = Vec::new();
    if len == 0 {
        return normalize(out);
    }
    let top = value.limbs[len - 1];
    let extra = top >> 63;
    let mut index2 = 0usize;
    while index2 < len {
        let current = value.limbs[index2] << 1;
        let carry = if index2 == 0 {
            0
        } else {
            value.limbs[index2 - 1] >> 63
        };
        out.push(current | carry);
        index2 += 1;
    }
    if extra != 0 {
        out.push(extra);
    }
    normalize(out)
}

/// value + 1.
pub fn succ(value: &BigNat) -> BigNat {
    let one = one();
    add(value, &one)
}

/// value + addend.
pub fn add_u64(value: &BigNat, addend: u64) -> BigNat {
    let word = from_u64(addend);
    add(value, &word)
}

/// Bit-serial division: q * divisor + r = value with r < divisor.
/// A zero divisor returns (value, zero) so that the caller matches Lean's
/// total division (a / 0 = 0, a % 0 = a).
pub fn divmod(value: &BigNat, divisor: &BigNat) -> (BigNat, BigNat) {
    if is_zero(divisor) {
        return (zero(), value.clone());
    }
    if compare(value, divisor) < 0 {
        return (zero(), value.clone());
    }
    let high = bit_length(value);
    let mut quotient = zero();
    let mut remainder = zero();
    let mut position = high;
    while position > 0 {
        position -= 1;
        let rest = shl_one(&remainder);
        let bit = bit_at(value, position);
        if bit == 1 {
            remainder = succ(&rest);
        } else {
            remainder = rest;
        }
        quotient = shl_one(&quotient);
        if compare(&remainder, divisor) >= 0 {
            remainder = sub(&remainder, divisor);
            quotient = succ(&quotient);
        }
    }
    (quotient, remainder)
}

pub fn div(value: &BigNat, divisor: &BigNat) -> BigNat {
    divmod(value, divisor).0
}

pub fn rem(value: &BigNat, divisor: &BigNat) -> BigNat {
    divmod(value, divisor).1
}

/// Low 64 bits, matching Lean UInt64.ofNat truncation.
pub fn low_u64(value: &BigNat) -> u64 {
    if value.limbs.len() == 0 {
        return 0;
    }
    value.limbs[0]
}

/// The value as usize when it fits, mirroring BigUint::to_usize.
pub fn to_usize(value: &BigNat) -> Option<usize> {
    if value.limbs.len() == 0 {
        return Some(0usize);
    }
    if value.limbs.len() > 1 {
        return None;
    }
    let low = value.limbs[0];
    if low > usize::MAX as u64 {
        return None;
    }
    Some(low as usize)
}

/// Greatest common divisor by the Euclidean algorithm; gcd(0, 0) is zero.
pub fn gcd(left: &BigNat, right: &BigNat) -> BigNat {
    let mut a = left.clone();
    let mut b = right.clone();
    while !is_zero(&b) {
        let rest = rem(&a, &b);
        a = b;
        b = rest;
    }
    a
}

/// Decimal digits, most significant first; zero renders as a single 0.
pub fn to_decimal_digits(value: &BigNat) -> Vec<u8> {
    let mut digits: Vec<u8> = Vec::new();
    if is_zero(value) {
        digits.push(b'0');
        return digits;
    }
    let mut rest = value.clone();
    while !is_zero(&rest) {
        let (remainder, quotient) = divmod_small(&rest, 10u64);
        digits.push(b'0' + (remainder as u8));
        rest = quotient;
    }
    let len = digits.len();
    let mut index = 0usize;
    while index < len / 2 {
        let opposite = len - 1 - index;
        digits.swap(index, opposite);
        index += 1;
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values below 2^128, used as a machine-word oracle for the limb code.
    fn nat(value: u128) -> BigNat {
        from_u64_pair((value >> 64) as u64, value as u64)
    }

    fn word(value: &BigNat) -> u128 {
        let mut total = 0u128;
        let mut index = value.limbs.len();
        while index > 0 {
            index -= 1;
            total = (total << 64) | (value.limbs[index] as u128);
        }
        total
    }

    fn canonical(value: &BigNat) -> bool {
        value.limbs.len() == 0 || value.limbs[value.limbs.len() - 1] != 0
    }

    #[test]
    fn normalization_and_order() {
        assert!(is_zero(&zero()));
        assert!(is_zero(&normalize(vec![0, 0, 0])));
        assert!(canonical(&normalize(vec![5, 0])));
        assert_eq!(normalize(vec![5, 0]).limbs, vec![5]);
        assert_eq!(compare(&nat(7), &nat(7)), 0);
        assert_eq!(compare(&nat(7), &nat(8)), -1);
        assert_eq!(compare(&nat(8), &nat(7)), 1);
        assert_eq!(compare(&nat(1 << 70), &nat(1 << 69)), 1);
        assert_eq!(compare(&nat(1 << 69), &nat(1 << 70)), -1);
        assert!(equal(&nat(1 << 100), &nat(1 << 100)));
        assert!(!equal(&nat(1 << 100), &nat(1 << 101)));
        assert!(!equal(&nat(1 << 100), &zero()));
        assert!(is_one(&one()));
        assert!(!is_one(&nat(2)));
    }

    #[test]
    fn word_arithmetic_matches_u128() {
        let cases: [u128; 9] = [
            0,
            1,
            2,
            3,
            1 << 63,
            0x8000_0000_0000_0001,
            0xffff_ffff_ffff_ffff,
            0xdead_beef_dead_beef,
            123_456_789_012_345_678,
        ];
        for left in cases {
            for right in cases {
                assert_eq!(word(&add(&nat(left), &nat(right))), left + right);
                assert_eq!(word(&mul(&nat(left), &nat(right))), left * right);
                assert_eq!(
                    word(&sub(&nat(left), &nat(right))),
                    left.saturating_sub(right)
                );
                assert_eq!(
                    compare(&nat(left), &nat(right)),
                    match left.cmp(&right) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    }
                );
                if right != 0 {
                    assert_eq!(word(&div(&nat(left), &nat(right))), left / right);
                    assert_eq!(word(&rem(&nat(left), &nat(right))), left % right);
                    let (low, high) = divmod_small(&nat(left), right as u64);
                    assert_eq!(word(&high), left / right);
                    assert_eq!(low as u128, left % right);
                    assert!(equal(
                        &divmod(&nat(left), &nat(right)).0,
                        &nat(left / right)
                    ));
                    assert!(equal(
                        &divmod(&nat(left), &nat(right)).1,
                        &nat(left % right)
                    ));
                }
            }
        }
    }

    #[test]
    fn division_is_total_and_consistent() {
        assert!(is_zero(&div(&nat(9), &zero())));
        assert!(equal(&rem(&nat(9), &zero()), &nat(9)));
        let big = add(&shl_one(&nat(1 << 127)), &nat(3));
        assert_eq!(compare(&big, &nat(1 << 127)), 1);
        assert!(equal(&div(&big, &nat(1 << 127)), &nat(2)));
        // 2^128 + 3 is congruent to 1 modulo 3.
        assert!(equal(&rem(&big, &nat(3)), &nat(1)));
        assert!(equal(&div(&big, &nat(1 << 120)), &nat(256)));
        // (a * b) / b == a for large operands.
        let a = nat(0x0123_4567_89ab_cdef);
        let b = nat(0xfedc_ba98_7654_3210);
        let product = mul(&a, &b);
        assert!(equal(&div(&product, &b), &a));
        assert!(is_zero(&rem(&product, &b)));
        assert!(equal(&divmod(&product, &b).0, &a));
        // Powers and a long division with a nontrivial remainder.
        let mut power = one();
        let mut step = 0usize;
        while step < 40 {
            power = mul(&power, &nat(10));
            step += 1;
        }
        let quotient = div(&power, &nat(7));
        let remainder = rem(&power, &nat(7));
        assert!(compare(&remainder, &nat(7)) < 0);
        assert!(equal(&add(&mul(&quotient, &nat(7)), &remainder), &power));
    }

    #[test]
    fn small_helpers_and_digits() {
        let value = nat(0xffff_ffff_ffff_ffff);
        assert!(equal(
            &mul_add_small(&value, 10, 7),
            &nat(184_467_440_737_095_516_157)
        ));
        assert!(equal(&mul_add_small(&zero(), 10, 7), &nat(7)));
        assert_eq!(bit_length(&zero()), 0);
        assert_eq!(bit_length(&nat(1)), 1);
        assert_eq!(bit_length(&nat(255)), 8);
        assert_eq!(bit_length(&nat(256)), 9);
        assert_eq!(bit_length(&nat(1 << 64)), 65);
        assert!(equal(&shl_one(&nat(1 << 63)), &nat(1 << 64)));
        assert!(equal(&shl_one(&zero()), &zero()));
        assert_eq!(bit_at(&nat(6), 1), 1);
        assert_eq!(bit_at(&nat(6), 2), 1);
        assert_eq!(bit_at(&nat(6), 0), 0);
        assert_eq!(bit_at(&nat(6), 99), 0);
        assert!(equal(&succ(&nat(u64::MAX as u128)), &nat(1u128 << 64)));
        assert!(equal(&add_u64(&zero(), 1), &one()));
        assert_eq!(to_decimal_digits(&zero()), vec![b'0']);
        assert_eq!(
            to_decimal_digits(&nat(1_234_567_890)),
            b"1234567890".to_vec()
        );
        assert_eq!(
            to_decimal_digits(&nat(10_000_000_000_000_000_000_000)),
            b"10000000000000000000000".to_vec()
        );
        let square = mul(&nat(1u128 << 64), &nat(1u128 << 64));
        assert_eq!(
            to_decimal_digits(&square),
            b"340282366920938463463374607431768211456".to_vec()
        );
        assert_eq!(to_usize(&nat(7)), Some(7));
        assert_eq!(to_usize(&zero()), Some(0));
        assert_eq!(to_usize(&nat(1 << 70)), None);
        assert_eq!(low_u64(&nat(1 << 64)), 0);
        assert_eq!(low_u64(&zero()), 0);
        assert!(equal(
            &gcd(&nat(0x1_0000_0000_0000_0000), &nat(0x8000_0000_0000_0000)),
            &nat(0x8000_0000_0000_0000)
        ));
        assert!(equal(&gcd(&zero(), &nat(5)), &nat(5)));
    }
}
