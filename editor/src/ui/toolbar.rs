//! In-window menus and shared editor settings.

use crate::app::{params, types::secs_to_measure};
use crate::player::state::PadPreviewState;
use crate::state::{EditorLayout, EditorState, PendingAction, Workspace};
use crate::theme::{ACCENT_RED, FONT_HEADER, TEXT, TEXT_DIM};
use egui_macroquad::egui;

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
    sidebar: &mut bool,
) -> (bool, Option<Workspace>) {
    let mut owned = false;
    let mut workspace = None;
    egui::menu::bar(ui, |ui| {
        ui.label(
            egui::RichText::new("Lambda DX")
                .size(FONT_HEADER)
                .color(TEXT)
                .strong(),
        );
        ui.label(
            egui::RichText::new(if editor.dirty { "●" } else { "·" }).color(if editor.dirty {
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
                    crate::io::save_chart(editor, &mut app.chart);
                }
                if command(ui, "另存为...", true) {
                    editor.save_as_request = true;
                }
                if command(ui, "导出 Simai...", true) {
                    editor.export_request = true;
                }
                ui.separator();
                if command(ui, "退出", true) {
                    editor.pending_action = Some(PendingAction::Quit);
                    editor.pending_confirmed = !editor.dirty;
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
                let event_selected = !chart_workspace && editor.autoplay_editor.selected.is_some() && app.has_engine();
                if command(ui, "删除", selected || event_selected) {
                    if chart_workspace { editor.delete_selection(&mut app.chart); }
                    else {
                        let mut events = std::mem::take(&mut editor.autoplay_editor);
                        events.delete(editor, &mut app.chart);
                        editor.autoplay_editor = events;
                    }
                }
                if command(ui, "全选", chart_workspace && !app.chart.notes.is_empty()) {
                    editor.selected = app.chart.notes.iter().map(|n| n.id).collect();
                }
                if command(ui, "取消选择", selected) {
                    editor.selected.clear();
                }
            })
            .inner
            .is_some();
        owned |= ui.menu_button("模板", |ui| {
            ui.add_enabled_ui(editor.workspace == Workspace::Chart, |ui| {
                ui.checkbox(&mut editor.template_panel, "模板库与 Scene");
                let main = editor.template_session.is_none();
                if command(ui, "从选择创建模板", main && !editor.selected.is_empty()) {
                    editor.template_action = Some(crate::templates::Action::CreateSelection);
                }
                if command(ui, "新建模板", main) { editor.template_action = Some(crate::templates::Action::New); }
                if command(ui, "返回主谱面", !main) { editor.template_action = Some(crate::templates::Action::Exit); }
            });
        }).inner.is_some();
        owned |= ui
            .menu_button("视图", |ui| {
                ui.add_enabled_ui(editor.workspace == Workspace::Chart, |ui| {
                    layout_options(ui, editor);
                    ui.checkbox(sidebar, "预览与属性面板");
                    ui.checkbox(&mut editor.template_panel, "模板库与 Scene");
                });
                ui.separator();
                crate::player::params_panel::judgment_option(ui, &mut app.params);
                ui.checkbox(&mut app.params.hide_notes, "隐藏 Notes");
                ui.checkbox(&mut app.params.hide_zones, "隐藏感应区");
                params::set(app.params.clone());
            })
            .inner
            .is_some();
        ui.separator();
        for (target, label) in [(Workspace::Chart, "谱面"), (Workspace::Autoplay, "Autoplay")] {
            if ui.selectable_label(editor.workspace == target, label).clicked() && editor.workspace != target {
                workspace = Some(target);
                owned = true;
            }
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
                    app.toggle_play();
                }
                if command(ui, "从头播放", true) {
                    app.start_playback_at(0.0);
                }
                if command(ui, "回到开头", true) {
                    app.seek_to(0.0);
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
            })
            .inner
            .is_some();
        owned |= ui
            .menu_button("设置", |ui| {
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
        if ui.available_width() > 190.0 && !editor.level_slots.is_empty() {
            ui.separator();
            draw_difficulty(ui, editor);
        }
        if ui.available_width() > 240.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(crate::runtime::preview_mode_label(editor.autoplay_mode));
                if let Some(status) = &editor.status {
                    ui.add(egui::Label::new(status).truncate());
                }
            });
        }
    });
    (owned, workspace)
}

fn layout_options(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut editor.layout, EditorLayout::Vertical, "纵向");
        ui.selectable_value(&mut editor.layout, EditorLayout::Horizontal, "横向");
    });
}

fn preview_options(ui: &mut egui::Ui, editor: &mut EditorState) {
    for (mode, label) in [
        (0, "NoCore"),
        (1, "手动判定"),
        (2, "自动打歌：模拟触摸"),
        (3, "自动打歌：直传核心"),
    ] {
        if editor.workspace == Workspace::Autoplay && mode < 2 { continue; }
        if ui
            .radio_value(&mut editor.autoplay_mode, mode, label)
            .changed()
        {
            editor.mode_dirty = true;
        }
    }
}

pub fn draw_editor_settings(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    pad_scale: &mut f32,
    sidebar: &mut bool,
) {
    layout_options(ui, editor);
    ui.checkbox(sidebar, "预览与属性面板");
    draw_subdivision(ui, editor);
    ui.checkbox(&mut editor.scroll_invert, "反转滚轮方向");
    ui.add(egui::Slider::new(pad_scale, 0.3..=2.0).text("预览缩放"));
    preview_options(ui, editor);
}

fn draw_subdivision(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.checkbox(&mut editor.snap_enabled, "吸附");
    egui::ComboBox::from_id_salt("grid_division")
        .width(96.0)
        .selected_text(format!("细分 1/{}", editor.snap_division))
        .show_ui(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("自定义 1/");
                ui.add(
                    egui::DragValue::new(&mut editor.snap_division)
                        .range(1..=256)
                        .speed(1),
                );
            });
            ui.separator();
            for division in [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256] {
                ui.selectable_value(&mut editor.snap_division, division, format!("1/{division}"));
            }
        });
}

fn draw_difficulty(ui: &mut egui::Ui, editor: &mut EditorState) {
    let n = editor.level_slots.len();
    let prev = ui
        .add_enabled(n > 1, egui::Button::new("◀"))
        .on_hover_text("上一难度");
    let next = ui
        .add_enabled(n > 1, egui::Button::new("▶"))
        .on_hover_text("下一难度");
    if let Some(slot) = editor.level_slots.get(editor.level_index) {
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
