//! Connected-slide metadata control flow.
//!
//! The normalized runtime slide also carries exact time values and parser
//! data.  This kernel keeps the source-group decisions independent from those
//! adapters: all identifiers and indices use the same canonical BigNat
//! representation as the runtime Natural type.

use crate::big_nat::{self, BigNat};
use crate::types::SlideKind;

/// Metadata copied to a slide when the source-group chain is valid.
pub struct AppliedMetadata {
    pub slide_kind: SlideKind,
    pub is_conn_slide: bool,
    pub parent_note_index: Option<BigNat>,
    pub is_group_head: bool,
    pub is_group_end: bool,
    pub next_group_id: Option<BigNat>,
    pub next_parent_note_index: Option<BigNat>,
}

/// Result of one applyConnectedSlideMetadata step.
pub enum MetadataStep {
    /// The token has no complete source-group metadata.  The caller must
    /// clear the chain and reset the four derived fields on the slide.
    Clear,
    /// The token has a complete source group, but it is a non-head item whose
    /// chain does not match the preceding group.  The original slide is kept
    /// and the caller must clear the chain.
    Reset,
    /// The source-group metadata can be applied.  The returned next fields are
    /// exactly the state consumed by the next list element.
    Apply(AppliedMetadata),
}

fn option_equal(left: &Option<BigNat>, right: &Option<BigNat>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => big_nat::equal(left, right),
        _ => false,
    }
}

/// Apply the source-group metadata rules for one normalized slide.
///
/// The arithmetic is deliberately expressed through big_nat: source group
/// IDs, indices, sizes and note indices are unbounded in the Lean model and
/// the runtime Natural type.  No timing or parser operation is hidden here.
pub fn apply(
    source_group_id: Option<BigNat>,
    source_group_index: Option<BigNat>,
    source_group_size: Option<BigNat>,
    original_slide_kind: SlideKind,
    current_group_id: Option<BigNat>,
    parent_note_index: Option<BigNat>,
    note_index: BigNat,
) -> MetadataStep {
    let (group_id, index, size) = match (source_group_id, source_group_index, source_group_size) {
        (Some(group_id), Some(index), Some(size)) => (group_id, index, size),
        _ => return MetadataStep::Clear,
    };

    let zero = big_nat::zero();
    let one = big_nat::one();
    let is_head = big_nat::equal(&index, &zero);
    let is_conn = big_nat::compare(&size, &one) > 0;
    let is_end = big_nat::equal(&big_nat::add_u64(&index, 1), &size);
    let actual_parent = if is_head { None } else { parent_note_index };
    let slide_kind = if !is_conn {
        original_slide_kind
    } else if original_slide_kind == SlideKind::Wifi {
        SlideKind::Wifi
    } else {
        SlideKind::ConnPart
    };

    if !is_head && !option_equal(&current_group_id, &Some(group_id.clone())) {
        return MetadataStep::Reset;
    }

    MetadataStep::Apply(AppliedMetadata {
        slide_kind,
        is_conn_slide: is_conn,
        parent_note_index: actual_parent,
        is_group_head: is_head,
        is_group_end: is_end,
        next_group_id: if is_end { None } else { Some(group_id) },
        next_parent_note_index: if is_end { None } else { Some(note_index) },
    })
}
