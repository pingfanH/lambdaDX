//! Simai shape to SDEZ170 SlideType mapping.
//!
//! JudgementCode owns the geometry tables; Simai owns the notation. This
//! mapping is the documented bridge between them.

use crate::areas::OuterSlot;
use crate::simai::shape::shape_rel_end;
use crate::simai::syntax::{CanonicalSlideShape, SlideShape};
use crate::symmetry::is_mirrored;

pub const SLIDE_STRAIGHT: u8 = 1;
pub const SLIDE_CIRCLE_L: u8 = 2;
pub const SLIDE_CIRCLE_R: u8 = 3;
pub const SLIDE_CURVE_L: u8 = 4;
pub const SLIDE_CURVE_R: u8 = 5;
pub const SLIDE_THUNDER_L: u8 = 6;
pub const SLIDE_THUNDER_R: u8 = 7;
pub const SLIDE_CORNER: u8 = 8;
pub const SLIDE_BEND_L: u8 = 9;
pub const SLIDE_BEND_R: u8 = 10;
pub const SLIDE_SKIP_L: u8 = 11;
pub const SLIDE_SKIP_R: u8 = 12;
pub const SLIDE_FAN: u8 = 13;

/// Map the canonical Simai shape to the official SlideType index.
///
/// Directional variants use the parser's mirror bit. CircleUp is classified by
/// its canonical mirror state because the original body kind is not retained.
pub fn slide_type_for_shape(shape: &SlideShape, slot: OuterSlot) -> u8 {
    let mirrored = is_mirrored(shape.symmetry);
    match &shape.canonical {
        CanonicalSlideShape::Line(_) => SLIDE_STRAIGHT,
        CanonicalSlideShape::Circle(_) => {
            let upper = matches!(
                slot,
                OuterSlot::S7 | OuterSlot::S8 | OuterSlot::S1 | OuterSlot::S2
            );
            let right = if upper { !mirrored } else { mirrored };
            if right {
                SLIDE_CIRCLE_R
            } else {
                SLIDE_CIRCLE_L
            }
        }
        CanonicalSlideShape::V(_) => SLIDE_CORNER,
        CanonicalSlideShape::Turn(_) => {
            if mirrored {
                SLIDE_BEND_R
            } else {
                SLIDE_BEND_L
            }
        }
        CanonicalSlideShape::Pq(_) => {
            if mirrored {
                SLIDE_CURVE_R
            } else {
                SLIDE_CURVE_L
            }
        }
        CanonicalSlideShape::Ppqq(_) => {
            if mirrored {
                SLIDE_SKIP_R
            } else {
                SLIDE_SKIP_L
            }
        }
        CanonicalSlideShape::S => {
            if mirrored {
                SLIDE_THUNDER_R
            } else {
                SLIDE_THUNDER_L
            }
        }
        CanonicalSlideShape::Wifi => SLIDE_FAN,
    }
}

/// Relative end position used by GetSlideHitArea. S/Wifi always end five
/// positions from the start; other shapes carry their canonical relative end.
pub fn slide_end_for_shape(shape: &SlideShape, slot: OuterSlot) -> u8 {
    let rel_end = shape_rel_end(shape)
        .and_then(|value| value.to_usize())
        .unwrap_or(5);
    ((slot.to_index() + rel_end) % 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simai::syntax::SlideShape;
    use crate::symmetry::direct;

    #[test]
    fn line_maps_to_straight_and_keeps_the_end() {
        let shape = SlideShape {
            canonical: CanonicalSlideShape::Line((2).into()),
            symmetry: direct(),
        };
        assert_eq!(slide_type_for_shape(&shape, OuterSlot::S1), SLIDE_STRAIGHT);
        assert_eq!(slide_end_for_shape(&shape, OuterSlot::S1), 2);
    }

    #[test]
    fn wifi_maps_to_fan_and_ends_five_steps_away() {
        let shape = SlideShape {
            canonical: CanonicalSlideShape::Wifi,
            symmetry: direct(),
        };
        assert_eq!(slide_type_for_shape(&shape, OuterSlot::S1), SLIDE_FAN);
        assert_eq!(slide_end_for_shape(&shape, OuterSlot::S1), 5);
    }
}
