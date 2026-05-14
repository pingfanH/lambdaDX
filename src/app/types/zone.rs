use std::cmp::Ordering;
use serde::{Deserialize, Serialize};

/// Pad Touch Zone
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadZone {
    A1, A2, A3, A4, A5, A6, A7, A8,
    B1, B2, B3, B4, B5, B6, B7, B8,
    C,
    D1, D2, D3, D4, D5, D6, D7, D8,
    E1, E2, E3, E4, E5, E6, E7, E8,
}
impl PadZone {
    pub fn to_id(self)->u8{
        zone_to_id(self)
    }
}

impl From<String> for PadZone {
    fn from(id: String) -> Self {
        PadZone::from(id.as_str())
    }
}
impl From<&str> for PadZone {
    fn from(value: &str) -> Self {
        svg_id_to_zone(value).unwrap()
    }
}
impl From<u8> for PadZone {
    fn from(value:u8) -> Self {
        id_to_zone(value).unwrap()
    }
}
impl std::fmt::Display for PadZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PadZone::A1 => "A1",
            PadZone::A2 => "A2",
            PadZone::A3 => "A3",
            PadZone::A4 => "A4",
            PadZone::A5 => "A5",
            PadZone::A6 => "A6",
            PadZone::A7 => "A7",
            PadZone::A8 => "A8",
            PadZone::B1 => "B1",
            PadZone::B2 => "B2",
            PadZone::B3 => "B3",
            PadZone::B4 => "B4",
            PadZone::B5 => "B5",
            PadZone::B6 => "B6",
            PadZone::B7 => "B7",
            PadZone::B8 => "B8",
            PadZone::C => "C",
            PadZone::D1 => "D1",
            PadZone::D2 => "D2",
            PadZone::D3 => "D3",
            PadZone::D4 => "D4",
            PadZone::D5 => "D5",
            PadZone::D6 => "D6",
            PadZone::D7 => "D7",
            PadZone::D8 => "D8",
            PadZone::E1 => "E1",
            PadZone::E2 => "E2",
            PadZone::E3 => "E3",
            PadZone::E4 => "E4",
            PadZone::E5 => "E5",
            PadZone::E6 => "E6",
            PadZone::E7 => "E7",
            PadZone::E8 => "E8",
        };
        write!(f, "{}", s)
    }
}
pub fn svg_id_to_zone(id: &str) -> Option<PadZone> {
    match id {
        // Outer ring (zones 1-8)
        "A1" => Some(PadZone::A1),
        "A2" => Some(PadZone::A2),
        "A3" => Some(PadZone::A3),
        "A4" => Some(PadZone::A4),
        "A5" => Some(PadZone::A5),
        "A6" => Some(PadZone::A6),
        "A7" => Some(PadZone::A7),
        "A8" => Some(PadZone::A8),
        // Inner ring (zones 9-16)
        "B1" => Some(PadZone::B1),
        "B2" => Some(PadZone::B2),
        "B3" => Some(PadZone::B3),
        "B4" => Some(PadZone::B4),
        "B5" => Some(PadZone::B5),
        "B6" => Some(PadZone::B6),
        "B7" => Some(PadZone::B7),
        "B8" => Some(PadZone::B8),
        // Center zone
        "C" | "C1" => Some(PadZone::C),
        // Left wing (zones 18-25)
        "D1" => Some(PadZone::D1),
        "D2" => Some(PadZone::D2),
        "D3" => Some(PadZone::D3),
        "D4" => Some(PadZone::D4),
        "D5" => Some(PadZone::D5),
        "D6" => Some(PadZone::D6),
        "D7" => Some(PadZone::D7),
        "D8" => Some(PadZone::D8),
        // Right wing (zones 26-33)
        "E1" => Some(PadZone::E1),
        "E1-2" => Some(PadZone::E2),
        "E1-3" => Some(PadZone::E3),
        "E1-4" => Some(PadZone::E4),
        "E1-5" => Some(PadZone::E5),
        "E1-6" => Some(PadZone::E6),
        "E1-7" => Some(PadZone::E7),
        "E1-8" => Some(PadZone::E8),
        _ => None,
    }
}
pub fn id_to_zone(id: u8) -> Option<PadZone> {
    match id {
        1 => Some(PadZone::A1), 2 => Some(PadZone::A2), 3 => Some(PadZone::A3), 4 => Some(PadZone::A4),
        5 => Some(PadZone::A5), 6 => Some(PadZone::A6), 7 => Some(PadZone::A7), 8 => Some(PadZone::A8),
        9 => Some(PadZone::B1), 10 => Some(PadZone::B2), 11 => Some(PadZone::B3), 12 => Some(PadZone::B4),
        13 => Some(PadZone::B5), 14 => Some(PadZone::B6), 15 => Some(PadZone::B7), 16 => Some(PadZone::B8),
        17 => Some(PadZone::C),
        18 => Some(PadZone::D1), 19 => Some(PadZone::D2), 20 => Some(PadZone::D3), 21 => Some(PadZone::D4),
        22 => Some(PadZone::D5), 23 => Some(PadZone::D6), 24 => Some(PadZone::D7), 25 => Some(PadZone::D8),
        26 => Some(PadZone::E1), 27 => Some(PadZone::E2), 28 => Some(PadZone::E3), 29 => Some(PadZone::E4),
        30 => Some(PadZone::E5), 31 => Some(PadZone::E6), 32 => Some(PadZone::E7), 33 => Some(PadZone::E8),
        _ => None,
    }
}
pub fn zone_to_id(zone: PadZone) -> u8 {
    match zone {
        PadZone::A1 => 1, PadZone::A2 => 2, PadZone::A3 => 3, PadZone::A4 => 4,
        PadZone::A5 => 5, PadZone::A6 => 6, PadZone::A7 => 7, PadZone::A8 => 8,
        PadZone::B1 => 9, PadZone::B2 => 10, PadZone::B3 => 11, PadZone::B4 => 12,
        PadZone::B5 => 13, PadZone::B6 => 14, PadZone::B7 => 15, PadZone::B8 => 16,
        PadZone::C => 17,
        PadZone::D1 => 18, PadZone::D2 => 19, PadZone::D3 => 20, PadZone::D4 => 21,
        PadZone::D5 => 22, PadZone::D6 => 23, PadZone::D7 => 24, PadZone::D8 => 25,
        PadZone::E1 => 26, PadZone::E2 => 27, PadZone::E3 => 28, PadZone::E4 => 29,
        PadZone::E5 => 30, PadZone::E6 => 31, PadZone::E7 => 32, PadZone::E8 => 33,
    }
}


// ==================== Partia ====================
macro_rules! impl_cmp_for_zone {
    ($($t:ty),*) => {
        $(
            impl PartialEq<$t> for PadZone {
                fn eq(&self, other: &$t) -> bool {
                    self.to_id() as $t == *other
                }
            }

            impl PartialEq<PadZone> for $t {
                fn eq(&self, other: &PadZone) -> bool {
                    *self == other.to_id() as $t
                }
            }

            impl PartialOrd<$t> for PadZone {
                fn partial_cmp(&self, other: &$t) -> Option<Ordering> {
                    (self.to_id() as $t).partial_cmp(other)
                }
                fn lt(&self, other: &$t) -> bool { (self.to_id() as $t) < *other }
                fn le(&self, other: &$t) -> bool { (self.to_id() as $t) <= *other }
                fn gt(&self, other: &$t) -> bool { (self.to_id() as $t) > *other }
                fn ge(&self, other: &$t) -> bool { (self.to_id() as $t) >= *other }
            }

            impl PartialOrd<PadZone> for $t {
                fn partial_cmp(&self, other: &PadZone) -> Option<Ordering> {
                    self.partial_cmp(&(other.to_id() as $t))
                }
                fn lt(&self, other: &PadZone) -> bool { *self < other.to_id() as $t }
                fn le(&self, other: &PadZone) -> bool { *self <= other.to_id() as $t }
                fn gt(&self, other: &PadZone) -> bool { *self > other.to_id() as $t }
                fn ge(&self, other: &PadZone) -> bool { *self >= other.to_id() as $t }
            }
        )*
    };
}

impl_cmp_for_zone!(u8, u16, u32, u64, usize, i32, i64);

use std::ops::{Add, Sub, Mul, Div, AddAssign, SubAssign, MulAssign, DivAssign};

macro_rules! impl_arithmetic_for_zone {
    ($($t:ty),*) => {
        $(
            // ========== PadZone + $t ==========
            impl Add<$t> for PadZone {
                type Output = PadZone;
                fn add(self, rhs: $t) -> PadZone {
                    let new_id = (self.to_id() as i32 + rhs as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            impl Add<$t> for &PadZone {
                type Output = PadZone;
                fn add(self, rhs: $t) -> PadZone {
                    let new_id = (self.to_id() as i32 + rhs as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            // ========== $t + PadZone ==========
            impl Add<PadZone> for $t {
                type Output = PadZone;
                fn add(self, rhs: PadZone) -> PadZone {
                    let new_id = (self as i32 + rhs.to_id() as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            // ========== PadZone - $t ==========
            impl Sub<$t> for PadZone {
                type Output = PadZone;
                fn sub(self, rhs: $t) -> PadZone {
                    let new_id = (self.to_id() as i32 - rhs as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            impl Sub<$t> for &PadZone {
                type Output = PadZone;
                fn sub(self, rhs: $t) -> PadZone {
                    let new_id = (self.to_id() as i32 - rhs as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            // ========== $t - PadZone ==========
            impl Sub<PadZone> for $t {
                type Output = PadZone;
                fn sub(self, rhs: PadZone) -> PadZone {
                    let new_id = (self as i32 - rhs.to_id() as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            // ========== PadZone * $t ==========
            impl Mul<$t> for PadZone {
                type Output = PadZone;
                fn mul(self, rhs: $t) -> PadZone {
                    let new_id = (self.to_id() as i32 * rhs as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            impl Mul<$t> for &PadZone {
                type Output = PadZone;
                fn mul(self, rhs: $t) -> PadZone {
                    let new_id = (self.to_id() as i32 * rhs as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            impl Mul<PadZone> for $t {
                type Output = PadZone;
                fn mul(self, rhs: PadZone) -> PadZone {
                    let new_id = (self as i32 * rhs.to_id() as i32).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            // ========== PadZone / $t ==========
            impl Div<$t> for PadZone {
                type Output = PadZone;
                fn div(self, rhs: $t) -> PadZone {
                    let rhs = rhs as i32;
                    if rhs == 0 {
                        return self; // 或 panic，视业务需求
                    }
                    let new_id = (self.to_id() as i32 / rhs).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            impl Div<$t> for &PadZone {
                type Output = PadZone;
                fn div(self, rhs: $t) -> PadZone {
                    let rhs = rhs as i32;
                    if rhs == 0 {
                        return *self;
                    }
                    let new_id = (self.to_id() as i32 / rhs).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            impl Div<PadZone> for $t {
                type Output = PadZone;
                fn div(self, rhs: PadZone) -> PadZone {
                    let rhs_id = rhs.to_id() as i32;
                    if rhs_id == 0 {
                        return PadZone::from(self.clamp(1, 33) as u8);
                    }
                    let new_id = (self as i32 / rhs_id).clamp(1, 33) as u8;
                    PadZone::from(new_id)
                }
            }

            // ========== 复合赋值 += -= *= /= ==========
            impl AddAssign<$t> for PadZone {
                fn add_assign(&mut self, rhs: $t) {
                    let new_id = (self.to_id() as i32 + rhs as i32).clamp(1, 33) as u8;
                    *self = PadZone::from(new_id);
                }
            }

            impl SubAssign<$t> for PadZone {
                fn sub_assign(&mut self, rhs: $t) {
                    let new_id = (self.to_id() as i32 - rhs as i32).clamp(1, 33) as u8;
                    *self = PadZone::from(new_id);
                }
            }

            impl MulAssign<$t> for PadZone {
                fn mul_assign(&mut self, rhs: $t) {
                    let new_id = (self.to_id() as i32 * rhs as i32).clamp(1, 33) as u8;
                    *self = PadZone::from(new_id);
                }
            }

            impl DivAssign<$t> for PadZone {
                fn div_assign(&mut self, rhs: $t) {
                    let rhs = rhs as i32;
                    if rhs == 0 {
                        return;
                    }
                    let new_id = (self.to_id() as i32 / rhs).clamp(1, 33) as u8;
                    *self = PadZone::from(new_id);
                }
            }
        )*
    };
}

impl_arithmetic_for_zone!(u8, u16, u32, u64, usize, i8, i16, i32, i64);