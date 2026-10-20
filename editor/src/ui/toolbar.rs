//! In-window menus and shared editor settings.

use super::{
    style::{glyph, icon},
    workbench::{PaneKind, Workbench},
};
use crate::app::{params, types::secs_to_measure};
use crate::player::state::PadPreviewState;
use crate::state::{EditorLayout, EditorState, PendingAction, Workspace};
use crate::theme::{ACCENT_RED, FONT_HEADER, TEXT, TEXT_DIM};
use egui_macroquad::egui;
use lucide_icons::Icon;

fn command(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let clicked = ui.add_enabled(enabled, egui::Button::new(label)).clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

/// Returns whether a menu owned input, including its activation frame.
pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    workbench: &mut Workbench,
) -> (bool, Option<Workspace>) {
    let mut owned = false;
    let mut workspace = None;
    egui::menu::bar(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let compact = ui.available_width() < 1000.0;
        ui.label(
            egui::RichText::new(if compact { "LDX" } else { "Lambda DX" })
                .size(FONT_HEADER)
                .color(TEXT)
                .strong(),
        );
        ui.label(
            egui::RichText::new(if editor.has_unsaved_changes() {
                "●"
            } else {
                "·"
            })
            .color(if editor.has_unsaved_changes() {
                ACCENT_RED
            } else {
                TEXT_DIM
            }),
        );
        ui.separator();
        owned |= ui
            .menu_button("文件", |ui| {
                if command(ui, "打开谱面...", true) {
                    editor.open_dialog = true;
                }
                if command(ui, "重新载入", true) {
                    editor.reload_request = true;
                }
                if command(ui, "保存", true) {
                    crate::io::save_document(editor, &mut app.chart);
                }
                if command(ui, "另存为...", true) {
                    editor.save_as_request = true;
                }
                if command(ui, "导出 Simai...", true) {
                    editor.export_request = true;
                }
                ui.checkbox(&mut editor.auto_save, "自动保存");
                if !editor.level_slots.is_empty() {
                    ui.separator();
                    ui.horizontal(|ui| draw_difficulty(ui, editor));
                }
                ui.separator();
                if command(ui, "退出", true) {
                    editor.pending_action = Some(PendingAction::Quit);
                    editor.pending_confirmed = !editor.has_unsaved_changes();
                }
            })
            .inner
            .is_some();
        owned |= ui
            .menu_button("编辑", |ui| {
                if command(ui, "撤销", !editor.undo.is_empty()) {
                    editor.undo(&mut app.chart);
                }
                if command(ui, "重做", !editor.redo.is_empty()) {
                    editor.redo(&mut app.chart);
                }
                ui.separator();
                let chart_workspace = editor.workspace == Workspace::Chart;
                let selected = chart_workspace && !editor.selected.is_empty();
                if command(ui, "复制", selected) {
                    editor.copy_selection(&app.chart);
                }
                if command(ui, "剪切", selected) {
                    editor.copy_selection(&app.chart);
                    editor.delete_selection(&mut app.chart);
                }
                if command(ui, "粘贴", chart_workspace && !editor.clipboard.is_empty()) {
                    let seconds = if app.mode == crate::app::types::Mode::Idle {
                        app.timeline_view_time
                    } else {
                        app.song_time()
                    };
                    let at = secs_to_measure(seconds, &app.chart.bpms);
                    editor.paste_at(&mut app.chart, at);
                }
                let event_selected = !chart_workspace
                    && editor.autoplay_editor.selected_row.is_some()
                    && app.has_engine();
                if command(ui, "删除", selected || event_selected) {
                    if chart_workspace {
                        editor.delete_selection(&mut app.chart);
                    } else {
                        let mut events = std::mem::take(&mut editor.autoplay_editor);
                        events.delete(editor, &mut app.chart);
                        editor.autoplay_editor = events;
                    }
                }
                if command(ui, "全选", chart_workspace && !app.chart.notes.is_empty()) {
                    editor.selected = app.chart.notes.iter().map(|n| n.id).collect();
                }
                if command(ui, "取消选择", selected || event_selected) {
                    if chart_workspace {
                        editor.selected.clear();
                    } else {
                        editor.autoplay_editor.clear_selection();
                    }
                }
            })
            .inner
            .is_some();
        owned |= ui
            .menu_button("模板", |ui| {
                ui.add_enabled_ui(editor.workspace == Workspace::Chart, |ui| {
                    ui.checkbox(&mut editor.template_panel, "模板库与 Scene");
                    let main = editor.template_session.is_none();
                    if command(ui, "从选择创建模板", main && !editor.selected.is_empty()) {
                        editor.template_action = Some(crate::templates::Action::CreateSelection);
                    }
                    if command(ui, "新建模板", main) {
                        editor.template_action = Some(crate::templates::Action::New);
                    }
                    if command(ui, "返回主谱面", !main) {
                        editor.template_action = Some(crate::templates::Action::Exit);
                    }
                });
            })
            .inner
            .is_some();
        owned |= ui
            .menu_button("视图", |ui| {
                pane_options(ui, editor, workbench);
                ui.menu_button("布局", |ui| workbench.layout_menu(ui));
                ui.separator();
                crate::player::params_panel::judgment_option(ui, &mut app.params);
                ui.checkbox(&mut app.params.hide_notes, "隐藏 Notes");
                ui.checkbox(&mut app.params.hide_zones, "隐藏感应区");
                params::set(app.params.clone());
            })
            .inner
            .is_some();
        ui.separator();
        workspace = workbench.workspace_picker(ui, compact);
        owned |= workspace.is_some();
        ui.separator();
        if icon(ui, Icon::SkipBack, "回到开头", false).clicked() {
            crate::runtime::rewind(editor, app);
        }
        let playing = app.mode == crate::app::types::Mode::Playing;
        if icon(
            ui,
            if playing { Icon::Pause } else { Icon::Play },
            "播放 / 暂停",
            playing,
        )
        .clicked()
        {
            crate::runtime::toggle_play(editor, app);
        }
        if icon(ui, Icon::Repeat2, "循环试听", editor.loop_enabled).clicked() {
            if editor.loop_range.is_none() {
                super::navigation::loop_selection(editor, &app.chart);
                if editor.loop_range.is_none() {
                    editor.loop_range = Some((
                        app.timeline_time().max(0.0),
                        app.timeline_time().max(0.0) + 4.0,
                    ));
                }
                editor.loop_enabled = true;
            } else {
                editor.loop_enabled = !editor.loop_enabled;
            }
        }
        let mut time = app.timeline_time();
        if ui
            .add(
                egui::DragValue::new(&mut time)
                    .range(0.0..=86400.0)
                    .speed(0.01)
                    .fixed_decimals(2)
                    .suffix(" s"),
            )
            .on_hover_text("播放位置")
            .changed()
        {
            editor.pending_seek = Some(time);
        }
        owned |= ui
            .menu_button("播放", |ui| {
                if command(
                    ui,
                    if app.mode == crate::app::types::Mode::Playing {
                        "暂停"
                    } else {
                        "播放"
                    },
                    true,
                ) {
                    crate::runtime::toggle_play(editor, app);
                }
                if command(ui, "从头播放", true) {
                    crate::runtime::restart_playback(editor, app);
                }
                if command(ui, "回到开头", true) {
                    crate::runtime::rewind(editor, app);
                }
                let audio = app.audio_enabled;
                if ui.checkbox(&mut app.audio_enabled, "音乐").changed() {
                    if audio {
                        app.stop_audio_if_any();
                    } else if app.mode == crate::app::types::Mode::Playing {
                        app.request_audio_start();
                    }
                }
                ui.separator();
                preview_options(ui, editor);
                ui.separator();
                loop_options(ui, editor, &app.chart);
            })
            .inner
            .is_some();
        owned |= ui
            .menu_button("设置", |ui| {
                ui.menu_button("Note 皮肤", |ui| {
                    for skin in crate::app::note_skin::NoteSkin::ALL {
                        if ui.selectable_value(&mut app.params.note_skin, skin, skin.label()).changed() {
                            params::set(app.params.clone());
                            crate::player::render::textures::sync_note_skin(app);
                            editor.status = Some(match params::save(&app.params) {
                                Ok(_) => format!("Note skin: {}", skin.label()),
                                Err(error) => format!("Settings: {error}"),
                            });
                            ui.close_menu();
                        }
                    }
                });
                draw_subdivision(ui, editor);
                ui.checkbox(&mut editor.scroll_invert, "反转滚轮方向");
                ui.separator();
                if command(ui, "所有设置...", true) {
                    app.show_params = true;
                }
                if command(ui, "保存显示与播放设置", true) {
                    editor.status = Some(match params::save(&app.params) {
                        Ok(_) => "Settings saved".into(),
                        Err(e) => format!("Settings: {e}"),
                    });
                }
            })
            .inner
            .is_some();
        owned |= ui
            .menu_button("帮助", |ui| {
                if command(ui, "快捷键", true) {
                    editor.show_help = true;
                }
            })
            .inner
            .is_some();
        if ui.available_width() > 100.0 {
            let mut speed = app.play_speed;
            if ui
                .add(
                    egui::DragValue::new(&mut speed)
                        .range(0.1..=3.0)
                        .speed(0.05)
                        .fixed_decimals(2)
                        .suffix("x"),
                )
                .on_hover_text("播放速度")
                .changed()
            {
                app.set_play_speed(speed);
            }
        }
        if ui.available_width() > 100.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon(ui, Icon::Settings2, "设置", false).clicked() {
                    app.show_params = true;
                }
                ui.label(crate::runtime::preview_mode_label(editor.autoplay_mode));
                if let Some(status) = &editor.status
                    && ui.available_width() > 140.0
                {
                    ui.add(egui::Label::new(status).truncate());
                }
            });
        }
    });
    (owned, workspace)
}

fn layout_options(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.horizontal(|ui| {
        let vertical = ui
            .selectable_value(&mut editor.layout, EditorLayout::Vertical, "纵向")
            .changed();
        let horizontal = ui
            .selectable_value(&mut editor.layout, EditorLayout::Horizontal, "横向")
            .changed();
        if vertical || horizontal {
            editor.follow_view_offset = None;
            editor.follow_zoom_detached = false;
        }
    });
}

fn preview_options(ui: &mut egui::Ui, editor: &mut EditorState) {
    for (mode, label) in [
        (0, "NoCore"),
        (1, "手动判定"),
        (2, "自动打歌：模拟触摸"),
        (3, "自动打歌：直传核心"),
    ] {
        if editor.workspace == Workspace::Autoplay && mode < 2 {
            continue;
        }
        if ui
            .radio_value(&mut editor.autoplay_mode, mode, label)
            .changed()
        {
            editor.mode_dirty = true;
        }
    }
}

fn loop_options(ui: &mut egui::Ui, editor: &mut EditorState, chart: &crate::app::types::ChartDoc) {
    ui.add_enabled_ui(editor.loop_range.is_some(), |ui| {
        ui.checkbox(&mut editor.loop_enabled, "循环试听");
    });
    if let Some((mut start, mut end)) = editor.loop_range {
        ui.horizontal(|ui| {
            ui.label("起点");
            ui.add(
                egui::DragValue::new(&mut start)
                    .range(0.0..=(end - 0.01).max(0.0))
                    .speed(0.01)
                    .fixed_decimals(3)
                    .suffix(" s"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("终点");
            ui.add(
                egui::DragValue::new(&mut end)
                    .range((start + 0.01)..=86400.0)
                    .speed(0.01)
                    .fixed_decimals(3)
                    .suffix(" s"),
            );
        });
        editor.loop_range = Some((start, end));
    }
    if command(
        ui,
        "循环所选范围",
        super::navigation::selection_span(editor, chart).is_some(),
    ) {
        super::navigation::loop_selection(editor, chart);
    }
    if command(ui, "清除循环范围", editor.loop_range.is_some()) {
        editor.loop_range = None;
        editor.loop_enabled = false;
    }
}

pub fn draw_editor_settings(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    pad_scale: &mut f32,
    workbench: &mut Workbench,
) {
    egui::CollapsingHeader::new("工作区与分区")
        .default_open(true)
        .show(ui, |ui| {
            layout_options(ui, editor);
            ui.checkbox(&mut editor.hide_chart_tools, "隐藏左侧工具栏");
            chart_note_size(ui, editor);
            pane_options(ui, editor, workbench);
            workbench.layout_menu(ui);
        });
    egui::CollapsingHeader::new("编辑与导航")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut editor.auto_save, "自动保存");
            draw_subdivision(ui, editor);
            ui.checkbox(&mut editor.scroll_invert, "反转滚轮方向");
            ui.checkbox(
                &mut editor.keyboard_seek_on_navigation,
                "时间轴跟随谱面光标",
            )
            .on_hover_text("Alt+Shift+Enter 切换；Alt+Enter 单次跳转到光标");
            if ui
                .checkbox(&mut editor.follow_playhead, "谱面跟随播放")
                .changed()
            {
                editor.keyboard.detached = !editor.follow_playhead;
                editor.follow_zoom_detached = false;
                if editor.follow_playhead {
                    editor.follow_view_offset = None;
                }
            }
            let mut autoplay_follow = editor.autoplay_editor.follow;
            if ui
                .checkbox(&mut autoplay_follow, "Autoplay 跟随播放")
                .changed()
            {
                editor.autoplay_editor.toggle_follow();
            }
        });
    egui::CollapsingHeader::new("预览与试听").show(ui, |ui| {
        ui.add(egui::Slider::new(pad_scale, 0.3..=2.0).text("预览缩放"));
        preview_options(ui, editor);
    });
    egui::CollapsingHeader::new("Simai 文本编辑").show(ui, |ui| {
        super::simai::options(ui, &mut editor.simai_editor.options);
    });
}

fn pane_options(ui: &mut egui::Ui, editor: &mut EditorState, workbench: &mut Workbench) {
    for kind in PaneKind::ALL {
        let mut shown = workbench.has(kind);
        if ui.checkbox(&mut shown, kind.title()).changed() {
            workbench.set_visible(kind, shown);
        }
    }
    // Do not mirror the dock back into the request here: a deferred add would
    // be cancelled and the pane would be added and removed in a loop.
}

pub fn grid_controls(ui: &mut egui::Ui, editor: &mut EditorState) {
    grid(ui, &mut editor.snap_enabled, &mut editor.snap_division);
}

pub fn autoplay_grid_controls(ui: &mut egui::Ui, editor: &mut EditorState) {
    grid(
        ui,
        &mut editor.autoplay_snap_enabled,
        &mut editor.autoplay_snap_division,
    );
}

fn grid(ui: &mut egui::Ui, enabled: &mut bool, division: &mut u32) {
    if icon(ui, Icon::Magnet, "网格吸附", *enabled).clicked() {
        *enabled = !*enabled;
    }
    if ui.available_width() >= 55.0 {
        ui.menu_button(format!("1/{}", division), |ui| {
            subdivision(ui, enabled, division)
        })
        .response
        .on_hover_text("网格细分");
    }
}

pub fn pane_tools(
    ui: &mut egui::Ui,
    kind: PaneKind,
    editor: &mut EditorState,
    app: &mut PadPreviewState,
    pad_scale: &mut f32,
) {
    match kind {
        PaneKind::Chart => {
            ui.menu_button(glyph(Icon::SlidersHorizontal, 14.0), |ui| {
                layout_options(ui, editor);
                ui.checkbox(&mut editor.hide_chart_tools, "隐藏左侧工具栏");
                chart_note_size(ui, editor);
                draw_subdivision(ui, editor);
                chart_zoom(ui, editor, app.timeline_time());
                ui.checkbox(&mut editor.scroll_invert, "反转滚轮方向");
                if ui
                    .checkbox(
                        &mut editor.keyboard_seek_on_navigation,
                        "时间轴跟随谱面光标",
                    )
                    .on_hover_text("Alt+Shift+Enter 切换；Alt+Enter 单次跳转到光标")
                    .changed()
                {
                    crate::keyboard::navigation::follow_cursor(editor, &app.chart);
                }
                if ui.button("显示完整谱面").clicked() {
                    super::navigation::fit_chart(editor, (0.0, app.song_duration()));
                    ui.close_menu();
                }
                let span = super::navigation::chart_selection_span(editor, &app.chart);
                if ui
                    .add_enabled(span.is_some(), egui::Button::new("显示所选 Notes"))
                    .clicked()
                {
                    super::navigation::fit_chart(editor, span.unwrap());
                    ui.close_menu();
                }
            })
            .response
            .on_hover_text("谱面视图");
            if icon(ui, Icon::LocateFixed, "回到播放线", false).clicked() {
                super::navigation::current_chart(editor, app.timeline_time());
            }
            if icon(
                ui,
                Icon::MoveRight,
                "跟随播放线",
                editor.follow_playhead && !editor.keyboard.detached,
            )
            .clicked()
            {
                if editor.follow_playhead && !editor.keyboard.detached {
                    editor.view_start =
                        crate::keyboard::navigation::reference_time(editor, app.timeline_time());
                    editor.follow_playhead = false;
                    editor.follow_zoom_detached = false;
                    editor.keyboard.detached = true;
                } else {
                    super::navigation::current_chart(editor, app.timeline_time());
                }
            }
            if ui.available_width() >= 30.0 {
                grid_controls(ui, editor);
            }
            if ui.available_width() >= 95.0 {
                chart_zoom(ui, editor, app.timeline_time());
            }
        }
        PaneKind::Pad => {
            ui.menu_button(glyph(Icon::SlidersHorizontal, 14.0), |ui| {
                preview_options(ui, editor);
                ui.separator();
                ui.checkbox(&mut editor.pane_pad_preview_only, "仅预览 (不可绘制)")
                    .on_hover_text("关闭后这块 Pad 可以放置音符和拖拽");
                ui.horizontal(|ui| {
                    ui.label("内容");
                    for (surface, label) in [
                        (crate::state::PadSurface::FollowFocus, "跟随聚焦"),
                        (crate::state::PadSurface::Chart, "谱面"),
                        (crate::state::PadSurface::Autoplay, "Autoplay"),
                    ] {
                        ui.selectable_value(&mut editor.pane_pad_surface, surface, label);
                    }
                });
                ui.separator();
                ui.add(egui::Slider::new(pad_scale, 0.3..=2.0).text("预览缩放"));
                crate::player::params_panel::judgment_option(ui, &mut app.params);
                ui.checkbox(&mut app.params.hide_notes, "隐藏 Notes");
                ui.checkbox(&mut app.params.hide_zones, "隐藏感应区");
                params::set(app.params.clone());
            })
            .response
            .on_hover_text("预览设置");
            if ui.available_width() > 80.0 {
                ui.label(
                    egui::RichText::new(crate::runtime::preview_mode_label(editor.autoplay_mode))
                        .color(TEXT_DIM),
                );
            }
        }
        PaneKind::Autoplay => crate::autoplay_editor::header(ui, editor, app),
        PaneKind::Simai => super::simai::header(ui, editor, &mut app.chart),
        PaneKind::Properties => {
            // Which inspector this pane shows is an option, not a workspace.
            ui.selectable_value(&mut editor.pane_properties_autoplay, false, "Note");
            ui.selectable_value(&mut editor.pane_properties_autoplay, true, "Autoplay");
        }
        PaneKind::History => {
            ui.add_enabled_ui(!editor.undo.is_empty(), |ui| {
                if icon(ui, Icon::Undo2, "撤销", false).clicked() {
                    editor.undo(&mut app.chart);
                }
            });
            ui.add_enabled_ui(!editor.redo.is_empty(), |ui| {
                if icon(ui, Icon::Redo2, "重做", false).clicked() {
                    editor.redo(&mut app.chart);
                }
            });
        }
        PaneKind::Templates => {
            if icon(ui, Icon::Plus, "新建模板", false).clicked()
                && editor.template_session.is_none()
            {
                editor.template_action = Some(crate::templates::Action::New);
            }
        }
        PaneKind::Overview => {}
    }
}

fn chart_note_size(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.horizontal(|ui| {
        ui.label("Note 大小");
        let mut scale = editor.note_scale();
        if ui.add(egui::DragValue::new(&mut scale).range(0.05..=8.0).speed(0.02).suffix(" x")).changed() {
            editor.chart_note_scale = Some(scale);
        }
        if icon(ui, Icon::RotateCcw, "跟随全局 Note 大小", false).clicked() {
            editor.chart_note_scale = None;
        }
    });
}

fn chart_zoom(ui: &mut egui::Ui, editor: &mut EditorState, now: f32) {
    let mut pps = editor.pixels_per_second;
    if ui
        .add(
            egui::DragValue::new(&mut pps)
                .range(crate::timeline::geometry::MIN_PPS..=crate::timeline::geometry::MAX_PPS)
                .speed(2.0)
                .max_decimals(0)
                .suffix(" px/s"),
        )
        .on_hover_text("谱面缩放")
        .changed()
    {
        crate::timeline::geometry::set_zoom(
            editor,
            pps,
            now,
            editor.layout == EditorLayout::Horizontal,
        );
    }
}

fn draw_subdivision(ui: &mut egui::Ui, editor: &mut EditorState) {
    if editor.workspace == Workspace::Autoplay {
        subdivision(
            ui,
            &mut editor.autoplay_snap_enabled,
            &mut editor.autoplay_snap_division,
        );
    } else {
        subdivision(ui, &mut editor.snap_enabled, &mut editor.snap_division);
    }
}

fn subdivision(ui: &mut egui::Ui, enabled: &mut bool, division: &mut u32) {
    ui.checkbox(enabled, "吸附");
    // Keep nested choices in the same menu tree so closing a parent cannot orphan a popup.
    ui.menu_button(format!("细分 1/{}", division), |ui| {
        ui.horizontal(|ui| {
            ui.label("自定义 1/");
            ui.add(egui::DragValue::new(division).range(1..=256).speed(1));
        });
        ui.separator();
        for value in [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256] {
            if ui
                .selectable_value(division, value, format!("1/{value}"))
                .clicked()
            {
                ui.close_menu();
            }
        }
    });
}

fn draw_difficulty(ui: &mut egui::Ui, editor: &mut EditorState) {
    let n = editor.level_slots.len();
    let prev = ui
        .add_enabled(n > 1, egui::Button::new(glyph(Icon::ChevronLeft, 14.0)))
        .on_hover_text("上一难度");
    let next = ui
        .add_enabled(n > 1, egui::Button::new(glyph(Icon::ChevronRight, 14.0)))
        .on_hover_text("下一难度");
    // Pick a difficulty directly; the arrows stay for stepping through slots.
    let slots = editor.level_slots.clone();
    if slots.len() > 1 {
        let current = slots
            .get(editor.level_index)
            .map(|slot| format!("Lv.{slot}"))
            .unwrap_or_else(|| "Lv.-".into());
        egui::ComboBox::from_id_salt("difficulty_select")
            .width(70.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (index, slot) in slots.iter().enumerate() {
                    if ui
                        .selectable_value(&mut editor.level_index, index, format!("Lv.{slot}"))
                        .changed()
                    {
                        editor.reload_request = true;
                    }
                }
            });
    } else if let Some(slot) = slots.first() {
        ui.label(format!("Lv.{slot}"));
    }
    if prev.clicked() {
        editor.level_index = (editor.level_index + n - 1) % n;
        editor.reload_request = true;
    }
    if next.clicked() {
        editor.level_index = (editor.level_index + 1) % n;
        editor.reload_request = true;
    }
}
