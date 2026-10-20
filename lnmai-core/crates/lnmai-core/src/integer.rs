//! Exact signed integers with inline storage for ordinary timeline ticks.
//! Large values are normalized back to inline storage whenever they fit.
//!
//! The unbounded representation is the extracted kernel in
//! `lnmai_core_verify::big_int`, so promotion arithmetic is kernel code. The
//! inline i64 case is a refinement of that kernel: it is only used when the
//! checked machine operation already gives the exact result. num-bigint is
//! used only for the JSON/FFI boundary conversions.

use crate::natural::Natural;
use lnmai_core_verify::big_int as kernel;
use lnmai_core_verify::big_nat as nat_kernel;
use num_bigint::BigInt;
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::{Add, Div, Mul, Neg, Rem, Sub},
};

#[derive(Clone)]
enum Repr {
    Small(i64),
    Large(kernel::BigIntKernel),
}

#[derive(Clone)]
pub struct Integer(Repr);

impl Integer {
    pub const fn small(value: i64) -> Self {
        Self(Repr::Small(value))
    }

    /// Canonical constructor: any value that fits an i64 returns to inline
    /// storage, so equal integers always share a representation.
    pub(crate) fn from_kernel(value: kernel::BigIntKernel) -> Self {
        match kernel::to_i64(&value) {
            Some(small) => Self(Repr::Small(small)),
            None => Self(Repr::Large(value)),
        }
    }

    pub(crate) fn kernel(&self) -> kernel::BigIntKernel {
        match &self.0 {
            Repr::Small(n) => kernel::from_i64(*n),
            Repr::Large(n) => n.clone(),
        }
    }

    pub fn to_bigint(&self) -> BigInt {
        match &self.0 {
            Repr::Small(n) => BigInt::from(*n),
            Repr::Large(n) => kernel_to_bigint(n),
        }
    }

    pub fn to_i64(&self) -> Option<i64> {
        match &self.0 {
            Repr::Small(n) => Some(*n),
            Repr::Large(_) => None,
        }
    }

    pub fn is_zero(&self) -> bool {
        match &self.0 {
            Repr::Small(n) => *n == 0,
            Repr::Large(n) => kernel::is_zero(n),
        }
    }

    pub fn is_negative(&self) -> bool {
        match &self.0 {
            Repr::Small(n) => *n < 0,
            Repr::Large(n) => kernel::is_negative(n),
        }
    }

    pub fn is_positive(&self) -> bool {
        match &self.0 {
            Repr::Small(n) => *n > 0,
            Repr::Large(n) => kernel::is_positive(n),
        }
    }

    pub fn abs(&self) -> Self {
        match &self.0 {
            Repr::Small(n) => match n.checked_abs() {
                Some(value) => Self(Repr::Small(value)),
                None => Self::from_kernel(kernel::abs(&self.kernel())),
            },
            Repr::Large(n) => Self::from_kernel(kernel::abs(n)),
        }
    }

    /// Lean Int division is Euclidean and total, including division by zero.
    pub fn div_euclid(&self, divisor: &Self) -> Self {
        Self::from_kernel(kernel::div_euclid(&self.kernel(), &divisor.kernel()))
    }

    /// Rust and Lean Int.tmod: the remainder takes the dividend's sign, and a
    /// zero divisor leaves the dividend unchanged.
    pub fn rem_trunc(&self, divisor: &Self) -> Self {
        Self::from_kernel(kernel::rem_trunc(&self.kernel(), &divisor.kernel()))
    }

    /// Lean Int.tdiv: truncated toward zero, with a zero divisor giving zero.
    pub fn div_trunc(&self, divisor: &Self) -> Self {
        Self::from_kernel(kernel::div_trunc(&self.kernel(), &divisor.kernel()))
    }

    /// Native ABI conversion agrees with Lean UInt64.ofInt (modulo 2^64).
    pub fn to_word(&self) -> u64 {
        match &self.0 {
            Repr::Small(n) => *n as u64,
            Repr::Large(n) => kernel::low_u64(n),
        }
    }
}

fn kernel_to_bigint(value: &kernel::BigIntKernel) -> BigInt {
    let mut bytes = Vec::with_capacity(value.magnitude.limbs.len() * 8);
    let mut index = 0;
    while index < value.magnitude.limbs.len() {
        bytes.extend_from_slice(&value.magnitude.limbs[index].to_le_bytes());
        index += 1;
    }
    let magnitude = num_bigint::BigUint::from_bytes_le(&bytes);
    if kernel::is_negative(value) {
        -BigInt::from(magnitude)
    } else {
        BigInt::from(magnitude)
    }
}

fn bigint_to_kernel(value: &BigInt) -> kernel::BigIntKernel {
    let (sign, magnitude) = value.clone().into_parts();
    let bytes = magnitude.to_bytes_le();
    let mut limbs: Vec<u64> = Vec::with_capacity(bytes.len() / 8 + 1);
    let mut index = 0;
    while index < bytes.len() {
        let mut chunk = [0u8; 8];
        let mut offset = 0;
        while offset < 8 && index + offset < bytes.len() {
            chunk[offset] = bytes[index + offset];
            offset += 1;
        }
        limbs.push(u64::from_le_bytes(chunk));
        index += 8;
    }
    let negative = matches!(sign, num_bigint::Sign::Minus);
    kernel::make(negative, nat_kernel::normalize(limbs))
}

impl Default for Integer {
    fn default() -> Self {
        Self::small(0)
    }
}

impl From<BigInt> for Integer {
    fn from(n: BigInt) -> Self {
        Self::from_kernel(bigint_to_kernel(&n))
    }
}

impl From<&Integer> for Integer {
    fn from(n: &Integer) -> Self {
        n.clone()
    }
}

impl From<Integer> for BigInt {
    fn from(n: Integer) -> Self {
        n.to_bigint()
    }
}

impl From<&Integer> for BigInt {
    fn from(n: &Integer) -> Self {
        n.to_bigint()
    }
}

impl From<&Natural> for Integer {
    fn from(n: &Natural) -> Self {
        Self::from_kernel(kernel::of_nat(&kernel_nat(n)))
    }
}

impl From<Natural> for Integer {
    fn from(n: Natural) -> Self {
        Self::from(&n)
    }
}

/// The kernel magnitude behind a runtime Natural.
fn kernel_nat(value: &Natural) -> nat_kernel::BigNat {
    value.kernel_value().clone()
}

impl Ord for Integer {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (Repr::Small(a), Repr::Small(b)) => a.cmp(b),
            _ => match kernel::compare(&self.kernel(), &other.kernel()) {
                -1 => Ordering::Less,
                1 => Ordering::Greater,
                _ => Ordering::Equal,
            },
        }
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Integer {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Repr::Small(a), Repr::Small(b)) => a == b,
            (Repr::Large(a), Repr::Large(b)) => kernel::equal(a, b),
            _ => false,
        }
    }
}

impl Eq for Integer {}

impl Hash for Integer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            Repr::Small(n) => n.hash(state),
            Repr::Large(n) => {
                kernel::is_negative(n).hash(state);
                let mut index = 0;
                while index < n.magnitude.limbs.len() {
                    n.magnitude.limbs[index].hash(state);
                    index += 1;
                }
            }
        }
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::Small(n) => n.fmt(f),
            Repr::Large(n) => {
                if kernel::is_negative(n) {
                    f.write_str("-")?;
                }
                let digits = nat_kernel::to_decimal_digits(&n.magnitude);
                let mut text = String::with_capacity(digits.len());
                let mut index = 0;
                while index < digits.len() {
                    text.push(digits[index] as char);
                    index += 1;
                }
                f.write_str(&text)
            }
        }
    }
}

impl fmt::Debug for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl serde::Serialize for Integer {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let n: serde_json::Number = self.to_string().parse().expect("integer JSON number");
        n.serialize(serializer)
    }
}

macro_rules! primitive {
    ($($ty:ty),*) => { $(
        impl From<$ty> for Integer {
            fn from(n: $ty) -> Self { Self::from_kernel(kernel::from_i128(n as i128)) }
        }
        impl PartialEq<$ty> for Integer { fn eq(&self, n: &$ty) -> bool { self == &Self::from(*n) } }
        impl PartialOrd<$ty> for Integer { fn partial_cmp(&self, n: &$ty) -> Option<Ordering> { Some(self.cmp(&Self::from(*n))) } }
        impl PartialEq<Integer> for $ty { fn eq(&self, n: &Integer) -> bool { n == self } }
        impl PartialOrd<Integer> for $ty { fn partial_cmp(&self, n: &Integer) -> Option<Ordering> { Some(Integer::from(*self).cmp(n)) } }
    )* };
}
primitive!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

// The 128-bit widths do not fit the i128 round trip the macro uses.
impl From<i128> for Integer {
    fn from(n: i128) -> Self {
        Self::from_kernel(kernel::from_i128(n))
    }
}
impl From<u128> for Integer {
    fn from(n: u128) -> Self {
        Self::from_kernel(kernel::from_u128(n))
    }
}
impl PartialEq<i128> for Integer {
    fn eq(&self, n: &i128) -> bool {
        self == &Self::from(*n)
    }
}
impl PartialOrd<i128> for Integer {
    fn partial_cmp(&self, n: &i128) -> Option<Ordering> {
        Some(self.cmp(&Self::from(*n)))
    }
}
impl PartialEq<Integer> for i128 {
    fn eq(&self, n: &Integer) -> bool {
        n == self
    }
}
impl PartialOrd<Integer> for i128 {
    fn partial_cmp(&self, n: &Integer) -> Option<Ordering> {
        Some(Integer::from(*self).cmp(n))
    }
}
impl PartialEq<u128> for Integer {
    fn eq(&self, n: &u128) -> bool {
        self == &Self::from(*n)
    }
}
impl PartialOrd<u128> for Integer {
    fn partial_cmp(&self, n: &u128) -> Option<Ordering> {
        Some(self.cmp(&Self::from(*n)))
    }
}
impl PartialEq<Integer> for u128 {
    fn eq(&self, n: &Integer) -> bool {
        n == self
    }
}
impl PartialOrd<Integer> for u128 {
    fn partial_cmp(&self, n: &Integer) -> Option<Ordering> {
        Some(Integer::from(*self).cmp(n))
    }
}

macro_rules! arithmetic {
    ($trait:ident,$method:ident,$checked:ident,$kernel:path) => {
        impl<T: Into<Integer>> $trait<T> for Integer {
            type Output = Integer;
            fn $method(self, rhs: T) -> Integer {
                (&self).$method(rhs)
            }
        }
        impl<T: Into<Integer>> $trait<T> for &Integer {
            type Output = Integer;
            fn $method(self, rhs: T) -> Integer {
                let rhs = rhs.into();
                if let (Repr::Small(a), Repr::Small(b)) = (&self.0, &rhs.0) {
                    if let Some(n) = a.$checked(*b) {
                        return Integer::small(n);
                    }
                }
                Integer::from_kernel($kernel(&self.kernel(), &rhs.kernel()))
            }
        }
    };
}
arithmetic!(Add, add, checked_add, kernel::add);
arithmetic!(Sub, sub, checked_sub, kernel::sub);
arithmetic!(Mul, mul, checked_mul, kernel::mul);

impl<T: Into<Integer>> Div<T> for Integer {
    type Output = Integer;
    fn div(self, rhs: T) -> Integer {
        self.div_euclid(&rhs.into())
    }
}

impl<T: Into<Integer>> Div<T> for &Integer {
    type Output = Integer;
    fn div(self, rhs: T) -> Integer {
        self.div_euclid(&rhs.into())
    }
}

impl<T: Into<Integer>> Rem<T> for Integer {
    type Output = Integer;
    fn rem(self, rhs: T) -> Integer {
        self.rem_trunc(&rhs.into())
    }
}

impl<T: Into<Integer>> Rem<T> for &Integer {
    type Output = Integer;
    fn rem(self, rhs: T) -> Integer {
        self.rem_trunc(&rhs.into())
    }
}

impl Neg for Integer {
    type Output = Integer;
    fn neg(self) -> Integer {
        match &self.0 {
            Repr::Small(n) => match n.checked_neg() {
                Some(value) => Integer::small(value),
                None => Integer::from_kernel(kernel::neg(&self.kernel())),
            },
            Repr::Large(n) => Integer::from_kernel(kernel::neg(n)),
        }
    }
}

impl Neg for &Integer {
    type Output = Integer;
    fn neg(self) -> Integer {
        self.clone().neg()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotes_and_normalizes_without_changing_order_or_value() {
        let large = Integer::from(i64::MAX) + 1;
        assert!(large > i64::MAX);
        assert_eq!(&large - 1, Integer::from(i64::MAX));
        assert_eq!((&large * &large).to_bigint(), BigInt::from(1u8) << 126usize);
        assert_eq!(Integer::from(i64::MIN).abs(), large);
        assert_eq!(large.to_word(), 1u64 << 63);
        assert_eq!((-large.clone()).to_word(), 1u64 << 63);
        assert_eq!(large.to_i64(), None);
        assert!(large.is_positive());
        assert!((-large).is_negative());
    }

    #[test]
    fn euclidean_division_is_total() {
        for (a, b, q) in [(-5, 3, -2), (-5, -3, 2), (5, -3, -1), (5, 0, 0)] {
            assert_eq!(Integer::from(a).div_euclid(&Integer::from(b)), q);
        }
    }

    #[test]
    fn truncated_division_and_remainder_match_rust() {
        for (a, b) in [(-7i64, 3i64), (7, -3), (-7, -3), (7, 3)] {
            assert_eq!(Integer::from(a).div_trunc(&Integer::from(b)), a / b);
            assert_eq!(&Integer::from(a) % b, a % b);
        }
        assert_eq!(Integer::from(7).rem_trunc(&Integer::from(0)), 7);
        assert_eq!(Integer::from(7).div_trunc(&Integer::from(0)), 0);
    }

    #[test]
    fn bigint_boundary_roundtrip() {
        let values = [
            BigInt::from(0),
            BigInt::from(-1),
            BigInt::from(i64::MAX),
            BigInt::from(i64::MIN),
            BigInt::from(i64::MAX) + 1,
            -(BigInt::from(1u8) << 200usize),
        ];
        for value in values {
            let integer = Integer::from(value.clone());
            assert_eq!(integer.to_bigint(), value);
            assert_eq!(integer.to_string(), value.to_string());
        }
        let large = Integer::from(BigInt::from(3)) * Integer::from(BigInt::from(1u8) << 100usize);
        assert_eq!(
            large.to_bigint(),
            BigInt::from(3) * (BigInt::from(1u8) << 100usize)
        );
        assert_eq!(
            large.to_string(),
            (BigInt::from(3) * (BigInt::from(1u8) << 100usize)).to_string()
        );
    }

    #[test]
    fn natural_conversion_keeps_value() {
        let natural = Natural::from(1u128 << 100);
        let integer = Integer::from(&natural);
        assert_eq!(integer.to_bigint(), BigInt::from(1u8) << 100);
        assert_eq!(Integer::from(7u8), Integer::small(7));
    }

    #[test]
    fn wide_unsigned_conversions_stay_nonnegative() {
        let maximum = Integer::from(u128::MAX);
        assert!(!maximum.is_negative());
        assert_eq!(maximum.to_bigint(), BigInt::from(u128::MAX));
        assert!(maximum > i128::MAX);
        assert_eq!(
            Integer::from(i128::MIN).to_bigint(),
            BigInt::from(i128::MIN)
        );
        assert!(Integer::from(i128::MIN).is_negative());
        assert_eq!(Integer::from(u64::MAX).to_i64(), None);
        assert_eq!(Integer::from(1u64 << 63).to_i64(), None);
    }
}
