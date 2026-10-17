//! Curve drafts own Pad input until Apply/Cancel; only Apply changes the chart.
use super::*;
use crate::app::autoplay_edit::{AutoplayCurve, CurveTiming};
use crate::app::types::PadGeom;
use crate::ui::style::icon;
use lucide_icons::Icon;

#[derive(Debug, Clone, Copy)]
enum Target {
    Point(usize),
    Handle(usize, bool),
}

impl Target {
    fn index(self) -> usize {
        match self {
            Self::Point(i) | Self::Handle(i, _) => i,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Draft {
    row: usize,
    created: bool,
    curve: AutoplayCurve,
    selected: Option<usize>,
    drag: Option<Target>,
    freehand: bool,
    stroking: bool,
    error: Option<String>,
}

impl AutoplayEditor {
    pub fn curve_editing(&self) -> bool {
        self.curve_draft.is_some()
    }

    pub(super) fn curve_candidate(&self, row: usize, chart: &ChartDoc) -> bool {
        let Some(track) = self.tracks.get(row) else {
            return false;
        };
        if let Some(item) = chart.autoplay_overrides.iter().find(|o| o.key == track.key) {
            return item.curve.is_some();
        }
        track.key.role == 2
    }

    pub(super) fn curve_view(&self, row: usize, chart: &ChartDoc) -> bool {
        !self.raw_events
            && !self.sensor_detail
            && !self.draw_mode
            && self
                .clip_rows(row)
                .iter()
                .any(|&r| self.curve_candidate(r, chart))
    }

    pub(super) fn selected_curve_row(&self) -> Option<usize> {
        let row = self.selected_row?;
        let rows = self.clip_rows(row);
        self.curve_source
            .as_ref()
            .and_then(|key| rows.iter().copied().find(|r| self.tracks[*r].key == *key))
            .or(Some(row))
    }

    pub(super) fn curve_for(
        &mut self,
        row: usize,
        app: &PadPreviewState,
    ) -> Result<AutoplayCurve, String> {
        let track = self.tracks.get(row).ok_or("Curve owner no longer exists")?;
        if let Some(item) = app
            .chart
            .autoplay_overrides
            .iter()
            .find(|o| o.key == track.key)
        {
            return item
                .curve
                .clone()
                .ok_or_else(|| "Sensor-edited owner; use Map Slide to replace its events".into());
        }
        if let Some(curve) = self.curve_cache.get(&track.key) {
            return curve.clone();
        }
        if app.pad_svg.is_none() || app.judge_engine.is_none() {
            return curve::from_slide(&track.key, app);
        }
        let curve = curve::from_slide(&track.key, app);
        self.curve_cache.insert(track.key.clone(), curve.clone());
        curve
    }

    pub(super) fn prepare_selected_curves(&mut self, app: &PadPreviewState) {
        if let Some(row) = self.selected_row {
            for row in self.clip_rows(row) {
                if self.curve_candidate(row, &app.chart) {
                    let _ = self.curve_for(row, app);
                }
            }
        }
    }

    pub(super) fn translated_track_curve(
        &self,
        chart: &ChartDoc,
        row: usize,
        events: &[TimedInputEvent],
    ) -> Option<AutoplayCurve> {
        let track = self.tracks.get(row)?;
        let relative = autoplay_edit::relative(events, track.head);
        if let Some(item) = chart.autoplay_overrides.iter().find(|o| o.key == track.key) {
            return autoplay_edit::translated_curve(item, &relative);
        }
        autoplay_edit::translated_curve(
            &AutoplayOverride {
                key: track.key.clone(),
                events: autoplay_edit::relative(&track.defaults, track.head),
                curve: self
                    .curve_cache
                    .get(&track.key)
                    .and_then(|curve| curve.as_ref().ok())
                    .cloned(),
            },
            &relative,
        )
    }

    pub(super) fn select_clip(&mut self, row: usize, app: &PadPreviewState) {
        if self
            .selected_row
            .is_none_or(|old| !self.clip_rows(old).contains(&row))
        {
            self.curve_source = None;
            self.sensor_detail = false;
        }
        self.selected_row = Some(row);
        self.select_only(row);
        self.selected = None;
        self.selected_pair = None;
        self.draw_mode = false;
        // Elements are compact by default; the sensor rows are an explicit
        // expansion (context menu / Properties) shown in the element's lane.
        self.sensor_detail = false;
        if self.curve_view(row, &app.chart) {
            self.detail_row = Some(row);
        }
        self.prepare_selected_curves(app);
    }

    /// Shift/Cmd click: keep the rest of the pick; the clicked element becomes
    /// active, and dropping it hands the active role back to the last pick.
    pub(super) fn toggle_clip(
        &mut self,
        editor: &mut EditorState,
        row: usize,
        app: &mut PadPreviewState,
    ) {
        self.finish_inspector(editor, &mut app.chart);
        let was_selected = self.selection.contains(&row);
        self.toggle_row(row);
        self.selected = None;
        self.selected_pair = None;
        self.draw_mode = false;
        if was_selected {
            if self.selected_row == Some(row) {
                self.selected_row = self.selection.last().copied();
                self.curve_source = None;
            }
        } else {
            self.selected_row = Some(row);
        }
        self.prepare_selected_curves(app);
    }

    pub(super) fn new_curve(&mut self, editor: &mut EditorState, app: &mut PadPreviewState) {
        let time = app.timeline_time();
        self.new_curve_at(editor, app, time);
    }

    /// A hand-drawn curve whose start is the given time, not the playhead.
    pub(super) fn new_curve_at(
        &mut self,
        editor: &mut EditorState,
        app: &mut PadPreviewState,
        time: f32,
    ) {
        self.finish_inspector(editor, &mut app.chart);
        self.clear_selection();
        self.detail_row = None;
        // A drawn curve always becomes its own clip: reusing the selected
        // owner would replace that note's events, and cancelling the draft
        // would remove the selected group from the timeline.
        let row = self.standalone_row(&app.chart);
        self.repack();
        if self.raw_events {
            self.layout_events(self.pixels_per_second);
        }
        self.selected_row = Some(row);
        self.curve_draft = Some(Draft {
            row,
            created: true,
            curve: AutoplayCurve {
                start: snap_time(time, editor, &app.chart) as f64,
                ..Default::default()
            },
            selected: None,
            drag: None,
            freehand: true,
            stroking: false,
            error: None,
        });
    }

    pub(super) fn edit_curve(
        &mut self,
        editor: &mut EditorState,
        app: &mut PadPreviewState,
        row: usize,
        map: bool,
    ) {
        self.finish_inspector(editor, &mut app.chart);
        let curve = if map {
            curve::from_slide(&self.tracks[row].key, app)
        } else {
            self.curve_for(row, app)
        };
        match curve {
            Ok(mut curve) => {
                curve::ensure_handles(&mut curve);
                self.draw_mode = false;
                self.selected_row = Some(row);
                self.selected = None;
                self.selected_pair = None;
                self.sensor_detail = false;
                self.raw_events = false;
                self.detail_row = Some(row);
                self.curve_source = Some(self.tracks[row].key.clone());
                self.repack();
                self.curve_draft = Some(Draft {
                    row,
                    created: false,
                    curve,
                    selected: None,
                    drag: None,
                    freehand: false,
                    stroking: false,
                    error: None,
                });
            }
            Err(error) => editor.status = Some(error),
        }
    }

    pub(super) fn cancel_curve(&mut self) {
        if let Some(draft) = self.curve_draft.take() {
            if draft.created && draft.row < self.tracks.len() {
                self.tracks.remove(draft.row);
                self.clear_selection();
                self.lanes_dirty = true;
            }
        }
    }

    pub(super) fn apply_curve(&mut self, editor: &mut EditorState, app: &mut PadPreviewState) {
        let Some(draft) = &self.curve_draft else {
            return;
        };
        let (row, curve) = (draft.row, draft.curve.clone());
        if let Err(error) = self.store_curve(editor, app, row, curve) {
            if let Some(draft) = &mut self.curve_draft {
                draft.error = Some(error);
            }
            return;
        }
        self.selected_row = Some(row);
        self.select_only(row);
        self.selected = None;
        self.selected_pair = None;
        self.detail_row = Some(row);
        self.sensor_detail = false;
        self.raw_events = false;
        self.curve_source = self.tracks.get(row).map(|track| track.key.clone());
        self.curve_draft = None;
    }

    /// Bake a curve into sensor events while keeping the curve itself editable.
    pub(super) fn store_curve(
        &mut self,
        editor: &mut EditorState,
        app: &mut PadPreviewState,
        row: usize,
        curve: AutoplayCurve,
    ) -> Result<(), String> {
        self.store_curve_with(editor, app, row, curve, true)
    }

    /// `history` lets one gesture over a grouped conslide record a single undo
    /// step for all of its parts.
    pub(super) fn store_curve_with(
        &mut self,
        editor: &mut EditorState,
        app: &mut PadPreviewState,
        row: usize,
        curve: AutoplayCurve,
        history: bool,
    ) -> Result<(), String> {
        let events = match app.pad_svg.as_ref() {
            Some(svg) => curve::bake(&curve, svg)?,
            None => return Err("Pad geometry is not loaded".into()),
        };
        let track = self
            .tracks
            .get_mut(row)
            .ok_or("Curve owner no longer exists")?;
        let item = AutoplayOverride {
            key: track.key.clone(),
            events,
            curve: Some(curve),
        };
        if app.chart.autoplay_overrides.iter().any(|o| *o == item) {
            return Ok(());
        }
        if history {
            editor.push_history(&app.chart);
        }
        app.chart.autoplay_overrides.retain(|o| o.key != track.key);
        track.events = item
            .events
            .iter()
            .map(|e| autoplay_edit::with_time(e, timed_input_tp(e).saturating_add(track.head)))
            .collect();
        track.authored = true;
        if track.key.is_standalone() {
            track.label = format!("Curve {}", track.key.note_id);
        }
        app.chart.autoplay_overrides.push(item);
        self.edits_pending_sync = true;
        self.lanes_dirty = true;
        Ok(())
    }

    pub(super) fn curve_keys(
        &mut self,
        ctx: &egui::Context,
        editor: &mut EditorState,
        app: &mut PadPreviewState,
    ) {
        if ctx.wants_keyboard_input() || ctx.memory(|m| m.any_popup_open()) {
            return;
        }
        let (cancel, apply, delete, play) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Space),
            )
        });
        if cancel {
            self.cancel_curve();
            self.suppress_pointer = true;
        } else if apply {
            self.apply_curve(editor, app);
            self.suppress_pointer = true;
        } else if delete {
            if let Some(draft) = &mut self.curve_draft {
                if let Some(index) = draft
                    .selected
                    .take()
                    .filter(|i| *i < draft.curve.points.len())
                {
                    curve::remove_point(&mut draft.curve, index);
                }
            }
        }
        if play {
            crate::runtime::toggle_play(editor, app);
        }
    }
}

pub(super) fn controls(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
) {
    if state.curve_draft.is_none() {
        let rows: Vec<_> = state
            .selected_row
            .map(|row| state.clip_rows(row))
            .unwrap_or_default()
            .into_iter()
            .filter(|&r| state.curve_candidate(r, &app.chart) || state.tracks[r].key.role == 2)
            .collect();
        let mut row = state
            .selected_curve_row()
            .filter(|r| rows.contains(r))
            .or_else(|| rows.first().copied());
        if rows.len() > 1 {
            egui::ComboBox::from_id_salt("curve_source")
                .width(ui.available_width().min(220.0))
                .selected_text(row.map_or("Slide", |r| &state.tracks[r].label))
                .show_ui(ui, |ui| {
                    for &r in &rows {
                        ui.selectable_value(&mut row, Some(r), &state.tracks[r].label);
                    }
                });
        }
        state.curve_source = row.map(|r| state.tracks[r].key.clone());
        // A conslide is one element by default; these two commands change the
        // editing granularity between the whole slide and its single segments.
        if let Some(row) = row.filter(|&row| state.tracks[row].key.role == 2) {
            let key = state.tracks[row].key.clone();
            let split = state.slide_is_split(key.note_id, key.slide);
            ui.horizontal_wrapped(|ui| {
                if split {
                    if ui.button("合并各段").clicked() {
                        state.set_slide_split(key.note_id, key.slide, false);
                    }
                } else if state.clip_rows(row).len() > 1 && ui.button("分离为单段").clicked() {
                    state.set_slide_split(key.note_id, key.slide, true);
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if icon(ui, Icon::PenTool, "New Pad curve", false).clicked() {
                state.new_curve(editor, app);
            }
            if let Some(row) = row {
                if state.curve_candidate(row, &app.chart)
                    && icon(ui, Icon::Spline, "Edit curve points (E / Enter)", false).clicked()
                {
                    state.edit_curve(editor, app, row, false);
                }
                if state.tracks[row].key.role == 2
                    && state.tracks[row].authored
                    && icon(
                        ui,
                        Icon::RotateCcw,
                        "Map Slide: replace with the original Slide path",
                        false,
                    )
                    .clicked()
                {
                    state.edit_curve(editor, app, row, true);
                }
                if icon(ui, Icon::AudioLines, "Sensor detail", state.sensor_detail).clicked() {
                    state.sensor_detail = !state.sensor_detail;
                    state.detail_row = Some(row);
                    state.selected = None;
                    state.selected_pair = None;
                }
            }
        });
        if let Some(row) = row.filter(|&r| state.curve_view(r, &app.chart)) {
            match state.curve_for(row, app) {
                Ok(curve) => {
                    let path = curve::Path::new(&curve);
                    ui.label(if state.tracks[row].authored {
                        "Curve"
                    } else {
                        "Slide curve"
                    });
                    ui.monospace(format!(
                        "{} points / {:.3} R",
                        curve.points.len(),
                        path.length
                    ));
                    ui.monospace(format!(
                        "{:.3} s / {:.3} R/s",
                        path.duration(&curve),
                        path.length as f64 / path.duration(&curve).max(0.001)
                    ));
                    ui.label(match curve.timing {
                        CurveTiming::ConstantSpeed => "Constant speed",
                        CurveTiming::FixedDuration => "Fixed duration",
                    });
                }
                Err(error) => {
                    ui.colored_label(crate::theme::ACCENT_RED, error);
                }
            }
        }
        return;
    }
    let draft = state.curve_draft.as_mut().unwrap();
    let path = curve::Path::new(&draft.curve);
    let head = state.tracks[draft.row].head as f64 / 1e6;
    ui.label(&state.tracks[draft.row].label);
    ui.horizontal_wrapped(|ui| {
        if icon(
            ui,
            Icon::MousePointer2,
            "Drag points; click path to insert; click elsewhere to append; Alt-click to delete",
            !draft.freehand,
        )
        .clicked()
        {
            draft.freehand = false;
        }
        if icon(
            ui,
            Icon::Pencil,
            "Draw a new stroke on Pad (replaces this draft path)",
            draft.freehand,
        )
        .clicked()
        {
            draft.freehand = true;
        }
        if icon(ui, Icon::Spline, "Auto smooth handles", false).clicked() {
            draft.curve.smooth = true;
            draft.curve.handles.clear();
            curve::ensure_handles(&mut draft.curve);
        }
        if icon(
            ui,
            Icon::MoveUpRight,
            "Straight segments / vector handles",
            false,
        )
        .clicked()
        {
            draft.curve.smooth = false;
            draft.curve.handles.clear();
            curve::ensure_handles(&mut draft.curve);
        }
        if icon(ui, Icon::Trash2, "Delete selected point", false).clicked() {
            if let Some(index) = draft
                .selected
                .take()
                .filter(|i| *i < draft.curve.points.len())
            {
                curve::remove_point(&mut draft.curve, index);
            }
        }
    });
    if let Some(index) = draft.selected.filter(|i| *i < draft.curve.points.len()) {
        curve::ensure_handles(&mut draft.curve);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Point {}", index + 1));
            let mut point = draft.curve.points[index];
            ui.add(
                egui::DragValue::new(&mut point[0])
                    .prefix("X ")
                    .speed(0.005)
                    .range(-4.0..=4.0),
            );
            ui.add(
                egui::DragValue::new(&mut point[1])
                    .prefix("Y ")
                    .speed(0.005)
                    .range(-4.0..=4.0),
            );
            if point != draft.curve.points[index] {
                curve::move_point(&mut draft.curve, index, point);
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut aligned = draft.curve.handles[index].aligned;
            ui.selectable_value(&mut aligned, true, "Aligned");
            ui.selectable_value(&mut aligned, false, "Free");
            if aligned != draft.curve.handles[index].aligned {
                draft.curve.handles[index].aligned = aligned;
                let point = draft.curve.handles[index].outgoing;
                curve::move_handle(&mut draft.curve, index, false, point, false);
            }
        });
    }
    let old = draft.curve.timing;
    let duration = path.duration(&draft.curve);
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(
            &mut draft.curve.timing,
            CurveTiming::ConstantSpeed,
            "Constant speed",
        );
        ui.selectable_value(
            &mut draft.curve.timing,
            CurveTiming::FixedDuration,
            "Fixed duration",
        );
    });
    if old != draft.curve.timing {
        draft.curve.duration = duration.max(0.001);
        draft.curve.speed = (path.length as f64 / duration.max(0.001)).max(0.001);
    }
    egui::Grid::new("curve_timing")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Start (s)");
            let mut start = draft.curve.start + head;
            if ui
                .add(
                    egui::DragValue::new(&mut start)
                        .speed(0.001)
                        .range(-86400.0..=86400.0)
                        .max_decimals(6),
                )
                .changed()
            {
                draft.curve.start = start - head;
            }
            ui.end_row();
            if draft.curve.timing == CurveTiming::ConstantSpeed {
                ui.label("Speed (R/s)");
                ui.add(
                    egui::DragValue::new(&mut draft.curve.speed)
                        .speed(0.01)
                        .range(0.001..=10000.0),
                );
            } else {
                ui.label("Duration (s)");
                ui.add(
                    egui::DragValue::new(&mut draft.curve.duration)
                        .speed(0.01)
                        .range(0.001..=86400.0),
                );
            }
            ui.end_row();
            ui.label("Touch radius (R)");
            ui.add(
                egui::DragValue::new(&mut draft.curve.radius)
                    .speed(0.001)
                    .range(0.0..=0.25),
            );
            ui.end_row();
            ui.label("Length / Duration");
            ui.monospace(format!(
                "{:.3} R / {:.3} s",
                path.length,
                path.duration(&draft.curve)
            ));
            ui.end_row();
        });
    if let Some(error) = &draft.error {
        ui.colored_label(crate::theme::ACCENT_RED, error);
    }
    ui.horizontal(|ui| {
        if ui
            .button("Apply")
            .on_hover_text("Bake sensor events (Enter)")
            .clicked()
        {
            state.apply_curve(editor, app);
        }
        if ui
            .button("Cancel")
            .on_hover_text("Discard draft (Esc)")
            .clicked()
        {
            state.cancel_curve();
        }
    });
}

/// Arc-length duration of one curve, shared with the element painting.
pub(super) fn path_duration(curve: &AutoplayCurve) -> f64 {
    curve::Path::new(curve).duration(curve)
}

/// Paint one curve element inside its timeline row: the path thumbnail, the
/// duration band with its start/end grips, and the timing label.
pub(super) fn band(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    row: usize,
    curve: &AutoplayCurve,
    rect: Rect,
    left: f32,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let path = curve::Path::new(curve);
    let start = state.tracks[row].head as f64 / 1e6 + curve.start;
    let end = start + path.duration(curve);
    let selected = state.selected_curve_row() == Some(row);
    let color = if selected {
        crate::theme::ACCENT_BLUE
    } else {
        crate::theme::TEXT_DIM
    };
    let thumbnail = Rect::from_center_size(
        Pos2::new(rect.left() + 23.0, rect.center().y),
        Vec2::splat(34.0),
    );
    if path.points.len() >= 2 {
        let radius = path
            .points
            .iter()
            .map(|p| p.to_vec2().length())
            .fold(1.0_f32, f32::max);
        ui.painter()
            .with_clip_rect(thumbnail)
            .add(egui::Shape::line(
                path.points
                    .iter()
                    .map(|p| thumbnail.center() + p.to_vec2() * (14.0 / radius))
                    .collect(),
                Stroke::new(1.5_f32, color),
            ));
    }
    ui.painter().text(
        Pos2::new(rect.left() + 44.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{}", state.tracks[row].key.segment + 1),
        egui::FontId::monospace(11.0),
        color,
    );
    let x_of = |time: f64| left + (time as f32 - state.view_start) * state.pixels_per_second;
    // The band is the element's own span, so its ends are the drag grips.
    let band = Rect::from_min_max(
        Pos2::new(x_of(start), rect.center().y - 9.0),
        Pos2::new(x_of(end).max(x_of(start) + 8.0), rect.center().y + 9.0),
    );
    p.rect_filled(band, 2.0, color.gamma_multiply(0.2));
    p.line_segment(
        [
            Pos2::new(band.left(), band.center().y),
            Pos2::new(band.right(), band.center().y),
        ],
        Stroke::new(1.5_f32, color),
    );
    p.circle_filled(Pos2::new(band.left(), band.center().y), 3.0, color);
    p.circle_stroke(
        Pos2::new(band.right(), band.center().y),
        3.0,
        Stroke::new(1.0_f32, color),
    );
    p.text(
        Pos2::new(band.left().max(left) + 4.0, rect.bottom() - 2.0),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "{:.3} s  /  {:.3} R/s",
            end - start,
            path.length as f64 / (end - start).max(0.001)
        ),
        egui::FontId::monospace(10.0),
        color,
    );
}

fn hit_target(draft: &Draft, point: Pos2, screen: impl Fn(Pos2) -> Pos2) -> Option<Target> {
    let points = draft
        .curve
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| (Target::Point(i), point.distance(screen(Pos2::from(*p)))));
    let handles = draft
        .selected
        .and_then(|i| draft.curve.handles.get(i).map(|h| (i, h)))
        .into_iter()
        .flat_map(|(i, h)| {
            [
                (Target::Handle(i, true), h.incoming),
                (Target::Handle(i, false), h.outgoing),
            ]
        })
        .map(|(target, p)| (target, point.distance(screen(Pos2::from(p)))));
    points
        .chain(handles)
        .filter(|(_, d)| *d <= 9.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(target, _)| target)
}

fn edit_on_pad(ui: &mut egui::Ui, draft: &mut Draft, pad: PadGeom, enabled: bool) {
    let screen = |p: Pos2| Pos2::new(pad.cx + p.x * pad.outer_r, pad.cy + p.y * pad.outer_r);
    let local = |p: Pos2| {
        [
            (p.x - pad.cx) / pad.outer_r.max(1.0),
            (p.y - pad.cy) / pad.outer_r.max(1.0),
        ]
    };
    let response = ui.interact(
        ui.max_rect(),
        ui.id().with("curve_pad"),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let pointer = response
        .interact_pointer_pos()
        .or_else(|| ui.input(|i| i.pointer.latest_pos()));
    let nearest = pointer.and_then(|p| hit_target(draft, p, screen));
    if enabled {
        if response.drag_started_by(egui::PointerButton::Primary) {
            if draft.freehand {
                let origin = ui.input(|i| i.pointer.press_origin()).or(pointer);
                draft.curve.points = origin.map(|p| vec![local(p)]).unwrap_or_default();
                draft.curve.handles.clear();
                draft.curve.smooth = true;
                draft.stroking = true;
                draft.selected = None;
            } else {
                let origin = ui.input(|i| i.pointer.press_origin()).or(pointer);
                draft.drag = origin.and_then(|p| hit_target(draft, p, screen));
                draft.selected = draft.drag.map(Target::index);
            }
        }
        if let Some(p) = pointer {
            let q = local(p);
            if q.iter().all(|v| v.abs() <= 4.0) {
                if draft.stroking
                    && ui.input(|i| i.pointer.primary_down())
                    && draft.curve.points.len() < 2048
                {
                    if draft
                        .curve
                        .points
                        .last()
                        .is_none_or(|v| Pos2::from(*v).distance(Pos2::from(q)) >= 0.035)
                    {
                        draft.curve.points.push(q);
                    }
                } else if let Some(target) = draft.drag {
                    match target {
                        Target::Point(index) => curve::move_point(&mut draft.curve, index, q),
                        Target::Handle(index, incoming) => curve::move_handle(
                            &mut draft.curve,
                            index,
                            incoming,
                            q,
                            ui.input(|i| i.modifiers.alt),
                        ),
                    }
                    draft.error = None;
                }
                if draft.stroking
                    && !ui.input(|i| i.pointer.primary_down())
                    && draft.curve.points.len() < 2048
                    && draft
                        .curve
                        .points
                        .last()
                        .is_none_or(|v| Pos2::from(*v).distance(Pos2::from(q)) > 0.001)
                {
                    draft.curve.points.push(q);
                }
                if response.clicked_by(egui::PointerButton::Primary) {
                    if let Some(target) = nearest {
                        let index = target.index();
                        draft.selected = Some(index);
                        if matches!(target, Target::Point(_)) && ui.input(|i| i.modifiers.alt) {
                            curve::remove_point(&mut draft.curve, index);
                            draft.selected = None;
                        }
                    } else if draft.curve.points.len() < 2048 {
                        let insertion = if !draft.freehand {
                            curve::insert_nearest(
                                &mut draft.curve,
                                Pos2::from(q),
                                8.0 / pad.outer_r.max(1.0),
                            )
                        } else {
                            None
                        };
                        let index = insertion.unwrap_or_else(|| {
                            let index = draft.curve.points.len();
                            curve::append_point(&mut draft.curve, q);
                            index
                        });
                        draft.selected = Some(index);
                        draft.error = None;
                    }
                }
            }
        }
        if !ui.input(|i| i.pointer.primary_down() && i.focused) {
            if draft.stroking {
                draft.freehand = false;
                curve::ensure_handles(&mut draft.curve);
                draft.selected = draft.curve.points.len().checked_sub(1);
            }
            draft.stroking = false;
            draft.drag = None;
        }
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
}

pub fn pad(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    mut pad: PadGeom,
    editable: bool,
) -> bool {
    let mut state = std::mem::take(&mut editor.autoplay_editor);
    pad.outer_r /= crate::app::params::pad_circle_scale().max(1.0);
    let handled = pad_inner(ui, &mut state, editor, app, pad, editable);
    editor.autoplay_editor = state;
    handled
}

fn pad_inner(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    pad: PadGeom,
    editable: bool,
) -> bool {
    let active = state.curve_editing();
    let Some(row) = state.selected_row else {
        return false;
    };
    if !active && !state.curve_view(row, &app.chart) {
        return false;
    }
    let screen = |p: Pos2| Pos2::new(pad.cx + p.x * pad.outer_r, pad.cy + p.y * pad.outer_r);
    let local = |p: Pos2| {
        Pos2::new(
            (p.x - pad.cx) / pad.outer_r.max(1.0),
            (p.y - pad.cy) / pad.outer_r.max(1.0),
        )
    };
    if let Some(draft) = &mut state.curve_draft {
        edit_on_pad(ui, draft, pad, editable && !state.suppress_pointer);
    }
    let mut paths = Vec::new();
    for row in state.clip_rows(row) {
        let curve = if let Some(draft) = state.curve_draft.as_ref().filter(|d| d.row == row) {
            Some(draft.curve.clone())
        } else if state.curve_candidate(row, &app.chart) {
            state.curve_for(row, app).ok()
        } else {
            None
        };
        if let Some(curve) = curve {
            let path = curve::Path::new(&curve);
            paths.push((row, curve, path));
        }
    }
    if paths.is_empty() && !active {
        return false;
    }
    if !active {
        let response = ui.interact(
            ui.max_rect(),
            ui.id().with("curve_objects"),
            if editable && !state.suppress_pointer {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let hit = response.interact_pointer_pos().and_then(|p| {
            paths
                .iter()
                .filter_map(|(row, _, path)| {
                    let distance = path
                        .points
                        .windows(2)
                        .map(|pair| curve::segment_distance(local(p), pair[0], pair[1]))
                        .min_by(f32::total_cmp)?;
                    (distance * pad.outer_r <= 10.0).then_some((*row, distance))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(row, _)| row)
        });
        if let Some(row) = hit {
            if response.clicked() {
                state.curve_source = Some(state.tracks[row].key.clone());
            }
            if response.double_clicked() {
                state.edit_curve(editor, app, row, false);
            }
        }
    }
    let selected_row = state.selected_curve_row();
    let p = ui.painter();
    if let Some(svg) = &app.pad_svg {
        for def in &svg.zones {
            p.add(egui::Shape::closed_line(
                svg.def_screen_verts(def, &pad)
                    .iter()
                    .map(|p| Pos2::new(p.x, p.y))
                    .collect(),
                Stroke::new(0.6_f32, Color32::from_white_alpha(45)),
            ));
        }
    }
    for (row, curve, path) in &paths {
        let selected = selected_row == Some(*row);
        let color = if selected {
            crate::theme::ACCENT_BLUE
        } else {
            crate::theme::ACCENT_BLUE.gamma_multiply(0.35)
        };
        if path.points.len() >= 2 {
            p.add(egui::Shape::line(
                path.points.iter().copied().map(screen).collect(),
                Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, color),
            ));
        }
        if selected && !path.points.is_empty() {
            p.circle_filled(screen(path.at(0.0)), 3.0, color);
            p.circle_stroke(screen(path.at(1.0)), 4.0, Stroke::new(1.0_f32, color));
            let duration = path.duration(curve);
            let progress =
                (app.timeline_time() as f64 - state.tracks[*row].head as f64 / 1e6 - curve.start)
                    / duration.max(0.001);
            let at = screen(path.at(progress as f32));
            p.circle_stroke(
                at,
                (curve.radius * pad.outer_r).max(5.0),
                Stroke::new(1.5_f32, crate::theme::ACCENT_RED),
            );
            p.circle_filled(at, 2.0, crate::theme::ACCENT_RED);
        }
    }
    if let Some(draft) = &state.curve_draft {
        for (i, point) in draft.curve.points.iter().enumerate() {
            let at = screen(Pos2::from(*point));
            if draft.selected == Some(i) {
                if let Some(h) = draft.curve.handles.get(i) {
                    for handle in [h.incoming, h.outgoing] {
                        let handle = screen(Pos2::from(handle));
                        p.line_segment([at, handle], Stroke::new(1.0_f32, crate::theme::TEXT_DIM));
                        p.rect_filled(
                            Rect::from_center_size(handle, Vec2::splat(6.0)),
                            0.0,
                            Color32::WHITE,
                        );
                    }
                }
            }
            p.circle_filled(
                at,
                if draft.selected == Some(i) { 4.5 } else { 3.0 },
                if draft.selected == Some(i) {
                    Color32::WHITE
                } else {
                    crate::theme::ACCENT_BLUE
                },
            );
        }
    }
    true
}
