//! A draft is applied atomically; graphical edits never silently overwrite it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::app::types::{BpmChange, ChartDoc, Note, NoteType};
use crate::state::EditorState;

pub mod source;
pub(crate) mod format;
pub mod editing;
pub mod modal;
pub mod completion;
pub mod input;
pub mod history;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClickSeek { Off, Click, CommandClick }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NoteHighlight { Off, Visible, Current }

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Options {
    pub edit_style: modal::Style,
    pub completion: bool,
    pub auto_completion: bool,
    pub auto_pairs: bool,
    pub highlight: bool,
    pub follow: bool,
    pub follow_cursor: bool,
    pub space_playback: bool,
    pub click_seek: ClickSeek,
    pub note_highlight: NoteHighlight,
    pub live_apply: bool,
}

impl Default for Options {
    fn default() -> Self { Self { edit_style: modal::Style::Standard, completion: true, auto_completion: true, auto_pairs: true,
        highlight: true, follow: true, follow_cursor: false, space_playback: true,
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
    chart: ChartDoc,
    note_order: Vec<usize>,
}

impl Source {
    fn new(chart: &ChartDoc) -> Self {
        let mut note_order: Vec<_> = (0..chart.notes.len()).collect();
        note_order.sort_by_key(|&index| chart.notes[index].id);
        Self { chart: chart.clone(), note_order }
    }

    fn matches(&self, chart: &ChartDoc) -> bool {
        if self.chart.notes.len() != chart.notes.len()
            || self.chart.bpms != chart.bpms || self.chart.bpm != chart.bpm { return false; }
        if self.chart.notes.iter().zip(&chart.notes).all(|(a, b)| Self::same_note(a, b)) { return true; }
        // IDs identify authored notes; sorting the storage vector is not an edit.
        let mut current: Vec<_> = chart.notes.iter().collect();
        current.sort_by_key(|note| note.id);
        self.note_order.iter().zip(current).all(|(&index, note)| Self::same_note(&self.chart.notes[index], note))
    }

    fn same_note(a: &Note, b: &Note) -> bool {
        // Keep this exhaustive so new authored fields cannot silently bypass conflicts.
        let Note { id, time, lane, note_type, hold_duration, is_each: _, is_each_head: _,
            is_break, is_ex, is_star, is_fake_rotate, is_hanabi, is_tapless, is_touch,
            hi_speed, slide, template_source } = a;
        *id == b.id && *time == b.time && *lane == b.lane && *note_type == b.note_type
            && *hold_duration == b.hold_duration && *is_break == b.is_break && *is_ex == b.is_ex
            // Slide double-star and Each flags are rebuilt by runtime synchronization.
            && (*note_type == NoteType::Slide || *is_star == b.is_star)
            && *is_fake_rotate == b.is_fake_rotate && *is_hanabi == b.is_hanabi
            && *is_tapless == b.is_tapless && *is_touch == b.is_touch
            && *hi_speed == b.hi_speed && *slide == b.slide && *template_source == b.template_source
    }
}

#[derive(Debug, Clone, Default)]
pub struct SimaiEditor {
    pub text: String,
    pub error: Option<String>,
    pub generation: u64,
    pub options: Options,
    pub playback: source::PlaybackMap,
    pub input: input::Input,
    source: Option<Arc<Source>>,
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
    failed_refresh: Option<Source>,
    history_typing_at: Option<f64>,
    history_text: Option<String>,
    pub(crate) restore_cursor: bool,
}

impl SimaiEditor {
    pub fn reset(&mut self) {
        *self = Self { generation: self.generation + 1, options: self.options.clone(), ..Self::default() };
    }

    pub fn modified(&self) -> bool { self.modified }

    pub(crate) fn document(&self) -> Option<format::Document> {
        Some(format::Document { text: self.applied.clone(), chart: self.source.as_ref()?.chart.clone() })
    }

    pub fn load(&mut self, chart: &ChartDoc, text: String) {
        self.input = Default::default();
        self.source = Some(Arc::new(Source::new(chart)));
        self.text = text;
        self.applied = self.text.clone();
        self.modified = false;
        self.error = None;
        self.failed_refresh = None;
        self.history_boundary();
        self.history_text = None;
        self.playback.invalidate();
    }

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
        let result = if let Some(document) = self.document() { document.update(chart) }
            else { crate::io::simai::editable_body(chart) };
        match result {
            Ok(text) => self.load(chart, text),
            Err(error) => {
                self.text = self.applied.clone();
                self.error = Some(error);
                self.failed_refresh = Some(Source::new(chart));
            }
        }
        self.applied = self.text.clone();
        self.modified = false;
        self.playback.invalidate();
    }

    pub fn sync(&mut self, chart: &ChartDoc) {
        if !self.modified && self.source.as_ref().is_none_or(|s| !s.matches(chart))
            && !self.failed_refresh.as_ref().is_some_and(|s| s.matches(chart)) {
            let mut input = std::mem::take(&mut self.input);
            let generation = self.generation;
            self.refresh(chart);
            self.generation = generation;
            input.external_change(&self.text);
            self.input = input;
            self.restore_cursor = true;
        }
    }
}

pub fn load_source(editor: &mut EditorState, chart: &ChartDoc) {
    let body = editor.source.as_ref().filter(|source| source.is_simai()).and_then(|source| {
        let level = source.level?;
        let text = std::fs::read_to_string(&source.path).ok()?;
        crate::io::simai::body_for_level(&text, level)
    });
    if let Some(body) = body { editor.simai_editor.load(chart, body); }
    else { editor.simai_editor.refresh(chart); }
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
        return Err("Both the chart and this Simai draft have changed; the draft is kept intact. Copy it before reloading from the chart".into());
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
    let chart_changed = !editor.simai_editor.source.as_ref().is_some_and(|s| s.matches(&updated));
    if editor.simai_editor.history_text.as_deref() != Some(editor.simai_editor.text.as_str()) {
        history::record_application(editor, chart);
    }
    if chart_changed {
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
    editor.simai_editor.source = Some(Arc::new(Source::new(chart)));
    editor.simai_editor.applied = editor.simai_editor.text.clone();
    editor.simai_editor.history_text = Some(editor.simai_editor.text.clone());
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
    fn draft_conflict_ignores_derived_flags_and_storage_order() {
        let body = "(120){4}1/2,3-5[4:1]*-7[4:1],4,E";
        let mut chart = parse_draft(body, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        for note in &mut chart.notes {
            note.is_each = false;
            note.is_each_head = false;
            if note.note_type == crate::app::types::NoteType::Slide { note.is_star = false; }
        }
        let mut editor = EditorState::default();
        editor.simai_editor.load(&chart, body.into());
        editor.simai_editor.text = body.replace("4,E", "6,E");
        editor.simai_editor.changed();
        let draft = editor.simai_editor.text.clone();
        crate::app::maichart::recompute_each(&mut chart.notes);
        crate::app::maichart::mark_double_stars(&mut chart.notes);
        assert!(!editor.simai_editor.conflict(&chart), "derived note flags are not authored edits");
        chart.notes.reverse();
        assert!(!editor.simai_editor.conflict(&chart), "note storage order is not an authored edit");
        editor.simai_editor.sync(&chart);
        assert_eq!(editor.simai_editor.text, draft);
        apply(&mut editor, &mut chart).unwrap();
        assert!(chart.notes.iter().any(|note| note.lane == 6));
        assert!(!editor.simai_editor.modified());
    }

    #[test]
    fn draft_conflict_keeps_real_graphical_edits_and_text_intact() {
        let body = "(120){4}1,2,E";
        let mut chart = parse_draft(body, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        let mut editor = EditorState::default();
        editor.simai_editor.load(&chart, body.into());
        editor.simai_editor.text = body.replace("2,E", "3,E");
        editor.simai_editor.changed();
        let draft = editor.simai_editor.text.clone();
        chart.notes[0].lane = 7;
        let graphical = chart.clone();
        editor.simai_editor.sync(&chart);
        assert!(editor.simai_editor.conflict(&chart));
        assert!(apply(&mut editor, &mut chart).is_err());
        assert_eq!(chart, graphical);
        assert_eq!(editor.simai_editor.text, draft);
        assert!(editor.simai_editor.modified());
    }

    #[test]
    fn draft_conflict_still_detects_authored_note_and_clock_changes() {
        let body = "(120){4}1,2-5[4:1],E";
        let mut baseline = parse_draft(body, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut baseline.notes);
        let mut simai = SimaiEditor::default();
        simai.load(&baseline, body.into());
        simai.text = body.replace("1,", "3,");
        simai.changed();
        let changes: &[(&str, fn(&mut ChartDoc))] = &[
            ("id", |chart| chart.notes[0].id += 10),
            ("time", |chart| chart.notes[0].time += 0.125),
            ("lane", |chart| chart.notes[0].lane = 7),
            ("type", |chart| chart.notes[0].note_type = NoteType::Hold),
            ("hold duration", |chart| chart.notes[0].hold_duration = 0.5),
            ("break", |chart| chart.notes[0].is_break = true),
            ("ex", |chart| chart.notes[0].is_ex = true),
            ("authored star", |chart| chart.notes[0].is_star = true),
            ("fake rotation", |chart| chart.notes[0].is_fake_rotate = true),
            ("firework", |chart| chart.notes[0].is_hanabi = true),
            ("tapless", |chart| chart.notes[1].is_tapless = true),
            ("touch", |chart| chart.notes[0].is_touch = true),
            ("hi-speed", |chart| chart.notes[0].hi_speed = 0.75),
            ("slide duration", |chart| chart.notes[1].slide[0].slide_duration += 0.25),
            ("slide delay", |chart| chart.notes[1].slide[0].slide_start_delay += 0.125),
            ("bpm", |chart| chart.bpm = 180.0),
            ("bpm directive", |chart| chart.bpms[0].bpm = 180.0),
            ("delete", |chart| { chart.notes.pop(); }),
            ("add", |chart| { let mut note = chart.notes[0].clone(); note.id = 100; chart.notes.push(note); }),
        ];
        for (name, change) in changes {
            let mut chart = baseline.clone();
            change(&mut chart);
            assert!(simai.conflict(&chart), "missed authored change: {name}");
        }
    }

    #[test]
    fn live_apply_accepts_derived_flag_changes_without_losing_the_draft() {
        let body = "(120){4}1/2,3,E";
        let mut chart = parse_draft(body, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        for note in &mut chart.notes { note.is_each = false; note.is_each_head = false; }
        let mut editor = EditorState::default();
        editor.simai_editor.load(&chart, body.into());
        editor.simai_editor.live_apply_seen = true;
        editor.simai_editor.text = body.replace("3,E", "4,E");
        editor.simai_editor.changed();
        let parsed = parse_draft(&editor.simai_editor.text, 120.0, 6).unwrap();
        editor.simai_editor.pending = Some(PendingApply {
            revision: editor.simai_editor.revision, bpm: 120.0, level: 6,
            result: Arc::new(Mutex::new(Some(Ok(parsed)))),
        });
        crate::app::maichart::recompute_each(&mut chart.notes);
        tick(&mut editor, &mut chart, true);
        assert!(editor.simai_editor.error.is_none(), "{:?}", editor.simai_editor.error);
        assert!(!editor.simai_editor.modified());
        assert!(chart.notes.iter().any(|note| note.lane == 4));
        assert_eq!(editor.undo.len(), 1);
        let applied = chart.clone();
        tick(&mut editor, &mut chart, true);
        assert_eq!(chart, applied);
        assert_eq!(editor.undo.len(), 1);
    }

    #[test]
    fn graphical_and_text_edits_keep_the_authored_body() {
        let original = "(188){4}3h[1:2], || authored comment\n4,6h[2:9],4<5-8[8:5],E";
        let mut chart = parse_draft(original, 188.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        let mut editor = EditorState::default();
        editor.simai_editor.load(&chart, original.into());
        chart.notes[1].lane = 7;
        editor.simai_editor.sync(&chart);
        let graphical = original.replacen("\n4,", "\n7,", 1);
        assert_eq!(editor.simai_editor.text, graphical);
        editor.simai_editor.text = graphical.replace("authored comment", "new comment");
        editor.simai_editor.changed();
        apply(&mut editor, &mut chart).unwrap();
        chart.notes[1].lane = 8;
        editor.simai_editor.sync(&chart);
        assert_eq!(editor.simai_editor.text, graphical.replace("authored comment", "new comment").replacen("\n7,", "\n8,", 1));
        assert!(!editor.simai_editor.modified());
    }

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
