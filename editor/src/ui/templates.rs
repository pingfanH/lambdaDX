//! Template library and the original editor's scene/isolation interactions.

use crate::app::types::{ChartDoc, TemplateInstance, measure_to_secs, note_secs, secs_to_measure};
use crate::state::{EditorLayout, EditorState};
use crate::templates::Action;
use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER, RULER_H, snap};
use egui_macroquad::egui::{self, Color32, Pos2, Rect, Stroke};

#[derive(Default)]
pub struct TemplateUi {
    rename: Option<(String, String)>,
    drag: Option<InstanceDrag>,
}

struct InstanceDrag {
    id: String,
    anchor: f32,
    pointer_measure: f32,
    moved: bool,
    before: Option<ChartDoc>,
    isolate_on_click: bool,
}

impl TemplateUi {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut EditorState,
        chart: &ChartDoc,
        enabled: bool,
    ) {
                ui.add_enabled_ui(enabled, |ui| {
                    if let Some(session) = &editor.template_session {
                        let name = session
                            .parent
                            .templates
                            .iter()
                            .find(|t| t.id == session.template_id)
                            .map(|t| t.name.as_str())
                            .unwrap_or("Template");
                        ui.horizontal_wrapped(|ui| {
                            if ui.link("Main").clicked() {
                                editor.template_action = Some(Action::Exit);
                            }
                            ui.label(">");
                            ui.strong(name);
                        });
                        ui.label(format!("{} notes", chart.notes.len()));
                        if ui.button("返回主谱面").clicked() {
                            editor.template_action = Some(Action::Exit);
                        }
                        ui.separator();
                    }
                    let isolated = editor.template_session.is_some();
                    let library = editor
                        .template_session
                        .as_ref()
                        .map(|s| &s.parent)
                        .unwrap_or(chart);
                    if editor
                        .selected_template
                        .as_ref()
                        .is_none_or(|id| !library.templates.iter().any(|t| &t.id == id))
                    {
                        editor.selected_template = library.templates.first().map(|t| t.id.clone());
                    }
                    ui.add_enabled_ui(!isolated, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut editor.template_name)
                                .desired_width(f32::INFINITY)
                                .hint_text("模板名称"),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("+ Tpl").on_hover_text("新建空模板").clicked() {
                                editor.template_action = Some(Action::New);
                            }
                            if ui
                                .add_enabled(
                                    !editor.selected.is_empty(),
                                    egui::Button::new("From Sel"),
                                )
                                .on_hover_text("从选中音符创建模板")
                                .clicked()
                            {
                                editor.template_action = Some(Action::CreateSelection);
                            }
                        });
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("template_list")
                        .max_height((ui.available_height() * 0.46).max(80.0))
                        .show(ui, |ui| {
                            for template in &library.templates {
                                let count = library
                                    .template_instances
                                    .iter()
                                    .filter(|i| i.template_id == template.id)
                                    .count();
                                let response = ui.selectable_label(
                                    editor.selected_template.as_ref() == Some(&template.id),
                                    format!("{}  v{}  ({count})", template.name, template.version),
                                );
                                if response.clicked() {
                                    editor.selected_template = Some(template.id.clone());
                                }
                                if response.double_clicked() && !isolated {
                                    editor.template_action =
                                        Some(Action::EnterDefinition(template.id.clone()));
                                }
                                if !isolated {
                                    response.context_menu(|ui| {
                                        if ui.button("插入实例").clicked() {
                                            editor.template_action =
                                                Some(Action::Insert(template.id.clone()));
                                            ui.close_menu();
                                        }
                                        if ui.button("编辑模板").clicked() {
                                            editor.template_action =
                                                Some(Action::EnterDefinition(template.id.clone()));
                                            ui.close_menu();
                                        }
                                        if ui.button("重命名").clicked() {
                                            self.rename =
                                                Some((template.id.clone(), template.name.clone()));
                                            ui.close_menu();
                                        }
                                        if ui.button("删除模板及实例").clicked() {
                                            editor.template_action =
                                                Some(Action::DeleteDefinition(template.id.clone()));
                                            ui.close_menu();
                                        }
                                    });
                                }
                            }
                        });
                    if let Some(id) = editor.selected_template.clone() {
                        ui.add_enabled_ui(!isolated, |ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .button("Insert")
                                    .on_hover_text("插入到当前时间")
                                    .clicked()
                                {
                                    editor.template_action = Some(Action::Insert(id.clone()));
                                }
                                if ui.button("Edit").on_hover_text("隔离编辑模板").clicked() {
                                    editor.template_action =
                                        Some(Action::EnterDefinition(id.clone()));
                                }
                                if ui.small_button("…").on_hover_text("重命名模板").clicked()
                                {
                                    if let Some(t) = library.templates.iter().find(|t| t.id == id) {
                                        self.rename = Some((id.clone(), t.name.clone()));
                                    }
                                }
                                if ui
                                    .small_button("×")
                                    .on_hover_text("删除模板及实例")
                                    .clicked()
                                {
                                    editor.template_action =
                                        Some(Action::DeleteDefinition(id.clone()));
                                }
                            });
                        });
                    }
                    if let Some((id, name)) = &mut self.rename {
                        ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                        if ui.button("确认重命名").clicked() {
                            editor.template_action = Some(Action::Rename(id.clone(), name.clone()));
                            self.rename = None;
                        }
                    }
                    ui.separator();
                    ui.strong("Scene");
                    if ui.selectable_label(!isolated, "Main Chart").clicked() && isolated {
                        editor.template_action = Some(Action::Exit);
                    }
                    let mut instances: Vec<_> = library.template_instances.iter().collect();
                    instances.sort_by(|a, b| {
                        a.anchor_time
                            .total_cmp(&b.anchor_time)
                            .then(a.instance_id.cmp(&b.instance_id))
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("template_instances")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for instance in instances {
                                let name = library
                                    .templates
                                    .iter()
                                    .find(|t| t.id == instance.template_id)
                                    .map(|t| t.name.as_str())
                                    .unwrap_or("?");
                                let label = format!("{}  ·  {:.3}", name, instance.anchor_time);
                                ui.add_enabled_ui(!isolated, |ui| {
                                    let response = ui.selectable_label(
                                        editor.selected_instance.as_ref()
                                            == Some(&instance.instance_id),
                                        label,
                                    );
                                    if response.clicked() {
                                        editor.selected_instance =
                                            Some(instance.instance_id.clone());
                                        editor.selected.clear();
                                        let time =
                                            measure_to_secs(instance.anchor_time, &library.bpms);
                                        editor.pending_seek = Some(time);
                                        editor.view_start = time;
                                    }
                                    if response.double_clicked() {
                                        editor.template_action = Some(Action::EnterInstance(
                                            instance.instance_id.clone(),
                                        ));
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("编辑实例对应模板").clicked() {
                                            editor.template_action = Some(Action::EnterInstance(
                                                instance.instance_id.clone(),
                                            ));
                                            ui.close_menu();
                                        }
                                        if ui.button("删除实例").clicked() {
                                            editor.template_action = Some(Action::DeleteInstance(
                                                instance.instance_id.clone(),
                                            ));
                                            ui.close_menu();
                                        }
                                    });
                                });
                            }
                        });
                });
    }

    pub fn timeline(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        tools: Rect,
        editor: &mut EditorState,
        chart: &mut ChartDoc,
        now: f32,
        enabled: bool,
    ) -> bool {
        let plot = Rect::from_min_max(rect.min + egui::vec2(LANE_GUTTER, RULER_H), rect.max);
        let painter = ui.painter().with_clip_rect(plot);
        if let Some(session) = &editor.template_session {
            let end = chart
                .notes
                .iter()
                .map(|note| {
                    note.time
                        + note.hold_duration.max(
                            note.slide
                                .iter()
                                .map(|slide| slide.slide_duration)
                                .fold(0.0, f32::max),
                        )
                        - session.offset
                        - 1.0
                })
                .fold(0.25, f32::max);
            let bounds = time_rect(
                rect,
                editor,
                chart,
                now,
                session.offset + 1.0,
                session.offset + 1.0 + end,
            );
            let shade = Color32::from_black_alpha(72);
            match editor.layout {
                EditorLayout::Vertical => {
                    painter.rect_filled(
                        Rect::from_min_max(
                            plot.min,
                            Pos2::new(plot.right(), bounds.top().clamp(plot.top(), plot.bottom())),
                        ),
                        0.0,
                        shade,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(
                                plot.left(),
                                bounds.bottom().clamp(plot.top(), plot.bottom()),
                            ),
                            plot.max,
                        ),
                        0.0,
                        shade,
                    );
                }
                EditorLayout::Horizontal => {
                    painter.rect_filled(
                        Rect::from_min_max(
                            plot.min,
                            Pos2::new(
                                bounds.left().clamp(plot.left(), plot.right()),
                                plot.bottom(),
                            ),
                        ),
                        0.0,
                        shade,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(bounds.right().clamp(plot.left(), plot.right()), plot.top()),
                            plot.max,
                        ),
                        0.0,
                        shade,
                    );
                }
            }
            if enabled
                && !ui.ctx().wants_keyboard_input()
                && !editor.keyboard.owns_input()
                && editor.star_placement.is_none()
                && editor.hold_placement.is_none()
                && editor.editing_slide.is_none()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                editor.template_action = Some(Action::Exit);
                return true;
            }
            return false;
        }
        let pos = ui.input(|i| i.pointer.hover_pos());
        let hit = chart
            .template_instances
            .iter()
            .rev()
            .find(|instance| {
                let block = instance_rect(rect, editor, chart, now, instance).intersect(plot);
                pos.is_some_and(|p| plot.contains(p) && !tools.contains(p) && block.contains(p))
            })
            .map(|i| i.instance_id.clone());
        for instance in &chart.template_instances {
            let block = instance_rect(rect, editor, chart, now, instance);
            if !block.intersects(plot) {
                continue;
            }
            let selected = editor.selected_instance.as_ref() == Some(&instance.instance_id);
            painter.rect_filled(
                block,
                0.0,
                Color32::from_rgba_unmultiplied(60, 157, 224, if selected { 26 } else { 12 }),
            );
            painter.rect_stroke(
                block,
                0.0,
                Stroke::new(
                    if selected { 2.0_f32 } else { 1.0 },
                    Color32::from_rgb(66, 161, 222),
                ),
                egui::StrokeKind::Inside,
            );
            for corner in [
                block.left_top(),
                block.right_top(),
                block.left_bottom(),
                block.right_bottom(),
            ] {
                painter.rect_filled(
                    Rect::from_center_size(corner, egui::Vec2::splat(5.0)),
                    0.0,
                    Color32::from_rgb(66, 161, 222),
                );
            }
            if let Some(template) = chart
                .templates
                .iter()
                .find(|t| t.id == instance.template_id)
            {
                painter.text(
                    block.min + egui::vec2(5.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &template.name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(130, 201, 245),
                );
            }
        }
        if !enabled {
            return false;
        }
        if let Some(drag) = &mut self.drag {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if let Some(instance) = chart
                    .template_instances
                    .iter_mut()
                    .find(|i| i.instance_id == drag.id)
                {
                    instance.anchor_time = drag.anchor;
                }
                self.drag = None;
                return true;
            }
            if let Some(pos) = pos {
                if drag.before.is_some() && ui.input(|i| i.pointer.primary_down()) {
                    let measure = pointer_measure(pos, rect, editor, chart, now);
                    let anchor = snap(
                        drag.anchor + measure - drag.pointer_measure,
                        editor.snap_enabled,
                        editor.snap_division,
                    )
                    .max(1.0);
                    drag.moved |= (anchor - drag.anchor).abs() > 1e-5;
                    if let Some(instance) = chart
                        .template_instances
                        .iter_mut()
                        .find(|i| i.instance_id == drag.id)
                    {
                        instance.anchor_time = anchor;
                    }
                }
            }
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                if let Some(before) = &drag.before {
                    if before.template_instances != chart.template_instances {
                        editor.push_history(before);
                    }
                }
                if !drag.moved && drag.isolate_on_click {
                    editor.template_action = Some(Action::EnterInstance(drag.id.clone()));
                }
                self.drag = None;
            }
            return true;
        }
        if ui.ctx().wants_keyboard_input() || editor.keyboard.owns_input() {
            return false;
        }
        if let Some(id) = hit {
            let shift = ui.input(|i| i.modifiers.shift);
            if shift && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) {
                editor.template_action = Some(Action::DeleteInstance(id));
                return true;
            }
            if ui.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
            }) {
                editor.template_action = Some(Action::EnterInstance(id));
                return true;
            }
            if ui.input(|i| i.pointer.primary_pressed())
                && (shift
                    || !pos.is_some_and(|pos| canonical_head_at(pos, rect, editor, chart, now)))
            {
                let instance = chart
                    .template_instances
                    .iter()
                    .find(|i| i.instance_id == id)
                    .unwrap();
                self.drag = Some(InstanceDrag {
                    id: id.clone(),
                    anchor: instance.anchor_time,
                    pointer_measure: pointer_measure(pos.unwrap(), rect, editor, chart, now),
                    moved: false,
                    before: shift.then(|| chart.clone()),
                    isolate_on_click: shift,
                });
                editor.selected_instance = Some(id);
                editor.selected.clear();
                return true;
            }
            if !pos.is_some_and(|pos| canonical_head_at(pos, rect, editor, chart, now)) {
                return true;
            }
        }
        if editor.selected.is_empty() && !editor.keyboard.active {
            if let Some(id) = &editor.selected_instance {
                if ui.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                        || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                }) {
                    editor.template_action = Some(Action::DeleteInstance(id.clone()));
                    return true;
                }
            }
        }
        false
    }
}

fn canonical_head_at(
    pos: Pos2,
    rect: Rect,
    editor: &EditorState,
    chart: &ChartDoc,
    now: f32,
) -> bool {
    chart.notes.iter().any(|note| {
        let lane = crate::timeline::geometry::lane_index_of(note) as f32 + 0.5;
        let time = note_secs(note, &chart.bpms);
        let center = match editor.layout {
            EditorLayout::Vertical => Pos2::new(
                rect.left() + LANE_GUTTER + lane * (rect.width() - LANE_GUTTER) / 9.0,
                rect.bottom()
                    - JUDGE_MARGIN
                    - (time - crate::keyboard::navigation::reference_time(editor, now))
                        * editor.pixels_per_second,
            ),
            EditorLayout::Horizontal => Pos2::new(
                rect.left() + LANE_GUTTER + (time - editor.view_start) * editor.pixels_per_second,
                rect.top() + RULER_H + lane * (rect.height() - RULER_H) / 9.0,
            ),
        };
        let size = crate::timeline::notes::head_size(note)
            * crate::app::params::get().editor_note_scale
            * editor.glyph_to_points;
        Rect::from_center_size(center, egui::Vec2::splat(size)).contains(pos)
    })
}

fn time_rect(
    rect: Rect,
    editor: &EditorState,
    chart: &ChartDoc,
    now: f32,
    start: f32,
    end: f32,
) -> Rect {
    let a = measure_to_secs(start, &chart.bpms);
    let b = measure_to_secs(end, &chart.bpms);
    match editor.layout {
        EditorLayout::Vertical => {
            let origin = rect.bottom() - JUDGE_MARGIN;
            let now = crate::keyboard::navigation::reference_time(editor, now);
            let y = |t| origin - (t - now) * editor.pixels_per_second;
            Rect::from_min_max(
                Pos2::new(rect.left() + LANE_GUTTER, y(b)),
                Pos2::new(rect.right(), y(a).max(y(b) + 8.0)),
            )
        }
        EditorLayout::Horizontal => {
            let x =
                |t| rect.left() + LANE_GUTTER + (t - editor.view_start) * editor.pixels_per_second;
            Rect::from_min_max(
                Pos2::new(x(a), rect.top() + RULER_H),
                Pos2::new(x(b).max(x(a) + 8.0), rect.bottom()),
            )
        }
    }
}

fn instance_rect(
    rect: Rect,
    editor: &EditorState,
    chart: &ChartDoc,
    now: f32,
    instance: &TemplateInstance,
) -> Rect {
    let duration = chart
        .templates
        .iter()
        .find(|t| t.id == instance.template_id)
        .map(|t| t.duration)
        .unwrap_or(0.25);
    time_rect(
        rect,
        editor,
        chart,
        now,
        instance.anchor_time,
        instance.anchor_time + duration,
    )
}

fn pointer_measure(pos: Pos2, rect: Rect, editor: &EditorState, chart: &ChartDoc, now: f32) -> f32 {
    let seconds = match editor.layout {
        EditorLayout::Vertical => {
            crate::keyboard::navigation::reference_time(editor, now)
                + (rect.bottom() - JUDGE_MARGIN - pos.y) / editor.pixels_per_second
        }
        EditorLayout::Horizontal => {
            editor.view_start + (pos.x - rect.left() - LANE_GUTTER) / editor.pixels_per_second
        }
    };
    secs_to_measure(seconds, &chart.bpms)
}
