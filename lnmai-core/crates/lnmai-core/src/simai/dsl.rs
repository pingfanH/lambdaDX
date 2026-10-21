//! Runtime projections from `LnmaiCore/Simai/DSL.lean`.
//! Lean's term elaborators remain compiler-specific; these functions implement
//! the data and literal operations that those elaborators call.

use super::frontend::{literal_or_default, parse_frontend_chart_result};
use super::shape::{canonical_shape_key, shape_kind, shape_mirrored};
use super::slide_tables::SlideAreaSpec;
use super::syntax::{RawNoteKind, SlideKind as SimaiSlideKind};
use crate::areas::{OuterSlot, SensorArea};
use crate::chart_loader::{SlideChartNote, SlideHeadChartNote};
use crate::integer::Integer;
use crate::natural::Natural;
use crate::types::{AreaPolicy, SlideKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNoteTokenIr {
    pub raw_text: String,
    pub kind: RawNoteKind,
    pub timing_micros: Integer,
    pub slot: Option<OuterSlot>,
    pub sensor_pos: Option<SensorArea>,
    pub length_micros: Option<Integer>,
    pub star_wait_micros: Option<Integer>,
    pub source_group_id: Option<Natural>,
    pub source_group_index: Option<Natural>,
    pub source_group_size: Option<Natural>,
    pub is_slide_no_head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideNoteIr {
    pub raw_text: String,
    pub start_slot: OuterSlot,
    pub end_area: SensorArea,
    pub kind: SimaiSlideKind,
    pub canonical: String,
    pub mirrored: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideAreaIr {
    pub target_areas: Vec<SensorArea>,
    pub policy: AreaPolicy,
    pub is_last: bool,
    pub is_skippable: bool,
    pub arrow_progress_when_on: Natural,
    pub arrow_progress_when_finished: Natural,
}

impl From<&SlideAreaSpec> for SlideAreaIr {
    fn from(area: &SlideAreaSpec) -> Self {
        Self {
            target_areas: area.target_areas.clone(),
            policy: area.policy,
            is_last: area.is_last,
            is_skippable: area.is_skippable,
            arrow_progress_when_on: area.arrow_progress_when_on.clone(),
            arrow_progress_when_finished: area.arrow_progress_when_finished.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSlideIr {
    pub logical_note_index: Natural,
    pub head_runtime_note_index: Option<Natural>,
    pub body_runtime_note_index: Option<Natural>,
    pub primary_runtime_note_index: Natural,
    pub note_index: Natural,
    pub head_timing_micros: Integer,
    pub slot: OuterSlot,
    pub start_timing_micros: Integer,
    pub length_micros: Integer,
    pub slide_kind: SlideKind,
    pub track_count: Natural,
    pub judge_at_micros: Option<Integer>,
    pub has_head_note: bool,
    pub has_body: bool,
    pub is_slide_no_head: bool,
    pub is_conn_slide: bool,
    pub is_group_head: bool,
    pub is_group_end: bool,
    pub total_judge_queue_len: Natural,
    pub judge_queues: Vec<Vec<SlideAreaIr>>,
    pub parent_note_index: Option<Natural>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NormalizedSlideRuntimeIds {
    pub logical_note_index: Natural,
    pub head_runtime_note_index: Option<Natural>,
    pub body_runtime_note_index: Option<Natural>,
    pub primary_runtime_note_index: Natural,
}

impl NormalizedSlideIr {
    pub fn runtime_ids(&self) -> NormalizedSlideRuntimeIds {
        NormalizedSlideRuntimeIds {
            logical_note_index: self.logical_note_index.clone(),
            head_runtime_note_index: self.head_runtime_note_index.clone(),
            body_runtime_note_index: self.body_runtime_note_index.clone(),
            primary_runtime_note_index: self.primary_runtime_note_index.clone(),
        }
    }
}

impl NormalizedSlideRuntimeIds {
    pub fn has_separate_head_and_body_runtime_ids(&self) -> bool {
        self.head_runtime_note_index.is_some() && self.body_runtime_note_index.is_some()
    }

    pub fn is_body_only_runtime_id_shape(&self) -> bool {
        self.head_runtime_note_index.is_none() && self.body_runtime_note_index.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSlideIr {
    pub logical_note_index: Natural,
    pub runtime_note_index: Natural,
    pub logical_slide_id: Natural,
    pub note_index: Natural,
    pub head_timing_micros: Integer,
    pub slot: OuterSlot,
    pub start_timing_micros: Integer,
    pub length_micros: Integer,
    pub slide_kind: SlideKind,
    pub judge_at_micros: Option<Integer>,
    pub is_slide_no_head: bool,
    pub is_conn_slide: bool,
    pub is_group_head: bool,
    pub is_group_end: bool,
    pub total_judge_queue_len: Natural,
    pub judge_queues: Vec<Vec<SlideAreaIr>>,
    pub parent_note_index: Option<Natural>,
}

impl From<&SlideChartNote> for LoweredSlideIr {
    fn from(slide: &SlideChartNote) -> Self {
        Self {
            logical_note_index: slide.logical_slide_id.clone(),
            runtime_note_index: slide.note_index.clone(),
            logical_slide_id: slide.logical_slide_id.clone(),
            note_index: slide.note_index.clone(),
            head_timing_micros: slide.head_timing.to_micros(),
            slot: slide.slot,
            start_timing_micros: slide.start_timing.to_micros(),
            length_micros: slide.length.to_micros(),
            slide_kind: slide.slide_kind,
            judge_at_micros: slide.judge_at.as_ref().map(|at| at.to_micros()),
            is_slide_no_head: slide.is_slide_no_head,
            is_conn_slide: slide.is_conn_slide,
            is_group_head: slide.is_group_head,
            is_group_end: slide.is_group_end,
            total_judge_queue_len: slide.total_judge_queue_len.clone(),
            judge_queues: slide
                .judge_queues
                .iter()
                .map(|queue| queue.iter().map(SlideAreaIr::from).collect())
                .collect(),
            parent_note_index: slide.parent_note_index.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSlideHeadIr {
    pub logical_note_index: Natural,
    pub runtime_note_index: Natural,
    pub logical_slide_id: Natural,
    pub note_index: Natural,
    pub timing_micros: Integer,
    pub slot: OuterSlot,
    pub is_break: bool,
    pub is_ex: bool,
}

impl From<&SlideHeadChartNote> for LoweredSlideHeadIr {
    fn from(head: &SlideHeadChartNote) -> Self {
        Self {
            logical_note_index: head.logical_slide_id.clone(),
            runtime_note_index: head.note_index.clone(),
            logical_slide_id: head.logical_slide_id.clone(),
            note_index: head.note_index.clone(),
            timing_micros: head.timing.to_micros(),
            slot: head.slot,
            is_break: head.is_break,
            is_ex: head.is_ex,
        }
    }
}

impl Default for LoweredSlideHeadIr {
    fn default() -> Self {
        Self::from(&SlideHeadChartNote::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoweredSlideSplitIr {
    pub slide_heads: Vec<LoweredSlideHeadIr>,
    pub slide_bodies: Vec<LoweredSlideIr>,
}

fn single_note_maidata(note_text: &str) -> String {
    format!("&first=0\n&inote_1=\n(120)\n{note_text},\n")
}

pub fn note_ir_literal(note_text: &str) -> Vec<RawNoteTokenIr> {
    literal_or_default(
        parse_frontend_chart_result(&single_note_maidata(note_text), 1.into()).map(|chart| {
            chart
                .inspection
                .tokens
                .into_iter()
                .map(|tok| RawNoteTokenIr {
                    raw_text: tok.raw_text,
                    kind: tok.kind,
                    timing_micros: tok.timing.to_micros(),
                    slot: tok.slot,
                    sensor_pos: tok.sensor_pos,
                    length_micros: tok.length.map(|length| length.to_micros()),
                    star_wait_micros: tok.star_wait.map(|wait| wait.to_micros()),
                    source_group_id: tok.source_group_id,
                    source_group_index: tok.source_group_index,
                    source_group_size: tok.source_group_size,
                    is_slide_no_head: tok.is_slide_no_head,
                })
                .collect()
        }),
        "note IR",
    )
}

pub fn slide_ir_literal(note_text: &str) -> Vec<SlideNoteIr> {
    literal_or_default(
        parse_frontend_chart_result(&single_note_maidata(note_text), 1.into()).map(|chart| {
            chart
                .inspection
                .slide_notes
                .into_iter()
                .map(|slide| SlideNoteIr {
                    raw_text: slide.raw_text,
                    start_slot: slide.start_slot,
                    end_area: slide.end_area,
                    kind: shape_kind(&slide.shape),
                    canonical: canonical_shape_key(&slide.shape),
                    mirrored: shape_mirrored(&slide.shape),
                })
                .collect()
        }),
        "slide IR",
    )
}

pub fn normalized_slide_ir_literal(note_text: &str) -> Vec<NormalizedSlideIr> {
    literal_or_default(
        parse_frontend_chart_result(&single_note_maidata(note_text), 1.into()).map(|chart| {
            chart
                .semantic
                .normalized
                .slides
                .iter()
                .map(|slide| {
                    let head_index = chart
                        .semantic
                        .lowered
                        .slide_heads
                        .iter()
                        .find(|head| head.logical_slide_id == slide.note_index)
                        .map(|head| head.note_index.clone());
                    let body_index = chart
                        .semantic
                        .lowered
                        .slides
                        .iter()
                        .find(|body| body.logical_slide_id == slide.note_index)
                        .map(|body| body.note_index.clone());
                    NormalizedSlideIr {
                        logical_note_index: slide.note_index.clone(),
                        primary_runtime_note_index: body_index
                            .clone()
                            .or_else(|| head_index.clone())
                            .unwrap_or_else(|| slide.note_index.clone()),
                        head_runtime_note_index: head_index,
                        body_runtime_note_index: body_index,
                        note_index: slide.note_index.clone(),
                        head_timing_micros: slide.head_timing.to_micros(),
                        slot: slide.slot,
                        start_timing_micros: slide.start_timing.to_micros(),
                        length_micros: slide.length.to_micros(),
                        slide_kind: slide.slide_kind,
                        track_count: slide.track_count.clone(),
                        judge_at_micros: slide.judge_at.as_ref().map(|at| at.to_micros()),
                        has_head_note: slide.has_head_note,
                        has_body: slide.has_body,
                        is_slide_no_head: slide.is_slide_no_head,
                        is_conn_slide: slide.is_conn_slide,
                        is_group_head: slide.is_group_head,
                        is_group_end: slide.is_group_end,
                        total_judge_queue_len: slide.total_judge_queue_len.clone(),
                        judge_queues: slide
                            .judge_queues
                            .iter()
                            .map(|queue| queue.iter().map(SlideAreaIr::from).collect())
                            .collect(),
                        parent_note_index: slide.parent_note_index.clone(),
                    }
                })
                .collect()
        }),
        "normalized slide IR",
    )
}

pub fn lowered_slide_ir_literal(note_text: &str) -> Vec<LoweredSlideIr> {
    literal_or_default(
        parse_frontend_chart_result(&single_note_maidata(note_text), 1.into()).map(|chart| {
            chart
                .semantic
                .lowered
                .slides
                .iter()
                .map(LoweredSlideIr::from)
                .collect()
        }),
        "lowered slide IR",
    )
}

pub fn lowered_slide_split_ir_literal(note_text: &str) -> LoweredSlideSplitIr {
    literal_or_default(
        parse_frontend_chart_result(&single_note_maidata(note_text), 1.into()).map(|chart| {
            LoweredSlideSplitIr {
                slide_heads: chart
                    .semantic
                    .lowered
                    .slide_heads
                    .iter()
                    .map(LoweredSlideHeadIr::from)
                    .collect(),
                slide_bodies: chart
                    .semantic
                    .lowered
                    .slides
                    .iter()
                    .map(LoweredSlideIr::from)
                    .collect(),
            }
        }),
        "lowered slide split IR",
    )
}
