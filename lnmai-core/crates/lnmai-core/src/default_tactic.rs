//! Default autoplay tactic derived from a lowered chart.
//!
//! Port of `LnmaiCore/Proofs/Runtime.lean`:
//! `chartTimingSkeleton`, `resolveDefaultTimingSkeleton` and
//! `defaultTacticFromChart`. Produces the same `TimedInputEvent` list the Lean
//! core uses for autoplay (button clicks/holds for taps/holds, sensor
//! clicks/holds for touches and slides).

use crate::areas::{ButtonZone, SensorArea};
use crate::chart_loader::{ChartSpec, SlideChartNote, SlideHeadChartNote};
use crate::constants::FRAME_LENGTH;
use crate::input_model::TimedInputEvent;
use crate::natural::Natural;
use crate::simai::slide_tables::SlideAreaSpec;
use crate::time::{Duration, TimePoint};
use crate::types::SlideKind;

fn tp_add(time: TimePoint, delta: Duration) -> TimePoint {
    TimePoint::from_micros(time.micros + delta.micros)
}

/// `TOUCH_PANEL_OFFSET` is zero, so sensor input time equals semantic time.
fn sensor_input_time(time: TimePoint) -> TimePoint {
    time
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct SlideTimingSkeleton {
    note_index: Natural,
    head_semantic_time: TimePoint,
    head_input_time: TimePoint,
    has_head_input: bool,
    start_semantic_time: TimePoint,
    start_input_time: TimePoint,
    end_semantic_time: TimePoint,
    end_input_time: TimePoint,
    head_zone: ButtonZone,
    path_steps: Vec<Vec<SensorArea>>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum NoteTimingSkeleton {
    Tap {
        note_index: Natural,
        semantic_time: TimePoint,
        input_time: TimePoint,
        zone: ButtonZone,
    },
    Hold {
        note_index: Natural,
        semantic_time: TimePoint,
        input_time: TimePoint,
        release_input_time: TimePoint,
        zone: ButtonZone,
    },
    Touch {
        note_index: Natural,
        semantic_time: TimePoint,
        input_time: TimePoint,
        area: SensorArea,
    },
    TouchHold {
        note_index: Natural,
        semantic_time: TimePoint,
        input_time: TimePoint,
        release_input_time: TimePoint,
        area: SensorArea,
    },
    Slide(SlideTimingSkeleton),
}

impl NoteTimingSkeleton {
    fn note_index(&self) -> Natural {
        match self {
            Self::Tap { note_index, .. }
            | Self::Hold { note_index, .. }
            | Self::Touch { note_index, .. }
            | Self::TouchHold { note_index, .. } => note_index.clone(),
            Self::Slide(s) => s.note_index.clone(),
        }
    }

    fn semantic_time(&self) -> TimePoint {
        match self {
            NoteTimingSkeleton::Tap { semantic_time, .. }
            | NoteTimingSkeleton::Hold { semantic_time, .. }
            | NoteTimingSkeleton::Touch { semantic_time, .. }
            | NoteTimingSkeleton::TouchHold { semantic_time, .. } => semantic_time.clone(),
            NoteTimingSkeleton::Slide(spec) => spec.head_semantic_time.clone(),
        }
    }
}

fn choose_slide_step_areas(fallback: SensorArea, step: &SlideAreaSpec) -> Vec<SensorArea> {
    if step.target_areas.is_empty() {
        vec![fallback]
    } else {
        step.target_areas.clone()
    }
}

/// Stable `eraseDups`: keep the first occurrence of each area.
fn merge_sensor_areas(lhs: &[SensorArea], rhs: &[SensorArea]) -> Vec<SensorArea> {
    let mut out: Vec<SensorArea> = Vec::with_capacity(lhs.len() + rhs.len());
    for area in lhs.iter().chain(rhs.iter()) {
        if !out.contains(area) {
            out.push(*area);
        }
    }
    out
}

fn slide_body_exists(slides: &[SlideChartNote], logical_slide_id: Natural) -> bool {
    slides
        .iter()
        .any(|s| s.logical_slide_id == logical_slide_id)
}

fn find_slide_head(
    heads: &[SlideHeadChartNote],
    logical_slide_id: Natural,
) -> Option<&SlideHeadChartNote> {
    heads
        .iter()
        .find(|h| h.logical_slide_id == logical_slide_id)
}

fn slide_representative_path_steps(note: &SlideChartNote) -> Vec<Vec<SensorArea>> {
    let fallback = note.slot.to_outer_sensor_area();
    match note.slide_kind {
        SlideKind::Wifi => {
            let max_len = note.judge_queues.iter().map(|q| q.len()).max().unwrap_or(0);
            (0..max_len)
                .map(|index| {
                    note.judge_queues
                        .iter()
                        .fold(Vec::new(), |acc, queue| match queue.get(index) {
                            Some(step) => {
                                merge_sensor_areas(&acc, &choose_slide_step_areas(fallback, step))
                            }
                            None => acc,
                        })
                })
                .collect()
        }
        _ => match note.judge_queues.first() {
            Some(queue) => queue
                .iter()
                .map(|step| choose_slide_step_areas(fallback, step))
                .collect(),
            None => Vec::new(),
        },
    }
}

/// Port of `chartTimingSkeleton`: one skeleton entry per tap/hold/touch/slide,
/// stably sorted by semantic time.
fn chart_timing_skeleton(chart: &ChartSpec) -> Vec<NoteTimingSkeleton> {
    let mut entries: Vec<NoteTimingSkeleton> = Vec::new();

    for note in &chart.taps {
        entries.push(NoteTimingSkeleton::Tap {
            note_index: note.note_index.clone(),
            semantic_time: note.timing.clone(),
            input_time: note.timing.clone(),
            zone: note.slot.to_button_zone(),
        });
    }

    for note in chart
        .slide_heads
        .iter()
        .filter(|h| !slide_body_exists(&chart.slides, h.logical_slide_id.clone()))
    {
        entries.push(NoteTimingSkeleton::Tap {
            note_index: note.note_index.clone(),
            semantic_time: note.timing.clone(),
            input_time: note.timing.clone(),
            zone: note.slot.to_button_zone(),
        });
    }

    for note in &chart.holds {
        entries.push(NoteTimingSkeleton::Hold {
            note_index: note.note_index.clone(),
            semantic_time: note.timing.clone(),
            input_time: note.timing.clone(),
            release_input_time: tp_add(note.timing.clone(), note.length.clone()),
            zone: note.slot.to_button_zone(),
        });
    }

    for note in &chart.touches {
        entries.push(NoteTimingSkeleton::Touch {
            note_index: note.note_index.clone(),
            semantic_time: note.timing.clone(),
            input_time: sensor_input_time(note.timing.clone()),
            area: note.sensor_pos,
        });
    }

    for note in &chart.touch_holds {
        entries.push(NoteTimingSkeleton::TouchHold {
            note_index: note.note_index.clone(),
            semantic_time: note.timing.clone(),
            input_time: sensor_input_time(note.timing.clone()),
            release_input_time: sensor_input_time(tp_add(note.timing.clone(), note.length.clone())),
            area: note.sensor_pos,
        });
    }

    for note in &chart.slides {
        let head = find_slide_head(&chart.slide_heads, note.logical_slide_id.clone());
        let judge_semantic_time = note
            .judge_at
            .clone()
            .unwrap_or_else(|| tp_add(note.start_timing.clone(), note.length.clone()));
        let head_timing = head
            .map(|h| h.timing.clone())
            .unwrap_or(note.head_timing.clone());
        let head_zone = head
            .map(|h| h.slot.to_button_zone())
            .unwrap_or_else(|| note.slot.to_button_zone());
        entries.push(NoteTimingSkeleton::Slide(SlideTimingSkeleton {
            note_index: note.note_index.clone(),
            head_semantic_time: head_timing.clone(),
            head_input_time: head_timing,
            has_head_input: head.is_some(),
            start_semantic_time: note.start_timing.clone(),
            start_input_time: sensor_input_time(note.start_timing.clone()),
            end_semantic_time: judge_semantic_time.clone(),
            end_input_time: sensor_input_time(judge_semantic_time),
            head_zone,
            path_steps: slide_representative_path_steps(note),
        }));
    }

    // Match Lean's `sortTimingSkeleton`: an insertion sort that inserts
    // *before* the first element with `semantic_time >= entry`, i.e. it
    // reverses ties. A stable `sort_by_key` would not reproduce that order.
    let mut sorted: Vec<NoteTimingSkeleton> = Vec::with_capacity(entries.len());
    for entry in entries {
        let pos = sorted
            .iter()
            .position(|h| entry.semantic_time().micros <= h.semantic_time().micros)
            .unwrap_or(sorted.len());
        sorted.insert(pos, entry);
    }
    sorted
}

/// Port of `evenlySpacedTimesBetween`. For `count >= 2` this yields `count`
/// points with stride `(end-start)/(count-1)`; `count = 1` yields `[end]`.
fn evenly_spaced_times_between(
    start_time: TimePoint,
    end_time: TimePoint,
    count: usize,
) -> Vec<TimePoint> {
    match count {
        0 => Vec::new(),
        1 => vec![end_time],
        n => {
            let steps = n - 1;
            let span = end_time - start_time.clone();
            let stride = span.div_nat(steps);
            (0..=steps)
                .map(|index| tp_add(start_time.clone(), stride.scale_nat(index)))
                .collect()
        }
    }
}

fn slide_step_release_times(start_times: &[TimePoint], end_time: TimePoint) -> Vec<TimePoint> {
    let mut out: Vec<TimePoint> = Vec::with_capacity(start_times.len());
    for i in 0..start_times.len() {
        if i + 1 < start_times.len() {
            out.push(tp_add(
                start_times[i + 1].clone(),
                Duration::from_micros(-1),
            ));
        } else {
            out.push(tp_add(end_time.clone(), FRAME_LENGTH));
        }
    }
    out
}

fn resolve_slide_path_step(
    areas: &[SensorArea],
    start_time: TimePoint,
    end_time: TimePoint,
) -> Vec<TimedInputEvent> {
    let mut out: Vec<TimedInputEvent> = Vec::with_capacity(areas.len() * 2);
    for area in areas {
        out.push(TimedInputEvent::SensorHold(start_time.clone(), *area, true));
    }
    for area in areas {
        out.push(TimedInputEvent::SensorHold(end_time.clone(), *area, false));
    }
    out
}

fn resolve_single_track_slide_with_head_evenly(spec: &SlideTimingSkeleton) -> Vec<TimedInputEvent> {
    let mut events: Vec<TimedInputEvent> = Vec::new();
    if spec.has_head_input {
        events.push(TimedInputEvent::ButtonClick(
            spec.head_input_time.clone(),
            spec.head_zone,
        ));
    }
    let start_times = evenly_spaced_times_between(
        spec.start_input_time.clone(),
        spec.end_input_time.clone(),
        spec.path_steps.len(),
    );
    let end_times = slide_step_release_times(&start_times, spec.end_input_time.clone());
    for (areas, (start, end)) in spec
        .path_steps
        .iter()
        .zip(start_times.iter().zip(end_times.iter()))
    {
        events.extend(resolve_slide_path_step(areas, start.clone(), end.clone()));
    }
    events
}

/// Port of `resolveDefaultTimingSkeleton` (`mkManualTacticSequence` = stable
/// sort by event time).
fn resolve_default_timing_skeleton(entry: &NoteTimingSkeleton) -> Vec<TimedInputEvent> {
    let mut events = match entry {
        NoteTimingSkeleton::Tap {
            input_time, zone, ..
        } => {
            vec![TimedInputEvent::ButtonClick(input_time.clone(), *zone)]
        }
        NoteTimingSkeleton::Hold {
            input_time,
            release_input_time,
            zone,
            ..
        } => {
            let release_time = tp_add(release_input_time.clone(), FRAME_LENGTH.scale_nat(4));
            vec![
                TimedInputEvent::ButtonClick(input_time.clone(), *zone),
                TimedInputEvent::ButtonHold(input_time.clone(), *zone, true),
                TimedInputEvent::ButtonHold(release_time, *zone, false),
            ]
        }
        NoteTimingSkeleton::Touch {
            input_time, area, ..
        } => {
            vec![TimedInputEvent::SensorClick(input_time.clone(), *area)]
        }
        NoteTimingSkeleton::TouchHold {
            input_time,
            release_input_time,
            area,
            ..
        } => {
            let release_time = tp_add(release_input_time.clone(), FRAME_LENGTH.scale_nat(4));
            vec![
                TimedInputEvent::SensorClick(input_time.clone(), *area),
                TimedInputEvent::SensorHold(input_time.clone(), *area, true),
                TimedInputEvent::SensorHold(release_time, *area, false),
            ]
        }
        NoteTimingSkeleton::Slide(spec) => resolve_single_track_slide_with_head_evenly(spec),
    };
    events.sort_by_key(|e| e.at().micros);
    events
}

/// Port of `defaultTacticFromChart`.
pub fn default_tactic_from_chart(chart: &ChartSpec) -> Vec<TimedInputEvent> {
    let skeleton = chart_timing_skeleton(chart);
    let mut events: Vec<TimedInputEvent> = Vec::new();
    for entry in &skeleton {
        events.extend(resolve_default_timing_skeleton(entry));
    }
    events.sort_by_key(|e| e.at().micros);
    events
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutoplayPart {
    pub runtime_note_index: Natural,
    pub role: Natural,
    pub track_index: Option<Natural>,
    pub events: Vec<TimedInputEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutoplayGroup {
    pub id: Natural,
    pub kind: Natural,
    pub runtime_note_indices: Vec<Natural>,
    pub parts: Vec<AutoplayPart>,
    pub depends_on_group_ids: Vec<Natural>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoplayPlan {
    pub version: Natural,
    pub groups: Vec<AutoplayGroup>,
}

impl Default for AutoplayPlan {
    fn default() -> Self {
        Self {
            version: 0.into(),
            groups: Vec::new(),
        }
    }
}

impl AutoplayPlan {
    pub fn events(&self) -> Vec<TimedInputEvent> {
        let mut events: Vec<_> = self
            .groups
            .iter()
            .flat_map(|g| &g.parts)
            .flat_map(|p| p.events.iter().cloned())
            .collect();
        events.sort_by_key(|e| e.at().micros);
        events
    }
}

/// Preserve logical connected-slide ownership before flattening tactic events.
pub fn autoplay_plan_from_chart(chart: &ChartSpec) -> AutoplayPlan {
    let mut groups: Vec<AutoplayGroup> = Vec::new();
    for skeleton in chart_timing_skeleton(chart) {
        let id = skeleton.note_index();
        let (group_id, kind, parts, ids) = match &skeleton {
            NoteTimingSkeleton::Slide(spec) => {
                let body = chart.slides.iter().find(|n| n.note_index == id);
                let root = crate::runtime_seek::slide_root(&chart.slides, id.clone());
                let heads: Vec<_> = chart
                    .slide_heads
                    .iter()
                    .filter(|h| body.is_some_and(|n| n.logical_slide_id == h.logical_slide_id))
                    .collect();
                let mut parts: Vec<_> = heads
                    .iter()
                    .map(|h| AutoplayPart {
                        runtime_note_index: h.note_index.clone(),
                        role: 1.into(),
                        track_index: None,
                        events: vec![TimedInputEvent::ButtonClick(
                            h.timing.clone(),
                            h.slot.to_button_zone(),
                        )],
                    })
                    .collect();
                let mut body_spec = spec.clone();
                body_spec.has_head_input = false;
                parts.push(AutoplayPart {
                    runtime_note_index: id.clone(),
                    role: 2.into(),
                    track_index: None,
                    events: resolve_default_timing_skeleton(&NoteTimingSkeleton::Slide(body_spec)),
                });
                let mut ids: Vec<_> = heads.iter().map(|h| h.note_index.clone()).collect();
                ids.push(id);
                (root, 4, parts, ids)
            }
            _ => {
                let kind = match skeleton {
                    NoteTimingSkeleton::Tap { .. } => 0,
                    NoteTimingSkeleton::Hold { .. } => 1,
                    NoteTimingSkeleton::Touch { .. } => 2,
                    NoteTimingSkeleton::TouchHold { .. } => 3,
                    NoteTimingSkeleton::Slide(_) => unreachable!(),
                };
                (
                    id.clone(),
                    kind,
                    vec![AutoplayPart {
                        runtime_note_index: id.clone(),
                        role: 0.into(),
                        track_index: None,
                        events: resolve_default_timing_skeleton(&skeleton),
                    }],
                    vec![id],
                )
            }
        };
        if let Some(group) = groups.iter_mut().find(|g| g.id == group_id) {
            group.runtime_note_indices.extend(ids);
            group.parts.extend(parts);
        } else {
            groups.push(AutoplayGroup {
                id: group_id,
                kind: kind.into(),
                runtime_note_indices: ids,
                parts,
                depends_on_group_ids: vec![],
            });
        }
    }
    AutoplayPlan {
        version: 1.into(),
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{ButtonZone, SensorArea};
    use crate::simai::frontend::frontend_lowered_chart;

    fn tactic(content: &str) -> Vec<TimedInputEvent> {
        let chart = frontend_lowered_chart(content, (1).into()).expect("parse");
        default_tactic_from_chart(&chart)
    }

    fn tp(us: i64) -> TimePoint {
        TimePoint::from_micros(us)
    }

    #[test]
    fn tap_default_tactic() {
        let evs = tactic("&first=0\n&inote_1=\n(120)\n1,\n");
        assert_eq!(
            evs,
            vec![TimedInputEvent::ButtonClick(tp(0), ButtonZone::K1)]
        );
    }

    #[test]
    fn hold_default_tactic() {
        let evs = tactic("&first=0\n&inote_1=\n(120)\n1h[4:1],\n");
        assert_eq!(
            evs,
            vec![
                TimedInputEvent::ButtonClick(tp(0), ButtonZone::K1),
                TimedInputEvent::ButtonHold(tp(0), ButtonZone::K1, true),
                TimedInputEvent::ButtonHold(tp(566668), ButtonZone::K1, false),
            ]
        );
    }

    #[test]
    fn multi_tap_tie_order_matches_lean() {
        // Lean's `sortTimingSkeleton` inserts before the first element whose
        // semantic time is `>=` the entry, so equal-time taps come out in
        // reverse slot order.
        let evs = tactic("&first=0\n&inote_1=\n(120)\n1/2/3/4,\n");
        let zones: Vec<ButtonZone> = evs
            .iter()
            .map(|e| match e {
                TimedInputEvent::ButtonClick(_, z) => *z,
                other => panic!("unexpected event {other:?}"),
            })
            .collect();
        assert_eq!(
            zones,
            vec![
                ButtonZone::K4,
                ButtonZone::K3,
                ButtonZone::K2,
                ButtonZone::K1
            ]
        );
    }

    #[test]
    fn slide_default_tactic() {
        let evs = tactic("&first=0\n&inote_1=\n(120)\n1-5[8:1],\n");
        assert_eq!(
            evs,
            vec![
                TimedInputEvent::ButtonClick(tp(0), ButtonZone::K1),
                TimedInputEvent::SensorHold(tp(500000), SensorArea::A1, true),
                TimedInputEvent::SensorHold(tp(552999), SensorArea::A1, false),
                TimedInputEvent::SensorHold(tp(553000), SensorArea::B1, true),
                TimedInputEvent::SensorHold(tp(605999), SensorArea::B1, false),
                TimedInputEvent::SensorHold(tp(606000), SensorArea::C, true),
                TimedInputEvent::SensorHold(tp(658999), SensorArea::C, false),
                TimedInputEvent::SensorHold(tp(659000), SensorArea::B5, true),
                TimedInputEvent::SensorHold(tp(711999), SensorArea::B5, false),
                TimedInputEvent::SensorHold(tp(712000), SensorArea::A5, true),
                TimedInputEvent::SensorHold(tp(728667), SensorArea::A5, false),
            ]
        );
    }
}
