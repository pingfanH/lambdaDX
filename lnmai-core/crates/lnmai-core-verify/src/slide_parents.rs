//! Connected-slide parent propagation shared with the runtime scheduler.
//!
//! Identifiers are opaque: equality and cloning their runtime representation
//! belong to the point-read adapter. Searches select the first matching slot,
//! whereas edits visit every matching slot, including duplicate identifiers.
//! The adapters read/write one slot or append one command; all scans, guards,
//! ancestor fuel, and command sequencing live here.

pub struct ParentInfo<I> {
    pub note_index: I,
    pub parent_index: Option<I>,
    pub zero_remaining: bool,
    pub one_remaining: bool,
    pub connected: bool,
    pub final_part: bool,
    pub ended: bool,
}

#[derive(Clone, Copy)]
pub enum ParentEdit {
    Flags { finished: bool, pending: bool },
    ClearQueues,
    End,
}

/// Named traits keep the borrowed point operations explicit without a
/// higher-ranked closure bound, which the extraction backend cannot translate.
pub trait ParentRead<S, I> {
    fn read(&self, state: &S, index: usize) -> ParentInfo<I>;
}

pub trait ParentEqual<I> {
    fn equal(&self, left: &I, right: &I) -> bool;
}

/// The reader returns the metadata of exactly one in-bounds slot. Identifier
/// equality is supplied separately, without conversion to a machine integer.
pub fn find_first<S, I, Read, Equal>(
    len: usize,
    state: &S,
    target: &I,
    read: &Read,
    equal: &Equal,
) -> Option<ParentInfo<I>>
where
    Read: ParentRead<S, I>,
    Equal: ParentEqual<I>,
{
    let mut index = 0;
    while index < len {
        let parent = read.read(state, index);
        if equal.equal(&parent.note_index, target) {
            return Some(parent);
        }
        index += 1;
    }
    None
}

/// Write every matching slot. A writer must preserve length, identifiers and
/// all fields outside the requested edit, including every other slot.
pub fn update_all<S, I, Read, Equal, Write>(
    len: usize,
    mut state: S,
    target: &I,
    edit: ParentEdit,
    read: &Read,
    equal: &Equal,
    write: &Write,
) -> S
where
    Read: ParentRead<S, I>,
    Equal: ParentEqual<I>,
    Write: Fn(S, usize, ParentEdit) -> S,
{
    let mut index = 0;
    while index < len {
        let parent = read.read(&state, index);
        if equal.equal(&parent.note_index, target) {
            state = write(state, index, edit);
        }
        index += 1;
    }
    state
}

/// Read flags from an immutable snapshot, exactly like the reference status
/// list. Writes do not feed back into the lookup for a later child.
pub fn refresh_flags<R, S, I, Read, Equal, Write>(
    len: usize,
    snapshot: &R,
    mut state: S,
    read: Read,
    equal: Equal,
    write: Write,
) -> S
where
    Read: ParentRead<R, I>,
    Equal: ParentEqual<I>,
    Write: Fn(S, usize, ParentEdit) -> S,
{
    let mut index = 0;
    while index < len {
        let child = read.read(snapshot, index);
        let parent = match child.parent_index {
            None => None,
            Some(parent_index) => find_first(len, snapshot, &parent_index, &read, &equal),
        };
        let (finished, pending) = match parent {
            None => (false, false),
            Some(parent) => (parent.zero_remaining, parent.one_remaining),
        };
        state = write(state, index, ParentEdit::Flags { finished, pending });
        index += 1;
    }
    state
}

#[derive(Clone, Copy)]
pub enum SensorPhase {
    Waiting,
    Active,
    Other,
}

/// The consumed comparison uses the runtime arbitrary-precision sum of queue
/// lengths. Only a queue head can supply historical On evidence.
pub fn sensor_triggered<Head>(
    before: SensorPhase,
    after: SensorPhase,
    checkable: bool,
    consumed: bool,
    queue_count: usize,
    head_on: Head,
) -> bool
where
    Head: Fn(usize) -> bool,
{
    match (before, after) {
        (SensorPhase::Waiting | SensorPhase::Active, SensorPhase::Active) => {}
        _ => return false,
    }
    if !checkable {
        return false;
    }
    if consumed {
        return true;
    }
    sensor_heads(queue_count, head_on)
}

/// Search queue heads after the phase/checkability/consumption guards.
pub fn sensor_heads<Head>(queue_count: usize, head_on: Head) -> bool
where
    Head: Fn(usize) -> bool,
{
    let mut index = 0;
    while index < queue_count {
        if head_on(index) {
            return true;
        }
        index += 1;
    }
    false
}

/// Clear all copies of the direct parent, but emit once using the first
/// parent. Neither lifecycle states nor earlier ancestors are changed.
#[allow(clippy::too_many_arguments)]
pub fn force_finish<S, C, I, Read, Equal, Write, Emit>(
    len: usize,
    state: S,
    commands: C,
    triggered: bool,
    parent_index: Option<I>,
    read: Read,
    equal: Equal,
    write: Write,
    emit: Emit,
) -> (S, C)
where
    Read: ParentRead<S, I>,
    Equal: ParentEqual<I>,
    Write: Fn(S, usize, ParentEdit) -> S,
    Emit: Fn(C, I) -> C,
{
    if !triggered {
        return (state, commands);
    }
    let Some(target) = parent_index else {
        return (state, commands);
    };
    let Some(parent) = find_first(len, &state, &target, &read, &equal) else {
        return (state, commands);
    };
    if !parent.connected || parent.final_part || parent.zero_remaining {
        return (state, commands);
    }
    let state = update_all(
        len,
        state,
        &target,
        ParentEdit::ClearQueues,
        &read,
        &equal,
        &write,
    );
    (state, emit(commands, parent.note_index))
}

/// Each successful iteration ends all same-ID slots and then appends the
/// first matching parent's hide command. Its original parent ID determines
/// the next iteration. Already-ended parents and missing IDs stop the loop;
/// fuel bounds even malformed cycles without assuming unique IDs or a DAG.
#[allow(clippy::too_many_arguments)]
pub fn end_ancestors_fuel<S, C, I, Read, Equal, Write, Emit>(
    len: usize,
    mut fuel: usize,
    mut state: S,
    mut commands: C,
    mut parent_index: Option<I>,
    read: Read,
    equal: Equal,
    write: Write,
    emit: Emit,
) -> (S, C)
where
    Read: ParentRead<S, I>,
    Equal: ParentEqual<I>,
    Write: Fn(S, usize, ParentEdit) -> S,
    Emit: Fn(C, I) -> C,
{
    while fuel > 0 {
        let Some(target) = parent_index else {
            return (state, commands);
        };
        let Some(parent) = find_first(len, &state, &target, &read, &equal) else {
            return (state, commands);
        };
        if parent.ended {
            return (state, commands);
        }
        state = update_all(len, state, &target, ParentEdit::End, &read, &equal, &write);
        commands = emit(commands, parent.note_index);
        parent_index = parent.parent_index;
        fuel -= 1;
    }
    (state, commands)
}

/// Only a newly ended child propagates, initially with one unit of fuel per
/// slide slot (including duplicate IDs).
#[allow(clippy::too_many_arguments)]
pub fn end_ancestors<S, C, I, Read, Equal, Write, Emit>(
    len: usize,
    state: S,
    commands: C,
    before_ended: bool,
    after_ended: bool,
    parent_index: Option<I>,
    read: Read,
    equal: Equal,
    write: Write,
    emit: Emit,
) -> (S, C)
where
    Read: ParentRead<S, I>,
    Equal: ParentEqual<I>,
    Write: Fn(S, usize, ParentEdit) -> S,
    Emit: Fn(C, I) -> C,
{
    if before_ended || !after_ended {
        (state, commands)
    } else {
        end_ancestors_fuel(
            len,
            len,
            state,
            commands,
            parent_index,
            read,
            equal,
            write,
            emit,
        )
    }
}
