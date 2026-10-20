//! One directory for adapter discovery, schemas and strict argument decoding.
use crate::contract::*;
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::OnceLock;

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub permission: Permission,
    pub input_schema: Value,
    pub output_schema: Value,
    decode: fn(Value) -> ToolResult<Request>,
}
fn decode<T: DeserializeOwned>(args: Value) -> ToolResult<T> {
    serde_json::from_value(args).map_err(|error| ToolError::invalid(error.to_string()))
}
fn tool<I: JsonSchema, O: JsonSchema>(
    name: &'static str,
    description: &'static str,
    permission: Permission,
    decode: fn(Value) -> ToolResult<Request>,
) -> Tool {
    Tool {
        name,
        description,
        permission,
        input_schema: schema_for!(I).to_value(),
        output_schema: schema_for!(O).to_value(),
        decode,
    }
}
pub fn tools() -> &'static [Tool] {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS.get_or_init(|| vec![
        tool::<ContextRequest, Context>("chart_get_context", "Capture the current authored chart and Simai draft as an immutable snapshot. Measures start at 1; durations are in measures. Template expansion is excluded.", Permission::Read, |v| decode(v).map(Request::Context)),
        tool::<QueryNotesRequest, NotesPage>("chart_query_notes", "Read authored notes from a snapshot, sorted by measure then numeric note ID. Ranges are half-open; paginate with the same target and filters.", Permission::Read, |v| decode(v).map(Request::QueryNotes)),
        tool::<ReadSimaiRequest, SimaiSlice>("chart_read_simai", "Read the exact current Simai body, including an unapplied draft. Ranges use zero-based Unicode scalar indices; returned line/column coordinates are one-based. No parsing or normalization.", Permission::Read, |v| decode(v).map(Request::ReadSimai)),
        tool::<CheckRequest, CheckReport>("chart_check", "Deterministic structural diagnostics with pagination. Core checks are not yet supported. This does not assess playability or hand movement.", Permission::Read, |v| decode(v).map(Request::Check)),
        tool::<FocusRequest, FocusResult>("editor_focus_range", "Focus an explicit range in the live document if it still matches the snapshot. Requires separate focus permission; does not seek playback or select notes.", Permission::Focus, |v| decode(v).map(Request::Focus)),
    ])
}
pub fn parse(name: &str, arguments: Value) -> ToolResult<Request> {
    if serde_json::to_vec(&arguments)
        .map_err(|e| ToolError::invalid(e.to_string()))?
        .len()
        > MAX_INPUT_BYTES
    {
        return Err(ToolError::invalid("Tool input exceeds 256 KiB"));
    }
    let descriptor = tools()
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| ToolError::new(ErrorCode::UnsupportedOperation, "Unknown tool"))?;
    let request = (descriptor.decode)(arguments)?;
    request.validate()?;
    Ok(request)
}
