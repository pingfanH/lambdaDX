//! Source-aware document persistence and native file dialogs.

use std::path::{Path, PathBuf};
use crate::app::types::ChartDoc;
use super::state::EditorState;

pub(crate) mod simai;
pub mod autosave;
pub mod dialogs;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartSource {
    pub path: PathBuf,
    /// Simai inote slot, NOT the difficulty rating in ChartDoc.
    pub level: Option<u32>,
}

impl ChartSource {
    pub fn resolve(path: &Path, level: Option<u32>) -> Self {
        let path = if path.is_dir() {
            let simai = path.join("maidata.txt");
            if simai.is_file() { simai } else { path.join("chart.json") }
        } else { path.to_path_buf() };
        Self { path, level }
    }

    pub fn is_simai(&self) -> bool {
        self.path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
    }
}

/// Write back to the file/slot actually loaded, never to a global scratch file.
pub fn save_chart(editor: &mut EditorState, chart: &ChartDoc) {
    if let Err(error) = editor.simai_editor.check_applied() {
        editor.status = Some(error);
        return;
    }
    let chart = crate::templates::document(editor, chart);
    let Some(source) = editor.source.as_ref() else {
        editor.save_as_request = true;
        return;
    };
    if source.is_simai() && has_editor_data(&chart) {
        editor.save_as_request = true;
        editor.status = Some("Save As JSON to preserve templates and Autoplay events".into());
        return;
    }
    let result = save_to_source(source, &chart);
    if result.is_ok() { crate::templates::mark_saved(editor); }
    editor.status = Some(match result {
        Ok(path) => format!("Saved {}", path.display()),
        Err(error) => format!("Save failed: {error}"),
    });
}

pub fn save_document(editor: &mut EditorState, chart: &mut ChartDoc) {
    if let Err(error) = crate::simai_editor::apply(editor, chart) {
        editor.status = Some(error);
        return;
    }
    save_chart(editor, chart);
}

fn save_to_source(source: &ChartSource, chart: &ChartDoc) -> Result<PathBuf, String> {
    let original = std::fs::read_to_string(&source.path)
        .map_err(|error| format!("read {}: {error}", source.path.display()))?;
    let text = source_text(source, chart, &original)?;
    atomic_write(&source.path, &text)?;
    Ok(source.path.clone())
}

fn source_text(source: &ChartSource, chart: &ChartDoc, original: &str) -> Result<String, String> {
    if source.is_simai() {
        let level = source.level.ok_or("No loaded Simai difficulty")?;
        if !crate::app::maidata::inote_keys(&original).contains(&level) {
            return Err(format!("Source no longer contains &inote_{level}; reload or Save As"));
        }
        simai::document(chart, level, original)
    } else {
        native_json(chart, original)
    }
}

fn native_json(chart: &ChartDoc, original: &str) -> Result<String, String> {
    let mut root: serde_json::Value = serde_json::from_str(original.trim_start_matches('\u{feff}'))
        .map_err(|error| error.to_string())?;
    if root.get("songName").is_some() {
        return Err("Multi-difficulty maichart JSON cannot be overwritten losslessly; use Save As Simai".into());
    }
    validate(chart)?;
    let mut chart = chart.clone();
    chart.version = "editor-1-measure".into();
    crate::app::maichart::recompute_each(&mut chart.notes);
    let value = serde_json::to_value(chart).map_err(|error| error.to_string())?;
    if root.get("chart").is_some() { root["chart"] = value; }
    else { root = value; }
    serde_json::to_string_pretty(&root).map(|text| text + "\n").map_err(|error| error.to_string())
}

pub fn export_to(editor: &EditorState, chart: &ChartDoc, path: &Path) -> Result<PathBuf, String> {
    editor.simai_editor.check_applied()?;
    if editor.source.as_ref().is_some_and(|source| same_path(&source.path, path)) {
        return Err("Export destination is the open source; use Save to update it".into());
    }
    let original = match &editor.source {
        Some(source) if source.is_simai() => std::fs::read_to_string(&source.path)
            .map_err(|error| format!("read {}: {error}", source.path.display()))?,
        _ => String::new(),
    };
    let level = editor.source.as_ref().and_then(|source| source.level).unwrap_or(6);
    let canonical = crate::templates::document(editor, chart);
    validate(&canonical)?;
    let mut expanded = crate::app::templates::expand(&canonical);
    expanded.templates.clear();
    expanded.autoplay_overrides.clear();
    atomic_write(path, &simai::document(&expanded, level, &original)?)?;
    Ok(path.to_path_buf())
}

fn save_as_to(editor: &mut EditorState, chart: &ChartDoc, path: &Path) -> Result<(), String> {
    editor.simai_editor.check_applied()?;
    let chart = crate::templates::document(editor, chart);
    if has_editor_data(&chart) && !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
        return Err("Templates and Autoplay edits require .json; use Export for flattened Simai".into());
    }
    let source = if let Some(source) = editor.source.as_ref().filter(|s| same_path(&s.path, path)) {
        save_to_source(source, &chart)?;
        source.clone()
    } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
        atomic_write(path, &native_json(&chart, "{}")?)?;
        ChartSource { path: path.to_path_buf(), level: None }
    } else {
        let level = editor.source.as_ref().and_then(|s| s.level).unwrap_or(6);
        let original = editor.source.as_ref().filter(|s| s.is_simai())
            .map(|s| std::fs::read_to_string(&s.path).map_err(|error| error.to_string()))
            .transpose()?.unwrap_or_default();
        atomic_write(path, &simai::document(&chart, level, &original)?)?;
        ChartSource {
            path: path.to_path_buf(),
            level: Some(editor.source.as_ref().and_then(|s| s.level).unwrap_or(6)),
        }
    };
    editor.level_slots = if source.is_simai() {
        crate::app::chart::read_simai_source(path)
            .map(|text| crate::app::maidata::inote_keys(&text)).unwrap_or_default()
    } else { Vec::new() };
    editor.level_index = source.level.and_then(|level| editor.level_slots.iter().position(|s| *s == level)).unwrap_or(0);
    editor.chart_dir = path.to_string_lossy().into_owned();
    editor.source = Some(source);
    crate::templates::mark_saved(editor);
    Ok(())
}

pub fn prepare_export(editor: &mut EditorState, chart: &mut ChartDoc, save_as: bool) -> Result<dialogs::Request, String> {
    crate::simai_editor::apply(editor, chart)?;
    let canonical = crate::templates::document(editor, chart);
    let native = has_editor_data(&canonical)
        || editor.source.as_ref().is_some_and(|s| !s.is_simai());
    Ok(dialogs::Request::Save { save_as, native })
}

pub fn finish_export(editor: &mut EditorState, chart: &ChartDoc, save_as: bool, path: &Path) {
    let result = if save_as { save_as_to(editor, chart, path) }
        else { export_to(editor, chart, path).map(|_| ()) };
    match result {
        Ok(()) => {
            if save_as {
                editor.pending_confirmed = editor.pending_action.is_some();
            }
            editor.status = Some(format!("{} {}", if save_as { "Saved" } else { "Exported" }, path.display()));
        }
        Err(error) => editor.status = Some(format!("{} failed: {error}", if save_as { "Save As" } else { "Export" })),
    }
}

fn has_editor_data(chart: &ChartDoc) -> bool {
    !chart.templates.is_empty() || !chart.template_instances.is_empty() || !chart.autoplay_overrides.is_empty()
}

fn same_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// Serialize fully before touching the destination; failed writes leave it intact.
fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    StagedWrite::new(path, text)?.commit()
}

/// An abandoned background save removes its temporary file without touching the source.
struct StagedWrite {
    path: PathBuf,
    temp: PathBuf,
}

impl Drop for StagedWrite {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.temp); }
}

impl StagedWrite {
    fn new(path: &Path, text: &str) -> Result<Self, String> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Preserve a symlink instead of replacing it with a regular file.
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let parent = path.parent().ok_or("Destination has no parent")?;
        let name = path.file_name().ok_or("Destination has no filename")?.to_string_lossy();
        let temp = parent.join(format!(".{name}.{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
            if let Ok(metadata) = std::fs::metadata(&path) { file.set_permissions(metadata.permissions())?; }
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            Ok(())
        })();
        if result.is_err() { let _ = std::fs::remove_file(&temp); }
        result.map_err(|error| format!("write {}: {error}", path.display()))?;
        Ok(Self { path, temp })
    }

    fn commit(self) -> Result<(), String> {
        std::fs::rename(&self.temp, &self.path)
            .map_err(|error| format!("write {}: {error}", self.path.display()))
    }
}

pub(crate) fn validate(chart: &ChartDoc) -> Result<(), String> {
    if !chart.bpm.is_finite() || chart.bpm <= 0.0 || !chart.audio_offset.is_finite() {
        return Err("Invalid BPM or audio offset".into());
    }
    if chart.bpms.iter().any(|b| !b.measure.is_finite() || b.measure < 1.0 || !b.bpm.is_finite() || b.bpm <= 0.0)
        || chart.bpms.windows(2).any(|pair| pair[0].measure >= pair[1].measure)
        || chart.bpms.first().is_some_and(|b| b.measure != 1.0) {
        return Err("BPM changes must be positive, ordered and start at measure 1".into());
    }
    for note in chart.notes.iter().chain(chart.templates.iter().flat_map(|t| &t.notes)) {
        if !note.time.is_finite() || !note.hold_duration.is_finite() || note.hold_duration < 0.0
            || !note.hi_speed.is_finite() || note.hi_speed < 0.0 {
            return Err(format!("Note {} has invalid timing or speed", note.id));
        }
        for slide in &note.slide {
            if !slide.slide_duration.is_finite() || !slide.slide_start_delay.is_finite()
                || slide.slide_start_delay < 0.0 || slide.slide_duration < slide.slide_start_delay
                || slide.segments.iter().flat_map(|s| &s.points).any(|p| !p.beat_offset.is_finite()) {
                return Err(format!("Note {} has invalid slide timing", note.id));
            }
        }
    }
    if chart.notes.iter().any(|n| n.time < 1.0) { return Err("Main chart notes must start at measure 1 or later".into()); }
    if chart.templates.iter().any(|t| !t.duration.is_finite() || t.duration < 0.0)
        || chart.template_instances.iter().any(|t| !t.anchor_time.is_finite() || t.anchor_time < 1.0) {
        return Err("Invalid template timing".into());
    }
    let mut ids = std::collections::HashSet::new();
    if chart.templates.iter().any(|t| !ids.insert(&t.id)) {
        return Err("Duplicate template ID".into());
    }
    let mut instances = std::collections::HashSet::new();
    if chart.template_instances.iter().any(|i| !instances.insert(&i.instance_id) || !ids.contains(&i.template_id)) {
        return Err("Duplicate instance ID or missing template definition".into());
    }
    for notes in std::iter::once(&chart.notes).chain(chart.templates.iter().map(|t| &t.notes)) {
        let mut note_ids = std::collections::HashSet::new();
        if notes.iter().any(|n| !note_ids.insert(n.id)) { return Err("Duplicate note ID".into()); }
    }
    let mut keys = std::collections::HashSet::new();
    if chart.autoplay_overrides.iter().any(|item| !keys.insert(&item.key)) {
        return Err("Duplicate Autoplay track override".into());
    }
    Ok(())
}
