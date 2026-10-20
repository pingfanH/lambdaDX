//! Lexical spans for coloring; timing remains owned by the core parser.

use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::app::types::{ChartDoc, Note, NoteType, note_secs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind { Bpm, Division, Duration, Speed, Comment, Note, Modifier, Slide, Separator, Space }

pub struct Token { pub bytes: Range<usize>, pub kind: Kind }

pub fn tokens(text: &str) -> Vec<Token> {
    let mut result = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        let (kind, close) = match ch {
            '(' => (Kind::Bpm, Some(')')),
            '{' => (Kind::Division, Some('}')),
            '[' => (Kind::Duration, Some(']')),
            '<' if text[start..].starts_with("<H") => (Kind::Speed, Some('>')),
            '|' if chars.peek().is_some_and(|(_, c)| *c == '|') => (Kind::Comment, None),
            '0'..='9' | 'A'..='E' => (Kind::Note, None),
            'h' | 'b' | 'x' | 'f' | '$' | '!' | '?' | '@' => (Kind::Modifier, None),
            '-' | '^' | '<' | '>' | 'v' | 'V' | 'p' | 'q' | 's' | 'z' | 'w' => (Kind::Slide, None),
            c if c.is_whitespace() => (Kind::Space, None),
            _ => (Kind::Separator, None),
        };
        if let Some(close) = close {
            while let Some(&(_, c)) = chars.peek() {
                if matches!(c, '\n' | '\r' | ',') { break; }
                chars.next();
                if c == close { break; }
            }
        } else if kind == Kind::Comment {
            while chars.peek().is_some_and(|(_, c)| *c != '\n') { chars.next(); }
        }
        let end = chars.peek().map_or(text.len(), |(i, _)| *i);
        result.push(Token { bytes: start..end, kind });
    }
    result
}

#[derive(Debug, Clone)]
pub struct NoteSpan {
    pub chars: Vec<Range<usize>>,
    pub(super) text: String,
    pub ids: Vec<u64>,
}

impl NoteSpan {
    fn matches(&self, note: &Note) -> bool {
        let text = self.text.as_bytes();
        let (lane, touch) = match text {
            [b'C', ..] => (17, true),
            [zone @ (b'A' | b'B' | b'D' | b'E'), lane @ b'1'..=b'8', ..] => {
                let base = match zone { b'A' => 0, b'B' => 8, b'D' => 17, _ => 25 };
                (base + lane - b'0', true)
            }
            [lane @ b'1'..=b'8', ..] => (lane - b'0', false),
            _ => return false,
        };
        if lane != note.lane || touch != crate::app::types::note_is_touch(note) { return false; }
        let slide = self.text.contains(['-', '^', '<', '>', 'v', 'V', 'p', 'q', 's', 'z', 'w']);
        let hold = self.text.contains('h');
        match note.note_type {
            NoteType::Slide => slide,
            NoteType::Hold | NoteType::TouchHold => hold,
            _ => !slide && !hold,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimedCell {
    pub time: f32,
    pub chars: Range<usize>,
    pub code: Vec<Range<usize>>,
    pub notes: Vec<NoteSpan>,
}

fn push_span(spans: &mut Vec<NoteSpan>, chars: &mut Vec<Range<usize>>, text: &mut String) {
    if !text.is_empty() && text != "E" {
        // Compact simultaneous taps such as 12 are separate notes too.
        if text.len() > 1 && text.bytes().all(|c| (b'1'..=b'8').contains(&c)) && chars.len() == 1 {
            for (i, c) in text.chars().enumerate() {
                spans.push(NoteSpan { chars: vec![chars[0].start + i..chars[0].start + i + 1], text: c.to_string(), ids: Vec::new() });
            }
        } else {
            spans.push(NoteSpan { chars: std::mem::take(chars), text: std::mem::take(text), ids: Vec::new() });
        }
    }
    chars.clear();
    text.clear();
}

fn append_range(ranges: &mut Vec<Range<usize>>, range: Range<usize>) {
    if let Some(last) = ranges.last_mut().filter(|last| last.end == range.start) { last.end = range.end; }
    else { ranges.push(range); }
}

pub fn timeline(text: &str, bpm: f32) -> Result<Vec<TimedCell>, String> {
    let mut projection = format!("&wholebpm={bpm}\n&inote_6=");
    let mut ranges = Vec::new();
    let mut offset = 0;
    let mut first = None;
    let mut note = None;
    let mut end = 0;
    let mut cells = Vec::new();
    let mut code = Vec::new();
    let mut notes = Vec::new();
    let mut note_chars = Vec::new();
    let mut note_text = String::new();
    for token in tokens(text) {
        let value = &text[token.bytes];
        let next = offset + value.chars().count();
        if !matches!(token.kind, Kind::Space | Kind::Comment) { append_range(&mut code, offset..next); }
        if value == "," {
            ranges.push(note.or(first).unwrap_or(offset)..end.max(next));
            push_span(&mut notes, &mut note_chars, &mut note_text);
            cells.push((std::mem::take(&mut code), std::mem::take(&mut notes)));
            projection.push_str("1,");
            first = None;
            note = None;
        } else if !matches!(token.kind, Kind::Comment | Kind::Space) {
            first.get_or_insert(offset);
            end = next;
            if matches!(token.kind, Kind::Bpm | Kind::Division | Kind::Speed) {
                projection.push_str(value);
            } else if value == "/" || value == "`" {
                push_span(&mut notes, &mut note_chars, &mut note_text);
            } else {
                note.get_or_insert(offset);
                append_range(&mut note_chars, offset..next);
                note_text.push_str(value);
            }
        }
        offset = next;
    }
    if let Some(start) = note.or(first) {
        ranges.push(start..end);
        push_span(&mut notes, &mut note_chars, &mut note_text);
        cells.push((code, notes));
        projection.push_str("1,");
    }
    if ranges.is_empty() { return Ok(Vec::new()); }
    // One marker per comma cell retains rests and clock directives without
    // relying on raw note text, which the core rewrites for connected slides.
    let times = crate::app::maidata::simai_timeline(&projection, 6)?;
    if times.len() != ranges.len() { return Err("Unable to map Simai playback positions".into()); }
    Ok(times.into_iter().zip(ranges).zip(cells).map(|((fragment, chars), (code, notes))|
        TimedCell { time: fragment.time, chars, code, notes }).collect())
}

fn bind_notes(cells: &mut [TimedCell], chart: &ChartDoc) {
    for note in &chart.notes {
        let time = note_secs(note, &chart.bpms);
        let start = cells.partition_point(|cell| cell.time < time - 0.0002);
        // Very fine subdivisions can put rests and several heads inside the
        // timing tolerance. Pick the nearest matching span, not the first cell.
        let candidate = cells.iter().enumerate().skip(start)
            .take_while(|(_, cell)| cell.time <= time + 0.0002)
            .flat_map(|(cell_index, cell)| cell.notes.iter().enumerate()
                .filter(|(_, span)| span.matches(note))
                .map(move |(span_index, span)| (cell_index, span_index, !span.ids.is_empty(), (cell.time - time).abs())))
            // Shared-head stars may reuse a span once unused matches are bound.
            .min_by(|a, b| a.2.cmp(&b.2).then_with(|| a.3.total_cmp(&b.3)));
        if let Some((cell, span, _, _)) = candidate { cells[cell].notes[span].ids.push(note.id); }
    }
}

pub(super) fn mapped_timeline(text: &str, chart: &ChartDoc) -> Result<Vec<TimedCell>, String> {
    let mut cells = timeline(text, chart.bpm)?;
    for cell in &mut cells { cell.time += chart.audio_offset; }
    bind_notes(&mut cells, chart);
    Ok(cells)
}

#[derive(Debug, Clone)]
struct Pending {
    revision: u64,
    result: Arc<Mutex<Option<Result<Vec<TimedCell>, String>>>>,
}

#[derive(Debug, Clone, Default)]
pub struct PlaybackMap {
    revision: u64,
    rebuild_at: Option<Instant>,
    pending: Option<Pending>,
    pub cells: Vec<TimedCell>,
    pub error: Option<String>,
}

impl PlaybackMap {
    pub fn invalidate(&mut self) {
        self.revision += 1;
        self.cells.clear();
        self.error = None;
        self.rebuild_at = Some(Instant::now() + Duration::from_millis(150));
    }

    pub fn update(&mut self, text: &str, chart: &ChartDoc) {
        let ready = self.pending.as_ref().and_then(|job| {
            job.result.try_lock().ok()?.take().map(|result| (job.revision, result))
        });
        if let Some((revision, result)) = ready {
            self.pending = None;
            if revision == self.revision {
                match result {
                    Ok(cells) => self.cells = cells,
                    Err(error) => self.error = Some(error),
                }
            }
        }
        if self.pending.is_none() && self.rebuild_at.is_some_and(|at| Instant::now() >= at) {
            self.rebuild_at = None;
            let result = Arc::new(Mutex::new(None));
            let output = result.clone();
            let text = text.to_owned();
            let chart = chart.clone();
            match std::thread::Builder::new().name("simai-source-map".into()).spawn(move || {
                let cells = timeline(&text, chart.bpm).map(|mut cells| { bind_notes(&mut cells, &chart); cells });
                if let Ok(mut slot) = output.lock() { *slot = Some(cells); }
            }) {
                Ok(_) => self.pending = Some(Pending { revision: self.revision, result }),
                Err(error) => self.error = Some(format!("Unable to map Simai playback: {error}")),
            }
        }
    }

    pub fn active(&self, time: f32) -> Option<&TimedCell> {
        let index = self.cells.partition_point(|cell| cell.time <= time).checked_sub(1)?;
        self.cells.get(index)
    }

    pub fn at_char(&self, index: usize) -> Option<&TimedCell> {
        self.cells.iter().find(|cell| cell.code.iter().any(|range| range.contains(&index)))
    }

    pub fn at_caret(&self, index: usize) -> Option<&TimedCell> {
        // A caret can sit between tokens, in comments, or past the final glyph.
        // Gaps belong to the next comma cell; EOF belongs to the last one.
        let next = self.cells.partition_point(|cell| cell.chars.end <= index);
        self.cells.get(next).or_else(|| self.cells.last())
    }
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;

    #[test]
    fn playback_spans_keep_rests_comments_slides_and_bpm_changes() {
        let text = "(120){4}1,|| 注释 ,\n,3-5-8[4:1],(240)C,,E";
        let cells = timeline(text, 100.0).unwrap();
        assert_eq!(cells.len(), 6);
        for (cell, expected) in cells.iter().zip([0.0, 0.5, 1.0, 1.5, 1.75, 2.0]) {
            assert!((cell.time - expected).abs() < 0.00002, "{cell:?}");
        }
        let slice = |cell: &TimedCell| text.chars().skip(cell.chars.start).take(cell.chars.len()).collect::<String>();
        assert_eq!(slice(&cells[1]), ",");
        assert_eq!(slice(&cells[2]), "3-5-8[4:1],");
        assert_eq!(slice(&cells[3]), "C,");
        let map = PlaybackMap { cells, ..Default::default() };
        assert!(map.active(-0.01).is_none());
        assert_eq!(slice(map.active(1.25).unwrap()), "3-5-8[4:1],");
        assert_eq!(slice(map.active(0.1).unwrap()), "1,");
    }
}
