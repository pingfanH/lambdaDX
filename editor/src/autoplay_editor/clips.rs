//! Clip overview and sensor intervals. Drafts never mutate the chart.

use super::*;
use crate::theme::{ACCENT_BLUE, BG_CANVAS, TEXT, TEXT_DIM};
use crate::ui::style::{glyph, icon};
use lucide_icons::Icon;

#[derive(Debug, Clone)]
pub(super) struct Clip {
    key: AutoplayKey,
    pub rows: Vec<usize>,
    start: i64,
    end: i64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ClipLayout {
    pub clips: Vec<Clip>,
    lanes: Vec<Vec<usize>>,
}

impl ClipLayout {
    pub fn build(tracks: &[Track], minimum: usize, previous: &Self) -> Self {
        let mut groups = HashMap::new();
        let mut clips: Vec<Clip> = Vec::new();
        for (row, track) in tracks.iter().enumerate() {
            let key = (
                track.key.note_id,
                track.key.role,
                track.key.slide,
                track.key.track,
            );
            let index = *groups.entry(key).or_insert_with(|| {
                let mut key = track.key.clone();
                key.segment = 0;
                clips.push(Clip {
                    key,
                    rows: Vec::new(),
                    start: i64::MAX,
                    end: i64::MIN,
                });
                clips.len() - 1
            });
            let (start, end) = event_span(track);
            let clip = &mut clips[index];
            clip.rows.push(row);
            clip.start = clip.start.min(start);
            clip.end = clip.end.max(end);
        }
        clips.sort_by_key(|clip| (clip.start, clip.rows[0]));
        let preferred: HashMap<_, _> = previous
            .lanes
            .iter()
            .enumerate()
            .flat_map(|(lane, clips)| {
                clips
                    .iter()
                    .map(move |&i| (previous.clips[i].key.clone(), lane))
            })
            .collect();
        let mut lanes = vec![Vec::new(); minimum.max(1).max(previous.lanes.len())];
        let mut ends = vec![i64::MIN; lanes.len()];
        for (index, clip) in clips.iter().enumerate() {
            let lane = preferred
                .get(&clip.key)
                .copied()
                .filter(|&lane| ends[lane] < clip.start)
                .or_else(|| ends.iter().position(|end| *end < clip.start))
                .unwrap_or_else(|| {
                    lanes.push(Vec::new());
                    ends.push(i64::MIN);
                    lanes.len() - 1
                });
            lanes[lane].push(index);
            ends[lane] = clip.end;
        }
        while lanes.len() > minimum.max(1) && lanes.last().is_some_and(Vec::is_empty) {
            lanes.pop();
        }
        Self { clips, lanes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Interval {
    pub press: usize,
    pub release: Option<usize>,
}

pub(super) fn intervals(events: &[TimedInputEvent]) -> Vec<Interval> {
    let mut order: Vec<_> = (0..events.len()).collect();
    order.sort_by_key(|&i| timed_input_tp(&events[i]));
    let mut active: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<Interval> = Vec::new();
    for index in order {
        let zone = event_zone(&events[index]);
        if event_kind(&events[index]) == 2 {
            if let Some(open) = active.remove(&zone) {
                result[open].release = Some(index);
                continue;
            }
        } else if event_kind(&events[index]) == 1 {
            // Repeated Press is retained as a raw event, not paired across state transitions.
            active.entry(zone).or_insert(result.len());
        }
        result.push(Interval {
            press: index,
            release: None,
        });
    }
    result
}

#[derive(Debug, Clone)]
pub(super) struct Gesture {
    pub originals: Vec<(usize, Vec<TimedInputEvent>)>,
    pub selection: Option<(usize, usize)>,
    pub pair: Option<usize>,
    pub row: Option<usize>,
    pub created_row: Option<usize>,
    targets: Vec<(usize, usize, TimedInputEvent)>,
    origin_x: f32,
    anchor: i64,
    minimum: i64,
    maximum: i64,
    keyboard: bool,
    offset: f32,
}

fn begin(
    state: &mut AutoplayEditor,
    targets: Vec<(usize, usize)>,
    origin_x: f32,
    keyboard: bool,
    duplicate: bool,
    limits: (i64, i64),
) {
    if targets.is_empty()
        || state.clip_drag.is_some()
        || targets.iter().any(|&(row, index)| {
            state
                .tracks
                .get(row)
                .is_none_or(|track| index >= track.events.len())
        })
    {
        return;
    }
    let selection = state.selected;
    let pair = state.selected_pair;
    let row = state.selected_row;
    let mut originals = Vec::new();
    let mut moved = Vec::new();
    for (row, index) in targets {
        if !originals.iter().any(|(r, _)| *r == row) {
            originals.push((row, state.tracks[row].events.clone()));
        }
        let event = state.tracks[row].events[index].clone();
        let target = if duplicate {
            let target = state.tracks[row].events.len();
            state.tracks[row].events.push(event.clone());
            target
        } else {
            index
        };
        moved.push((row, target, event));
    }
    let anchor = moved
        .iter()
        .map(|(_, _, event)| timed_input_tp(event))
        .min()
        .unwrap();
    if duplicate && state.selected.is_some() {
        state.selected = Some((moved[0].0, moved[0].1));
        state.selected_pair = moved.get(1).map(|(_, index, _)| *index);
    }
    state.clip_drag = Some(Gesture {
        originals,
        selection,
        pair,
        row,
        created_row: None,
        targets: moved,
        origin_x,
        anchor,
        minimum: limits.0,
        maximum: limits.1,
        keyboard,
        offset: 0.0,
    });
    state.center_requested = false;
}

pub(super) fn begin_selected(state: &mut AutoplayEditor, ctx: &egui::Context, duplicate: bool) {
    state.validate_selection();
    let limits = if let Some((row, index)) = state
        .selected
        .filter(|_| state.selected_pair.is_none() && !duplicate)
    {
        endpoint_limits(&state.tracks[row].events, index)
    } else {
        (i64::MIN, i64::MAX)
    };
    let targets = if let Some((row, index)) = state.selected {
        let mut targets = vec![(row, index)];
        if let Some(pair) = state.selected_pair {
            targets.push((row, pair));
        }
        targets
    } else if let Some(row) = state.selected_row {
        state
            .clip_rows(row)
            .into_iter()
            .flat_map(|row| (0..state.tracks[row].events.len()).map(move |i| (row, i)))
            .collect()
    } else {
        return;
    };
    let x = ctx.input(|i| i.pointer.latest_pos().map_or(0.0, |p| p.x));
    begin(state, targets, x, true, duplicate, limits);
}

pub(super) fn endpoint_limits(events: &[TimedInputEvent], index: usize) -> (i64, i64) {
    for span in intervals(events) {
        if let Some(release) = span.release {
            let duration = timed_input_tp(&events[release])
                .saturating_sub(timed_input_tp(&events[span.press]));
            if span.press == index {
                return (i64::MIN, duration);
            }
            if release == index {
                return (-duration, i64::MAX);
            }
        }
    }
    (i64::MIN, i64::MAX)
}

pub(super) fn navigate(state: &mut AutoplayEditor, reverse: bool) {
    if let Some(row) = state.detail_row {
        let mut ends: Vec<_> = state
            .clip_rows(row)
            .into_iter()
            .flat_map(|row| (0..state.tracks[row].events.len()).map(move |i| (row, i)))
            .collect();
        ends.sort_by_key(|&(row, i)| (timed_input_tp(&state.tracks[row].events[i]), row, i));
        if ends.is_empty() {
            return;
        }
        let index = state
            .selected
            .and_then(|selected| ends.iter().position(|end| *end == selected));
        let index = match index {
            Some(i) if reverse => (i + ends.len() - 1) % ends.len(),
            Some(i) => (i + 1) % ends.len(),
            None if reverse => ends.len() - 1,
            None => 0,
        };
        state.selected = Some(ends[index]);
        state.selected_row = Some(ends[index].0);
        state.selected_pair = None;
        let time = timed_input_tp(&state.tracks[ends[index].0].events[ends[index].1]) as f32 / 1e6;
        let width = state.plot_width / state.pixels_per_second;
        if time < state.view_start || time > state.view_start + width {
            state.view_start = time - width * 0.5;
        }
        state.center_requested = false;
        state.view_changed = true;
        return;
    }
    let clips = &state.clip_layout.clips;
    if clips.is_empty() {
        return;
    }
    let index = state
        .selected_row
        .and_then(|row| clips.iter().position(|c| c.rows.contains(&row)));
    let index = match index {
        Some(i) if reverse => (i + clips.len() - 1) % clips.len(),
        Some(i) => (i + 1) % clips.len(),
        None if reverse => clips.len() - 1,
        None => 0,
    };
    state.selected_row = Some(clips[index].rows[0]);
    state.selected = None;
    state.selected_pair = None;
    let time = clips[index].start as f32 / 1e6;
    let width = state.plot_width / state.pixels_per_second;
    if time < state.view_start || time > state.view_start + width {
        state.view_start = time - width * 0.5;
    }
    state.center_requested = false;
    state.view_changed = true;
}

fn finish_gesture(
    ui: &egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
) {
    let Some(mut gesture) = state.clip_drag.take() else {
        return;
    };
    let mut confirm = false;
    ui.input_mut(|i| {
        if gesture.keyboard {
            let step = measure_to_secs(
                secs_to_measure(gesture.anchor as f32 / 1e6, &chart.bpms)
                    + 1.0 / editor.snap_division.max(1) as f32,
                &chart.bpms,
            ) - gesture.anchor as f32 / 1e6;
            i.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    let direction = match key {
                        egui::Key::ArrowLeft => -1.0,
                        egui::Key::ArrowRight => 1.0,
                        _ => 0.0,
                    };
                    gesture.offset += direction * step * if modifiers.shift { 4.0 } else { 1.0 };
                    if direction != 0.0 {
                        return false;
                    }
                }
                true
            });
            gesture.offset += i.raw_scroll_delta.y / 40.0 * step;
            i.raw_scroll_delta = Vec2::ZERO;
            i.smooth_scroll_delta = Vec2::ZERO;
            confirm = i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || i.pointer.primary_clicked();
        } else {
            confirm = !i.pointer.primary_down();
        }
    });
    {
        let x = ui.input(|i| i.pointer.latest_pos().map_or(gesture.origin_x, |p| p.x));
        let offset = (x - gesture.origin_x) / state.pixels_per_second + gesture.offset;
        let time = gesture.anchor as f32 / 1e6 + offset;
        let target = (snap_time(time, editor, chart) * 1e6).round() as i64;
        let delta = if offset.abs() <= f32::EPSILON {
            0
        } else {
            target.saturating_sub(gesture.anchor)
        }
        .clamp(gesture.minimum, gesture.maximum);
        for (row, index, event) in &gesture.targets {
            state.tracks[*row].events[*index] =
                autoplay_edit::with_time(event, timed_input_tp(event).saturating_add(delta));
        }
    }
    if confirm {
        let edits = gesture
            .originals
            .iter()
            .map(|(row, _)| (*row, state.tracks[*row].events.clone()))
            .collect();
        state.commit_many(editor, chart, edits);
        state.lanes_dirty = true;
    } else {
        state.clip_drag = Some(gesture);
    }
}

fn paint_grid(p: &egui::Painter, plot: Rect, grid: &[(f32, bool)], play_x: f32) {
    super::paint_grid(p, plot, grid);
    p.vline(
        play_x,
        plot.y_range(),
        Stroke::new(1.5_f32, crate::theme::ACCENT_RED),
    );
}

fn begin_drawing(
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    row: Option<usize>,
    zone: &str,
    origin: f32,
    time: f32,
) {
    state.finish_inspector(editor, chart);
    let previous_len = state.tracks.len();
    let row = row.unwrap_or_else(|| state.creation_row(chart));
    let original = state.tracks[row].events.clone();
    let index = original.len();
    let time = (snap_time(time, editor, chart) * 1e6).round() as i64;
    state.tracks[row].events.push(make_event(zone, 1, time));
    state.tracks[row].events.push(make_event(zone, 2, time));
    begin(
        state,
        vec![(row, index + 1)],
        origin,
        false,
        false,
        (0, i64::MAX),
    );
    if let Some(draft) = &mut state.clip_drag {
        draft.originals = vec![(row, original)];
        draft.created_row = (row == previous_len).then_some(row);
    }
    state.selected = Some((row, index));
    state.selected_row = Some(row);
    state.selected_pair = Some(index + 1);
    if row == previous_len {
        state.clip_layout = Arc::new(ClipLayout::build(&state.tracks, state.lane_count, &state.clip_layout));
    }
}

fn stable_events(state: &AutoplayEditor, row: usize) -> Vec<TimedInputEvent> {
    let mut events = state.tracks[row].events.clone();
    if let Some(draft) = &state.clip_drag {
        for (r, index, original) in &draft.targets {
            if *r == row {
                events[*index] = original.clone();
            }
        }
    }
    events
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    editable: bool,
    raw_wheel: f32,
) {
    let Viewport { ruler, left, grid } = viewport(ui, state, editor, app, raw_wheel);
    let width = ruler.width();
    let pps = state.pixels_per_second;
    let view = state.view_start;
    let x_of = |t: i64| left + (t as f32 / 1e6 - view) * pps;
    let play_x = left + (editor.pending_seek.unwrap_or_else(|| app.timeline_time()) - view) * pps;
    let captured = state.clip_drag.is_some();
    finish_gesture(ui, state, editor, &mut app.chart);
    let layout = Arc::clone(&state.clip_layout);
    let detail = state.detail_row.filter(|row| *row < state.tracks.len());
    let inline = state
        .selected
        .filter(|_| detail.is_none())
        .and_then(|(row, index)| {
            intervals(&stable_events(state, row))
                .into_iter()
                .find(|span| span.press == index || span.release == Some(index))
                .map(|span| (row, span))
        });
    let filter = state.filter.trim().to_lowercase();
    let visible_start = ((view - 8.0 / pps) * 1e6) as i64;
    let visible_end = ((view + (ruler.right() - left + 8.0) / pps) * 1e6) as i64;
    let height = if detail.is_some() {
        (ui.available_height() * 0.43).max(45.0)
    } else {
        ui.available_height()
    };
    egui::ScrollArea::vertical()
        .id_salt("autoplay_clips")
        .drag_to_scroll(state.pan_x.is_none())
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            ui.set_min_height(height);
            for (lane, clips) in layout.lanes.iter().enumerate() {
                let inline = inline
                    .filter(|(row, _)| clips.iter().any(|&i| layout.clips[i].rows.contains(row)));
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(width, if inline.is_some() { 68.0 } else { 35.0 }),
                    Sense::hover(),
                );
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let plot = Rect::from_min_max(Pos2::new(left, rect.top()), rect.max);
                let p = ui.painter().with_clip_rect(plot.intersect(ui.clip_rect()));
                p.rect_filled(
                    plot,
                    0.0,
                    if lane % 2 == 0 {
                        BG_CANVAS
                    } else {
                        Color32::from_gray(39)
                    },
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 9.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{:02}", lane + 1),
                    egui::FontId::monospace(11.0),
                    TEXT_DIM,
                );
                crate::timeline::overview::paint_loop(&p, plot, editor, |time| left + (time - view) * pps, false);
                paint_grid(&p, plot, &grid, play_x);
                let empty = ui.interact(
                    plot.intersect(ui.clip_rect()),
                    ui.id().with(("clip_empty", lane)),
                    if editable && !captured && !state.pointer_dragging() {
                        Sense::click_and_drag()
                    } else {
                        Sense::hover()
                    },
                );
                if empty.clicked() && editable && !captured {
                    state.finish_inspector(editor, &mut app.chart);
                    if state.draw_mode {
                        let time = view + (empty.interact_pointer_pos().unwrap().x - left) / pps;
                        state.add_at_playhead(editor, &mut app.chart, time);
                    } else {
                        state.clear_selection();
                    }
                }
                if empty.drag_started_by(egui::PointerButton::Primary)
                    && editable
                    && !captured
                    && !state.pointer_dragging()
                    && state.draw_mode
                {
                    let origin = ui
                        .input(|i| i.pointer.press_origin())
                        .unwrap_or_else(|| empty.interact_pointer_pos().unwrap())
                        .x;
                    let zone = state.new_zone.clone();
                    begin_drawing(
                        state,
                        editor,
                        &mut app.chart,
                        None,
                        &zone,
                        origin,
                        view + (origin - left) / pps,
                    );
                }
                let visible = if captured {
                    clips.as_slice()
                } else {
                    let first = clips.partition_point(|&i| layout.clips[i].end < visible_start);
                    let last = clips.partition_point(|&i| layout.clips[i].start <= visible_end);
                    &clips[first..last]
                };
                for &index in visible {
                    let clip = &layout.clips[index];
                    let row = clip.rows[0];
                    let track = &state.tracks[row];
                    if !filter.is_empty()
                        && !clip
                            .rows
                            .iter()
                            .any(|&row| state.tracks[row].label.to_lowercase().contains(&filter))
                    {
                        continue;
                    }
                    let (start, end) = if captured {
                        clip.rows
                            .iter()
                            .map(|&r| event_span(&state.tracks[r]))
                            .fold((i64::MAX, i64::MIN), |(a, b), (s, e)| (a.min(s), b.max(e)))
                    } else {
                        (clip.start, clip.end)
                    };
                    let x1 = x_of(start);
                    let x2 = if end == i64::MAX {
                        plot.right()
                    } else {
                        x_of(end)
                    };
                    let bounds = Rect::from_min_max(
                        Pos2::new(x1 - 3.0, rect.top() + 5.0),
                        Pos2::new((x2 + 3.0).max(x1 + 10.0), rect.bottom() - 5.0),
                    );
                    if !bounds.intersects(plot) {
                        continue;
                    }
                    let selected = state.selected_row.is_some_and(|r| clip.rows.contains(&r));
                    let inline_clip = inline.is_some_and(|(r, _)| clip.rows.contains(&r));
                    let color = track.events.first().map(event_color).unwrap_or(TEXT_DIM);
                    p.rect_filled(
                        bounds,
                        3.0,
                        Color32::from_gray(if selected { 66 } else { 51 }),
                    );
                    p.vline(
                        bounds.left() + 1.0,
                        bounds.y_range(),
                        Stroke::new(2.0_f32, color),
                    );
                    if selected {
                        p.rect_stroke(
                            bounds,
                            3.0,
                            Stroke::new(1.0_f32, ACCENT_BLUE),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let label = if track.key.role == 2 {
                        format!("#{} Slide {}", track.key.note_id, track.key.slide + 1)
                    } else {
                        track.label.clone()
                    };
                    if bounds.width() > 48.0 {
                        p.with_clip_rect(bounds.shrink(4.0).intersect(p.clip_rect()))
                            .text(
                                Pos2::new(
                                    bounds.left().max(left) + 5.0,
                                    if inline_clip {
                                        rect.top() + 12.0
                                    } else {
                                        bounds.center().y
                                    },
                                ),
                                egui::Align2::LEFT_CENTER,
                                &label,
                                egui::FontId::proportional(11.0),
                                TEXT,
                            );
                    }
                    let response = ui
                        .interact(
                            bounds.intersect(plot).intersect(ui.clip_rect()),
                            ui.id().with(("clip", row)),
                            if editable && !captured && !state.pointer_dragging() {
                                Sense::click_and_drag()
                            } else {
                                Sense::hover()
                            },
                        )
                        .on_hover_text(format!("{label} / {} groups", clip.rows.len()));
                    if response.clicked() || response.drag_started_by(egui::PointerButton::Primary)
                    {
                        state.finish_inspector(editor, &mut app.chart);
                        state.selected_row = Some(row);
                        if state.detail_row.is_some() || response.double_clicked() {
                            state.detail_row = Some(row);
                        }
                        state.selected = None;
                        state.selected_pair = None;
                        state.draw_mode = false;
                        if response.clicked() && clip.rows.len() == 1 {
                            let spans = intervals(&state.tracks[row].events);
                            if spans.len() == 1 {
                                state.selected = Some((row, spans[0].press));
                                state.selected_pair = spans[0].release;
                            }
                        }
                    }
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        let targets = clip
                            .rows
                            .iter()
                            .flat_map(|&row| {
                                (0..state.tracks[row].events.len()).map(move |i| (row, i))
                            })
                            .collect();
                        let origin =
                            response.interact_pointer_pos().unwrap().x - response.drag_delta().x;
                        begin(state, targets, origin, false, false, (i64::MIN, i64::MAX));
                    }
                    if editable && !captured && !state.pointer_dragging() {
                        response.context_menu(|ui| {
                            if ui.button("展开传感器").clicked() {
                                state.selected_row = Some(row);
                                state.detail_row = Some(row);
                                state.selected = None;
                                state.selected_pair = None;
                                state.draw_mode = false;
                                ui.close_menu();
                            }
                            if !state.tracks[row].key.is_standalone() && ui.button("恢复默认事件").clicked() {
                                let edits = clip
                                    .rows
                                    .iter()
                                    .map(|&row| (row, state.tracks[row].defaults.clone()))
                                    .collect();
                                state.commit_many(editor, &mut app.chart, edits);
                                ui.close_menu();
                            }
                            if ui.button(if state.tracks[row].key.is_standalone() { "删除片段" } else { "停用片段" }).clicked() {
                                state.commit_many(
                                    editor,
                                    &mut app.chart,
                                    clip.rows.iter().map(|&row| (row, Vec::new())).collect(),
                                );
                                ui.close_menu();
                            }
                        });
                    }
                }
                if let Some((row, span)) = inline {
                    let events = state.tracks[row].events.clone();
                    ui.push_id("overview_interval", |ui| {
                        draw_interval(
                            ui,
                            state,
                            editor,
                            &mut app.chart,
                            row,
                            &events,
                            span,
                            plot,
                            rect.top() + 42.0,
                            editable && !captured,
                        );
                    });
                }
                p.vline(
                    play_x,
                    plot.y_range(),
                    Stroke::new(1.5_f32, crate::theme::ACCENT_RED),
                );
            }
        });
    if let Some(row) = detail {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(glyph(Icon::AudioLines, 13.0));
            ui.add(egui::Label::new(&state.tracks[row].label).truncate());
            ui.add_enabled_ui(editable && !captured && !state.pointer_dragging(), |ui| {
                drawing_tools(ui, state, editor, app);
            });
            ui.add_enabled_ui(editable && !captured && !state.pointer_dragging(), |ui| {
                if icon(ui, Icon::X, "关闭详情", false).clicked() {
                    state.finish_inspector(editor, &mut app.chart);
                    state.detail_row = None;
                }
            });
        });
        let rows = state.clip_rows(row);
        egui::ScrollArea::vertical()
            .id_salt("autoplay_sensors")
            .drag_to_scroll(state.pan_x.is_none())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in rows {
                    sensor_rows(
                        ui,
                        state,
                        editor,
                        app,
                        row,
                        left,
                        width,
                        &grid,
                        play_x,
                        editable && !captured && !state.pointer_dragging(),
                    );
                }
            });
    }
    if !captured {
        finish_gesture(ui, state, editor, &mut app.chart);
    }
}

fn sensor_layout(events: &[TimedInputEvent], zone: &str, pps: f32) -> Vec<Vec<Interval>> {
    let indices: Vec<_> = (0..events.len())
        .filter(|&i| event_zone(&events[i]) == zone)
        .collect();
    let sensor: Vec<_> = indices.iter().map(|&i| events[i].clone()).collect();
    let layout = event_layout(&sensor, pps);
    let mut rows = vec![Vec::new(); layout.count];
    for span in intervals(&sensor) {
        rows[layout.rows[span.press]].push(Interval {
            press: indices[span.press],
            release: span.release.map(|i| indices[i]),
        });
    }
    rows
}

fn draw_interval(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    row: usize,
    events: &[TimedInputEvent],
    span: Interval,
    plot: Rect,
    y: f32,
    editable: bool,
) {
    let Some(event) = events.get(span.press) else {
        return;
    };
    if span.release.is_some_and(|i| i >= events.len()) {
        return;
    }
    let start = timed_input_tp(event);
    let end = span
        .release
        .map(|i| timed_input_tp(&events[i]))
        .unwrap_or(start);
    let x_of = |t: i64| plot.left() + (t as f32 / 1e6 - state.view_start) * state.pixels_per_second;
    let x1 = x_of(start);
    let x2 = x_of(end);
    let body = Rect::from_min_max(
        Pos2::new(x1 - 3.0, y - 6.0),
        Pos2::new((x2 + 3.0).max(x1 + 9.0), y + 6.0),
    );
    if !body.expand(10.0).intersects(plot) {
        return;
    }
    let p = ui.painter().with_clip_rect(plot.intersect(ui.clip_rect()));
    let selected = state
        .selected
        .is_some_and(|(r, i)| r == row && (i == span.press || span.release == Some(i)));
    let color = if selected {
        ACCENT_BLUE
    } else {
        event_color(&events[span.press])
    };
    if span.release.is_some() {
        p.rect_filled(body, 2.0, color.gamma_multiply(0.4));
        p.rect_stroke(
            body,
            2.0,
            Stroke::new(1.0_f32, color),
            egui::StrokeKind::Inside,
        );
    } else {
        p.circle_filled(Pos2::new(x1, y), 4.0, color);
    }
    let response = ui
        .interact(
            body.intersect(plot).intersect(ui.clip_rect()),
            ui.id().with(("interval", row, span.press)),
            if editable && !state.pointer_dragging() {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        )
        .on_hover_text(format!(
            "{} / {} / {:.6} - {:.6} s",
            state.tracks[row].label,
            event_zone(&events[span.press]),
            start as f64 / 1e6,
            end as f64 / 1e6,
        ));
    if response.clicked() || response.drag_started_by(egui::PointerButton::Primary) {
        state.finish_inspector(editor, chart);
        state.selected = Some((row, span.press));
        state.selected_row = Some(row);
        state.selected_pair = span.release;
        state.draw_mode = false;
        if response.double_clicked() {
            state.detail_row = Some(row);
        }
    }
    if response.drag_started_by(egui::PointerButton::Primary) {
        let mut targets = vec![(row, span.press)];
        if let Some(release) = span.release {
            targets.push((row, release));
        }
        begin(
            state,
            targets,
            response.interact_pointer_pos().unwrap().x - response.drag_delta().x,
            false,
            false,
            (i64::MIN, i64::MAX),
        );
    }
    if let Some(release) = span.release {
        for (index, x, press) in [(span.press, x1, true), (release, x2, false)] {
            // Keep zero-width interval endpoints separately reachable.
            let handle_y = if (x2 - x1).abs() < 28.0 {
                y + if press { -16.0 } else { 16.0 }
            } else {
                y
            };
            let handle = Rect::from_center_size(Pos2::new(x, handle_y), Vec2::new(20.0, 18.0));
            if handle_y != y {
                p.line_segment(
                    [Pos2::new(x, y), handle.center()],
                    Stroke::new(1.0_f32, color),
                );
            }
            p.rect_filled(
                Rect::from_center_size(handle.center(), Vec2::new(5.0, 9.0)),
                1.0,
                color,
            );
            let response = ui
                .interact(
                    handle.intersect(plot).intersect(ui.clip_rect()),
                    ui.id().with(("interval_end", row, index)),
                    if editable && !state.pointer_dragging() {
                        Sense::click_and_drag()
                    } else {
                        Sense::hover()
                    },
                )
                .on_hover_text(if press { "Press" } else { "Release" });
            if response.clicked() || response.drag_started_by(egui::PointerButton::Primary) {
                state.finish_inspector(editor, chart);
                state.selected = Some((row, index));
                state.selected_row = Some(row);
                state.selected_pair = None;
                state.draw_mode = false;
            }
            if response.drag_started_by(egui::PointerButton::Primary) {
                let limits = if press {
                    (i64::MIN, end.saturating_sub(start))
                } else {
                    (start.saturating_sub(end), i64::MAX)
                };
                begin(
                    state,
                    vec![(row, index)],
                    response.interact_pointer_pos().unwrap().x - response.drag_delta().x,
                    false,
                    false,
                    limits,
                );
            }
        }
    }
}

fn sensor_rows(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    row: usize,
    left: f32,
    width: f32,
    grid: &[(f32, bool)],
    play_x: f32,
    editable: bool,
) {
    let events = state.tracks[row].events.clone();
    // Keep interval pairing and row assignment fixed while an endpoint moves.
    let layout_events = stable_events(state, row);
    let mut zones: Vec<_> = events.iter().map(event_zone).collect();
    if state.draw_mode && state.selected_row == Some(row) {
        zones.push(state.new_zone.clone());
    }
    zones.sort();
    zones.dedup();
    let pps = state.pixels_per_second;
    let view = state.view_start;
    for zone in zones {
        for (lane, spans) in sensor_layout(&layout_events, &zone, pps)
            .into_iter()
            .enumerate()
        {
            let editable = editable && !state.pointer_dragging();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::hover());
            if !ui.is_rect_visible(rect) {
                continue;
            }
            let plot = Rect::from_min_max(Pos2::new(left, rect.top()), rect.max);
            let p = ui.painter().with_clip_rect(plot.intersect(ui.clip_rect()));
            p.rect_filled(plot, 0.0, BG_CANVAS);
            let label = if lane != 0 {
                String::new()
            } else if state.tracks[row].key.role == 2 {
                format!("{} {zone}", state.tracks[row].key.segment + 1)
            } else {
                zone.clone()
            };
            let label_rect = Rect::from_min_max(rect.min, Pos2::new(left, rect.bottom()));
            ui.painter()
                .with_clip_rect(label_rect.intersect(ui.clip_rect()))
                .text(
                    Pos2::new(rect.left() + 8.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    egui::FontId::monospace(11.0),
                    TEXT_DIM,
                );
            crate::timeline::overview::paint_loop(&p, plot, editor, |time| left + (time - view) * pps, false);
            paint_grid(&p, plot, grid, play_x);
            let can_draw = editable && state.draw_mode;
            let background = ui.interact(
                plot.intersect(ui.clip_rect()),
                ui.id().with(("sensor_empty", row, &zone, lane)),
                if editable {
                    Sense::click_and_drag()
                } else {
                    Sense::hover()
                },
            );
            if background.clicked() && editable {
                state.finish_inspector(editor, &mut app.chart);
                if can_draw {
                    let time = snap_time(
                        view + (background.interact_pointer_pos().unwrap().x - left) / pps,
                        editor,
                        &app.chart,
                    );
                    state.selected_row = Some(row);
                    state.drawing_owner = Some(state.tracks[row].key.clone());
                    state.new_zone = zone.clone();
                    state.add_at_playhead(editor, &mut app.chart, time);
                } else {
                    state.clear_selection();
                }
            }
            if background.drag_started_by(egui::PointerButton::Primary) && can_draw {
                let origin = ui
                    .input(|i| i.pointer.press_origin())
                    .unwrap_or_else(|| background.interact_pointer_pos().unwrap())
                    .x;
                begin_drawing(
                    state,
                    editor,
                    &mut app.chart,
                    Some(row),
                    &zone,
                    origin,
                    view + (origin - left) / pps,
                );
            }
            for span in spans {
                draw_interval(
                    ui,
                    state,
                    editor,
                    &mut app.chart,
                    row,
                    &events,
                    span,
                    plot,
                    rect.center().y,
                    editable,
                );
            }
            p.vline(
                play_x,
                plot.y_range(),
                Stroke::new(1.5_f32, crate::theme::ACCENT_RED),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensor_edit_detail_separates_close_intervals_without_splitting_pairs() {
        let events = vec![
            make_event("A1", 1, 0),
            make_event("A1", 2, 0),
            make_event("B1", 0, 0),
            make_event("A1", 1, 10_000),
            make_event("A1", 2, 20_000),
            make_event("A1", 1, 1_000_000),
            make_event("A1", 2, 1_100_000),
        ];
        let rows = sensor_layout(&events, "A1", 100.0);
        assert_eq!(
            rows,
            vec![
                vec![
                    Interval {
                        press: 0,
                        release: Some(1)
                    },
                    Interval {
                        press: 5,
                        release: Some(6)
                    }
                ],
                vec![Interval {
                    press: 3,
                    release: Some(4)
                }],
            ]
        );
        assert_eq!(
            sensor_layout(&[], "C", 100.0).len(),
            1,
            "a new sensor keeps a drawable row"
        );
    }

    #[test]
    fn pairing_preserves_zero_duration_and_unmatched_events() {
        let events = vec![
            make_event("A1", 2, 0),
            make_event("A1", 1, 1),
            make_event("A1", 1, 1),
            make_event("B1", 1, 1),
            make_event("A1", 2, 1),
        ];
        assert_eq!(
            intervals(&events),
            vec![
                Interval {
                    press: 0,
                    release: None
                },
                Interval {
                    press: 1,
                    release: Some(4)
                },
                Interval {
                    press: 2,
                    release: None
                },
                Interval {
                    press: 3,
                    release: None
                }
            ]
        );
    }
}
