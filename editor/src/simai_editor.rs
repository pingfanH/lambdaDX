//! A draft is applied atomically; graphical edits never silently overwrite it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::app::types::{BpmChange, ChartDoc, Note};
use crate::state::EditorState;

pub mod source;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClickSeek { Off, Click, CommandClick }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NoteHighlight { Off, Visible, Current }

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Options {
    pub highlight: bool,
    pub follow: bool,
    pub follow_cursor: bool,
    pub space_playback: bool,
    pub click_seek: ClickSeek,
    pub note_highlight: NoteHighlight,
    pub live_apply: bool,
}

impl Default for Options {
    fn default() -> Self { Self { highlight: true, follow: true, follow_cursor: false, space_playback: true,
        click_seek: ClickSeek::CommandClick, note_highlight: NoteHighlight::Visible, live_apply: true } }
}

#[derive(Debug, Clone)]
struct PendingApply {
    revision: u64,
    bpm: f32,
    level: u32,
    result: Arc<Mutex<Option<Result<ChartDoc, String>>>>,
}

#[derive(Debug, Clone)]
struct Source {
    notes: Vec<Note>,
    bpms: Vec<BpmChange>,
    bpm: f32,
}

impl Source {
    fn new(chart: &ChartDoc) -> Self {
        Self { notes: chart.notes.clone(), bpms: chart.bpms.clone(), bpm: chart.bpm }
    }

    fn matches(&self, chart: &ChartDoc) -> bool {
        self.notes == chart.notes && self.bpms == chart.bpms && self.bpm == chart.bpm
    }
}

#[derive(Debug, Clone, Default)]
pub struct SimaiEditor {
    pub text: String,
    pub error: Option<String>,
    pub generation: u64,
    pub options: Options,
    pub playback: source::PlaybackMap,
    source: Option<Source>,
    applied: String,
    modified: bool,
    revision: u64,
    apply_at: Option<Instant>,
    pending: Option<PendingApply>,
    live_apply_seen: bool,
    cursor: Option<usize>,
    cursor_time: Option<f32>,
    cursor_follow_seen: bool,
    pending_cursor_seek: Option<bool>,
}

impl SimaiEditor {
    pub fn reset(&mut self) {
        *self = Self { generation: self.generation + 1, options: self.options.clone(), ..Self::default() };
    }

    pub fn modified(&self) -> bool { self.modified }

    pub fn cursor_seek(&mut self, index: usize, jump: bool) -> Option<f32> {
        let moved = self.cursor.replace(index) != Some(index);
        let enabled = self.options.follow_cursor && !self.cursor_follow_seen;
        let disabled = !self.options.follow_cursor && self.cursor_follow_seen;
        self.cursor_follow_seen = self.options.follow_cursor;
        if disabled || (moved && !self.options.follow_cursor) { self.pending_cursor_seek = None; }
        if enabled { self.cursor_time = None; }
        if jump { self.pending_cursor_seek = Some(true); }
        else if self.options.follow_cursor && (moved || enabled) {
            self.pending_cursor_seek.get_or_insert(false);
        }
        let force = self.pending_cursor_seek?;
        // Keep the latest caret request while live apply or its map is pending.
        if self.modified { return None; }
        let time = self.playback.at_caret(index)?.time;
        self.pending_cursor_seek = None;
        let changed = self.cursor_time.replace(time) != Some(time);
        (force || changed).then_some(time)
    }

    pub fn changed(&mut self) {
        self.modified = self.text != self.applied;
        self.error = None;
        self.revision += 1;
        self.apply_at = Some(Instant::now() + Duration::from_millis(300));
    }

    pub fn conflict(&self, chart: &ChartDoc) -> bool {
        self.modified && self.source.as_ref().is_some_and(|s| !s.matches(chart))
    }

    pub fn check_applied(&self) -> Result<(), String> {
        if self.modified { Err("Simai text has unapplied changes".into()) } else { Ok(()) }
    }

    pub fn refresh(&mut self, chart: &ChartDoc) {
        self.generation += 1;
        self.cursor = None;
        self.cursor_time = None;
        self.pending_cursor_seek = None;
        self.revision += 1;
        self.apply_at = None;
        self.pending = None;
        self.source = Some(Source::new(chart));
        match crate::io::simai::editable_body(chart) {
            Ok(text) => { self.text = text; self.error = None; }
            Err(error) => { self.text.clear(); self.error = Some(error); }
        }
        self.applied = self.text.clone();
        self.modified = false;
        self.playback.invalidate();
    }

    pub fn sync(&mut self, chart: &ChartDoc) {
        if !self.modified && self.source.as_ref().is_none_or(|s| !s.matches(chart)) {
            self.refresh(chart);
        }
    }
}

pub fn apply(editor: &mut EditorState, chart: &mut ChartDoc) -> Result<(), String> {
    if !editor.simai_editor.modified() { return Ok(()); }
    editor.simai_editor.revision += 1;
    editor.simai_editor.apply_at = None;
    editor.simai_editor.pending = None;
    let result = apply_draft(editor, chart);
    editor.simai_editor.error = result.as_ref().err().cloned();
    result
}

fn apply_draft(editor: &mut EditorState, chart: &mut ChartDoc) -> Result<(), String> {
    let level = editor.source.as_ref().and_then(|s| s.level).unwrap_or(6);
    let parsed = parse_draft(&editor.simai_editor.text, chart.bpm, level)?;
    apply_parsed(editor, chart, parsed)
}

fn parse_draft(body: &str, bpm: f32, level: u32) -> Result<ChartDoc, String> {
    let body = body.trim();
    if body.is_empty() { return Err("Simai text is empty; use E for an empty chart".into()); }
    if body.lines().any(|line| line.trim_start().starts_with('&')) {
        return Err("Edit the current difficulty's note text here, without maidata &fields".into());
    }
    // The core tolerates unfinished tokens; live editing must keep those as drafts.
    let mut delimiters = Vec::new();
    for (line, text) in body.lines().enumerate() {
        let text = text.split_once("||").map_or(text, |(code, _)| code);
        for ch in text.chars() {
            match ch {
                '(' | '{' | '[' => delimiters.push((ch, line + 1)),
                ')' | '}' | ']' => {
                    let expected = match ch { ')' => '(', '}' => '{', _ => '[' };
                    if delimiters.pop().is_none_or(|(open, _)| open != expected) {
                        return Err(format!("Unmatched {ch} on line {}", line + 1));
                    }
                }
                _ => {}
            }
        }
    }
    if let Some((ch, line)) = delimiters.last() {
        return Err(format!("Unclosed {ch} on line {line}"));
    }
    crate::app::maidata::from_maidata_level(&format!("&wholebpm={bpm}\n&inote_{level}={body}\n"), level)
}

/// Parsing is debounced and off-thread; only the latest valid draft can commit.
pub fn tick(editor: &mut EditorState, chart: &mut ChartDoc, enabled: bool) {
    let simai = &mut editor.simai_editor;
    if simai.live_apply_seen != simai.options.live_apply {
        simai.live_apply_seen = simai.options.live_apply;
        if simai.options.live_apply && simai.modified() {
            simai.apply_at = Some(Instant::now() + Duration::from_millis(300));
            simai.error = None;
        }
    }
    if !enabled || !editor.simai_editor.options.live_apply || editor.template_session.is_some()
        || editor.keyboard.ime || !editor.simai_editor.modified() { return; }
    let level = editor.source.as_ref().and_then(|s| s.level).unwrap_or(6);
    let ready = editor.simai_editor.pending.as_ref().and_then(|job| {
        job.result.try_lock().ok()?.take().map(|result| (job.revision, job.bpm, job.level, result))
    });
    if let Some((revision, bpm, parsed_level, result)) = ready {
        editor.simai_editor.pending = None;
        if revision == editor.simai_editor.revision && bpm == chart.bpm && parsed_level == level {
            let result = result.and_then(|parsed| apply_parsed(editor, chart, parsed));
            editor.simai_editor.error = result.err();
        } else if revision == editor.simai_editor.revision {
            editor.simai_editor.apply_at = Some(Instant::now() + Duration::from_millis(300));
        }
    }
    let simai = &mut editor.simai_editor;
    if !simai.modified() || simai.conflict(chart) || simai.pending.is_some()
        || !simai.apply_at.is_some_and(|at| Instant::now() >= at) { return; }
    simai.apply_at = None;
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    let text = simai.text.clone();
    let bpm = chart.bpm;
    match std::thread::Builder::new().name("simai-live-apply".into()).spawn(move || {
        let parsed = parse_draft(&text, bpm, level);
        if let Ok(mut slot) = output.lock() { *slot = Some(parsed); }
    }) {
        Ok(_) => simai.pending = Some(PendingApply { revision: simai.revision, bpm, level, result }),
        Err(error) => simai.error = Some(format!("Unable to apply Simai: {error}")),
    }
}

fn apply_parsed(editor: &mut EditorState, chart: &mut ChartDoc, mut parsed: ChartDoc) -> Result<(), String> {
    if editor.template_session.is_some() {
        return Err("Exit template editing before applying Simai text".into());
    }
    if editor.simai_editor.conflict(chart) {
        return Err("Chart changed after this draft began; copy the draft, then reload it from the chart".into());
    }
    preserve_ids(chart, &mut parsed)?;
    let mut updated = chart.clone();
    updated.notes = parsed.notes;
    updated.bpms = parsed.bpms;
    updated.bpm = parsed.bpm;
    // Only remove owners deleted from the main chart, leaving template and
    // standalone authored events intact. New notes cannot inherit deleted IDs.
    let live: std::collections::HashSet<_> = updated.notes.iter().map(|n| n.id).collect();
    let removed: std::collections::HashSet<_> = chart.notes.iter().map(|n| n.id)
        .filter(|id| !live.contains(id)).collect();
    updated.autoplay_overrides.retain(|item| item.key.is_standalone() || !removed.contains(&item.key.note_id));
    crate::app::maichart::recompute_each(&mut updated.notes);
    crate::app::maichart::mark_double_stars(&mut updated.notes);
    crate::io::validate(&updated)?;
    if !editor.simai_editor.source.as_ref().is_some_and(|s| s.matches(&updated)) {
        editor.push_history(chart);
        *chart = updated;
        editor.selected.clear();
        editor.hovered = None;
        editor.pinned_note = None;
        editor.keyboard.candidate = None;
        editor.editing_slide = None;
        editor.pending_shape = None;
        editor.last_edit = None;
        editor.autoplay_editor.cancel();
    }
    editor.simai_editor.source = Some(Source::new(chart));
    editor.simai_editor.applied = editor.simai_editor.text.clone();
    editor.simai_editor.modified = false;
    editor.simai_editor.apply_at = None;
    editor.simai_editor.playback.invalidate();
    editor.status = Some(format!("Simai applied: {} notes", chart.notes.len()));
    Ok(())
}

fn preserve_ids(previous: &ChartDoc, parsed: &mut ChartDoc) -> Result<(), String> {
    fn key(note: &Note, bpms: &[BpmChange]) -> Option<(i64, String)> {
        let text = crate::core_chart::note_text(note, bpms).ok()?;
        Some(((note.time as f64 * 100_000.0).round() as i64, text))
    }
    let mut ids: HashMap<_, Vec<u64>> = HashMap::new();
    for note in previous.notes.iter().rev() {
        if let Some(key) = key(note, &previous.bpms) { ids.entry(key).or_default().push(note.id); }
    }
    let mut next = previous.notes.iter().map(|n| n.id).max().unwrap_or(0);
    for note in &mut parsed.notes {
        note.id = if let Some(id) = key(note, &parsed.bpms).and_then(|key| ids.get_mut(&key)?.pop()) {
            id
        } else {
            next = next.checked_add(1).ok_or("Note IDs exhausted")?;
            next
        };
    }
    Ok(())
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;

    #[test]
    fn cursor_seek_waits_for_mapping_and_skips_redundant_follow() {
        let mut simai = SimaiEditor::default();
        let text = "(120){4}1,  ,3,E";
        let cells = source::timeline(text, 120.0).unwrap();
        let (first, second, third) = (cells[0].chars.start, cells[1].chars.start, cells[2].chars.start);
        assert!(simai.cursor_seek(second, true).is_none());
        simai.playback.cells = cells.clone();
        assert_eq!(simai.cursor_seek(second, false), Some(0.5));
        assert!(simai.cursor_seek(first, false).is_none());
        simai.options.follow_cursor = true;
        assert_eq!(simai.cursor_seek(first, false), Some(0.0));
        assert!(simai.cursor_seek(first, false).is_none());
        assert!(simai.cursor_seek(first + 1, false).is_none());
        assert_eq!(simai.cursor_seek(first, true), Some(0.0));
        simai.modified = true;
        assert!(simai.cursor_seek(second, false).is_none());
        assert!(simai.cursor_seek(third, false).is_none());
        simai.modified = false;
        simai.playback.cells.clear();
        assert!(simai.cursor_seek(third, false).is_none());
        simai.playback.cells = cells;
        assert_eq!(simai.cursor_seek(third, false), Some(1.0));
        assert!(simai.cursor_seek(third, false).is_none());
        assert_eq!(simai.playback.at_caret(first + 2).unwrap().time, 0.5);
        assert_eq!(simai.playback.at_caret(text.chars().count()).unwrap().time, 1.5);
    }

    #[test]
    fn live_apply_discards_stale_results_and_keeps_invalid_drafts() {
        let mut chart = parse_draft("(120){4}1,E", 120.0, 6).unwrap();
        let original = chart.clone();
        let mut editor = EditorState::default();
        editor.simai_editor.refresh(&chart);
        editor.simai_editor.live_apply_seen = true;
        editor.simai_editor.text = "(120){4}2,E".into();
        editor.simai_editor.changed();
        let revision = editor.simai_editor.revision;
        editor.simai_editor.pending = Some(PendingApply { revision, bpm: 120.0, level: 6,
            result: Arc::new(Mutex::new(Some(parse_draft("(120){4}2,E", 120.0, 6)))) });
        editor.simai_editor.text = "(120){4}3,E".into();
        editor.simai_editor.changed();
        tick(&mut editor, &mut chart, true);
        assert_eq!(chart, original);
        assert!(editor.simai_editor.modified());
        editor.simai_editor.pending = Some(PendingApply { revision: editor.simai_editor.revision, bpm: 120.0, level: 6,
            result: Arc::new(Mutex::new(Some(parse_draft("(120){4}3,E", 120.0, 6)))) });
        tick(&mut editor, &mut chart, false);
        assert_eq!(chart, original);
        tick(&mut editor, &mut chart, true);
        assert_eq!(chart.notes[0].lane, 3);
        assert!(editor.dirty);
        assert!(!editor.simai_editor.modified());
        let applied = chart.clone();
        for draft in ["(120){4}3h[", "(120){4}3h[4:1)", "(120){4}3,(", "(120){4}3,{"] {
            editor.simai_editor.text = draft.into();
            editor.simai_editor.changed();
            assert!(apply(&mut editor, &mut chart).is_err());
            assert_eq!(chart, applied);
            assert!(editor.simai_editor.modified());
        }
        assert!(parse_draft("(120){4}3,|| [draft comment] (\nE", 120.0, 6).is_ok());
    }
}
