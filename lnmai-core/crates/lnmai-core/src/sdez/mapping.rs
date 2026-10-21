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
pub fn slide_type_for_shape(shape: &SlideShape, _slot: OuterSlot) -> u8 {
    let mirrored = is_mirrored(shape.symmetry);
    match &shape.canonical {
        CanonicalSlideShape::Line(_) => SLIDE_STRAIGHT,
        CanonicalSlideShape::Circle(_) => {
            // `<` (unmirrored) walks the A/B ring upward; `>` is its mirror.
            // The official Circle_R table is the increasing path, so the
            // direction follows the shape mirror bit, not the start quadrant.
            if !mirrored {
                SLIDE_CIRCLE_R
            } else {
                SLIDE_CIRCLE_L
            }
        }
        CanonicalSlideShape::V(_) => SLIDE_CORNER,
        CanonicalSlideShape::Turn(_) => {
            // Simai V/turn notation walks the same seven-area ring path as the
            // official Skip tables; Bend's hit sequence diverges at the second
            // area (C2/B6 instead of C/B7).
            if mirrored {
                SLIDE_SKIP_R
            } else {
                SLIDE_SKIP_L
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
    // `rel_end == 8` is a full loop only for Circle. For P/Q, V and Turn it
    // means seven steps forward (the end is one step before the start), so the
    // normal relative-end formula is correct there.
    if rel_end == 8 && matches!(shape.canonical, CanonicalSlideShape::Circle(_)) {
        return slot.to_index() as u8;
    }
    ((slot.to_index() + rel_end - 1) % 8) as u8
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
        assert_eq!(slide_end_for_shape(&shape, OuterSlot::S1), 1);
    }

    #[test]
    fn wifi_maps_to_fan_and_ends_five_steps_away() {
        let shape = SlideShape {
            canonical: CanonicalSlideShape::Wifi,
            symmetry: direct(),
        };
        assert_eq!(slide_type_for_shape(&shape, OuterSlot::S1), SLIDE_FAN);
        assert_eq!(slide_end_for_shape(&shape, OuterSlot::S1), 4);
    }

    #[test]
    fn lowered_chart_carries_the_sdez_fields() {
        let chart = crate::simai::frontend::frontend_lowered_chart(
            "&inote_1=(120){4}1-3[4:1],E",
            (1).into(),
        )
        .expect("chart");
        let slide = chart.slides.first().expect("slide");
        assert_eq!(slide.sdez_slide_type, SLIDE_STRAIGHT);
        assert_eq!(slide.sdez_end, 2);
    }

    #[test]
    fn terminal_slot_comes_from_the_written_end_position() {
        for (body, slide_type, end) in [
            ("3<6", 3, 5),
            ("6>3", 2, 2),
            ("1>8", 3, 7),
            ("8<1", 2, 0),
            ("5p4", 4, 3),
            ("1p8", 4, 7),
            ("1V37", 12, 6),
            ("8V62", 11, 1),
            ("1w5", 13, 4),
        ] {
            let text = format!("&inote_1=(120){{4}}{body}[4:1],E");
            let chart =
                crate::simai::frontend::frontend_lowered_chart(&text, (1).into()).unwrap();
            let slide = chart.slides.first().expect(body);
            assert_eq!(slide.sdez_slide_type, slide_type, "{body}");
            assert_eq!(slide.sdez_end, end, "{body}");
        }
    }

}
