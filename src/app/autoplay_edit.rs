//! Editor ownership keys do not depend on core allocation order or absolute time.

use super::types::{ChartDoc, note_secs};
use crate::core::types::{
    AutoplayGroup, AutoplayPart, AutoplayPlan, TimedInputEvent, sort_timed_input_events,
};
use crate::player::engine::{JudgeEngine, timed_input_tp};
use serde::{Deserialize, Serialize};

// Editor-only ownership namespace; note_id stores a clip ID and times are absolute.
const STANDALONE_ROLE: u64 = u64::MAX;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutoplayKey {
    pub note_id: u64,
    pub role: u64,
    pub slide: usize,
    pub segment: usize,
    pub track: Option<u64>,
}

impl AutoplayKey {
    pub fn standalone(id: u64) -> Self {
        Self {
            note_id: id,
            role: STANDALONE_ROLE,
            slide: 0,
            segment: 0,
            track: None,
        }
    }

    pub fn is_standalone(&self) -> bool {
        self.role == STANDALONE_ROLE
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoplayOverride {
    pub key: AutoplayKey,
    pub events: Vec<TimedInputEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<AutoplayCurve>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveTiming {
    ConstantSpeed,
    FixedDuration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveHandles {
    pub incoming: [f32; 2],
    pub outgoing: [f32; 2],
    pub aligned: bool,
}

/// Pad-radius coordinates; start uses the same relative anchor as events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoplayCurve {
    pub points: Vec<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handles: Vec<CurveHandles>,
    pub smooth: bool,
    pub start: f64,
    pub timing: CurveTiming,
    pub speed: f64,
    pub duration: f64,
    pub radius: f32,
}

impl Default for AutoplayCurve {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            handles: Vec::new(),
            smooth: true,
            start: 0.0,
            timing: CurveTiming::ConstantSpeed,
            speed: 2.0,
            duration: 1.0,
            radius: 0.025,
        }
    }
}

/// A whole-track time translation preserves the editable curve; other raw
/// event edits deliberately detach it so stale geometry cannot overwrite them.
pub fn translated_curve(
    item: &AutoplayOverride,
    events: &[TimedInputEvent],
) -> Option<AutoplayCurve> {
    let mut curve = item.curve.clone()?;
    if events.len() != item.events.len() {
        return None;
    }
    let delta =
        timed_input_tp(events.first()?).checked_sub(timed_input_tp(item.events.first()?))?;
    if !events.iter().zip(&item.events).all(|(a, b)| {
        timed_input_tp(b)
            .checked_add(delta)
            .is_some_and(|tp| *a == with_time(b, tp))
    }) {
        return None;
    }
    curve.start += delta as f64 / 1e6;
    Some(curve)
}

pub fn key(chart: &ChartDoc, engine: &JudgeEngine, part: &AutoplayPart) -> Option<AutoplayKey> {
    if part.role == STANDALONE_ROLE {
        return Some(AutoplayKey::standalone(part.runtime_note_index));
    }
    let (note_id, slide, segment) = match part.role {
        1 => (
            engine.slide_head_note_id(chart, part.runtime_note_index)?,
            0,
            0,
        ),
        2 => {
            let runtime = engine.runtime_slide_index(part.runtime_note_index)?;
            let (note_id, slide, segment, _) = engine.slide_position(chart, runtime)?;
            (note_id, slide, segment)
        }
        _ => (engine.chart_note_id(part.runtime_note_index)?, 0, 0),
    };
    Some(AutoplayKey {
        note_id,
        role: part.role,
        slide,
        segment,
        track: part.track_index,
    })
}

pub fn with_time(event: &TimedInputEvent, tp: i64) -> TimedInputEvent {
    let mut event = event.clone();
    match &mut event {
        TimedInputEvent::ButtonClick { tp: time, .. }
        | TimedInputEvent::ButtonHold { tp: time, .. }
        | TimedInputEvent::SensorClick { tp: time, .. }
        | TimedInputEvent::SensorHold { tp: time, .. } => *time = tp,
    }
    event
}

pub fn relative(events: &[TimedInputEvent], head: i64) -> Vec<TimedInputEvent> {
    events
        .iter()
        .map(|event| with_time(event, timed_input_tp(event).saturating_sub(head)))
        .collect()
}

pub fn generated_plan(
    chart: &ChartDoc,
    engine: &JudgeEngine,
    defaults: &AutoplayPlan,
    inner_screen: bool,
) -> AutoplayPlan {
    let mut plan = defaults.clone();
    if inner_screen {
        for event in plan
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.parts)
            .flat_map(|part| &mut part.events)
        {
            *event = crate::player::engine::normalize_tactic_event(event.clone());
        }
    }
    // Authored events keep their explicit button/sensor choice in either mode.
    apply(chart, engine, &mut plan);
    plan
}

pub fn apply(chart: &ChartDoc, engine: &JudgeEngine, plan: &mut AutoplayPlan) {
    plan.groups.retain(|group| group.kind != STANDALONE_ROLE);
    if chart.autoplay_overrides.is_empty() {
        return;
    }
    let authored: std::collections::HashMap<_, _> = chart
        .autoplay_overrides
        .iter()
        .map(|item| (&item.key, &item.events))
        .collect();
    let heads: std::collections::HashMap<_, _> = chart
        .notes
        .iter()
        .map(|note| (note.id, (note_secs(note, &chart.bpms) * 1e6).round() as i64))
        .collect();
    for part in plan.groups.iter_mut().flat_map(|group| &mut group.parts) {
        let Some(key) = key(chart, engine, part) else {
            continue;
        };
        let Some(events) = authored.get(&key) else {
            continue;
        };
        let Some(head) = heads.get(&key.note_id) else {
            continue;
        };
        part.events = events
            .iter()
            .map(|event| with_time(event, timed_input_tp(event).saturating_add(*head)))
            .collect();
        sort_timed_input_events(&mut part.events);
    }
    let mut ids: std::collections::HashSet<_> = plan.groups.iter().map(|group| group.id).collect();
    let mut next_id = 0;
    for item in chart
        .autoplay_overrides
        .iter()
        .filter(|item| item.key.is_standalone())
    {
        while ids.contains(&next_id) {
            next_id += 1;
        }
        ids.insert(next_id);
        let mut events = item.events.clone();
        sort_timed_input_events(&mut events);
        plan.groups.push(AutoplayGroup {
            id: next_id,
            kind: STANDALONE_ROLE,
            // No note dependency: seeking must retain these explicit inputs.
            runtime_note_indices: Vec::new(),
            depends_on_group_ids: Vec::new(),
            parts: vec![AutoplayPart {
                runtime_note_index: item.key.note_id,
                role: STANDALONE_ROLE,
                track_index: None,
                events,
            }],
        });
    }
}

/// Compile each owner's held intervals, not the last down/up event per area.
pub fn compiled_events(plan: &AutoplayPlan) -> Vec<TimedInputEvent> {
    use crate::core::types::timed_input_event_sort_key;
    use std::collections::{HashMap, HashSet};
    let mut owned: Vec<_> = plan.groups.iter().flat_map(|group| &group.parts).enumerate()
        .flat_map(|(owner, part)| part.events.iter().cloned().map(move |event| (owner, event)))
        .collect();
    owned.sort_by_key(|(_, event)| timed_input_event_sort_key(event));
    let mut active = HashSet::new();
    let mut counts = HashMap::<u8, usize>::new();
    let mut result = Vec::with_capacity(owned.len());
    for (owner, event) in owned {
        let (channel, down) = match &event {
            TimedInputEvent::ButtonHold { zone, is_down, .. } => (crate::player::engine::zone_for_button(*zone).to_id(), *is_down),
            TimedInputEvent::SensorHold { area, is_down, .. } => (8 + crate::player::engine::zone_for_sensor(*area).to_id(), *is_down),
            _ => { result.push(event); continue; }
        };
        let count = counts.entry(channel).or_default();
        if down && active.insert((owner, channel)) {
            if *count == 0 { result.push(event); }
            *count += 1;
        } else if !down && active.remove(&(owner, channel)) {
            *count -= 1;
            if *count == 0 { result.push(event); }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ButtonZone, SensorArea};

    fn plan(parts: Vec<Vec<TimedInputEvent>>) -> AutoplayPlan {
        AutoplayPlan {
            version: 1, chart_fingerprint: String::new(),
            groups: parts.into_iter().enumerate().map(|(id, events)| AutoplayGroup {
                id: id as u64, kind: 4, runtime_note_indices: vec![id as u64],
                depends_on_group_ids: Vec::new(),
                parts: vec![AutoplayPart { runtime_note_index: id as u64, role: 2, track_index: None, events }],
            }).collect(),
        }
    }

    fn hold(tp: i64, is_down: bool) -> TimedInputEvent {
        TimedInputEvent::SensorHold { tp, area: SensorArea::A4, is_down }
    }

    #[test]
    fn compiled_holds_keep_overlapping_owners_and_every_click() {
        let click = TimedInputEvent::SensorClick { tp: 10, area: SensorArea::A4 };
        let plan = plan(vec![vec![hold(0, true), hold(20, false)],
            vec![click.clone(), hold(10, true), hold(100, false)]]);
        let original = plan.clone();
        assert_eq!(compiled_events(&plan), vec![hold(0, true), click, hold(100, false)]);
        assert_eq!(plan, original, "compilation must preserve editable ownership");
        let retained = plan.retained(&[1]);
        assert_eq!(compiled_events(&retained), retained.events());
    }

    #[test]
    fn compiled_holds_keep_button_and_sensor_channels_separate() {
        let button_down = TimedInputEvent::ButtonHold { tp: 0, zone: ButtonZone::K4, is_down: true };
        let button_up = TimedInputEvent::ButtonHold { tp: 30, zone: ButtonZone::K4, is_down: false };
        let plan = plan(vec![vec![button_down.clone(), button_up.clone()],
            vec![hold(10, true), hold(20, false)]]);
        assert_eq!(compiled_events(&plan), vec![button_down, hold(10, true), hold(20, false), button_up]);
    }

    #[test]
    fn compiled_holds_ignore_duplicate_down_without_leaving_stuck_inputs() {
        let plan = plan(vec![vec![hold(0, true), hold(5, true), hold(10, false), hold(20, true), hold(30, false)]]);
        assert_eq!(compiled_events(&plan), vec![hold(0, true), hold(10, false), hold(20, true), hold(30, false)]);
    }
}
