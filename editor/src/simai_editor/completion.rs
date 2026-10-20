//! Local Simai snippets. All replacement and field ranges are character based.

use super::editing::{self as edit, Selection};
use crate::app::types::{SlideShape, validate_slide_shape};
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub label: String,
    pub detail: &'static str,
    pub value: String,
    pub fields: Vec<Range<usize>>,
}

#[derive(Debug, Clone, Default)]
pub struct Completion {
    pub candidates: Vec<Candidate>,
    pub selected: usize,
    pub range: Range<usize>,
    pub fields: Vec<Range<usize>>,
    pub field: usize,
    pub final_cursor: usize,
    context: Option<(String, usize)>,
}

impl Completion {
    pub fn close(&mut self) {
        self.candidates.clear();
        self.context = None;
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn open(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub fn update(&mut self, text: &str, at: usize, manual: bool) {
        if !manual
            && self
                .context
                .as_ref()
                .is_some_and(|(old, index)| old == text && *index == at)
        {
            return;
        }
        let (range, candidates) = candidates(text, at, manual);
        self.range = range;
        self.candidates = candidates;
        self.selected = 0;
        self.context = Some((text.to_owned(), at));
    }
    pub fn accept(&mut self, text: &mut String, selection: &mut Selection) {
        let Some(candidate) = self.candidates.get(self.selected).cloned() else {
            return;
        };
        let start = self.range.start;
        *selection = edit::replace(text, self.range.clone(), &candidate.value);
        self.final_cursor = selection.head;
        self.fields = candidate
            .fields
            .into_iter()
            .map(|r| start + r.start..start + r.end)
            .collect();
        self.field = 0;
        if let Some(range) = self.fields.first() {
            *selection = Selection {
                anchor: range.start,
                head: range.end,
            };
        }
        self.close();
    }
    pub fn jump_field(&mut self, selection: &mut Selection, backward: bool) -> bool {
        if self.fields.is_empty() {
            return false;
        }
        if backward {
            self.field = self.field.saturating_sub(1);
        } else {
            self.field += 1;
        }
        if let Some(range) = self.fields.get(self.field) {
            *selection = Selection {
                anchor: range.start,
                head: range.end,
            };
        } else {
            *selection = Selection::caret(self.final_cursor);
            self.fields.clear();
        }
        true
    }
    pub fn text_changed(&mut self, before: &str, after: &str) {
        if before == after || self.fields.is_empty() {
            return;
        }
        let a: Vec<_> = before.chars().collect();
        let b: Vec<_> = after.chars().collect();
        let start = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
        let suffix = a[start..]
            .iter()
            .rev()
            .zip(b[start..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let end = a.len() - suffix;
        let Some(active) = self.fields.get(self.field) else {
            self.fields.clear();
            return;
        };
        if start < active.start || end > active.end {
            self.fields.clear();
            return;
        }
        let delta = b.len() as isize - a.len() as isize;
        for (index, range) in self.fields.iter_mut().enumerate().skip(self.field) {
            if index != self.field {
                range.start = range.start.saturating_add_signed(delta);
            }
            range.end = range.end.saturating_add_signed(delta);
        }
        self.final_cursor = self.final_cursor.saturating_add_signed(delta);
    }
}

fn snippet(
    label: impl Into<String>,
    detail: &'static str,
    value: impl Into<String>,
    fields: &[Range<usize>],
) -> Candidate {
    Candidate {
        label: label.into(),
        detail,
        value: value.into(),
        fields: fields.to_vec(),
    }
}

pub fn in_comment(text: &str, at: usize) -> bool {
    let row = edit::line(text, at);
    edit::slice(text, row.start..at).contains("||")
}

fn candidates(text: &str, at: usize, manual: bool) -> (Range<usize>, Vec<Candidate>) {
    let at = at.min(text.chars().count());
    let empty = || (at..at, Vec::new());
    if in_comment(text, at) {
        return empty();
    }
    let prefix = edit::slice(text, 0..at);
    let boundary = prefix
        .chars()
        .enumerate()
        .filter(|(_, ch)| matches!(ch, ',' | '/' | '\n' | '\r' | '`'))
        .last()
        .map_or(0, |(i, _)| i + 1);
    let token = edit::slice(text, boundary..at);
    let chars: Vec<_> = token.chars().collect();
    for (open, close, kind) in [('[', ']', 0), ('{', '}', 1), ('(', ')', 2)] {
        if let Some(start) = chars.iter().rposition(|c| *c == open) {
            if chars[start..].contains(&close) {
                continue;
            }
            let start = boundary + start + 1;
            let mut end = at;
            while let Some(ch) = text.chars().nth(end) {
                if ch == close {
                    end += 1;
                    break;
                }
                if matches!(ch, ',' | '/' | '\n' | '\r') {
                    break;
                }
                end += 1;
            }
            let typed = edit::slice(text, start..at);
            let slide = chars.iter().take(start - boundary - 1).any(|c| {
                matches!(
                    c,
                    '-' | '^' | '<' | '>' | 'v' | 'V' | 'p' | 'q' | 's' | 'z' | 'w'
                )
            });
            let values = match kind {
                0 => ["4:1", "8:1", "16:1", "2:1", "1:1", "1:2"]
                    .into_iter()
                    .map(|v| {
                        let split = v.find(':').unwrap();
                        snippet(
                            format!("[{v}]"),
                            "时值",
                            format!("{v}]"),
                            &[0..split, split + 1..v.len()],
                        )
                    })
                    .chain(slide.then(|| {
                        snippet("[0##1]", "Slide 等待 / 移动秒数", "0##1]", &[0..1, 3..4])
                    }))
                    .collect(),
                1 => ["4", "8", "16", "32", "12", "24", "1"]
                    .into_iter()
                    .map(|v| snippet(format!("{{{v}}}"), "节拍细分", format!("{v}}}"), &[]))
                    .collect::<Vec<_>>(),
                _ => ["120", "150", "180", "188", "200"]
                    .into_iter()
                    .map(|v| snippet(format!("({v})"), "BPM", format!("{v})"), &[0..v.len()]))
                    .collect(),
            };
            return (
                start..end,
                values
                    .into_iter()
                    .filter(|v| manual || v.value.starts_with(typed))
                    .collect(),
            );
        }
    }
    if let Some(start) = token.rfind("<H") {
        if !token[start..].contains('>') {
            let start = boundary + token[..start].chars().count() + 2;
            let mut end = at;
            while let Some(ch) = text.chars().nth(end) {
                if ch == '>' {
                    end += 1;
                    break;
                }
                if !(ch.is_ascii_digit() || ch == '.') {
                    break;
                }
                end += 1;
            }
            return (
                start..end,
                ["1", "0.5", "1.5", "2"]
                    .into_iter()
                    .map(|v| {
                        snippet(
                            format!("<H{v}>"),
                            "Hi-Speed",
                            format!("{v}>"),
                            &[0..v.len()],
                        )
                    })
                    .collect(),
            );
        }
    }
    let code_start = super::source::tokens(token)
        .into_iter()
        .find(|t| {
            !matches!(
                t.kind,
                super::source::Kind::Space
                    | super::source::Kind::Bpm
                    | super::source::Kind::Division
                    | super::source::Kind::Speed
            )
        })
        .map(|t| token[..t.bytes.start].chars().count())
        .unwrap_or(chars.len());
    let code = &chars[code_start..];
    if matches!(code, [zone, ..] if matches!(zone, 'A' | 'B' | 'C' | 'D' | 'E')) {
        if matches!(code, ['A' | 'B' | 'D' | 'E']) {
            return (
                at..at,
                (1..=8)
                    .map(|lane| snippet(lane.to_string(), "Touch 区域", lane.to_string(), &[]))
                    .collect(),
            );
        }
        let has_lane = matches!(code, ['C', ..] | ['A' | 'B' | 'D' | 'E', '1'..='8', ..]);
        let suffix = if code.first() == Some(&'C') {
            &code[1..]
        } else {
            &code[2..]
        };
        if has_lane
            && suffix.iter().all(|ch| matches!(ch, 'b' | 'x' | 'f' | 'h'))
            && !suffix.contains(&'h')
        {
            return (
                at..at,
                vec![
                    snippet(
                        "h[4:1]",
                        "Touch Hold",
                        if code.last() == Some(&'h') {
                            "[4:1]"
                        } else {
                            "h[4:1]"
                        },
                        if code.last() == Some(&'h') {
                            &[1..2, 3..4]
                        } else {
                            &[2..3, 4..5]
                        },
                    ),
                    snippet("f", "烟花", "f", &[]),
                ],
            );
        }
        return empty();
    }
    let head = code
        .first()
        .filter(|c| ('1'..='8').contains(c))
        .map(|_| code_start);
    if let Some(head) = head {
        let lane = chars[head].to_digit(10).unwrap() as u8;
        let tail: String = chars[head + 1..].iter().collect();
        if let Some(shape) = shape(&tail) {
            let candidates = (1..=8)
                .filter(|end| {
                    validate_slide_shape(shape, lane, *end).is_ok()
                        && crate::app::slide_svg::prefab_key(shape, lane, *end, 0).is_some()
                })
                .map(|end| {
                    snippet(
                        end.to_string(),
                        "Slide 终点",
                        format!("{end}[4:1]"),
                        &[2..3, 4..5],
                    )
                })
                .collect();
            return (at..at, candidates);
        }
        if tail == "h" {
            return (
                at..at,
                vec![snippet("h[4:1]", "Hold 时值", "[4:1]", &[1..2, 3..4])],
            );
        }
        if tail
            .chars()
            .all(|ch| matches!(ch, 'b' | 'x' | '$' | '!' | '?' | '@'))
        {
            return (
                at..at,
                vec![
                    snippet("h[4:1]", "Hold", "h[4:1]", &[2..3, 4..5]),
                    snippet(
                        format!("-{}[4:1]", (lane + 3) % 8 + 1),
                        "Slide",
                        format!("-{}[4:1]", (lane + 3) % 8 + 1),
                        &[1..2, 3..4, 5..6],
                    ),
                    snippet("b", "Break", "b", &[]),
                    snippet("x", "EX", "x", &[]),
                    snippet("$", "星星", "$", &[]),
                    snippet("!", "无头 Slide", "!", &[]),
                ],
            );
        }
        return empty();
    }
    if matches!(chars.last(), Some('A' | 'B' | 'D' | 'E')) {
        return (
            at..at,
            (1..=8)
                .map(|lane| snippet(lane.to_string(), "Touch 区域", lane.to_string(), &[]))
                .collect(),
        );
    }
    if manual {
        return (
            at..at,
            vec![
                snippet("1", "Tap", "1", &[0..1]),
                snippet("C", "Touch", "C", &[]),
                snippet("B1", "Touch", "B1", &[1..2]),
                snippet("Chf[4:1]", "Touch Hold / 烟花", "Chf[4:1]", &[4..5, 6..7]),
                snippet("Cf", "Touch / 烟花", "Cf", &[]),
                snippet("{4}", "节拍细分", "{4}", &[1..2]),
                snippet("(120)", "BPM", "(120)", &[1..4]),
                snippet("<H1>", "Hi-Speed", "<H1>", &[2..3]),
            ],
        );
    }
    empty()
}

fn shape(text: &str) -> Option<SlideShape> {
    Some(match text {
        "-" => SlideShape::Line,
        "^" => SlideShape::Caret,
        "<" => SlideShape::Left,
        ">" => SlideShape::Right,
        "v" => SlideShape::VShape,
        "p" => SlideShape::P,
        "q" => SlideShape::Q,
        "pp" => SlideShape::PP,
        "qq" => SlideShape::QQ,
        "s" => SlideShape::S,
        "z" => SlideShape::Z,
        "w" => SlideShape::Wifi,
        _ => return None,
    })
}

pub fn paired_input(text: &mut String, selection: &mut Selection, ch: char) -> bool {
    if in_comment(text, selection.head) {
        return false;
    }
    if matches!(ch, ')' | ']' | '}')
        && selection.range().is_empty()
        && text.chars().nth(selection.head) == Some(ch)
    {
        *selection = Selection::caret(selection.head + 1);
        return true;
    }
    let close = match ch {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return false,
    };
    let range = selection.range();
    let contents = edit::slice(text, range.clone()).to_owned();
    edit::replace(text, range.clone(), &format!("{ch}{contents}{close}"));
    *selection = Selection {
        anchor: range.start + 1,
        head: range.start + 1 + contents.chars().count(),
    };
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durations_replace_paired_closer_and_fields_track_typing() {
        let mut completion = Completion::default();
        let mut text = String::from("1h[]");
        let mut selection = Selection::caret(3);
        completion.update(&text, 3, false);
        completion.accept(&mut text, &mut selection);
        assert_eq!(text, "1h[4:1]");
        assert_eq!(edit::slice(&text, selection.range()), "4");
        let before = text.clone();
        selection = edit::replace(&mut text, selection.range(), "16");
        completion.text_changed(&before, &text);
        completion.jump_field(&mut selection, false);
        assert_eq!(edit::slice(&text, selection.range()), "1");
    }
    #[test]
    fn wifi_endpoints_and_comments_are_contextual() {
        let (_, values) = candidates("1w", 2, false);
        assert_eq!(
            values.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(),
            ["5"]
        );
        assert!(candidates("|| 注释 1h[", 10, true).1.is_empty());
        assert!(!candidates("1h[", 3, false).1.is_empty());
        assert_eq!(candidates("(188){4}1w", 10, false).1[0].label, "5");
        assert!(
            candidates("(188){4}B1", 10, false)
                .1
                .iter()
                .all(|v| !v.value.starts_with('-'))
        );
    }
    #[test]
    fn pairing_and_skip_are_unicode_safe() {
        let mut text = String::from("注释\n1h");
        let mut selection = Selection::caret(5);
        assert!(paired_input(&mut text, &mut selection, '['));
        assert_eq!(text, "注释\n1h[]");
        assert!(paired_input(&mut text, &mut selection, ']'));
        assert_eq!(text, "注释\n1h[]");
    }
    #[test]
    fn unchanged_events_keep_fields_and_hispeed_replaces_existing_suffix() {
        let mut completion = Completion::default();
        let mut text = String::from("<H0.5>1,E");
        let mut selection = Selection::caret(2);
        completion.update(&text, 2, true);
        completion.accept(&mut text, &mut selection);
        assert_eq!(text, "<H1>1,E");
        completion.text_changed(&text.clone(), &text);
        assert_eq!(completion.fields, [2..3]);
        assert!(candidates("1-5[4:1]", 8, true).1.is_empty());
        assert!(
            candidates("1h[", 3, true)
                .1
                .iter()
                .all(|c| !c.value.contains("##"))
        );
    }
}
