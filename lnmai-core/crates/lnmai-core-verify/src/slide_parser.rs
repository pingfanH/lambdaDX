//! Slide command dispatch shared with the runtime parser.
//! UTF-8 decoding, substring search, digit/error construction, and shape
//! arithmetic stay in explicit runtime adapters. No well-formedness assumption
//! is made about the facts: conflicting commands retain Lean's priority.

use crate::shape::SlideBodyKind;

pub fn keep_slide_char(c: char) -> bool {
    c != 'b' && c != 'x' && c != 'f' && c != '!' && c != '?' && c != '$'
}

#[derive(Debug, Clone, Copy)]
pub struct CommandFacts {
    pub line: bool,
    pub circle_right: bool,
    pub circle_left: bool,
    pub circle_up: bool,
    pub v: bool,
    pub pp: bool,
    pub qq: bool,
    pub p: bool,
    pub q: bool,
    pub s: bool,
    pub z: bool,
    pub turn: bool,
    pub wifi: bool,
}

/// The ordered contains-branches of Lean `parseSlideBodyFromText`.
/// In particular, a doubled p/q beats its single-character command, while
/// an earlier line/circle/v command still beats the doubled command.
pub fn select_command(facts: CommandFacts) -> Option<SlideBodyKind> {
    if facts.line {
        Some(SlideBodyKind::Line)
    } else if facts.circle_right {
        Some(SlideBodyKind::CircleRight)
    } else if facts.circle_left {
        Some(SlideBodyKind::CircleLeft)
    } else if facts.circle_up {
        Some(SlideBodyKind::CircleUp)
    } else if facts.v {
        Some(SlideBodyKind::V)
    } else if facts.pp {
        Some(SlideBodyKind::Pp)
    } else if facts.qq {
        Some(SlideBodyKind::Qq)
    } else if facts.p {
        Some(SlideBodyKind::P)
    } else if facts.q {
        Some(SlideBodyKind::Q)
    } else if facts.s {
        Some(SlideBodyKind::S)
    } else if facts.z {
        Some(SlideBodyKind::Z)
    } else if facts.turn {
        Some(SlideBodyKind::Turn)
    } else if facts.wifi {
        Some(SlideBodyKind::Wifi)
    } else {
        None
    }
}

pub fn command_end_index(kind: SlideBodyKind) -> usize {
    match kind {
        SlideBodyKind::Pp | SlideBodyKind::Qq | SlideBodyKind::Turn => 3,
        _ => 2,
    }
}

/// The adapter must read this optional turn BEFORE reading the endpoint.
pub fn command_turn_index(kind: SlideBodyKind) -> Option<usize> {
    match kind {
        SlideBodyKind::Turn => Some(2),
        _ => None,
    }
}

/// Terminal parsing deliberately observes all text, independently of the
/// command chosen above (even a lower-priority V/pp/qq forces index 3).
pub fn terminal_end_index(has_turn: bool, has_pp: bool, has_qq: bool) -> usize {
    if has_turn || has_pp || has_qq {
        3
    } else {
        2
    }
}
