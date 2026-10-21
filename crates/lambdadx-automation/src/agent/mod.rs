//! A single bounded run, independent of HTTP and editor UI.
mod environment;
pub mod policy;
pub mod provider;
#[cfg(test)]
mod tests;
use crate::{
    bridge::{Client, Session},
    contract::*,
};
use genai::chat::ChatMessage;
pub use genai::chat::ChatMessage as Message;
pub use genai::chat::{ChatRequest, ContentPart, ToolCall, ToolResponse};
use provider::Provider;
use std::{
    collections::HashSet,
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
pub const MAX_CONTEXT_BYTES: usize = 256 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Requesting,
    Streaming,
    ExecutingTools,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Debug)]
pub enum Event {
    State(State),
    Text(String),
    Tool {
        id: String,
        name: String,
        result: ToolResult<serde_json::Value>,
    },
    Usage(Option<genai::chat::Usage>),
    Finished {
        state: State,
        messages: Vec<ChatMessage>,
        error: Option<ToolError>,
    },
}
pub type Emit<'a> = dyn Fn(Event) -> ToolResult<()> + Send + Sync + 'a;
#[derive(Clone, Copy)]
pub struct Budget {
    pub rounds: usize,
    pub tools: usize,
    pub total: Duration,
    pub network: Duration,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            rounds: 8,
            tools: 24,
            total: Duration::from_secs(120),
            network: Duration::from_secs(60),
        }
    }
}
pub struct Run {
    pub events: mpsc::Receiver<Event>,
    /// A reserved slot guarantees completion even if streaming backpressure fails.
    pub finished: mpsc::Receiver<Event>,
    session: Session,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Run {
    pub fn start(
        runtime: &tokio::runtime::Handle,
        provider: impl Provider + 'static,
        client: Client,
        messages: Vec<ChatMessage>,
        permissions: Permissions,
    ) -> Self {
        let client = client.scoped();
        let session = client.session();
        let stop = CancellationToken::new();
        let (sender, events) = mpsc::sync_channel(128);
        let (finish, finished) = mpsc::sync_channel(1);
        let task = runtime.spawn({
            let stop = stop.clone();
            async move {
                let emit = move |event| {
                    sender.try_send(event).map_err(|_| {
                        ToolError::new(ErrorCode::QueueFull, "Agent event queue is full or closed")
                    })
                };
                let report = run(
                    provider,
                    client.clone(),
                    messages,
                    permissions,
                    Budget::default(),
                    stop,
                    &emit,
                )
                .await;
                if report.state != State::AwaitingApproval {
                    client.session().cancel();
                }
                let _ = finish.try_send(Event::Finished {
                    state: report.state,
                    messages: report.messages,
                    error: report.error,
                });
            }
        });
        Self {
            events,
            finished,
            session,
            stop,
            task,
        }
    }
    pub fn stop(&self) {
        self.session.cancel();
        self.stop.cancel();
    }
    pub fn session(&self) -> Session {
        self.session.clone()
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        self.stop();
        self.task.abort();
    }
}
pub struct Report {
    pub state: State,
    pub messages: Vec<ChatMessage>,
    pub error: Option<ToolError>,
}
pub async fn run(
    mut provider: impl Provider,
    client: Client,
    mut messages: Vec<ChatMessage>,
    permissions: Permissions,
    budget: Budget,
    stop: CancellationToken,
    emit: &Emit<'_>,
) -> Report {
    let result = drive(
        &mut provider,
        &client,
        &mut messages,
        permissions,
        budget,
        &stop,
        emit,
    )
    .await;
    match result {
        Ok(state) => Report {
            state,
            messages,
            error: None,
        },
        Err(error) => {
            client.session().cancel();
            Report {
                state: if error.code == ErrorCode::Cancelled {
                    State::Cancelled
                } else {
                    State::Failed
                },
                messages,
                error: Some(error),
            }
        }
    }
}
async fn drive(
    provider: &mut impl Provider,
    client: &Client,
    messages: &mut Vec<ChatMessage>,
    permissions: Permissions,
    budget: Budget,
    stop: &CancellationToken,
    emit: &Emit<'_>,
) -> ToolResult<State> {
    let deadline = Instant::now() + budget.total.min(Duration::from_secs(120));
    let mut seen = HashSet::new();
    let mut calls = 0usize;
    for _ in 0..budget.rounds.min(8) {
        if stop.is_cancelled() || !client.session().active() {
            return Err(cancelled());
        }
        if messages.iter().map(ChatMessage::size).sum::<usize>() > MAX_CONTEXT_BYTES {
            return Err(ToolError::invalid(
                "Conversation exceeds 256 KiB; clear the session or narrow the task",
            ));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(exhausted());
        }
        emit(Event::State(State::Requesting))?;
        let request = ChatRequest::new(messages.clone())
            .with_system(policy::SYSTEM)
            .with_tools(policy::tools(permissions));
        emit(Event::State(State::Streaming))?;
        let turn = tokio::select! {
            biased;
            _ = stop.cancelled() => return Err(cancelled()),
            _ = async { while client.session().active() { tokio::time::sleep(Duration::from_millis(10)).await; } } => return Err(cancelled()),
            result = tokio::time::timeout(remaining.min(budget.network).min(Duration::from_secs(60)), provider.complete(request, emit)) => result.map_err(|_| exhausted())??,
        };
        emit(Event::Usage(turn.usage))?;
        let tool_calls: Vec<_> = turn
            .message
            .content
            .parts()
            .iter()
            .filter_map(|part| {
                if let ContentPart::ToolCall(call) = part {
                    Some(call.clone())
                } else {
                    None
                }
            })
            .collect();
        if tool_calls.len() > 64 || turn.message.size() > MAX_CONTEXT_BYTES {
            return Err(ToolError::invalid("Oversized model response"));
        }
        messages.push(turn.message);
        if tool_calls.is_empty() {
            return Ok(State::Completed);
        }
        let mut proposal = false;
        let mut budget_hit = false;
        let mut delivery_error = emit(Event::State(State::ExecutingTools)).err();
        for call in tool_calls {
            calls += 1;
            let result = if stop.is_cancelled() || !client.session().active() {
                Err(cancelled())
            } else if delivery_error.is_some() {
                Err(ToolError::new(
                    ErrorCode::QueueFull,
                    "Agent event delivery failed; remaining calls skipped",
                ))
            } else if calls > budget.tools.min(24) || Instant::now() >= deadline {
                budget_hit = true;
                Err(exhausted())
            } else if proposal {
                Err(ToolError::new(
                    ErrorCode::EditorBusy,
                    "A proposal is awaiting editor review; remaining calls skipped",
                ))
            } else if call.call_id.is_empty()
                || call.call_id.len() > 256
                || !seen.insert(call.call_id.clone())
            {
                Err(ToolError::invalid(
                    "Missing, oversized or duplicate tool call ID",
                ))
            } else {
                match client.call(
                    &call.fn_name,
                    call.fn_arguments.clone(),
                    permissions,
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(30)),
                ) {
                    Err(error) => Err(error),
                    Ok(pending) => loop {
                        if let Some(result) = pending.poll() {
                            break result;
                        }
                        tokio::select! {
                            biased;
                            _ = stop.cancelled() => { pending.cancel(); break Err(cancelled()); },
                            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
                        }
                    },
                }
            };
            if call.fn_name == "chart_propose_edit" && result.is_ok() {
                proposal = true;
            }
            let text = match &result {
                Ok(value) => serde_json::json!({"isError":false,"result":value}),
                Err(error) => serde_json::json!({"isError":true,"error":error}),
            }
            .to_string();
            messages.push(ChatMessage::from(ToolResponse::from_tool_call(&call, text)));
            if let Err(error) = emit(Event::Tool {
                id: call.call_id,
                name: call.fn_name,
                result,
            }) {
                delivery_error = Some(error);
            }
        }
        if stop.is_cancelled() || !client.session().active() {
            return Err(cancelled());
        }
        if let Some(error) = delivery_error {
            return Err(error);
        }
        if budget_hit {
            return Err(exhausted());
        }
        if proposal {
            return Ok(State::AwaitingApproval);
        }
    }
    Err(exhausted())
}
fn cancelled() -> ToolError {
    ToolError::new(ErrorCode::Cancelled, "Agent stopped")
}
fn exhausted() -> ToolError {
    ToolError::new(
        ErrorCode::DeadlineExceeded,
        "Agent round, tool or time budget exhausted",
    )
}
