//! A single bounded run, independent of HTTP and editor UI.
mod environment;
pub mod cursor;
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
pub use genai::chat::{ChatRequest, ChatRole, ContentPart, MessageContent, ToolCall, ToolResponse};
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
enum Mode {
    Conversation,
    Prefetch,
    Cursor(cursor::Context),
}
impl From<bool> for Mode {
    fn from(prefetch: bool) -> Self {
        if prefetch { Self::Prefetch } else { Self::Conversation }
    }
}
impl Run {
    pub fn start(
        runtime: &tokio::runtime::Handle,
        provider: impl Provider + 'static,
        client: Client,
        messages: Vec<ChatMessage>,
        permissions: Permissions,
    ) -> Self {
        Self::start_inner(runtime, provider, client.scoped(), messages, permissions, Mode::Conversation)
    }
    /// Capture fresh editor context through the same scoped host bridge before
    /// the first model request. No model/tool capability is reduced.
    pub fn start_with_context(
        runtime: &tokio::runtime::Handle,
        provider: impl Provider + 'static,
        client: Client,
        messages: Vec<ChatMessage>,
        permissions: Permissions,
    ) -> Self {
        Self::start_inner(runtime, provider, client.scoped(), messages, permissions, Mode::Prefetch)
    }
    /// The editor supplies a scoped client with a cursor insertion grant.
    pub fn start_at_cursor(
        runtime: &tokio::runtime::Handle,
        provider: impl Provider + 'static,
        client: Client,
        messages: Vec<ChatMessage>,
        permissions: Permissions,
        context: cursor::Context,
    ) -> Self {
        Self::start_inner(runtime, provider, client, messages, permissions, Mode::Cursor(context))
    }
    fn start_inner(
        runtime: &tokio::runtime::Handle,
        provider: impl Provider + 'static,
        client: Client,
        messages: Vec<ChatMessage>,
        permissions: Permissions,
        mode: Mode,
    ) -> Self {
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
                let report = run_inner(
                    provider,
                    client.clone(),
                    messages,
                    permissions,
                    Budget::default(),
                    stop,
                    &emit,
                    mode,
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
    provider: impl Provider,
    client: Client,
    messages: Vec<ChatMessage>,
    permissions: Permissions,
    budget: Budget,
    stop: CancellationToken,
    emit: &Emit<'_>,
) -> Report {
    run_inner(
        provider,
        client,
        messages,
        permissions,
        budget,
        stop,
        emit,
        false,
    )
    .await
}
async fn run_inner(
    mut provider: impl Provider,
    client: Client,
    mut messages: Vec<ChatMessage>,
    permissions: Permissions,
    budget: Budget,
    stop: CancellationToken,
    emit: &Emit<'_>,
    mode: impl Into<Mode>,
) -> Report {
    let mode = mode.into();
    let prefetch = matches!(&mode, Mode::Prefetch);
    let deadline = Instant::now() + budget.total.min(Duration::from_secs(120));
    let result = async {
        check_context_size(&messages)?;
        if let Mode::Cursor(context) = &mode {
            messages.push(ChatMessage::user(
                serde_json::json!({"cursor_context":context}).to_string(),
            ));
        }
        if prefetch {
            if budget.tools == 0 || Instant::now() >= deadline {
                return Err(exhausted());
            }
            emit(Event::State(State::ExecutingTools))?;
            let result = execute(
                &client,
                "chart_get_context",
                serde_json::json!({"schema_version":SCHEMA_VERSION}),
                permissions,
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_secs(30)),
                &stop,
            )
            .await;
            emit(Event::Tool {
                id: "editor-context".into(),
                name: "chart_get_context".into(),
                result: result.clone(),
            })?;
            let context = result?;
            messages.push(ChatMessage::user(
                serde_json::json!({"editor_context":context}).to_string(),
            ));
        }
        drive(
            &mut provider,
            &client,
            &mut messages,
            permissions,
            budget,
            &stop,
            emit,
            deadline,
            usize::from(prefetch),
            &mode,
        )
        .await
    }
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
    deadline: Instant,
    mut calls: usize,
    mode: &Mode,
) -> ToolResult<State> {
    let mut seen = HashSet::new();
    for _ in 0..budget.rounds.min(8) {
        if stop.is_cancelled() || !client.session().active() {
            return Err(cancelled());
        }
        check_context_size(messages)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(exhausted());
        }
        emit(Event::State(State::Requesting))?;
        let cursor_mode = matches!(mode, Mode::Cursor(_));
        let request = ChatRequest::new(messages.clone())
            .with_system(if cursor_mode { policy::CURSOR_SYSTEM } else { policy::SYSTEM })
            .with_tools(if cursor_mode { policy::cursor_tools(permissions) } else { policy::tools(permissions) });
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
                    "An edit has been submitted to the editor; remaining calls skipped",
                ))
            } else if call.call_id.is_empty()
                || call.call_id.len() > 256
                || !seen.insert(call.call_id.clone())
            {
                Err(ToolError::invalid(
                    "Missing, oversized or duplicate tool call ID",
                ))
            } else {
                execute_in_mode(
                    mode,
                    client,
                    &call.fn_name,
                    call.fn_arguments.clone(),
                    permissions,
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(30)),
                    stop,
                )
                .await
            };
            if matches!(call.fn_name.as_str(), "chart_propose_edit" | cursor::TOOL) && result.is_ok() {
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
async fn execute_in_mode(
    mode: &Mode,
    client: &Client,
    name: &str,
    arguments: serde_json::Value,
    permissions: Permissions,
    timeout: Duration,
    stop: &CancellationToken,
) -> ToolResult<serde_json::Value> {
    if let Mode::Cursor(context) = mode {
        match name {
            cursor::TOOL => {
                let request = cursor::proposal(arguments, context)?;
                return execute(client, "chart_propose_edit", serde_json::to_value(request).unwrap(), permissions, timeout, stop).await;
            }
            "library_search" | "library_read" => {}
            _ => return Err(ToolError::new(ErrorCode::PermissionDenied, "Cursor mode only allows library definitions and cursor_insert_notes")),
        }
    }
    execute(client, name, arguments, permissions, timeout, stop).await
}
fn check_context_size(messages: &[ChatMessage]) -> ToolResult<()> {
    if messages.iter().map(ChatMessage::size).sum::<usize>() > MAX_CONTEXT_BYTES {
        return Err(ToolError::invalid(
            "Conversation exceeds 256 KiB; clear the session or narrow the task",
        ));
    }
    Ok(())
}
async fn execute(
    client: &Client,
    name: &str,
    arguments: serde_json::Value,
    permissions: Permissions,
    timeout: Duration,
    stop: &CancellationToken,
) -> ToolResult<serde_json::Value> {
    if stop.is_cancelled() || !client.session().active() {
        return Err(cancelled());
    }
    if timeout.is_zero() {
        return Err(exhausted());
    }
    let pending = client.call(name, arguments, permissions, timeout)?;
    loop {
        if stop.is_cancelled() || !client.session().active() {
            pending.cancel();
            return Err(cancelled());
        }
        if let Some(result) = pending.poll() {
            return result;
        }
        tokio::select! {
            biased;
            _ = stop.cancelled() => { pending.cancel(); return Err(cancelled()); },
            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
        }
    }
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
