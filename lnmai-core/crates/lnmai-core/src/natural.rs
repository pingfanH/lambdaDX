//! Exact Lean natural numbers. Subtraction and division are total.
//!
//! The value is the extracted limb-vector kernel in `lnmai_core_verify`
//! (`big_nat`), so every arithmetic step the runtime performs is a function
//! that Charon/Aeneas extracts and that the Lean proofs relate to Nat. The
//! num-bigint conversions below exist only for the JSON/FFI boundary.
use lnmai_core_verify::{big_nat, natural_parser};
use num_bigint::{BigInt, BigUint};
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::{Add, AddAssign, Div, Mul, Rem, Sub, SubAssign},
    str::FromStr,
};

#[derive(Clone)]
pub struct Natural(big_nat::BigNat);

impl Default for Natural {
    fn default() -> Self {
        Self(big_nat::zero())
    }
}

impl Natural {
    /// Lean 4.30 `String.toNat?`: decimal digits with single underscores
    /// between digits. Signs, surrounding whitespace and empty strings fail.
    pub fn from_lean_str(s: &str) -> Option<Self> {
        let chars: Vec<char> = s.chars().collect();
        natural_parser::parse_decimal(&chars, Natural::default, |value: Natural, digit| {
            Natural(big_nat::mul_add_small(&value.0, 10u64, digit as u64))
        })
    }

    pub fn to_biguint(&self) -> BigUint {
        let mut bytes = Vec::with_capacity(self.0.limbs.len() * 8);
        let mut index = 0;
        while index < self.0.limbs.len() {
            bytes.extend_from_slice(&self.0.limbs[index].to_le_bytes());
            index += 1;
        }
        BigUint::from_bytes_le(&bytes)
    }

    pub fn to_usize(&self) -> Option<usize> {
        big_nat::to_usize(&self.0)
    }

    pub fn is_zero(&self) -> bool {
        big_nat::is_zero(&self.0)
    }

    /// The extracted kernel value, shared with the integer and rational
    /// adapters so they operate on kernel limbs directly.
    pub fn kernel_value(&self) -> &big_nat::BigNat {
        &self.0
    }

    pub(crate) fn from_kernel(value: big_nat::BigNat) -> Self {
        Self(value)
    }

    /// Lean's native UInt64 conversion retains the low 64 bits.
    pub fn to_word(&self) -> u64 {
        big_nat::low_u64(&self.0)
    }

    pub fn saturating_sub(&self, rhs: impl Into<Self>) -> Self {
        self - rhs
    }

    pub fn range(&self) -> impl Iterator<Item = Self> + '_ {
        std::iter::successors(Some(Self::default()), |n| Some(n + 1)).take_while(|n| n < self)
    }
}

impl From<BigUint> for Natural {
    fn from(n: BigUint) -> Self {
        let bytes = n.to_bytes_le();
        let mut limbs = Vec::with_capacity(bytes.len() / 8 + 1);
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
        Natural(big_nat::normalize(limbs))
    }
}

impl From<Natural> for BigUint {
    fn from(n: Natural) -> Self {
        n.to_biguint()
    }
}
impl From<&Natural> for BigUint {
    fn from(n: &Natural) -> Self {
        n.to_biguint()
    }
}
impl From<Natural> for BigInt {
    fn from(n: Natural) -> Self {
        n.to_biguint().into()
    }
}
impl From<&Natural> for BigInt {
    fn from(n: &Natural) -> Self {
        n.to_biguint().into()
    }
}
impl From<&Natural> for Natural {
    fn from(n: &Natural) -> Self {
        n.clone()
    }
}
impl FromStr for Natural {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        let mut index = 0;
        if !bytes.is_empty() && bytes[0] == b'+' {
            index = 1;
        }
        if index == bytes.len() {
            return Err("invalid digit found in string".into());
        }
        let mut value = Natural::default();
        while index < bytes.len() {
            let digit = bytes[index];
            if !digit.is_ascii_digit() {
                return Err("invalid digit found in string".into());
            }
            value = Natural(big_nat::mul_add_small(
                &value.0,
                10u64,
                (digit - b'0') as u64,
            ));
            index += 1;
        }
        Ok(value)
    }
}
impl fmt::Display for Natural {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = big_nat::to_decimal_digits(&self.0);
        let mut text = String::with_capacity(digits.len());
        let mut index = 0;
        while index < digits.len() {
            text.push(digits[index] as char);
            index += 1;
        }
        f.write_str(&text)
    }
}
impl fmt::Debug for Natural {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl serde::Serialize for Natural {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_string()
            .parse::<serde_json::Number>()
            .expect("natural JSON number")
            .serialize(s)
    }
}
impl PartialEq for Natural {
    fn eq(&self, other: &Self) -> bool {
        big_nat::equal(&self.0, &other.0)
    }
}
impl Eq for Natural {}
impl Hash for Natural {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut index = 0;
        while index < self.0.limbs.len() {
            self.0.limbs[index].hash(state);
            index += 1;
        }
        self.0.limbs.len().hash(state);
    }
}
impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        match big_nat::compare(&self.0, &other.0) {
            -1 => Ordering::Less,
            1 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

macro_rules! primitive {
    ($($ty:ty),*) => { $(
        impl From<$ty> for Natural {
            fn from(n: $ty) -> Self {
                let wide = n as u128;
                Self(big_nat::from_u64_pair((wide >> 64) as u64, wide as u64))
            }
        }
        impl PartialEq<$ty> for Natural { fn eq(&self, n: &$ty) -> bool { self == &Self::from(*n) } }
        impl PartialOrd<$ty> for Natural { fn partial_cmp(&self, n: &$ty) -> Option<Ordering> { Some(self.cmp(&Self::from(*n))) } }
        impl PartialEq<Natural> for $ty { fn eq(&self, n: &Natural) -> bool { n == self } }
        impl PartialOrd<Natural> for $ty { fn partial_cmp(&self, n: &Natural) -> Option<Ordering> { Some(Natural::from(*self).cmp(n)) } }
    )* };
}
primitive!(u8, u16, u32, u64, u128, usize);
// Unsuffixed nonnegative Rust literals are i32. Fallible external conversion
// uses the JSON decoder / FromStr instead.
impl From<i32> for Natural {
    fn from(n: i32) -> Self {
        Self(big_nat::from_u64(
            u32::try_from(n).expect("nonnegative natural literal") as u64,
        ))
    }
}
impl PartialEq<i32> for Natural {
    fn eq(&self, n: &i32) -> bool {
        *n >= 0 && self == &Self::from(*n)
    }
}
impl PartialOrd<i32> for Natural {
    fn partial_cmp(&self, n: &i32) -> Option<Ordering> {
        Some(if *n < 0 {
            Ordering::Greater
        } else {
            self.cmp(&Self::from(*n))
        })
    }
}

macro_rules! operation {
    ($trait:ident,$method:ident,$body:expr) => {
        impl<T: Into<Natural>> $trait<T> for &Natural {
            type Output = Natural;
            fn $method(self, rhs: T) -> Natural {
                let rhs = rhs.into();
                Natural(($body)(&self.0, &rhs.0))
            }
        }
        impl<T: Into<Natural>> $trait<T> for Natural {
            type Output = Natural;
            fn $method(self, rhs: T) -> Natural {
                (&self).$method(rhs)
            }
        }
    };
}
operation!(Add, add, |a: &big_nat::BigNat, b: &big_nat::BigNat| {
    big_nat::add(a, b)
});
operation!(Mul, mul, |a: &big_nat::BigNat, b: &big_nat::BigNat| {
    big_nat::mul(a, b)
});
operation!(Sub, sub, |a: &big_nat::BigNat, b: &big_nat::BigNat| {
    big_nat::sub(a, b)
});
operation!(Div, div, |a: &big_nat::BigNat, b: &big_nat::BigNat| {
    big_nat::div(a, b)
});
operation!(Rem, rem, |a: &big_nat::BigNat, b: &big_nat::BigNat| {
    big_nat::rem(a, b)
});
impl<T: Into<Natural>> AddAssign<T> for Natural {
    fn add_assign(&mut self, rhs: T) {
        *self = &*self + rhs;
    }
}
impl<T: Into<Natural>> SubAssign<T> for Natural {
    fn sub_assign(&mut self, rhs: T) {
        *self = &*self - rhs;
    }
}
impl std::iter::Sum for Natural {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |a, b| a + b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_total_arithmetic_and_native_conversion() {
        let n = Natural::from(BigUint::from(1u8) << 1024);
        assert_eq!((&n + 17).to_word(), 17);
        assert_eq!((&n + 17) - &n, 17);
        assert_eq!((&n - (&n + 1)), 0);
        assert_eq!((&n / 0), 0);
        assert_eq!((&n % 0), n);
        assert_eq!((&n * &n) / &n, n);
        assert_eq!(n.to_usize(), None);
    }

    #[test]
    fn biguint_roundtrip_and_text() {
        let values = [
            BigUint::from(0u8),
            BigUint::from(1u8),
            BigUint::from(u64::MAX),
            BigUint::from(u64::MAX) + 1u8,
            BigUint::from(1u8) << 1024,
            BigUint::from(123456789012345678901234567890u128),
        ];
        for value in values {
            let natural = Natural::from(value.clone());
            assert_eq!(natural.to_biguint(), value);
            assert_eq!(natural.to_string(), value.to_string());
            assert_eq!(natural.to_string().parse::<Natural>().unwrap(), natural);
        }
        assert!(Natural::from_lean_str("1_000").unwrap() == 1000u32);
        assert!(Natural::from_lean_str("_").is_none());
        assert!(Natural::from_lean_str("").is_none());
        assert!("12a".parse::<Natural>().is_err());
        assert_eq!("+12".parse::<Natural>().unwrap(), Natural::from(12u32));
    }
}
