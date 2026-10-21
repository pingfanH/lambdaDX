use schemars::JsonSchema;
pub use crate::proposal::*;
pub use crate::library::{LibraryReadRequest, LibrarySearchRequest};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_RESULT_BYTES: usize = 64 * 1024;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_PAGE_SIZE: usize = 128;
pub const MAX_IDS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NoDocument,
    DocumentChanged,
    EditorBusy,
    UnappliedDraft,
    InvalidArgument,
    UnsupportedOperation,
    ValidationFailed,
    FormatPreservationFailed,
    PermissionDenied,
    QueueFull,
    Cancelled,
    DeadlineExceeded,
    CoreUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
}
impl ToolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }
}
impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for ToolError {}
pub type ToolResult<T> = Result<T, ToolError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Focus,
    Propose,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Permissions {
    pub read: bool,
    pub focus: bool,
    pub propose: bool,
}
impl Permissions {
    pub const READ_ONLY: Self = Self {
        read: true,
        focus: false,
        propose: false,
    };
    pub fn allows(self, permission: Permission) -> bool {
        match permission {
            Permission::Read => self.read,
            Permission::Focus => self.focus,
            Permission::Propose => self.propose,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextRequest {
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRef {
    pub document_id: String,
    pub snapshot_token: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeasureRange {
    /// Inclusive, in measures; measure 1 is playback second zero.
    pub start_measure: f64,
    /// Exclusive, in measures.
    pub end_measure: f64,
}
impl MeasureRange {
    pub fn validate(self) -> ToolResult<()> {
        if !self.start_measure.is_finite()
            || !self.end_measure.is_finite()
            || self.start_measure < 1.0
            || self.end_measure <= self.start_measure
            || self.end_measure > f32::MAX as f64
        {
            return Err(ToolError::invalid(
                "Expected finite range [start_measure, end_measure), starting at 1 or later",
            ));
        }
        Ok(())
    }
    pub fn contains(self, measure: f64) -> bool {
        measure >= self.start_measure && measure < self.end_measure
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Tap,
    Hold,
    Touch,
    TouchHold,
    Slide,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryNotesRequest {
    pub schema_version: u32,
    pub target: SnapshotRef,
    pub range: Option<MeasureRange>,
    #[serde(default)]
    pub note_ids: Vec<String>,
    #[serde(default)]
    pub note_types: Vec<NoteKind>,
    /// Offset in the filtered, sorted immutable snapshot.
    #[serde(default)]
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterRange {
    /// Zero-based Unicode scalar index, inclusive. CR and LF each count as one.
    pub start_char: usize,
    pub end_char: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadSimaiRequest {
    pub schema_version: u32,
    pub target: SnapshotRef,
    pub range: CharacterRange,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckLevel {
    Structure,
    Core,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    pub schema_version: u32,
    pub target: SnapshotRef,
    pub range: Option<MeasureRange>,
    pub level: CheckLevel,
    #[serde(default)]
    pub offset: usize,
    pub limit: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusRequest {
    pub schema_version: u32,
    pub target: SnapshotRef,
    pub range: MeasureRange,
}

#[derive(Debug, Clone)]
pub enum Request {
    LibrarySearch(LibrarySearchRequest),
    LibraryRead(LibraryReadRequest),
    Context(ContextRequest),
    QueryNotes(QueryNotesRequest),
    ReadSimai(ReadSimaiRequest),
    Check(CheckRequest),
    Focus(FocusRequest),
    Propose(ProposeRequest),
    ProposalStatus(ProposalStatusRequest),
}
impl Request {
    pub fn permission(&self) -> Permission {
        if matches!(self, Self::Propose(_)) {
            Permission::Propose
        } else if matches!(self, Self::Focus(_)) {
            Permission::Focus
        } else {
            Permission::Read
        }
    }
    pub fn validate(&self) -> ToolResult<()> {
        let (version, target) = match self {
            Self::LibrarySearch(r) => { r.validate()?; (r.schema_version, None) }
            Self::LibraryRead(r) => { r.validate()?; (r.schema_version, None) }
            Self::Context(r) => (r.schema_version, None),
            Self::QueryNotes(r) => {
                page(r.limit)?;
                if let Some(range) = r.range {
                    range.validate()?;
                }
                if r.note_ids.len() > MAX_IDS || r.note_types.len() > 5 {
                    return Err(ToolError::invalid("Too many note filters"));
                }
                for id in &r.note_ids {
                    let parsed = id
                        .parse::<u64>()
                        .map_err(|_| ToolError::invalid("Note IDs must be decimal u64 strings"))?;
                    if parsed.to_string() != *id {
                        return Err(ToolError::invalid(
                            "Note IDs must use canonical decimal notation",
                        ));
                    }
                }
                (r.schema_version, Some(&r.target))
            }
            Self::ReadSimai(r) => {
                if r.range.end_char < r.range.start_char
                    || r.range.end_char - r.range.start_char > MAX_TEXT_BYTES
                {
                    return Err(ToolError::invalid(
                        "Invalid or oversized character range; request a smaller slice",
                    ));
                }
                (r.schema_version, Some(&r.target))
            }
            Self::Check(r) => {
                page(r.limit)?;
                if let Some(range) = r.range {
                    range.validate()?;
                }
                (r.schema_version, Some(&r.target))
            }
            Self::Propose(r) => {
                r.validate()?;
                (r.schema_version, Some(&r.target))
            }
            Self::ProposalStatus(r) => {
                if r.proposal_id.is_empty() || r.proposal_id.len() > 128 {
                    return Err(ToolError::invalid("Invalid proposal ID"));
                }
                (r.schema_version, None)
            }
            Self::Focus(r) => {
                r.range.validate()?;
                (r.schema_version, Some(&r.target))
            }
        };
        if version != SCHEMA_VERSION {
            return Err(ToolError::invalid("Unsupported schema_version"));
        }
        if let Some(target) = target {
            if [&target.document_id, &target.snapshot_token]
                .iter()
                .any(|s| s.is_empty() || s.len() > 128)
            {
                return Err(ToolError::invalid("Missing or oversized snapshot identity"));
            }
        }
        Ok(())
    }
}
fn page(limit: usize) -> ToolResult<()> {
    if !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(ToolError::invalid("limit must be 1..128; use pagination"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema_version: u32,
    pub instance_id: String,
    pub target: SnapshotRef,
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub bpm_changes: Vec<BpmChange>,
    pub audio_offset_seconds: f64,
    pub source_format: String,
    pub difficulty_slot: Option<u32>,
    pub authored_note_count: usize,
    pub template_count: usize,
    pub template_instance_count: usize,
    pub selected_note_ids: Vec<String>,
    /// Complete selected-note records, bounded by page and response byte limits.
    #[serde(default)]
    pub selected_notes: Vec<Note>,
    /// If true, query selected_note_ids with pagination for the remaining records.
    #[serde(default)]
    pub selection_notes_truncated: bool,
    pub has_simai_source: bool,
    pub unapplied_draft: bool,
    pub draft_conflict: bool,
    pub simai_matches_chart: bool,
    pub core_available: bool,
    pub core_current: bool,
    pub auto_save: bool,
    pub dirty: bool,
    pub editor_busy: bool,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BpmChange {
    pub measure: f64,
    pub bpm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub id: String,
    pub measure: f64,
    pub note_type: NoteKind,
    pub ring_lane: Option<u8>,
    pub touch_zone: Option<String>,
    pub hold_duration_measures: f64,
    pub hi_speed: f64,
    pub is_break: bool,
    pub is_ex: bool,
    pub is_star: bool,
    pub is_fake_rotate: bool,
    pub is_hanabi: bool,
    pub is_tapless: bool,
    pub is_touch: bool,
    pub is_each: bool,
    pub is_each_head: bool,
    pub slides: Vec<Slide>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub delay_measures: f64,
    pub motion_duration_measures: f64,
    pub is_break: bool,
    pub segments: Vec<SlideSegment>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlideSegment {
    pub shape: String,
    pub points: Vec<SlidePoint>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlidePoint {
    pub zone: String,
    /// Legacy stored beat_offset; unused by playback. No timing conversion is implied.
    pub stored_beat_offset: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotesPage {
    pub target: SnapshotRef,
    pub notes: Vec<Note>,
    pub total: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimaiSlice {
    pub target: SnapshotRef,
    pub text: String,
    pub range: CharacterRange,
    pub start: SourcePosition,
    pub end: SourcePosition,
    pub total_chars: usize,
    pub unapplied_draft: bool,
    pub matches_chart: bool,
    pub mapping_note: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub rule_id: String,
    pub severity: String,
    pub note_id: Option<String>,
    pub measure: Option<f64>,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckReport {
    pub target: SnapshotRef,
    pub diagnostics: Vec<Diagnostic>,
    pub total: usize,
    pub next_offset: Option<usize>,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusResult {
    pub target: SnapshotRef,
    pub range: MeasureRange,
}
