//! Writable fields deliberately exclude IDs and all derived visual flags.
use crate::contract::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeRequest {
    pub schema_version: u32,
    pub target: SnapshotRef,
    pub operation_id: String,
    pub summary: String,
    pub edits: Vec<Edit>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    TransformNotes { note_ids: Vec<String>, delta_measures: f64, lane_rotation: i32 },
    DeleteNotes { note_ids: Vec<String> },
    AddNotes { notes: Vec<NewNote> },
    UpdateNotes { notes: Vec<NoteUpdate> },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewNote {
    pub note_type: NoteKind,
    pub measure: f64,
    pub ring_lane: Option<u8>,
    pub touch_zone: Option<String>,
    #[serde(default)] pub properties: NoteProperties,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoteUpdate {
    pub id: String,
    pub measure: Option<f64>,
    pub ring_lane: Option<u8>,
    pub touch_zone: Option<String>,
    #[serde(default)] pub properties: NoteProperties,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoteProperties {
    pub hold_duration_measures: Option<f64>,
    pub hi_speed: Option<f64>,
    pub is_break: Option<bool>,
    pub is_ex: Option<bool>,
    pub is_star: Option<bool>,
    pub is_fake_rotate: Option<bool>,
    pub is_hanabi: Option<bool>,
    pub is_tapless: Option<bool>,
    pub slides: Option<Vec<WritableSlide>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WritableSlide {
    pub delay_measures: f64,
    pub motion_duration_measures: f64,
    pub is_break: bool,
    pub segments: Vec<SlideSegment>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalStatusRequest { pub schema_version: u32, pub proposal_id: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState { Preparing, AwaitingApproval, Applied, Rejected, Expired, Cancelled, Failed }
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoteDiff { pub id: String, pub before: Option<Note>, pub after: Option<Note> }
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalResult {
    pub proposal_id: String,
    pub target: SnapshotRef,
    pub state: ProposalState,
    pub summary: String,
    pub changes: Vec<NoteDiff>,
    pub warnings: Vec<String>,
    pub error: Option<ToolError>,
    pub applied_target: Option<SnapshotRef>,
}
pub fn note_id(value: &str) -> ToolResult<u64> {
    let id = value.parse::<u64>().map_err(|_| ToolError::invalid("Invalid decimal note ID"))?;
    if id.to_string() != value { return Err(ToolError::invalid("Note IDs must be canonical decimal strings")); }
    Ok(id)
}
pub fn number(value: f64) -> ToolResult<f32> {
    let stored = value as f32;
    if !value.is_finite() || !stored.is_finite() || (value != 0.0 && stored == 0.0) {
        return Err(ToolError::invalid("Number cannot be represented as a finite f32"));
    }
    Ok(stored)
}
impl ProposeRequest {
    pub fn validate(&self) -> ToolResult<()> {
        if self.operation_id.is_empty() || self.operation_id.len() > 128 || self.summary.trim().is_empty() || self.summary.len() > 2048 || self.edits.is_empty() || self.edits.len() > MAX_IDS {
            return Err(ToolError::invalid("Missing or oversized operation ID, summary or edits"));
        }
        let mut count = 0;
        for edit in &self.edits {
            match edit {
                Edit::TransformNotes { note_ids, delta_measures, lane_rotation } => {
                    number(*delta_measures)?;
                    if !(-7..=7).contains(lane_rotation) { return Err(ToolError::invalid("lane_rotation must be -7..7")); }
                    for id in note_ids { note_id(id)?; }
                    if note_ids.is_empty() { return Err(ToolError::invalid("Explicit IDs required")); }
                    count += note_ids.len();
                }
                Edit::DeleteNotes { note_ids } => {
                    for id in note_ids { note_id(id)?; }
                    if note_ids.is_empty() { return Err(ToolError::invalid("Explicit IDs required")); }
                    count += note_ids.len();
                }
                Edit::UpdateNotes { notes } => { for n in notes { note_id(&n.id)?; } count += notes.len(); }
                Edit::AddNotes { notes } => count += notes.len(),
            }
        }
        if !(1..=MAX_IDS).contains(&count) { return Err(ToolError::invalid("A proposal must affect 1..256 notes")); }
        Ok(())
    }
}
