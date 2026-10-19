//! Clip overview and sensor intervals. Drafts never mutate the chart.

use super::*;
use crate::core::types::timed_input_event_sort_key;
use crate::theme::{ACCENT_BLUE, BG_CANVAS, TEXT, TEXT_DIM};
use crate::ui::style::{glyph, icon};
use lucide_icons::Icon;

#[derive(Debug, Clone)]
pub(super) struct Clip {
    pub key: AutoplayKey,
    pub rows: Vec<usize>,
    start: i64,
    end: i64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ClipLayout {
    pub clips: Vec<Clip>,
    pub lanes: Vec<Vec<usize>>,
}

/// How the pane's view settings place elements on lanes.
#[derive(Debug, Clone, Copy)]
pub(super) struct LaneOptions<'a> {
    /// Fold the parts of one Slide or note back into a single clip.
    pub grouped: bool,
    /// Start Slide bodies on the second lane even when nothing blocks them.
    pub slide_new_lane: bool,
    /// Rows the user pinned to a lane with a vertical drag.
    pub pins: &'a HashMap<AutoplayKey, usize>,
    /// Conslide (note id, slide) pairs the user split into per-segment rows.
    pub split_slides: &'a [(u64, usize)],
}

impl ClipLayout {
    /// One clip per real owner by default; `grouped` folds the parts of one
    /// Slide or note back into a single clip.
    pub fn build(
        tracks: &[Track],
        minimum: usize,
        previous: &Self,
        options: LaneOptions<'_>,
    ) -> Self {
        let LaneOptions {
            grouped,
            slide_new_lane,
            pins,
            split_slides,
        } = options;
        let mut groups = HashMap::new();
        let mut slide_groups: HashMap<(u64, usize), usize> = HashMap::new();
        let mut clips: Vec<Clip> = Vec::new();
        for (row, track) in tracks.iter().enumerate() {
            let (start, end) = event_span(track);
            if !grouped {
                // A conslide is one element by default: its judged parts share
                // one box until the user splits that slide.
                if track.key.role == 2 {
                    let pair = (track.key.note_id, track.key.slide);
                    if !split_slides.contains(&pair) {
                        let index = *slide_groups.entry(pair).or_insert_with(|| {
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
                        let clip = &mut clips[index];
                        clip.rows.push(row);
                        clip.start = clip.start.min(start);
                        clip.end = clip.end.max(end);
                        continue;
                    }
                }
                clips.push(Clip {
                    key: track.key.clone(),
                    rows: vec![row],
                    start,
                    end,
                });
                continue;
            }
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
        // Everything that is not a Slide body is placed first, so a Slide that
        // shares time with a tap ends up on the lower lane.
        let mut order: Vec<usize> = (0..clips.len()).collect();
        order.sort_by_key(|&index| {
            (
                clips[index].key.role == 2,
                clips[index].start,
                clips[index].rows.first().copied().unwrap_or(0),
            )
        });
        for index in order {
            let clip = &clips[index];
            let first = usize::from(slide_new_lane && clip.key.role == 2).min(ends.len() - 1);
            let lane = pins
                .get(&clip.key)
                .copied()
                .filter(|&lane| lane < ends.len() && ends[lane] < clip.start)
                .or_else(|| {
                    preferred
                        .get(&clip.key)
                        .copied()
                        .filter(|&lane| lane >= first && ends[lane] < clip.start)
                })
                .or_else(|| (first..ends.len()).find(|&lane| ends[lane] < clip.start))
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
    // Editing keeps the authored order for events sharing a timestamp. The
    // runtime stream uses release-first ordering, but applying that here
    // changes zero-duration pairs and makes adjacent sensor intervals appear
    // to belong to different gestures.
    order.sort_by_key(|&i| crate::player::engine::timed_input_tp(&events[i]));
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
    } else if state.selected_row.is_some() || !state.selection.is_empty() {
        state
            .selected_elements()
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
    if let Some(row) = state.detail_row.filter(|_| state.sensor_detail) {
        let mut ends: Vec<_> = state
            .clip_rows(row)
            .into_iter()
            .flat_map(|row| (0..state.tracks[row].events.len()).map(move |i| (row, i)))
            .collect();
        ends.sort_by_key(|&(row, i)| {
            (
                timed_input_event_sort_key(&state.tracks[row].events[i]),
                row,
                i,
            )
        });
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
    state.curve_source = None;
    state.sensor_detail = false;
    if state.detail_row.is_some() {
        state.detail_row = state.selected_row;
    }
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
    state.lane_drag = false;
    let mut confirm = false;
    ui.input_mut(|i| {
        if gesture.keyboard {
            let step = measure_to_secs(
                secs_to_measure(gesture.anchor as f32 / 1e6, &chart.bpms)
                    + 1.0 / editor.autoplay_snap_division.max(1) as f32,
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
        state.clip_layout = Arc::new(ClipLayout::build(
            &state.tracks,
            state.lane_count,
            &state.clip_layout,
            state.lane_options(),
        ));
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
    let captured = state.clip_drag.is_some() || state.band_drag.is_some();
    finish_gesture(ui, state, editor, &mut app.chart);
    drag_band(ui, state, editor, app);
    let layout = Arc::clone(&state.clip_layout);
    let inline = state.selected.and_then(|(row, index)| {
        intervals(&stable_events(state, row))
            .into_iter()
            .find(|span| span.press == index || span.release == Some(index))
            .map(|span| (row, span))
    });
    let filter = state.filter.trim().to_lowercase();
    let visible_start = ((view - 8.0 / pps) * 1e6) as i64;
    let visible_end = ((view + (ruler.right() - left + 8.0) / pps) * 1e6) as i64;
    let height = ui.available_height();
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
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(width, ELEMENT_ROW), Sense::hover());
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let plot = Rect::from_min_max(Pos2::new(left, rect.top()), rect.max);
                let p = ui.painter().with_clip_rect(plot.intersect(ui.clip_rect()));
                if lane % 2 != 0 {
                    p.rect_filled(
                        plot,
                        0.0,
                        crate::theme::surface(Color32::from_white_alpha(3)),
                    );
                }
                ui.painter().text(
                    Pos2::new(rect.left() + 9.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{:02}", lane + 1),
                    egui::FontId::monospace(11.0),
                    TEXT_DIM,
                );
                crate::timeline::overview::paint_loop(
                    &p,
                    plot,
                    editor,
                    |time| left + (time - view) * pps,
                    false,
                );
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
                marquee(
                    ui,
                    &empty,
                    state,
                    app,
                    editable && !captured,
                    view,
                    left,
                    pps,
                );
                if editable && !captured && !state.pointer_dragging() {
                    let at = empty
                        .interact_pointer_pos()
                        .map(|pos| view + (pos.x - left) / pps);
                    empty.context_menu(|ui| {
                        let Some(time) = at else { return };
                        ui.label(format!("{:.3} s", time));
                        if ui.button("新建点击").clicked() {
                            state.new_hold = false;
                            state.clear_selection();
                            state.add_at_playhead(editor, &mut app.chart, time);
                            ui.close_menu();
                        }
                        if ui.button("新建长按").clicked() {
                            state.new_hold = true;
                            state.clear_selection();
                            state.add_at_playhead(editor, &mut app.chart, time);
                            ui.close_menu();
                        }
                        if ui.button("新建曲线").clicked() {
                            state.new_curve_at(editor, app, time);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("把播放头移到这里").clicked() {
                            editor.pending_seek = Some(snap_time(time, editor, &app.chart));
                            ui.close_menu();
                        }
                    });
                }
                let visible = if captured {
                    clips.as_slice()
                } else {
                    let first = clips.partition_point(|&i| layout.clips[i].end < visible_start);
                    let last = clips.partition_point(|&i| layout.clips[i].start <= visible_end);
                    &clips[first..last]
                };
                // A vertical element drag picks the lane under the pointer.
                if state.lane_drag {
                    if let (Some(row), Some(pos)) = (
                        state.clip_drag.as_ref().and_then(|drag| drag.row),
                        ui.input(|i| i.pointer.latest_pos()),
                    ) {
                        if rect.contains(pos) {
                            state.pin_lane(row, lane);
                        }
                    }
                }
                let mut grips = Vec::new();
                for &index in visible {
                    let clip = &layout.clips[index];
                    let row = clip.rows[0];
                    if !filter.is_empty()
                        && !clip
                            .rows
                            .iter()
                            .any(|&row| state.tracks[row].label.to_lowercase().contains(&filter))
                    {
                        continue;
                    }
                    let (label, color) = {
                        let track = &state.tracks[row];
                        let label = if track.key.role == 2 {
                            format!(
                                "#{} Slide {}.{}",
                                track.key.note_id,
                                track.key.slide + 1,
                                track.key.segment + 1
                            )
                        } else {
                            track.label.clone()
                        };
                        (
                            label,
                            track.events.first().map(event_color).unwrap_or(TEXT_DIM),
                        )
                    };
                    let curve = state.element_curve(row, app);
                    let (start, end) = state.element_span(row, app);
                    // A Slide sharing its time with a tap drops a little, so
                    // both boxes stay readable and their edges stay reachable.
                    let (start_us, end_us) =
                        ((start as f64 * 1e6) as i64, (end as f64 * 1e6) as i64);
                    let lowered = curve.is_some()
                        && clips.iter().any(|&other| {
                            other != index
                                && layout.clips[other].key.role != 2
                                && layout.clips[other].end > start_us
                                && layout.clips[other].start < end_us
                        });
                    let x1 = left + (start - view) * pps;
                    let x2 = left + (end - view) * pps;
                    let top = rect.top() + 4.0 + if lowered { SLIDE_ROW_OFFSET } else { 0.0 };
                    let bounds = Rect::from_min_max(
                        Pos2::new(x1 - 3.0, top),
                        Pos2::new((x2 + 3.0).max(x1 + 14.0), rect.bottom() - 4.0),
                    );
                    if !bounds.intersects(plot) {
                        continue;
                    }
                    let selected = state.row_selected(row);
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
                            Stroke::new(
                                if state.selected_row == Some(row) {
                                    1.4_f32
                                } else {
                                    1.0_f32
                                },
                                ACCENT_BLUE,
                            ),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if let Some(curve) = &curve {
                        curves::band(ui, state, editor, app, row, curve, bounds.shrink(2.0), left);
                        if bounds.width() > 88.0 {
                            p.with_clip_rect(bounds.shrink(4.0).intersect(p.clip_rect()))
                                .text(
                                    Pos2::new(bounds.left() + 42.0, bounds.center().y - 8.0),
                                    egui::Align2::LEFT_CENTER,
                                    &label,
                                    egui::FontId::proportional(11.0),
                                    TEXT,
                                );
                        }
                    } else {
                        // Sensor elements keep their inline intervals; the
                        // selected one additionally exposes its endpoints.
                        for span in intervals(&state.tracks[row].events) {
                            let events = &state.tracks[row].events;
                            let a = x_of(timed_input_tp(&events[span.press]));
                            let b = span
                                .release
                                .map(|i| x_of(timed_input_tp(&events[i])))
                                .unwrap_or(a);
                            let bar = Rect::from_min_max(
                                Pos2::new(a, bounds.center().y - 3.0),
                                Pos2::new(b.max(a + 1.0), bounds.center().y + 3.0),
                            );
                            p.rect_filled(bar, 1.0, event_color(&events[span.press]));
                        }
                        if bounds.width() > 48.0 {
                            p.with_clip_rect(bounds.shrink(4.0).intersect(p.clip_rect()))
                                .text(
                                    Pos2::new(bounds.left().max(left) + 5.0, bounds.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &label,
                                    egui::FontId::proportional(11.0),
                                    TEXT,
                                );
                        }
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
                        .on_hover_text(format!(
                            "{label} / {} events",
                            state.tracks[row].events.len()
                        ));
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        let targets = clip
                            .rows
                            .iter()
                            .flat_map(|&row| {
                                (0..state.tracks[row].events.len()).map(move |i| (row, i))
                            })
                            .collect();
                        state.finish_inspector(editor, &mut app.chart);
                        if !state.row_selected(row) {
                            state.select_clip(row, app);
                        }
                        state.selected = None;
                        state.selected_pair = None;
                        state.draw_mode = false;
                        let origin =
                            response.interact_pointer_pos().unwrap().x - response.drag_delta().x;
                        state.lane_drag = true;
                        begin(state, targets, origin, false, false, (i64::MIN, i64::MAX));
                    } else if response.clicked() {
                        let additive = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                        state.finish_inspector(editor, &mut app.chart);
                        if additive {
                            state.toggle_clip(editor, row, app);
                        } else {
                            state.select_clip(row, app);
                            if state.detail_row.is_some() {
                                state.detail_row = Some(row);
                            }
                            state.selected = None;
                            state.selected_pair = None;
                            state.draw_mode = false;
                            if clip.rows.len() == 1 && !state.curve_view(row, &app.chart) {
                                let spans = intervals(&state.tracks[row].events);
                                if spans.len() == 1 {
                                    state.selected = Some((row, spans[0].press));
                                    state.selected_pair = spans[0].release;
                                }
                            }
                            if response.double_clicked() && state.curve_view(row, &app.chart) {
                                let source = state.selected_curve_row().unwrap_or(row);
                                state.edit_curve(editor, app, source, false);
                            }
                        }
                    }
                    // Resizing: the left grip shifts the element, the right one
                    // changes how long it lasts.
                    grips.push((row, bounds));
                    if response.secondary_clicked() && !state.row_selected(row) {
                        // The menu acts on the element it was opened on.
                        state.finish_inspector(editor, &mut app.chart);
                        state.select_clip(row, app);
                    }
                    if editable && !captured && !state.pointer_dragging() {
                        response.context_menu(|ui| {
                            if state.curve_candidate(row, &app.chart)
                                && ui.button("编辑曲线 (E / 双击)").clicked()
                            {
                                state.edit_curve(editor, app, row, false);
                                ui.close_menu();
                            }
                            if ui
                                .button(if state.sensor_detail {
                                    "收起传感器"
                                } else {
                                    "展开传感器"
                                })
                                .clicked()
                            {
                                state.sensor_detail = !state.sensor_detail;
                                state.selected_row = Some(row);
                                state.detail_row = Some(row);
                                state.selected = None;
                                state.selected_pair = None;
                                state.draw_mode = false;
                                ui.close_menu();
                            }
                            if ui.button("隐藏所选元素").clicked() {
                                state.hide_elements(editor, &mut app.chart);
                                ui.close_menu();
                            }
                            if clip.key.role == 2 {
                                if clip.rows.len() > 1 {
                                    if ui.button("分离为单段").clicked() {
                                        state.set_slide_split(
                                            clip.key.note_id,
                                            clip.key.slide,
                                            true,
                                        );
                                        ui.close_menu();
                                    }
                                } else if ui.button("合并该 Slide 各段").clicked() {
                                    state.set_slide_split(clip.key.note_id, clip.key.slide, false);
                                    ui.close_menu();
                                }
                            }
                            if ui
                                .button("自动排轨")
                                .on_hover_text("取消手动指定的轨道")
                                .clicked()
                            {
                                state.clear_lane_pins();
                                ui.close_menu();
                            }
                            if !state.tracks[row].key.is_standalone()
                                && ui.button("恢复默认事件").clicked()
                            {
                                state.reset_rows(editor, &mut app.chart, &clip.rows);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(if state.tracks[row].key.is_standalone() {
                                    "删除片段"
                                } else {
                                    "删除所选元素"
                                })
                                .clicked()
                            {
                                state.delete_elements(editor, &mut app.chart);
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
                            rect.center().y,
                            editable && !captured,
                        );
                    });
                }
                p.vline(
                    play_x,
                    plot.y_range(),
                    Stroke::new(1.5_f32, crate::theme::ACCENT_RED),
                );
                // Grips are registered after every box in this lane, so an
                // element that sits next to another cannot swallow its edge
                // drag with its own box hit area.
                if editable && !captured && !state.pointer_dragging() {
                    for (row, bounds) in grips {
                        for (edge, grip_x) in [
                            (BandEdge::Start, bounds.left()),
                            (BandEdge::End, bounds.right()),
                        ] {
                            let grip = Rect::from_min_max(
                                Pos2::new(grip_x - 4.0, bounds.top()),
                                Pos2::new(grip_x + 4.0, bounds.bottom()),
                            );
                            let response = ui
                                .interact(
                                    grip.intersect(plot).intersect(ui.clip_rect()),
                                    ui.id().with(("band_grip", row, edge == BandEdge::End)),
                                    Sense::click_and_drag(),
                                )
                                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                                .on_hover_text(if edge == BandEdge::Start {
                                    "拖动起点"
                                } else {
                                    "拖动时长"
                                });
                            if response.drag_started_by(egui::PointerButton::Primary) {
                                let origin = ui
                                    .input(|i| i.pointer.press_origin())
                                    .or(response.interact_pointer_pos())
                                    .map_or(grip_x, |p| p.x);
                                begin_band(
                                    state,
                                    editor,
                                    app,
                                    row,
                                    edge,
                                    origin,
                                    view + (origin - left) / pps,
                                );
                            }
                        }
                    }
                }
                // An explicit sensor expansion stays in this lane instead of
                // opening a separate view under the timeline.
                let expand = clips
                    .iter()
                    .map(|&index| layout.clips[index].rows[0])
                    .find(|&row| {
                        state.detail_row == Some(row)
                            && state.sensor_detail
                            && !state.curve_view(row, &app.chart)
                    });
                if let Some(row) = expand {
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
                        editable && !captured,
                    );
                }
            }
        });
    if !captured {
        finish_gesture(ui, state, editor, &mut app.chart);
    }
}

/// Height of one element row in the overview.
const ELEMENT_ROW: f32 = 48.0;

/// How far a Slide drops inside its lane when it shares time with a tap.
const SLIDE_ROW_OFFSET: f32 = 6.0;

/// Which end of an element box is being dragged: the left end moves the whole
/// element, the right end changes how long it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BandEdge {
    Start,
    End,
}

#[derive(Debug, Clone)]
struct BandRow {
    row: usize,
    events: Vec<TimedInputEvent>,
    curve: Option<autoplay_edit::AutoplayCurve>,
    start: f64,
    end: f64,
}

#[derive(Debug, Clone)]
pub(super) struct Band {
    edge: BandEdge,
    origin_x: f32,
    anchor: f64,
    rows: Vec<BandRow>,
}

impl AutoplayEditor {
    /// The in-progress curve of an element that is being resized.
    pub(super) fn band_curve(&self, row: usize) -> Option<autoplay_edit::AutoplayCurve> {
        self.band_drag
            .as_ref()?
            .rows
            .iter()
            .find(|band| band.row == row)?
            .curve
            .clone()
    }
}

fn moved_events(events: &[TimedInputEvent], offset: i64) -> Vec<TimedInputEvent> {
    events
        .iter()
        .map(|event| autoplay_edit::with_time(event, timed_input_tp(event).saturating_add(offset)))
        .collect()
}

fn scaled_events(events: &[TimedInputEvent], start: i64, ratio: f64) -> Vec<TimedInputEvent> {
    events
        .iter()
        .map(|event| {
            let tp = timed_input_tp(event);
            let scaled = start as f64 + (tp - start) as f64 * ratio;
            autoplay_edit::with_time(event, (scaled.round() as i64).max(0))
        })
        .collect()
}

/// Starts a resize gesture on the picked elements. The left edge moves them;
/// the right edge changes their length.
pub(super) fn begin_band(
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    row: usize,
    edge: BandEdge,
    origin_x: f32,
    anchor: f32,
) {
    if state.band_drag.is_some() {
        return;
    }
    let mut picks = state.selected_elements();
    if !picks.contains(&row) {
        picks = vec![row];
    }
    let mut rows = Vec::new();
    for row in picks {
        let (start, end) = state.element_span(row, app);
        if end <= start {
            continue;
        }
        rows.push(BandRow {
            row,
            events: state.tracks[row].events.clone(),
            curve: state.element_curve(row, app),
            start: start as f64,
            end: end as f64,
        });
    }
    if rows.is_empty() {
        return;
    }
    state.finish_inspector(editor, &mut app.chart);
    state.band_drag = Some(Band {
        edge,
        origin_x,
        anchor: anchor as f64,
        rows,
    });
}

pub(super) fn update_band(
    state: &mut AutoplayEditor,
    editor: &EditorState,
    app: &PadPreviewState,
    x: f32,
) {
    let Some(mut band) = state.band_drag.take() else {
        return;
    };
    let offset = (x - band.origin_x) / state.pixels_per_second;
    let snapped = snap_time((band.anchor + offset as f64) as f32, editor, &app.chart) as f64;
    let delta = snapped - band.anchor;
    let mut preview = Vec::new();
    for entry in &mut band.rows {
        let head = state.tracks[entry.row].head as f64 / 1e6;
        match band.edge {
            BandEdge::Start => {
                let start = (entry.start + delta).max(0.0);
                match &mut entry.curve {
                    Some(curve) => curve.start = start - head,
                    None => preview.push((
                        entry.row,
                        moved_events(&entry.events, ((start - entry.start) * 1e6).round() as i64),
                    )),
                }
            }
            BandEdge::End => {
                let duration = ((entry.end + delta) - entry.start).max(0.01);
                match &mut entry.curve {
                    Some(curve) => {
                        let length = curve::Path::new(curve).length as f64;
                        match curve.timing {
                            autoplay_edit::CurveTiming::FixedDuration => curve.duration = duration,
                            autoplay_edit::CurveTiming::ConstantSpeed => {
                                curve.speed = (length / duration).max(0.001);
                            }
                        }
                    }
                    None => {
                        let ratio = duration / (entry.end - entry.start).max(0.001);
                        preview.push((
                            entry.row,
                            scaled_events(&entry.events, (entry.start * 1e6).round() as i64, ratio),
                        ));
                    }
                }
            }
        }
    }
    for (row, events) in preview {
        state.tracks[row].events = events;
    }
    state.band_drag = Some(band);
}

pub(super) fn finish_band(
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    confirm: bool,
) {
    let Some(band) = state.band_drag.take() else {
        return;
    };
    if !confirm {
        for entry in &band.rows {
            if entry.curve.is_none() {
                state.tracks[entry.row].events = entry.events.clone();
            }
        }
        state.lanes_dirty = true;
        return;
    }
    let mut edits = Vec::new();
    let mut curves = Vec::new();
    for entry in &band.rows {
        match &entry.curve {
            Some(curve) => curves.push((entry.row, curve.clone())),
            None => edits.push((entry.row, state.tracks[entry.row].events.clone())),
        }
    }
    if !edits.is_empty() {
        state.commit_many(editor, &mut app.chart, edits);
    }
    if !curves.is_empty() {
        editor.push_history(&app.chart);
    }
    for (row, curve) in curves {
        if let Err(error) = state.store_curve_with(editor, app, row, curve, false) {
            editor.status = Some(error);
        }
    }
    state.lanes_dirty = true;
}

fn drag_band(
    ui: &egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
) {
    if state.band_drag.is_none() {
        return;
    }
    let (pointer, down) = ui.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down()));
    if let Some(pos) = pointer {
        update_band(state, editor, app, pos.x);
    }
    if !down {
        finish_band(state, editor, app, true);
    }
}

/// Rubber-band selection over the shared time axis.
fn marquee(
    ui: &egui::Ui,
    empty: &egui::Response,
    state: &mut AutoplayEditor,
    app: &PadPreviewState,
    active: bool,
    view: f32,
    left: f32,
    pps: f32,
) {
    if !active {
        state.marquee = None;
        return;
    }
    let time_of = |x: f32| view + (x - left) / pps;
    if empty.drag_started_by(egui::PointerButton::Primary) && !state.draw_mode {
        let start = ui
            .input(|i| i.pointer.press_origin())
            .or_else(|| ui.input(|i| i.pointer.latest_pos()))
            .map_or(view, |p| time_of(p.x));
        state.marquee = Some((start, start));
    }
    let Some((start, _)) = state.marquee else {
        return;
    };
    let pointer = ui.input(|i| i.pointer.latest_pos()).map(|p| time_of(p.x));
    if let Some(now) = pointer {
        state.marquee = Some((start, now));
        let p = ui.painter().with_clip_rect(ui.clip_rect());
        let (from, to) = (start.min(now), start.max(now));
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(left + (from - view) * pps, empty.rect.top()),
                Pos2::new(left + (to - view) * pps, empty.rect.bottom()),
            ),
            0.0,
            ACCENT_BLUE.gamma_multiply(0.15),
        );
    }
    if !ui.input(|i| i.pointer.primary_down()) {
        let (a, b) = state.marquee.unwrap_or((start, start));
        select_range(state, app, a.min(b), a.max(b));
        state.marquee = None;
    }
}

fn select_range(state: &mut AutoplayEditor, app: &PadPreviewState, from: f32, to: f32) {
    let layout = Arc::clone(&state.clip_layout);
    let mut rows = Vec::new();
    for clip in &layout.clips {
        let row = clip.rows[0];
        let (start, end) = state.element_span(row, app);
        if end >= from && start <= to {
            rows.push(row);
        }
    }
    state.selection = rows.clone();
    state.selected_row = rows.first().copied();
    state.selected = None;
    state.selected_pair = None;
    state.draw_mode = false;
    state.prepare_selected_curves(app);
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
            crate::timeline::overview::paint_loop(
                &p,
                plot,
                editor,
                |time| left + (time - view) * pps,
                false,
            );
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
