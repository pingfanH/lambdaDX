//! Keyboard ownership, independent edit cursor, and editor commands.

pub mod edit;
pub mod navigation;
pub mod panels;

use egui_macroquad::egui;
use crate::app::types::{ChartDoc, SlideShape, measure_to_secs, secs_to_measure};
use crate::state::{EditorState, SelectionTransformKind, Tool};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditCursor {
    pub measure: f32,
    pub zone: u8,
    pub touch: bool,
}

impl Default for EditCursor {
    fn default() -> Self { Self { measure: 1.0, zone: 1, touch: false } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus { #[default] Timeline, Tools, Properties, Playback, Settings }

#[derive(Debug, Clone, Copy)]
pub enum Transport { Toggle, Seek(f32), Jump(f32), Cursor(f32), Speed(f32), Audio, Restart, Rewind }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorCommand { Jump, ToggleFollow }

pub fn cursor_command(key: egui::Key, mods: egui::Modifiers) -> Option<CursorCommand> {
    (key == egui::Key::Enter && mods.alt && !mods.command && !mods.ctrl && !mods.mac_cmd)
        .then_some(if mods.shift { CursorCommand::ToggleFollow } else { CursorCommand::Jump })
}

#[derive(Debug, Clone, Copy)]
pub enum ViewCommand {
    FitSelection, FitSong, Current, Follow, LoopSelection, ClearLoop,
    Settings, History, Maximize, SaveLayout, RestoreLayout, ResetLayout,
}

#[derive(Debug, Clone)]
pub struct KeyboardState {
    pub active: bool,
    pub initialized: bool,
    pub cursor: EditCursor,
    pub touch_ring: u8,
    pub candidate: Option<u64>,
    pub range: Option<(EditCursor, bool)>,
    pub detached: bool,
    pub rect: egui::Rect,
    pub focus: Focus,
    pub transaction: Option<edit::Transaction>,
    pub dialog: Option<panels::Dialog>,
    pub transport: Vec<Transport>,
    pub view_commands: Vec<ViewCommand>,
    pub numeric: String,
    pub segment: usize,
    pub bindings: Vec<panels::Binding>,
    pub bindings_loaded: bool,
    pub ime: bool,
    pub pending_path: Option<String>,
    pub confirm_choice: usize,
    pub suppress_pointer: bool,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            active: false, initialized: false, cursor: EditCursor::default(), touch_ring: 0,
            candidate: None, range: None, detached: false, rect: egui::Rect::NOTHING,
            focus: Focus::Timeline, transaction: None, dialog: None, transport: Vec::new(), view_commands: Vec::new(),
            numeric: String::new(), segment: 0, bindings: panels::default_bindings(),
            bindings_loaded: false, ime: false, pending_path: None, confirm_choice: 0,
            suppress_pointer: false,
        }
    }
}

impl KeyboardState {
    pub fn owns_input(&self) -> bool { self.transaction.is_some() || self.dialog.is_some() || self.range.is_some() || self.ime }
    pub fn reset_document(&mut self) {
        let bindings = self.bindings.clone();
        let loaded = self.bindings_loaded;
        *self = Self::default();
        self.bindings = bindings;
        self.bindings_loaded = loaded;
    }
}

pub fn initialize(editor: &mut EditorState, chart: &ChartDoc, now: f32) {
    if !editor.keyboard.initialized {
        editor.keyboard.cursor.measure = secs_to_measure(now, &chart.bpms).max(1.0);
        editor.keyboard.initialized = true;
    }
}

fn activate(editor: &mut EditorState) {
    editor.keyboard.active = true;
    editor.keyboard.detached = true;
    editor.keyboard.focus = Focus::Timeline;
}

pub fn apply_transport(editor: &mut EditorState, app: &mut crate::player::state::PadPreviewState) {
    for command in std::mem::take(&mut editor.keyboard.transport) {
        match command {
            Transport::Toggle => crate::runtime::toggle_play(editor, app),
            Transport::Seek(time) => app.seek_to(time),
            Transport::Cursor(time) => {
                if let Err(error) = app.try_seek_to(time) {
                    editor.status = Some(format!("Seek: {error}"));
                }
            }
            Transport::Jump(time) => match app.try_seek_to(time) {
                Ok(()) => {
                    let actual = app.timeline_view_time;
                    editor.keyboard.cursor.measure = secs_to_measure(actual, &app.chart.bpms);
                    editor.keyboard.candidate = None;
                    editor.view_start = actual;
                    activate(editor);
                }
                Err(error) => editor.status = Some(format!("Jump: {error}")),
            },
            Transport::Speed(delta) => app.nudge_play_speed(delta),
            Transport::Restart => crate::runtime::restart_playback(editor, app),
            Transport::Rewind => crate::runtime::rewind(editor, app),
            Transport::Audio => {
                app.audio_enabled = !app.audio_enabled;
                if !app.audio_enabled { app.stop_audio_if_any(); }
                else if app.mode == crate::app::types::Mode::Playing { app.request_audio_start(); }
            }
        }
    }
}

pub fn shape_key(key: egui::Key, shift: bool) -> Option<SlideShape> {
    use egui::Key;
    Some(match key {
        Key::Q if shift => SlideShape::QQ, Key::Q => SlideShape::Q,
        Key::P if shift => SlideShape::PP, Key::P => SlideShape::P,
        Key::V => SlideShape::VShape, Key::W => SlideShape::Wifi,
        Key::Minus => SlideShape::Line, Key::Comma => SlideShape::Left,
        Key::Period => SlideShape::Right, Key::Semicolon => SlideShape::Caret,
        Key::S => SlideShape::S, Key::Z => SlideShape::Z,
        _ => return None,
    })
}

pub fn lane_key(key: egui::Key) -> Option<u8> {
    use egui::Key;
    Some(match key {
        Key::Num1 => 1, Key::Num2 => 2, Key::Num3 => 3, Key::Num4 => 4,
        Key::Num5 => 5, Key::Num6 => 6, Key::Num7 => 7, Key::Num8 => 8,
        _ => return None,
    })
}

/// Help and file confirmations own keys in both editor workspaces.
pub fn handle_modals(ctx: &egui::Context, editor: &mut EditorState, chart: &mut ChartDoc) -> bool {
    let events = ctx.input(|i| i.events.clone());
    for event in &events {
        if let egui::Event::Ime(ime) = event {
            editor.keyboard.ime = !matches!(ime, egui::ImeEvent::Disabled | egui::ImeEvent::Commit(_));
        }
    }
    let modal = editor.show_help || editor.pending_action.is_some();
    if !modal { return false; }
    for event in events {
        let egui::Event::Key { key, pressed: true, repeat: false, .. } = event else { continue; };
        if editor.show_help {
            if matches!(key, egui::Key::Escape | egui::Key::F12 | egui::Key::Questionmark) { editor.show_help = false; }
        } else if editor.pending_action.is_some() {
            panels::confirm_key(editor, chart, key);
        }
    }
    ctx.input_mut(|i| i.events.retain(|event| !matches!(event, egui::Event::Key { .. })));
    true
}

/// Read individual events, not frame-wide key flags: confirmation cannot fall
/// through into placement, and OS key repeat is accepted only for navigation.
pub fn handle(ui: &egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc, now: f32) -> Option<bool> {
    initialize(editor, chart, now);
    panels::load_bindings(editor);
    editor.keyboard.suppress_pointer = false;
    if editor.follow_playhead && !editor.keyboard.detached && editor.layout == crate::state::EditorLayout::Vertical {
        editor.view_start = navigation::reference_time(editor, now);
    }
    if !editor.keyboard.owns_input() && editor.selection_transform.is_none()
        && ui.input(|i| i.pointer.any_pressed())
    {
        editor.keyboard.active = false;
        editor.keyboard.detached = !editor.follow_playhead;
    }
    let events = ui.input(|i| i.events.clone());
    let mut consumed = Vec::new();
    let was_modal = editor.keyboard.dialog.is_some() || editor.pending_action.is_some() || editor.show_help;
    let text_focus = ui.ctx().wants_keyboard_input();
    let mut repeated_navigation = false;
    for (index, event) in events.iter().enumerate() {
        if let egui::Event::Ime(ime) = event {
            editor.keyboard.ime = !matches!(ime, egui::ImeEvent::Disabled | egui::ImeEvent::Commit(_));
            continue;
        }
        let egui::Event::Key { key, pressed: true, repeat, modifiers, .. } = event else {
            if let egui::Event::Text(text) | egui::Event::Paste(text) = event {
                if !was_modal && !text_focus && editor.selection_transform.is_some() && !editor.keyboard.ime {
                    if text.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                        activate(editor);
                        editor.keyboard.numeric.push_str(text);
                        edit::numeric_transform(editor, chart);
                        consumed.push(index);
                    }
                }
            }
            continue;
        };
        if editor.show_help {
            if matches!(key, egui::Key::Escape | egui::Key::F12 | egui::Key::Questionmark) && !repeat { editor.show_help = false; }
            consumed.push(index);
            continue;
        }
        if editor.pending_action.is_some() {
            if !repeat { panels::confirm_key(editor, chart, *key); }
            consumed.push(index);
            continue;
        }
        if was_modal { continue; }
        if editor.keyboard.ime || text_focus { continue; }
        if editor.dragging.is_some() || editor.handle_grab.is_some() || editor.scrubbing || editor.marquee_start.is_some() {
            continue;
        }
        if *repeat && !matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown | egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::PageUp | egui::Key::PageDown | egui::Key::Backspace | egui::Key::Tab) {
            consumed.push(index);
            continue;
        }
        // An open modal owns the rest of this frame, including a second Enter.
        if editor.keyboard.dialog.is_some() { consumed.push(index); continue; }
        if *repeat && is_arrow(*key) && !modifiers.alt
            && editor.selection_transform.is_none() && editor.keyboard.transaction.is_none()
            && editor.star_placement.is_none() && editor.hold_placement.is_none()
            && !editor.is_slide_edit() && panels::bound_command(editor, *key, *modifiers).is_none()
        {
            if !repeated_navigation {
                let held: Vec<_> = [egui::Key::ArrowLeft, egui::Key::ArrowRight, egui::Key::ArrowUp, egui::Key::ArrowDown]
                    .into_iter().filter(|key| ui.input(|i| i.key_down(*key))).collect();
                activate(editor);
                navigation::repeat_held(editor, chart, &held, modifiers.shift);
                repeated_navigation = true;
            }
            consumed.push(index);
            continue;
        }
        dispatch(editor, chart, now, *key, *modifiers, *repeat, ui.input(|i| i.pointer.hover_pos()));
        consumed.push(index);
    }
    ui.input_mut(|i| {
        let mut index = 0;
        i.events.retain(|_| { let keep = !consumed.contains(&index); index += 1; keep });
    });
    if editor.keyboard.active { navigation::ensure_visible(editor, chart, now); }
    Some(true)
}

fn is_arrow(key: egui::Key) -> bool {
    matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown | egui::Key::ArrowLeft | egui::Key::ArrowRight)
}

fn dispatch(editor: &mut EditorState, chart: &mut ChartDoc, now: f32, key: egui::Key, mods: egui::Modifiers, repeat: bool, pointer: Option<egui::Pos2>) {
    use egui::Key;
    if (editor.workspace.is_viewer() || editor.mairis.annotation.active)
        && !editor.mairis.annotation.is_editing()
    {
        dispatch_viewer(editor, chart, now, key, mods, repeat);
        return;
    }
    if editor.mairis.annotation.is_editing() && editor.mairis.annotation.selected_ghost.is_some()
        && !mods.any() && matches!(key, Key::Delete | Key::Backspace)
    {
        crate::viewer::restore_selected_ghost(editor, chart);
        return;
    }
    let bound = panels::bound_command(editor, key, mods).map(str::to_owned);
    let command = bound.as_deref();
    if command == Some("help") || key == Key::Questionmark || (key == Key::Slash && mods.shift) { editor.show_help = true; return; }
    if command == Some("jump") { activate(editor); panels::open(editor, chart, panels::DialogKind::Jump); return; }
    if editor.selection_transform.is_some() {
        edit::transform_key(editor, chart, key, mods, repeat);
        return;
    }
    if editor.keyboard.transaction.is_some() {
        if command == Some("handles") { panels::open(editor, chart, panels::DialogKind::Handles); return; }
        edit::transaction_key(editor, chart, key, mods, repeat);
        return;
    }
    if editor.star_placement.is_some() || editor.hold_placement.is_some() {
        if key == Key::Escape { editor.star_placement = None; editor.hold_placement = None; }
        return;
    }
    if editor.is_slide_edit() {
        if edit::slide_key(editor, chart, key, mods, repeat) { return; }
    }
    if editor.keyboard.range.is_some() {
        if key == Key::Enter && !repeat {
            navigation::range(editor, chart, now, mods.shift);
            editor.keyboard.suppress_pointer = true;
            return;
        }
        if key == Key::Escape {
            editor.keyboard.range = None;
            editor.keyboard.suppress_pointer = true;
            return;
        }
        if mods.alt || command.is_some() || !(is_arrow(key) || lane_key(key).is_some()
            || matches!(key, Key::Home | Key::End | Key::PageUp | Key::PageDown | Key::Tab))
        { return; }
    }
    if let Some(command) = command {
        match command {
            "commands" => panels::open(editor, chart, panels::DialogKind::Commands),
            "tools" => panels::open(editor, chart, panels::DialogKind::Tools),
            "properties" => panels::open(editor, chart, panels::DialogKind::Properties),
            "handles" => panels::open(editor, chart, panels::DialogKind::Handles),
            "focus" => panels::cycle_focus(editor, chart, mods.shift),
            "snap" => editor.snap_enabled = !editor.snap_enabled,
            "select" => { activate(editor); navigation::select(editor, chart, now, mods.shift); }
            "play" => editor.keyboard.transport.push(Transport::Toggle),
            "save" => crate::io::save_document(editor, chart),
            "save_as" => editor.save_as_request = true,
            "export" => editor.export_request = true,
            "open" => editor.open_dialog = true,
            "undo" => editor.undo(chart), "redo" => editor.redo(chart),
            "copy" => editor.copy_selection(chart),
            "cut" => { editor.copy_selection(chart); editor.delete_selection(chart); }
            "paste" => { activate(editor); edit::paste(editor, chart); }
            "all" => { if mods.shift { editor.selected.clear(); } else { editor.selected = chart.notes.iter().map(|n| n.id).collect(); } }
            "duplicate" => {
                let anchor = pointer.unwrap_or(editor.keyboard.rect.center());
                crate::timeline::transform::duplicate(editor, chart, anchor);
                if let Some(t) = &mut editor.selection_transform { t.keyboard_driven = editor.keyboard.active; }
                editor.keyboard.numeric.clear();
            }
            "repeat" => {
                if editor.keyboard.active { edit::repeat(editor, chart); }
                else { crate::timeline::transform::repeat(editor, chart); }
                if let Some(note) = chart.notes.iter().find(|n| Some(n.id) == editor.single_selected()) {
                    editor.keyboard.cursor = navigation::note_cursor(note);
                    editor.keyboard.candidate = Some(note.id);
                }
            }
            _ => {}
        }
        return;
    }
    if mods.alt && !mods.command && !mods.ctrl {
        if cursor_command(key, mods) == Some(CursorCommand::ToggleFollow) {
            editor.keyboard_seek_on_navigation = !editor.keyboard_seek_on_navigation;
            navigation::follow_cursor(editor, chart);
            return;
        }
        let transport = match key {
            Key::Space => Some(Transport::Restart),
            Key::Enter => Some(Transport::Seek(measure_to_secs(editor.keyboard.cursor.measure, &chart.bpms))),
            Key::Home => Some(Transport::Rewind),
            Key::ArrowLeft => Some(Transport::Seek((now - 1.0).max(0.0))),
            Key::ArrowRight => Some(Transport::Seek(now + 1.0)),
            Key::ArrowUp => Some(Transport::Speed(0.1)), Key::ArrowDown => Some(Transport::Speed(-0.1)),
            _ => None,
        };
        if let Some(t) = transport { editor.keyboard.transport.push(t); }
        return;
    }
    if mods.command && key == Key::Enter {
        activate(editor); navigation::range(editor, chart, now, mods.shift); return;
    }
    if mods.command && matches!(key, Key::Delete | Key::Backspace) && !repeat {
        activate(editor); editor.delete_selection(chart); return;
    }
    if is_arrow(key) {
        activate(editor); navigation::navigate(editor, chart, now, key, mods.shift); return;
    }
    if mods.command { return; }
    if matches!(key, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End | Key::PageUp | Key::PageDown) {
        activate(editor); navigation::navigate(editor, chart, now, key, mods.shift); return;
    }
    if let Some(lane) = lane_key(key) {
        activate(editor); navigation::set_lane(editor, lane);
        navigation::follow_cursor(editor, chart); return;
    }
    if key == Key::Tab { activate(editor); navigation::cycle(editor, chart, mods.shift); return; }
    if key == Key::Enter && !repeat { activate(editor); edit::place(editor, chart); return; }
    if key == Key::Escape {
        if editor.keyboard.range.take().is_none() {
            if editor.selected.is_empty() { editor.tool = Tool::Select; } else { editor.selected.clear(); }
        }
        return;
    }
    if key == Key::Delete || (key == Key::Backspace && !repeat) || (key == Key::D && !mods.shift) {
        if navigation::cursor_mode(editor) {
            if let Some(id) = navigation::target(editor, chart) { editor.delete_note(chart, id); }
        } else { editor.delete_selection(chart); }
        return;
    }
    if !mods.shift && matches!(key, Key::G | Key::S | Key::R) && !editor.selected.is_empty() {
        let kind = match key { Key::G => SelectionTransformKind::Grab, Key::S => SelectionTransformKind::Scale, _ => SelectionTransformKind::Rotate };
        crate::timeline::transform::begin(editor, chart, kind, pointer.unwrap_or(editor.keyboard.rect.center()));
        editor.selection_transform.as_mut().unwrap().keyboard_driven = editor.keyboard.active;
        editor.keyboard.numeric.clear(); return;
    }
    if key == Key::E && !mods.shift && edit::begin_slide(editor, chart) { return; }
    if mods.shift { return; }
    match key {
        Key::V => editor.tool = Tool::Select, Key::T => editor.tool = Tool::Tap,
        Key::H => editor.tool = Tool::Hold, Key::C => editor.tool = Tool::Touch,
        Key::G => editor.tool = Tool::TouchHold, Key::S => editor.tool = Tool::Slide,
        Key::K => editor.tool = Tool::Break, Key::E => editor.tool = Tool::Eraser,
        Key::P => { editor.autoplay_mode = (editor.autoplay_mode + 1) % 4; editor.mode_dirty = true; }
        Key::O => { editor.autoplay_mode = if editor.autoplay_mode == 1 { 2 } else { 1 }; editor.mode_dirty = true; }
        Key::A => editor.keyboard.transport.push(Transport::Audio),
        Key::B | Key::X | Key::N if !editor.selected.is_empty() => editor.edit_selected(chart, |n| match key {
            Key::B => n.is_break = !n.is_break, Key::X => n.is_ex = !n.is_ex,
            _ => { let on = !(n.is_break && n.is_ex); n.is_break = on; n.is_ex = on; }
        }),
        Key::OpenBracket | Key::CloseBracket => navigation::division(editor, key == Key::CloseBracket),
        _ => {}
    }
    if matches!(key, Key::V | Key::T | Key::H | Key::C | Key::G | Key::S | Key::K | Key::E) {
        editor.keyboard.cursor.touch = matches!(editor.tool, Tool::Touch | Tool::TouchHold);
        navigation::set_lane(editor, ((editor.keyboard.cursor.zone.saturating_sub(1)) % 8) + 1);
    }
}

/// Viewer workspace: playback, seeking and view navigation stay available,
/// while every document mutation is ignored.
fn dispatch_viewer(
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    now: f32,
    key: egui::Key,
    mods: egui::Modifiers,
    _repeat: bool,
) {
    use egui::Key;
    let bound = panels::bound_command(editor, key, mods).map(str::to_owned);
    let command = bound.as_deref();
    if command == Some("help") || key == Key::Questionmark || (key == Key::Slash && mods.shift) {
        editor.show_help = true;
        return;
    }
    if command == Some("play") && !mods.alt {
        editor.keyboard.transport.push(Transport::Toggle);
        return;
    }
    if command == Some("focus") {
        panels::cycle_focus(editor, chart, mods.shift);
        return;
    }
    if mods.alt && !mods.command && !mods.ctrl {
        let transport = match key {
            Key::Space => Some(Transport::Restart),
            Key::Enter => Some(Transport::Seek(measure_to_secs(
                editor.keyboard.cursor.measure,
                &chart.bpms,
            ))),
            Key::Home => Some(Transport::Rewind),
            Key::ArrowLeft => Some(Transport::Seek((now - 1.0).max(0.0))),
            Key::ArrowRight => Some(Transport::Seek(now + 1.0)),
            Key::ArrowUp => Some(Transport::Speed(0.1)),
            Key::ArrowDown => Some(Transport::Speed(-0.1)),
            _ => None,
        };
        if let Some(command) = transport {
            editor.keyboard.transport.push(command);
        }
        return;
    }
    if is_arrow(key) || matches!(key, Key::Home | Key::End | Key::PageUp | Key::PageDown) {
        activate(editor);
        navigation::navigate(editor, chart, now, key, mods.shift);
        return;
    }
    if key == Key::Tab {
        activate(editor);
        navigation::cycle(editor, chart, mods.shift);
        return;
    }
    if key == Key::Escape {
        editor.selected.clear();
        editor.tool = Tool::Select;
        return;
    }
    if !mods.command && !mods.ctrl && !mods.alt && !mods.shift {
        match key {
            Key::P => {
                editor.autoplay_mode = (editor.autoplay_mode + 1) % 4;
                editor.mode_dirty = true;
            }
            Key::O => {
                editor.autoplay_mode = if editor.autoplay_mode == 1 { 2 } else { 1 };
                editor.mode_dirty = true;
            }
            Key::A => editor.keyboard.transport.push(Transport::Audio),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer_chart() -> ChartDoc {
        crate::app::maidata::from_maidata_level("&inote_6=(120){4}1,E", 6).unwrap()
    }

    #[test]
    fn annotation_review_blocks_keyboard_edits_in_chart_workspace() {
        let mut chart = viewer_chart();
        let original = chart.clone();
        let mut editor = EditorState::default();
        editor.mairis.annotation.active = true;
        editor.mairis.annotation.set_original(&chart.notes);
        editor.mairis.annotation.phase = crate::viewer::AnnotationPhase::Review;
        editor.select_only(chart.notes[0].id);
        for (key, modifiers) in [
            (egui::Key::Delete, egui::Modifiers::NONE),
            (egui::Key::Z, egui::Modifiers::COMMAND),
            (egui::Key::B, egui::Modifiers::NONE),
        ] {
            dispatch(&mut editor, &mut chart, 0.0, key, modifiers, false, None);
        }
        assert_eq!(chart, original);
        assert!(editor.undo.is_empty());
        dispatch(&mut editor, &mut chart, 0.0, egui::Key::Space, egui::Modifiers::NONE, false, None);
        assert!(matches!(editor.keyboard.transport.as_slice(), [Transport::Toggle]));
    }

    #[test]
    fn viewer_keeps_transport_and_navigation_but_ignores_edits() {
        let mut chart = viewer_chart();
        let id = chart.notes[0].id;
        let mut editor = EditorState {
            workspace: crate::state::Workspace::Viewer,
            ..Default::default()
        };
        editor.selected.insert(id);

        dispatch_viewer(
            &mut editor,
            &mut chart,
            1.0,
            egui::Key::Space,
            egui::Modifiers::NONE,
            false,
        );
        assert!(matches!(
            editor.keyboard.transport.as_slice(),
            [Transport::Toggle]
        ));

        dispatch_viewer(
            &mut editor,
            &mut chart,
            1.0,
            egui::Key::Delete,
            egui::Modifiers::NONE,
            false,
        );
        assert_eq!(chart.notes.len(), 1);
        assert!(editor.selected.contains(&id));

        dispatch_viewer(
            &mut editor,
            &mut chart,
            1.0,
            egui::Key::Home,
            egui::Modifiers::ALT,
            false,
        );
        assert!(matches!(
            editor.keyboard.transport.last(),
            Some(Transport::Rewind)
        ));
    }
}
