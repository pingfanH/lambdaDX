//! Embedded Vim and Helix command subsets; no UI or gameplay dependencies.

use super::editing::{self as edit, Motion, Navigation, Selection};
use std::ops::Range;
mod search;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Style {
    #[default]
    Standard,
    Vim,
    Helix,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Select,
    Line,
}

#[derive(Debug, Clone, Copy)]
enum Pending {
    Go,
    Operator(char),
    Object(char, bool),
    Match,
    MatchObject(bool),
    Find(char),
}

#[derive(Debug, Clone, Default)]
pub struct Modal {
    pub mode: Mode,
    pub search: Option<String>,
    last_search: search::Search,
    search_backwards: bool,
    pending: Option<Pending>,
    count: usize,
    operator_count: usize,
    register: String,
    linewise: bool,
    last_find: Option<(char, char)>,
    visual_anchor: Option<usize>,
    visual_head: Option<usize>,
    navigation: Navigation,
}

#[derive(Default)]
pub struct Outcome {
    pub history: Option<bool>,
    pub copied: Option<String>,
}

impl Modal {
    /// Selections use an exclusive end, while a modal cursor names a character.
    pub fn cursor(&self, text: &str, selection: Selection, style: Style) -> usize {
        let at = if self.mode == Mode::Insert || style == Style::Standard {
            selection.head
        } else if matches!(self.mode, Mode::Select | Mode::Line) {
            self.visual_head.unwrap_or_else(|| {
                selection
                    .head
                    .saturating_sub(usize::from(selection.head > selection.anchor))
            })
        } else if style == Style::Helix || !selection.range().is_empty() {
            selection
                .head
                .saturating_sub(usize::from(selection.head > selection.anchor))
        } else {
            selection.head
        };
        if self.mode == Mode::Insert || style == Style::Standard {
            at.min(text.chars().count())
        } else {
            edit::normal_cursor(text, at)
        }
    }

    pub fn pointer_selection(&mut self, selection: Selection) {
        self.navigation.reset();
        self.pending = None;
        self.count = 0;
        self.operator_count = 0;
        if matches!(self.mode, Mode::Select | Mode::Line) {
            self.visual_anchor = Some(
                selection
                    .anchor
                    .saturating_sub(usize::from(selection.anchor > selection.head)),
            );
            self.visual_head = Some(if selection.head > selection.anchor {
                selection.head - 1
            } else {
                selection.head
            });
        }
    }
    pub fn label(&self, style: Style) -> String {
        if let Some(query) = &self.search {
            return format!("{}{query}", if self.search_backwards { '?' } else { '/' });
        }
        let mode = match (style, self.mode) {
            (Style::Standard, _) => return String::new(),
            (_, Mode::Insert) => "INSERT",
            (Style::Vim, Mode::Select) => "VISUAL",
            (_, Mode::Select) => "SELECT",
            (_, Mode::Line) => "VISUAL LINE",
            _ => "NORMAL",
        };
        let command = match self.pending {
            Some(Pending::Go) => "g".into(),
            Some(Pending::Operator(ch)) => ch.to_string(),
            Some(Pending::Object(ch, around)) => format!("{ch}{}", if around { 'a' } else { 'i' }),
            Some(Pending::Match) => "m".into(),
            Some(Pending::MatchObject(around)) => format!("m{}", if around { 'a' } else { 'i' }),
            Some(Pending::Find(ch)) => ch.to_string(),
            None => String::new(),
        };
        format!(
            "{mode} {}{command}",
            if self.count == 0 {
                String::new()
            } else {
                self.count.to_string()
            }
        )
    }

    pub fn escape(&mut self, selection: &mut Selection, style: Style, text: &str) {
        if self.search.take().is_some() {
            return;
        }
        let at = if self.mode == Mode::Insert && style == Style::Vim {
            selection
                .head
                .saturating_sub(1)
                .max(edit::line(text, selection.head).start)
        } else {
            self.cursor(text, *selection, style)
        };
        self.navigation.reset();
        self.mode = Mode::Normal;
        self.visual_anchor = None;
        self.visual_head = None;
        self.pending = None;
        self.count = 0;
        self.operator_count = 0;
        *selection = Selection::caret(edit::normal_cursor(text, at));
    }

    pub fn motion(
        &mut self,
        text: &str,
        selection: &mut Selection,
        command: Motion,
        style: Style,
    ) -> Outcome {
        let count = self.take_count();
        let from = if style == Style::Helix
            && self.mode == Mode::Normal
            && !selection.range().is_empty()
        {
            match command {
                Motion::Word => selection.range().end,
                Motion::BackWord => self.cursor(text, *selection, style),
                _ => self.cursor(text, *selection, style),
            }
        } else {
            self.cursor(text, *selection, style)
        };
        let to = self.navigation.target(text, from, command, count);
        let to = if matches!(command, Motion::Word) {
            to
        } else {
            edit::normal_cursor(text, to)
        };
        if style == Style::Helix
            && self.mode == Mode::Normal
            && !matches!(command, Motion::Word | Motion::BackWord | Motion::EndWord)
        {
            *selection = Selection {
                anchor: to,
                head: (to + 1).min(text.chars().count()),
            };
        } else {
            self.move_to(
                text,
                selection,
                from,
                to,
                style,
                matches!(command, Motion::EndWord | Motion::End | Motion::Bottom),
            );
        }
        Outcome::default()
    }

    fn take_count(&mut self) -> usize {
        let count = self
            .count
            .max(1)
            .saturating_mul(self.operator_count.max(1))
            .min(100_000);
        self.count = 0;
        self.operator_count = 0;
        count
    }

    fn move_to(
        &mut self,
        text: &str,
        selection: &mut Selection,
        from: usize,
        to: usize,
        style: Style,
        inclusive: bool,
    ) {
        let len = text.chars().count();
        if self.mode == Mode::Line {
            self.visual_head = Some(to);
            let anchor = edit::line(text, self.visual_anchor.unwrap_or(selection.anchor)).start;
            let end = edit::lines(text, to, 1);
            *selection = if to < anchor {
                Selection {
                    anchor: edit::lines(text, anchor, 1).end,
                    head: end.start,
                }
            } else {
                Selection {
                    anchor,
                    head: end.end,
                }
            };
        } else if self.mode == Mode::Select {
            self.visual_head = Some(to);
            let anchor = self.visual_anchor.unwrap_or(selection.anchor);
            *selection = if to >= anchor {
                Selection {
                    anchor,
                    head: (to + 1).min(len),
                }
            } else {
                Selection {
                    anchor: (anchor + 1).min(len),
                    head: to,
                }
            };
        } else if style == Style::Helix {
            *selection = if to < from {
                Selection {
                    anchor: from.saturating_add(1).min(len),
                    head: to,
                }
            } else {
                Selection {
                    anchor: from,
                    head: to.saturating_add(usize::from(inclusive)).min(len),
                }
            };
            if selection.range().is_empty() && to < len {
                selection.head = to + 1;
            }
        } else {
            *selection = Selection::caret(to);
        }
    }

    pub fn character(
        &mut self,
        text: &mut String,
        selection: &mut Selection,
        ch: char,
        style: Style,
    ) -> Outcome {
        selection.clamp(text);
        if let Some(query) = &mut self.search {
            query.push(ch);
            return Outcome::default();
        }
        if self.mode == Mode::Insert {
            return Outcome::default();
        }
        let cursor = self.cursor(text, *selection, style);
        if self.mode == Mode::Normal && selection.range().is_empty() {
            *selection = Selection::caret(cursor);
        }
        if let Some(pending) = self.pending.take() {
            match pending {
                Pending::Go => {
                    if ch == 'g' && self.count > 0 {
                        let row = edit::line_number(text, self.take_count());
                        self.jump(text, selection, row, style);
                        return Outcome::default();
                    }
                    let command = match ch {
                        'g' => Some(Motion::Top),
                        'e' if style == Style::Helix => Some(Motion::Bottom),
                        'h' if style == Style::Helix => Some(Motion::Start),
                        'l' if style == Style::Helix => Some(Motion::End),
                        _ => None,
                    };
                    if let Some(command) = command {
                        return self.motion(text, selection, command, style);
                    }
                }
                Pending::Operator(operator) => {
                    if ch.is_ascii_digit() && (ch != '0' || self.count > 0) {
                        self.count =
                            (self.count * 10 + ch.to_digit(10).unwrap() as usize).min(100_000);
                        self.pending = Some(pending);
                        return Outcome::default();
                    }
                    if matches!(ch, 'i' | 'a') {
                        self.pending = Some(Pending::Object(operator, ch == 'a'));
                        return Outcome::default();
                    }
                    let count = self.take_count();
                    let linewise = ch == operator || matches!(ch, 'j' | 'k');
                    let range = if ch == operator {
                        Some(edit::lines(text, cursor, count))
                    } else if matches!(ch, 'j' | 'k') {
                        let to = edit::motion(text, cursor, movement(ch).unwrap(), count);
                        Some(
                            edit::line(text, cursor.min(to)).start
                                ..edit::lines(text, cursor.max(to), 1).end,
                        )
                    } else if operator == 'c' && ch == 'w' {
                        operator_range(text, cursor, 'e', count)
                    } else {
                        operator_range(text, cursor, ch, count)
                    };
                    if let Some(mut range) = range {
                        if operator == 'c' && linewise {
                            range.end = range.end.saturating_sub(usize::from(
                                edit::slice(text, range.clone()).ends_with('\n'),
                            ));
                            if edit::slice(text, range.clone()).ends_with('\r') {
                                range.end -= 1;
                            }
                        }
                        return self.operate(text, selection, range, operator, linewise);
                    }
                }
                Pending::Object(operator, around) => {
                    if let Some(range) = edit::object(text, cursor, ch, around) {
                        return self.operate(text, selection, range, operator, false);
                    }
                }
                Pending::Match => {
                    if matches!(ch, 'i' | 'a') {
                        self.pending = Some(Pending::MatchObject(ch == 'a'));
                    }
                }
                Pending::MatchObject(around) => {
                    if let Some(range) = edit::object(text, cursor, ch, around) {
                        *selection = Selection {
                            anchor: range.start,
                            head: range.end,
                        };
                    }
                }
                Pending::Find(kind) => {
                    self.last_find = Some((kind, ch));
                    self.find(text, selection, kind, ch, style);
                }
            }
            return Outcome::default();
        }
        if ch.is_ascii_digit() && (ch != '0' || self.count > 0) {
            self.count = (self.count * 10 + ch.to_digit(10).unwrap() as usize).min(100_000);
            return Outcome::default();
        }
        if let Some(command) = movement(ch) {
            return self.motion(text, selection, command, style);
        }
        match ch {
            'g' => self.pending = Some(Pending::Go),
            'G' => {
                if self.count > 0 {
                    let row = edit::line_number(text, self.take_count());
                    self.jump(text, selection, row, style);
                    return Outcome::default();
                }
                return self.motion(text, selection, Motion::Bottom, style);
            }
            'i' | 'a' | 'I' | 'A' => {
                self.navigation.reset();
                let row = edit::line(text, cursor);
                let at = match ch {
                    'I' => edit::motion(text, cursor, Motion::First, 1),
                    'A' => row.end,
                    'a' if style == Style::Helix => selection.range().end,
                    'i' if style == Style::Helix => selection.range().start,
                    'a' => (cursor + 1).min(row.end),
                    _ => cursor,
                };
                *selection = Selection::caret(at);
                self.mode = Mode::Insert;
            }
            'o' | 'O' => {
                self.navigation.reset();
                let row = edit::line(text, cursor);
                let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
                let at = if ch == 'O' {
                    row.start
                } else {
                    edit::lines(text, cursor, 1).end
                };
                let append = ch == 'o'
                    && at == text.chars().count()
                    && !text.ends_with('\n')
                    && !text.is_empty();
                let value = newline.to_owned();
                edit::replace(text, at..at, &value);
                *selection = Selection::caret(if append { at + newline.len() } else { at });
                self.mode = Mode::Insert;
            }
            'v' | 'V' => {
                if self.mode != Mode::Normal {
                    self.mode = Mode::Normal;
                    *selection = Selection::caret(self.visual_head.unwrap_or(selection.head));
                    self.visual_anchor = None;
                    self.visual_head = None;
                } else {
                    self.visual_anchor = Some(
                        selection
                            .anchor
                            .saturating_sub(usize::from(selection.anchor > selection.head)),
                    );
                    self.visual_head = Some(cursor);
                    if ch == 'V' && style == Style::Vim {
                        self.mode = Mode::Line;
                        let r = edit::lines(text, cursor, 1);
                        *selection = Selection {
                            anchor: r.start,
                            head: r.end,
                        };
                    } else {
                        self.mode = Mode::Select;
                        selection.head = (selection.head
                            + usize::from(selection.range().is_empty()))
                        .min(text.chars().count());
                    }
                }
            }
            'd' | 'c' | 'y' if style == Style::Vim && self.mode == Mode::Normal => {
                self.operator_count = self.count.max(1);
                self.count = 0;
                self.pending = Some(Pending::Operator(ch));
            }
            'd' | 'c' | 'y' => {
                let range = current(text, *selection);
                return self.operate(text, selection, range, ch, self.mode == Mode::Line);
            }
            'x' if style == Style::Helix => {
                let count = self.take_count();
                let range = edit::lines(text, selection.range().start, count);
                *selection = Selection {
                    anchor: range.start,
                    head: range.end,
                };
            }
            'x' | 'D' | 'C' => {
                let range = if ch == 'x' {
                    let r = current(text, *selection);
                    r.start..(r.start + self.take_count()).min(edit::line(text, r.start).end)
                } else {
                    cursor..edit::line(text, cursor).end
                };
                return self.operate(
                    text,
                    selection,
                    range,
                    if ch == 'C' { 'c' } else { 'd' },
                    false,
                );
            }
            'p' | 'P' => {
                let count = self.take_count().min(1000);
                let at = if self.linewise {
                    if ch == 'p' {
                        edit::lines(text, selection.head, 1).end
                    } else {
                        edit::line(text, selection.head).start
                    }
                } else if ch == 'p' {
                    (selection.range().end + usize::from(selection.range().is_empty()))
                        .min(text.chars().count())
                } else {
                    selection.range().start
                };
                let mut value = self.register.repeat(count);
                if self.linewise
                    && ch == 'p'
                    && at > 0
                    && !edit::slice(text, at - 1..at).ends_with('\n')
                {
                    value.insert_str(0, if text.contains("\r\n") { "\r\n" } else { "\n" });
                }
                *selection = edit::replace(text, at..at, &value);
                selection.head = at;
                selection.anchor = at;
            }
            'u' => {
                return Outcome {
                    history: Some(false),
                    ..Default::default()
                };
            }
            'U' if style == Style::Helix => {
                return Outcome {
                    history: Some(true),
                    ..Default::default()
                };
            }
            '/' | '?' => {
                self.search_backwards = ch == '?';
                self.search = Some(String::new());
            }
            'n' | 'N' => self.search_next(text, selection, ch == 'N', style),
            'f' | 'F' | 't' | 'T' => self.pending = Some(Pending::Find(ch)),
            ';' => {
                if let Some((kind, ch)) = self.last_find {
                    self.find(text, selection, kind, ch, style);
                }
            }
            ',' => {
                if let Some((kind, ch)) = self.last_find {
                    let reverse = if kind.is_uppercase() {
                        kind.to_ascii_lowercase()
                    } else {
                        kind.to_ascii_uppercase()
                    };
                    self.find(text, selection, reverse, ch, style);
                }
            }
            'm' if style == Style::Helix => self.pending = Some(Pending::Match),
            '%' => {
                if let Some(ch) = text.chars().nth(cursor) {
                    if let Some(range) = edit::object(text, cursor, ch, true) {
                        let to = if cursor == range.start {
                            range.end - 1
                        } else {
                            range.start
                        };
                        self.navigation.reset();
                        self.move_to(text, selection, cursor, to, style, true);
                    }
                }
            }
            _ => {
                self.count = 0;
            }
        }
        Outcome::default()
    }

    fn operate(
        &mut self,
        text: &mut String,
        selection: &mut Selection,
        range: Range<usize>,
        op: char,
        linewise: bool,
    ) -> Outcome {
        self.navigation.reset();
        self.register = edit::slice(text, range.clone()).to_owned();
        self.linewise = linewise;
        let copied = (op == 'y').then(|| self.register.clone());
        if op != 'y' {
            *selection = edit::replace(text, range, "");
        } else {
            *selection = Selection::caret(range.start);
        }
        self.mode = if op == 'c' {
            Mode::Insert
        } else {
            Mode::Normal
        };
        self.visual_anchor = None;
        self.visual_head = None;
        selection.clamp(text);
        if self.mode == Mode::Normal && selection.range().is_empty() {
            *selection = Selection::caret(edit::normal_cursor(text, selection.head));
        }
        Outcome {
            copied,
            ..Default::default()
        }
    }

    fn find(&mut self, text: &str, selection: &mut Selection, kind: char, ch: char, style: Style) {
        let chars: Vec<_> = text.chars().collect();
        let cursor = self.cursor(text, *selection, style);
        let row = edit::line(text, cursor);
        self.navigation.reset();
        let count = self.take_count();
        let backward = kind.is_uppercase();
        let found = if backward {
            (row.start..cursor)
                .rev()
                .filter(|i| chars[*i] == ch)
                .nth(count - 1)
        } else {
            (cursor.saturating_add(1)..row.end)
                .filter(|i| chars[*i] == ch)
                .nth(count - 1)
        };
        if let Some(mut to) = found {
            if kind == 't' {
                to = to.saturating_sub(1);
            }
            if kind == 'T' {
                to += 1;
            }
            self.move_to(text, selection, cursor, to, style, true);
        }
    }

    pub fn submit_search(&mut self, text: &str, selection: &mut Selection, style: Style) {
        if let Some(query) = self.search.take().filter(|s| !s.is_empty()) {
            self.last_search.set(query, self.search_backwards);
        }
        self.search_next(text, selection, false, style);
    }

    fn search_next(&mut self, text: &str, selection: &mut Selection, opposite: bool, style: Style) {
        let count = self.take_count();
        let at = if style == Style::Vim {
            self.cursor(text, *selection, style)
        } else {
            selection.range().start
        };
        if let Some(range) = self.last_search.next(text, at, opposite, count) {
            self.navigation.reset();
            if style == Style::Vim {
                self.jump(text, selection, range.start, style);
            } else {
                if self.mode == Mode::Select {
                    self.visual_anchor = Some(range.start);
                    self.visual_head = Some(range.end - 1);
                }
                *selection = Selection {
                    anchor: range.start,
                    head: range.end,
                };
            }
        }
    }

    fn jump(&mut self, text: &str, selection: &mut Selection, at: usize, style: Style) {
        self.navigation.reset();
        let from = self.cursor(text, *selection, style);
        if style == Style::Helix && self.mode == Mode::Normal {
            *selection = Selection {
                anchor: at,
                head: (at + 1).min(text.chars().count()),
            };
        } else {
            self.move_to(text, selection, from, at, style, true);
        }
    }

    pub fn search_ranges(&self, text: &str) -> Vec<Range<usize>> {
        self.last_search.ranges(text)
    }
}

pub fn movement(ch: char) -> Option<Motion> {
    Some(match ch {
        'h' => Motion::Left,
        'j' => Motion::Down,
        'k' => Motion::Up,
        'l' => Motion::Right,
        'w' => Motion::Word,
        'b' => Motion::BackWord,
        'e' => Motion::EndWord,
        '0' => Motion::Start,
        '^' => Motion::First,
        '$' => Motion::End,
        _ => return None,
    })
}
fn current(text: &str, selection: Selection) -> Range<usize> {
    let range = selection.range();
    if range.is_empty() {
        range.start..(range.start + 1).min(text.chars().count())
    } else {
        range
    }
}
fn operator_range(text: &str, at: usize, ch: char, count: usize) -> Option<Range<usize>> {
    let to = edit::motion(text, at, movement(ch)?, count);
    Some(at.min(to)..(at.max(to) + usize::from(matches!(ch, 'e' | '$'))).min(text.chars().count()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(text: &str, keys: &str, style: Style) -> (String, Selection, Modal) {
        let mut text = text.to_owned();
        let mut selection = Selection::caret(0);
        let mut modal = Modal::default();
        for ch in keys.chars() {
            modal.character(&mut text, &mut selection, ch, style);
        }
        (text, selection, modal)
    }
    #[test]
    fn vim_operator_and_helix_selection_have_distinct_order() {
        assert_eq!(command("one two three", "dw", Style::Vim).0, "two three");
        let (text, selected, _) = command("one two three", "w", Style::Helix);
        assert_eq!(edit::slice(&text, selected.range()), "one ");
        assert_eq!(command("one two three", "wd", Style::Helix).0, "two three");
        assert_eq!(command("one two three", "2dw", Style::Vim).0, "three");
    }
    #[test]
    fn objects_line_operations_and_search() {
        assert_eq!(command("[4:1]", "ci[", Style::Vim).0, "[]");
        assert_eq!(command("[4:1]", "mi[d", Style::Helix).0, "[]");
        assert_eq!(command("one\r\ntwo", "dd", Style::Vim).0, "two");
        let (mut text, mut selection, mut modal) = command("1,2,3,E", "/3", Style::Vim);
        modal.submit_search(&text, &mut selection, Style::Vim);
        assert_eq!(selection, Selection::caret(4));
        modal.character(&mut text, &mut selection, 'n', Style::Vim);
        assert_eq!(selection, Selection::caret(4));
    }
    #[test]
    fn visual_motion_is_inclusive_without_skipping_and_open_line_is_single() {
        assert_eq!(command("abcd", "vld", Style::Vim).0, "cd");
        assert_eq!(command("one\ntwo\nthree", "Vjd", Style::Vim).0, "three");
        let (text, selection, mode) = command("one", "o", Style::Vim);
        assert_eq!(text, "one\n");
        assert_eq!(selection.head, 4);
        assert_eq!(mode.mode, Mode::Insert);
        assert_eq!(command("one\ntwo", "yyp", Style::Vim).0, "one\none\ntwo");
        let (text, selected, _) = command("one two three", "ww", Style::Helix);
        assert_eq!(edit::slice(&text, selected.range()), "two ");
        let (text, selected, _) = command("abcd", "ll", Style::Helix);
        assert_eq!(edit::slice(&text, selected.range()), "c");
        assert_eq!(command("one two", "cw", Style::Vim).0, " two");
        assert_eq!(command("one\r\ntwo", "cc", Style::Vim).0, "\r\ntwo");
        assert_eq!(command("one\ntwo\nthree", "dj", Style::Vim).0, "three");
    }

    #[test]
    fn word_objects_row_jumps_and_reverse_search() {
        assert_eq!(command("one two", "ciw", Style::Vim).0, " two");
        assert_eq!(command("one two", "daw", Style::Vim).0, "two");
        assert_eq!(command("one two", "miwd", Style::Helix).0, " two");
        for style in [Style::Vim, Style::Helix] {
            let (text, selection, modal) = command("one\ntwo\nthree", "2gg", style);
            assert_eq!(modal.cursor(&text, selection, style), 4);
            let (text, selection, modal) = command("one\ntwo\nthree", "3G", style);
            assert_eq!(modal.cursor(&text, selection, style), 8);
            let (mut text, mut selection, mut modal) = command("abc abc abc", "?abc", style);
            modal.submit_search(&text, &mut selection, style);
            assert_eq!(selection.range().start, 8);
            modal.character(&mut text, &mut selection, 'n', style);
            assert_eq!(selection.range().start, 4);
            modal.character(&mut text, &mut selection, 'N', style);
            assert_eq!(selection.range().start, 8);
        }
    }

    #[test]
    fn helix_cursor_tracks_forward_reverse_and_visual_endpoints() {
        let style = Style::Helix;
        let (mut text, mut selection, mut modal) = command("abc def", "w", style);
        assert_eq!(modal.cursor(&text, selection, style), 3);
        modal.character(&mut text, &mut selection, 'b', style);
        assert!(selection.head < selection.anchor);
        assert_eq!(modal.cursor(&text, selection, style), 0);
        modal.character(&mut text, &mut selection, 'v', style);
        assert_eq!(
            modal.cursor(&text, selection, style),
            0,
            "Select retains the reverse selection's active end"
        );
        modal.pointer_selection(Selection { anchor: 6, head: 2 });
        selection = Selection { anchor: 6, head: 2 };
        assert_eq!(modal.cursor(&text, selection, style), 2);
        modal.escape(&mut selection, style, &text);
        assert_eq!(modal.cursor(&text, selection, style), 2);
        assert_eq!(selection, Selection::caret(2));
        for style in [Style::Vim, Style::Helix] {
            let (text, selection, modal) = command("abcdef\nx\nabcdef", "5ljj", style);
            assert_eq!(modal.cursor(&text, selection, style), 14);
            let (text, selection, modal) = command("ab\ncd", "$", style);
            assert_eq!(modal.cursor(&text, selection, style), 1);
        }
    }

    #[test]
    fn commands_at_line_end_target_the_visible_block_character() {
        for style in [Style::Vim, Style::Helix] {
            let mut text = String::from("ab\ncd");
            let mut modal = Modal::default();
            let mut selection = Selection::caret(2);
            assert_eq!(modal.cursor(&text, selection, style), 1);
            modal.character(
                &mut text,
                &mut selection,
                if style == Style::Vim { 'x' } else { 'd' },
                style,
            );
            assert_eq!(text, "a\ncd");
            assert_eq!(modal.cursor(&text, selection, style), 0);
        }
    }
}
