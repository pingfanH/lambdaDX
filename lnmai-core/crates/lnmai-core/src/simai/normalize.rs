//! Lower raw tokens into the normalized chart IR and then into `ChartSpec`.
//!
//! Mirrors `LnmaiCore/Simai/Normalize.lean`.

use crate::chart_loader::{
    ChartSpec, HoldChartNote, SlideChartNote, SlideHeadChartNote, TapChartNote, TouchChartNote,
    TouchHoldChartNote,
};
use crate::integer::Integer;
use crate::natural::Natural;
use crate::rat::Rat;
use crate::simai::ir::{
    NormalizedChart, NormalizedHold, NormalizedSlide, NormalizedSlideDebug, NormalizedTap,
    NormalizedTouch, NormalizedTouchHold,
};
use crate::simai::shape::{shape_key, shape_kind};
use crate::simai::slide_parser::{
    parse_slide_just_text, parse_slide_note, parse_slide_note_from_body, parse_terminal_end_area,
};
use crate::simai::slide_tables::{
    judge_queues_for_shape, rotate_judge_queues, slide_last_wait_ratio_for_shape, SlideAreaSpec,
};
use crate::simai::syntax::{
    RawNoteKind, RawNoteToken, SlideKind as ShapeSlideKind, SlideNoteSemantics,
};
use crate::simai::timing::note_timing_increment;
use crate::time::{time_point_add_duration, Duration, TimePoint};
use lnmai_core_verify::connected_slide_lifecycle as connected_slide_lifecycle_kernel;
use lnmai_core_verify::connected_slide_metadata::{self as connected_slide_kernel, MetadataStep};
use lnmai_core_verify::normalization::{self as normalize_kernel, NotePlan};

fn normalization_kind(kind: RawNoteKind) -> normalize_kernel::NoteKind {
    use normalize_kernel::NoteKind as K;
    match kind {
        RawNoteKind::Tap => K::Tap,
        RawNoteKind::Hold => K::Hold,
        RawNoteKind::Slide => K::Slide,
        RawNoteKind::Touch => K::Touch,
        RawNoteKind::TouchHold => K::TouchHold,
        RawNoteKind::Rest => K::Rest,
        RawNoteKind::Unknown => K::Unknown,
    }
}

fn normalization_flags(token: &RawNoteToken) -> normalize_kernel::RawFlags {
    normalize_kernel::RawFlags {
        is_break: token.is_break,
        is_ex: token.is_ex,
        is_hanabi: token.is_hanabi,
        is_slide_no_head: token.is_slide_no_head,
        is_force_star: token.is_force_star,
        is_fake_rotate: token.is_fake_rotate,
        is_slide_break: token.is_slide_break,
    }
}

struct NormalizationShapeKind;

impl normalize_kernel::ShapeKind<crate::simai::syntax::SlideShape> for NormalizationShapeKind {
    fn kind(&self, shape: &crate::simai::syntax::SlideShape) -> ShapeSlideKind {
        shape_kind(shape)
    }
}

struct NormalizationTrackWrite;

impl normalize_kernel::TrackWrite<Vec<SlideAreaSpec>> for NormalizationTrackWrite {
    fn set_skippable(
        &self,
        mut queue: Vec<SlideAreaSpec>,
        index: usize,
        value: bool,
    ) -> Vec<SlideAreaSpec> {
        queue[index].is_skippable = value;
        queue
    }
}

fn total_judge_queue_len(queues: &[Vec<SlideAreaSpec>]) -> usize {
    queues.iter().map(|q| q.len()).max().unwrap_or(0)
}

fn apply_single_track_conn_rules_normalized(
    slide: &NormalizedSlide,
    queue: &[SlideAreaSpec],
) -> Vec<SlideAreaSpec> {
    normalize_kernel::normalize_track(
        slide.is_conn_slide,
        slide.total_judge_queue_len < 4,
        slide.is_group_head,
        slide.is_group_end,
        queue.len(),
        queue.to_vec(),
        NormalizationTrackWrite,
    )
}

fn attach_judge_queues(slide: &NormalizedSlide) -> NormalizedSlide {
    let raw_queues =
        judge_queues_for_shape(&slide.simai_shape, slide.is_classic).unwrap_or_default();
    let placed_queues = rotate_judge_queues(slide.slot.to_index(), &raw_queues);
    NormalizedSlide {
        total_judge_queue_len: (total_judge_queue_len(&placed_queues)).into(),
        judge_queues: placed_queues,
        ..slide.clone()
    }
}

fn apply_connected_queue_rules(group: &[NormalizedSlide]) -> Vec<NormalizedSlide> {
    let total = group
        .iter()
        .map(|s| s.total_judge_queue_len.clone())
        .sum::<Natural>()
        .saturating_sub(group.len().saturating_sub(1));
    group
        .iter()
        .map(|slide| {
            let with_len = NormalizedSlide {
                total_judge_queue_len: total.clone(),
                ..slide.clone()
            };
            NormalizedSlide {
                judge_queues: with_len
                    .judge_queues
                    .iter()
                    .map(|q| apply_single_track_conn_rules_normalized(&with_len, q))
                    .collect(),
                ..with_len
            }
        })
        .collect()
}

fn slide_debug_for(chart: &NormalizedChart, note_index: Natural) -> Option<NormalizedSlideDebug> {
    chart
        .slide_debug
        .iter()
        .find(|dbg| dbg.note_index == note_index)
        .cloned()
}

/// `lowerSlideToken`.
pub fn lower_slide_token_with_mode(
    is_classic: bool,
    note_index: Natural,
    token: &RawNoteToken,
) -> Option<(NormalizedSlide, SlideNoteSemantics)> {
    // Direct calls deliberately do not inspect token.kind.
    let slot = token.slot?;
    lower_slide_token_at_slot(
        is_classic,
        note_index,
        token,
        slot,
        normalization_flags(token),
    )
}

/// Slot comes from the plan or public wrapper's Some guard.
/// Parsing and exact-time operations retain their original order.
fn lower_slide_token_at_slot(
    is_classic: bool,
    note_index: Natural,
    token: &RawNoteToken,
    slot: crate::areas::OuterSlot,
    flags: normalize_kernel::RawFlags,
) -> Option<(NormalizedSlide, SlideNoteSemantics)> {
    let end_area = parse_terminal_end_area(&token.raw_text).ok()?;
    let parsed = match &token.slide_body {
        Some(body) => parse_slide_note_from_body(&token.raw_text, body, end_area),
        None => parse_slide_note(&token.raw_text, slot, end_area),
    }
    .ok()?;
    let layout =
        normalize_kernel::slide_layout(&parsed.shape, flags, is_classic, NormalizationShapeKind);
    let length = token.length.clone().unwrap_or_else(|| {
        note_timing_increment(token.bpm.clone(), token.divisor.clone().max((1).into()))
    });
    let star_wait = token.star_wait.clone().unwrap_or_else(Duration::zero);
    let start_timing = time_point_add_duration(token.timing.clone(), star_wait);
    let last_wait_ratio =
        slide_last_wait_ratio_for_shape(&parsed.shape, is_classic).unwrap_or_else(Rat::zero);
    let judge_duration = crate::time::duration_from_rat_micros(
        Rat::from_int(length.to_micros()).mul(Rat::one().sub(last_wait_ratio)),
    );
    let judge_at = time_point_add_duration(start_timing.clone(), judge_duration);
    let slide = NormalizedSlide {
        head_timing: token.timing.clone(),
        slot,
        length,
        start_timing,
        group_start_timing: None,
        h_speed: token.h_speed.clone(),
        slide_kind: layout.slide_kind,
        is_classic: layout.is_classic,
        track_count: layout.track_count.into(),
        judge_at: Some(judge_at),
        is_break: layout.flags.is_break,
        is_ex: layout.flags.is_ex,
        is_hanabi: layout.flags.is_hanabi,
        has_head_note: layout.has_head_note,
        has_body: layout.has_body,
        is_slide_no_head: layout.flags.is_slide_no_head,
        is_force_star: layout.flags.is_force_star,
        is_fake_rotate: layout.flags.is_fake_rotate,
        is_slide_break: layout.flags.is_slide_break,
        is_conn_slide: layout.is_conn_slide,
        parent_note_index: None,
        is_group_head: layout.is_group_head,
        is_group_end: layout.is_group_end,
        total_judge_queue_len: layout.total_judge_queue_len.into(),
        judge_queues: Vec::new(),
        source_group_id: token.source_group_id.clone(),
        source_group_index: token.source_group_index.clone(),
        source_group_size: token.source_group_size.clone(),
        multiple: layout.multiple.into(),
        note_index,
        simai_shape: parsed.shape.clone(),
    };
    Some((slide, parsed))
}

pub fn lower_slide_token(
    note_index: Natural,
    token: &RawNoteToken,
) -> Option<(NormalizedSlide, SlideNoteSemantics)> {
    lower_slide_token_with_mode(false, note_index, token)
}

fn apply_connected_slide_metadata(slides: &[NormalizedSlide]) -> Vec<NormalizedSlide> {
    let mut acc: Vec<NormalizedSlide> = Vec::new();
    let mut current_group_id: Option<Natural> = None;
    let mut parent_note_index: Option<Natural> = None;
    let mut parent_end_timing: Option<TimePoint> = None;
    let mut group_start_timing_state: Option<TimePoint> = None;

    for slide in slides {
        match (
            slide.source_group_id.clone(),
            slide.source_group_index.clone(),
            slide.source_group_size.clone(),
        ) {
            (Some(_), Some(_), Some(_)) => {
                let metadata = connected_slide_kernel::apply(
                    slide
                        .source_group_id
                        .as_ref()
                        .map(|value| value.kernel_value().clone()),
                    slide
                        .source_group_index
                        .as_ref()
                        .map(|value| value.kernel_value().clone()),
                    slide
                        .source_group_size
                        .as_ref()
                        .map(|value| value.kernel_value().clone()),
                    slide.slide_kind,
                    current_group_id
                        .as_ref()
                        .map(|value| value.kernel_value().clone()),
                    parent_note_index
                        .as_ref()
                        .map(|value| value.kernel_value().clone()),
                    slide.note_index.kernel_value().clone(),
                );
                match metadata {
                    MetadataStep::Clear | MetadataStep::Reset => {
                        acc.push(NormalizedSlide {
                            is_conn_slide: false,
                            is_group_head: false,
                            is_group_end: false,
                            parent_note_index: None,
                            ..slide.clone()
                        });
                        current_group_id = None;
                        parent_note_index = None;
                        parent_end_timing = None;
                        group_start_timing_state = None;
                    }
                    MetadataStep::Apply(metadata) => {
                        let timing = connected_slide_lifecycle_kernel::apply(
                            metadata.is_conn_slide,
                            metadata.is_group_head,
                            metadata.is_group_end,
                            slide.start_timing.micros.kernel(),
                            slide.judge_at.as_ref().map(|tp| tp.micros.kernel()),
                            slide.length.micros.kernel(),
                            parent_end_timing.as_ref().map(|tp| tp.micros.kernel()),
                            group_start_timing_state
                                .as_ref()
                                .map(|tp| tp.micros.kernel()),
                        );
                        let start_timing =
                            TimePoint::from_micros(Integer::from_kernel(timing.start_timing));
                        let stored_group_start = timing
                            .stored_group_start_timing
                            .map(|value| TimePoint::from_micros(Integer::from_kernel(value)));
                        let judge_at = timing
                            .judge_at
                            .map(|value| TimePoint::from_micros(Integer::from_kernel(value)));
                        let updated = NormalizedSlide {
                            start_timing,
                            group_start_timing: stored_group_start,
                            judge_at,
                            slide_kind: metadata.slide_kind,
                            is_conn_slide: metadata.is_conn_slide,
                            parent_note_index: metadata.parent_note_index.map(Natural::from_kernel),
                            is_group_head: metadata.is_group_head,
                            is_group_end: metadata.is_group_end,
                            ..slide.clone()
                        };
                        acc.push(updated.clone());
                        current_group_id = metadata.next_group_id.map(Natural::from_kernel);
                        parent_note_index =
                            metadata.next_parent_note_index.map(Natural::from_kernel);
                        parent_end_timing = timing
                            .next_parent_end_timing
                            .map(|value| TimePoint::from_micros(Integer::from_kernel(value)));
                        group_start_timing_state = timing
                            .next_group_start_timing
                            .map(|value| TimePoint::from_micros(Integer::from_kernel(value)));
                    }
                }
            }
            _ => {
                acc.push(NormalizedSlide {
                    is_conn_slide: false,
                    is_group_head: false,
                    is_group_end: false,
                    parent_note_index: None,
                    ..slide.clone()
                });
                current_group_id = None;
                parent_note_index = None;
                parent_end_timing = None;
                group_start_timing_state = None;
            }
        }
    }
    acc
}

fn slide_fold_fields_match(left: &NormalizedSlide, right: &NormalizedSlide) -> bool {
    left.head_timing == right.head_timing
        && left.slot == right.slot
        && left.length == right.length
        && left.start_timing == right.start_timing
        && left.h_speed == right.h_speed
        && left.slide_kind == right.slide_kind
        && left.is_classic == right.is_classic
        && left.track_count == right.track_count
        && left.judge_at == right.judge_at
        && left.is_break == right.is_break
        && left.is_ex == right.is_ex
        && left.is_hanabi == right.is_hanabi
        && left.has_head_note == right.has_head_note
        && left.has_body == right.has_body
        && left.is_slide_no_head == right.is_slide_no_head
        && left.is_force_star == right.is_force_star
        && left.is_fake_rotate == right.is_fake_rotate
        && left.is_slide_break == right.is_slide_break
        && left.source_group_index == right.source_group_index
        && left.source_group_size == right.source_group_size
        && left.simai_shape == right.simai_shape
}

fn take_same_source_group(
    gid: Natural,
    slides: &[NormalizedSlide],
) -> (Vec<NormalizedSlide>, Vec<NormalizedSlide>) {
    let mut group = Vec::new();
    let mut i = 0;
    while i < slides.len()
        && slides[i].source_group_id == Some(gid.clone())
        && slides[i].source_group_index != Some((0).into())
    {
        group.push(slides[i].clone());
        i += 1;
    }
    (group, slides[i..].to_vec())
}

fn split_slide_multiplicity_units(slides: &[NormalizedSlide]) -> Vec<Vec<NormalizedSlide>> {
    let mut units = Vec::new();
    let mut rest: Vec<NormalizedSlide> = slides.to_vec();
    while !rest.is_empty() {
        match rest[0].source_group_id.clone() {
            None => {
                units.push(vec![rest[0].clone()]);
                rest.remove(0);
            }
            Some(gid) => {
                let (group_tail, remaining) = take_same_source_group(gid, &rest[1..]);
                let mut unit = vec![rest[0].clone()];
                unit.extend(group_tail);
                units.push(unit);
                rest = remaining;
            }
        }
    }
    units
}

fn slide_multiplicity_units_can_fold(left: &[NormalizedSlide], right: &[NormalizedSlide]) -> bool {
    match (left.first(), right.first()) {
        (Some(lh), Some(rh)) => match (lh.source_group_id.clone(), rh.source_group_id.clone()) {
            (None, None) => match (left, right) {
                ([ls], [rs]) => slide_fold_fields_match(ls, rs),
                _ => false,
            },
            (Some(lg), Some(rg)) => {
                lg != rg
                    && left.len() == right.len()
                    && left
                        .iter()
                        .zip(right.iter())
                        .all(|(l, r)| slide_fold_fields_match(l, r))
            }
            _ => false,
        },
        _ => false,
    }
}

fn merge_slide_multiplicity_unit(
    left: &[NormalizedSlide],
    right: &[NormalizedSlide],
) -> Vec<NormalizedSlide> {
    left.iter()
        .zip(right.iter())
        .map(|(l, r)| NormalizedSlide {
            multiple: l.multiple.clone() + r.multiple.clone(),
            ..l.clone()
        })
        .collect()
}

fn fold_slide_multiplicity(slides: &[NormalizedSlide]) -> Vec<NormalizedSlide> {
    let mut units: Vec<Vec<NormalizedSlide>> = Vec::new();
    for unit in split_slide_multiplicity_units(slides) {
        if let Some(pos) = units
            .iter()
            .position(|u| slide_multiplicity_units_can_fold(u, &unit))
        {
            let merged = merge_slide_multiplicity_unit(&units[pos], &unit);
            units[pos] = merged;
        } else {
            units.push(unit);
        }
    }
    units.into_iter().flatten().collect()
}

/// `lowerRawTokens`.
pub fn lower_raw_tokens_with_mode(
    is_classic: bool,
    measure_dur_sec: &dyn Fn(Rat) -> Duration,
    tokens: &[RawNoteToken],
) -> (NormalizedChart, Vec<SlideNoteSemantics>) {
    let mut note_index: Natural = 1.into();
    let mut taps: Vec<NormalizedTap> = Vec::new();
    let mut holds: Vec<NormalizedHold> = Vec::new();
    let mut touches: Vec<NormalizedTouch> = Vec::new();
    let mut touch_holds: Vec<NormalizedTouchHold> = Vec::new();
    let mut slides: Vec<NormalizedSlide> = Vec::new();
    let mut slide_debug: Vec<NormalizedSlideDebug> = Vec::new();
    let mut slide_semantics: Vec<SlideNoteSemantics> = Vec::new();

    for token in tokens {
        let plan = normalize_kernel::plan_note(
            normalization_kind(token.kind),
            token.slot,
            token.sensor_pos,
            normalization_flags(token),
        );
        match plan {
            NotePlan::Skip => {}
            NotePlan::Tap(slot, flags) => {
                taps.push(NormalizedTap {
                    timing: token.timing.clone(),
                    slot,
                    is_break: flags.is_break,
                    is_ex: flags.is_ex,
                    is_hanabi: flags.is_hanabi,
                    is_force_star: flags.is_force_star,
                    note_index: note_index.clone(),
                });
                note_index += 1;
            }
            NotePlan::Hold(slot, flags) => {
                holds.push(NormalizedHold {
                    timing: token.timing.clone(),
                    slot,
                    length: token
                        .length
                        .clone()
                        .unwrap_or_else(|| measure_dur_sec(token.bpm.clone())),
                    is_break: flags.is_break,
                    is_ex: flags.is_ex,
                    is_hanabi: flags.is_hanabi,
                    note_index: note_index.clone(),
                });
                note_index += 1;
            }
            NotePlan::Touch(sensor_pos, flags) => {
                touches.push(NormalizedTouch {
                    timing: token.timing.clone(),
                    sensor_pos,
                    is_break: flags.is_break,
                    is_hanabi: flags.is_hanabi,
                    source_group_id: token.source_group_id.clone(),
                    source_group_index: token.source_group_index.clone(),
                    source_group_size: token.source_group_size.clone(),
                    note_index: note_index.clone(),
                });
                note_index += 1;
            }
            NotePlan::TouchHold(sensor_pos, flags) => {
                touch_holds.push(NormalizedTouchHold {
                    timing: token.timing.clone(),
                    sensor_pos,
                    length: token
                        .length
                        .clone()
                        .unwrap_or_else(|| measure_dur_sec(token.bpm.clone())),
                    is_break: flags.is_break,
                    is_ex: flags.is_ex,
                    is_hanabi: flags.is_hanabi,
                    source_group_id: token.source_group_id.clone(),
                    source_group_index: token.source_group_index.clone(),
                    source_group_size: token.source_group_size.clone(),
                    note_index: note_index.clone(),
                });
                note_index += 1;
            }
            NotePlan::Slide(slot, flags) => {
                if let Some((slide, parsed)) =
                    lower_slide_token_at_slot(is_classic, note_index.clone(), token, slot, flags)
                {
                    slides.push(slide);
                    slide_debug.push(NormalizedSlideDebug {
                        note_index: note_index.clone(),
                        raw_text: token.raw_text.clone(),
                    });
                    slide_semantics.push(parsed);
                    note_index += 1;
                }
            }
        }
    }

    let folded = fold_slide_multiplicity(&slides);
    let folded = apply_connected_slide_metadata(&folded);
    let attached: Vec<NormalizedSlide> = folded.iter().map(attach_judge_queues).collect();
    let lowered_slides = split_slide_multiplicity_units(&attached)
        .iter()
        .flat_map(|group| apply_connected_queue_rules(group))
        .collect();

    (
        NormalizedChart {
            taps,
            holds,
            touches,
            touch_holds,
            slides: lowered_slides,
            slide_debug,
            slide_skipping: true,
        },
        slide_semantics,
    )
}

/// `toChartSpec`.
pub fn lower_raw_tokens(
    measure_dur_sec: &dyn Fn(Rat) -> Duration,
    tokens: &[RawNoteToken],
) -> (NormalizedChart, Vec<SlideNoteSemantics>) {
    lower_raw_tokens_with_mode(false, measure_dur_sec, tokens)
}

pub fn to_chart_spec(chart: &NormalizedChart) -> ChartSpec {
    let mut max_note_index = Natural::from(0);
    for note in &chart.taps {
        max_note_index = max_note_index.max(note.note_index.clone());
    }
    for note in &chart.holds {
        max_note_index = max_note_index.max(note.note_index.clone());
    }
    for note in &chart.touches {
        max_note_index = max_note_index.max(note.note_index.clone());
    }
    for note in &chart.touch_holds {
        max_note_index = max_note_index.max(note.note_index.clone());
    }
    for note in &chart.slides {
        max_note_index = max_note_index.max(note.note_index.clone());
    }

    let mut next_note_index = max_note_index + 1;
    let mut slide_heads_rev: Vec<SlideHeadChartNote> = Vec::new();
    for note in &chart.slides {
        if !note.has_head_note {
            continue;
        }
        let count = note.multiple.clone().max((1).into());
        let mut generated: Vec<SlideHeadChartNote> = count
            .range()
            .map(|offset| SlideHeadChartNote {
                timing: note.head_timing.clone(),
                slot: note.slot,
                is_break: note.is_break,
                is_ex: note.is_ex,
                logical_slide_id: note.note_index.clone(),
                note_index: next_note_index.clone() + offset,
            })
            .collect();
        generated.reverse();
        generated.extend(slide_heads_rev);
        slide_heads_rev = generated;
        next_note_index += count;
    }
    slide_heads_rev.reverse();

    ChartSpec {
        taps: chart
            .taps
            .iter()
            .map(|note| TapChartNote {
                timing: note.timing.clone(),
                slot: note.slot,
                is_break: note.is_break,
                is_ex: note.is_ex,
                note_index: note.note_index.clone(),
                ..TapChartNote::default()
            })
            .collect(),
        holds: chart
            .holds
            .iter()
            .map(|note| HoldChartNote {
                timing: note.timing.clone(),
                slot: note.slot,
                length: note.length.clone(),
                is_break: note.is_break,
                is_ex: note.is_ex,
                is_touch: false,
                note_index: note.note_index.clone(),
                ..HoldChartNote::default()
            })
            .collect(),
        touches: chart
            .touches
            .iter()
            .map(|note| TouchChartNote {
                timing: note.timing.clone(),
                sensor_pos: note.sensor_pos,
                is_break: note.is_break,
                source_group_id: note.source_group_id.clone(),
                source_group_index: note.source_group_index.clone(),
                source_group_size: note.source_group_size.clone(),
                note_index: note.note_index.clone(),
                ..TouchChartNote::default()
            })
            .collect(),
        touch_holds: chart
            .touch_holds
            .iter()
            .map(|note| TouchHoldChartNote {
                timing: note.timing.clone(),
                sensor_pos: note.sensor_pos,
                length: note.length.clone(),
                is_break: note.is_break,
                is_ex: note.is_ex,
                source_group_id: note.source_group_id.clone(),
                source_group_index: note.source_group_index.clone(),
                source_group_size: note.source_group_size.clone(),
                note_index: note.note_index.clone(),
                ..TouchHoldChartNote::default()
            })
            .collect(),
        slide_heads: slide_heads_rev,
        slides: chart
            .slides
            .iter()
            .map(|note| SlideChartNote {
                head_timing: note.head_timing.clone(),
                slot: note.slot,
                length: note.length.clone(),
                start_timing: note.start_timing.clone(),
                group_start_timing: note.group_start_timing.clone(),
                slide_kind: note.slide_kind,
                is_classic: note.is_classic,
                is_slide_no_head: note.is_slide_no_head,
                is_conn_slide: note.is_conn_slide,
                parent_note_index: note.parent_note_index.clone(),
                is_group_head: note.is_group_head,
                is_group_end: note.is_group_end,
                total_judge_queue_len: note.total_judge_queue_len.clone(),
                track_count: note.track_count.clone(),
                judge_at: note.judge_at.clone(),
                is_break: note.is_slide_break,
                is_ex: note.is_ex,
                multiple: note.multiple.clone(),
                logical_slide_id: note.note_index.clone(),
                note_index: note.note_index.clone(),
                judge_queues: note.judge_queues.clone(),
                debug_simai: slide_debug_for(chart, note.note_index.clone()).map(|dbg| {
                    let just = parse_slide_just_text(&dbg.raw_text).unwrap_or(false);
                    (dbg.raw_text, shape_key(&note.simai_shape), just)
                }),
                ..SlideChartNote::default()
            })
            .collect(),
        button_ring_for_touch: None,
        slide_skipping: Some(chart.slide_skipping),
    }
}
