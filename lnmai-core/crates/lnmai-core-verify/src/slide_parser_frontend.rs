//! Character reads and just-type dispatch shared with the runtime parser.
//! UTF-8 decoding, unbounded index conversion, and diagnostic String allocation
//! are runtime adapters. All character scans and branch/error choices live here.

use crate::areas::{OuterSlot, SensorArea};
use crate::shape::{self, ShapeError};

#[derive(Debug, Clone, Copy)]
pub enum ReadError {
    MissingDigit,
    ExpectedDigit,
    InvalidStartLane,
    InvalidEndArea,
}

#[derive(Debug, Clone, Copy)]
pub enum JustError {
    At(ReadError, usize),
    Shape(ShapeError),
    EndNotOuter,
}

pub fn get_at(chars: &[char], index: usize) -> Option<char> {
    if index < chars.len() {
        Some(chars[index])
    } else {
        None
    }
}

/// Accept only ASCII 1..=8, including when the input is an arbitrary Unicode char.
pub fn read_digit(found: Option<char>) -> Result<usize, ReadError> {
    match found {
        Some(c) => match shape::digit_to_nat(c) {
            Some(n) => Ok(n),
            None => Err(ReadError::ExpectedDigit),
        },
        None => Err(ReadError::MissingDigit),
    }
}

macro_rules! read_try {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
    };
}

pub fn read_start_lane(found: Option<char>) -> Result<OuterSlot, ReadError> {
    let pos = read_try!(read_digit(found));
    match OuterSlot::of_index(pos.saturating_sub(1)) {
        Some(slot) => Ok(slot),
        None => Err(ReadError::InvalidStartLane),
    }
}

pub fn read_end_area(found: Option<char>) -> Result<SensorArea, ReadError> {
    let pos = read_try!(read_digit(found));
    match OuterSlot::of_index(pos.saturating_sub(1)) {
        Some(slot) => Ok(slot.to_outer_sensor_area()),
        None => Err(ReadError::InvalidEndArea),
    }
}

/// Search the entire character sequence, including malformed or mixed commands.
pub fn contains_char(chars: &[char], target: char) -> bool {
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == target {
            return true;
        }
        index += 1;
    }
    false
}

/// Consecutive equal characters only: p...p does not count as pp.
pub fn contains_double(chars: &[char], target: char) -> bool {
    let mut previous = false;
    let mut index = 0usize;
    while index < chars.len() {
        let current = chars[index] == target;
        if previous && current {
            return true;
        }
        previous = current;
        index += 1;
    }
    false
}

#[derive(Debug, Clone, Copy)]
pub struct JustFacts {
    pub circle_right: bool,
    pub circle_left: bool,
    pub circle_up: bool,
    pub turn: bool,
    pub wifi: bool,
    pub qq: bool,
    pub pp: bool,
}

pub fn just_facts(chars: &[char]) -> JustFacts {
    JustFacts {
        circle_right: contains_char(chars, '>'),
        circle_left: contains_char(chars, '<'),
        circle_up: contains_char(chars, '^'),
        turn: contains_char(chars, 'V'),
        wifi: contains_char(chars, 'w'),
        qq: contains_double(chars, 'q'),
        pp: contains_double(chars, 'p'),
    }
}

fn just_start(found: Option<char>) -> Result<OuterSlot, JustError> {
    match read_start_lane(found) {
        Ok(slot) => Ok(slot),
        Err(error) => Err(JustError::At(error, 0)),
    }
}

fn just_end(found: Option<char>, index: usize) -> Result<SensorArea, JustError> {
    match read_end_area(found) {
        Ok(area) => Ok(area),
        Err(error) => Err(JustError::At(error, index)),
    }
}

fn just_outer(area: SensorArea) -> Result<OuterSlot, JustError> {
    match area.to_outer_slot() {
        Some(slot) => Ok(slot),
        None => Err(JustError::EndNotOuter),
    }
}

/// The priority is >, <, ^, V, w, default; it is independent of shape dispatch.
/// In the default branch the start character is deliberately never validated.
pub fn detect_just_type_observed(
    facts: JustFacts,
    start: Option<char>,
    end2: Option<char>,
    end3: Option<char>,
) -> Result<bool, JustError> {
    if facts.circle_right {
        let start_lane = read_try!(just_start(start));
        let _end = read_try!(just_end(end2, 2));
        Ok(shape::outer_slot_is_upper_half(start_lane))
    } else if facts.circle_left {
        let start_lane = read_try!(just_start(start));
        let _end = read_try!(just_end(end2, 2));
        Ok(!shape::outer_slot_is_upper_half(start_lane))
    } else if facts.circle_up {
        let start_lane = read_try!(just_start(start));
        let end_area = read_try!(just_end(end2, 2));
        match shape::relative_end_from_typed(start_lane, end_area) {
            Ok(rel_end) => Ok(rel_end < 4),
            Err(error) => Err(JustError::Shape(error)),
        }
    } else if facts.turn {
        let _start = read_try!(just_start(start));
        let end_area = read_try!(just_end(end3, 3));
        let end_zone = read_try!(just_outer(end_area));
        Ok(shape::outer_slot_is_right_half(end_zone))
    } else if facts.wifi {
        let _start = read_try!(just_start(start));
        let end_area = read_try!(just_end(end2, 2));
        let end_zone = read_try!(just_outer(end_area));
        Ok(shape::outer_slot_is_upper_half(end_zone))
    } else {
        let end_area = if facts.qq || facts.pp {
            read_try!(just_end(end3, 3))
        } else {
            read_try!(just_end(end2, 2))
        };
        let end_zone = read_try!(just_outer(end_area));
        Ok(shape::outer_slot_is_right_half(end_zone))
    }
}

pub fn detect_just_type(chars: &[char]) -> Result<bool, JustError> {
    let facts = just_facts(chars);
    detect_just_type_observed(facts, get_at(chars, 0), get_at(chars, 2), get_at(chars, 3))
}
