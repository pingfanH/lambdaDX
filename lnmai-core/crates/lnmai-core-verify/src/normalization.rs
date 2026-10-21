//! Shared, parser-independent normalization control flow.
//! This new module is a source artifact: Charon/Aeneas extraction is pending.
//! Runtime adapters may read one discriminant or write one queue slot; none
//! may return an entire normalized note, queue traversal, or parser result.

use crate::shape::SlideKind as ShapeSlideKind;
use crate::types::SlideKind;

#[derive(Debug, Clone, Copy)]
pub enum NoteKind {
    Tap,
    Hold,
    Slide,
    Touch,
    TouchHold,
    Rest,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct RawFlags {
    pub is_break: bool,
    pub is_ex: bool,
    pub is_hanabi: bool,
    pub is_slide_no_head: bool,
    pub is_force_star: bool,
    pub is_fake_rotate: bool,
    pub is_slide_break: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TapFlags {
    pub is_break: bool,
    pub is_ex: bool,
    pub is_hanabi: bool,
    pub is_force_star: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct HoldFlags {
    pub is_break: bool,
    pub is_ex: bool,
    pub is_hanabi: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TouchFlags {
    pub is_break: bool,
    pub is_hanabi: bool,
}

/// Slide requests parsing; it is not itself an accepted/emitted note.
/// Identifiers, times, durations and source metadata stay outside this plan.
pub enum NotePlan<Slot, Sensor> {
    Skip,
    Tap(Slot, TapFlags),
    Hold(Slot, HoldFlags),
    Touch(Sensor, TouchFlags),
    TouchHold(Sensor, HoldFlags),
    Slide(Slot, RawFlags),
}

/// The complete kind/position/flag decision for one raw token.
/// Sensor presence cannot rescue a missing Tap/Hold/Slide slot and vice versa.
pub fn plan_note<Slot, Sensor>(
    kind: NoteKind,
    slot: Option<Slot>,
    sensor: Option<Sensor>,
    flags: RawFlags,
) -> NotePlan<Slot, Sensor> {
    match kind {
        NoteKind::Tap => match slot {
            Some(slot) => NotePlan::Tap(
                slot,
                TapFlags {
                    is_break: flags.is_break,
                    is_ex: flags.is_ex,
                    is_hanabi: flags.is_hanabi,
                    is_force_star: flags.is_force_star,
                },
            ),
            None => NotePlan::Skip,
        },
        NoteKind::Hold => match slot {
            Some(slot) => NotePlan::Hold(
                slot,
                HoldFlags {
                    is_break: flags.is_break,
                    is_ex: flags.is_ex,
                    is_hanabi: flags.is_hanabi,
                },
            ),
            None => NotePlan::Skip,
        },
        NoteKind::Touch => match sensor {
            Some(sensor) => NotePlan::Touch(
                sensor,
                TouchFlags {
                    is_break: flags.is_break,
                    is_hanabi: flags.is_hanabi,
                },
            ),
            None => NotePlan::Skip,
        },
        NoteKind::TouchHold => match sensor {
            Some(sensor) => NotePlan::TouchHold(
                sensor,
                HoldFlags {
                    is_break: flags.is_break,
                    is_ex: flags.is_ex,
                    is_hanabi: flags.is_hanabi,
                },
            ),
            None => NotePlan::Skip,
        },
        NoteKind::Slide => match slot {
            Some(slot) => NotePlan::Slide(slot, flags),
            None => NotePlan::Skip,
        },
        NoteKind::Rest | NoteKind::Unknown => NotePlan::Skip,
    }
}

/// One discriminant read, including shapes with arbitrary Natural endpoints.
pub trait ShapeKind<Shape> {
    fn kind(&self, shape: &Shape) -> ShapeSlideKind;
}

pub struct SlideLayout {
    pub flags: RawFlags,
    pub slide_kind: SlideKind,
    pub track_count: usize,
    pub is_classic: bool,
    pub has_head_note: bool,
    pub has_body: bool,
    pub is_conn_slide: bool,
    pub is_group_head: bool,
    pub is_group_end: bool,
    pub total_judge_queue_len: usize,
    pub multiple: usize,
}

/// Successful lowerSlideToken construction only: no parser or timing callback.
/// Kind/track count depend solely on shape kind, including malformed shapes.
/// No-head affects the head flag only, never body existence or multiplicity.
pub fn slide_layout<Shape, Ops>(
    input_shape: &Shape,
    flags: RawFlags,
    is_classic: bool,
    ops: Ops,
) -> SlideLayout
where
    Ops: ShapeKind<Shape>,
{
    let (slide_kind, track_count) = match ops.kind(input_shape) {
        ShapeSlideKind::Wifi => (SlideKind::Wifi, 3),
        _ => (SlideKind::Single, 1),
    };
    let has_head_note = !flags.is_slide_no_head;
    SlideLayout {
        flags,
        slide_kind,
        track_count,
        is_classic,
        has_head_note,
        has_body: true,
        is_conn_slide: false,
        is_group_head: false,
        is_group_end: false,
        total_judge_queue_len: 0,
        multiple: 1,
    }
}

/// In-bounds, single-slot lens. No search, iteration, filtering or allocation
/// is delegated. Preserve all other slots and every non-is_skippable field.
pub trait TrackWrite<Q> {
    fn set_skippable(&self, queue: Q, index: usize, value: bool) -> Q;
}

/// Full applySingleTrackConnRulesNormalized control flow.
/// `len` equals the physical queue length; writes preserve it.
/// `total_lt_four` is the exact Natural comparison totalJudgeQueueLen < 4.
/// Small tracks change at most their first two slots, regardless of len.
/// Long tracks visit every slot, preserving order and any duplicate contents.
pub fn normalize_track<Q, Ops>(
    is_conn_slide: bool,
    total_lt_four: bool,
    is_group_head: bool,
    is_group_end: bool,
    len: usize,
    mut queue: Q,
    ops: Ops,
) -> Q
where
    Ops: TrackWrite<Q>,
{
    if !is_conn_slide {
        return queue;
    }
    if total_lt_four {
        if len == 0 {
            return queue;
        }
        if len == 1 {
            return ops.set_skippable(queue, 0, is_group_head || is_group_end);
        }
        queue = ops.set_skippable(queue, 0, is_group_head);
        ops.set_skippable(queue, 1, is_group_end)
    } else {
        let mut index = 0;
        while index < len {
            queue = ops.set_skippable(queue, index, true);
            // index < len <= usize::MAX proves this addition cannot overflow.
            index += 1;
        }
        queue
    }
}
