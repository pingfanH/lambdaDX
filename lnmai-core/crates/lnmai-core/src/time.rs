//! Exact integer timeline and rational quantization, matching Lean Int.
//!
//! The runtime does not narrow ticks to a machine integer. Only the native
//! ABI encodes ticks modulo 2^64, exactly as Lean UInt64.ofInt does.

/// `TimeTick`: an exact microsecond count, before assigning point/duration meaning.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeTick {
    pub val: Integer,
}

impl TimeTick {
    pub fn of_int(value: impl Into<Integer>) -> Self {
        Self { val: value.into() }
    }

    pub fn to_int(&self) -> Integer {
        self.val.clone()
    }

    pub const fn zero() -> Self {
        Self {
            val: Integer::small(0),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration {
    pub micros: Integer,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimePoint {
    pub micros: Integer,
}

macro_rules! timeline_type {
    ($ty:ident) => {
        impl $ty {
            pub fn of_tick(tick: TimeTick) -> Self {
                Self { micros: tick.val }
            }
            pub fn of_int(value: impl Into<Integer>) -> Self {
                Self::from_micros(value)
            }
            pub fn to_tick(&self) -> TimeTick {
                TimeTick::of_int(self.micros.clone())
            }
            pub fn to_int(&self) -> Integer {
                self.micros.clone()
            }
            pub fn from_micros(micros: impl Into<Integer>) -> Self {
                Self {
                    micros: micros.into(),
                }
            }
            pub const fn from_const_micros(micros: i64) -> Self {
                Self {
                    micros: Integer::small(micros),
                }
            }
            pub const fn zero() -> Self {
                Self::from_const_micros(0)
            }
            pub fn to_micros(&self) -> Integer {
                self.micros.clone()
            }
        }
    };
}
timeline_type!(Duration);
timeline_type!(TimePoint);

impl Duration {
    pub fn abs(&self) -> Self {
        Self::from_micros(self.micros.abs())
    }
    pub fn scale_nat(&self, factor: impl Into<crate::natural::Natural>) -> Self {
        let factor = Integer::from(factor.into());
        Self::from_micros(&self.micros * &factor)
    }
    pub fn div_nat(&self, divisor: impl Into<crate::natural::Natural>) -> Self {
        let divisor = Integer::from(divisor.into());
        Self::from_micros(self.micros.div_euclid(&divisor))
    }
}

macro_rules! timeline_op {
    ($lhs:ident, $trait:ident, $op:ident, $rhs:ident, $out:ident) => {
        impl std::ops::$trait<$rhs> for $lhs {
            type Output = $out;
            fn $op(self, rhs: $rhs) -> $out {
                $out::from_micros(std::ops::$trait::$op(self.micros, rhs.micros))
            }
        }
    };
}
timeline_op!(Duration, Add, add, Duration, Duration);
timeline_op!(Duration, Sub, sub, Duration, Duration);
timeline_op!(TimePoint, Sub, sub, TimePoint, Duration);
timeline_op!(TimePoint, Add, add, Duration, TimePoint);
timeline_op!(TimePoint, Sub, sub, Duration, TimePoint);

impl std::ops::Neg for Duration {
    type Output = Self;

    fn neg(self) -> Self {
        Self::of_int(-self.micros)
    }
}

pub fn time_point_add_duration(point: TimePoint, duration: Duration) -> TimePoint {
    point + duration
}
pub fn time_point_sub_duration(point: TimePoint, duration: Duration) -> TimePoint {
    point - duration
}

use crate::integer::Integer;
use crate::rat::Rat;

pub const MICROS_PER_MILLI: i128 = 1000;
pub const MICROS_PER_SECOND: i128 = 1_000_000;
pub const MICROS_PER_MINUTE: i128 = 60 * MICROS_PER_SECOND;

pub fn millis_to_micros(millis: impl Into<Integer>) -> Integer {
    millis.into() * MICROS_PER_MILLI
}

/// `Time.roundDivAwayFromZero`: add/subtract half the positive denominator,
/// then use Euclidean division. This preserves the Lean formula even for
/// negative inputs where it differs from ordinary half-away rounding.
fn round_div_away_from_zero(num: &Integer, den: &Integer) -> Integer {
    if den.is_zero() {
        return Integer::small(0);
    }
    let den_pos = den.abs();
    let num_adj = if num.is_negative() {
        num - &den_pos / 2u8
    } else {
        num + &den_pos / 2u8
    };
    let quotient = num_adj.div_trunc(&den_pos);
    if num_adj.rem_trunc(&den_pos).is_negative() {
        quotient - 1
    } else {
        quotient
    }
}

/// Exact integer quantization, including values outside machine integer ranges.
pub fn quantize_rat_micros(value: Rat) -> Integer {
    let num = value.num();
    let den = Integer::from(&value.den());
    round_div_away_from_zero(&num, &den)
}

pub fn duration_from_rat_micros(value: Rat) -> Duration {
    Duration::from_micros(quantize_rat_micros(value))
}

pub fn point_from_rat_micros(value: Rat) -> TimePoint {
    TimePoint::from_micros(quantize_rat_micros(value))
}

/// `Time.bpmBeatMicrosRat`.
pub fn bpm_beat_micros_rat(bpm: Rat) -> Rat {
    if bpm.is_zero() {
        Rat::one()
    } else {
        Rat::from_int(MICROS_PER_MINUTE).div(bpm)
    }
}

/// `Time.bpmMeasureMicrosRat`.
pub fn bpm_measure_micros_rat(bpm: Rat) -> Rat {
    bpm_beat_micros_rat(bpm).mul(Rat::from_int(4))
}

/// `Time.durationFromSecondsRat`.
pub fn duration_from_seconds_rat(seconds: Rat) -> Duration {
    duration_from_rat_micros(seconds.mul(Rat::from_int(MICROS_PER_SECOND)))
}

/// `Time.pointFromSecondsRat`.
pub fn point_from_seconds_rat(seconds: Rat) -> TimePoint {
    point_from_rat_micros(seconds.mul(Rat::from_int(MICROS_PER_SECOND)))
}

/// `Time.fromMillis`.
pub fn from_millis(millis: impl Into<Integer>) -> Duration {
    Duration::from_micros(millis_to_micros(millis))
}

/// `Time.pointFromMillis`.
pub fn point_from_millis(millis: impl Into<Integer>) -> TimePoint {
    TimePoint::from_micros(millis_to_micros(millis))
}

/// Parse decimal seconds and round fractional microseconds half away from zero.
pub fn quantize_seconds_string(text: &str) -> Option<Integer> {
    let value = crate::simai::timing::parse_rat_string(text)?;
    let num = value.num();
    let den = Integer::from(&value.den());
    let numerator = num.abs() * MICROS_PER_SECOND;
    let micros = (numerator + (&den / 2u8)).div_trunc(&den);
    Some(if num.is_negative() { -micros } else { micros })
}

/// `Time.parseSecondsString?`.
pub fn parse_seconds_string(text: &str) -> Option<Duration> {
    quantize_seconds_string(text).map(Duration::from_micros)
}

/// `Time.parseSecondsPointString?`.
pub fn parse_seconds_point_string(text: &str) -> Option<TimePoint> {
    quantize_seconds_string(text).map(TimePoint::from_micros)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_measure() {
        // 120 BPM → one beat = 500000µs, one measure = 2000000µs.
        assert_eq!(
            quantize_rat_micros(bpm_beat_micros_rat(Rat::from_int(120))),
            Integer::from(500000)
        );
        assert_eq!(
            quantize_rat_micros(bpm_measure_micros_rat(Rat::from_int(120))),
            Integer::from(2000000)
        );
    }

    #[test]
    fn bpm_zero() {
        assert_eq!(bpm_beat_micros_rat(Rat::zero()), Rat::one());
    }

    #[test]
    fn seconds_string() {
        assert_eq!(
            quantize_seconds_string("1.5"),
            Some(Integer::from(1_500_000))
        );
        assert_eq!(
            quantize_seconds_string("-0.25"),
            Some(Integer::from(-250_000))
        );
        assert_eq!(quantize_seconds_string("0.0000005"), Some(Integer::from(1))); // half away from zero
        assert_eq!(quantize_seconds_string("abc"), None);
        assert_eq!(quantize_seconds_string(""), None);
    }
}
