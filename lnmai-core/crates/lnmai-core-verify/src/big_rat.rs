//! Exact rationals over the extracted integer and natural kernels.
//!
//! This is the shared implementation of the runtime Rat type. Values are kept
//! in lowest terms with a positive denominator; a zero denominator denotes
//! zero, matching Lean's total division. The decimal renderer keeps at most
//! twelve fractional digits, as Lean's Rat.toDecimalString does.

use crate::big_int::{self, BigIntKernel};
use crate::big_nat::{self, BigNat};

/// Lowest-terms rational with a positive denominator.
#[derive(Clone)]
pub struct BigRat {
    pub num: BigIntKernel,
    pub den: BigNat,
}

pub fn from_int(num: &BigIntKernel) -> BigRat {
    BigRat {
        num: num.clone(),
        den: big_nat::one(),
    }
}

pub fn zero() -> BigRat {
    BigRat {
        num: big_int::zero(),
        den: big_nat::one(),
    }
}

pub fn one() -> BigRat {
    BigRat {
        num: big_int::from_u64(1u64),
        den: big_nat::one(),
    }
}

pub fn is_zero(value: &BigRat) -> bool {
    big_int::is_zero(&value.num)
}

pub fn is_negative(value: &BigRat) -> bool {
    big_int::is_negative(&value.num)
}

fn den_int(value: &BigRat) -> BigIntKernel {
    big_int::of_nat(&value.den)
}

/// Reduce to lowest terms with a nonnegative denominator.
fn reduce(num: BigIntKernel, den: BigIntKernel) -> BigRat {
    if big_int::is_zero(&den) {
        return zero();
    }
    let negative = big_int::is_negative(&den);
    let numer = if negative { big_int::neg(&num) } else { num };
    let denom = big_int::abs(&den);
    let divisor = big_int::gcd(&numer, &denom);
    if big_nat::is_zero(&divisor) {
        return BigRat {
            num: numer,
            den: denom.magnitude,
        };
    }
    let divisor_int = big_int::of_nat(&divisor);
    BigRat {
        num: big_int::div_trunc(&numer, &divisor_int),
        den: big_nat::div(&denom.magnitude, &divisor),
    }
}

/// Lean Rational.mk: a zero denominator is zero.
pub fn new(num: &BigIntKernel, den: &BigIntKernel) -> BigRat {
    reduce(num.clone(), den.clone())
}

pub fn add(left: &BigRat, right: &BigRat) -> BigRat {
    let first = big_int::mul(&left.num, &den_int(right));
    let second = big_int::mul(&right.num, &den_int(left));
    let numerator = big_int::add(&first, &second);
    let denominator = big_int::of_nat(&big_nat::mul(&left.den, &right.den));
    reduce(numerator, denominator)
}

pub fn sub(left: &BigRat, right: &BigRat) -> BigRat {
    let first = big_int::mul(&left.num, &den_int(right));
    let second = big_int::mul(&right.num, &den_int(left));
    let numerator = big_int::sub(&first, &second);
    let denominator = big_int::of_nat(&big_nat::mul(&left.den, &right.den));
    reduce(numerator, denominator)
}

pub fn mul(left: &BigRat, right: &BigRat) -> BigRat {
    let numerator = big_int::mul(&left.num, &right.num);
    let denominator = big_int::of_nat(&big_nat::mul(&left.den, &right.den));
    reduce(numerator, denominator)
}

pub fn div(left: &BigRat, right: &BigRat) -> BigRat {
    let numerator = big_int::mul(&left.num, &den_int(right));
    let denominator = big_int::mul(&right.num, &den_int(left));
    reduce(numerator, denominator)
}

/// Comparison by cross multiplication: -1, 0 or 1.
pub fn compare(left: &BigRat, right: &BigRat) -> i8 {
    let first = big_int::mul(&left.num, &den_int(right));
    let second = big_int::mul(&right.num, &den_int(left));
    big_int::compare(&first, &second)
}

pub fn equal(left: &BigRat, right: &BigRat) -> bool {
    compare(left, right) == 0
}

/// Decimal digits of the truncated absolute whole part.
pub fn whole_digits(value: &BigRat) -> Vec<u8> {
    let magnitude = &value.num.magnitude;
    let whole = big_nat::div(magnitude, &value.den);
    big_nat::to_decimal_digits(&whole)
}

/// At most twelve fractional digits with trailing zeros removed.
pub fn fraction_digits(value: &BigRat) -> Vec<u8> {
    let mut fraction: Vec<u8> = Vec::new();
    if big_nat::is_zero(&value.num.magnitude) {
        return fraction;
    }
    let mut rest = big_nat::rem(&value.num.magnitude, &value.den);
    let mut count = 0usize;
    while count < 12usize && !big_nat::is_zero(&rest) {
        let scaled = big_nat::mul_add_small(&rest, 10u64, 0u64);
        let digit = big_nat::div(&scaled, &value.den);
        rest = big_nat::rem(&scaled, &value.den);
        fraction.push(b'0' + (big_nat::low_u64(&digit) as u8));
        count += 1;
    }
    let mut length = fraction.len();
    while length > 0 && fraction[length - 1] == b'0' {
        length -= 1;
    }
    let mut trimmed: Vec<u8> = Vec::new();
    let mut index = 0usize;
    while index < length {
        trimmed.push(fraction[index]);
        index += 1;
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rat(num: i128, den: i128) -> BigRat {
        new(&big_int::from_i128(num), &big_int::from_i128(den))
    }

    fn render(value: &BigRat) -> (Vec<u8>, Vec<u8>) {
        (whole_digits(value), fraction_digits(value))
    }

    #[test]
    fn normalization_keeps_lowest_terms() {
        assert!(equal(&rat(2, 4), &rat(1, 2)));
        assert!(equal(&rat(-2, 4), &rat(-1, 2)));
        assert!(equal(&rat(2, -4), &rat(-1, 2)));
        assert!(is_zero(&rat(5, 0)));
        assert!(equal(&rat(-3, -3), &one()));
        assert!(equal(&from_int(&big_int::from_i128(0)), &zero()));
        assert!(equal(&from_int(&big_int::from_i128(4)), &rat(4, 1)));
        assert!(equal(&rat(0, 7), &zero()));
        assert!(equal(&rat(6, 3), &from_int(&big_int::from_i128(2))));
    }

    #[test]
    fn arithmetic_matches_cross_products() {
        let values: [(i128, i128); 7] =
            [(0, 1), (1, 3), (1, 6), (-1, 4), (7, 5), (-22, 7), (100, 9)];
        for (ln, ld) in values {
            for (rn, rd) in values {
                let left = rat(ln, ld);
                let right = rat(rn, rd);
                assert!(equal(&add(&left, &right), &rat(ln * rd + rn * ld, ld * rd)));
                assert!(equal(&sub(&left, &right), &rat(ln * rd - rn * ld, ld * rd)));
                assert!(equal(&mul(&left, &right), &rat(ln * rn, ld * rd)));
                if rn != 0 {
                    assert!(equal(&div(&left, &right), &rat(ln * rd, ld * rn)));
                }
                let ordering = (ln * rd).cmp(&(rn * ld));
                let expected = match ordering {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                assert_eq!(compare(&left, &right), expected);
                assert_eq!(equal(&left, &right), ordering.is_eq());
            }
        }
        assert!(is_zero(&div(&zero(), &zero())));
        assert!(is_negative(&rat(-1, 2)));
        assert!(!is_negative(&rat(0, 2)));
    }

    #[test]
    fn decimal_renderer_matches_decimal_expansion() {
        assert_eq!(render(&rat(1, 2)), (b"0".to_vec(), b"5".to_vec()));
        assert_eq!(render(&rat(3, 1)), (b"3".to_vec(), Vec::new()));
        assert_eq!(render(&rat(-1, 4)), (b"0".to_vec(), b"25".to_vec()));
        assert_eq!(render(&zero()), (b"0".to_vec(), Vec::new()));
        assert_eq!(
            render(&rat(1, 3)),
            (b"0".to_vec(), b"333333333333".to_vec())
        );
        assert_eq!(render(&rat(1, 8)), (b"0".to_vec(), b"125".to_vec()));
        assert_eq!(render(&rat(7, 4)), (b"1".to_vec(), b"75".to_vec()));
        assert_eq!(
            render(&rat(22, 7)),
            (b"3".to_vec(), b"142857142857".to_vec())
        );
        assert_eq!(render(&rat(100, 1)), (b"100".to_vec(), Vec::new()));
        let huge = big_nat::from_u64_pair(1, 0);
        let whole = BigRat {
            num: big_int::of_nat(&huge),
            den: big_nat::from_u64(1),
        };
        assert_eq!(whole_digits(&whole), b"18446744073709551616".to_vec());
        let half = BigRat {
            num: big_int::of_nat(&huge),
            den: big_nat::from_u64(2),
        };
        assert_eq!(whole_digits(&half), b"9223372036854775808".to_vec());
    }
}
