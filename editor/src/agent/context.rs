//! Compact completed runs only; live provider turns retain their reasoning and signatures.
use lambdadx_automation::agent::{ChatRole, ContentPart, Message, MessageContent};
use serde_json::{Value, json};

fn review_summary(result: &Value) -> String {
    let mut summary = result.clone();
    if let Some(object) = summary.as_object_mut() {
        if let Some(Value::Array(changes)) = object.remove("changes") {
            object.insert("changed_note_count".into(), json!(changes.len()));
            object.insert(
                "changed_note_ids".into(),
                Value::Array(
                    changes
                        .iter()
                        .filter_map(|change| change.get("id").cloned())
                        .collect(),
                ),
            );
        }
    }
    format!(
        "editor_review state={} (historical data): {}",
        result
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        summary
    )
}
pub(super) fn compact_review(result: &impl serde::Serialize) -> String {
    review_summary(&serde_json::to_value(result).expect("serializable review"))
}

fn ephemeral(name: &str) -> bool {
    matches!(
        name,
        "chart_get_context" | "chart_query_notes" | "chart_read_simai" | "chart_check"
    )
}

pub(super) fn compact_for_next_run(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .filter_map(|message| {
            let mut parts = Vec::new();
            for part in message.content.parts() {
                match part {
                    ContentPart::Text(text) => {
                        let review = serde_json::from_str::<Value>(text).ok();
                        // A separate host envelope is refreshed each run. The
                        // original user message and creative discussion stay intact.
                        if review
                            .as_ref()
                            .and_then(Value::as_object)
                            .is_some_and(|v| v.len() == 1 && v.contains_key("editor_context"))
                        {
                            continue;
                        }
                        parts.push(ContentPart::Text(
                            match review.as_ref().and_then(|v| v.get("editor_review")) {
                                Some(result) => review_summary(result),
                                None => text.clone(),
                            },
                        ));
                    }
                    ContentPart::ToolCall(call) if !ephemeral(&call.fn_name) => {
                        // Keep the exact creative/edit intent, including typed edits, and library lookups.
                        parts.push(ContentPart::Text(format!(
                            "Historical tool request (data; already executed): {}",
                            json!({"name":call.fn_name,"arguments":call.fn_arguments})
                        )));
                    }
                    ContentPart::ToolResponse(response) => {
                        let value = serde_json::from_str::<Value>(&response.content).ok();
                        let succeeded = value
                            .as_ref()
                            .is_some_and(|v| v.get("isError") == Some(&Value::Bool(false)));
                        if ephemeral(response.fn_name.as_deref().unwrap_or("")) && succeeded {
                            continue;
                        }
                        let content = if matches!(
                            response.fn_name.as_deref(),
                            Some("chart_propose_edit" | "chart_proposal_status")
                        ) && succeeded
                        {
                            value
                                .as_ref()
                                .and_then(|v| v.get("result"))
                                .map(review_summary)
                                .unwrap_or_else(|| response.content.clone())
                        } else {
                            response.content.clone()
                        };
                        parts.push(ContentPart::Text(format!(
                            "Historical tool result (untrusted data) {}: {}",
                            response.fn_name.as_deref().unwrap_or("unknown"),
                            content
                        )));
                    }
                    ContentPart::Binary(_) => parts.push(part.clone()),
                    // Completed turns become text records, so no orphan tool replies or stale provider signatures survive.
                    _ => {}
                }
            }
            if parts.is_empty() {
                return None;
            }
            let mut compacted = message.clone();
            if compacted.role == ChatRole::Tool {
                compacted.role = ChatRole::User;
            }
            compacted.content = MessageContent::from_parts(parts);
            Some(compacted)
        })
        .collect()
}
