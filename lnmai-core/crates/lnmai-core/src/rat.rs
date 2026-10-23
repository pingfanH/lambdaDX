//! Arbitrary-precision rational arithmetic matching Lean's normalized Rat.
//! A zero denominator denotes zero, as in Lean's total division operation.
//!
//! The arithmetic is the extracted kernel in `lnmai_core_verify::big_rat`:
//! lowest terms with a positive denominator, cross-product comparison and the
//! twelve-digit decimal expansion. num-bigint appears only in the constructor
//! that accepts decoded JSON integers.

use crate::{integer::Integer, natural::Natural};
use core::cmp::Ordering;
use lnmai_core_verify::big_int as int_kernel;
use lnmai_core_verify::big_rat as kernel;
use num_bigint::BigInt;
use std::borrow::Borrow;

#[derive(Clone)]
pub struct Rat {
    value: kernel::BigRat,
}

impl std::fmt::Debug for Rat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rat {{ num: {}, den: {} }}", self.num(), self.den())
    }
}

impl Rat {
    /// Ordinary chart clocks fit in i128. Keep the same canonical kernel
    /// representation, but avoid limb division/GCD when machine arithmetic
    /// can compute the exact result. Overflow always uses the unbounded path.
    fn small_parts(&self) -> Option<(i128, i128)> {
        fn magnitude(value: &lnmai_core_verify::big_nat::BigNat) -> Option<u128> {
            match value.limbs.as_slice() {
                [] => Some(0),
                [low] => Some(*low as u128),
                [low, high] => Some((*high as u128) << 64 | *low as u128),
                _ => None,
            }
        }
        let abs = magnitude(&self.value.num.magnitude)?;
        let num = if self.value.num.negative {
            if abs == 1u128 << 127 { i128::MIN } else { i128::try_from(abs).ok()?.checked_neg()? }
        } else { i128::try_from(abs).ok()? };
        Some((num, i128::try_from(magnitude(&self.value.den)?).ok()?))
    }

    fn from_small_ratio(mut num: i128, mut den: i128) -> Option<Self> {
        if den == 0 { return Some(Self::zero()); }
        if den < 0 {
            num = num.checked_neg()?;
            den = den.checked_neg()?;
        }
        let mut a = num.unsigned_abs();
        let mut b = den as u128;
        while b != 0 { (a, b) = (b, a % b); }
        let divisor = i128::try_from(a).ok()?;
        let num = num / divisor;
        let den = (den / divisor) as u128;
        Some(Self { value: kernel::BigRat {
            num: int_kernel::from_i128(num),
            den: lnmai_core_verify::big_nat::from_u64_pair((den >> 64) as u64, den as u64),
        } })
    }

    fn small_binary(&self, other: &Self, op: u8) -> Option<Self> {
        let (a, b) = self.small_parts()?;
        let (c, d) = other.small_parts()?;
        let (num, den) = match op {
            b'+' => (a.checked_mul(d)?.checked_add(c.checked_mul(b)?)?, b.checked_mul(d)?),
            b'-' => (a.checked_mul(d)?.checked_sub(c.checked_mul(b)?)?, b.checked_mul(d)?),
            b'*' => (a.checked_mul(c)?, b.checked_mul(d)?),
            b'/' => (a.checked_mul(d)?, b.checked_mul(c)?),
            _ => unreachable!(),
        };
        Self::from_small_ratio(num, den)
    }

    /// Boundary constructor: the operands arrive as JSON integers.
    pub fn new(num: impl Into<BigInt>, den: impl Into<BigInt>) -> Self {
        let num = Integer::from(num.into());
        let den = Integer::from(den.into());
        Self::from_numeric(&num, &den)
    }

    /// Kernel-typed constructor: a zero denominator is zero.
    pub fn from_numeric(num: &Integer, den: &Integer) -> Self {
        if let (Some(num), Some(den)) = (num.to_i64(), den.to_i64()) {
            return Self::from_small_ratio(num as i128, den as i128).unwrap();
        }
        Self {
            value: kernel::new(&num.kernel(), &den.kernel()),
        }
    }

    pub fn from_int(n: impl Into<BigInt>) -> Self {
        let value = Integer::from(n.into());
        Self {
            value: kernel::from_int(&value.kernel()),
        }
    }

    pub fn from_integer(n: &Integer) -> Self {
        Self {
            value: kernel::from_int(&n.kernel()),
        }
    }

    pub fn zero() -> Self {
        Self {
            value: kernel::zero(),
        }
    }

    pub fn one() -> Self {
        Self {
            value: kernel::one(),
        }
    }

    pub fn num(&self) -> Integer {
        Integer::from_kernel(self.value.num.clone())
    }

    pub fn den(&self) -> Natural {
        Natural::from_kernel(self.value.den.clone())
    }

    pub fn is_zero(&self) -> bool {
        kernel::is_zero(&self.value)
    }

    pub fn is_negative(&self) -> bool {
        kernel::is_negative(&self.value)
    }

    pub fn add(&self, other: impl Borrow<Rat>) -> Self {
        if let Some(value) = self.small_binary(other.borrow(), b'+') { return value; }
        Self {
            value: kernel::add(&self.value, &other.borrow().value),
        }
    }

    pub fn sub(&self, other: impl Borrow<Rat>) -> Self {
        if let Some(value) = self.small_binary(other.borrow(), b'-') { return value; }
        Self {
            value: kernel::sub(&self.value, &other.borrow().value),
        }
    }

    pub fn mul(&self, other: impl Borrow<Rat>) -> Self {
        if let Some(value) = self.small_binary(other.borrow(), b'*') { return value; }
        Self {
            value: kernel::mul(&self.value, &other.borrow().value),
        }
    }

    pub fn div(&self, other: impl Borrow<Rat>) -> Self {
        if let Some(value) = self.small_binary(other.borrow(), b'/') { return value; }
        Self {
            value: kernel::div(&self.value, &other.borrow().value),
        }
    }

    /// Lean's decimal renderer uses at most twelve fractional digits.
    pub fn to_decimal_string(&self) -> String {
        if let Some(text) = self.small_decimal_string() {
            return text;
        }
        let mut text = String::new();
        if kernel::is_negative(&self.value) {
            text.push('-');
        }
        let whole = kernel::whole_digits(&self.value);
        let mut index = 0;
        while index < whole.len() {
            text.push(whole[index] as char);
            index += 1;
        }
        let fraction = kernel::fraction_digits(&self.value);
        if !fraction.is_empty() {
            text.push('.');
            let mut index = 0;
            while index < fraction.len() {
                text.push(fraction[index] as char);
                index += 1;
            }
        }
        text
    }

    fn small_decimal_string(&self) -> Option<String> {
        let (num, den) = self.small_parts()?;
        let magnitude = num.unsigned_abs();
        let den = den as u128;
        let mut text = (magnitude / den).to_string();
        if num < 0 {
            text.insert(0, '-');
        }
        let mut rest = magnitude % den;
        if rest != 0 {
            text.push('.');
            for _ in 0..12 {
                let scaled = rest.checked_mul(10)?;
                text.push(b'0'.checked_add((scaled / den) as u8)? as char);
                rest = scaled % den;
                if rest == 0 {
                    break;
                }
            }
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.pop();
            }
        }
        Some(text)
    }
}

impl Ord for Rat {
    fn cmp(&self, other: &Self) -> Ordering {
        if let (Some((a, b)), Some((c, d))) = (self.small_parts(), other.small_parts()) {
            if let (Some(left), Some(right)) = (a.checked_mul(d), c.checked_mul(b)) {
                return left.cmp(&right);
            }
        }
        match kernel::compare(&self.value, &other.value) {
            -1 => Ordering::Less,
            1 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Rat {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Rat {}

/// The kernel value, shared with adapters that need the exact limbs.
impl Rat {
    pub fn kernel_value(&self) -> &kernel::BigRat {
        &self.value
    }

    pub fn from_kernel(value: kernel::BigRat) -> Self {
        Self { value }
    }
}

/// Used by time.rs to move between kernel and runtime integers.
pub fn integer_kernel(value: &Integer) -> int_kernel::BigIntKernel {
    value.kernel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_matches_exact_kernel_at_fast_path_boundaries() {
        let huge = BigInt::from(1u8) << 256;
        let values = [
            Rat::zero(), Rat::one(), Rat::new(-7, 3), Rat::new(240_000_000, 167),
            Rat::new(i64::MIN, 1000), Rat::new(u64::MAX, u64::MAX - 1),
            Rat::new(i128::MIN, 3), Rat::new(i128::MAX, 5),
            Rat::new(&huge + 1, &huge + 3),
        ];
        for left in &values {
            let mut expected = String::new();
            if kernel::is_negative(&left.value) { expected.push('-'); }
            expected.extend(kernel::whole_digits(&left.value).into_iter().map(char::from));
            let fraction = kernel::fraction_digits(&left.value);
            if !fraction.is_empty() {
                expected.push('.');
                expected.extend(fraction.into_iter().map(char::from));
            }
            assert_eq!(left.to_decimal_string(), expected);
            for right in &values {
                for (actual, expected) in [
                    (left.add(right), kernel::add(&left.value, &right.value)),
                    (left.sub(right), kernel::sub(&left.value, &right.value)),
                    (left.mul(right), kernel::mul(&left.value, &right.value)),
                    (left.div(right), kernel::div(&left.value, &right.value)),
                ] {
                    assert!(kernel::equal(&actual.value, &expected));
                    assert!(int_kernel::equal(&actual.value.num, &expected.num));
                    assert!(lnmai_core_verify::big_nat::equal(&actual.value.den, &expected.den));
                }
                assert_eq!(left.cmp(right), kernel::compare(&left.value, &right.value).cmp(&0));
            }
        }
    }

    #[test]
    fn normalizes() {
        assert_eq!(Rat::new(2, 4), Rat::new(1, 2));
        assert_eq!(Rat::new(-2, 4), Rat::new(-1, 2));
        assert_eq!(Rat::new(2, -4), Rat::new(-1, 2));
        assert_eq!(Rat::new(5, 0), Rat::zero());
        assert_eq!(Rat::new(i128::MIN, i128::MIN), Rat::one());
    }

    #[test]
    fn arithmetic() {
        let a = Rat::new(1, 3);
        let b = Rat::new(1, 6);
        assert_eq!(a.add(&b), Rat::new(1, 2));
        assert_eq!(a.sub(&b), Rat::new(1, 6));
        assert_eq!(a.mul(&b), Rat::new(1, 18));
        assert_eq!(a.div(&b), Rat::from_int(2));
        let huge = BigInt::from(1u8) << 1024;
        let a = Rat::new(&huge + 1, &huge + 2);
        let b = Rat::new(&huge + 2, &huge + 1);
        assert_eq!(a.mul(&b), Rat::one());
        assert!(a < b);
    }

    #[test]
    fn decimal_string() {
        assert_eq!(Rat::new(1, 2).to_decimal_string(), "0.5");
        assert_eq!(Rat::from_int(3).to_decimal_string(), "3");
        assert_eq!(Rat::new(-1, 4).to_decimal_string(), "-0.25");
        assert_eq!(
            Rat::new(i128::MIN, 1).to_decimal_string(),
            i128::MIN.to_string()
        );
        assert_eq!(Rat::zero().to_decimal_string(), "0");
        assert_eq!(Rat::new(1, 3).to_decimal_string(), "0.333333333333");
    }

    #[test]
    fn accessors_keep_exact_value() {
        let value = Rat::new(6, -4);
        assert_eq!(value.num().to_bigint(), BigInt::from(-3));
        assert_eq!(value.den().to_biguint(), num_bigint::BigUint::from(2u8));
        assert!(value.is_negative());
        assert!(!value.is_zero());
        assert!(Rat::new(0, 5).is_zero());
    }
}
