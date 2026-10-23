//! One provider completion, followed by local validation and proposal creation.
use super::*;
use crate::library::ResolvedLibrary;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Output {
    summary: String,
    #[schemars(length(min = 1, max = 256))]
    notes: Vec<cursor::CursorNote>,
}

#[derive(Serialize)]
struct GenerationRequest<'a> {
    request_id: String,
    cursor_context: &'a cursor::Context,
    library: ResolvedLibrary,
    output_schema: Value,
}

fn library_request(messages: &[ChatMessage]) -> ToolResult<LibraryResolveRequest> {
    // Only original user prompts, never host envelopes, tool output or assistant JSON.
    let mut prompts = messages
        .iter()
        .rev()
        .filter(|m| m.role == ChatRole::User)
        .flat_map(|m| {
            m.content.parts().iter().filter_map(|p| {
                let ContentPart::Text(text) = p else {
                    return None;
                };
                let host = text.starts_with("Historical tool ")
                    || text.starts_with("editor_review ")
                    || serde_json::from_str::<Value>(text)
                        .ok()
                        .and_then(|v| v.as_object().cloned())
                        .is_some_and(|v| {
                            [
                                "editor_context",
                                "cursor_context",
                                "generation_request",
                                "editor_event",
                                "editor_review",
                                "editor_review_error",
                            ]
                            .iter()
                            .any(|k| v.contains_key(*k))
                        });
                (!host).then(|| text.clone())
            })
        });
    let prompt = prompts
        .next()
        .ok_or_else(|| ToolError::invalid("A generation prompt is required"))?;
    let mut remaining = (64 * 1024usize).saturating_sub(prompt.len());
    let previous_prompts = prompts
        .take(8)
        .take_while(|p| {
            if p.len() > remaining {
                return false;
            }
            remaining -= p.len();
            true
        })
        .collect();
    let request = LibraryResolveRequest {
        schema_version: SCHEMA_VERSION,
        prompt,
        previous_prompts,
    };
    request.validate()?;
    Ok(request)
}

pub(super) async fn generate(
    provider: &mut impl Provider,
    client: &Client,
    messages: &mut Vec<ChatMessage>,
    permissions: Permissions,
    budget: Budget,
    stop: &CancellationToken,
    emit: &Emit<'_>,
    deadline: Instant,
    context: &cursor::Context,
) -> ToolResult<State> {
    if !permissions.read || !permissions.propose {
        return Err(ToolError::new(
            ErrorCode::PermissionDenied,
            "Direct generation requires read and proposal permissions",
        ));
    }
    if budget.rounds == 0 || budget.tools < 2 {
        return Err(exhausted());
    }
    emit(Event::State(State::ExecutingTools))?;
    let library = execute(
        client,
        "library_resolve",
        serde_json::to_value(library_request(messages)?).unwrap(),
        permissions,
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(30)),
        stop,
    )
    .await?;
    let request = GenerationRequest {
        request_id: format!("cursor-{}", client.session().id()),
        cursor_context: context,
        library: serde_json::from_value(library).map_err(|e| ToolError::invalid(e.to_string()))?,
        output_schema: schema_for!(Output).to_value(),
    };
    // The envelope belongs only to this request; do not accumulate definitions in history.
    let mut outgoing = messages.clone();
    outgoing.push(ChatMessage::user(
        json!({"generation_request":request}).to_string(),
    ));
    check_context_size(&outgoing)?;
    let chat = ChatRequest::new(outgoing).with_system(policy::CURSOR_SYSTEM);
    // Structured output is displayed as a summary only after validation. Suppressing
    // token events also prevents a fast JSON stream from filling the UI event queue.
    let quiet = |event| match event {
        Event::Text(_) => Ok(()),
        other => emit(other),
    };
    let turn = complete_turn(provider, client, chat, budget, stop, &quiet, deadline).await?;
    let mut text = String::new();
    for part in turn.message.content.parts() {
        match part {
            ContentPart::Text(t) => text.push_str(t),
            _ => {
                return Err(ToolError::invalid(
                    "Direct generation must return JSON content, without tool calls",
                ));
            }
        }
    }
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .and_then(|t| t.strip_suffix("```"))
        .unwrap_or(text)
        .trim();
    let output: Output = serde_json::from_str(text).map_err(|e| {
        ToolError::invalid(format!("Invalid generation JSON: {e}; no preview created"))
    })?;
    let summary = format!(
        "{}（{} 个音符，虚影待接受）",
        output.summary,
        output.notes.len()
    );
    let proposal = cursor::proposal_from_request(
        cursor::InsertRequest {
            operation_id: request.request_id.clone(),
            summary: output.summary,
            notes: output.notes,
        },
        context,
    )?;
    emit(Event::State(State::ExecutingTools))?;
    let result = execute(
        client,
        "chart_propose_edit",
        serde_json::to_value(proposal).unwrap(),
        permissions,
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(30)),
        stop,
    )
    .await;
    emit(Event::Tool {
        id: request.request_id,
        name: cursor::TOOL.into(),
        result: result.clone(),
    })?;
    result?;
    messages.push(ChatMessage::assistant(text.to_owned()));
    emit(Event::Text(summary))?;
    Ok(State::AwaitingApproval)
}
