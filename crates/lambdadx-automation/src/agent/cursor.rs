//! Agent-only insertion protocol. The editor pins the destination before the run.
use crate::contract::*;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Context {
    pub target: SnapshotRef,
    pub measure: f64,
    pub ring_lane: u8,
    pub touch_zone: Option<String>,
    pub step_measures: f64,
    pub bpm: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InsertRequest {
    pub operation_id: String,
    pub summary: String,
    pub notes: Vec<CursorNote>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CursorNote {
    pub note_type: NoteKind,
    /// Time relative to the pinned cursor, in measures. The first head is at 0.
    pub offset_measures: f64,
    /// Absolute lane 1..8; defaults to the cursor lane for ring notes.
    pub ring_lane: Option<u8>,
    /// Absolute zone; defaults to the cursor touch zone for touch notes.
    pub touch_zone: Option<String>,
    #[serde(default)]
    pub properties: NoteProperties,
}

pub const TOOL: &str = "cursor_insert_notes";
pub fn tool() -> genai::chat::Tool {
    genai::chat::Tool::new(TOOL)
        .with_description("Insert 1..256 notes at the pinned editor cursor. Read the requested library definition, then call once with the whole pattern. Times are nonnegative offsets from the cursor, with the first head at 0. The editor validates and displays a playable ghost preview for user acceptance. On acceptance it applies atomically, selects the new notes and advances the cursor. No chart or Simai read is needed.")
        .with_schema(serde_json::to_value(schema_for!(InsertRequest)).unwrap())
}

pub fn proposal(arguments: serde_json::Value, context: &Context) -> ToolResult<ProposeRequest> {
    let request: InsertRequest = serde_json::from_value(arguments)
        .map_err(|e| ToolError::invalid(e.to_string()))?;
    if !(1..=MAX_IDS).contains(&request.notes.len())
        || !request.notes.iter().any(|n| n.offset_measures == 0.0)
    {
        return Err(ToolError::invalid("Insert 1..256 notes with the first head at offset 0"));
    }
    let notes = request.notes.into_iter().map(|n| {
        if !n.offset_measures.is_finite() || n.offset_measures < 0.0 {
            return Err(ToolError::invalid("Offsets must be finite and nonnegative"));
        }
        let measure = context.measure + n.offset_measures;
        number(measure)?;
        let touch = matches!(n.note_type, NoteKind::Touch | NoteKind::TouchHold);
        Ok(NewNote {
            note_type: n.note_type,
            measure,
            ring_lane: n.ring_lane.or((!touch).then_some(context.ring_lane)),
            touch_zone: n.touch_zone.or_else(|| touch.then(|| context.touch_zone.clone()).flatten()),
            properties: n.properties,
        })
    }).collect::<ToolResult<Vec<_>>>()?;
    let result = ProposeRequest {
        schema_version: SCHEMA_VERSION,
        target: context.target.clone(),
        operation_id: request.operation_id,
        summary: request.summary,
        edits: vec![Edit::AddNotes { notes }],
    };
    result.validate()?;
    Ok(result)
}
