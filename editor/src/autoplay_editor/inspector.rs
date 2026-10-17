//! Owner-scoped properties. Field drags commit one document transaction.

use super::*;
use crate::ui::style::icon;
use lucide_icons::Icon;

pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    editable: bool,
) {
    state.validate_selection();
    if state.curve_editing() {
        crate::ui::style::enabled_ui(ui, editable, |ui| curves::controls(ui, state, editor, app));
        return;
    }
    crate::ui::style::enabled_ui(ui, editable, |ui| {
        let mut owner = state.selected_row;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 30.0).max(60.0);
            egui::ComboBox::from_id_salt("autoplay_owner")
                .width(width)
                .selected_text(
                    owner
                        .and_then(|r| state.tracks.get(r))
                        .map_or("Event owner", |t| &t.label),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut owner, None, "None");
                    egui::ScrollArea::vertical().max_height(220.0).show_rows(
                        ui,
                        20.0,
                        state.tracks.len(),
                        |ui, rows| {
                            for row in rows {
                                ui.selectable_value(
                                    &mut owner,
                                    Some(row),
                                    &state.tracks[row].label,
                                );
                            }
                        },
                    );
                });
            if icon(ui, Icon::X, "Clear selection", false).clicked() {
                owner = None;
            }
        });
        if owner != state.selected_row {
            state.finish_inspector(editor, &mut app.chart);
            state.clear_selection();
            state.selected_row = owner;
            if let Some(row) = owner { state.select_clip(row, app); }
            if state.detail_row.is_some() && owner.is_some() {
                state.detail_row = owner;
            }
        }
        curves::controls(ui, state, editor, app);
        if state.curve_editing() { return; }
        let Some(row) = owner else {
            drawing_tools(ui, state, editor, app);
            return;
        };
        if state.curve_view(row, &app.chart) { return; }
        let standalone = state.tracks[row].key.is_standalone();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} {} / {} events",
                    if standalone { "Clip" } else { "Note" },
                    state.tracks[row].key.note_id,
                    state.tracks[row].events.len()
                ))
                .color(crate::theme::TEXT_DIM),
            );
            if state.tracks[row].authored {
                ui.label(egui::RichText::new("Edited").color(crate::theme::ACCENT_BLUE));
            }
        });
        ui.horizontal(|ui| {
            if icon(ui, Icon::LocateFixed, "Seek to start", false).clicked() {
                state.finish_inspector(editor, &mut app.chart);
                let time = if standalone { event_span(&state.tracks[row]).0 } else { state.tracks[row].head };
                editor.pending_seek = Some(time as f32 / 1e6);
                state.return_current();
            }
            if icon(
                ui,
                Icon::AudioLines,
                "Open sensor detail",
                state.detail_row == Some(row),
            )
            .clicked()
            {
                state.finish_inspector(editor, &mut app.chart);
                state.raw_events = false;
                state.sensor_detail = true;
                state.detail_row = Some(row);
            }
            if !standalone && icon(
                ui,
                Icon::RotateCcw,
                "Restore this owner's generated events",
                false,
            )
            .clicked()
            {
                state.finish_inspector(editor, &mut app.chart);
                state.reset(editor, &mut app.chart, row);
            }
            crate::ui::style::enabled_ui(ui,
                state.selected.is_some() || !state.selection.is_empty(), |ui| {
                if icon(ui, Icon::EyeOff, "隐藏所选元素", false).clicked() {
                    state.finish_inspector(editor, &mut app.chart);
                    state.hide_elements(editor, &mut app.chart);
                }
                if icon(ui, Icon::Trash2, if state.selected.is_some() {
                    "删除所选事件或区间"
                } else {
                    "删除所选元素"
                }, false).clicked() {
                    state.finish_inspector(editor, &mut app.chart);
                    if state.selected.is_some() { state.delete(editor, &mut app.chart); }
                    else { state.delete_elements(editor, &mut app.chart); }
                }
            });
        });
        ui.separator();
        let active = if state.raw_events {
            raw(ui, state, editor, &mut app.chart, row)
        } else {
            intervals(ui, state, editor, &mut app.chart, row)
        };
        if !active && !ui.ctx().memory(|m| m.any_popup_open()) {
            state.finish_inspector(editor, &mut app.chart);
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if state.raw_events {
                egui::ComboBox::from_id_salt("new_event_kind")
                    .width(65.0)
                    .selected_text(kind_label(state.new_kind))
                    .show_ui(ui, |ui| {
                        for kind in 0..=2 {
                            ui.selectable_value(&mut state.new_kind, kind, kind_label(kind));
                        }
                    });
            }
            drawing_tools(ui, state, editor, app);
        });
    });
    egui::CollapsingHeader::new("Playback").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (mode, label) in [(2, "Touch"), (3, "Direct core")] {
                if ui
                    .selectable_value(&mut editor.autoplay_mode, mode, label)
                    .changed()
                {
                    editor.mode_dirty = true;
                }
            }
        });
        let mut inner = app.params.autoplay_inner_screen;
        if ui.checkbox(&mut inner, "Inner screen (K -> A)").changed() {
            state.finish_inspector(editor, &mut app.chart);
            app.params.autoplay_inner_screen = inner;
            crate::app::params::set(app.params.clone());
            state.sync(app);
        }
    });
    if !app.has_engine() {
        ui.label(if app.status.is_empty() {
            "Core updating"
        } else {
            &app.status
        });
    }
}

fn kind_label(kind: u8) -> &'static str {
    match kind {
        0 => "Click",
        1 => "Press",
        _ => "Release",
    }
}

fn stage(
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    row: usize,
    events: Vec<TimedInputEvent>,
) {
    if state.tracks[row].events == events {
        return;
    }
    if state
        .inspector_edit
        .as_ref()
        .is_some_and(|edit| edit.row != row)
    {
        state.finish_inspector(editor, chart);
    }
    state.inspector_edit.get_or_insert_with(|| InspectorEdit {
        row,
        original: state.tracks[row].events.clone(),
    });
    state.tracks[row].events = events;
    state.lanes_dirty = true;
    state.center_requested = false;
}

fn time_field(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    time: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) -> egui::Response {
    ui.push_id(id, |ui| {
        ui.add_sized(
            [70.0, 22.0],
            egui::DragValue::new(time)
                .speed(0.001)
                .max_decimals(6)
                .range(range),
        )
    })
    .inner
}

fn intervals(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    row: usize,
) -> bool {
    let events = state.tracks[row].events.clone();
    let mut active = false;
    egui::Grid::new(("sensor_intervals", row))
        .num_columns(3)
        .spacing(Vec2::new(5.0, 4.0))
        .show(ui, |ui| {
            ui.weak("Sensor");
            ui.weak("Press (s)");
            ui.weak("Release (s)");
            ui.end_row();
            for span in clips::intervals(&events) {
                let event = &events[span.press];
                let zone = event_zone(event);
                let selected = state
                    .selected
                    .is_some_and(|(r, i)| r == row && (i == span.press || span.release == Some(i)));
                let sensor = ui
                    .selectable_label(selected, &zone)
                    .on_hover_text("Select interval, then click a sensor in Pad");
                let mut start = timed_input_tp(event) as f64 / 1e6;
                let mut end = span
                    .release
                    .map(|i| timed_input_tp(&events[i]) as f64 / 1e6)
                    .unwrap_or(start);
                let mut responses = vec![sensor];
                if event_kind(event) == 2 {
                    ui.weak("-").on_hover_text("Unmatched Release");
                    responses.push(time_field(
                        ui,
                        (row, span.press),
                        &mut start,
                        -86400.0..=86400.0,
                    ));
                } else {
                    responses.push(time_field(
                        ui,
                        (row, span.press),
                        &mut start,
                        -86400.0..=if span.release.is_some() { end } else { 86400.0 },
                    ));
                    if let Some(release) = span.release {
                        responses.push(time_field(ui, (row, release), &mut end, start..=86400.0));
                    } else {
                        ui.weak(kind_label(event_kind(event))).on_hover_text(
                            if event_kind(event) == 1 {
                                "Unmatched Press"
                            } else {
                                "Single click"
                            },
                        );
                    }
                }
                let first = make_event(&zone, event_kind(event), (start * 1e6).round() as i64);
                let last = span
                    .release
                    .map(|release| (release, make_event(&zone, 2, (end * 1e6).round() as i64)));
                let changed =
                    first != *event || last.as_ref().is_some_and(|(i, e)| *e != events[*i]);
                if changed || responses.iter().any(|r| r.clicked() || r.drag_started()) {
                    state.selected = Some((row, span.press));
                    state.selected_pair = span.release;
                    state.draw_mode = false;
                }
                if changed {
                    let mut updated = state.tracks[row].events.clone();
                    updated[span.press] = first;
                    if let Some((i, e)) = last {
                        updated[i] = e;
                    }
                    stage(state, editor, chart, row, updated);
                }
                active |= responses.iter().any(|r| r.dragged() || r.has_focus());
                ui.end_row();
            }
        });
    active
}

fn raw(
    ui: &mut egui::Ui,
    state: &mut AutoplayEditor,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    row: usize,
) -> bool {
    let mut choice = state.selected.filter(|(r, _)| *r == row).map(|(_, i)| i);
    egui::ComboBox::from_id_salt("raw_event")
        .width(ui.available_width())
        .selected_text(
            choice
                .map(|i| event_label(i, &state.tracks[row].events[i]))
                .unwrap_or_else(|| "Event".into()),
        )
        .show_ui(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).show_rows(
                ui,
                20.0,
                state.tracks[row].events.len(),
                |ui, rows| {
                    for index in rows {
                        ui.selectable_value(
                            &mut choice,
                            Some(index),
                            event_label(index, &state.tracks[row].events[index]),
                        );
                    }
                },
            );
        });
    if choice != state.selected.filter(|(r, _)| *r == row).map(|(_, i)| i) {
        state.finish_inspector(editor, chart);
        state.selected = choice.map(|i| (row, i));
        state.selected_pair = None;
        state.draw_mode = false;
    }
    let Some(index) = choice else {
        return false;
    };
    let event = &state.tracks[row].events[index];
    let mut time = timed_input_tp(event) as f64 / 1e6;
    let mut kind = event_kind(event);
    let zone = event_zone(event);
    let response = ui
        .horizontal(|ui| {
            ui.label("Time (s)");
            time_field(ui, (row, index), &mut time, -86400.0..=86400.0)
        })
        .inner;
    ui.horizontal_wrapped(|ui| {
        ui.monospace(&zone)
            .on_hover_text("Click a sensor in Pad to change this event");
        for k in 0..=2 {
            ui.selectable_value(&mut kind, k, kind_label(k));
        }
    });
    let updated = make_event(&zone, kind, (time * 1e6).round() as i64);
    if updated != *event {
        let mut events = state.tracks[row].events.clone();
        events[index] = updated;
        stage(state, editor, chart, row, events);
    }
    state.selected_pair = None;
    response.dragged() || response.has_focus()
}
