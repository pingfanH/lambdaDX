//! Concrete TouchHold body-group storage and field operations.
//!
//! IDs remain generic: PartialEq, Clone and Default are primitive dictionaries.
//! Only list positions and lengths use usize. Source HoldNote reads and the
//! existing reconstruction/pass control flow remain outside this module.
//! All scans stop at the first equal ID, including on inputs with duplicates.

#![allow(clippy::len_zero)]

use crate::{vec_erase, vec_prepend};

/// Same public fields and derives as the runtime record. The runtime aliases
/// this type at its arbitrary-precision Natural; JSON implementations stay there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchHoldBodyGroupState<Id> {
    pub group_id: Id,
    pub member_note_indices: Vec<Id>,
    pub triggered_note_indices: Vec<Id>,
}

impl<Id: Default> Default for TouchHoldBodyGroupState<Id> {
    fn default() -> Self {
        Self {
            group_id: Id::default(),
            member_note_indices: Vec::new(),
            triggered_note_indices: Vec::new(),
        }
    }
}

pub fn empty<Id>() -> Vec<TouchHoldBodyGroupState<Id>> {
    Vec::new()
}

pub fn group_length<Id>(groups: &[TouchHoldBodyGroupState<Id>]) -> usize {
    groups.len()
}

pub fn group_matches<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    index: usize,
    id: &Id,
) -> bool {
    &groups[index].group_id == id
}

pub fn member_length<Id>(groups: &[TouchHoldBodyGroupState<Id>], group: usize) -> usize {
    groups[group].member_note_indices.len()
}

pub fn member_matches<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    group: usize,
    index: usize,
    note: &Id,
) -> bool {
    &groups[group].member_note_indices[index] == note
}

pub fn trigger_length<Id>(groups: &[TouchHoldBodyGroupState<Id>], group: usize) -> usize {
    groups[group].triggered_note_indices.len()
}

pub fn trigger_matches<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    group: usize,
    index: usize,
    note: &Id,
) -> bool {
    &groups[group].triggered_note_indices[index] == note
}

/// Concrete first-match outer scan; a later duplicate group is never selected.
pub fn find_group<Id: PartialEq>(groups: &[TouchHoldBodyGroupState<Id>], id: &Id) -> Option<usize> {
    let len = group_length(groups);
    let mut index = 0;
    while index < len {
        if group_matches(groups, index, id) {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// Shared first-match scan for member and trigger lists. No ID conversion,
/// clone, allocation, uniqueness assumption, or source adapter is involved.
pub fn find_id<Id: PartialEq>(values: &[Id], note: &Id) -> Option<usize> {
    let len = values.len();
    let mut index = 0;
    while index < len {
        if &values[index] == note {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub fn find_member<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    group: usize,
    note: &Id,
) -> Option<usize> {
    find_id(&groups[group].member_note_indices, note)
}

pub fn find_trigger<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    group: usize,
    note: &Id,
) -> Option<usize> {
    find_id(&groups[group].triggered_note_indices, note)
}

pub fn has_member<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    group: usize,
    note: &Id,
) -> bool {
    find_member(groups, group, note).is_some()
}

pub fn has_trigger<Id: PartialEq>(
    groups: &[TouchHoldBodyGroupState<Id>],
    group: usize,
    note: &Id,
) -> bool {
    find_trigger(groups, group, note).is_some()
}

pub fn members_empty<Id>(groups: &[TouchHoldBodyGroupState<Id>], group: usize) -> bool {
    groups[group].member_note_indices.len() == 0
}

/// Clone only the incoming note once; push/swap moves all existing elements.
pub fn prepend_member<Id: Clone>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    group: usize,
    note: &Id,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    vec_prepend::prepend(&mut groups[group].member_note_indices, note.clone());
    groups
}

pub fn prepend_trigger<Id: Clone>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    group: usize,
    note: &Id,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    vec_prepend::prepend(&mut groups[group].triggered_note_indices, note.clone());
    groups
}

/// Indexed erasure. The shrinking resize branch never clones its filler or
/// any old value; Clone is only required by the Rust resize signature.
pub fn remove_member<Id: Clone + Default>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    group: usize,
    index: usize,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    vec_erase::erase(&mut groups[group].member_note_indices, index, Id::default());
    groups
}

pub fn remove_trigger<Id: Clone + Default>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    group: usize,
    index: usize,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    vec_erase::erase(
        &mut groups[group].triggered_note_indices,
        index,
        Id::default(),
    );
    groups
}

/// Erase only the first matching member; a miss does not construct a filler.
pub fn erase_member<Id: PartialEq + Clone + Default>(
    groups: Vec<TouchHoldBodyGroupState<Id>>,
    group: usize,
    note: &Id,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    match find_member(&groups, group, note) {
        Some(index) => remove_member(groups, group, index),
        None => groups,
    }
}

pub fn erase_trigger<Id: PartialEq + Clone + Default>(
    groups: Vec<TouchHoldBodyGroupState<Id>>,
    group: usize,
    note: &Id,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    match find_trigger(&groups, group, note) {
        Some(index) => remove_trigger(groups, group, index),
        None => groups,
    }
}

/// Append exactly one group; clone the supplied group ID once.
pub fn append_empty<Id: Clone>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    id: &Id,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    groups.push(TouchHoldBodyGroupState {
        group_id: id.clone(),
        member_note_indices: Vec::new(),
        triggered_note_indices: Vec::new(),
    });
    groups
}

/// Same clone order/count as the runtime literal: id, member note, trigger note.
/// Each new singleton has its own one-element buffer; no old tail is copied.
#[allow(clippy::vec_init_then_push)]
pub fn append<Id: Clone>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    id: &Id,
    note: &Id,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    let group_id = id.clone();
    let mut member_note_indices = Vec::with_capacity(1);
    member_note_indices.push(note.clone());
    let mut triggered_note_indices = Vec::with_capacity(1);
    triggered_note_indices.push(note.clone());
    groups.push(TouchHoldBodyGroupState {
        group_id,
        member_note_indices,
        triggered_note_indices,
    });
    groups
}

/// Delete exactly one outer slot in order. The default record is a discarded
/// shrink filler, never inserted or cloned; retained groups are moved by swaps.
pub fn remove_group<Id: Clone + Default>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    index: usize,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    vec_erase::erase(&mut groups, index, TouchHoldBodyGroupState::default());
    groups
}

pub fn swap_groups<Id>(
    mut groups: Vec<TouchHoldBodyGroupState<Id>>,
    left: usize,
    right: usize,
) -> Vec<TouchHoldBodyGroupState<Id>> {
    groups.swap(left, right);
    groups
}
