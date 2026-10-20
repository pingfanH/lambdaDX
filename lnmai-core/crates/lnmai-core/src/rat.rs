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
    /// Boundary constructor: the operands arrive as JSON integers.
    pub fn new(num: impl Into<BigInt>, den: impl Into<BigInt>) -> Self {
        let num = Integer::from(num.into());
        let den = Integer::from(den.into());
        Self::from_numeric(&num, &den)
    }

    /// Kernel-typed constructor: a zero denominator is zero.
    pub fn from_numeric(num: &Integer, den: &Integer) -> Self {
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
        Self {
            value: kernel::add(&self.value, &other.borrow().value),
        }
    }

    pub fn sub(&self, other: impl Borrow<Rat>) -> Self {
        Self {
            value: kernel::sub(&self.value, &other.borrow().value),
        }
    }

    pub fn mul(&self, other: impl Borrow<Rat>) -> Self {
        Self {
            value: kernel::mul(&self.value, &other.borrow().value),
        }
    }

    pub fn div(&self, other: impl Borrow<Rat>) -> Self {
        Self {
            value: kernel::div(&self.value, &other.borrow().value),
        }
    }

    /// Lean's decimal renderer uses at most twelve fractional digits.
    pub fn to_decimal_string(&self) -> String {
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
}

impl Ord for Rat {
    fn cmp(&self, other: &Self) -> Ordering {
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
        kernel::equal(&self.value, &other.value)
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
