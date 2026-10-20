//! Character-indexed text operations shared by modal commands and completion.

use std::ops::Range;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn caret(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
    pub fn clamp(&mut self, text: &str) {
        let len = text.chars().count();
        self.anchor = self.anchor.min(len);
        self.head = self.head.min(len);
    }
}

pub fn byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(i, _)| i)
}
pub fn slice(text: &str, range: Range<usize>) -> &str {
    &text[byte(text, range.start)..byte(text, range.end)]
}
pub fn replace(text: &mut String, range: Range<usize>, value: &str) -> Selection {
    let at = range.start + value.chars().count();
    text.replace_range(byte(text, range.start)..byte(text, range.end), value);
    Selection::caret(at)
}

pub fn line(text: &str, at: usize) -> Range<usize> {
    let chars: Vec<_> = text.chars().collect();
    let at = at.min(chars.len());
    let start = chars[..at]
        .iter()
        .rposition(|c| *c == '\n')
        .map_or(0, |i| i + 1);
    let end = chars[at..]
        .iter()
        .position(|c| *c == '\n')
        .map_or(chars.len(), |i| at + i);
    start..end.saturating_sub(usize::from(end > start && chars[end - 1] == '\r'))
}

pub fn lines(text: &str, at: usize, count: usize) -> Range<usize> {
    let start = line(text, at).start;
    let mut end = start;
    for _ in 0..count.max(1).min(100_000) {
        let row = line(text, end);
        end = row.end;
        if text.chars().nth(end) == Some('\r') {
            end += 1;
        }
        if text.chars().nth(end) == Some('\n') {
            end += 1;
        } else {
            break;
        }
    }
    start..end
}

pub fn line_number(text: &str, number: usize) -> usize {
    text.chars()
        .enumerate()
        .filter(|(_, c)| *c == '\n')
        .nth(number.saturating_sub(2))
        .filter(|_| number > 1)
        .map_or_else(
            || {
                if number <= 1 {
                    0
                } else {
                    line(text, text.chars().count()).start
                }
            },
            |(i, _)| i + 1,
        )
}

#[derive(Debug, Clone, Copy)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    Word,
    BackWord,
    EndWord,
    Start,
    First,
    End,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Default)]
pub struct Navigation {
    column: Option<usize>,
}

impl Navigation {
    pub fn reset(&mut self) {
        self.column = None;
    }

    pub fn target(&mut self, text: &str, at: usize, command: Motion, count: usize) -> usize {
        if !matches!(command, Motion::Up | Motion::Down) {
            self.reset();
            return motion(text, at, command, count);
        }
        let at = at.min(text.chars().count());
        let row = line(text, at);
        let column = *self.column.get_or_insert(at.saturating_sub(row.start));
        let mut start = row.start;
        for _ in 0..count.max(1).min(100_000) {
            if matches!(command, Motion::Up) {
                if start == 0 {
                    break;
                }
                start = line(text, start - 1).start;
            } else {
                let end = lines(text, start, 1).end;
                if end == start || !slice(text, start..end).ends_with('\n') {
                    break;
                }
                start = end;
            }
        }
        (start + column).min(line(text, start).end)
    }
}

pub fn normal_cursor(text: &str, at: usize) -> usize {
    let row = line(text, at);
    at.min(row.end.saturating_sub(usize::from(!row.is_empty())))
        .max(row.start)
}

pub fn motion(text: &str, at: usize, command: Motion, count: usize) -> usize {
    let chars: Vec<_> = text.chars().collect();
    let mut at = at.min(chars.len());
    let row = line(text, at);
    match command {
        Motion::Start => return row.start,
        Motion::First => {
            return row.start
                + chars[row.clone()]
                    .iter()
                    .take_while(|c| c.is_whitespace())
                    .count();
        }
        Motion::End => return row.end.saturating_sub(usize::from(row.end > row.start)),
        Motion::Top => return 0,
        Motion::Bottom => return line(text, chars.len()).start,
        _ => {}
    }
    let column = at - row.start;
    for _ in 0..count.max(1).min(100_000) {
        match command {
            Motion::Left => at = at.saturating_sub(1).max(line(text, at).start),
            Motion::Right => {
                at = (at + 1).min(
                    line(text, at)
                        .end
                        .saturating_sub(usize::from(!line(text, at).is_empty())),
                )
            }
            Motion::Up => {
                let row = line(text, at);
                if row.start == 0 {
                    break;
                }
                let prev = line(text, row.start - 1);
                at = (prev.start + column).min(prev.end);
            }
            Motion::Down => {
                let row = lines(text, at, 1);
                if row.end >= chars.len() {
                    break;
                }
                let next = line(text, row.end);
                at = (next.start + column).min(next.end);
            }
            Motion::Word => {
                let kind = chars.get(at).copied().map(class);
                while at < chars.len() && Some(class(chars[at])) == kind {
                    at += 1;
                }
                while at < chars.len() && chars[at].is_whitespace() {
                    at += 1;
                }
            }
            Motion::BackWord => {
                at = at.saturating_sub(1);
                while at > 0 && chars[at].is_whitespace() {
                    at -= 1;
                }
                let kind = chars.get(at).copied().map(class);
                while at > 0 && Some(class(chars[at - 1])) == kind {
                    at -= 1;
                }
            }
            Motion::EndWord => {
                if at + 1 < chars.len() {
                    at += 1;
                }
                while at < chars.len() && chars[at].is_whitespace() {
                    at += 1;
                }
                let kind = chars.get(at).copied().map(class);
                while at + 1 < chars.len() && Some(class(chars[at + 1])) == kind {
                    at += 1;
                }
            }
            _ => {}
        }
    }
    at.min(chars.len())
}

fn class(ch: char) -> u8 {
    if ch.is_whitespace() {
        0
    } else if ch.is_alphanumeric() || ch == '_' {
        1
    } else {
        2
    }
}

pub fn pair(ch: char) -> Option<(char, char)> {
    Some(match ch {
        '(' | ')' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        '{' | '}' => ('{', '}'),
        '<' | '>' => ('<', '>'),
        _ => return None,
    })
}

pub fn object(text: &str, at: usize, delimiter: char, around: bool) -> Option<Range<usize>> {
    if matches!(delimiter, 'w' | 'W') {
        let chars: Vec<_> = text.chars().collect();
        let at = at.min(chars.len().saturating_sub(1));
        let category = |ch: char| {
            if delimiter == 'W' {
                u8::from(!ch.is_whitespace())
            } else {
                class(ch)
            }
        };
        let kind = category(*chars.get(at)?);
        let mut start = at;
        let mut end = at + 1;
        while start > 0
            && !matches!(chars[start - 1], '\r' | '\n')
            && category(chars[start - 1]) == kind
        {
            start -= 1;
        }
        while end < chars.len()
            && !matches!(chars[end], '\r' | '\n')
            && category(chars[end]) == kind
        {
            end += 1;
        }
        if around {
            let tail = end;
            while end < chars.len() && matches!(chars[end], ' ' | '\t') {
                end += 1;
            }
            if end == tail {
                while start > 0 && matches!(chars[start - 1], ' ' | '\t') {
                    start -= 1;
                }
            }
        }
        return Some(start..end);
    }
    let (open, close) = pair(delimiter)?;
    let chars: Vec<_> = text.chars().collect();
    let mut stack = Vec::new();
    let mut found = None;
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch == open {
            stack.push(index);
        }
        if ch == close {
            if let Some(start) = stack.pop() {
                if start <= at
                    && at <= index
                    && found
                        .as_ref()
                        .is_none_or(|r: &Range<usize>| index - start < r.len())
                {
                    found = Some(start..index + 1);
                }
            }
        }
    }
    found.map(|r| if around { r } else { r.start + 1..r.end - 1 })
}

#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    text: String,
    selection: Selection,
}

#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    typing_at: Option<f64>,
}

pub trait EditHistory {
    fn boundary(&mut self);
    fn record(&mut self, before: &str, selection: Selection, after: &str, typing: bool, now: f64);
    fn restore(&mut self, text: &mut String, selection: &mut Selection, redo: bool);
}

impl EditHistory for History {
    fn boundary(&mut self) { History::boundary(self); }
    fn record(&mut self, before: &str, selection: Selection, after: &str, typing: bool, now: f64) {
        History::record(self, before, selection, after, typing, now);
    }
    fn restore(&mut self, text: &mut String, selection: &mut Selection, redo: bool) {
        History::restore(self, text, selection, redo);
    }
}

impl History {
    pub fn record(
        &mut self,
        before: &str,
        selection: Selection,
        after: &str,
        typing: bool,
        now: f64,
    ) {
        if before == after {
            return;
        }
        let join = typing && self.typing_at.is_some_and(|at| now - at < 0.7);
        if !join {
            self.undo.push(Snapshot {
                text: before.to_owned(),
                selection,
            });
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing_at = typing.then_some(now);
    }
    pub fn boundary(&mut self) {
        self.typing_at = None;
    }
    pub fn restore(&mut self, text: &mut String, selection: &mut Selection, redo: bool) {
        self.boundary();
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        if let Some(previous) = from.pop() {
            to.push(Snapshot {
                text: text.clone(),
                selection: *selection,
            });
            *text = previous.text;
            *selection = previous.selection;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_ranges_crlf_and_nested_objects() {
        let mut text = String::from("注释\r\n1h[4:1],E");
        assert_eq!(line(&text, 1), 0..2);
        assert_eq!(slice(&text, lines(&text, 0, 1)), "注释\r\n");
        assert_eq!(object(&text, 8, '[', false), Some(7..10));
        replace(&mut text, 0..2, "备注");
        assert!(text.starts_with("备注\r\n"));
        assert_eq!(object("[[x]]", 2, '[', false), Some(2..3));
    }
    #[test]
    fn word_objects_and_line_numbers_are_character_safe() {
        assert_eq!(object("one two", 1, 'w', false), Some(0..3));
        assert_eq!(object("one two", 1, 'w', true), Some(0..4));
        assert_eq!(object("one two", 6, 'w', true), Some(3..7));
        assert_eq!(object("1h[4:1], E", 4, 'W', false), Some(0..8));
        assert_eq!(line_number("注释\r\n1,\r\nE", 2), 4);
        assert_eq!(line_number("注释\r\n1,\r\nE", 99), 8);
        assert_eq!(line_number("abc", 1), 0);
    }
    #[test]
    fn history_groups_typing_and_separates_commands() {
        let mut history = History::default();
        history.record("", Selection::caret(0), "1", true, 0.0);
        history.record("1", Selection::caret(1), "12", true, 0.1);
        history.record("12", Selection::caret(2), "123", false, 0.2);
        let mut text = String::from("123");
        let mut selection = Selection::caret(3);
        history.restore(&mut text, &mut selection, false);
        assert_eq!(text, "12");
        history.restore(&mut text, &mut selection, false);
        assert_eq!(text, "");
        history.restore(&mut text, &mut selection, true);
        assert_eq!(text, "12");
    }
}
