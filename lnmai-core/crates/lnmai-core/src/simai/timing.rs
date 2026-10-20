//! Simai timing-string parsing.
//!
//! Mirrors `LnmaiCore/Simai/Timing.lean`.

use crate::natural::Natural;
use crate::rat::Rat;
use crate::time::{self, Duration};
use lnmai_core_verify::timing_aggregate;
use lnmai_core_verify::timing_number;
use lnmai_core_verify::timing_parser::{self, DurationRoute};

/// `Timing.trim`.
pub fn trim(s: &str) -> String {
    s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'))
        .to_string()
}

/// `Timing.parseNatString?`.
pub fn parse_nat_string(s: &str) -> Option<Natural> {
    Natural::from_lean_str(&trim(s))
}

/// `Timing.parseRatString?`.
pub fn parse_rat_string(s: &str) -> Option<Rat> {
    let chars: Vec<char> = trim(s).chars().collect();
    let parts = timing_number::parse(&chars, &TimingDecimalOps)?;
    let whole = Rat::from_int(parts.whole);
    let value = match parts.fraction {
        None => whole,
        Some((numerator, denominator)) => {
            whole.add(Rat::from_int(numerator).div(Rat::from_int(denominator)))
        }
    };
    Some(if parts.negative {
        Rat::zero().sub(value)
    } else {
        value
    })
}

struct TimingDecimalOps;

impl timing_number::DecimalOps<Natural> for TimingDecimalOps {
    fn zero(&self) -> Natural {
        Natural::from(0u8)
    }

    fn one(&self) -> Natural {
        Natural::from(1u8)
    }

    fn append(&self, value: Natural, digit: u8) -> Natural {
        value * 10u8 + digit
    }
}

/// `Timing.parseRatDef`.
pub fn parse_rat_def(s: &str, fallback: Rat) -> Rat {
    parse_rat_string(s).unwrap_or(fallback)
}

/// `Timing.parseNatDef`.
pub fn parse_nat_def(s: &str, fallback: Natural) -> Natural {
    parse_nat_string(s).unwrap_or(fallback)
}

/// `Timing.parseDurationString?`.
pub fn parse_duration_string(s: &str) -> Option<Duration> {
    time::parse_seconds_string(s)
}

/// `Timing.parseSecondsRatString?`.
pub fn parse_seconds_rat_string(s: &str) -> Option<Rat> {
    parse_rat_string(s)
}

/// `Timing.measureDurSec`.
pub fn measure_dur_sec(bpm: Rat) -> Duration {
    time::duration_from_rat_micros(time::bpm_measure_micros_rat(bpm))
}

/// `Timing.beatSec`.
pub fn beat_sec(bpm: Rat) -> Duration {
    time::duration_from_rat_micros(time::bpm_beat_micros_rat(bpm))
}

/// `Timing.extractBracketContents`.
pub fn extract_bracket_contents(token: &str) -> Vec<String> {
    let chars: Vec<char> = token.chars().collect();
    timing_parser::extract_brackets(&chars)
        .into_iter()
        .map(|piece| piece.into_iter().collect())
        .collect()
}

/// `Timing.splitHash2`.
pub fn split_hash2(s: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = s.split('#').collect();
    match parts.as_slice() {
        [a, b, c] => Some((a.to_string(), b.to_string(), c.to_string())),
        _ => None,
    }
}

/// `Timing.splitHash1`.
pub fn split_hash1(s: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = s.split('#').collect();
    match parts.as_slice() {
        [a, b] => Some((a.to_string(), b.to_string())),
        _ => None,
    }
}

fn parse_nd_micros(bpm: Rat, timing: &str) -> Option<Rat> {
    let parts: Vec<&str> = timing.split(':').collect();
    match parts.as_slice() {
        [division, beats] => {
            let division = parse_nat_string(division)?;
            let beats = parse_nat_string(beats)?;
            if division == 0 {
                None
            } else {
                Some(
                    time::bpm_measure_micros_rat(bpm)
                        .mul(Rat::from_int(beats))
                        .div(Rat::from_int(division)),
                )
            }
        }
        _ => None,
    }
}

pub fn parse_nd_duration(bpm: Rat, timing: &str) -> Option<Duration> {
    parse_nd_micros(bpm, timing).map(time::duration_from_rat_micros)
}

pub fn parse_nd_duration_exact(bpm: Rat, timing: &str) -> Option<Duration> {
    parse_nd_duration(bpm, timing)
}

fn parse_seconds_micros(text: &str) -> Option<Rat> {
    parse_seconds_rat_string(text).map(|seconds| seconds.mul(Rat::from_int(1_000_000)))
}

fn parse_ratio_or_seconds_micros(bpm: Rat, text: &str) -> Option<Rat> {
    parse_nd_micros(bpm, text).or_else(|| parse_seconds_micros(text))
}

fn duration_bpm(current_bpm: Rat, text: &str) -> Rat {
    parse_rat_string(text)
        .filter(|value| Rat::zero().lt(value))
        .unwrap_or(current_bpm)
}

fn parse_duration_inner_micros(current_bpm: Rat, inner: &str) -> Option<Rat> {
    let parts: Vec<&str> = inner.split('#').collect();
    let fields: Vec<Vec<char>> = parts.iter().map(|field| field.chars().collect()).collect();
    match timing_parser::duration_route(&fields) {
        DurationRoute::Reject => None,
        DurationRoute::CurrentRatioOrSeconds { timing } => {
            parse_ratio_or_seconds_micros(current_bpm, parts[timing])
        }
        DurationRoute::SecondsOnly { timing } => parse_seconds_micros(parts[timing]),
        DurationRoute::CustomRatioOrSeconds { bpm, timing } => {
            parse_ratio_or_seconds_micros(duration_bpm(current_bpm, parts[bpm]), parts[timing])
        }
        DurationRoute::CustomRatioOnly { bpm, timing } => {
            parse_nd_micros(duration_bpm(current_bpm, parts[bpm]), parts[timing])
        }
    }
}

pub fn parse_duration_inner(current_bpm: Rat, inner: &str) -> Option<Duration> {
    parse_duration_inner_micros(current_bpm, inner).map(time::duration_from_rat_micros)
}

pub fn parse_duration_spec(bpm: Rat, token: &str) -> Option<Duration> {
    let contents = extract_bracket_contents(token);
    timing_aggregate::sum_then_map_indexed(
        contents.len(),
        |index| parse_duration_inner_micros(bpm.clone(), &contents[index]),
        |sum: Rat, duration| sum.add(duration),
        time::duration_from_rat_micros,
    )
}

pub fn parse_star_wait_spec(bpm: Rat, token: &str) -> Option<Duration> {
    let contents = extract_bracket_contents(token);
    let custom = timing_aggregate::first_some_indexed(contents.len(), |index| {
        let parts: Vec<&str> = contents[index].split('#').collect();
        match parts.as_slice() {
            [custom_bpm, _] => parse_rat_string(custom_bpm).map(|value| {
                beat_sec(if Rat::zero().lt(&value) {
                    value
                } else {
                    bpm.clone()
                })
            }),
            [wait, _, _] | [wait, _, _, _] => {
                parse_seconds_rat_string(wait).map(time::duration_from_seconds_rat)
            }
            _ => None,
        }
    });
    Some(custom.unwrap_or_else(|| beat_sec(bpm)))
}

/// `Timing.noteTimingIncrement`.
pub fn note_timing_increment(bpm: Rat, divisor: Natural) -> Duration {
    if Rat::zero().lt(&bpm) && divisor > 0 {
        time::duration_from_rat_micros(
            time::bpm_measure_micros_rat(bpm).div(Rat::from_int(divisor)),
        )
    } else {
        Duration::zero()
    }
}

/// `Timing.pseudoIncrement`.
pub fn pseudo_increment(bpm: Rat) -> Duration {
    if Rat::zero().lt(&bpm) {
        time::duration_from_rat_micros(time::bpm_beat_micros_rat(bpm).div(Rat::from_int(32)))
    } else {
        Duration::from_micros(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rat_string() {
        assert_eq!(parse_rat_string("1.5"), Some(Rat::new(3, 2)));
        assert_eq!(parse_rat_string("-0.25"), Some(Rat::new(-1, 4)));
        assert_eq!(parse_rat_string("120"), Some(Rat::from_int(120)));
        assert_eq!(parse_rat_string("1.2.3"), None);
    }

    #[test]
    fn nd_duration() {
        // 120 BPM, "4:1" = one quarter note = 0.5s.
        let d = parse_nd_duration(Rat::from_int(120), "4:1").unwrap();
        assert_eq!(d.to_micros(), 500_000);
        assert_eq!(parse_nd_duration(Rat::from_int(120), "0:1"), None);
    }

    #[test]
    fn bracket_contents() {
        assert_eq!(
            extract_bracket_contents("1-3[4:1][8:1]"),
            vec!["4:1", "8:1"]
        );
        assert_eq!(extract_bracket_contents("[#0.5]"), vec!["#0.5"]);
    }

    #[test]
    fn duration_spec_sum() {
        // Two quarter-note segments sum to 1s at 120 BPM.
        let d = parse_duration_spec(Rat::from_int(120), "[4:1][4:1]").unwrap();
        assert_eq!(d.to_micros(), 1_000_000);
    }

    #[test]
    fn star_wait_default_is_beat() {
        assert_eq!(
            parse_star_wait_spec(Rat::from_int(120), "1")
                .unwrap()
                .to_micros(),
            500_000
        );
    }

    #[test]
    fn note_increment_bpm_zero() {
        assert_eq!(
            note_timing_increment(Rat::zero(), (4).into()).to_micros(),
            0
        );
        assert_eq!(pseudo_increment(Rat::zero()).to_micros(), 1000);
    }
}
