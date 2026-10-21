//! Pure Simai shape operations shared by the runtime and the extracted model.
//! Mirrors LnmaiCore/Simai/Shape.lean; textual parsing stays at the boundary.

use crate::areas::{OuterSlot, SensorArea};
use crate::symmetry::{self, SlideSymmetry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlideKind {
    #[default]
    Line,
    Circle,
    V,
    Turn,
    Pq,
    Ppqq,
    S,
    Wifi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalSlideShape {
    Line(usize),
    Circle(usize),
    V(usize),
    Turn(usize),
    Pq(usize),
    Ppqq(usize),
    S,
    Wifi,
}

impl Default for CanonicalSlideShape {
    fn default() -> Self {
        Self::Line(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideShape {
    pub canonical: CanonicalSlideShape,
    pub symmetry: SlideSymmetry,
}

impl Default for SlideShape {
    fn default() -> Self {
        SlideShape {
            canonical: CanonicalSlideShape::Line(0),
            symmetry: crate::symmetry::direct(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlideBodyKind {
    #[default]
    Line,
    CircleRight,
    CircleLeft,
    CircleUp,
    V,
    Pp,
    Qq,
    P,
    Q,
    S,
    Z,
    Turn,
    Wifi,
}

/// Structured errors preserve Lean's distinction between a non-outer area,
/// an invalid endpoint, and the two different turn failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeError {
    EndNotOuter,
    InvalidEnd,
    InvalidTurnEnd,
    InvalidTurn,
}

/// `SlideShape.kind`.
pub fn shape_kind(slide_shape: SlideShape) -> SlideKind {
    match slide_shape.canonical {
        CanonicalSlideShape::Line(_) => SlideKind::Line,
        CanonicalSlideShape::Circle(_) => SlideKind::Circle,
        CanonicalSlideShape::V(_) => SlideKind::V,
        CanonicalSlideShape::Turn(_) => SlideKind::Turn,
        CanonicalSlideShape::Pq(_) => SlideKind::Pq,
        CanonicalSlideShape::Ppqq(_) => SlideKind::Ppqq,
        CanonicalSlideShape::S => SlideKind::S,
        CanonicalSlideShape::Wifi => SlideKind::Wifi,
    }
}

/// `SlideShape.relEnd`.
pub fn shape_rel_end(slide_shape: SlideShape) -> Option<usize> {
    match slide_shape.canonical {
        CanonicalSlideShape::Line(n)
        | CanonicalSlideShape::Circle(n)
        | CanonicalSlideShape::V(n)
        | CanonicalSlideShape::Turn(n)
        | CanonicalSlideShape::Pq(n)
        | CanonicalSlideShape::Ppqq(n) => Some(n),
        CanonicalSlideShape::S | CanonicalSlideShape::Wifi => None,
    }
}

/// `SlideShape.mirrored`.
pub fn shape_mirrored(slide_shape: SlideShape) -> bool {
    symmetry::is_mirrored(slide_shape.symmetry)
}

/// `mkCanonicalSlideShape?`.
#[allow(clippy::manual_range_contains)]
pub fn mk_canonical_slide_shape_opt(
    kind: SlideKind,
    rel_end: Option<usize>,
) -> Option<CanonicalSlideShape> {
    match (kind, rel_end) {
        (SlideKind::Line, Some(n)) => {
            if 2 <= n && n <= 8 {
                Some(CanonicalSlideShape::Line(n))
            } else {
                None
            }
        }
        (SlideKind::Circle, Some(n)) => Some(CanonicalSlideShape::Circle(n)),
        (SlideKind::V, Some(n)) => {
            if n != 5 {
                Some(CanonicalSlideShape::V(n))
            } else {
                None
            }
        }
        (SlideKind::Turn, Some(n)) => {
            if 2 <= n && n <= 8 {
                Some(CanonicalSlideShape::Turn(n))
            } else {
                None
            }
        }
        (SlideKind::Pq, Some(n)) => Some(CanonicalSlideShape::Pq(n)),
        (SlideKind::Ppqq, Some(n)) => Some(CanonicalSlideShape::Ppqq(n)),
        (SlideKind::S, None) => Some(CanonicalSlideShape::S),
        (SlideKind::Wifi, None) => Some(CanonicalSlideShape::Wifi),
        _ => None,
    }
}

fn mk_canonical_slide_shape(
    kind: SlideKind,
    rel_end: Option<usize>,
) -> Result<CanonicalSlideShape, ShapeError> {
    match mk_canonical_slide_shape_opt(kind, rel_end) {
        Some(shape) => Ok(shape),
        None => Err(ShapeError::InvalidEnd),
    }
}

/// `mkCanonicalSlideShapeUnchecked`.
pub fn mk_canonical_slide_shape_unchecked(
    kind: SlideKind,
    rel_end: Option<usize>,
) -> CanonicalSlideShape {
    match (kind, rel_end) {
        (SlideKind::Line, Some(n)) => CanonicalSlideShape::Line(n),
        (SlideKind::Circle, Some(n)) => CanonicalSlideShape::Circle(n),
        (SlideKind::V, Some(n)) => CanonicalSlideShape::V(n),
        (SlideKind::Turn, Some(n)) => CanonicalSlideShape::Turn(n),
        (SlideKind::Pq, Some(n)) => CanonicalSlideShape::Pq(n),
        (SlideKind::Ppqq, Some(n)) => CanonicalSlideShape::Ppqq(n),
        (SlideKind::S, _) => CanonicalSlideShape::S,
        (SlideKind::Wifi, _) => CanonicalSlideShape::Wifi,
        (SlideKind::Line, None) => CanonicalSlideShape::Line(3),
        (SlideKind::Circle, None) => CanonicalSlideShape::Circle(2),
        (SlideKind::V, None) => CanonicalSlideShape::V(1),
        (SlideKind::Turn, None) => CanonicalSlideShape::Turn(2),
        (SlideKind::Pq, None) => CanonicalSlideShape::Pq(1),
        (SlideKind::Ppqq, None) => CanonicalSlideShape::Ppqq(1),
    }
}

/// `mkSlideShape`.
pub fn mk_slide_shape(canonical: CanonicalSlideShape, sym: SlideSymmetry) -> SlideShape {
    SlideShape {
        canonical,
        symmetry: sym,
    }
}

/// `digitToNat?`.
pub fn digit_to_nat(c: char) -> Option<usize> {
    if c == '1' {
        Some(1)
    } else if c == '2' {
        Some(2)
    } else if c == '3' {
        Some(3)
    } else if c == '4' {
        Some(4)
    } else if c == '5' {
        Some(5)
    } else if c == '6' {
        Some(6)
    } else if c == '7' {
        Some(7)
    } else if c == '8' {
        Some(8)
    } else {
        None
    }
}

fn mirror_key(n: usize) -> usize {
    match n {
        1 => 1,
        2 => 8,
        3 => 7,
        4 => 6,
        5 => 5,
        6 => 4,
        7 => 3,
        8 => 2,
        n => n,
    }
}

/// `canonicalRelEnd`.
pub fn canonical_rel_end(sym: SlideSymmetry, rel_end: usize) -> usize {
    if symmetry::is_mirrored(sym) {
        mirror_key(rel_end)
    } else {
        rel_end
    }
}

/// `baseRelEnd`.
pub fn base_rel_end(sym: SlideSymmetry, rel_end: usize) -> usize {
    canonical_rel_end(sym, rel_end)
}

fn relative_end_pos(start_pos: usize, end_pos: usize) -> usize {
    (((end_pos - 1) + 8 - (start_pos - 1)) % 8) + 1
}

pub fn outer_slot_is_right_half(slot: OuterSlot) -> bool {
    slot.to_index() < 4
}

pub fn outer_slot_is_upper_half(slot: OuterSlot) -> bool {
    matches!(
        slot,
        OuterSlot::S7 | OuterSlot::S8 | OuterSlot::S1 | OuterSlot::S2
    )
}

pub fn relative_end_from_typed(
    start_lane: OuterSlot,
    end_area: SensorArea,
) -> Result<usize, ShapeError> {
    match end_area.to_outer_slot() {
        Some(end_zone) => Ok(relative_end_pos(
            start_lane.to_index() + 1,
            end_zone.to_index() + 1,
        )),
        None => Err(ShapeError::EndNotOuter),
    }
}

// Explicit propagation keeps the complete error path transparent to extraction.
macro_rules! shape_try {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
    };
}

/// `solveSlideShape`.
#[allow(clippy::manual_range_contains)]
pub fn solve_shape(
    kind: SlideBodyKind,
    start_lane: OuterSlot,
    end_area: Option<SensorArea>,
    turn_area: Option<SensorArea>,
) -> Result<SlideShape, ShapeError> {
    let end_area = end_area.unwrap_or(SensorArea::A1);
    let direct = symmetry::direct();
    let mirror = symmetry::mirror();
    match kind {
        SlideBodyKind::Line => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Line, Some(rel_end)));
            Ok(mk_slide_shape(canonical, direct))
        }
        SlideBodyKind::CircleRight => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Circle, Some(rel_end)));
            Ok(mk_slide_shape(
                canonical,
                if outer_slot_is_upper_half(start_lane) {
                    direct
                } else {
                    mirror
                },
            ))
        }
        SlideBodyKind::CircleLeft => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Circle, Some(rel_end)));
            Ok(mk_slide_shape(
                canonical,
                if !outer_slot_is_upper_half(start_lane) {
                    direct
                } else {
                    mirror
                },
            ))
        }
        SlideBodyKind::CircleUp => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Circle, Some(rel_end)));
            Ok(mk_slide_shape(
                canonical,
                if rel_end < 5 { direct } else { mirror },
            ))
        }
        SlideBodyKind::V => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::V, Some(rel_end)));
            Ok(mk_slide_shape(canonical, direct))
        }
        SlideBodyKind::Pp => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Ppqq, Some(rel_end)));
            Ok(mk_slide_shape(canonical, direct))
        }
        SlideBodyKind::Qq => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Ppqq, Some(rel_end)));
            Ok(mk_slide_shape(canonical, mirror))
        }
        SlideBodyKind::P => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Pq, Some(rel_end)));
            Ok(mk_slide_shape(canonical, direct))
        }
        SlideBodyKind::Q => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Pq, Some(rel_end)));
            Ok(mk_slide_shape(canonical, mirror))
        }
        SlideBodyKind::S | SlideBodyKind::Z => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            if rel_end != 5 {
                Err(ShapeError::InvalidEnd)
            } else {
                let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::S, None));
                let sym = if kind == SlideBodyKind::Z {
                    mirror
                } else {
                    direct
                };
                Ok(mk_slide_shape(canonical, sym))
            }
        }
        SlideBodyKind::Turn => {
            let turn_area = turn_area.unwrap_or(SensorArea::A1);
            let turn_rel = shape_try!(relative_end_from_typed(start_lane, turn_area));
            let end_rel = shape_try!(relative_end_from_typed(start_lane, end_area));
            if turn_rel == 7 {
                if end_rel < 2 || end_rel > 5 {
                    Err(ShapeError::InvalidTurnEnd)
                } else {
                    let canonical =
                        shape_try!(mk_canonical_slide_shape(SlideKind::Turn, Some(end_rel)));
                    Ok(mk_slide_shape(canonical, direct))
                }
            } else if turn_rel == 3 {
                if end_rel < 5 {
                    Err(ShapeError::InvalidTurnEnd)
                } else {
                    let canonical =
                        shape_try!(mk_canonical_slide_shape(SlideKind::Turn, Some(end_rel)));
                    Ok(mk_slide_shape(canonical, mirror))
                }
            } else {
                Err(ShapeError::InvalidTurn)
            }
        }
        SlideBodyKind::Wifi => {
            let rel_end = shape_try!(relative_end_from_typed(start_lane, end_area));
            if rel_end != 5 {
                Err(ShapeError::InvalidEnd)
            } else {
                let canonical = shape_try!(mk_canonical_slide_shape(SlideKind::Wifi, None));
                Ok(mk_slide_shape(canonical, direct))
            }
        }
    }
}
