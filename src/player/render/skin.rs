//! Central note-skin configuration.
//!
//! Every renderer asks this module which texture to use, so the mapping from a
//! note to its skin lives in **one** place instead of scattered `if is_break /
//! is_each / ...` chains:
//!
//! * [`SkinVariant`] — the body variant. Precedence is `Break > Each > Normal`
//!   (`is_each` is set by `maichart::recompute_each` when notes share a hit
//!   time).
//! * [`SkinKind`] — which body part (tap, hold, touch arm/dot, slide trail,
//!   star head, double star head).
//! * [`SKIN_TABLE`] / [`EX_TABLE`] — the constant tables.
//!
//! Add a skin by adding a row; renderers never need to change.

use macroquad::prelude::Texture2D;

use crate::app::types::Note;
use crate::player::state::PadPreviewState;

/// Body variant of a note (mutually exclusive, in precedence order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinVariant {
    Normal,
    Each,
    Break,
}

impl SkinVariant {
    /// Variant from raw flags (break wins over each).
    pub fn of_flags(is_break: bool, is_each: bool) -> Self {
        if is_break {
            SkinVariant::Break
        } else if is_each {
            SkinVariant::Each
        } else {
            SkinVariant::Normal
        }
    }

    pub fn of(note: &Note) -> Self {
        Self::of_flags(note.is_break, note.is_each)
    }
}

/// Which texture of a note is being drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinKind {
    Tap,
    Hold,
    TouchTri,
    TouchPoint,
    SlideTrail,
    Star,
    StarDouble,
}

/// A texture accessor on [`PadPreviewState`].
pub type TexRef = fn(&PadPreviewState) -> Option<&Texture2D>;

/// `(kind, variant) -> texture`. The constant **is** the configuration.
#[rustfmt::skip]
const SKIN_TABLE: &[(SkinKind, SkinVariant, TexRef)] = &[
    (SkinKind::Tap,        SkinVariant::Normal, |a| a.tap_texture.as_ref()),
    (SkinKind::Tap,        SkinVariant::Each,   |a| a.tap_each_tex.as_ref()),
    (SkinKind::Tap,        SkinVariant::Break,  |a| a.tap_break_tex.as_ref()),

    (SkinKind::Hold,       SkinVariant::Normal, |a| a.hold_texture.as_ref()),
    (SkinKind::Hold,       SkinVariant::Each,   |a| a.hold_each_tex.as_ref()),
    (SkinKind::Hold,       SkinVariant::Break,  |a| a.hold_break_tex.as_ref()),

    (SkinKind::TouchTri,   SkinVariant::Normal, |a| a.touch_tri_tex.as_ref()),
    (SkinKind::TouchTri,   SkinVariant::Each,   |a| a.touch_tri_each_tex.as_ref()),

    (SkinKind::TouchPoint, SkinVariant::Normal, |a| a.touch_point_tex.as_ref()),
    (SkinKind::TouchPoint, SkinVariant::Each,   |a| a.touch_point_each_tex.as_ref()),

    (SkinKind::SlideTrail, SkinVariant::Normal, |a| a.slide_tex.as_ref()),
    (SkinKind::SlideTrail, SkinVariant::Each,   |a| a.slide_each_tex.as_ref()),
    (SkinKind::SlideTrail, SkinVariant::Break,  |a| a.slide_break_tex.as_ref()),

    (SkinKind::Star,       SkinVariant::Normal, |a| a.star_tex.as_ref()),
    (SkinKind::Star,       SkinVariant::Each,   |a| a.star_each_tex.as_ref()),
    (SkinKind::Star,       SkinVariant::Break,  |a| a.star_break_tex.as_ref()),

    (SkinKind::StarDouble, SkinVariant::Normal, |a| a.star_double_tex.as_ref()),
    (SkinKind::StarDouble, SkinVariant::Each,   |a| a.star_double_each_tex.as_ref()),
    (SkinKind::StarDouble, SkinVariant::Break,  |a| a.star_double_break_tex.as_ref()),
];

/// `kind -> Ex overlay` (drawn on top of the body).
#[rustfmt::skip]
const EX_TABLE: &[(SkinKind, TexRef)] = &[
    (SkinKind::Tap,        |a| a.tap_ex_tex.as_ref()),
    (SkinKind::Hold,       |a| a.hold_ex_tex.as_ref()),
    (SkinKind::Star,       |a| a.star_ex_tex.as_ref()),
    (SkinKind::StarDouble, |a| a.star_double_ex_tex.as_ref()),
];

/// Texture for exactly this kind/variant, if present.
pub fn body(app: &PadPreviewState, kind: SkinKind, variant: SkinVariant) -> Option<&Texture2D> {
    SKIN_TABLE
        .iter()
        .find(|(k, v, _)| *k == kind && *v == variant)
        .and_then(|(_, _, tex)| tex(app))
}

/// Texture for this kind/variant, falling back to the kind's `Normal` skin
/// (e.g. a missing `*_each`/`*_break` asset still renders).
pub fn body_or_normal(
    app: &PadPreviewState,
    kind: SkinKind,
    variant: SkinVariant,
) -> Option<&Texture2D> {
    body(app, kind, variant).or_else(|| body(app, kind, SkinVariant::Normal))
}

/// Ex overlay texture for a kind, if present.
pub fn ex(app: &PadPreviewState, kind: SkinKind) -> Option<&Texture2D> {
    EX_TABLE
        .iter()
        .find(|(k, _)| *k == kind)
        .and_then(|(_, tex)| tex(app))
}

/// The slide trail / star kind for a note (double-star heads get their own set).
pub fn star_kind(note: &Note) -> SkinKind {
    if note.is_star {
        SkinKind::StarDouble
    } else {
        SkinKind::Star
    }
}

/// Star body: variant texture, falling back to that kind's `Normal`.
pub fn star_body<'a>(app: &'a PadPreviewState, note: &Note) -> Option<&'a Texture2D> {
    let kind = star_kind(note);
    body_or_normal(app, kind, SkinVariant::of(note))
}

/// Star Ex overlay, falling back to the single-star Ex.
pub fn star_ex<'a>(app: &'a PadPreviewState, note: &Note) -> Option<&'a Texture2D> {
    if !note.is_ex {
        return None;
    }
    let kind = star_kind(note);
    ex(app, kind).or_else(|| ex(app, SkinKind::Star))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [SkinKind; 7] = [
        SkinKind::Tap,
        SkinKind::Hold,
        SkinKind::TouchTri,
        SkinKind::TouchPoint,
        SkinKind::SlideTrail,
        SkinKind::Star,
        SkinKind::StarDouble,
    ];

    #[test]
    fn every_kind_has_a_normal_skin() {
        for kind in KINDS {
            assert!(
                SKIN_TABLE
                    .iter()
                    .any(|(k, v, _)| *k == kind && *v == SkinVariant::Normal),
                "{kind:?} is missing its Normal skin"
            );
        }
    }

    #[test]
    fn break_beats_each_beats_normal() {
        assert_eq!(SkinVariant::of_flags(true, true), SkinVariant::Break);
        assert_eq!(SkinVariant::of_flags(false, true), SkinVariant::Each);
        assert_eq!(SkinVariant::of_flags(false, false), SkinVariant::Normal);
    }
}
