//! In-app path picker for platforms without a native file dialog.
//!
//! The editor's Open/Save actions run on Linux and Windows through this modal.
//! It accepts a chart folder (`maidata.txt` / `chart.json`), a Simai/JSON file,
//! or the destination for a save/export.

use egui_macroquad::egui;

use crate::io::dialogs::Request;
use crate::state::EditorState;
use crate::theme::{ACCENT_RED, TEXT_DIM};

pub fn begin(editor: &mut EditorState, request: Request, hint: Option<std::path::PathBuf>) {
    editor.path_prompt = Some(request);
    editor.path_prompt_text = default_text(request, hint);
    editor.path_prompt_error = None;
    editor.path_prompt_focus = true;
    editor.file_dialog_active = true;
}

pub fn cancel(editor: &mut EditorState) {
    editor.path_prompt = None;
    editor.path_prompt_error = None;
    editor.path_prompt_focus = false;
    editor.file_dialog_active = false;
}

pub fn draw(ctx: &egui::Context, editor: &mut EditorState) {
    let Some(request) = editor.path_prompt else {
        return;
    };
    let mut confirm = false;
    let mut cancel_requested = false;
    let mut open = true;
    egui::Window::new(title(request))
        .id(egui::Id::new("editor-path-prompt"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(520.0);
            ui.label(hint(request));
            let response = ui.add(
                egui::TextEdit::singleline(&mut editor.path_prompt_text)
                    .desired_width(f32::INFINITY)
                    .hint_text("/path/to/chart"),
            );
            if editor.path_prompt_focus {
                response.request_focus();
                editor.path_prompt_focus = false;
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                confirm = true;
            }
            if let Some(error) = &editor.path_prompt_error {
                ui.colored_label(ACCENT_RED, error);
            }
            ui.horizontal(|ui| {
                if ui.button("确定").clicked() {
                    confirm = true;
                }
                if ui.button("取消").clicked() {
                    cancel_requested = true;
                }
                ui.label(
                    egui::RichText::new("Enter 确定 · Esc 取消")
                        .color(TEXT_DIM)
                        .small(),
                );
            });
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel_requested = true;
    }
    if confirm {
        match validate(request, &editor.path_prompt_text) {
            Ok(path) => {
                editor.pending_path = Some((request, path));
                cancel(editor);
            }
            Err(error) => editor.path_prompt_error = Some(error),
        }
    } else if cancel_requested {
        cancel(editor);
    }
}

fn title(request: Request) -> &'static str {
    match request {
        Request::Open => "打开谱面",
        Request::Save { save_as: true, .. } => "另存为",
        Request::Save { save_as: false, .. } => "导出 Simai",
        Request::Background => "选择背景图片",
    }
}

fn hint(request: Request) -> &'static str {
    match request {
        Request::Open => "输入谱面文件夹（含 maidata.txt / chart.json）或 .txt / .json 文件路径",
        Request::Save { save_as: true, .. } => "输入目标 .txt 或 .json 路径",
        Request::Save { save_as: false, .. } => "输入目标 .txt 路径",
        Request::Background => "输入图片文件路径",
    }
}

fn default_text(request: Request, hint: Option<std::path::PathBuf>) -> String {
    let Some(path) = hint else {
        return String::new();
    };
    let looks_like_file = path.is_file()
        || path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("json"));
    let path = if matches!(request, Request::Open) && looks_like_file {
        path.parent().map(ToOwned::to_owned).unwrap_or(path)
    } else {
        path
    };
    path.display().to_string()
}

fn validate(request: Request, text: &str) -> Result<std::path::PathBuf, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("请输入路径".into());
    }
    let path = expand_tilde(text);
    match request {
        Request::Open => {
            if !path.exists() {
                return Err(format!("路径不存在：{}", path.display()));
            }
            if !path.is_file() && !path.is_dir() {
                return Err("请选择谱面文件夹或文件".into());
            }
        }
        Request::Save { .. } => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() && !parent.is_dir() {
                    return Err(format!("目录不存在：{}", parent.display()));
                }
            }
        }
        Request::Background => {
            if !path.is_file() {
                return Err(format!("图片不存在：{}", path.display()));
            }
        }
    }
    Ok(path)
}

fn expand_tilde(text: &str) -> std::path::PathBuf {
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn open_hint_uses_the_chart_folder_for_a_file() {
        let hint = default_text(Request::Open, Some(PathBuf::from("/charts/song/maidata.txt")));
        assert_eq!(hint, "/charts/song");
    }

    #[test]
    fn empty_paths_are_rejected() {
        assert!(validate(Request::Open, "   ").is_err());
    }

    #[test]
    fn save_requires_an_existing_parent() {
        let missing = std::env::temp_dir().join("lambdadx-missing-parent-xyz");
        let path = missing.join("chart.json");
        assert!(validate(Request::Save { save_as: true, native: true }, &path.display().to_string()).is_err());
    }
}
