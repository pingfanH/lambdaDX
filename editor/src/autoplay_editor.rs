//! Reusable timeline lanes with note-owned event groups.

use crate::app::autoplay_edit::{self, AutoplayKey, AutoplayOverride};
use crate::app::types::{ChartDoc, Mode, measure_to_secs, note_secs, secs_to_measure};
use crate::core::types::{AutoplayPlan, ButtonZone, SensorArea, TimedInputEvent};
use crate::player::{engine::timed_input_tp, state::PadPreviewState};
use crate::state::EditorState;
use crate::ui::layout::Layout;
use egui_macroquad::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[cfg(all(test, feature = "backend-lean"))]
mod tests;

const HEADER: f32 = 96.0;
const ROW: f32 = 52.0;
const EVENT_ROW: f32 = 32.0;
const DEFAULT_LANES: usize = 4;

#[derive(Debug, Clone)]
pub struct Track {
    pub key: AutoplayKey,
    pub label: String,
    pub head: i64,
    pub defaults: Vec<TimedInputEvent>,
    pub events: Vec<TimedInputEvent>,
    authored: bool,
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
    pub selected: Option<(usize, usize)>,
    pub selected_row: Option<usize>,
    drag: Option<Drag>,
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
}

impl Default for AutoplayEditor {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            plan: None,
            inner_screen: false,
            overrides: Vec::new(),
            selected: None,
            selected_row: None,
            drag: None,
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
        }
    }
}

impl AutoplayEditor {
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.inspector_edit.is_some()
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
        if changed {
            let Some(engine) = &app.judge_engine else {
                return;
            };
            let old_key = self
                .selected_row
                .and_then(|row| self.tracks.get(row))
                .map(|track| track.key.clone());
            let old_event = self.selected.and_then(|(row, index)| {
                self.tracks.get(row).map(|track| (track.key.clone(), index))
            });
            let chart = app.playback_chart();
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
            self.selected_row =
                old_key.and_then(|key| self.tracks.iter().position(|track| track.key == key));
            self.selected = old_event.and_then(|(key, index)| {
                self.tracks
                    .iter()
                    .position(|track| track.key == key)
                    .map(|row| (row, index))
            });
            self.plan = Some(Arc::clone(plan));
            self.inner_screen = app.params.autoplay_inner_screen;
        }
        if changed || self.overrides != app.chart.autoplay_overrides {
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
            self.lanes_dirty = true;
        }
        if self.selected.is_some_and(|(r, i)| {
            self.tracks
                .get(r)
                .is_none_or(|track| i >= track.events.len())
        }) {
            self.selected = None;
        }
        self.repack();
    }

    fn repack(&mut self) {
        if self.lanes_dirty && !self.dragging() {
            self.lanes = pack_lanes(&self.tracks, self.lane_count);
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

    fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_row = None;
    }

    pub fn commit(
        &mut self,
        editor: &mut EditorState,
        chart: &mut ChartDoc,
        row: usize,
        events: Vec<TimedInputEvent>,
    ) {
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
        editor.push_history(chart);
        chart
            .autoplay_overrides
            .retain(|item| item.key != track.key);
        if events != track.defaults {
            chart.autoplay_overrides.push(AutoplayOverride {
                key: track.key.clone(),
                events: relative,
            });
        }
        track.authored = events != track.defaults;
        track.events = events;
        self.lanes_dirty = true;
    }

    pub fn reset(&mut self, editor: &mut EditorState, chart: &mut ChartDoc, row: usize) {
        let Some(track) = self.tracks.get_mut(row) else {
            return;
        };
        if !chart
            .autoplay_overrides
            .iter()
            .any(|item| item.key == track.key)
        {
            return;
        }
        editor.push_history(chart);
        chart
            .autoplay_overrides
            .retain(|item| item.key != track.key);
        track.events = track.defaults.clone();
        track.authored = false;
        self.selected = None;
        self.lanes_dirty = true;
    }

    pub fn delete(&mut self, editor: &mut EditorState, chart: &mut ChartDoc) {
        if let Some((row, index)) = self.selected {
            if let Some(track) = self.tracks.get(row) {
                let mut events = track.events.clone();
                if index < events.len() {
                    events.remove(index);
                    self.commit(editor, chart, row, events);
                }
            }
            self.selected = None;
        }
    }

    pub fn cancel(&mut self) {
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
    let mut order: Vec<_> = (0..events.len()).collect();
    order.sort_by_key(|&index| timed_input_tp(&events[index]));
    let mut spans: Vec<(i64, i64, usize, Option<usize>)> = Vec::new();
    let mut held: HashMap<String, Vec<usize>> = HashMap::new();
    for index in order {
        let event = &events[index];
        let time = timed_input_tp(event);
        let zone = event_zone(event);
        match event_kind(event) {
            1 => {
                held.entry(zone).or_default().push(spans.len());
                spans.push((time, i64::MAX, index, None));
            }
            2 => {
                if let Some(span) = held.get_mut(&zone).and_then(|starts| {
                    if starts.is_empty() { None } else { Some(starts.remove(0)) }
                }) {
                    spans[span].1 = time;
                    spans[span].3 = Some(index);
                } else {
                    spans.push((time, time, index, None));
                }
            }
            _ => spans.push((time, time, index, None)),
        }
    }
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
        if event_kind(&events[index]) == 1 {
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

pub fn draw(
    ctx: &egui::Context,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    layout: &mut Layout,
    enabled: bool,
    raw_wheel: f32,
) {
    let mut state = std::mem::take(&mut editor.autoplay_editor);
    if !enabled {
        state.cancel();
    }
    state.sync(app);
    layout.sidebar = Rect::NOTHING;
    layout.timeline = Rect::NOTHING;
    layout.tools = Rect::NOTHING;
    layout.overview = Rect::NOTHING;
    if enabled {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            state.cancel();
            state.clear_selection();
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        if !ctx.wants_keyboard_input() && !ctx.memory(|m| m.any_popup_open())
            && !state.dragging() && !editor.keyboard.ime {
            crate::keyboard::panels::load_bindings(editor);
            let events = ctx.input(|i| i.events.clone());
            for event in events {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                else {
                    continue;
                };
                match crate::keyboard::panels::bound_command(editor, key, modifiers) {
                    Some("play") => app.toggle_play(),
                    Some("help") => editor.show_help = true,
                    Some("save") => crate::io::save_chart(editor, &app.chart),
                    Some("save_as") => editor.save_as_request = true,
                    Some("export") => editor.export_request = true,
                    Some("open") => editor.open_dialog = true,
                    Some("undo") => editor.undo(&mut app.chart),
                    Some("redo") => editor.redo(&mut app.chart),
                    Some("snap") => editor.snap_enabled = !editor.snap_enabled,
                    _ if !modifiers.command && !modifiers.ctrl && !modifiers.alt => match key {
                        egui::Key::Delete | egui::Key::Backspace if !modifiers.shift => {
                            state.delete(editor, &mut app.chart)
                        }
                        egui::Key::Questionmark | egui::Key::Slash
                            if modifiers.shift || key == egui::Key::Questionmark =>
                        {
                            editor.show_help = true
                        }
                        _ => {}
                    },
                    _ => {}
                }
                if editor.show_help {
                    break;
                }
            }
        }
    }
    state.sync(app);
    let editable = enabled && app.has_engine();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let top_h = (rect.height() * 0.44)
                .clamp(140.0, 400.0)
                .min((rect.height() - 130.0).max(100.0));
            let inspector_w = (rect.width() * 0.38)
                .clamp(220.0, 400.0)
                .min(rect.width() * 0.55);
            layout.preview = Rect::from_min_max(
                rect.min,
                egui::pos2(rect.right() - inspector_w, rect.top() + top_h),
            );
            let inspector = Rect::from_min_max(
                egui::pos2(layout.preview.right(), rect.top()),
                egui::pos2(rect.right(), rect.top() + top_h),
            )
            .shrink(10.0);
            ui.scope_builder(egui::UiBuilder::new().max_rect(inspector), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("autoplay_inspector")
                    .show(ui, |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            inspector_ui(ui, &mut state, editor, app, editable)
                        });
                    });
            });
            let bottom = Rect::from_min_max(egui::pos2(rect.left(), rect.top() + top_h), rect.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    tracks_ui(ui, &mut state, editor, app, editable, raw_wheel)
                });
            });
        });
    editor.autoplay_editor = state;
}

fn inspector_ui(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    editable: bool,
) {
    ui.horizontal(|ui| {
        ui.heading("Autoplay");
        if ui
            .button(if app.mode == Mode::Playing {
                "⏸"
            } else {
                "▶"
            })
            .on_hover_text("播放 / 暂停")
            .clicked()
        {
            app.toggle_play();
        }
        ui.label(format!("{:.3} s", app.timeline_time()));
    });
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in [(2, "模拟触摸"), (3, "直传核心")] {
            if ui
                .selectable_value(&mut editor.autoplay_mode, mode, label)
                .changed()
            {
                editor.mode_dirty = true;
            }
        }
    });
    let mut inner_screen = app.params.autoplay_inner_screen;
    if ui.checkbox(&mut inner_screen, "内屏模式 (K1-K8 → A1-A8)").changed() {
        state.finish_inspector(editor, &mut app.chart);
        app.params.autoplay_inner_screen = inner_screen;
        crate::app::params::set(app.params.clone());
        state.sync(app);
    }
    ui.separator();
    ui.add_enabled_ui(editable, |ui| {
        let mut owner = state.selected_row;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("autoplay_owner")
                .selected_text(owner.and_then(|row| state.tracks.get(row))
                    .map(|track| track.label.as_str()).unwrap_or("事件组"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut owner, None, "无");
                    for (row, track) in state.tracks.iter().enumerate() {
                        ui.selectable_value(&mut owner, Some(row), &track.label);
                    }
                });
            if ui.add_enabled(owner.is_some(), egui::Button::new("×"))
                .on_hover_text("取消选择").clicked() {
                owner = None;
            }
        });
        if owner != state.selected_row {
            state.finish_inspector(editor, &mut app.chart);
            state.selected_row = owner;
            state.selected = None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("新建");
            egui::ComboBox::from_id_salt("autoplay_new_zone")
                .selected_text(&state.new_zone)
                .width(48.0)
                .show_ui(ui, |ui| {
                    for zone in zone_names() {
                        ui.selectable_value(&mut state.new_zone, zone.clone(), zone);
                    }
                });
            for (kind, label) in [(0, "点击"), (1, "按下"), (2, "松开")] {
                ui.selectable_value(&mut state.new_kind, kind, label);
            }
            if ui.add_enabled(owner.is_some(), egui::Button::new("+"))
                .on_hover_text("在红线处添加事件").clicked() {
                if let Some(row) = owner {
                    state.finish_inspector(editor, &mut app.chart);
                    let tp = (snap_time(app.timeline_time(), editor, &app.chart) * 1e6).round() as i64;
                    let mut events = state.tracks[row].events.clone();
                    events.push(make_event(&state.new_zone, state.new_kind, tp));
                    state.commit(editor, &mut app.chart, row, events);
                    state.selected = None;
                }
            }
        });
        if let Some(row) = state.selected_row.filter(|row| *row < state.tracks.len()) {
            ui.label(format!("Note {}", state.tracks[row].key.note_id));
            if let Some(lane) = state
                .lanes
                .iter()
                .position(|groups| groups.iter().any(|group| group.row == row))
            {
                ui.label(format!("轨道 {}", lane + 1));
            }
            ui.horizontal(|ui| {
                if ui.button("↺").on_hover_text("恢复此组默认事件").clicked() {
                    state.finish_inspector(editor, &mut app.chart);
                    state.reset(editor, &mut app.chart, row);
                }
                if ui.button("跳转").clicked() {
                    editor.pending_seek = Some(state.tracks[row].head as f32 / 1e6);
                    editor.view_start = (state.tracks[row].head as f32 / 1e6 - 0.5).max(0.0);
                }
            });
            let mut choice = state
                .selected
                .filter(|(selected_row, _)| *selected_row == row)
                .map(|(_, index)| index);
            egui::ComboBox::from_id_salt("autoplay_event_list")
                .selected_text(
                    choice
                        .map(|index| event_label(index, &state.tracks[row].events[index]))
                        .unwrap_or_else(|| "事件".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut choice, None, "无");
                    for (index, event) in state.tracks[row].events.iter().enumerate() {
                        ui.selectable_value(&mut choice, Some(index), event_label(index, event));
                    }
                });
            if choice
                != state
                    .selected
                    .filter(|(selected_row, _)| *selected_row == row)
                    .map(|(_, index)| index)
            {
                state.finish_inspector(editor, &mut app.chart);
                state.selected = choice.map(|index| (row, index));
            }
        }
        if let Some((row, index)) = state.selected.filter(|(row, index)| {
            state
                .tracks
                .get(*row)
                .is_some_and(|t| *index < t.events.len())
        }) {
            let original = state.tracks[row].events[index].clone();
            let mut time = timed_input_tp(&original) as f64 / 1e6;
            let mut kind = event_kind(&original);
            let mut zone = event_zone(&original);
            let response = ui
                .horizontal(|ui| {
                    ui.label("时间 (s)");
                    ui.add(egui::DragValue::new(&mut time).speed(0.001).max_decimals(6))
                })
                .inner;
            ui.horizontal(|ui| {
                ui.label("动作");
                ui.selectable_value(&mut kind, 0, "点击");
                ui.selectable_value(&mut kind, 1, "按下");
                ui.selectable_value(&mut kind, 2, "松开");
            });
            egui::ComboBox::from_id_salt("event_zone")
                .selected_text(&zone)
                .show_ui(ui, |ui| {
                    for candidate in zone_names() {
                        ui.selectable_value(&mut zone, candidate.clone(), candidate);
                    }
                });
            let event = make_event(&zone, kind, (time * 1e6).round() as i64);
            if event != original {
                if response.dragged() || response.has_focus() || state.inspector_edit.is_some() {
                    state.inspector_edit.get_or_insert_with(|| InspectorEdit {
                        row,
                        original: state.tracks[row].events.clone(),
                    });
                    state.tracks[row].events[index] = event;
                } else {
                    let mut events = state.tracks[row].events.clone();
                    events[index] = event;
                    state.commit(editor, &mut app.chart, row, events);
                }
            }
            if response.drag_stopped()
                || response.lost_focus()
                || state.inspector_edit.is_some() && !response.dragged() && !response.has_focus()
            {
                state.finish_inspector(editor, &mut app.chart);
            }
            if ui.button("×").on_hover_text("删除事件").clicked() {
                state.finish_inspector(editor, &mut app.chart);
                state.delete(editor, &mut app.chart);
            }
        }
    });
    if !app.has_engine() {
        ui.label(if app.status.is_empty() {
            "核心更新中"
        } else {
            &app.status
        });
    }
    if let Some(score) = &app.core_score {
        ui.separator();
        ui.label(format!(
            "Combo {}   DX {} / {}",
            score.combo,
            score.dx_score_remaining(),
            score.max_dx_score
        ));
    }
}

fn set_centered_zoom(editor: &mut EditorState, pps: f32, playhead: f32, plot_width: f32) {
    editor.pixels_per_second = pps.clamp(
        crate::timeline::geometry::MIN_PPS,
        crate::timeline::geometry::MAX_PPS,
    );
    editor.view_start = playhead - plot_width.max(0.0) * 0.5 / editor.pixels_per_second;
}

fn tracks_ui(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    editable: bool,
    raw_wheel: f32,
) {
    let now = app.timeline_time();
    let mut zoom = editor.pixels_per_second;
    let mut center_playhead = false;
    let command = ui.input(|i| {
        i.modifiers.command || i.modifiers.ctrl || i.modifiers.mac_cmd
            || i.events.iter().any(|event| matches!(event,
                egui::Event::MouseWheel { modifiers, .. }
                    if modifiers.command || modifiers.ctrl || modifiers.mac_cmd
            ))
    });
    let over_timeline = ui.rect_contains_pointer(ui.max_rect());
    let pinch = crate::gesture::take();
    let wheel_zoom = command && over_timeline && raw_wheel.abs() > f32::EPSILON;
    if ui.is_enabled() && !state.dragging() {
        let delta = if pinch.abs() > 1e-6 { pinch } else if wheel_zoom { raw_wheel * 0.05 } else { 0.0 };
        if delta != 0.0 {
            zoom = crate::timeline::input::zoomed_pps(zoom, delta, 1.0);
            center_playhead = true;
        }
    }
    if wheel_zoom || pinch.abs() > 1e-6 {
        ui.input_mut(|i| {
            i.smooth_scroll_delta = Vec2::ZERO;
            i.raw_scroll_delta = Vec2::ZERO;
        });
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.strong("事件轨道");
        ui.add(
            egui::TextEdit::singleline(&mut state.filter)
                .desired_width(115.0)
                .hint_text("Note ID"),
        );
        ui.label("基础轨道");
        if ui
            .add(egui::DragValue::new(&mut state.lane_count).range(1..=32))
            .changed()
        {
            state.lanes_dirty = true;
        }
        ui.checkbox(&mut editor.snap_enabled, "吸附");
        ui.label("1/");
        ui.add(egui::DragValue::new(&mut editor.snap_division).range(1..=256));
        ui.add(
            egui::Slider::new(
                &mut zoom,
                crate::timeline::geometry::MIN_PPS..=crate::timeline::geometry::MAX_PPS,
            )
            .logarithmic(true)
            .text("缩放")
            .show_value(false),
        );
        if ui.button("当前").clicked() {
            center_playhead = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("时间 (s)");
        ui.add(
            egui::DragValue::new(&mut editor.view_start)
                .range(-86400.0..=86400.0)
                .speed(0.1),
        );
    });
    let width = ui.available_width();
    let (ruler, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click_and_drag());
    let left = ruler.left() + HEADER.min(width * 0.4);
    if !state.dragging() {
        if center_playhead || zoom != editor.pixels_per_second
            || matches!(app.mode, Mode::Playing | Mode::Recording)
        {
            set_centered_zoom(editor, zoom, now, ruler.right() - left);
        } else if ui.is_enabled() && over_timeline && !command && pinch.abs() <= 1e-6 {
            // Smoothed scroll can outlive a zoom and move the freshly centered view.
            editor.view_start -= ui.input(|i| i.raw_scroll_delta.x) / editor.pixels_per_second;
        }
    }
    state.repack();
    state.layout_events(editor.pixels_per_second);
    let pps = editor.pixels_per_second;
    let view = editor.view_start;
    let x_of = |time: f32| left + (time - view) * pps;
    let painter = ui.painter().with_clip_rect(ruler);
    painter.rect_filled(ruler, 0.0, Color32::from_rgb(32, 33, 35));
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
        let division = editor.snap_division.max(1);
        let step =
            (measure_to_secs(measure as f32 + 1.0, &app.chart.bpms) - time) * pps / division as f32;
        for sub in 0..division {
            if sub != 0 && step < 5.0 {
                continue;
            }
            let x = x_of(measure_to_secs(
                measure as f32 + sub as f32 / division as f32,
                &app.chart.bpms,
            ));
            if x >= left && x <= ruler.right() {
                grid.push((x, sub == 0));
            }
        }
    }
    if (response.clicked() || response.dragged())
        && response
            .interact_pointer_pos()
            .is_some_and(|pos| pos.x >= left)
    {
        let pos = response.interact_pointer_pos().unwrap();
        let time = snap_time(view + (pos.x - left) / pps, editor, &app.chart).max(0.0);
        editor.pending_seek = Some(time);
        editor.scrubbing = response.dragged();
    }
    if response.drag_stopped() {
        editor.scrubbing = false;
    }
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
                    ui.id().with(("autoplay_empty", lane)), Sense::click());
                if background.clicked() && state.drag.is_none() {
                    state.finish_inspector(editor, &mut app.chart);
                    state.clear_selection();
                }
                let painter = ui.painter().clone();
                let bg = if lane % 2 == 0 {
                    Color32::from_rgb(26, 27, 29)
                } else {
                    Color32::from_rgb(30, 31, 33)
                };
                painter.rect_filled(rect, 0.0, bg);
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
                for &(x, major) in &grid {
                    p.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(1.0_f32, Color32::from_gray(if major { 65 } else { 39 })),
                    );
                }
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
                        color.gamma_multiply(if selected { 0.3 } else { 0.12 }),
                    );
                    if selected {
                        p.rect_stroke(
                            clip,
                            3.0,
                            Stroke::new(1.0_f32, color),
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
                            Sense::click(),
                        )
                        .on_hover_text(format!("{} / Note {}", track.label, track.key.note_id));
                    if response.clicked() && state.drag.is_none() {
                        state.finish_inspector(editor, &mut app.chart);
                        state.selected_row = Some(row);
                        state.selected = None;
                    }
                    if editable {
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
                            if editable {
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
                        if event_response.clicked() || event_response.drag_started() {
                            state.finish_inspector(editor, &mut app.chart);
                            state.selected = Some((row, index));
                            state.selected_row = Some(row);
                        }
                        if event_response.drag_started() {
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
                        if editable {
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
            editor.snap_enabled,
            editor.snap_division,
        ),
        &chart.bpms,
    )
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
