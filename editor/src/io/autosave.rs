//! Debounced source saves. Serialization and file staging never run in the frame loop.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime};
use super::{ChartSource, StagedWrite};
use crate::app::types::ChartDoc;
use crate::state::EditorState;

struct Snapshot {
    source: ChartSource,
    chart: ChartDoc,
    document: u64,
    since: Instant,
    attempted: bool,
}

#[derive(PartialEq)]
struct Stamp {
    path: std::path::PathBuf,
    modified: SystemTime,
    len: u64,
}

impl Stamp {
    fn read(source: &ChartSource) -> Result<Self, String> {
        let path = source.path.canonicalize().map_err(|e| e.to_string())?;
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        Ok(Self { path, modified: meta.modified().map_err(|e| e.to_string())?, len: meta.len() })
    }
}

struct Prepared {
    file: StagedWrite,
    original: Stamp,
}

impl Prepared {
    fn new(source: &ChartSource, chart: &ChartDoc) -> Result<Self, String> {
        let original = Stamp::read(source)?;
        let text = std::fs::read_to_string(&original.path).map_err(|e| e.to_string())?;
        let text = super::source_text(source, chart, &text)?;
        let file = StagedWrite::new(&original.path, &text)?;
        Ok(Self { file, original })
    }

    fn commit(self, source: &ChartSource) -> Result<(), String> {
        if Stamp::read(source)? != self.original {
            return Err("Source changed during automatic save; reload or save manually".into());
        }
        self.file.commit()
    }
}

#[derive(Default)]
pub struct AutoSave {
    observed: Option<Snapshot>,
    pending: Option<Receiver<Result<Prepared, String>>>,
    next_poll: Option<Instant>,
}

impl AutoSave {
    pub fn tick(&mut self, editor: &mut EditorState, chart: &ChartDoc, document: u64, enabled: bool) {
        if !enabled || !editor.auto_save || !editor.dirty || editor.simai_editor.modified()
            || editor.keyboard.ime || editor.pending_action.is_some() || editor.source.is_none() {
            self.observed = None;
            self.pending = None;
            self.next_poll = None;
            return;
        }
        let now = Instant::now();
        if self.next_poll.is_some_and(|at| now < at) { return; }
        self.next_poll = Some(now + Duration::from_millis(250));
        let source = editor.source.as_ref().unwrap();
        let canonical = crate::templates::document(editor, chart);
        let unchanged = self.observed.as_ref().is_some_and(|s|
            s.document == document && s.source == *source && s.chart == canonical);
        if !unchanged {
            self.pending = None;
            self.observed = Some(Snapshot { source: source.clone(), chart: canonical,
                document, since: now, attempted: false });
            return;
        }
        if let Some(job) = &self.pending {
            let result = match job.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => Err("Automatic save worker stopped".into()),
            };
            self.pending = None;
            let result = result.and_then(|file| file.commit(source));
            editor.status = Some(match result {
                Ok(()) => {
                    let path = source.path.display().to_string();
                    crate::templates::mark_saved(editor);
                    format!("Auto-saved {path}")
                }
                Err(error) => format!("Auto-save: {error}"),
            });
            return;
        }
        let snapshot = self.observed.as_mut().unwrap();
        if snapshot.attempted || now.duration_since(snapshot.since) < Duration::from_secs(2) { return; }
        snapshot.attempted = true;
        if source.is_simai() && super::has_editor_data(&snapshot.chart) {
            editor.status = Some("Auto-save paused: Save As JSON to preserve templates and Autoplay events".into());
            return;
        }
        let source = snapshot.source.clone();
        let chart = snapshot.chart.clone();
        let (send, receive) = mpsc::channel();
        match std::thread::Builder::new().name("editor-auto-save".into()).spawn(move || {
            let _ = send.send(Prepared::new(&source, &chart));
        }) {
            Ok(_) => self.pending = Some(receive),
            Err(error) => editor.status = Some(format!("Auto-save: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_commits_current_document_but_discards_changed_snapshots() {
        let path = std::env::temp_dir().join(format!("lambda-autosave-current-{}.json", std::process::id()));
        std::fs::write(&path, "{}").unwrap();
        let source = ChartSource { path: path.clone(), level: None };
        let mut chart: ChartDoc = serde_json::from_value(serde_json::json!({
            "version": "test", "title": "First", "bpm": 120, "notes": []
        })).unwrap();
        let mut editor = EditorState { auto_save: true, dirty: true, source: Some(source.clone()), ..Default::default() };
        let mut saves = AutoSave::default();
        saves.tick(&mut editor, &chart, 1, true);
        let (send, receive) = mpsc::channel();
        send.send(Prepared::new(&source, &chart)).ok().unwrap();
        saves.pending = Some(receive);
        saves.next_poll = None;
        chart.title = "Second".into();
        saves.tick(&mut editor, &chart, 1, true);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
        assert!(editor.dirty);
        let (send, receive) = mpsc::channel();
        send.send(Prepared::new(&source, &chart)).ok().unwrap();
        saves.pending = Some(receive);
        saves.next_poll = None;
        saves.tick(&mut editor, &chart, 1, true);
        let saved: ChartDoc = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.title, "Second");
        assert!(!editor.dirty);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_staged_save_cannot_overwrite_a_newer_save() {
        let path = std::env::temp_dir().join(format!("lambda-autosave-{}.json", std::process::id()));
        std::fs::write(&path, "{}").unwrap();
        let source = ChartSource { path: path.clone(), level: None };
        let original = Stamp::read(&source).unwrap();
        let file = StagedWrite::new(&path, "stale background output").unwrap();
        let temp = file.temp.clone();
        std::fs::write(&path, "newer manual output").unwrap();
        assert!(Prepared { file, original }.commit(&source).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "newer manual output");
        assert!(!temp.exists());
        std::fs::remove_file(path).unwrap();
    }
}
