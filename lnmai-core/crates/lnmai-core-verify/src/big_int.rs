//! Exact signed integers over the extracted limb-vector magnitude.
//!
//! This is the shared implementation of the runtime Integer type. The
//! canonical form stores a sign bit plus a canonical BigNat magnitude; zero
//! is never negative. Division is total and Euclidean, matching Lean Int:
//! division by zero is zero and a / b with a remainder is rounded downward in
//! the Euclidean sense. The truncated quotient and remainder used by the
//! rounding helpers are kept separate.

use crate::big_nat::{self, BigNat};

/// Sign plus canonical magnitude; zero has no sign.
#[derive(Clone)]
pub struct BigIntKernel {
    pub negative: bool,
    pub magnitude: BigNat,
}

/// Canonical constructor: a zero magnitude is never negative.
pub fn make(negative: bool, magnitude: BigNat) -> BigIntKernel {
    if big_nat::is_zero(&magnitude) {
        BigIntKernel {
            negative: false,
            magnitude,
        }
    } else {
        BigIntKernel {
            negative,
            magnitude,
        }
    }
}

pub fn zero() -> BigIntKernel {
    BigIntKernel {
        negative: false,
        magnitude: big_nat::zero(),
    }
}

pub fn from_u64(value: u64) -> BigIntKernel {
    make(false, big_nat::from_u64(value))
}

pub fn from_u128(value: u128) -> BigIntKernel {
    let low = value as u64;
    let high = (value >> 64) as u64;
    make(false, big_nat::from_u64_pair(high, low))
}

pub fn from_i64(value: i64) -> BigIntKernel {
    if value < 0 {
        let magnitude = (0i128 - (value as i128)) as u64;
        make(true, big_nat::from_u64(magnitude))
    } else {
        make(false, big_nat::from_u64(value as u64))
    }
}

pub fn from_i128(value: i128) -> BigIntKernel {
    if value < 0 {
        let magnitude = if value == i128::MIN {
            1u128 << 127
        } else {
            (0i128 - value) as u128
        };
        make(true, from_u128(magnitude).magnitude)
    } else {
        make(false, from_u128(value as u128).magnitude)
    }
}

/// The magnitude of a natural value, with no sign.
pub fn of_nat(value: &BigNat) -> BigIntKernel {
    make(false, value.clone())
}

pub fn is_zero(value: &BigIntKernel) -> bool {
    big_nat::is_zero(&value.magnitude)
}

pub fn is_negative(value: &BigIntKernel) -> bool {
    value.negative && !big_nat::is_zero(&value.magnitude)
}

pub fn is_positive(value: &BigIntKernel) -> bool {
    !value.negative && !big_nat::is_zero(&value.magnitude)
}

pub fn neg(value: &BigIntKernel) -> BigIntKernel {
    if big_nat::is_zero(&value.magnitude) {
        return zero();
    }
    make(!value.negative, value.magnitude.clone())
}

pub fn abs(value: &BigIntKernel) -> BigIntKernel {
    make(false, value.magnitude.clone())
}

/// Comparison: -1, 0 or 1.
pub fn compare(left: &BigIntKernel, right: &BigIntKernel) -> i8 {
    let left_negative = is_negative(left);
    let right_negative = is_negative(right);
    if left_negative && !right_negative {
        return -1;
    }
    if !left_negative && right_negative {
        return 1;
    }
    let magnitude = big_nat::compare(&left.magnitude, &right.magnitude);
    if left_negative {
        return 0 - magnitude;
    }
    magnitude
}

pub fn equal(left: &BigIntKernel, right: &BigIntKernel) -> bool {
    left.negative == right.negative && big_nat::equal(&left.magnitude, &right.magnitude)
}

/// Same-sign operands add magnitudes; opposite signs subtract the smaller
/// magnitude from the larger and keep the winner's sign.
pub fn add(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    if big_nat::is_zero(&left.magnitude) {
        return right.clone();
    }
    if big_nat::is_zero(&right.magnitude) {
        return left.clone();
    }
    let left_negative = is_negative(left);
    let right_negative = is_negative(right);
    if left_negative == right_negative {
        let magnitude = big_nat::add(&left.magnitude, &right.magnitude);
        return make(left_negative, magnitude);
    }
    let order = big_nat::compare(&left.magnitude, &right.magnitude);
    if order == 0 {
        return zero();
    }
    if order > 0 {
        let magnitude = big_nat::sub(&left.magnitude, &right.magnitude);
        return make(left_negative, magnitude);
    }
    let magnitude = big_nat::sub(&right.magnitude, &left.magnitude);
    make(right_negative, magnitude)
}

pub fn sub(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    add(left, &neg(right))
}

pub fn mul(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    let negative = is_negative(left) != is_negative(right);
    make(negative, big_nat::mul(&left.magnitude, &right.magnitude))
}

/// Truncated quotient (sign of the dividend) with zero divisor giving zero.
pub fn div_trunc(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    if is_zero(right) {
        return zero();
    }
    let negative = is_negative(left) != is_negative(right);
    make(negative, big_nat::div(&left.magnitude, &right.magnitude))
}

/// Truncated remainder (sign of the dividend) with zero divisor giving the
/// dividend, as in Rust and Lean Int.tmod.
pub fn rem_trunc(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    if is_zero(right) {
        return left.clone();
    }
    make(
        is_negative(left),
        big_nat::rem(&left.magnitude, &right.magnitude),
    )
}

/// Euclidean quotient: the unique q with a = q * b + r and 0 <= r < |b|.
pub fn div_euclid(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    if is_zero(right) {
        return zero();
    }
    let quotient = big_nat::div(&left.magnitude, &right.magnitude);
    let remainder = big_nat::rem(&left.magnitude, &right.magnitude);
    if big_nat::is_zero(&remainder) {
        return make(is_negative(left) != is_negative(right), quotient);
    }
    if is_negative(left) {
        let bumped = big_nat::succ(&quotient);
        return make(!is_negative(right), bumped);
    }
    make(is_negative(right), quotient)
}

/// Euclidean remainder: always nonnegative and below |right|.
pub fn rem_euclid(left: &BigIntKernel, right: &BigIntKernel) -> BigIntKernel {
    let residual = sub(left, &mul(&div_euclid(left, right), right));
    abs(&residual)
}

/// Low 64 bits, matching Lean UInt64.ofInt and native two's-complement casts.
pub fn low_u64(value: &BigIntKernel) -> u64 {
    let magnitude = big_nat::low_u64(&value.magnitude);
    if is_negative(value) {
        let (negated, _) = 0u64.overflowing_sub(magnitude);
        return negated;
    }
    magnitude
}

/// The value as an i64 when it fits exactly.
pub fn to_i64(value: &BigIntKernel) -> Option<i64> {
    if big_nat::is_zero(&value.magnitude) {
        return Some(0i64);
    }
    if value.magnitude.limbs.len() > 1 {
        return None;
    }
    let magnitude = value.magnitude.limbs[0];
    if is_negative(value) {
        if magnitude > 9223372036854775808u64 {
            return None;
        }
        if magnitude == 9223372036854775808u64 {
            return Some(i64::MIN);
        }
        return Some(0i64 - (magnitude as i64));
    }
    if magnitude > i64::MAX as u64 {
        return None;
    }
    Some(magnitude as i64)
}

/// Greatest common divisor of magnitudes; used to normalize rationals.
pub fn gcd(left: &BigIntKernel, right: &BigIntKernel) -> BigNat {
    big_nat::gcd(&left.magnitude, &right.magnitude)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(value: i128) -> BigIntKernel {
        from_i128(value)
    }

    /// Values below 2^63 exercise the machine-word oracle exactly.
    const CASES: [i128; 11] = [
        0,
        1,
        -1,
        2,
        -3,
        7,
        -10,
        1_000_000_007,
        -1_000_000_007,
        303_700_037,
        -999_999_937,
    ];

    #[test]
    fn arithmetic_matches_i128() {
        for left in CASES {
            for right in CASES {
                assert!(equal(&add(&int(left), &int(right)), &int(left + right)));
                assert!(equal(&sub(&int(left), &int(right)), &int(left - right)));
                assert!(equal(&mul(&int(left), &int(right)), &int(left * right)));
                assert!(equal(&neg(&int(left)), &int(-left)));
                assert!(equal(&abs(&int(left)), &int(left.abs())));
                let order = match left.cmp(&right) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                assert_eq!(compare(&int(left), &int(right)), order);
                assert_eq!(is_negative(&int(left)), left < 0);
                assert_eq!(is_positive(&int(left)), left > 0);
                assert_eq!(is_zero(&int(left)), left == 0);
                if right != 0 {
                    assert!(equal(
                        &div_trunc(&int(left), &int(right)),
                        &int(left / right)
                    ));
                    assert!(equal(
                        &rem_trunc(&int(left), &int(right)),
                        &int(left % right)
                    ));
                    let quotient = div_euclid(&int(left), &int(right));
                    let remainder = rem_euclid(&int(left), &int(right));
                    assert!(equal(
                        &add(&mul(&quotient, &int(right)), &remainder),
                        &int(left)
                    ));
                    assert!(!is_negative(&remainder));
                    assert!(compare(&remainder, &abs(&int(right))) < 0);
                }
            }
        }
        assert!(is_zero(&div_euclid(&int(5), &int(0))));
        assert!(is_zero(&div_trunc(&int(5), &int(0))));
        assert!(equal(&rem_trunc(&int(5), &int(0)), &int(5)));
        assert!(equal(&div_euclid(&int(-5), &int(3)), &int(-2)));
        assert!(equal(&div_euclid(&int(-5), &int(-3)), &int(2)));
        assert!(equal(&div_euclid(&int(5), &int(-3)), &int(-1)));
        assert!(equal(&rem_euclid(&int(-5), &int(-3)), &int(1)));
    }

    #[test]
    fn conversions_and_exactness() {
        assert_eq!(to_i64(&int(-7)), Some(-7));
        assert_eq!(to_i64(&int(i64::MIN as i128)), Some(i64::MIN));
        assert_eq!(to_i64(&int(i64::MAX as i128)), Some(i64::MAX));
        assert_eq!(to_i64(&from_i128(i64::MIN as i128 - 1)), None);
        assert_eq!(to_i64(&from_i128(i64::MAX as i128 + 1)), None);
        assert_eq!(low_u64(&int(-1)), u64::MAX);
        assert_eq!(low_u64(&int(1)), 1);
        let big = from_u128(u128::MAX);
        assert!(equal(&big, &from_u128(u128::MAX)));
        assert_eq!(low_u64(&big), u64::MAX);
        assert_eq!(low_u64(&neg(&big)), 1);
        // Zero is never negative even when the sign bit is set at construction.
        assert!(!is_negative(&make(true, big_nat::zero())));
        assert!(equal(&make(true, big_nat::zero()), &zero()));
        // gcd ignores signs.
        assert!(big_nat::equal(
            &gcd(&int(-12), &int(18)),
            &big_nat::from_u64(6)
        ));
    }
}
