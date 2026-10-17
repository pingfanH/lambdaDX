//! Reusable timeline lanes with note-owned and standalone event groups.

use crate::app::autoplay_edit::{self, AutoplayKey, AutoplayOverride};
use crate::app::types::{ChartDoc, Mode, measure_to_secs, note_secs, secs_to_measure};
use crate::core::types::{AutoplayPlan, ButtonZone, SensorArea, TimedInputEvent};
use crate::player::{engine::timed_input_tp, state::PadPreviewState};
use crate::state::EditorState;
use egui_macroquad::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[cfg(all(test, feature = "backend-lean"))]
mod tests;
mod clips;
mod follow;
mod inspector;
mod curve;
pub mod curves;

const HEADER: f32 = 96.0;
const ROW: f32 = 52.0;
const EVENT_ROW: f32 = 32.0;
const DEFAULT_LANES: usize = 4;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ViewOptions {
    start: f32,
    pps: f32,
    follow: bool,
    lanes: usize,
    raw: bool,
    filter: String,
}
impl Default for ViewOptions {
    fn default() -> Self { Self { start: 0.0, pps: crate::timeline::geometry::DEFAULT_PPS,
        follow: true, lanes: DEFAULT_LANES, raw: false, filter: String::new() } }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub key: AutoplayKey,
    pub label: String,
    pub head: i64,
    pub defaults: Vec<TimedInputEvent>,
    pub events: Vec<TimedInputEvent>,
    authored: bool,
}

impl Track {
    fn standalone(key: AutoplayKey, events: Vec<TimedInputEvent>) -> Self {
        Self {
            label: format!("Event {}", key.note_id),
            key,
            head: 0,
            defaults: Vec::new(),
            authored: !events.is_empty(),
            events,
        }
    }
}

#[derive(Debug, Clone)]
struct Drag {
    row: usize,
    index: usize,
    original: TimedInputEvent,
    origin_x: f32,
}

#[derive(Debug, Clone)]
struct InspectorEdit {
    row: usize,
    original: Vec<TimedInputEvent>,
}

#[derive(Debug, Clone, Copy)]
struct LaneGroup {
    row: usize,
    start: i64,
    end: i64,
}

#[derive(Debug, Clone, Default)]
struct EventLayout {
    rows: Vec<usize>,
    holds: Vec<(usize, Option<usize>)>,
    count: usize,
}

#[derive(Debug, Clone)]
pub struct AutoplayEditor {
    pub tracks: Vec<Track>,
    plan: Option<Arc<AutoplayPlan>>,
    inner_screen: bool,
    overrides: Vec<AutoplayOverride>,
    edits_pending_sync: bool,
    pub selected: Option<(usize, usize)>,
    pub selected_row: Option<usize>,
    drag: Option<Drag>,
    pan_x: Option<f32>,
    inspector_edit: Option<InspectorEdit>,
    pub filter: String,
    lanes: Vec<Vec<LaneGroup>>,
    lane_count: usize,
    lanes_dirty: bool,
    layouts: Vec<EventLayout>,
    layout_pps: f32,
    lane_heights: Vec<f32>,
    new_zone: String,
    new_kind: u8,
    new_hold: bool,
    pub view_start: f32,
    pub pixels_per_second: f32,
    center_requested: bool,
    view_changed: bool,
    plot_width: f32,
    pub follow: bool,
    follow_motion: follow::PlayheadFollow,
    raw_events: bool,
    draw_mode: bool,
    drawing_owner: Option<AutoplayKey>,
    clip_layout: Arc<clips::ClipLayout>,
    clip_drag: Option<clips::Gesture>,
    detail_row: Option<usize>,
    selected_pair: Option<usize>,
    fit_requested: Option<(f32, f32)>,
    suppress_pointer: bool,
    curve_draft: Option<curves::Draft>,
    curve_source: Option<AutoplayKey>,
    curve_cache: HashMap<AutoplayKey, Result<crate::app::autoplay_edit::AutoplayCurve, String>>,
    sensor_detail: bool,
}

impl Default for AutoplayEditor {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            plan: None,
            inner_screen: false,
            overrides: Vec::new(),
            edits_pending_sync: false,
            selected: None,
            selected_row: None,
            drag: None,
            pan_x: None,
            inspector_edit: None,
            filter: String::new(),
            lanes: vec![Vec::new(); DEFAULT_LANES],
            lane_count: DEFAULT_LANES,
            lanes_dirty: true,
            layouts: Vec::new(),
            layout_pps: 0.0,
            lane_heights: Vec::new(),
            new_zone: "A1".into(),
            new_kind: 0,
            new_hold: true,
            view_start: 0.0,
            pixels_per_second: crate::timeline::geometry::DEFAULT_PPS,
            center_requested: false,
            view_changed: false,
            plot_width: 0.0,
            follow: true,
            follow_motion: Default::default(),
            raw_events: false,
            draw_mode: false,
            drawing_owner: None,
            clip_layout: Default::default(),
            clip_drag: None,
            detail_row: None,
            selected_pair: None,
            fit_requested: None,
            suppress_pointer: false,
            curve_draft: None,
            curve_source: None,
            curve_cache: HashMap::new(),
            sensor_detail: false,
        }
    }
}

impl AutoplayEditor {
    pub fn view_options(&self) -> ViewOptions {
        ViewOptions { start: self.view_start, pps: self.pixels_per_second, follow: self.follow,
            lanes: self.lane_count, raw: self.raw_events, filter: self.filter.clone() }
    }

    pub fn restore_view(&mut self, view: &ViewOptions) {
        self.view_start = view.start;
        self.pixels_per_second = view.pps.clamp(crate::timeline::geometry::MIN_PPS, crate::timeline::geometry::MAX_PPS);
        self.follow = view.follow;
        self.lane_count = view.lanes.clamp(1, 32);
        self.raw_events = view.raw;
        self.filter.clone_from(&view.filter);
        self.follow_motion.reset();
        self.center_requested = false;
        self.lanes_dirty = true;
    }

    pub fn pointer_dragging(&self) -> bool { self.drag.is_some() || self.clip_drag.is_some() || self.pan_x.is_some() }

    pub fn dragging(&self) -> bool {
        self.pointer_dragging() || self.inspector_edit.is_some() || self.curve_editing()
    }

    fn sync(&mut self, app: &PadPreviewState) {
        if self.dragging() {
            return;
        }
        let Some(plan) = &app.default_autoplay_plan else {
            return;
        };
        let changed = self.plan.as_ref().is_none_or(|old| !Arc::ptr_eq(old, plan))
            || self.inner_screen != app.params.autoplay_inner_screen;
        let refresh = changed || self.edits_pending_sync || self.overrides != app.chart.autoplay_overrides;
        let old_key = self.selected_row.and_then(|row| self.tracks.get(row)).map(|track| track.key.clone());
        let old_detail = refresh.then(|| self.detail_row.and_then(|row| self.tracks.get(row)).map(|track| track.key.clone())).flatten();
        let old_selection = refresh.then(|| self.selected.and_then(|(row, index)| {
            let track = self.tracks.get(row)?;
            let event = track.events.get(index)?;
            let relative = |event: &TimedInputEvent| autoplay_edit::with_time(event, timed_input_tp(event).saturating_sub(track.head));
            Some((track.key.clone(), index, relative(event), self.selected_pair.and_then(|i| track.events.get(i)).map(relative)))
        })).flatten();
        if changed {
            let Some(engine) = &app.judge_engine else {
                return;
            };
            let chart = app.playback_chart();
            self.curve_cache.clear();
            let notes: HashMap<_, _> = chart.notes.iter().map(|note| (note.id, note)).collect();
            self.tracks.clear();
            for part in plan.groups.iter().flat_map(|group| &group.parts) {
                let Some(key) = autoplay_edit::key(chart, engine, part) else {
                    continue;
                };
                let Some(note) = notes.get(&key.note_id) else {
                    continue;
                };
                let role = match key.role {
                    1 => "Head".into(),
                    2 => format!("Slide {}.{}", key.slide + 1, key.segment + 1),
                    _ => format!("{:?}", note.note_type),
                };
                let branch = key
                    .track
                    .map(|t| format!(" / {}", t + 1))
                    .unwrap_or_default();
                let owner = if note.template_source.is_some() {
                    format!("T:{:06x}", note.id & 0xffffff)
                } else {
                    format!("#{}", note.id)
                };
                let defaults: Vec<_> = part.events.iter().cloned()
                    .map(|event| {
                        if app.params.autoplay_inner_screen {
                            crate::player::engine::normalize_tactic_event(event)
                        } else {
                            event
                        }
                    })
                    .collect();
                self.tracks.push(Track {
                    label: format!("{role}{branch}  {owner}"),
                    key,
                    head: (note_secs(note, &chart.bpms) * 1e6).round() as i64,
                    events: defaults.clone(),
                    defaults,
                    authored: false,
                });
            }
            self.tracks.sort_by(|a, b| {
                a.head
                    .cmp(&b.head)
                    .then(a.key.note_id.cmp(&b.key.note_id))
                    .then(a.key.role.cmp(&b.key.role))
                    .then(a.key.slide.cmp(&b.key.slide))
                    .then(a.key.segment.cmp(&b.key.segment))
                    .then(a.key.track.cmp(&b.key.track))
            });
            self.plan = Some(Arc::clone(plan));
            self.inner_screen = app.params.autoplay_inner_screen;
        }
        if refresh {
            self.tracks.retain(|track| !track.key.is_standalone());
            for item in app.chart.autoplay_overrides.iter().filter(|item| item.key.is_standalone()) {
                let mut track = Track::standalone(item.key.clone(), item.events.clone());
                if item.curve.is_some() { track.label = format!("Curve {}", item.key.note_id); }
                self.tracks.push(track);
            }
            let authored: HashMap<_, _> = app
                .chart
                .autoplay_overrides
                .iter()
                .map(|item| (&item.key, &item.events))
                .collect();
            for track in &mut self.tracks {
                track.authored = authored.contains_key(&track.key);
                track.events = authored
                    .get(&track.key)
                    .map(|events| {
                        events
                            .iter()
                            .map(|event| {
                                autoplay_edit::with_time(
                                    event,
                                    timed_input_tp(event).saturating_add(track.head),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_else(|| track.defaults.clone());
            }
            self.overrides.clone_from(&app.chart.autoplay_overrides);
            self.edits_pending_sync = false;
            self.selected_row = old_key.and_then(|key| self.tracks.iter().position(|track| track.key == key));
            self.detail_row = old_detail.and_then(|key| self.tracks.iter().position(|t| t.key == key));
            self.selected_pair = None;
            self.selected = old_selection.and_then(|(key, old_index, event, pair)| {
                let row = self.tracks.iter().position(|t| t.key == key)?;
                let track = &self.tracks[row];
                let matches = |candidate: &TimedInputEvent, target: &TimedInputEvent| {
                    autoplay_edit::with_time(candidate, timed_input_tp(candidate).saturating_sub(track.head)) == *target
                };
                let index = if track.events.get(old_index).is_some_and(|e| matches(e, &event)) { old_index }
                    else { track.events.iter().position(|e| matches(e, &event))? };
                self.selected_pair = pair.and_then(|pair| track.events.iter().enumerate()
                    .find(|(i, e)| *i != index && matches(e, &pair)).map(|(i, _)| i));
                Some((row, index))
            });
            self.lanes_dirty = true;
        }
        self.validate_selection();
        self.repack();
    }

    fn validate_selection(&mut self) {
        if self.selected_row.is_some_and(|row| row >= self.tracks.len()) { self.selected_row = None; }
        if self.detail_row.is_some_and(|row| row >= self.tracks.len()) { self.detail_row = None; }
        if self.selected.is_some_and(|(r, i)| {
            self.tracks
                .get(r)
                .is_none_or(|track| i >= track.events.len())
        }) {
            self.selected = None;
        }
        if let Some((row, index)) = self.selected {
            if self.selected_pair.is_some_and(|pair| !clips::intervals(&self.tracks[row].events)
                .iter().any(|span| span.press == index && span.release == Some(pair))) { self.selected_pair = None; }
        } else { self.selected_pair = None; }
    }

    fn repack(&mut self) {
        if self.lanes_dirty && !self.dragging() {
            self.lanes = pack_lanes(&self.tracks, self.lane_count);
            self.clip_layout = Arc::new(clips::ClipLayout::build(&self.tracks, self.lane_count, &self.clip_layout));
            self.lanes_dirty = false;
            self.layout_pps = 0.0;
        }
    }

    fn layout_events(&mut self, pps: f32) {
        if self.layout_pps == pps || self.dragging() {
            return;
        }
        self.layouts = self.tracks.iter().map(|track| event_layout(&track.events, pps)).collect();
        self.lane_heights = self.lanes.iter().map(|groups| {
            let count = groups.iter().map(|group| self.layouts[group.row].count).max().unwrap_or(1);
            (20.0 + count as f32 * EVENT_ROW).max(ROW)
        }).collect();
        self.layout_pps = pps;
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_row = None;
        self.selected_pair = None;
        self.draw_mode = false;
        self.drawing_owner = None;
        self.curve_source = None;
        self.sensor_detail = false;
    }

    fn start_drawing(&mut self) {
        self.sensor_detail = true;
        self.drawing_owner = self.selected_row.and_then(|row| self.tracks.get(row)).map(|track| track.key.clone());
        self.selected = None;
        self.selected_pair = None;
        self.draw_mode = true;
    }

    fn selected_interval(&self) -> Option<(usize, clips::Interval)> {
        let (row, index) = self.selected?;
        let events = &self.tracks.get(row)?.events;
        clips::intervals(events).into_iter()
            .find(|span| span.press == index || span.release == Some(index))
            .map(|span| (row, span))
    }

    fn choose_hold(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, hold: bool) {
        if !self.can_pick_zone() { return; }
        self.new_hold = hold;
        if self.draw_mode || self.raw_events { return; }
        self.finish_inspector(editor, chart);
        self.validate_selection();
        let Some((row, span)) = self.selected_interval() else { return; };
        let event = &self.tracks[row].events[span.press];
        if (hold && span.release.is_some()) || (!hold && event_kind(event) == 0) { return; }
        let start = timed_input_tp(event);
        let zone = event_zone(event);
        let mut events = self.tracks[row].events.clone();
        let mut index = span.press;
        let pair = if hold {
            events[index] = make_event(&zone, 1, start);
            let release = events.len();
            events.push(make_event(&zone, 2, default_hold_end(start, editor, chart)));
            Some(release)
        } else {
            events[index] = make_event(&zone, 0, start);
            if let Some(release) = span.release {
                events.remove(release);
                if release < index { index -= 1; }
            }
            None
        };
        self.commit(editor, chart, row, events);
        self.selected_row = Some(row);
        self.selected = Some((row, index));
        self.selected_pair = pair;
    }

    pub fn pad_zone(&self) -> String {
        if !self.draw_mode {
            if let Some(event) = self.selected.and_then(|(row, index)|
                self.tracks.get(row).and_then(|t| t.events.get(index))) {
                return event_zone(event);
            }
        }
        self.new_zone.clone()
    }

    pub fn can_pick_zone(&self) -> bool {
        !self.pointer_dragging() && !self.curve_editing() && !self.suppress_pointer
    }

    pub fn pick_zone(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, zone: &str, time: f32) {
        if !self.can_pick_zone() { return; }
        self.finish_inspector(editor, chart);
        self.validate_selection();
        self.new_zone = zone.into();
        if self.draw_mode {
            self.add_at_playhead(editor, chart, time);
            return;
        }
        let Some((row, index)) = self.selected else { return; };
        let mut events = self.tracks[row].events.clone();
        let indices = if self.raw_events { vec![index] } else {
            self.selected_interval().map(|(_, span)|
                std::iter::once(span.press).chain(span.release).collect())
                .unwrap_or_else(|| vec![index])
        };
        for index in indices {
            events[index] = make_event(&self.new_zone, event_kind(&events[index]), timed_input_tp(&events[index]));
        }
        self.commit(editor, chart, row, events);
    }

    fn add_at_playhead(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, time: f32) {
        self.finish_inspector(editor, chart);
        let row = self.creation_row(chart);
        let time = snap_time(time, editor, chart);
        let start = (time * 1e6).round() as i64;
        let mut events = self.tracks[row].events.clone();
        let index = events.len();
        let pair = if self.raw_events || !self.new_hold {
            events.push(make_event(&self.new_zone, if self.raw_events { self.new_kind } else { 0 }, start));
            None
        } else {
            events.push(make_event(&self.new_zone, 1, start));
            events.push(make_event(&self.new_zone, 2, default_hold_end(start, editor, chart)));
            Some(index + 1)
        };
        self.commit(editor, chart, row, events);
        self.selected_row = Some(row);
        self.selected = Some((row, index));
        self.selected_pair = pair;
        self.center_requested = false;
    }

    fn creation_row(&mut self, chart: &ChartDoc) -> usize {
        let row = if self.draw_mode {
            self.drawing_owner.as_ref().and_then(|key| self.tracks.iter().position(|track| &track.key == key))
        } else {
            self.selected_row.filter(|&row| row < self.tracks.len())
        };
        if let Some(row) = row { return row; }
        let ids: HashSet<_> = chart.autoplay_overrides.iter().map(|item| &item.key)
            .chain(self.tracks.iter().map(|track| &track.key))
            .filter(|key| key.is_standalone()).map(|key| key.note_id).collect();
        let id = (1..=u64::MAX).find(|id| !ids.contains(id)).expect("standalone clip IDs exhausted");
        let row = self.tracks.len();
        self.tracks.push(Track::standalone(AutoplayKey::standalone(id), Vec::new()));
        self.lanes_dirty = true;
        row
    }

    pub fn commit(
        &mut self,
        editor: &mut EditorState,
        chart: &mut ChartDoc,
        row: usize,
        events: Vec<TimedInputEvent>,
    ) {
        let curve = self.translated_track_curve(chart, row, &events);
        let Some(track) = self.tracks.get_mut(row) else {
            return;
        };
        let relative = autoplay_edit::relative(&events, track.head);
        let existing = chart
            .autoplay_overrides
            .iter()
            .find(|item| item.key == track.key);
        if existing.is_some_and(|item| item.events == relative)
            || existing.is_none() && events == track.defaults
        {
            return;
        }
        let authored = events != track.defaults || curve.is_some();
        editor.push_history(chart);
        chart
            .autoplay_overrides
            .retain(|item| item.key != track.key);
        if authored {
            chart.autoplay_overrides.push(AutoplayOverride {
                key: track.key.clone(),
                events: relative,
                curve,
            });
        }
        track.authored = authored;
        track.events = events;
        self.edits_pending_sync = true;
        self.lanes_dirty = true;
    }

    fn commit_many(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, edits: Vec<(usize, Vec<TimedInputEvent>)>) {
        let edits: Vec<_> = edits.into_iter().filter(|(row, events)| {
            self.tracks.get(*row).is_some_and(|track| {
                let relative = autoplay_edit::relative(events, track.head);
                chart.autoplay_overrides.iter().find(|item| item.key == track.key)
                    .map_or(*events != track.defaults, |item| item.events != relative)
            })
        }).collect();
        if edits.is_empty() { return; }
        editor.push_history(chart);
        for (row, events) in edits {
            let curve = self.translated_track_curve(chart, row, &events);
            let track = &mut self.tracks[row];
            let relative = autoplay_edit::relative(&events, track.head);
            chart.autoplay_overrides.retain(|item| item.key != track.key);
            track.authored = events != track.defaults || curve.is_some();
            if track.authored {
                chart.autoplay_overrides.push(AutoplayOverride {
                    key: track.key.clone(), events: relative, curve,
                });
            }
            track.events = events;
        }
        self.edits_pending_sync = true;
        self.lanes_dirty = true;
    }

    fn clip_rows(&self, row: usize) -> Vec<usize> {
        self.clip_layout.clips.iter().find(|clip| clip.rows.contains(&row))
            .map(|clip| clip.rows.clone()).unwrap_or_else(|| vec![row])
    }

    pub fn selection_span(&self) -> Option<(f32, f32)> {
        if let Some((row, index)) = self.selected {
            let events = &self.tracks.get(row)?.events;
            let start = timed_input_tp(events.get(index)?);
            let end = self.selected_pair.and_then(|i| events.get(i)).map(timed_input_tp).unwrap_or(start);
            return Some((start.min(end) as f32 / 1e6, start.max(end) as f32 / 1e6));
        }
        let row = self.selected_row?;
        let rows = self.clip_rows(row);
        let mut times = rows.iter().filter_map(|row| self.tracks.get(*row))
            .flat_map(|track| &track.events).map(timed_input_tp);
        let first = times.next()?;
        let (start, end) = times.fold((first, first), |(a, b), t| (a.min(t), b.max(t)));
        Some((start as f32 / 1e6, end as f32 / 1e6))
    }

    pub fn locate(&mut self, span: Option<(f32, f32)>) {
        self.fit_requested = span;
        self.center_requested = false;
    }

    pub fn return_current(&mut self) { self.follow = true; self.center_requested = true; }

    pub fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        self.center_requested = false;
        self.follow_motion.reset();
    }

    pub fn reset(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, row: usize) {
        self.reset_rows(editor, chart, &[row]);
    }

    fn reset_rows(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, rows: &[usize]) {
        let keys: HashSet<_> = rows.iter().filter_map(|&row| self.tracks.get(row))
            .map(|track| track.key.clone()).collect();
        if !chart.autoplay_overrides.iter().any(|item| keys.contains(&item.key)) { return; }
        editor.push_history(chart);
        chart.autoplay_overrides.retain(|item| !keys.contains(&item.key));
        for track in &mut self.tracks {
            if keys.contains(&track.key) {
                track.events = track.defaults.clone();
                track.authored = false;
            }
        }
        self.edits_pending_sync = true;
        self.selected = None;
        self.selected_pair = None;
        self.lanes_dirty = true;
    }

    pub fn delete(&mut self, editor: &mut EditorState, chart: &mut ChartDoc) {
        if let Some((row, index)) = self.selected {
            let span = (!self.raw_events).then(|| self.selected_interval()).flatten().map(|(_, span)| span);
            let (index, pair) = span.map(|span| (span.press, span.release)).unwrap_or((index, self.selected_pair));
            if let Some(track) = self.tracks.get(row) {
                let mut events = track.events.clone();
                if index < events.len() {
                    if let Some(pair) = pair.filter(|pair| *pair < events.len() && *pair != index) {
                        events.remove(index.max(pair));
                        events.remove(index.min(pair));
                    } else {
                    events.remove(index);
                    }
                    self.commit(editor, chart, row, events);
                }
            }
            self.selected = None;
            self.selected_pair = None;
        } else if let Some(row) = self.selected_row {
            let edits = self.clip_rows(row).into_iter().map(|row| (row, Vec::new())).collect();
            self.commit_many(editor, chart, edits);
        }
    }

    pub fn cancel(&mut self) {
        self.cancel_curve();
        self.pan_x = None;
        if let Some(gesture) = self.clip_drag.take() {
            self.selected = gesture.selection;
            self.selected_pair = gesture.pair;
            self.selected_row = gesture.row;
            for (row, events) in gesture.originals {
                if let Some(track) = self.tracks.get_mut(row) { track.events = events; }
            }
            if let Some(row) = gesture.created_row {
                self.tracks.remove(row);
            }
            self.lanes_dirty = true;
        }
        if let Some(drag) = self.drag.take() {
            if let Some(event) = self
                .tracks
                .get_mut(drag.row)
                .and_then(|t| t.events.get_mut(drag.index))
            {
                *event = drag.original;
            }
            self.lanes_dirty = true;
        }
        if let Some(edit) = self.inspector_edit.take() {
            if let Some(track) = self.tracks.get_mut(edit.row) {
                track.events = edit.original;
            }
            self.lanes_dirty = true;
        }
        self.validate_selection();
    }

    fn finish_inspector(&mut self, editor: &mut EditorState, chart: &mut ChartDoc) {
        if let Some(edit) = self.inspector_edit.take() {
            if let Some(track) = self.tracks.get(edit.row) {
                if track.events != edit.original {
                    self.commit(editor, chart, edit.row, track.events.clone());
                }
            }
        }
    }
}

fn event_span(track: &Track) -> (i64, i64) {
    let events = if track.events.is_empty() {
        &track.defaults
    } else {
        &track.events
    };
    let mut sorted: Vec<_> = events.iter().collect();
    sorted.sort_by_key(|event| timed_input_tp(event));
    let start = sorted
        .first()
        .map(|event| timed_input_tp(event))
        .unwrap_or(track.head);
    let mut end = sorted
        .last()
        .map(|event| timed_input_tp(event))
        .unwrap_or(start);
    let mut held = HashSet::new();
    for event in sorted {
        match event_kind(event) {
            1 => {
                held.insert(event_zone(event));
            }
            2 => {
                held.remove(&event_zone(event));
            }
            _ => {}
        }
    }
    if !track.events.is_empty() && !held.is_empty() {
        end = i64::MAX;
    }
    (start, end)
}

fn pack_lanes(tracks: &[Track], minimum: usize) -> Vec<Vec<LaneGroup>> {
    let mut groups: Vec<_> = tracks
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let (start, end) = event_span(track);
            (start, index, end)
        })
        .collect();
    groups.sort_by_key(|&(start, index, _)| (start, index));
    let mut lanes = vec![Vec::new(); minimum.max(1)];
    let mut ends = vec![i64::MIN; lanes.len()];
    for (start, index, end) in groups {
        let lane = ends.iter().position(|end| *end < start).unwrap_or_else(|| {
            lanes.push(Vec::new());
            ends.push(i64::MIN);
            lanes.len() - 1
        });
        lanes[lane].push(LaneGroup {
            row: index,
            start,
            end,
        });
        ends[lane] = end;
    }
    lanes
}

fn event_layout(events: &[TimedInputEvent], pps: f32) -> EventLayout {
    let mut spans: Vec<_> = clips::intervals(events).into_iter().map(|span| {
        let start = timed_input_tp(&events[span.press]);
        let end = span.release.map(|i| timed_input_tp(&events[i])).unwrap_or(start);
        (start, end, span.press, span.release)
    }).collect();
    spans.sort_by_key(|&(start, _, index, _)| (start, index));
    let padding = (20.0 / pps.max(1.0) * 1e6).ceil() as i64;
    let mut ends: Vec<i64> = Vec::new();
    let mut layout = EventLayout { rows: vec![0; events.len()], ..Default::default() };
    for (start, end, index, release) in spans {
        let row = ends.iter().position(|end| end.saturating_add(padding) < start).unwrap_or_else(|| {
            ends.push(i64::MIN);
            ends.len() - 1
        });
        ends[row] = end;
        layout.rows[index] = row;
        if let Some(release) = release {
            layout.rows[release] = row;
        }
        if event_kind(&events[index]) == 1 && release.is_some() {
            layout.holds.push((index, release));
        }
    }
    layout.count = ends.len().max(1);
    layout
}

fn event_center(event: &TimedInputEvent, index: usize, layout: &EventLayout, top: f32, x: f32) -> Pos2 {
    let offset = match event_kind(event) { 1 => 8.0, 2 => 24.0, _ => 16.0 };
    Pos2::new(x, top + layout.rows[index] as f32 * EVENT_ROW + offset)
}

pub fn prepare(
    ctx: &egui::Context,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    enabled: bool,
    event_commands: bool,
) {
    let mut state = std::mem::take(&mut editor.autoplay_editor);
    state.suppress_pointer = false;
    state.sync(app);
    if state.curve_editing() {
        if enabled { state.curve_keys(ctx, editor, app); }
        editor.autoplay_editor = state;
        return;
    }
    if enabled {
        if event_commands && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if state.dragging() { state.cancel(); }
            else if state.draw_mode { state.draw_mode = false; }
            else if state.selected_row.is_some() { state.clear_selection(); }
            else { state.detail_row = None; }
            state.suppress_pointer = true;
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            editor.autoplay_editor = state;
            return;
        }
        if !ctx.wants_keyboard_input() && !ctx.memory(|m| m.any_popup_open())
            && !state.dragging() && !editor.keyboard.ime {
            crate::keyboard::panels::load_bindings(editor);
            let events = ctx.input(|i| i.events.clone());
            let mut consumed = Vec::new();
            for (event_index, event) in events.into_iter().enumerate() {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                else {
                    continue;
                };
                if event_commands && key == egui::Key::Tab && !modifiers.command && !modifiers.ctrl && !modifiers.alt {
                    clips::navigate(&mut state, modifiers.shift);
                    if let Some(row) = state.selected_row.filter(|_| state.selected.is_none()) { state.select_clip(row, app); }
                    state.prepare_selected_curves(app);
                    consumed.push(event_index);
                    continue;
                }
                if repeat { continue; }
                let mut handled = true;
                match crate::keyboard::panels::bound_command(editor, key, modifiers) {
                    Some("play") => crate::runtime::toggle_play(editor, app),
                    Some("help") => editor.show_help = true,
                    Some("save") => crate::io::save_document(editor, &mut app.chart),
                    Some("save_as") => editor.save_as_request = true,
                    Some("export") => editor.export_request = true,
                    Some("open") => editor.open_dialog = true,
                    Some("undo") => { editor.undo(&mut app.chart); state.suppress_pointer = true; }
                    Some("redo") => { editor.redo(&mut app.chart); state.suppress_pointer = true; }
                    Some("snap") => editor.autoplay_snap_enabled = !editor.autoplay_snap_enabled,
                    Some("commands") => crate::keyboard::panels::open(editor, &app.chart, crate::keyboard::panels::DialogKind::Commands),
                    _ if !modifiers.command && !modifiers.ctrl && !modifiers.alt => match key {
                        egui::Key::G if event_commands && !state.raw_events && !modifiers.shift => clips::begin_selected(&mut state, ctx, false),
                        egui::Key::D if event_commands && !state.raw_events && modifiers.shift => clips::begin_selected(&mut state, ctx, true),
                        egui::Key::V if event_commands => state.draw_mode = false,
                        egui::Key::Enter | egui::Key::E if event_commands && !state.raw_events => {
                            if let Some(row) = state.selected_row {
                                state.detail_row = Some(row);
                                if state.curve_view(row, &app.chart) {
                                    let row = state.selected_curve_row().unwrap_or(row);
                                    state.edit_curve(editor, app, row, false);
                                } else { state.sensor_detail = true; }
                            }
                        }
                        egui::Key::D if event_commands && !state.raw_events => {
                            state.start_drawing();
                        }
                        egui::Key::Delete | egui::Key::Backspace if event_commands && !modifiers.shift => {
                            state.delete(editor, &mut app.chart);
                            state.suppress_pointer = true;
                        }
                        egui::Key::Questionmark | egui::Key::Slash
                            if modifiers.shift || key == egui::Key::Questionmark =>
                        {
                            editor.show_help = true
                        }
                        _ => handled = false,
                    },
                    _ => handled = false,
                }
                if handled { consumed.push(event_index); }
                if state.dragging() || state.suppress_pointer || editor.keyboard.dialog.is_some()
                    || editor.open_dialog || editor.save_as_request || editor.export_request || editor.show_help {
                    break;
                }
            }
            ctx.input_mut(|i| {
                let mut index = 0;
                i.events.retain(|_| { let keep = !consumed.contains(&index); index += 1; keep });
            });
        }
    }
    state.sync(app);
    editor.autoplay_editor = state;
}

pub fn tracks_panel(ui: &mut egui::Ui, editor: &mut EditorState, app: &mut PadPreviewState, enabled: bool, raw_wheel: f32) {
    let mut state = std::mem::take(&mut editor.autoplay_editor);
    let enabled = enabled && !state.curve_editing();
    ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::surface(crate::theme::BG_CANVAS));
    crate::ui::style::enabled_ui(ui, enabled, |ui| {
        let editable = enabled && app.has_engine() && !state.suppress_pointer;
        if state.raw_events { tracks_ui(ui, &mut state, editor, app, editable, raw_wheel); }
        else { clips::draw(ui, &mut state, editor, app, editable, raw_wheel); }
    });
    editor.autoplay_editor = state;
}

pub fn inspector_panel(ui: &mut egui::Ui, editor: &mut EditorState, app: &mut PadPreviewState, enabled: bool) {
    let mut state = std::mem::take(&mut editor.autoplay_editor);
    crate::ui::style::enabled_ui(ui, enabled && !state.suppress_pointer, |ui| inspector::draw(ui, &mut state, editor, app, enabled && app.has_engine()));
    editor.autoplay_editor = state;
}

fn drawing_tools(ui: &mut egui::Ui, state: &mut AutoplayEditor, editor: &mut EditorState, app: &mut PadPreviewState) {
    use crate::ui::style::icon;
    use lucide_icons::Icon;
    if state.curve_editing() { return; }
    if icon(ui, Icon::PenTool, "New Pad curve", false).clicked() {
        state.new_curve(editor, app);
        return;
    }
    if !state.raw_events && icon(ui, Icon::MousePointer2, "Select (V)", !state.draw_mode).clicked() {
        state.finish_inspector(editor, &mut app.chart);
        state.draw_mode = false;
    }
    ui.scope(|ui| {
        if !state.raw_events {
            if icon(ui, Icon::Pencil, "Draw in main timeline or Pad (D)", state.draw_mode).clicked() {
                state.finish_inspector(editor, &mut app.chart);
                if state.draw_mode { state.draw_mode = false; } else { state.start_drawing(); }
            }
            let selected = state.selected.filter(|_| !state.draw_mode)
                .and_then(|(row, index)| state.tracks.get(row)?.events.get(index));
            let hold = selected.map_or(state.new_hold, |event| event_kind(event) != 0);
            let click_label = if selected.is_some() { "Convert selected event or interval to Click" } else { "Create Click events" };
            let hold_label = if selected.is_some() { "Convert selected event or interval to Hold" } else { "Create Hold intervals" };
            if icon(ui, Icon::CircleDot, click_label, !hold).clicked() {
                state.choose_hold(editor, &mut app.chart, false);
            }
            if icon(ui, Icon::RectangleHorizontal, hold_label, hold).clicked() {
                state.choose_hold(editor, &mut app.chart, true);
            }
        }
        if icon(ui, Icon::Plus, "Add at playhead", false).clicked() {
            let time = app.timeline_time();
            state.add_at_playhead(editor, &mut app.chart, time);
        }
    });
    if ui.available_width() > 45.0 {
        ui.monospace(&state.new_zone).on_hover_text("Sensor for new events; choose directly in Pad");
    }
}

pub fn header(ui: &mut egui::Ui, editor: &mut EditorState, app: &mut PadPreviewState) {
    use crate::ui::style::{glyph, icon};
    use lucide_icons::Icon;
    let mut owned = std::mem::take(&mut editor.autoplay_editor);
    let state = &mut owned;
    crate::ui::style::enabled_ui(ui, app.has_engine() && !state.pointer_dragging() && !state.suppress_pointer, |ui| {
        if ui.available_width() >= 210.0 {
            drawing_tools(ui, state, editor, app);
        } else {
            ui.menu_button(glyph(Icon::Pencil, 14.0), |ui| {
                ui.horizontal(|ui| drawing_tools(ui, state, editor, app));
            }).response.on_hover_text("Event tools");
        }
    });
    if ui.available_width() > 380.0 {
        ui.add(egui::TextEdit::singleline(&mut state.filter).desired_width(80.0).hint_text("Note ID"));
    }
    ui.menu_button(glyph(Icon::ListFilter, 14.0), |ui| {
        if ui.checkbox(&mut state.raw_events, "原始事件视图").changed() {
            state.selected_pair = None;
            state.draw_mode = false;
        }
        ui.add(egui::TextEdit::singleline(&mut state.filter).desired_width(120.0).hint_text("Note ID"));
        ui.horizontal(|ui| {
            ui.label("基础轨道");
            if ui.add(egui::DragValue::new(&mut state.lane_count).range(1..=32)).changed() { state.lanes_dirty = true; }
        });
        ui.horizontal(|ui| {
            ui.label("视图起点 (s)");
            if ui.add(egui::DragValue::new(&mut state.view_start).range(-86400.0..=86400.0).speed(0.1)).changed() {
                state.center_requested = false;
                state.view_changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("缩放");
            let mut zoom = state.pixels_per_second;
            if ui.add(egui::DragValue::new(&mut zoom)
                .range(crate::timeline::geometry::MIN_PPS..=crate::timeline::geometry::MAX_PPS)
                .speed(2.0).max_decimals(0).suffix(" px/s")).changed() {
                set_anchored_zoom(state, zoom, state.plot_width * 0.5);
            }
        });
        if ui.button("显示完整谱面").clicked() { state.locate(Some((0.0, app.song_duration()))); ui.close_menu(); }
        let selection = state.selection_span();
        if ui.add_enabled(selection.is_some(), egui::Button::new("显示所选片段")).clicked() {
            state.locate(selection); ui.close_menu();
        }
    }).response.on_hover_text("筛选与轨道");
    if ui.available_width() > 75.0 {
        if icon(ui, Icon::LocateFixed, "回到播放线", false).clicked() { state.return_current(); }
        if icon(ui, Icon::MoveRight, "跟随播放线", state.follow).clicked() { state.toggle_follow(); }
    }
    let mut zoom = state.pixels_per_second;
    if ui.available_width() > 140.0 {
        if ui.add(egui::DragValue::new(&mut zoom).range(crate::timeline::geometry::MIN_PPS..=crate::timeline::geometry::MAX_PPS)
            .speed(2.0).max_decimals(0).suffix(" px/s")).changed() {
            set_anchored_zoom(state, zoom, state.plot_width * 0.5);
        }
    }
    if ui.available_width() > 25.0 { crate::ui::toolbar::autoplay_grid_controls(ui, editor); }
    editor.autoplay_editor = owned;
}


fn set_anchored_zoom(editor: &mut AutoplayEditor, pps: f32, offset: f32) {
    let time = editor.view_start + offset / editor.pixels_per_second;
    editor.pixels_per_second = pps.clamp(
        crate::timeline::geometry::MIN_PPS,
        crate::timeline::geometry::MAX_PPS,
    );
    editor.view_start = time - offset / editor.pixels_per_second;
    editor.view_changed = true;
    editor.center_requested = false;
    editor.follow_motion.reset();
}

struct Viewport {
    ruler: Rect,
    left: f32,
    grid: Vec<(f32, bool)>,
}

fn grid_stroke(major: bool) -> Stroke {
    Stroke::new(1.0_f32, if major { crate::theme::GRID_BEAT } else { crate::theme::GRID_LINE })
}

fn paint_grid(painter: &egui::Painter, plot: Rect, grid: &[(f32, bool)]) {
    for &(x, major) in grid {
        painter.vline(x, plot.y_range(), grid_stroke(major));
    }
}

fn viewport(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    raw_wheel: f32,
) -> Viewport {
    let now = app.timeline_time();
    let navigation = ui.is_enabled() && (!state.dragging() || state.pan_x.is_some()) && !state.suppress_pointer;
    let bounds = ui.max_rect().intersect(ui.clip_rect());
    let over_timeline = ui.rect_contains_pointer(bounds);
    let width = ui.available_width();
    let (ruler, response) = ui.allocate_exact_size(Vec2::new(width, 26.0),
        if navigation { Sense::click_and_drag() } else { Sense::hover() });
    let left = ruler.left() + HEADER.min(width * 0.4);
    let plot_width = ruler.right() - left;
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let middle = egui::PointerButton::Middle;
    let primary = egui::PointerButton::Primary;
    let was_panning = state.pan_x.is_some();
    if !navigation || !ui.input(|i| i.focused && i.pointer.button_down(middle)) {
        state.pan_x = None;
    } else if !editor.scrubbing && editor.loop_drag.is_none() {
        if over_timeline && ui.input(|i| i.pointer.button_pressed(middle) && !i.pointer.primary_down()) {
            state.pan_x = pointer.map(|p| p.x);
            state.center_requested = false;
        }
        if let (Some(origin), Some(pos)) = (state.pan_x, pointer) {
            let step = crate::timeline::input::rate_pan_step(pos.x - origin, ui.input(|i| i.stable_dt));
            state.view_start += step / state.pixels_per_second;
            ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);
            if step != 0.0 { ui.ctx().request_repaint(); }
        }
    }
    let panning = was_panning || state.pan_x.is_some();
    let command = ui.input(|i| {
        i.modifiers.command || i.modifiers.ctrl || i.modifiers.mac_cmd
            || i.events.iter().any(|event| matches!(event,
                egui::Event::MouseWheel { modifiers, .. }
                    if modifiers.command || modifiers.ctrl || modifiers.mac_cmd
            ))
    });
    let shift = ui.input(|i| i.modifiers.shift || i.events.iter().any(|event| matches!(event,
        egui::Event::MouseWheel { modifiers, .. } if modifiers.shift
    )));
    let pinch = if over_timeline { crate::gesture::take() } else { 0.0 };
    let wheel_zoom = command && over_timeline && raw_wheel.abs() > f32::EPSILON;
    let zoom_delta = if pinch.abs() > 1e-6 { pinch } else if wheel_zoom { raw_wheel * 0.05 } else { 0.0 };
    if wheel_zoom || pinch.abs() > 1e-6 {
        ui.input_mut(|i| {
            i.smooth_scroll_delta = Vec2::ZERO;
            i.raw_scroll_delta = Vec2::ZERO;
        });
    }
    let mut following = false;
    if !state.dragging() && !panning && editor.loop_drag.is_none() && !editor.scrubbing {
        let pan = if navigation && over_timeline && !command && pinch.abs() <= 1e-6 {
            ui.input(|i| {
                let delta = i.raw_scroll_delta;
                if shift && delta.y.abs() > delta.x.abs() { delta.y } else { delta.x }
            })
        } else { 0.0 };
        if let Some((a, b)) = state.fit_requested.take() {
            let margin = ((b - a) * 0.1).max(0.2);
            state.pixels_per_second = (plot_width / (b - a + margin * 2.0).max(0.1))
                .clamp(crate::timeline::geometry::MIN_PPS, crate::timeline::geometry::MAX_PPS);
            state.view_start = (a + b) * 0.5 - plot_width * 0.5 / state.pixels_per_second;
            state.center_requested = false;
        } else if navigation && zoom_delta != 0.0 {
            let zoom = crate::timeline::input::zoomed_pps(state.pixels_per_second, zoom_delta, 1.0);
            let offset = pointer.map_or(plot_width * 0.5, |pos| (pos.x - left).clamp(0.0, plot_width));
            set_anchored_zoom(state, zoom, offset);
        } else if pan != 0.0 {
            state.center_requested = false;
            state.view_start -= pan / state.pixels_per_second;
            ui.input_mut(|i| {
                i.raw_scroll_delta.x = 0.0;
                i.smooth_scroll_delta.x = 0.0;
                if shift { i.raw_scroll_delta.y = 0.0; i.smooth_scroll_delta.y = 0.0; }
            });
        } else if !state.view_changed {
            if state.plot_width > 0.0 && (plot_width - state.plot_width).abs() > 0.5 {
                state.view_start += (state.plot_width - plot_width) * 0.5 / state.pixels_per_second;
                state.follow_motion.reset();
            }
            if !ui.input(|i| i.pointer.primary_down() || i.pointer.button_released(primary)) {
                if state.center_requested {
                    state.view_start = now - plot_width * 0.5 / state.pixels_per_second;
                    state.center_requested = false;
                } else if state.follow && matches!(app.mode, Mode::Playing | Mode::Recording) {
                    state.view_start = state.follow_motion.advance(state.view_start, now,
                        plot_width / state.pixels_per_second, ui.input(|i| i.stable_dt), app.play_speed);
                    following = true;
                }
            }
        }
    }
    if !following { state.follow_motion.reset(); }
    state.view_changed = false;
    state.plot_width = plot_width;
    state.repack();
    if state.raw_events { state.layout_events(state.pixels_per_second); }
    let pps = state.pixels_per_second;
    let snap = editor.autoplay_snap_enabled;
    let division = editor.autoplay_snap_division.max(1) as f32;
    let looping = crate::timeline::overview::loop_gesture(ui, &response, editor,
        navigation && !panning && response.interact_pointer_pos().is_some_and(|pos| pos.x >= left), |x| {
            let time = state.view_start + (x.max(left) - left) / pps;
            if snap {
                let measure = (secs_to_measure(time, &app.chart.bpms) * division).round() / division;
                measure_to_secs(measure, &app.chart.bpms).max(0.0)
            } else { time.max(0.0) }
        }, |time| left + (time - state.view_start) * pps);
    if looping { state.center_requested = false; }
    let dragging = response.dragged_by(primary);
    let released = response.drag_stopped_by(primary);
    let started_in_plot = ui.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos())
        .is_some_and(|pos| pos.x >= left && pos.x <= ruler.right());
    if navigation && !panning && !looping && started_in_plot
        && (response.clicked() || dragging || released)
        && let Some(pos) = response.interact_pointer_pos()
    {
        state.center_requested = false;
        // Continue moving while held outside, even when no new pointer event arrives.
        let x = pos.x.clamp(left, ruler.right());
        if dragging && pos.x != x {
            state.view_start += crate::timeline::input::scroll_step(pos.x - x, ui.input(|i| i.stable_dt)) / pps;
            ui.ctx().request_repaint();
        }
        editor.pending_seek = Some(snap_time(state.view_start + (x - left) / pps, editor, &app.chart).max(0.0));
        editor.scrubbing = dragging;
    }
    if released { editor.scrubbing = false; }
    let view = state.view_start;
    let x_of = |time: f32| left + (time - view) * pps;
    let painter = ui.painter().with_clip_rect(ruler);
    painter.rect_filled(ruler, 0.0, crate::theme::surface(crate::theme::BG_OVERVIEW));
    crate::timeline::overview::paint_loop(&painter, Rect::from_min_max(Pos2::new(left, ruler.top()), ruler.max), editor, x_of, true);
    let start = secs_to_measure(view, &app.chart.bpms).floor().max(1.0);
    let end = secs_to_measure(view + (ruler.right() - left) / pps, &app.chart.bpms)
        .ceil()
        .min(start + 1000.0);
    let mut grid = Vec::new();
    for measure in start as u32..=end as u32 {
        let time = measure_to_secs(measure as f32, &app.chart.bpms);
        let x = x_of(time);
        painter.text(
            Pos2::new(x + 3.0, ruler.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{measure}"),
            egui::FontId::monospace(11.0),
            Color32::GRAY,
        );
        let division = editor.autoplay_snap_division.max(1);
        let step =
            (measure_to_secs(measure as f32 + 1.0, &app.chart.bpms) - time) * pps / division as f32;
        // Thin dense grids to a coarser divisor without losing all subdivisions.
        let stride = (1..=division)
            .find(|stride| division % *stride == 0 && step * *stride as f32 >= 5.0)
            .unwrap_or(division);
        for sub in (0..division).step_by(stride as usize) {
            let x = x_of(measure_to_secs(
                measure as f32 + sub as f32 / division as f32,
                &app.chart.bpms,
            ));
            if x >= left && x <= ruler.right() {
                grid.push((x, sub == 0));
                let height = if sub == 0 { 8.0 } else { 4.0 };
                painter.vline(x, (ruler.bottom() - height)..=ruler.bottom(), grid_stroke(sub == 0));
            }
        }
    }
    Viewport { ruler, left, grid }
}

fn tracks_ui(ui: &mut egui::Ui, state: &mut AutoplayEditor, editor: &mut EditorState,
    app: &mut PadPreviewState, editable: bool, raw_wheel: f32) {
    let Viewport { ruler, left, grid } = viewport(ui, state, editor, app, raw_wheel);
    let width = ruler.width();
    let now = editor.pending_seek.unwrap_or_else(|| app.timeline_time());
    let pps = state.pixels_per_second;
    let view = state.view_start;
    let x_of = |time: f32| left + (time - view) * pps;
    let mut reset_row = None;
    let filter = state.filter.trim().to_lowercase();
    let visible_start = ((view - 8.0 / pps) * 1e6).floor() as i64;
    let visible_end = ((view + (ruler.right() - left + 8.0) / pps) * 1e6).ceil() as i64;
    let draft_row = state
        .drag
        .as_ref()
        .map(|drag| drag.row)
        .or_else(|| state.inspector_edit.as_ref().map(|edit| edit.row));
    egui::ScrollArea::vertical()
        .id_salt("autoplay_tracks")
        .drag_to_scroll(state.pan_x.is_none())
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for lane in 0..state.lanes.len() {
                let height = state.lane_heights.get(lane).copied().unwrap_or(ROW);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let header = Rect::from_min_max(rect.min, Pos2::new(left, rect.bottom()));
                let plot = Rect::from_min_max(Pos2::new(left, rect.top()), rect.max);
                let background = ui.interact(plot.intersect(ui.clip_rect()),
                    ui.id().with(("autoplay_empty", lane)), if editable && !state.pointer_dragging() { Sense::click() } else { Sense::hover() });
                if background.clicked() && editable && state.drag.is_none() {
                    state.finish_inspector(editor, &mut app.chart);
                    state.clear_selection();
                }
                let painter = ui.painter().clone();
                if lane % 2 != 0 {
                    painter.rect_filled(rect, 0.0, crate::theme::surface(Color32::from_white_alpha(3)));
                }
                painter.with_clip_rect(header.shrink(6.0)).text(
                    Pos2::new(header.left() + 8.0, header.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("轨道 {}", lane + 1),
                    egui::FontId::monospace(11.0),
                    Color32::LIGHT_GRAY,
                );
                painter.line_segment(
                    [header.right_top(), header.right_bottom()],
                    Stroke::new(1.0_f32, Color32::from_gray(55)),
                );
                let p = painter.with_clip_rect(plot.intersect(ui.clip_rect()));
                crate::timeline::overview::paint_loop(&p, plot, editor, x_of, false);
                paint_grid(&p, plot, &grid);
                // Groups in a lane are ordered, disjoint spans; skip offscreen notes.
                let groups = &state.lanes[lane];
                let first = groups.partition_point(|group| group.end < visible_start);
                let last = groups.partition_point(|group| group.start <= visible_end);
                let mut visible = groups[first..last].to_vec();
                if let Some(draft) =
                    draft_row.and_then(|row| groups.iter().find(|group| group.row == row).copied())
                {
                    visible.retain(|group| group.row != draft.row);
                    visible.push(draft);
                }
                for (position, group) in visible.iter().enumerate() {
                    let row = group.row;
                    let track = &state.tracks[row];
                    if !filter.is_empty()
                        && !track.label.to_lowercase().contains(&filter)
                        && !track.key.note_id.to_string().contains(&filter)
                    {
                        continue;
                    }
                    let (start, end) = if draft_row == Some(row) {
                        event_span(track)
                    } else {
                        (group.start, group.end)
                    };
                    if end < visible_start || start > visible_end {
                        continue;
                    }
                    let start_x = x_of(start as f32 / 1e6);
                    let end_x = x_of(end as f32 / 1e6);
                    let event_layout = state.layouts[row].clone();
                    let clip = Rect::from_min_max(
                        Pos2::new((start_x - 7.0).clamp(left, rect.right()), rect.top() + 17.0),
                        Pos2::new((end_x + 7.0).clamp(left, rect.right()),
                            rect.top() + 17.0 + event_layout.count as f32 * EVENT_ROW),
                    );
                    let selected = state.selected_row == Some(row);
                    let color = track
                        .events
                        .first()
                        .map(event_color)
                        .unwrap_or(Color32::GRAY);
                    p.rect_filled(
                        clip,
                        3.0,
                        Color32::from_gray(if selected { 55 } else { 46 }),
                    );
                    p.vline(clip.left() + 1.0, clip.y_range(), Stroke::new(2.0_f32, color));
                    if selected {
                        p.rect_stroke(
                            clip,
                            3.0,
                            Stroke::new(1.0_f32, crate::theme::ACCENT_BLUE),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let title_left = (start_x - 5.0).max(left + 3.0);
                    let title_right = visible
                        .get(position + 1)
                        .map(|next| x_of(next.start as f32 / 1e6) - 8.0)
                        .unwrap_or(rect.right())
                        .min(rect.right());
                    if title_right - title_left >= 22.0 {
                        let title = if end_x - start_x >= 90.0 || track.events.is_empty() {
                            track.label.clone()
                        } else {
                            track.events.first().map(event_zone).unwrap_or_default()
                        };
                        p.with_clip_rect(
                            Rect::from_min_max(
                                Pos2::new(title_left, rect.top()),
                                Pos2::new(title_right, rect.top() + 17.0),
                            )
                            .intersect(p.clip_rect()),
                        )
                        .text(
                            Pos2::new(title_left, rect.top() + 8.0),
                            egui::Align2::LEFT_CENTER,
                            title,
                            egui::FontId::monospace(10.0),
                            if selected {
                                Color32::WHITE
                            } else {
                                Color32::LIGHT_GRAY
                            },
                        );
                    }
                    let response = ui
                        .interact(
                            clip.intersect(ui.clip_rect()),
                            ui.id().with(("event_group", row)),
                            if editable && !state.pointer_dragging() { Sense::click() } else { Sense::hover() },
                        )
                        .on_hover_text(format!("{} / Note {}", track.label, track.key.note_id));
                    if response.clicked() && state.drag.is_none() {
                        state.finish_inspector(editor, &mut app.chart);
                        state.selected_row = Some(row);
                        state.selected = None;
                        state.selected_pair = None;
                    }
                    if editable && !state.pointer_dragging() {
                        response.context_menu(|ui| {
                            if ui.button("恢复此组默认事件").clicked() {
                                reset_row = Some(row);
                                ui.close_menu();
                            }
                        });
                    }
                    let event_plot = Rect::from_min_max(
                        Pos2::new(left, clip.top()),
                        Pos2::new(rect.right(), clip.bottom()),
                    );
                    paint_holds(&p, &state.tracks[row].events, &event_layout, event_plot, left, view, pps);
                    if state.tracks[row].events.is_empty() {
                        let center = Pos2::new(start_x, event_plot.center().y);
                        for dy in [-4.0, 4.0] {
                            p.line_segment(
                                [center + Vec2::new(-4.0, dy), center + Vec2::new(4.0, -dy)],
                                Stroke::new(1.0_f32, Color32::GRAY),
                            );
                        }
                    }
                    if state.tracks[row].authored {
                        p.circle_filled(
                            Pos2::new(clip.left() + 4.0, clip.bottom() - 4.0),
                            2.5,
                            Color32::from_rgb(84, 209, 173),
                        );
                    }
                    for index in 0..state.tracks[row].events.len() {
                        let event = state.tracks[row].events[index].clone();
                        let x = x_of(timed_input_tp(&event) as f32 / 1e6);
                        if x < left - 8.0 || x > rect.right() + 8.0 {
                            continue;
                        }
                        let center = event_center(&event, index, &event_layout, event_plot.top(), x);
                        let hit = Rect::from_center_size(center, Vec2::splat(14.0))
                            .intersect(plot).intersect(ui.clip_rect());
                        let event_response = ui.interact(
                            hit,
                            ui.id().with(("event", row, index)),
                            if editable && !state.pointer_dragging() {
                                Sense::click_and_drag()
                            } else {
                                Sense::hover()
                            },
                        );
                        let color = if state.selected == Some((row, index)) {
                            Color32::WHITE
                        } else {
                            event_color(&event)
                        };
                        match event_kind(&event) {
                            0 => {
                                p.circle_filled(center, 5.0, color);
                            }
                            1 => {
                                p.rect_filled(
                                    Rect::from_center_size(center, Vec2::new(7.0, 16.0)),
                                    1.0,
                                    color,
                                );
                            }
                            _ => {
                                p.circle_stroke(center, 5.0, Stroke::new(2.0_f32, color));
                            }
                        }
                        event_response.clone().on_hover_text(format!("{} / {}",
                            state.tracks[row].label, event_label(index, &event)));
                        if event_response.clicked() || event_response.drag_started_by(egui::PointerButton::Primary) {
                            state.finish_inspector(editor, &mut app.chart);
                            state.selected = Some((row, index));
                            state.selected_row = Some(row);
                            state.selected_pair = None;
                        }
                        if event_response.drag_started_by(egui::PointerButton::Primary) {
                            state.center_requested = false;
                            state.drag = Some(Drag {
                                row,
                                index,
                                original: event.clone(),
                                origin_x: event_response
                                    .interact_pointer_pos()
                                    .map(|p| p.x - event_response.drag_delta().x)
                                    .unwrap_or(x),
                            });
                        }
                        if editable && !state.pointer_dragging() {
                            event_response.context_menu(|ui| {
                                if ui.button("恢复此组默认事件").clicked() {
                                    reset_row = Some(row);
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                }
                let px = x_of(now);
                p.line_segment(
                    [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 73, 92)),
                );
            }
        });
    // Captured drags finish even after their marker or row leaves the viewport.
    if let Some(drag) = state.drag.clone() {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let time = timed_input_tp(&drag.original) as f32 / 1e6 + (pos.x - drag.origin_x) / pps;
            let time = snap_time(time, editor, &app.chart);
            if let Some(event) = state
                .tracks
                .get_mut(drag.row)
                .and_then(|t| t.events.get_mut(drag.index))
            {
                *event = autoplay_edit::with_time(&drag.original, (time * 1e6).round() as i64);
            }
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            state.drag = None;
            if let Some(track) = state.tracks.get(drag.row) {
                state.commit(editor, &mut app.chart, drag.row, track.events.clone());
            }
        }
    }
    if let Some(row) = reset_row {
        state.finish_inspector(editor, &mut app.chart);
        state.reset(editor, &mut app.chart, row);
    }
}

fn snap_time(time: f32, editor: &EditorState, chart: &ChartDoc) -> f32 {
    measure_to_secs(
        crate::timeline::geometry::snap(
            secs_to_measure(time, &chart.bpms),
            editor.autoplay_snap_enabled,
            editor.autoplay_snap_division,
        ),
        &chart.bpms,
    )
}

fn default_hold_end(start: i64, editor: &EditorState, chart: &ChartDoc) -> i64 {
    let measure = secs_to_measure(start as f32 / 1e6, &chart.bpms);
    let end = (measure_to_secs(measure + 1.0 / editor.autoplay_snap_division.max(1) as f32, &chart.bpms) * 1e6).round() as i64;
    end.max(start.saturating_add(1))
}

fn event_kind(event: &TimedInputEvent) -> u8 {
    match event {
        TimedInputEvent::ButtonClick { .. } | TimedInputEvent::SensorClick { .. } => 0,
        TimedInputEvent::ButtonHold { is_down, .. }
        | TimedInputEvent::SensorHold { is_down, .. } => {
            if *is_down {
                1
            } else {
                2
            }
        }
    }
}

fn event_label(index: usize, event: &TimedInputEvent) -> String {
    let kind = match event_kind(event) {
        0 => "点击",
        1 => "按下",
        _ => "松开",
    };
    format!(
        "{} · {} {} · {:.6} s",
        index + 1,
        event_zone(event),
        kind,
        timed_input_tp(event) as f64 / 1e6
    )
}

fn event_zone(event: &TimedInputEvent) -> String {
    match event {
        TimedInputEvent::ButtonClick { zone, .. } | TimedInputEvent::ButtonHold { zone, .. } => {
            format!("{zone:?}")
        }
        TimedInputEvent::SensorClick { area, .. } | TimedInputEvent::SensorHold { area, .. } => {
            format!("{area:?}")
        }
    }
}

fn zone_names() -> Vec<String> {
    let mut names: Vec<_> = (1..=8).map(|i| format!("K{i}")).collect();
    for ring in ["A", "B", "D", "E"] {
        names.extend((1..=8).map(|i| format!("{ring}{i}")));
    }
    names.push("C".into());
    names
}

fn make_event(zone: &str, kind: u8, tp: i64) -> TimedInputEvent {
    let button = match zone {
        "K1" => Some(ButtonZone::K1),
        "K2" => Some(ButtonZone::K2),
        "K3" => Some(ButtonZone::K3),
        "K4" => Some(ButtonZone::K4),
        "K5" => Some(ButtonZone::K5),
        "K6" => Some(ButtonZone::K6),
        "K7" => Some(ButtonZone::K7),
        "K8" => Some(ButtonZone::K8),
        _ => None,
    };
    if let Some(zone) = button {
        return if kind == 0 {
            TimedInputEvent::ButtonClick { tp, zone }
        } else {
            TimedInputEvent::ButtonHold {
                tp,
                zone,
                is_down: kind == 1,
            }
        };
    }
    let area = match zone {
        "A2" => SensorArea::A2,
        "A3" => SensorArea::A3,
        "A4" => SensorArea::A4,
        "A5" => SensorArea::A5,
        "A6" => SensorArea::A6,
        "A7" => SensorArea::A7,
        "A8" => SensorArea::A8,
        "B1" => SensorArea::B1,
        "B2" => SensorArea::B2,
        "B3" => SensorArea::B3,
        "B4" => SensorArea::B4,
        "B5" => SensorArea::B5,
        "B6" => SensorArea::B6,
        "B7" => SensorArea::B7,
        "B8" => SensorArea::B8,
        "D1" => SensorArea::D1,
        "D2" => SensorArea::D2,
        "D3" => SensorArea::D3,
        "D4" => SensorArea::D4,
        "D5" => SensorArea::D5,
        "D6" => SensorArea::D6,
        "D7" => SensorArea::D7,
        "D8" => SensorArea::D8,
        "E1" => SensorArea::E1,
        "E2" => SensorArea::E2,
        "E3" => SensorArea::E3,
        "E4" => SensorArea::E4,
        "E5" => SensorArea::E5,
        "E6" => SensorArea::E6,
        "E7" => SensorArea::E7,
        "E8" => SensorArea::E8,
        "C" => SensorArea::C,
        _ => SensorArea::A1,
    };
    if kind == 0 {
        TimedInputEvent::SensorClick { tp, area }
    } else {
        TimedInputEvent::SensorHold {
            tp,
            area,
            is_down: kind == 1,
        }
    }
}

fn event_color(event: &TimedInputEvent) -> Color32 {
    match event {
        TimedInputEvent::ButtonClick { .. } | TimedInputEvent::ButtonHold { .. } => {
            Color32::from_rgb(242, 180, 86)
        }
        _ => Color32::from_rgb(76, 202, 180),
    }
}

fn paint_holds(
    p: &egui::Painter,
    events: &[TimedInputEvent],
    layout: &EventLayout,
    plot: Rect,
    left: f32,
    view: f32,
    pps: f32,
) {
    let x_of = |index: usize| left + (timed_input_tp(&events[index]) as f32 / 1e6 - view) * pps;
    for &(press, release) in &layout.holds {
        let start = event_center(&events[press], press, layout, plot.top(), x_of(press));
        let y = start.y + 8.0;
        let end = release.map(|index| event_center(&events[index], index, layout, plot.top(), x_of(index)));
        let right = end.map_or(plot.right(), |end| end.x);
        if release.is_none() && start.x > plot.right() {
            continue;
        }
        let color = event_color(&events[press]);
        p.rect_filled(Rect::from_min_max(Pos2::new(start.x.min(right), y - 2.0), Pos2::new(start.x.max(right), y + 2.0)),
            0.0, color.gamma_multiply(0.4));
        p.line_segment([start, Pos2::new(start.x, y)], Stroke::new(1.0_f32, color));
        if let Some(end) = end {
            p.line_segment([Pos2::new(end.x, y), end], Stroke::new(1.0_f32, color));
        }
        if right - start.x > 42.0 {
            p.text(Pos2::new(start.x + 7.0, y - 3.0), egui::Align2::LEFT_BOTTOM,
                event_zone(&events[press]), egui::FontId::monospace(9.0), color);
        }
    }
}
