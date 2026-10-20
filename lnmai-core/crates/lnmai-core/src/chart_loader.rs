//! Chart IR consumed by the runtime (`ChartSpec` and note types).
//!
//! Mirrors the data types in `LnmaiCore/ChartLoader.lean`. The runtime
//! `buildGameState` / JSON adapters are ported in M4; this module provides the
//! pure data model used by `Simai.Normalize`.

use crate::areas::{ButtonZone, OuterSlot, SensorArea};
use crate::input_model::{
    ButtonQueueVec, GameState, SensorQueueVec, TouchHoldBodyGroupState, ZoneQueue,
};
use crate::lifecycle::{
    HoldNote, HoldStart, HoldSubState, SlideArea, SlideNote, SlideState, TapFamilyNote, TapNote,
    TapState, TouchNote, TouchState,
};
use crate::natural::Natural;
use crate::scheduler::TouchHoldRebuildAtoms;
pub use crate::simai::slide_tables::SlideAreaSpec;
use crate::storage::{button_zone_storage_order, ButtonVec, SensorVec};
use crate::time::{Duration, TimePoint};
use crate::types::{JudgeGrade, NoteType, SlideKind};
use lnmai_core_verify::touch_hold_body_adapter::{SliceHoldSource, TouchHoldBodyAdapter};
use lnmai_core_verify::touch_hold_rebuild as touch_hold_rebuild_kernel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapChartNote {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    pub is_break: bool,
    pub is_ex: bool,
    pub button_queue_index: Natural,
    pub note_index: Natural,
}

impl Default for TapChartNote {
    fn default() -> Self {
        TapChartNote {
            timing: TimePoint::zero(),
            slot: OuterSlot::S1,
            is_break: false,
            is_ex: false,
            button_queue_index: 0.into(),
            note_index: 0.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldChartNote {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    pub length: Duration,
    pub is_break: bool,
    pub is_ex: bool,
    pub is_touch: bool,
    pub is_classic: Option<bool>,
    pub button_queue_index: Natural,
    pub touch_hold_group_id: Option<Natural>,
    pub touch_hold_group_size: Option<Natural>,
    pub note_index: Natural,
}

impl Default for HoldChartNote {
    fn default() -> Self {
        HoldChartNote {
            timing: TimePoint::zero(),
            slot: OuterSlot::S1,
            length: Duration::zero(),
            is_break: false,
            is_ex: false,
            is_touch: false,
            is_classic: None,
            button_queue_index: 0.into(),
            touch_hold_group_id: None,
            touch_hold_group_size: None,
            note_index: 0.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchHoldChartNote {
    pub timing: TimePoint,
    pub sensor_pos: SensorArea,
    pub length: Duration,
    pub is_break: bool,
    pub is_ex: bool,
    pub source_group_id: Option<Natural>,
    pub source_group_index: Option<Natural>,
    pub source_group_size: Option<Natural>,
    pub touch_queue_index: Natural,
    pub touch_group_id: Option<Natural>,
    pub touch_group_size: Option<Natural>,
    pub touch_hold_group_id: Option<Natural>,
    pub touch_hold_group_size: Option<Natural>,
    pub note_index: Natural,
}

impl Default for TouchHoldChartNote {
    fn default() -> Self {
        TouchHoldChartNote {
            timing: TimePoint::zero(),
            sensor_pos: SensorArea::A1,
            length: Duration::zero(),
            is_break: false,
            is_ex: false,
            source_group_id: None,
            source_group_index: None,
            source_group_size: None,
            touch_queue_index: 0.into(),
            touch_group_id: None,
            touch_group_size: None,
            touch_hold_group_id: None,
            touch_hold_group_size: None,
            note_index: 0.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchChartNote {
    pub timing: TimePoint,
    pub sensor_pos: SensorArea,
    pub is_break: bool,
    pub source_group_id: Option<Natural>,
    pub source_group_index: Option<Natural>,
    pub source_group_size: Option<Natural>,
    pub touch_queue_index: Natural,
    pub touch_group_id: Option<Natural>,
    pub touch_group_size: Option<Natural>,
    pub note_index: Natural,
}

impl Default for TouchChartNote {
    fn default() -> Self {
        TouchChartNote {
            timing: TimePoint::zero(),
            sensor_pos: SensorArea::A1,
            is_break: false,
            source_group_id: None,
            source_group_index: None,
            source_group_size: None,
            touch_queue_index: 0.into(),
            touch_group_id: None,
            touch_group_size: None,
            note_index: 0.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideHeadChartNote {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    pub is_break: bool,
    pub is_ex: bool,
    pub logical_slide_id: Natural,
    pub note_index: Natural,
}

impl Default for SlideHeadChartNote {
    fn default() -> Self {
        SlideHeadChartNote {
            timing: TimePoint::zero(),
            slot: OuterSlot::S1,
            is_break: false,
            is_ex: false,
            logical_slide_id: 0.into(),
            note_index: 0.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideChartNote {
    pub head_timing: TimePoint,
    pub slot: OuterSlot,
    pub length: Duration,
    pub start_timing: TimePoint,
    pub group_start_timing: Option<TimePoint>,
    pub slide_kind: SlideKind,
    pub is_classic: bool,
    pub is_slide_no_head: bool,
    pub is_conn_slide: bool,
    pub parent_note_index: Option<Natural>,
    pub is_group_head: bool,
    pub is_group_end: bool,
    pub parent_finished: bool,
    pub parent_pending_finish: bool,
    pub total_judge_queue_len: Natural,
    pub track_count: Natural,
    pub judge_at: Option<TimePoint>,
    pub is_break: bool,
    pub is_ex: bool,
    pub multiple: Natural,
    pub logical_slide_id: Natural,
    pub note_index: Natural,
    pub judge_queues: Vec<Vec<SlideAreaSpec>>,
    pub debug_simai: Option<(String, String, bool)>,
}

impl Default for SlideChartNote {
    fn default() -> Self {
        SlideChartNote {
            head_timing: TimePoint::zero(),
            slot: OuterSlot::S1,
            length: Duration::zero(),
            start_timing: TimePoint::zero(),
            group_start_timing: None,
            slide_kind: SlideKind::Single,
            is_classic: false,
            is_slide_no_head: false,
            is_conn_slide: false,
            parent_note_index: None,
            is_group_head: false,
            is_group_end: false,
            parent_finished: false,
            parent_pending_finish: false,
            total_judge_queue_len: 0.into(),
            track_count: 0.into(),
            judge_at: None,
            is_break: false,
            is_ex: false,
            multiple: 0.into(),
            logical_slide_id: 0.into(),
            note_index: 0.into(),
            judge_queues: Vec::new(),
            debug_simai: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChartSpec {
    pub taps: Vec<TapChartNote>,
    pub holds: Vec<HoldChartNote>,
    pub touches: Vec<TouchChartNote>,
    pub touch_holds: Vec<TouchHoldChartNote>,
    pub slide_heads: Vec<SlideHeadChartNote>,
    pub slides: Vec<SlideChartNote>,
    pub slide_skipping: Option<bool>,
    pub button_ring_for_touch: Option<bool>,
}

// ---------------------------------------------------------------------------
// Sorting helpers
// ---------------------------------------------------------------------------

fn insert_by_timing<T: Clone>(
    get_timing: &dyn Fn(&T) -> TimePoint,
    item: T,
    items: &[T],
) -> Vec<T> {
    match items.split_first() {
        None => vec![item],
        Some((head, rest)) => {
            if get_timing(&item).to_micros() <= get_timing(head).to_micros() {
                let mut out = vec![item];
                out.extend_from_slice(items);
                out
            } else {
                let mut out = vec![head.clone()];
                out.extend(insert_by_timing(get_timing, item, rest));
                out
            }
        }
    }
}

fn sort_by_timing<T: Clone>(get_timing: &dyn Fn(&T) -> TimePoint, items: &[T]) -> Vec<T> {
    let mut acc: Vec<T> = Vec::new();
    for item in items {
        acc = insert_by_timing(get_timing, item.clone(), &acc);
    }
    acc
}

fn insert_by_nat_key<T: Clone>(get_key: &dyn Fn(&T) -> Natural, item: T, items: &[T]) -> Vec<T> {
    match items.split_first() {
        None => vec![item],
        Some((head, rest)) => {
            if get_key(&item) <= get_key(head) {
                let mut out = vec![item];
                out.extend_from_slice(items);
                out
            } else {
                let mut out = vec![head.clone()];
                out.extend(insert_by_nat_key(get_key, item, rest));
                out
            }
        }
    }
}

fn sort_by_nat_key<T: Clone>(get_key: &dyn Fn(&T) -> Natural, items: &[T]) -> Vec<T> {
    let mut acc: Vec<T> = Vec::new();
    for item in items {
        acc = insert_by_nat_key(get_key, item.clone(), &acc);
    }
    acc
}

fn find_button_queue_index_by_note_index(notes: &[TapChartNote], note_index: Natural) -> Natural {
    notes
        .iter()
        .find(|note| note.note_index == note_index)
        .map(|note| note.button_queue_index.clone())
        .unwrap_or((0).into())
}

// ---------------------------------------------------------------------------
// Note builders
// ---------------------------------------------------------------------------

fn judge_params(
    timing: TimePoint,
    is_break: bool,
    is_ex: bool,
    note_index: Natural,
) -> crate::lifecycle::CommonNoteParams {
    crate::lifecycle::CommonNoteParams {
        judge_timing: timing,
        judge_offset: Duration::zero(),
        is_break,
        is_ex,
        note_index,
    }
}

fn build_tap(note: &TapChartNote) -> TapFamilyNote {
    TapFamilyNote::Tap(TapNote {
        params: judge_params(
            note.timing.clone(),
            note.is_break,
            note.is_ex,
            note.note_index.clone(),
        ),
        lane: note.slot,
        state: TapState::Waiting,
        button_queue_index: note.button_queue_index.clone(),
    })
}

#[allow(dead_code)]
fn build_slide_head(note: &SlideHeadChartNote) -> TapFamilyNote {
    TapFamilyNote::SlideHead(crate::lifecycle::SlideHeadNote {
        params: judge_params(
            note.timing.clone(),
            note.is_break,
            note.is_ex,
            note.note_index.clone(),
        ),
        lane: note.slot,
        state: TapState::Waiting,
        logical_slide_id: note.logical_slide_id.clone(),
        button_queue_index: 0.into(),
    })
}

fn build_hold(note: &HoldChartNote) -> HoldNote {
    HoldNote {
        params: judge_params(
            note.timing.clone(),
            note.is_break,
            note.is_ex,
            note.note_index.clone(),
        ),
        start: HoldStart::Button(note.slot.to_button_zone()),
        state: HoldSubState::HeadWaiting,
        length: note.length.clone(),
        button_queue_index: note.button_queue_index.clone(),
        head_diff: Duration::zero(),
        head_grade: JudgeGrade::Miss,
        player_release_time: Duration::zero(),
        release_ignore_time: Duration::zero(),
        is_classic: note.is_classic.unwrap_or(false),
        is_touch_hold: note.is_touch,
        touch_queue_index: 0.into(),
        touch_group_id: None,
        touch_group_size: 1.into(),
        touch_hold_group_id: note.touch_hold_group_id.clone(),
        touch_hold_group_size: note.touch_hold_group_size.clone().unwrap_or((1).into()),
        touch_hold_group_triggered: false,
    }
}

fn build_touch_hold(note: &TouchHoldChartNote) -> HoldNote {
    HoldNote {
        params: judge_params(
            note.timing.clone(),
            note.is_break,
            note.is_ex,
            note.note_index.clone(),
        ),
        start: HoldStart::Sensor(note.sensor_pos),
        state: HoldSubState::HeadWaiting,
        length: note.length.clone(),
        button_queue_index: 0.into(),
        head_diff: Duration::zero(),
        head_grade: JudgeGrade::Miss,
        player_release_time: Duration::zero(),
        release_ignore_time: Duration::zero(),
        is_classic: false,
        is_touch_hold: true,
        touch_queue_index: note.touch_queue_index.clone(),
        touch_group_id: note.touch_group_id.clone(),
        touch_group_size: note.touch_group_size.clone().unwrap_or((1).into()),
        touch_hold_group_id: note.touch_hold_group_id.clone(),
        touch_hold_group_size: note.touch_hold_group_size.clone().unwrap_or((1).into()),
        touch_hold_group_triggered: false,
    }
}

fn build_touch(note: &TouchChartNote) -> TouchNote {
    TouchNote {
        params: judge_params(
            note.timing.clone(),
            note.is_break,
            false,
            note.note_index.clone(),
        ),
        state: TouchState::Waiting,
        sensor_pos: note.sensor_pos,
        touch_queue_index: note.touch_queue_index.clone(),
        touch_group_id: note.touch_group_id.clone(),
        touch_group_size: note.touch_group_size.clone().unwrap_or((1).into()),
    }
}

fn build_slide_area(spec: &SlideAreaSpec) -> SlideArea {
    SlideArea {
        target_areas: spec.target_areas.clone(),
        policy: spec.policy,
        is_last: spec.is_last,
        is_skippable: spec.is_skippable,
        arrow_progress_when_on: spec.arrow_progress_when_on.clone(),
        arrow_progress_when_finished: spec.arrow_progress_when_finished.clone(),
        was_on: false,
        was_off: false,
        target_was_on: Vec::new(),
        target_was_off: Vec::new(),
    }
}

fn disable_slide_skipping(queues: &[Vec<SlideArea>]) -> Vec<Vec<SlideArea>> {
    queues
        .iter()
        .map(|queue| {
            queue
                .iter()
                .map(|a| SlideArea {
                    is_skippable: false,
                    ..a.clone()
                })
                .collect()
        })
        .collect()
}

fn apply_single_track_conn_rules(note: &SlideChartNote, queue: &[SlideArea]) -> Vec<SlideArea> {
    if !note.is_conn_slide {
        queue.to_vec()
    } else if note.total_judge_queue_len < 4 {
        match queue {
            [] => Vec::new(),
            [first, second, rest @ ..] => {
                let mut out = vec![SlideArea {
                    is_skippable: note.is_group_head,
                    ..first.clone()
                }];
                out.push(SlideArea {
                    is_skippable: note.is_group_end,
                    ..second.clone()
                });
                out.extend_from_slice(rest);
                out
            }
            only => only.to_vec(),
        }
    } else {
        queue
            .iter()
            .map(|a| SlideArea {
                is_skippable: true,
                ..a.clone()
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Touch-hold components
// ---------------------------------------------------------------------------

fn touch_hold_neighbors(area: SensorArea) -> Vec<SensorArea> {
    use SensorArea::*;
    match area {
        A1 => vec![D1, D2, E1, E2, B1],
        A2 => vec![D2, D3, E2, E3, B2],
        A3 => vec![D3, D4, E3, E4, B3],
        A4 => vec![D4, D5, E4, E5, B4],
        A5 => vec![D5, D6, E5, E6, B5],
        A6 => vec![D6, D7, E6, E7, B6],
        A7 => vec![D7, D8, E7, E8, B7],
        A8 => vec![D8, D1, E8, E1, B8],
        D1 => vec![A1, A8, E1],
        D2 => vec![A2, A1, E2],
        D3 => vec![A3, A2, E3],
        D4 => vec![A4, A3, E4],
        D5 => vec![A5, A4, E5],
        D6 => vec![A6, A5, E6],
        D7 => vec![A7, A6, E7],
        D8 => vec![A8, A7, E8],
        E1 => vec![D1, A1, A8, B1, B8],
        E2 => vec![D2, A2, A1, B2, B1],
        E3 => vec![D3, A3, A2, B3, B2],
        E4 => vec![D4, A4, A3, B4, B3],
        E5 => vec![D5, A5, A4, B5, B4],
        E6 => vec![D6, A6, A5, B6, B5],
        E7 => vec![D7, A7, A6, B7, B6],
        E8 => vec![D8, A8, A7, B8, B7],
        B1 => vec![E1, E2, B8, B2, A1, C],
        B2 => vec![E2, E3, B1, B3, A2, C],
        B3 => vec![E3, E4, B2, B4, A3, C],
        B4 => vec![E4, E5, B3, B5, A4, C],
        B5 => vec![E5, E6, B4, B6, A5, C],
        B6 => vec![E6, E7, B5, B7, A6, C],
        B7 => vec![E7, E8, B6, B8, A7, C],
        B8 => vec![E8, E1, B7, B1, A8, C],
        C => vec![B1, B2, B3, B4, B5, B6, B7, B8],
    }
}

fn contains_area(items: &[SensorArea], value: SensorArea) -> bool {
    items.contains(&value)
}

fn collect_touch_hold_component_fuel(
    fuel: usize,
    pending: &[SensorArea],
    remaining: &[SensorArea],
    component: &mut Vec<SensorArea>,
) {
    let (area, rest) = match pending.split_first() {
        None => return,
        Some((a, r)) => (*a, r),
    };
    if fuel == 0 {
        return;
    }
    if contains_area(component, area) {
        collect_touch_hold_component_fuel(fuel - 1, rest, remaining, component);
    } else {
        let neighbors = touch_hold_neighbors(area);
        let newly_reached: Vec<SensorArea> = remaining
            .iter()
            .filter(|c| contains_area(&neighbors, **c))
            .cloned()
            .collect();
        let remaining2: Vec<SensorArea> = remaining
            .iter()
            .filter(|c| **c != area && !contains_area(&neighbors, **c))
            .cloned()
            .collect();
        component.insert(0, area);
        let mut next: Vec<SensorArea> = rest.to_vec();
        next.extend(newly_reached);
        collect_touch_hold_component_fuel(fuel - 1, &next, &remaining2, component);
    }
}

pub fn collect_touch_hold_component(
    pending: &[SensorArea],
    remaining: &[SensorArea],
    component: &[SensorArea],
) -> Vec<SensorArea> {
    let mut component = component.to_vec();
    let fuel = pending.len() + remaining.len() + 1;
    collect_touch_hold_component_fuel(fuel, pending, remaining, &mut component);
    component
}

fn assign_touch_hold_groups_for_batch(notes: &[TouchHoldChartNote]) -> Vec<TouchHoldChartNote> {
    let mut sensor_types: Vec<SensorArea> = Vec::new();
    for note in notes {
        if !contains_area(&sensor_types, note.sensor_pos) {
            sensor_types.insert(0, note.sensor_pos);
        }
    }
    let mut acc = notes.to_vec();
    let mut remaining = sensor_types;
    let mut group_id = Natural::from(0);
    while let Some(area) = remaining.first().cloned() {
        let component = collect_touch_hold_component(&[area], &remaining, &[]);
        let component_size = notes
            .iter()
            .filter(|n| contains_area(&component, n.sensor_pos))
            .count();
        for note in acc.iter_mut() {
            if contains_area(&component, note.sensor_pos) {
                note.touch_hold_group_id = Some((group_id).clone());
                note.touch_hold_group_size = Some((component_size).into());
            }
        }
        remaining = remaining
            .iter()
            .filter(|c| **c != area && !contains_area(&component, **c))
            .cloned()
            .collect();
        group_id += 1;
    }
    acc
}

fn assign_touch_groups_for_batch(notes: &[TouchChartNote]) -> Vec<TouchChartNote> {
    let mut sensor_types: Vec<SensorArea> = Vec::new();
    for note in notes {
        if !contains_area(&sensor_types, note.sensor_pos) {
            sensor_types.insert(0, note.sensor_pos);
        }
    }
    let mut acc = notes.to_vec();
    let mut remaining = sensor_types;
    let mut group_id = Natural::from(0);
    while let Some(area) = remaining.first().cloned() {
        let component = collect_touch_hold_component(&[area], &remaining, &[]);
        let component_size = notes
            .iter()
            .filter(|n| contains_area(&component, n.sensor_pos))
            .count();
        for note in acc.iter_mut() {
            if contains_area(&component, note.sensor_pos) {
                note.touch_group_id = Some((group_id).clone());
                note.touch_group_size = Some((component_size).into());
            }
        }
        remaining = remaining
            .iter()
            .filter(|c| **c != area && !contains_area(&component, **c))
            .cloned()
            .collect();
        group_id += 1;
    }
    acc
}

fn is_each_source_group_id(
    source_group_id: Option<Natural>,
    source_group_size: Option<Natural>,
) -> Option<Natural> {
    match (source_group_id, source_group_size) {
        (Some(group_id), Some(size)) => {
            if size > 1 {
                Some(group_id)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn assign_touch_groups_by_source_group(
    touches: &[TouchChartNote],
    touch_holds: &[TouchHoldChartNote],
) -> (Vec<TouchChartNote>, Vec<TouchHoldChartNote>) {
    let mut source_group_ids: Vec<Natural> = Vec::new();
    for note in touches {
        if let Some(gid) =
            is_each_source_group_id(note.source_group_id.clone(), note.source_group_size.clone())
        {
            if !source_group_ids.contains(&gid) {
                source_group_ids.push(gid);
            }
        }
    }
    for note in touch_holds {
        if let Some(gid) =
            is_each_source_group_id(note.source_group_id.clone(), note.source_group_size.clone())
        {
            if !source_group_ids.contains(&gid) {
                source_group_ids.push(gid);
            }
        }
    }
    let mut touch_acc = touches.to_vec();
    let mut touch_hold_acc = touch_holds.to_vec();
    let mut next_touch_group_id = Natural::from(0);
    let mut next_touch_hold_group_id = Natural::from(0);

    for source_group_id in &source_group_ids {
        let touch_batch: Vec<TouchChartNote> = touches
            .iter()
            .filter(|n| {
                is_each_source_group_id(n.source_group_id.clone(), n.source_group_size.clone())
                    == Some(source_group_id.clone())
            })
            .cloned()
            .collect();
        let touch_hold_batch: Vec<TouchHoldChartNote> = touch_holds
            .iter()
            .filter(|n| {
                is_each_source_group_id(n.source_group_id.clone(), n.source_group_size.clone())
                    == Some(source_group_id.clone())
            })
            .cloned()
            .collect();
        let mut combined: Vec<TouchChartNote> = touch_batch.clone();
        combined.extend(touch_hold_batch.iter().map(|note| TouchChartNote {
            timing: note.timing.clone(),
            sensor_pos: note.sensor_pos,
            is_break: note.is_break,
            source_group_id: note.source_group_id.clone(),
            source_group_index: note.source_group_index.clone(),
            source_group_size: note.source_group_size.clone(),
            note_index: note.note_index.clone(),
            ..TouchChartNote::default()
        }));
        let combined_grouped = assign_touch_groups_for_batch(&combined);
        let touch_grouped: Vec<TouchChartNote> = combined_grouped
            .iter()
            .filter(|note| {
                touch_batch
                    .iter()
                    .any(|orig| orig.note_index == note.note_index)
            })
            .cloned()
            .collect();
        let touch_hold_metadata: Vec<(Natural, Option<Natural>, Option<Natural>)> =
            combined_grouped
                .iter()
                .filter(|note| {
                    touch_hold_batch
                        .iter()
                        .any(|orig| orig.note_index == note.note_index)
                })
                .map(|note| {
                    (
                        note.note_index.clone(),
                        note.touch_group_id.clone(),
                        note.touch_group_size.clone(),
                    )
                })
                .collect();
        let touch_hold_body_grouped = assign_touch_hold_groups_for_batch(&touch_hold_batch);
        let touch_group_base = next_touch_group_id;
        let touch_hold_group_base = next_touch_hold_group_id;

        let mut touch_component_ids: Vec<Natural> = touch_grouped
            .iter()
            .filter_map(|n| n.touch_group_id.clone())
            .collect();
        touch_component_ids.sort_unstable();
        touch_component_ids.dedup();
        let touch_component_count = touch_component_ids.len();
        let mut th_component_ids: Vec<Natural> = touch_hold_body_grouped
            .iter()
            .filter_map(|n| n.touch_hold_group_id.clone())
            .collect();
        th_component_ids.sort_unstable();
        th_component_ids.dedup();
        let touch_hold_component_count = th_component_ids.len();

        let touch_grouped2: Vec<TouchChartNote> = touch_grouped
            .iter()
            .map(|note| match note.touch_group_id.clone() {
                Some(gid) => TouchChartNote {
                    touch_group_id: Some(touch_group_base.clone() + gid),
                    ..note.clone()
                },
                None => note.clone(),
            })
            .collect();
        let touch_hold_with_touch_group: Vec<TouchHoldChartNote> = touch_hold_batch
            .iter()
            .map(|note| {
                match touch_hold_metadata
                    .iter()
                    .find(|(idx, _, _)| *idx == note.note_index)
                {
                    Some((_, gid, size)) => TouchHoldChartNote {
                        touch_group_id: gid.clone().map(|id| touch_group_base.clone() + id),
                        touch_group_size: size.clone(),
                        ..note.clone()
                    },
                    None => note.clone(),
                }
            })
            .collect();
        let touch_hold_grouped2: Vec<TouchHoldChartNote> =
            assign_touch_hold_groups_for_batch(&touch_hold_with_touch_group)
                .iter()
                .map(|note| match note.touch_hold_group_id.clone() {
                    Some(gid) => TouchHoldChartNote {
                        touch_hold_group_id: Some(touch_hold_group_base.clone() + gid),
                        ..note.clone()
                    },
                    None => note.clone(),
                })
                .collect();

        for note in touch_acc.iter_mut() {
            if let Some(grouped) = touch_grouped2
                .iter()
                .find(|g| g.note_index == note.note_index)
            {
                *note = grouped.clone();
            }
        }
        for note in touch_hold_acc.iter_mut() {
            if let Some(grouped) = touch_hold_grouped2
                .iter()
                .find(|g| g.note_index == note.note_index)
            {
                *note = grouped.clone();
            }
        }
        next_touch_group_id = touch_group_base + touch_component_count;
        next_touch_hold_group_id = touch_hold_group_base + touch_hold_component_count;
    }
    (touch_acc, touch_hold_acc)
}

fn unique_areas_from_touches(
    touches: &[TouchChartNote],
    touch_holds: &[TouchHoldChartNote],
) -> Vec<SensorArea> {
    let mut areas: Vec<SensorArea> = Vec::new();
    for note in touches {
        if !contains_area(&areas, note.sensor_pos) {
            areas.insert(0, note.sensor_pos);
        }
    }
    for note in touch_holds {
        if !contains_area(&areas, note.sensor_pos) {
            areas.insert(0, note.sensor_pos);
        }
    }
    areas
}

fn merge_assign_touch(
    index: Natural,
    ts: &[TouchChartNote],
    hs: &[TouchHoldChartNote],
    acc_t: Vec<TouchChartNote>,
    acc_h: Vec<TouchHoldChartNote>,
) -> (Vec<TouchChartNote>, Vec<TouchHoldChartNote>) {
    match (ts.split_first(), hs.split_first()) {
        (None, None) => (acc_t, acc_h),
        (Some((t, ts_rest)), None) => merge_assign_touch(
            index.clone() + 1,
            ts_rest,
            &[],
            {
                let mut a = acc_t.clone();
                a.insert(
                    0,
                    TouchChartNote {
                        touch_queue_index: index,
                        ..t.clone()
                    },
                );
                a
            },
            acc_h,
        ),
        (None, Some((h, hs_rest))) => merge_assign_touch(index.clone() + 1, &[], hs_rest, acc_t, {
            let mut a = acc_h.clone();
            a.insert(
                0,
                TouchHoldChartNote {
                    touch_queue_index: index,
                    ..h.clone()
                },
            );
            a
        }),
        (Some((t, ts_rest)), Some((h, hs_rest))) => {
            if t.timing.to_micros() <= h.timing.to_micros() {
                merge_assign_touch(
                    index.clone() + 1,
                    ts_rest,
                    hs,
                    {
                        let mut a = acc_t.clone();
                        a.insert(
                            0,
                            TouchChartNote {
                                touch_queue_index: index,
                                ..t.clone()
                            },
                        );
                        a
                    },
                    acc_h,
                )
            } else {
                merge_assign_touch(index.clone() + 1, ts, hs_rest, acc_t, {
                    let mut a = acc_h.clone();
                    a.insert(
                        0,
                        TouchHoldChartNote {
                            touch_queue_index: index,
                            ..h.clone()
                        },
                    );
                    a
                })
            }
        }
    }
}

fn assign_shared_touch_queue_indices(
    touches: &[TouchChartNote],
    touch_holds: &[TouchHoldChartNote],
) -> (Vec<TouchChartNote>, Vec<TouchHoldChartNote>) {
    let all_areas = unique_areas_from_touches(touches, touch_holds);
    let mut acc_t: Vec<TouchChartNote> = Vec::new();
    let mut acc_h: Vec<TouchHoldChartNote> = Vec::new();
    for area in &all_areas {
        let ts = sort_by_timing(
            &|n: &TouchChartNote| n.timing.clone(),
            &touches
                .iter()
                .filter(|n| n.sensor_pos == *area)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let hs = sort_by_timing(
            &|n: &TouchHoldChartNote| n.timing.clone(),
            &touch_holds
                .iter()
                .filter(|n| n.sensor_pos == *area)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let (acc_t2, acc_h2) = merge_assign_touch((0).into(), &ts, &hs, acc_t, acc_h);
        acc_t = acc_t2;
        acc_h = acc_h2;
    }
    acc_t.reverse();
    acc_h.reverse();
    (acc_t, acc_h)
}

fn merge_assign_button(
    index: Natural,
    ts: &[TapChartNote],
    hs: &[HoldChartNote],
    acc_t: Vec<TapChartNote>,
    acc_h: Vec<HoldChartNote>,
) -> (Vec<TapChartNote>, Vec<HoldChartNote>) {
    match (ts.split_first(), hs.split_first()) {
        (None, None) => (acc_t, acc_h),
        (Some((t, ts_rest)), None) => merge_assign_button(
            index.clone() + 1,
            ts_rest,
            &[],
            {
                let mut a = acc_t.clone();
                a.insert(
                    0,
                    TapChartNote {
                        button_queue_index: index,
                        ..t.clone()
                    },
                );
                a
            },
            acc_h,
        ),
        (None, Some((h, hs_rest))) => {
            merge_assign_button(index.clone() + 1, &[], hs_rest, acc_t, {
                let mut a = acc_h.clone();
                a.insert(
                    0,
                    HoldChartNote {
                        button_queue_index: index,
                        ..h.clone()
                    },
                );
                a
            })
        }
        (Some((t, ts_rest)), Some((h, hs_rest))) => {
            if t.timing.to_micros() <= h.timing.to_micros() {
                merge_assign_button(
                    index.clone() + 1,
                    ts_rest,
                    hs,
                    {
                        let mut a = acc_t.clone();
                        a.insert(
                            0,
                            TapChartNote {
                                button_queue_index: index,
                                ..t.clone()
                            },
                        );
                        a
                    },
                    acc_h,
                )
            } else {
                merge_assign_button(index.clone() + 1, ts, hs_rest, acc_t, {
                    let mut a = acc_h.clone();
                    a.insert(
                        0,
                        HoldChartNote {
                            button_queue_index: index,
                            ..h.clone()
                        },
                    );
                    a
                })
            }
        }
    }
}

fn assign_shared_button_queue_indices(
    taps: &[TapChartNote],
    holds: &[HoldChartNote],
) -> (Vec<TapChartNote>, Vec<HoldChartNote>) {
    let mut all_zones: Vec<ButtonZone> = Vec::new();
    for note in taps {
        let zone = note.slot.to_button_zone();
        if !all_zones.contains(&zone) {
            all_zones.insert(0, zone);
        }
    }
    for note in holds {
        let zone = note.slot.to_button_zone();
        if !all_zones.contains(&zone) {
            all_zones.insert(0, zone);
        }
    }
    let mut acc_t: Vec<TapChartNote> = Vec::new();
    let mut acc_h: Vec<HoldChartNote> = Vec::new();
    for zone in &all_zones {
        let ts = sort_by_timing(
            &|n: &TapChartNote| n.timing.clone(),
            &taps
                .iter()
                .filter(|n| n.slot.to_button_zone() == *zone)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let hs = sort_by_timing(
            &|n: &HoldChartNote| n.timing.clone(),
            &holds
                .iter()
                .filter(|n| n.slot.to_button_zone() == *zone)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let (acc_t2, acc_h2) = merge_assign_button((0).into(), &ts, &hs, acc_t, acc_h);
        acc_t = acc_t2;
        acc_h = acc_h2;
    }
    acc_t.reverse();
    acc_h.reverse();
    (acc_t, acc_h)
}

fn build_slide(slide_skipping: bool, note: &SlideChartNote) -> SlideNote {
    let queues: Vec<Vec<SlideArea>> = note
        .judge_queues
        .iter()
        .map(|queue| queue.iter().map(build_slide_area).collect())
        .collect();
    let judge_queues = if !slide_skipping {
        disable_slide_skipping(&queues)
    } else {
        match queues.as_slice() {
            [queue] => vec![apply_single_track_conn_rules(note, queue)],
            _ => queues,
        }
    };
    let judge_timing = note.judge_at.clone().unwrap_or(note.head_timing.clone());
    let max_queue_length = judge_queues.iter().map(|q| q.len()).max().unwrap_or(0);
    SlideNote {
        params: judge_params(
            judge_timing,
            note.is_break,
            note.is_ex,
            note.note_index.clone(),
        ),
        lane: note.slot,
        state: SlideState::Waiting,
        length: note.length.clone(),
        head_timing: note.head_timing.clone(),
        start_timing: note.start_timing.clone(),
        group_start_timing: note.group_start_timing.clone(),
        slide_kind: note.slide_kind,
        is_classic: note.is_classic,
        is_conn_slide: note.is_conn_slide,
        parent_note_index: note.parent_note_index.clone(),
        is_group_part_head: note.is_group_head,
        is_group_part_end: note.is_group_end,
        parent_finished: note.parent_finished,
        parent_pending_finish: note.parent_pending_finish,
        initial_queue_remaining: (max_queue_length).into(),
        total_judge_queue_len: note.total_judge_queue_len.clone(),
        track_count: note.track_count.clone(),
        is_checkable: false,
        slide_sound_played: false,
        multiple: note.multiple.clone().max((1).into()),
        judge_queues,
    }
}

fn touch_hold_body_group_states_from_holds(
    holds: &[(SensorArea, HoldNote)],
) -> Vec<TouchHoldBodyGroupState> {
    touch_hold_rebuild_kernel::rebuild(
        &holds,
        TouchHoldBodyAdapter {
            ops: SliceHoldSource {
                fields: TouchHoldRebuildAtoms,
            },
        },
    )
}

#[derive(Debug, Clone, Default)]
pub struct ChartScoreTotals {
    pub total_base: Natural,
    pub total_extra: Natural,
    pub note_count: Natural,
}

fn add_score_total(
    totals: ChartScoreTotals,
    kind: NoteType,
    is_break: bool,
    multiple: Natural,
) -> ChartScoreTotals {
    let multiple = multiple.max(1.into());
    let score_kind = if is_break { NoteType::Break } else { kind };
    ChartScoreTotals {
        total_base: totals.total_base + Natural::from(score_kind.base_score()) * &multiple,
        total_extra: totals.total_extra + Natural::from(score_kind.extra_score()) * &multiple,
        note_count: totals.note_count + multiple,
    }
}

pub fn chart_score_totals(chart: &ChartSpec) -> ChartScoreTotals {
    let mut totals = ChartScoreTotals::default();
    for note in &chart.taps {
        totals = add_score_total(totals, NoteType::Tap, note.is_break, (1).into());
    }
    for note in &chart.slide_heads {
        totals = add_score_total(totals, NoteType::Tap, note.is_break, (1).into());
    }
    for note in &chart.holds {
        totals = add_score_total(totals, NoteType::Hold, note.is_break, (1).into());
    }
    for note in &chart.touch_holds {
        totals = add_score_total(totals, NoteType::Hold, note.is_break, (1).into());
    }
    for note in &chart.touches {
        totals = add_score_total(totals, NoteType::Touch, note.is_break, (1).into());
    }
    for note in &chart.slides {
        if note.is_conn_slide && !note.is_group_end {
            continue;
        }
        totals = add_score_total(
            totals,
            NoteType::Slide,
            note.is_break,
            note.multiple.clone(),
        );
    }
    totals
}

/// `buildGameState`.
pub fn build_game_state(chart: &ChartSpec) -> GameState {
    let score_totals = chart_score_totals(chart);

    let mut tap_family_heads: Vec<TapChartNote> = chart.taps.clone();
    tap_family_heads.extend(chart.slide_heads.iter().map(|note| TapChartNote {
        timing: note.timing.clone(),
        slot: note.slot,
        is_break: note.is_break,
        is_ex: note.is_ex,
        note_index: note.note_index.clone(),
        ..TapChartNote::default()
    }));

    let (taps_with_indices, holds_with_indices) =
        assign_shared_button_queue_indices(&tap_family_heads, &chart.holds);

    let taps_with_indices_owned = taps_with_indices.clone();
    let tap_queues: ButtonQueueVec<TapFamilyNote> = ButtonVec::of_fn(|zone| {
        let taps: Vec<TapFamilyNote> = sort_by_nat_key(
            &|n: &TapChartNote| n.button_queue_index.clone(),
            &taps_with_indices_owned
                .iter()
                .filter(|note| {
                    note.slot.to_button_zone() == zone
                        && !chart
                            .slide_heads
                            .iter()
                            .any(|head| head.note_index == note.note_index)
                })
                .cloned()
                .collect::<Vec<_>>(),
        )
        .iter()
        .map(build_tap)
        .collect();
        let slide_heads: Vec<TapFamilyNote> = sort_by_nat_key(
            &|n: &SlideHeadChartNote| {
                find_button_queue_index_by_note_index(
                    &taps_with_indices_owned,
                    n.note_index.clone(),
                )
            },
            &chart
                .slide_heads
                .iter()
                .filter(|note| note.slot.to_button_zone() == zone)
                .cloned()
                .collect::<Vec<_>>(),
        )
        .iter()
        .map(|note| {
            let button_queue_index = find_button_queue_index_by_note_index(
                &taps_with_indices_owned,
                note.note_index.clone(),
            );
            TapFamilyNote::SlideHead(crate::lifecycle::SlideHeadNote {
                params: judge_params(
                    note.timing.clone(),
                    note.is_break,
                    note.is_ex,
                    note.note_index.clone(),
                ),
                lane: note.slot,
                state: TapState::Waiting,
                logical_slide_id: note.logical_slide_id.clone(),
                button_queue_index,
            })
        })
        .collect();
        let mut notes = taps;
        notes.extend(slide_heads);
        let notes = sort_by_nat_key(&|n: &TapFamilyNote| n.button_queue_index(), &notes);
        ZoneQueue {
            notes,
            current_index: (0).into(),
        }
    });

    let hold_queues: ButtonQueueVec<HoldNote> = ButtonVec::of_fn(|zone| {
        let notes: Vec<HoldNote> = sort_by_nat_key(
            &|n: &HoldChartNote| n.button_queue_index.clone(),
            &holds_with_indices
                .iter()
                .filter(|note| note.slot.to_button_zone() == zone)
                .cloned()
                .collect::<Vec<_>>(),
        )
        .iter()
        .map(build_hold)
        .collect();
        ZoneQueue {
            notes,
            current_index: (0).into(),
        }
    });

    let (touch_grouped0, touch_hold0) =
        assign_touch_groups_by_source_group(&chart.touches, &chart.touch_holds);
    let (touch_grouped, touch_hold_queued) =
        assign_shared_touch_queue_indices(&touch_grouped0, &touch_hold0);
    let touch_hold_notes: Vec<TouchHoldChartNote> = touch_hold_queued
        .iter()
        .map(|note| {
            match touch_grouped.iter().find(|t| {
                t.touch_queue_index == note.touch_queue_index && t.sensor_pos == note.sensor_pos
            }) {
                Some(touch) => TouchHoldChartNote {
                    touch_group_id: touch.touch_group_id.clone(),
                    touch_group_size: touch.touch_group_size.clone(),
                    ..note.clone()
                },
                None => note.clone(),
            }
        })
        .collect();

    let touch_hold_queues: SensorQueueVec<HoldNote> = SensorVec::of_fn(|area| {
        let notes: Vec<HoldNote> = sort_by_nat_key(
            &|n: &TouchHoldChartNote| n.touch_queue_index.clone(),
            &touch_hold_notes
                .iter()
                .filter(|note| note.sensor_pos == area)
                .cloned()
                .collect::<Vec<_>>(),
        )
        .iter()
        .map(build_touch_hold)
        .collect();
        ZoneQueue {
            notes,
            current_index: (0).into(),
        }
    });

    let touch_queues: SensorQueueVec<TouchNote> = SensorVec::of_fn(|area| {
        let notes: Vec<TouchNote> = sort_by_nat_key(
            &|n: &TouchChartNote| n.touch_queue_index.clone(),
            &touch_grouped
                .iter()
                .filter(|note| note.sensor_pos == area)
                .cloned()
                .collect::<Vec<_>>(),
        )
        .iter()
        .map(build_touch)
        .collect();
        ZoneQueue {
            notes,
            current_index: (0).into(),
        }
    });

    let mut active_holds: Vec<(ButtonZone, HoldNote)> = Vec::new();
    for zone in button_zone_storage_order() {
        let queue = hold_queues.get_d(zone, ZoneQueue::default());
        for note in queue.notes {
            active_holds.push((zone, note));
        }
    }

    let mut active_touch_holds: Vec<(SensorArea, HoldNote)> = Vec::new();
    for (area, queue) in touch_hold_queues.entries() {
        for note in queue.notes {
            active_touch_holds.push((area, note));
        }
    }

    let touch_hold_group_states = touch_hold_body_group_states_from_holds(&active_touch_holds);

    let slides: Vec<SlideNote> = chart
        .slides
        .iter()
        .map(|note| build_slide(chart.slide_skipping.unwrap_or(true), note))
        .collect();

    GameState {
        tap_queues,
        hold_queues,
        touch_hold_queues,
        touch_queues,
        slides,
        active_holds,
        active_touch_holds,
        touch_hold_group_states,
        score: crate::runtime_score::RuntimeScoreState {
            total_base: score_totals.total_base,
            total_extra: score_totals.total_extra,
            max_dx_score: score_totals.note_count * 3u32,
            ..crate::runtime_score::RuntimeScoreState::default()
        },
        button_ring_for_touch: chart.button_ring_for_touch.unwrap_or(false),
        ..GameState::with_field_defaults()
    }
}

/// `ChartLoader.parseChartJson`, including Lean's field/default error rules.
pub fn parse_chart_json(json: &serde_json::Value) -> Result<ChartSpec, String> {
    use crate::json_decode::Decode;
    ChartSpec::decode(json)
}

/// `ChartLoader.parseChartJsonString`, using the Lean-compatible JSON parser.
pub fn parse_chart_json_string(content: &str) -> Result<ChartSpec, String> {
    crate::json_decode::from_str(content)
}

/// File errors belong to `IO`; JSON/chart errors remain in the inner result,
/// matching `IO (Except String ChartSpec)` in Lean.
pub fn load_chart_file(
    path: impl AsRef<std::path::Path>,
) -> std::io::Result<Result<ChartSpec, String>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_chart_json_string(&content))
}

#[cfg(test)]
mod loader_tests {
    use super::*;
    use crate::simai::frontend::frontend_lowered_chart;

    const MAIDATA: &str = "&title=Test\n&wholebpm=120\n&first=0\n&inote_1=\n(120)\n1,2,3,\n";

    #[test]
    fn build_game_state_from_chart() {
        let spec: ChartSpec = frontend_lowered_chart(MAIDATA, (1).into()).unwrap();
        let state = build_game_state(&spec);
        assert_eq!(state.score.total_base, Natural::from(1500u32)); // 3 taps * 500
        assert_eq!(state.score.max_dx_score, Natural::from(9u32)); // 3 notes * 3
        let total_queued: usize = (0..8)
            .map(|i| {
                let zone = ButtonZone::of_index(i).unwrap();
                state
                    .tap_queues
                    .get_d(zone, ZoneQueue::default())
                    .notes
                    .len()
            })
            .sum();
        assert_eq!(total_queued, 3);
    }
}
