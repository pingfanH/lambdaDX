use super::*;
use crate::app::types::ChartDoc;
use std::{
    collections::{VecDeque, hash_map::RandomState},
    hash::BuildHasher,
    io::Write,
    time::{Duration, Instant},
};

const MAX_SNAPSHOTS: usize = 8;
const MAX_STORE_BYTES: usize = 32 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(300);

#[derive(PartialEq, Eq)]
struct Identity {
    revision: u64,
    source: Option<crate::io::ChartSource>,
    level: u32,
    isolation: Option<String>,
}
#[derive(Clone)]
pub(super) struct Snapshot {
    pub target: SnapshotRef,
    pub chart: ChartDoc,
    pub order: Vec<usize>,
    pub text: Option<String>,
    pub draft: bool,
    pub matches_chart: bool,
    pub(super) created: Instant,
    pub(super) bytes: usize,
}
pub(super) struct Store {
    instance_id: String,
    document_id: String,
    identity: Option<Identity>,
    snapshots: VecDeque<Snapshot>,
    bytes: usize,
}
// Opaque random identifiers are references, never authentication credentials.
fn token() -> String {
    format!(
        "{:016x}{:016x}",
        RandomState::new().hash_one(0_u64),
        RandomState::new().hash_one(1_u64)
    )
}
impl Store {
    pub fn new() -> Self {
        Self {
            instance_id: token(),
            document_id: token(),
            identity: None,
            snapshots: VecDeque::new(),
            bytes: 0,
        }
    }
    pub fn observe(&mut self, editor: &EditorState, app: &PadPreviewState) {
        let identity = Identity {
            revision: app.document_revision(),
            source: editor.source.clone(),
            level: app.chart.simai_level,
            isolation: editor
                .template_session
                .as_ref()
                .map(|s| s.template_id.clone()),
        };
        if self.identity.as_ref() != Some(&identity) {
            self.identity = Some(identity);
            self.document_id = token();
            self.snapshots.clear();
            self.bytes = 0;
        }
        while self
            .snapshots
            .front()
            .is_some_and(|s| s.created.elapsed() >= TTL)
        {
            self.evict();
        }
    }
    fn evict(&mut self) {
        if let Some(s) = self.snapshots.pop_front() {
            self.bytes -= s.bytes;
        }
    }
    pub fn owns_document(&self, target: &SnapshotRef) -> bool {
        target.document_id == self.document_id
    }
    pub fn get(&self, target: &SnapshotRef) -> ToolResult<&Snapshot> {
        if target.document_id == self.document_id {
            if let Some(snapshot) = self
                .snapshots
                .iter()
                .find(|s| s.target == *target && s.created.elapsed() < TTL)
            {
                return Ok(snapshot);
            }
        }
        Err(ToolError::new(
            ErrorCode::DocumentChanged,
            "Unknown, evicted or expired snapshot; capture a new context",
        ))
    }
    pub fn capture(
        &mut self,
        editor: &EditorState,
        app: &PadPreviewState,
        core_current: bool,
    ) -> ToolResult<Context> {
        // Keep finite but malformed tempo maps inspectable by chart_check.
        // JSON has no representation for NaN/infinity in context metadata.
        if !app.chart.bpm.is_finite()
            || !app.chart.audio_offset.is_finite()
            || app
                .chart
                .bpms
                .iter()
                .any(|b| !b.measure.is_finite() || !b.bpm.is_finite())
        {
            return Err(ToolError::new(
                ErrorCode::ValidationFailed,
                "Context tempo metadata contains a non-finite number",
            ));
        }
        let (has_source, matches_chart) = editor.simai_editor.automation_source(&app.chart);
        let mut size = Counter(0);
        serde_json::to_writer(&mut size, &app.chart)
            .map_err(|e| ToolError::new(ErrorCode::ValidationFailed, e.to_string()))?;
        // Count serialized data plus in-memory fixed fields and sorted indices.
        let nested_notes = app
            .chart
            .templates
            .iter()
            .map(|t| t.notes.len())
            .sum::<usize>();
        let bytes = size
            .0
            .saturating_add(editor.simai_editor.text.len())
            .saturating_add((app.chart.notes.len() + nested_notes).saturating_mul(
                std::mem::size_of::<crate::app::types::Note>() + std::mem::size_of::<usize>(),
            ));
        if bytes > MAX_SNAPSHOT_BYTES {
            return Err(ToolError::invalid(
                "Chart snapshot exceeds 16 MiB; no partial snapshot was captured",
            ));
        }
        let target = SnapshotRef {
            document_id: self.document_id.clone(),
            snapshot_token: token(),
        };
        let mut chart = app.chart.clone();
        chart.notes.retain(|n| n.template_source.is_none());
        let mut order: Vec<_> = (0..chart.notes.len()).collect();
        order.sort_by(|&a, &b| {
            chart.notes[a]
                .time
                .total_cmp(&chart.notes[b].time)
                .then_with(|| chart.notes[a].id.cmp(&chart.notes[b].id))
        });
        let authored: std::collections::HashSet<_> = chart.notes.iter().map(|n| n.id).collect();
        let mut selected: Vec<_> = editor
            .selected
            .iter()
            .copied()
            .filter(|id| authored.contains(id))
            .collect();
        selected.sort_unstable();
        let selected_set: std::collections::HashSet<_> = selected.iter().copied().collect();
        let mut selected_notes = Vec::new();
        let mut selection_bytes = 0usize;
        for &index in &order {
            let note = &chart.notes[index];
            if !selected_set.contains(&note.id) {
                continue;
            }
            // A malformed note must remain inspectable through chart_check.
            let Ok(note) = super::query::dto(note) else {
                continue;
            };
            let bytes = serde_json::to_vec(&note)
                .map_err(|e| ToolError::invalid(e.to_string()))?
                .len()
                + 1;
            if selected_notes.len() >= MAX_PAGE_SIZE
                || selection_bytes + bytes > MAX_RESULT_BYTES / 2
            {
                break;
            }
            selection_bytes += bytes;
            selected_notes.push(note);
        }
        let selection_notes_truncated = selected_notes.len() < selected.len();
        let mut warnings = vec![
            "Queries refer to this immutable snapshot; capture a new context to see later edits."
                .into(),
        ];
        if let Some(error) = super::query::tempo_error(&chart) {
            warnings.push(error.into());
        }
        if !matches_chart {
            warnings.push("Simai text does not represent the current canonical chart.".into());
        }
        if !chart.templates.is_empty() {
            warnings.push("Only authored main-chart notes are queried; template definitions and expanded notes are excluded.".into());
        }
        let mut context = Context {
            schema_version: SCHEMA_VERSION,
            instance_id: self.instance_id.clone(),
            target: target.clone(),
            title: chart.title.clone(),
            artist: chart.artist.clone(),
            bpm: chart.bpm as f64,
            bpm_changes: chart
                .bpms
                .iter()
                .map(|b| BpmChange {
                    measure: b.measure as f64,
                    bpm: b.bpm as f64,
                })
                .collect(),
            audio_offset_seconds: chart.audio_offset as f64,
            source_format: editor
                .source
                .as_ref()
                .map_or(
                    "memory",
                    |s| if s.is_simai() { "simai" } else { "native_json" },
                )
                .into(),
            difficulty_slot: editor.source.as_ref().and_then(|s| s.level),
            authored_note_count: chart.notes.len(),
            template_count: chart.templates.len(),
            template_instance_count: chart.template_instances.len(),
            selected_note_ids: selected.into_iter().map(|id| id.to_string()).collect(),
            selected_notes,
            selection_notes_truncated,
            has_simai_source: has_source,
            unapplied_draft: editor.simai_editor.modified(),
            draft_conflict: editor.simai_editor.conflict(&app.chart),
            simai_matches_chart: matches_chart,
            core_available: app.judge_engine.is_some(),
            core_current: core_current && app.judge_engine.is_some(),
            auto_save: editor.auto_save,
            dirty: editor.dirty,
            editor_busy: false,
            warnings,
        };
        while serde_json::to_vec(&context)
            .map_err(|e| ToolError::invalid(e.to_string()))?
            .len()
            > MAX_RESULT_BYTES
        {
            if context.selected_notes.pop().is_none() {
                return Err(ToolError::invalid(
                    "Context metadata exceeds 64 KiB; snapshot was not captured",
                ));
            }
            context.selection_notes_truncated = true;
        }
        while self.snapshots.len() >= MAX_SNAPSHOTS || self.bytes + bytes > MAX_STORE_BYTES {
            self.evict();
        }
        self.snapshots.push_back(Snapshot {
            target,
            chart,
            order,
            text: has_source.then(|| editor.simai_editor.text.clone()),
            draft: context.unapplied_draft,
            matches_chart,
            created: Instant::now(),
            bytes,
        });
        self.bytes += bytes;
        Ok(context)
    }
}
struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        if self.0 > MAX_SNAPSHOT_BYTES {
            return Err(std::io::Error::other("Snapshot exceeds size limit"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_snapshots_are_rejected_and_release_their_accounted_bytes() {
        let mut store = Store::new();
        let mut chart = crate::app::chart::fallback_chart();
        chart.notes.clear();
        let app = PadPreviewState::new_at(chart, None, None, 0.0, None);
        let editor = EditorState::default();
        store.observe(&editor, &app);
        let target = store.capture(&editor, &app, false).unwrap().target;
        assert!(store.bytes > 0);
        store.snapshots.front_mut().unwrap().created = Instant::now() - TTL;
        assert_eq!(
            store.get(&target).err().unwrap().code,
            ErrorCode::DocumentChanged
        );
        store.observe(&editor, &app);
        assert!(store.snapshots.is_empty());
        assert_eq!(store.bytes, 0);
    }
}
