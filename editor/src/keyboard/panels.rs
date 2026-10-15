//! Dialog support for the editor's keyboard command router.

use egui_macroquad::egui;
use crate::app::types::{ChartDoc, Note, NoteType, measure_to_secs, secs_to_measure};
use crate::state::{EditorState, Tool};
use super::{edit, Focus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind { Commands, Tools, Properties, Handles, Jump, DeleteSlide }

#[derive(Debug, Clone)]
pub struct Dialog {
    pub kind: DialogKind,
    pub value: String,
    pub index: usize,
    pub request_focus: bool,
    pub note: Option<Note>,
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub command: String,
    pub key: egui::Key,
    pub primary: bool,
    pub shift: bool,
}

pub fn default_bindings() -> Vec<Binding> {
    use egui::Key;
    [
        ("help", Key::F12, false, false), ("select", Key::F2, false, false),
        ("select", Key::F2, false, true), ("commands", Key::F3, false, false),
        ("tools", Key::F4, false, false), ("properties", Key::F5, false, false),
        ("focus", Key::F6, false, false), ("focus", Key::F6, false, true),
        ("handles", Key::F7, false, false), ("snap", Key::F8, false, false),
        ("jump", Key::J, false, false), ("play", Key::Space, false, false),
        ("save", Key::S, true, false), ("save_as", Key::S, true, true),
        ("open", Key::O, true, false), ("export", Key::E, true, true),
        ("undo", Key::Z, true, false),
        ("redo", Key::Z, true, true), ("redo", Key::Y, true, false),
        ("copy", Key::C, true, false), ("cut", Key::X, true, false),
        ("paste", Key::V, true, false), ("all", Key::A, true, false),
        ("all", Key::A, true, true), ("duplicate", Key::D, false, true),
        ("repeat", Key::R, false, true),
    ].into_iter().map(|(command, key, primary, shift)| Binding {
        command: command.into(), key, primary, shift,
    }).collect()
}

pub fn load_bindings(editor: &mut EditorState) {
    if !editor.keyboard.bindings_loaded {
        if editor.keyboard.bindings.is_empty() { editor.keyboard.bindings = default_bindings(); }
        editor.keyboard.bindings_loaded = true;
    }
}

pub fn bound_command(editor: &EditorState, key: egui::Key, mods: egui::Modifiers) -> Option<&str> {
    if mods.alt { return None; }
    editor.keyboard.bindings.iter().find(|b| b.key == key
        && b.primary == mods.command && b.shift == mods.shift).map(|b| b.command.as_str())
}

pub fn open(editor: &mut EditorState, chart: &ChartDoc, kind: DialogKind) {
    let target = super::navigation::target(editor, chart);
    let note = chart.notes.iter().find(|n| Some(n.id) == target).cloned();
    let index = if kind == DialogKind::Tools {
        Tool::ALL.iter().position(|t| *t == editor.tool).unwrap_or(0)
    } else { 0 };
    editor.keyboard.dialog = Some(Dialog { kind, value: String::new(), index, request_focus: true, note });
    editor.keyboard.suppress_pointer = true;
}

pub fn confirm_key(editor: &mut EditorState, chart: &mut ChartDoc, key: egui::Key) {
    use egui::Key;
    match key {
        Key::Escape => editor.cancel_file_action(),
        Key::ArrowLeft => editor.keyboard.confirm_choice = (editor.keyboard.confirm_choice + 2) % 3,
        Key::ArrowRight | Key::Tab => editor.keyboard.confirm_choice = (editor.keyboard.confirm_choice + 1) % 3,
        Key::Enter => match editor.keyboard.confirm_choice {
            0 => { crate::io::save_chart(editor, chart); editor.pending_confirmed = !editor.dirty; }
            1 => editor.pending_confirmed = true,
            _ => editor.cancel_file_action(),
        },
        _ => {}
    }
}

pub fn cycle_focus(editor: &mut EditorState, _chart: &ChartDoc, reverse: bool) {
    let modes = [Focus::Timeline, Focus::Tools, Focus::Properties, Focus::Playback, Focus::Settings];
    let index = modes.iter().position(|f| *f == editor.keyboard.focus).unwrap_or(0);
    editor.keyboard.focus = modes[(index + if reverse { 4 } else { 1 }) % modes.len()];
}

pub fn draw(ctx: &egui::Context, editor: &mut EditorState, chart: &mut ChartDoc) {
    let Some(mut dialog) = editor.keyboard.dialog.take() else { return; };
    let pressed = |key| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    let mut done = pressed(egui::Key::Escape);
    let list_navigation = matches!(dialog.kind, DialogKind::Tools | DialogKind::Handles | DialogKind::Commands);
    let up = list_navigation && pressed(egui::Key::ArrowUp);
    let down = list_navigation && pressed(egui::Key::ArrowDown);
    let enter = pressed(egui::Key::Enter);
    let mut open = true;
    egui::Window::new(format!("{:?}", dialog.kind)).open(&mut open).collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
        let navigate = |index: &mut usize, count: usize| {
            if count == 0 { *index = 0; return; }
            *index = (*index).min(count - 1);
            if up { *index = (*index + count - 1) % count; }
            if down { *index = (*index + 1) % count; }
        };
        match dialog.kind {
            DialogKind::Tools => {
                navigate(&mut dialog.index, Tool::ALL.len());
                for (index, tool) in Tool::ALL.into_iter().enumerate() {
                    if ui.selectable_label(dialog.index == index, tool.label()).clicked() || (enter && !done && dialog.index == index) {
                        editor.tool = tool;
                        let lane = ((editor.keyboard.cursor.zone.saturating_sub(1)) % 8) + 1;
                        super::navigation::set_lane(editor, lane);
                        done = true;
                    }
                }
            }
            DialogKind::Handles => {
                let handles: &[(_, &str)] = match dialog.note.as_ref().map(|n| n.note_type) {
                    Some(NoteType::Hold | NoteType::TouchHold) => &[(edit::Handle::HoldTail, "Hold tail")],
                    Some(NoteType::Slide) => &[(edit::Handle::Delay, "Slide start"), (edit::Handle::Tail, "Slide tail"), (edit::Handle::Lane, "Slide endpoint lane")],
                    _ => &[],
                };
                navigate(&mut dialog.index, handles.len());
                for (index, &(handle, label)) in handles.iter().enumerate() {
                    if ui.selectable_label(dialog.index == index, label).clicked() || (enter && !done && dialog.index == index) {
                        edit::begin_handle(editor, chart, handle); done = true;
                    }
                }
                if handles.is_empty() { ui.label("Select one Hold or Star"); }
            }
            DialogKind::Jump => {
                let response = ui.text_edit_singleline(&mut dialog.value);
                if dialog.request_focus { response.request_focus(); }
                if ui.button("Jump").clicked() || (enter && !done) {
                    if let Some(measure) = parse_position(&dialog.value, chart) {
                        response.surrender_focus();
                        editor.keyboard.transport.push(super::Transport::Jump(measure_to_secs(measure, &chart.bpms)));
                        done = true;
                    } else { editor.status = Some("Invalid position".into()); }
                }
            }
            DialogKind::Properties => {
                if let Some(value) = &mut dialog.note {
                    let response = ui.add(egui::DragValue::new(&mut value.time).range(1.0..=100000.0));
                    if dialog.request_focus { response.request_focus(); }
                    ui.checkbox(&mut value.is_break, "Break"); ui.checkbox(&mut value.is_ex, "EX");
                    if ui.button("Apply").clicked() || (enter && !done) {
                        if let Some(index) = chart.notes.iter().position(|n| n.id == value.id) {
                            editor.push_history(chart);
                            let n = &mut chart.notes[index];
                            n.time = value.time; n.is_break = value.is_break; n.is_ex = value.is_ex;
                        }
                        done = true;
                    }
                } else { ui.label("Select one note"); }
            }
            DialogKind::DeleteSlide => {
                if ui.button("Delete active slide").clicked() || (enter && !done) {
                    let sub = editor.editing_slide_idx;
                    editor.edit_selected(chart, |n| {
                        if sub < n.slide.len() { n.slide.remove(sub); }
                        if n.slide.is_empty() { n.note_type = NoteType::Tap; }
                    });
                    editor.editing_slide = None; done = true;
                }
            }
            DialogKind::Commands => {
                let response = ui.text_edit_singleline(&mut dialog.value);
                if dialog.request_focus { response.request_focus(); }
                let options: Vec<_> = [("Tools", "tools"), ("Properties", "properties"),
                    ("Handles", "handles"), ("Jump", "jump"), ("Open chart", "open"),
                    ("Save chart", "save"), ("Save As", "save_as"), ("Export Simai", "export")].into_iter()
                    .filter(|(label, _)| label.to_lowercase().contains(&dialog.value.to_lowercase())).collect();
                navigate(&mut dialog.index, options.len());
                for (index, (label, command)) in options.into_iter().enumerate() {
                    if ui.selectable_label(dialog.index == index, label).clicked() || (enter && !done && dialog.index == index) {
                        response.surrender_focus();
                        match command {
                            "tools" => super::panels::open(editor, chart, DialogKind::Tools),
                            "properties" => super::panels::open(editor, chart, DialogKind::Properties),
                            "handles" => super::panels::open(editor, chart, DialogKind::Handles),
                            "jump" => super::panels::open(editor, chart, DialogKind::Jump),
                            "open" => editor.open_dialog = true,
                            "save" => crate::io::save_chart(editor, chart),
                            "save_as" => editor.save_as_request = true,
                            "export" => editor.export_request = true,
                            _ => {}
                        }
                        done = true;
                    }
                }
            }
        }
    });
    dialog.request_focus = false;
    if open && !done { editor.keyboard.dialog = Some(dialog); }
    else {
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        editor.keyboard.suppress_pointer = true;
    }
}

fn parse_position(text: &str, chart: &ChartDoc) -> Option<f32> {
    let text = text.trim();
    let measure = if let Some(seconds) = text.strip_suffix('s') {
        secs_to_measure(seconds.parse().ok()?, &chart.bpms)
    } else { text.parse().ok()? };
    (measure.is_finite() && measure >= 1.0).then_some(measure)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_do_not_steal_slide_shapes_with_modifiers() {
        let editor = EditorState::default();
        assert_eq!(bound_command(&editor, egui::Key::D, egui::Modifiers::SHIFT), Some("duplicate"));
        assert_eq!(bound_command(&editor, egui::Key::D, egui::Modifiers::NONE), None);
        assert_eq!(bound_command(&editor, egui::Key::D, egui::Modifiers::ALT), None);
    }
}
