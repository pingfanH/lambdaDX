//! In-app file browser for platforms without a native file dialog.
//!
//! Linux and Windows open charts through this modal. It lists folders and
//! chart/image files, supports the usual directory navigation, and keeps a
//! path field for pasting a location directly.

use std::path::{Path, PathBuf};

use egui_macroquad::egui;

use crate::io::dialogs::Request;
use crate::state::EditorState;
use crate::theme::{ACCENT_RED, TEXT_DIM};

/// Entries shown in one directory listing.
struct Entry {
    path: PathBuf,
    name: String,
    directory: bool,
}
pub fn begin(editor: &mut EditorState, request: Request, hint: Option<PathBuf>) {
    let start = start_dir(request, hint.as_deref());
    editor.path_prompt = Some(request);
    editor.path_prompt_dir = start.clone();
    editor.path_prompt_text = start.display().to_string();
    editor.path_prompt_file = default_filename(request, hint.as_deref());
    editor.path_prompt_selected = None;
    editor.path_prompt_error = None;
    editor.path_prompt_focus = false;
    editor.file_dialog_active = true;
}

pub fn cancel(editor: &mut EditorState) {
    editor.path_prompt = None;
    editor.path_prompt_text.clear();
    editor.path_prompt_dir = PathBuf::new();
    editor.path_prompt_file.clear();
    editor.path_prompt_selected = None;
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
    let mut go = false;
    let mut open = true;
    egui::Window::new(title(request))
        .id(egui::Id::new("editor-path-prompt"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(900.0)
        .min_width(680.0)
        .max_height(720.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(hint(request));
            ui.horizontal(|ui| {
                if ui.button("上一级").clicked() {
                    navigate_up(editor);
                }
                let response = ui.add(
                    egui::TextEdit::singleline(&mut editor.path_prompt_text)
                        .desired_width(ui.available_width() - 70.0)
                        .hint_text("/path/to/chart"),
                );
                if editor.path_prompt_focus {
                    response.request_focus();
                    editor.path_prompt_focus = false;
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    go = true;
                }
                if ui.button("转到").clicked() {
                    go = true;
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(confirm_label(request)).clicked() {
                    confirm = true;
                }
                if ui.button("取消").clicked() {
                    cancel_requested = true;
                }
                ui.label(
                    egui::RichText::new("双击打开 · Enter 前往 · Esc 取消")
                        .color(TEXT_DIM)
                        .small(),
                );
            });
            ui.separator();
            let entries = list_entries(request, &editor.path_prompt_dir);
            egui::ScrollArea::vertical()
                .id_salt("editor-file-browser")
                .max_height(320.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if entries.is_empty() {
                        ui.label(
                            egui::RichText::new("此目录没有可用项目").color(TEXT_DIM),
                        );
                    }
                    for entry in entries {
                        let selected = editor.path_prompt_selected.as_deref()
                            == Some(entry.path.as_path());
                        let label = if entry.directory {
                            format!("{}/", entry.name)
                        } else {
                            entry.name.clone()
                        };
                        let response = ui
                            .push_id(&entry.path, |ui| {
                                ui.selectable_label(selected, label)
                            })
                            .inner;
                        if response.clicked() {
                            editor.path_prompt_selected = Some(entry.path.clone());
                            if !entry.directory {
                                editor.path_prompt_file = entry.name.clone();
                            }
                        }
                        if response.double_clicked() {
                            if entry.directory {
                                set_dir(editor, entry.path.clone());
                            } else {
                                editor.path_prompt_selected = Some(entry.path.clone());
                                confirm = true;
                            }
                        }
                    }
                });
            if let Request::Save { .. } = request {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("文件名");
                    ui.add(
                        egui::TextEdit::singleline(&mut editor.path_prompt_file)
                            .desired_width(ui.available_width() - 10.0),
                    );
                });
            }
            if let Some(error) = &editor.path_prompt_error {
                ui.colored_label(ACCENT_RED, error);
            }
            ui.separator();
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel_requested = true;
    }
    if go {
        if let Err(error) = navigate_text(editor, request) {
            editor.path_prompt_error = Some(error);
        }
    }
    if confirm {
        match confirm_path(editor, request) {
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

fn confirm_label(request: Request) -> &'static str {
    match request {
        Request::Open => "打开",
        Request::Save { save_as: true, .. } => "保存",
        Request::Save { save_as: false, .. } => "导出",
        Request::Background => "选择",
    }
}

fn hint(request: Request) -> &'static str {
    match request {
        Request::Open => "选择谱面文件夹（含 maidata.txt / chart.json）或谱面文件",
        Request::Save { save_as: true, .. } => "选择目录并输入 .txt / .json 文件名",
        Request::Save { save_as: false, .. } => "选择目录并输入 .txt 文件名",
        Request::Background => "选择图片文件",
    }
}

fn start_dir(request: Request, hint: Option<&Path>) -> PathBuf {
    let candidate = hint
        .map(|path| {
            if path.is_dir() {
                path.to_path_buf()
            } else {
                path.parent().unwrap_or(path).to_path_buf()
            }
        })
        .filter(|path| path.is_dir());
    candidate
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_dir()))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(std::env::temp_dir)
}

fn default_filename(request: Request, hint: Option<&Path>) -> String {
    match request {
        Request::Save { native, .. } => hint
            .and_then(|path| path.file_name())
            .filter(|name| !name.is_empty())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| if native { "chart.json".into() } else { "chart.txt".into() }),
        _ => String::new(),
    }
}

fn navigate_up(editor: &mut EditorState) {
    if let Some(parent) = editor.path_prompt_dir.parent() {
        set_dir(editor, parent.to_path_buf());
    }
}

fn set_dir(editor: &mut EditorState, dir: PathBuf) {
    editor.path_prompt_dir = dir.clone();
    editor.path_prompt_text = dir.display().to_string();
    editor.path_prompt_selected = None;
    editor.path_prompt_error = None;
}

fn navigate_text(editor: &mut EditorState, request: Request) -> Result<(), String> {
    let text = editor.path_prompt_text.trim();
    if text.is_empty() {
        return Err("请输入路径".into());
    }
    let path = expand_tilde(text);
    if path.is_dir() {
        set_dir(editor, path);
        return Ok(());
    }
    if path.is_file() {
        if !accepts_file(request, &path) {
            return Err("当前操作不接受这个文件类型".into());
        }
        if let Request::Save { .. } = request {
            editor.path_prompt_file = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(parent) = path.parent() {
                set_dir(editor, parent.to_path_buf());
            }
        }
        editor.path_prompt_selected = Some(path);
        return Ok(());
    }
    Err(format!("路径不存在：{}", path.display()))
}

fn confirm_path(editor: &EditorState, request: Request) -> Result<PathBuf, String> {
    match request {
        Request::Open => {
            let path = editor
                .path_prompt_selected
                .clone()
                .or_else(|| {
                    let text = editor.path_prompt_text.trim();
                    (!text.is_empty()).then(|| expand_tilde(text))
                })
                .ok_or_else(|| "请选择谱面文件夹或文件".to_string())?;
            if !path.exists() {
                return Err(format!("路径不存在：{}", path.display()));
            }
            Ok(path)
        }
        Request::Save { save_as, native } => {
            let name = editor.path_prompt_file.trim();
            if name.is_empty() {
                return Err("请输入文件名".into());
            }
            let mut path = editor.path_prompt_dir.join(name);
            if path.extension().is_none() {
                path.set_extension(if save_as && native { "json" } else { "txt" });
            }
            if !editor.path_prompt_dir.is_dir() {
                return Err(format!("目录不存在：{}", editor.path_prompt_dir.display()));
            }
            Ok(path)
        }
        Request::Background => {
            let path = editor
                .path_prompt_selected
                .clone()
                .ok_or_else(|| "请选择图片文件".to_string())?;
            if !path.is_file() {
                return Err(format!("图片不存在：{}", path.display()));
            }
            Ok(path)
        }
    }
}

fn list_entries(request: Request, dir: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return entries;
    };
    for entry in read.flatten().take(500) {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let directory = meta.is_dir();
        if !directory && !accepts_file(request, &path) {
            continue;
        }
        entries.push(Entry {
            path,
            name,
            directory,
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

fn accepts_file(request: Request, path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    match request {
        Request::Open => matches!(ext.as_str(), "txt" | "json"),
        Request::Save { save_as: true, .. } => matches!(ext.as_str(), "txt" | "json"),
        Request::Save { save_as: false, .. } => ext == "txt",
        Request::Background => matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp"),
    }
}

fn expand_tilde(text: &str) -> PathBuf {
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_starts_in_the_chart_folder() {
        let dir = std::env::temp_dir();
        let file = dir.join("maidata.txt");
        assert_eq!(start_dir(Request::Open, Some(&file)), dir);
    }

    #[test]
    fn save_defaults_to_the_chart_filename() {
        assert_eq!(
            default_filename(
                Request::Save {
                    save_as: true,
                    native: true
                },
                None
            ),
            "chart.json"
        );
        assert_eq!(
            default_filename(
                Request::Save {
                    save_as: false,
                    native: false
                },
                None
            ),
            "chart.txt"
        );
    }

    #[test]
    fn file_filters_match_the_request() {
        assert!(accepts_file(Request::Open, Path::new("maidata.txt")));
        assert!(accepts_file(Request::Open, Path::new("chart.json")));
        assert!(!accepts_file(Request::Open, Path::new("track.mp3")));
        assert!(accepts_file(Request::Background, Path::new("cover.png")));
        assert!(!accepts_file(
            Request::Save {
                save_as: false,
                native: false
            },
            Path::new("chart.json")
        ));
    }

    #[test]
    fn save_appends_the_expected_extension() {
        let mut editor = EditorState::default();
        editor.path_prompt_dir = std::env::temp_dir();
        editor.path_prompt_file = "chart".into();
        let path = confirm_path(
            &editor,
            Request::Save {
                save_as: true,
                native: true,
            },
        )
        .unwrap();
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("json"));
    }

}
