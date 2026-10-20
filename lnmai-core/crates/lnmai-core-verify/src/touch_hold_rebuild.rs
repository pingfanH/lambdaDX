//! Shared reconstruction of TouchHold body groups for scheduler and ChartLoader.
//! No lifecycle filter: every source hold with a body group participates.
//! The adapter exposes fields and indexed container operations only. All scans,
//! duplicate handling, insertion/removal decisions, source traversal, and the
//! final group-order reversal live here. Every loop uses the original length of
//! its scanned list; no usize len + 1 and no recursive Rust traversal.

/// Point laws required by the extracted proof:
/// - source reads preserve the full immutable source, including duplicate notes;
/// - matches compares exact IDs, without converting them to machine indices;
/// - prepend_* inserts at zero and changes only that one group's named list;
/// - remove_trigger removes exactly the specified slot (not all equal IDs);
/// - append_empty appends one group with the supplied ID and two empty lists;
/// - swap_groups exchanges exactly two outer-list slots, preserving both groups;
/// - every other group/field and all unaffected lengths are unchanged.
///   Indexed reads/mutations require in-bounds indices; insert/append also need
///   successful allocation and representable resulting lengths.
pub trait TouchHoldRebuild<S, B, Id> {
    fn hold_length(&self, holds: &S) -> usize;
    fn hold_group_id(&self, holds: &S, index: usize) -> Option<Id>;
    fn hold_note_index(&self, holds: &S, index: usize) -> Id;
    fn hold_triggered(&self, holds: &S, index: usize) -> bool;

    fn empty(&self) -> B;
    fn group_length(&self, groups: &B) -> usize;
    fn group_matches(&self, groups: &B, index: usize, id: &Id) -> bool;
    fn member_length(&self, groups: &B, group: usize) -> usize;
    fn member_matches(&self, groups: &B, group: usize, index: usize, note: &Id) -> bool;
    fn trigger_length(&self, groups: &B, group: usize) -> usize;
    fn trigger_matches(&self, groups: &B, group: usize, index: usize, note: &Id) -> bool;

    fn prepend_member(&self, groups: B, group: usize, note: &Id) -> B;
    fn prepend_trigger(&self, groups: B, group: usize, note: &Id) -> B;
    fn remove_trigger(&self, groups: B, group: usize, index: usize) -> B;
    fn append_empty(&self, groups: B, id: &Id) -> B;
    fn swap_groups(&self, groups: B, left: usize, right: usize) -> B;
}

/// Return the FIRST matching group. Later duplicates are neither read nor edited.
pub fn find_group<S, B, Id, Ops>(groups: &B, id: &Id, ops: &Ops) -> Option<usize>
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    let len = ops.group_length(groups);
    let mut index = 0;
    while index < len {
        if ops.group_matches(groups, index, id) {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub fn find_member<S, B, Id, Ops>(groups: &B, group: usize, note: &Id, ops: &Ops) -> Option<usize>
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    let len = ops.member_length(groups, group);
    let mut index = 0;
    while index < len {
        if ops.member_matches(groups, group, index, note) {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub fn find_trigger<S, B, Id, Ops>(groups: &B, group: usize, note: &Id, ops: &Ops) -> Option<usize>
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    let len = ops.trigger_length(groups, group);
    let mut index = 0;
    while index < len {
        if ops.trigger_matches(groups, group, index, note) {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// Update one source note. In particular, false still inserts a missing member,
/// and removes at most ONE trigger. No sets, deduplication, or length clamping.
pub fn upsert<S, B, Id, Ops>(mut groups: B, id: &Id, note: &Id, triggered: bool, ops: &Ops) -> B
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    match find_group::<S, B, Id, Ops>(&groups, id, ops) {
        Some(group) => {
            if find_member::<S, B, Id, Ops>(&groups, group, note, ops).is_none() {
                groups = ops.prepend_member(groups, group, note);
            }
            // A member write preserves the original trigger list and its length.
            let trigger = find_trigger::<S, B, Id, Ops>(&groups, group, note, ops);
            if triggered {
                if trigger.is_none() {
                    groups = ops.prepend_trigger(groups, group, note);
                }
            } else if let Some(index) = trigger {
                groups = ops.remove_trigger(groups, group, index);
            }
            groups
        }
        None => {
            let group = ops.group_length(&groups);
            groups = ops.append_empty(groups, id);
            groups = ops.prepend_member(groups, group, note);
            if triggered {
                groups = ops.prepend_trigger(groups, group, note);
            }
            groups
        }
    }
}

/// Reverse outer group order ONLY. The loop performs floor(len / 2) point swaps;
/// member and trigger order inside each group remains unchanged.
pub fn reverse_groups<S, B, Id, Ops>(mut groups: B, ops: &Ops) -> B
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    let len = ops.group_length(&groups);
    let half = len / 2;
    let mut left = 0;
    while left < half {
        // Entering the loop implies len >= 2 and left < len.
        let right = len - 1 - left;
        groups = ops.swap_groups(groups, left, right);
        left += 1;
    }
    groups
}

/// Generalized accumulator form, corresponding to the reference private loop.
/// The groups argument is in its pre-reversal order. Duplicate group IDs and
/// duplicate member/trigger entries in an accumulator are intentionally allowed.
pub fn rebuild_from<S, B, Id, Ops>(holds: &S, mut groups: B, ops: &Ops) -> B
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    let len = ops.hold_length(holds);
    let mut index = 0;
    while index < len {
        if let Some(id) = ops.hold_group_id(holds, index) {
            let note = ops.hold_note_index(holds, index);
            let triggered = ops.hold_triggered(holds, index);
            groups = upsert::<S, B, Id, Ops>(groups, &id, &note, triggered, ops);
        }
        index += 1;
    }
    reverse_groups::<S, B, Id, Ops>(groups, ops)
}

/// Both concrete runtime helpers call this function using the same point adapter.
pub fn rebuild<S, B, Id, Ops>(holds: &S, ops: Ops) -> B
where
    Ops: TouchHoldRebuild<S, B, Id>,
{
    let groups = ops.empty();
    rebuild_from::<S, B, Id, Ops>(holds, groups, &ops)
}
