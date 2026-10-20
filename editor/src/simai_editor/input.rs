//! Event ownership and a common transaction stream for all Simai edit styles.

use super::{
    Options,
    completion::{self, Completion},
    editing::{self as edit, History, Motion, Navigation, Selection},
    modal::{Modal, Mode, Style},
};
use egui_macroquad::egui::{Event, ImeEvent, Key};

#[derive(Debug, Clone, Default)]
pub struct Input {
    pub modal: Modal,
    pub completion: Completion,
    pub selection: Selection,
    history: History,
    style: Style,
    navigation: Navigation,
}

#[derive(Default)]
pub struct Output {
    pub copied: Option<String>,
    pub changed: bool,
}

impl Input {
    pub fn cursor(&self, text: &str, options: &Options) -> usize {
        self.modal.cursor(text, self.selection, options.edit_style)
    }

    pub fn pointer_selection(&mut self, selection: Selection) {
        self.navigation.reset();
        self.history.boundary();
        self.completion.reset();
        self.modal.pointer_selection(selection);
        self.selection = selection;
    }
    pub fn inserting(&self, options: &Options) -> bool {
        options.edit_style == Style::Standard || self.modal.mode == Mode::Insert
    }
    pub fn owns_text(&self, options: &Options) -> bool {
        options.edit_style != Style::Standard && !self.inserting(options)
    }

    pub fn process(
        &mut self,
        events: &mut Vec<Event>,
        text: &mut String,
        options: &Options,
        ime: bool,
        now: f64,
    ) -> Output {
        if self.style != options.edit_style {
            self.style = options.edit_style;
            self.modal = Modal::default();
            self.completion.reset();
            self.history.boundary();
            self.navigation.reset();
        }
        self.selection.clamp(text);
        if !options.completion {
            self.completion.reset();
        }
        let mut result = Output::default();
        // Native IME edits stay with TextEdit. Their resulting transaction is
        // observed after rendering, not interpreted as modal commands.
        if ime
            || events
                .iter()
                .any(|e| matches!(e, Event::Ime(i) if !matches!(i, ImeEvent::Disabled)))
        {
            self.completion.close();
            if options.edit_style != Style::Standard {
                self.modal.mode = Mode::Insert;
            }
            return result;
        }
        let original = text.clone();
        let mut kept = Vec::new();
        for event in std::mem::take(events) {
            let before = text.clone();
            let selection = self.selection;
            let mut typing = false;
            let mut history = None;
            let mut consumed = true;
            match &event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    repeat,
                    ..
                } => {
                    if *key == Key::Escape {
                        self.history.boundary();
                        if self.completion.open() {
                            self.completion.close();
                        } else if !self.completion.fields.is_empty() {
                            self.completion.fields.clear();
                        } else if options.edit_style != Style::Standard {
                            self.modal
                                .escape(&mut self.selection, options.edit_style, text);
                        } else {
                            consumed = false;
                        }
                    } else if *key == Key::Space
                        && modifiers.ctrl
                        && !modifiers.alt
                        && !modifiers.mac_cmd
                    {
                        if options.completion && self.inserting(options) {
                            self.completion.update(text, self.selection.head, true);
                        }
                    } else if modifiers.command && *key == Key::Z {
                        history = Some(modifiers.shift);
                    } else if modifiers.command && *key == Key::A {
                        self.selection = Selection {
                            anchor: 0,
                            head: text.chars().count(),
                        };
                        self.history.boundary();
                        self.completion.reset();
                    } else if (modifiers.command && *key == Key::Y)
                        || (modifiers.ctrl && *key == Key::R && options.edit_style == Style::Vim)
                    {
                        history = Some(true);
                    } else if self.completion.open()
                        && modifiers.is_none()
                        && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Enter | Key::Tab)
                    {
                        match key {
                            Key::ArrowUp => {
                                self.completion.selected = (self.completion.selected
                                    + self.completion.candidates.len()
                                    - 1)
                                    % self.completion.candidates.len()
                            }
                            Key::ArrowDown => {
                                self.completion.selected = (self.completion.selected + 1)
                                    % self.completion.candidates.len()
                            }
                            _ => {
                                self.completion.accept(text, &mut self.selection);
                                self.history.boundary();
                            }
                        }
                    } else if *key == Key::Tab
                        && !modifiers.command
                        && !modifiers.ctrl
                        && !modifiers.alt
                        && self
                            .completion
                            .jump_field(&mut self.selection, modifiers.shift)
                    {
                    } else if self.modal.search.is_some() && *key == Key::Enter {
                        self.modal
                            .submit_search(text, &mut self.selection, options.edit_style);
                    } else if self.modal.search.is_some() && *key == Key::Backspace {
                        if let Some(query) = &mut self.modal.search {
                            query.pop();
                        }
                    } else if self.inserting(options)
                        && matches!(
                            key,
                            Key::ArrowLeft
                                | Key::ArrowRight
                                | Key::ArrowUp
                                | Key::ArrowDown
                                | Key::Home
                                | Key::End
                        )
                    {
                        self.insert_motion(text, *key, *modifiers);
                        self.history.boundary();
                        self.completion.reset();
                    } else if !self.inserting(options)
                        && modifiers.ctrl
                        && matches!(key, Key::D | Key::U)
                    {
                        for _ in 0..10 {
                            self.modal.motion(
                                text,
                                &mut self.selection,
                                if *key == Key::D {
                                    Motion::Down
                                } else {
                                    Motion::Up
                                },
                                options.edit_style,
                            );
                        }
                    } else if !self.inserting(options)
                        && !modifiers.command
                        && !modifiers.ctrl
                        && !modifiers.alt
                    {
                        let motion = match key {
                            Key::ArrowLeft => Some(Motion::Left),
                            Key::ArrowRight => Some(Motion::Right),
                            Key::ArrowUp => Some(Motion::Up),
                            Key::ArrowDown => Some(Motion::Down),
                            Key::Home => Some(Motion::Start),
                            Key::End => Some(Motion::End),
                            _ => None,
                        };
                        if let Some(motion) = motion {
                            self.modal.motion(
                                text,
                                &mut self.selection,
                                motion,
                                options.edit_style,
                            );
                        } else if *key == Key::Delete || *key == Key::Backspace {
                            let ch = if options.edit_style == Style::Vim
                                && self.modal.mode == Mode::Normal
                            {
                                'x'
                            } else {
                                'd'
                            };
                            let output = self.modal.character(
                                text,
                                &mut self.selection,
                                ch,
                                options.edit_style,
                            );
                            result.copied = output.copied;
                        }
                        // Printable modal commands are dispatched by Text,
                        // never by both Key and Text (which may be separate frames).
                    } else if self.inserting(options)
                        && !modifiers.command
                        && !modifiers.ctrl
                        && !modifiers.alt
                    {
                        match key {
                            Key::Backspace | Key::Delete => {
                                let mut range = self.selection.range();
                                if range.is_empty() {
                                    if *key == Key::Backspace {
                                        range.start = range.start.saturating_sub(1);
                                    } else {
                                        range.end = (range.end + 1).min(text.chars().count());
                                    }
                                }
                                self.selection = edit::replace(text, range, "");
                                typing = true;
                                self.completion.close();
                            }
                            Key::Enter => {
                                let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
                                self.selection =
                                    edit::replace(text, self.selection.range(), newline);
                                self.completion.reset();
                            }
                            Key::Tab => {
                                self.selection =
                                    edit::replace(text, self.selection.range(), "    ");
                                self.completion.reset();
                            }
                            Key::ArrowUp
                            | Key::ArrowDown
                            | Key::ArrowLeft
                            | Key::ArrowRight
                            | Key::Home
                            | Key::End
                            | Key::PageUp
                            | Key::PageDown => {
                                consumed = false;
                                self.history.boundary();
                                self.completion.close();
                            }
                            _ => {
                                consumed = false;
                            }
                        }
                    } else {
                        consumed = false;
                    }
                    if *repeat && *key == Key::Escape {
                        consumed = true;
                    }
                }
                Event::Key { .. } => {}
                Event::Text(value) if self.modal.search.is_some() || !self.inserting(options) => {
                    for ch in value.chars() {
                        if self.inserting(options) && self.modal.search.is_none() {
                            self.selection =
                                edit::replace(text, self.selection.range(), &ch.to_string());
                        } else {
                            let output = self.modal.character(
                                text,
                                &mut self.selection,
                                ch,
                                options.edit_style,
                            );
                            if output.history.is_some() {
                                history = output.history;
                            }
                            if output.copied.is_some() {
                                result.copied = output.copied;
                            }
                        }
                    }
                    self.history.boundary();
                    self.completion.reset();
                }
                Event::Text(value) => {
                    typing = true;
                    for ch in value
                        .chars()
                        .filter(|c| *c != '\n' && *c != '\r' && *c != '\t')
                    {
                        if !options.auto_pairs
                            || !completion::paired_input(text, &mut self.selection, ch)
                        {
                            self.selection =
                                edit::replace(text, self.selection.range(), &ch.to_string());
                        }
                    }
                    if options.completion
                        && options.auto_completion
                        && self.completion.fields.is_empty()
                    {
                        self.completion.update(text, self.selection.head, false);
                    }
                }
                Event::Paste(value) => {
                    self.selection = edit::replace(text, self.selection.range(), value);
                    self.completion.reset();
                }
                Event::Copy | Event::Cut => {
                    result.copied = Some(edit::slice(text, self.selection.range()).to_owned());
                    if matches!(event, Event::Cut) {
                        self.selection = edit::replace(text, self.selection.range(), "");
                    }
                    self.completion.reset();
                }
                _ => consumed = false,
            }
            if let Some(redo) = history {
                self.navigation.reset();
                self.history.restore(text, &mut self.selection, redo);
                self.completion.reset();
            } else {
                if before != *text {
                    self.navigation.reset();
                }
                self.history.record(&before, selection, text, typing, now);
                if !matches!(
                    event,
                    Event::Key {
                        key: Key::Tab | Key::Enter,
                        ..
                    }
                ) {
                    self.completion.text_changed(&before, text);
                }
            }
            if !consumed {
                kept.push(event);
            }
        }
        *events = kept;
        result.changed = original != *text;
        result
    }

    pub fn observe_native(&mut self, before: &str, selection: Selection, text: &str, now: f64) {
        self.history.record(before, selection, text, true, now);
        self.completion.text_changed(before, text);
    }

    fn insert_motion(&mut self, text: &str, key: Key, modifiers: egui_macroquad::egui::Modifiers) {
        let range = self.selection.range();
        let at = self.selection.head;
        let len = text.chars().count();
        let row = edit::line(text, at);
        let word = modifiers.ctrl || modifiers.alt;
        if !matches!(key, Key::ArrowUp | Key::ArrowDown) {
            self.navigation.reset();
        }
        let to = match key {
            Key::ArrowLeft if !modifiers.shift && !range.is_empty() => range.start,
            Key::ArrowRight if !modifiers.shift && !range.is_empty() => range.end,
            Key::ArrowLeft if modifiers.mac_cmd => row.start,
            Key::ArrowRight if modifiers.mac_cmd => row.end,
            Key::ArrowLeft if word => edit::motion(text, at, Motion::BackWord, 1),
            Key::ArrowRight if word => edit::motion(text, at, Motion::Word, 1),
            Key::ArrowLeft => at.saturating_sub(1),
            Key::ArrowRight => (at + 1).min(len),
            Key::Home => {
                if modifiers.ctrl {
                    0
                } else {
                    row.start
                }
            }
            Key::End => {
                if modifiers.ctrl {
                    len
                } else {
                    row.end
                }
            }
            Key::ArrowUp => self.navigation.target(text, at, Motion::Up, 1),
            Key::ArrowDown => self.navigation.target(text, at, Motion::Down, 1),
            _ => at,
        };
        if modifiers.shift {
            self.selection.head = to;
        } else {
            self.selection = Selection::caret(to);
        }
    }

    pub fn accept(&mut self, text: &mut String, now: f64) {
        let before = text.clone();
        let selection = self.selection;
        self.completion.accept(text, &mut self.selection);
        self.history.record(&before, selection, text, false, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_macroquad::egui::Modifiers;
    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            modifiers,
            pressed: true,
            repeat: false,
            physical_key: None,
        }
    }
    fn run(input: &mut Input, text: &mut String, options: &Options, events: Vec<Event>) {
        input.process(&mut events.clone(), text, options, false, 0.0);
    }
    #[test]
    fn modal_key_and_text_in_separate_frames_execute_once() {
        for style in [Style::Vim, Style::Helix] {
            let mut input = Input::default();
            let mut text = String::from("1,2,E");
            let options = Options {
                edit_style: style,
                ..Default::default()
            };
            run(
                &mut input,
                &mut text,
                &options,
                vec![key(Key::I, Modifiers::NONE)],
            );
            assert_eq!(text, "1,2,E");
            run(
                &mut input,
                &mut text,
                &options,
                vec![Event::Text("i".into())],
            );
            assert_eq!(input.modal.mode, Mode::Insert);
            run(
                &mut input,
                &mut text,
                &options,
                vec![Event::Text(" ".into())],
            );
            assert_eq!(text, " 1,2,E");
        }
    }
    #[test]
    fn completion_undo_and_paste_share_history() {
        let mut input = Input::default();
        let mut text = String::from("1h");
        input.selection = Selection::caret(2);
        let options = Options::default();
        run(
            &mut input,
            &mut text,
            &options,
            vec![Event::Text("[".into())],
        );
        assert!(input.completion.open());
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::Tab, Modifiers::NONE)],
        );
        assert_eq!(text, "1h[4:1]");
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::Z, Modifiers::COMMAND)],
        );
        assert_eq!(text, "1h[]");
        run(
            &mut input,
            &mut text,
            &options,
            vec![Event::Paste("[8:1]".into())],
        );
        assert_eq!(text, "1h[[8:1]]");
    }

    #[test]
    fn modal_preferences_roundtrip_and_legacy_layouts_keep_standard_mode() {
        assert_eq!(
            serde_json::from_str::<Options>("{}").unwrap().edit_style,
            Style::Standard
        );
        for style in [Style::Standard, Style::Vim, Style::Helix] {
            let options = Options {
                edit_style: style,
                completion: false,
                auto_pairs: false,
                ..Default::default()
            };
            let restored: Options =
                serde_json::from_str(&serde_json::to_string(&options).unwrap()).unwrap();
            assert_eq!(restored, options);
        }
    }

    #[test]
    fn ime_and_paste_are_not_modal_commands_or_paired_input() {
        let mut input = Input::default();
        let mut text = String::from("1,E");
        let options = Options {
            edit_style: Style::Vim,
            ..Default::default()
        };
        let mut events = vec![Event::Ime(ImeEvent::Preedit("注".into()))];
        input.process(&mut events, &mut text, &options, false, 0.0);
        assert_eq!(events.len(), 1);
        assert_eq!(input.modal.mode, Mode::Insert);
        run(
            &mut input,
            &mut text,
            &options,
            vec![Event::Paste("[4:1]".into())],
        );
        assert_eq!(text, "[4:1]1,E");
    }

    #[test]
    fn same_frame_navigation_and_select_all_use_latest_selection() {
        let mut input = Input::default();
        let mut text = String::from("1,2,E");
        let options = Options {
            auto_completion: false,
            ..Default::default()
        };
        run(
            &mut input,
            &mut text,
            &options,
            vec![
                key(Key::ArrowRight, Modifiers::NONE),
                Event::Text("h".into()),
            ],
        );
        assert_eq!(text, "1h,2,E");
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::A, Modifiers::COMMAND), Event::Paste("3,E".into())],
        );
        assert_eq!(text, "3,E");
    }

    #[test]
    fn vertical_cursor_returns_to_its_column_after_short_lines() {
        let mut input = Input::default();
        let mut text = String::from("123456\nx\nabcdef");
        let options = Options {
            auto_completion: false,
            ..Default::default()
        };
        input.selection = Selection::caret(5);
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::ArrowDown, Modifiers::NONE)],
        );
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::ArrowDown, Modifiers::NONE)],
        );
        assert_eq!(input.selection.head, 14);
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::ArrowUp, Modifiers::NONE)],
        );
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::ArrowUp, Modifiers::NONE)],
        );
        assert_eq!(input.selection.head, 5);
    }

    #[test]
    fn snippet_fields_survive_real_key_press_release_sequences() {
        let mut input = Input::default();
        let mut text = String::from("1h");
        input.selection = Selection::caret(2);
        let options = Options::default();
        run(
            &mut input,
            &mut text,
            &options,
            vec![Event::Text("[".into())],
        );
        let mut release = key(Key::Tab, Modifiers::NONE);
        if let Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::Tab, Modifiers::NONE), release],
        );
        assert_eq!(input.completion.fields.len(), 2);
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::Num1, Modifiers::NONE), Event::Text("16".into())],
        );
        assert_eq!(text, "1h[16:1]");
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::Tab, Modifiers::NONE)],
        );
        assert_eq!(edit::slice(&text, input.selection.range()), "1");
        run(
            &mut input,
            &mut text,
            &options,
            vec![key(Key::Tab, Modifiers::SHIFT)],
        );
        assert_eq!(edit::slice(&text, input.selection.range()), "16");
    }
}
