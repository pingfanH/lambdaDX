//! Microsecond timeline types.
//!
//! Matches `LnmaiCore/Time.lean`: a `Duration`/`TimePoint` is an integer
//! microsecond tick count. Lean models the underlying value as an unbounded
//! `ℤ`; here it is an `i64`. Equivalence proofs must therefore establish the
//! game-range bounds under which the two agree.

/// A duration on the song timeline, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Duration {
    pub micros: i64,
}

impl Duration {
    pub const fn from_micros(micros: i64) -> Duration {
        Duration { micros }
    }

    pub const fn zero() -> Duration {
        Duration { micros: 0 }
    }

    pub const fn to_micros(self) -> i64 {
        self.micros
    }

    pub fn abs(self) -> Duration {
        if self.micros < 0 {
            Duration {
                micros: -self.micros,
            }
        } else {
            self
        }
    }

    pub fn scale_nat(self, factor: u32) -> Duration {
        Duration {
            micros: self.micros * (factor as i64),
        }
    }

    pub fn div_nat(self, divisor: u32) -> Duration {
        if divisor == 0 {
            Duration { micros: 0 }
        } else {
            let divisor = divisor as i64;
            let quotient = self.micros / divisor;
            let remainder = self.micros % divisor;
            Duration {
                micros: if remainder < 0 {
                    quotient - 1
                } else {
                    quotient
                },
            }
        }
    }
}

impl core::ops::Add for Duration {
    type Output = Duration;
    fn add(self, rhs: Duration) -> Duration {
        Duration {
            micros: self.micros + rhs.micros,
        }
    }
}

impl core::ops::Sub for Duration {
    type Output = Duration;
    fn sub(self, rhs: Duration) -> Duration {
        Duration {
            micros: self.micros - rhs.micros,
        }
    }
}

/// A point on the song timeline, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimePoint {
    pub micros: i64,
}

impl TimePoint {
    pub const fn from_micros(micros: i64) -> TimePoint {
        TimePoint { micros }
    }

    pub const fn zero() -> TimePoint {
        TimePoint { micros: 0 }
    }

    pub const fn to_micros(self) -> i64 {
        self.micros
    }
}

impl core::ops::Sub for TimePoint {
    type Output = Duration;
    fn sub(self, rhs: TimePoint) -> Duration {
        Duration {
            micros: self.micros - rhs.micros,
        }
    }
}
