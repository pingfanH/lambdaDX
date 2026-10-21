use super::{Emit, Event};
use crate::contract::*;
use futures_util::StreamExt;
use genai::{
    ModelIden, ModelSpec, ServiceTarget,
    adapter::AdapterKind,
    chat::*,
    resolver::{AuthData, Endpoint},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProviderKind {
    #[default]
    OpenAI,
    OpenAICompatible,
    Ollama,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub provider: ProviderKind,
    pub model: String,
    pub base_url: String,
    /// Only this named environment variable is resolved, at run start.
    pub credential_env: String,
    pub allow_focus: bool,
    pub allow_proposals: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: ProviderKind::OpenAI,
            model: "gpt-4o-mini".into(),
            base_url: "https://api.openai.com/v1/".into(),
            credential_env: "OPENAI_API_KEY".into(),
            allow_focus: false,
            allow_proposals: false,
        }
    }
}
impl Settings {
    /// Overlay LDX_AGENT_* from the process environment and nearest .env.
    pub fn with_environment(self) -> ToolResult<Self> {
        super::environment::settings(self)
    }

    pub fn validate(&self) -> ToolResult<()> {
        if self.model.trim().is_empty() || self.model.len() > 256 {
            return Err(ToolError::invalid("Specify a model (at most 256 bytes)"));
        }
        let url = url::Url::parse(&self.base_url)
            .map_err(|_| ToolError::invalid("Invalid endpoint URL"))?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(url.scheme() == "https" || (url.scheme() == "http" && local))
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ToolError::invalid(
                "Endpoint must use HTTPS or loopback HTTP, with no credentials, query or fragment",
            ));
        }
        if self.credential_env.len() > 128
            || !self
                .credential_env
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || (self.credential_env.is_empty() && self.provider != ProviderKind::Ollama)
        {
            return Err(ToolError::invalid(
                "Specify a credential environment variable name",
            ));
        }
        Ok(())
    }
    pub fn permissions(&self) -> Permissions {
        Permissions {
            read: true,
            focus: self.allow_focus,
            propose: self.allow_proposals,
        }
    }
}
pub struct Turn {
    pub message: ChatMessage,
    pub usage: Option<Usage>,
}
pub type Completion<'a> = Pin<Box<dyn Future<Output = ToolResult<Turn>> + Send + 'a>>;
pub trait Provider: Send {
    fn complete<'a>(&'a mut self, request: ChatRequest, emit: &'a Emit<'a>) -> Completion<'a>;
}
fn provider_error(error: genai::Error) -> ToolError {
    let status = error.status().or_else(|| {
        if let genai::Error::WebStream { error, .. } = &error {
            error
                .downcast_ref::<genai::Error>()
                .and_then(genai::Error::status)
        } else {
            None
        }
    });
    // Never format the SDK error: it can contain request/response bodies.
    let detail = match status.map(|s| s.as_u16()) {
        Some(400 | 404 | 422) => {
            "Provider rejected model or tool schema; check model name and endpoint support".into()
        }
        Some(401 | 403) => {
            "Provider authentication or access denied; check the configured credential".into()
        }
        Some(429) => "Provider rate limit reached; retry explicitly later".into(),
        Some(code) => format!("Provider HTTP {code}; no incomplete tool calls executed"),
        None => {
            "Provider request or stream failed; check endpoint, model, credential and tool support"
                .into()
        }
    };
    ToolError::new(ErrorCode::CoreUnavailable, detail)
}
pub struct Genai {
    client: genai::Client,
    target: ModelSpec,
    openai: bool,
}
impl Genai {
    pub fn new(settings: &Settings) -> ToolResult<Self> {
        settings.validate()?;
        let key = if settings.credential_env.is_empty() {
            String::new()
        } else {
            super::environment::credential(&settings.credential_env)?
        };
        Self::with_key(settings, key)
    }
    fn with_key(settings: &Settings, key: String) -> ToolResult<Self> {
        settings.validate()?;
        let adapter = match settings.provider {
            ProviderKind::OpenAI | ProviderKind::OpenAICompatible => AdapterKind::OpenAI,
            ProviderKind::Ollama => AdapterKind::Ollama,
        };
        // Redirects and environment proxies cannot send user data elsewhere.
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|_| ToolError::invalid("Cannot create provider HTTP client"))?;
        let client = genai::Client::builder()
            .with_reqwest(http)
            .with_adapter_kind(adapter)
            .build()
            .map_err(|_| ToolError::invalid("Cannot create model adapter"))?;
        let mut endpoint = settings.base_url.clone();
        if !endpoint.ends_with('/') {
            endpoint.push('/');
        }
        let target = ModelSpec::Target(ServiceTarget {
            model: ModelIden::new(adapter, settings.model.clone()),
            endpoint: Endpoint::from_owned(endpoint),
            auth: AuthData::from_single(key),
        });
        Ok(Self {
            client,
            target,
            openai: adapter == AdapterKind::OpenAI,
        })
    }
}

// SSE envelopes repeat on every token. Budget transport separately from the
// decoded text/tool limits so metadata does not consume the content allowance.
const MAX_WIRE_BYTES: usize = 32 * 1024 * 1024;
const MAX_STREAM_FRAMES: usize = 65_536;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_TOOL_BYTES: usize = super::MAX_CONTEXT_BYTES;
const MAX_REASONING_BYTES: usize = 256 * 1024;

#[derive(Default)]
struct WireBudget {
    bytes: usize,
    frames: usize,
}
impl WireBudget {
    fn charge(&mut self, frame: RawFrameRef<'_>) -> ToolResult<()> {
        self.bytes = self.bytes.saturating_add(frame.data.len());
        self.frames = frame.index as usize + 1;
        if self.frames > MAX_STREAM_FRAMES {
            return Err(ToolError::invalid(format!(
                "Provider stream exceeds frame limit ({} > {} frames; {} bytes); no tools executed",
                self.frames, MAX_STREAM_FRAMES, self.bytes
            )));
        }
        if self.bytes > MAX_WIRE_BYTES {
            return Err(ToolError::invalid(format!(
                "Provider stream exceeds wire byte limit ({} > {} bytes across {} frames); no tools executed",
                self.bytes, MAX_WIRE_BYTES, self.frames
            )));
        }
        Ok(())
    }
}

/// genai 0.7's OpenAI streamer only captures the first call in a regular
/// delta array. Its public frame hook lets us assemble every call without
/// retaining raw frames, while genai still owns HTTP, text, usage and finish.
#[derive(Default)]
struct OpenAiCalls {
    calls: BTreeMap<u64, (String, String, String)>,
    wire: WireBudget,
    error: Option<ToolError>,
}
impl OpenAiCalls {
    fn capture(&mut self, frame: RawFrameRef<'_>) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = self.wire.charge(frame) {
            self.error = Some(error);
            return;
        }
        let Some(value) = frame.json() else {
            return;
        };
        let Some(calls) = value
            .pointer("/choices/0/delta/tool_calls")
            .and_then(|v| v.as_array())
        else {
            return;
        };
        for call in calls {
            let Some(index) = call
                .get("index")
                .and_then(|v| v.as_u64())
                .filter(|i| *i < 64)
            else {
                self.error = Some(ToolError::invalid("Invalid tool call stream index"));
                return;
            };
            let entry = self.calls.entry(index).or_default();
            for (field, dest) in [("id", &mut entry.0), ("function/name", &mut entry.1)] {
                if let Some(text) = call
                    .pointer(&format!("/{field}"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    if !dest.is_empty() && dest != text {
                        self.error =
                            Some(ToolError::invalid("Tool identity changed within a stream"));
                        return;
                    }
                    *dest = text.into();
                }
            }
            if let Some(text) = call.pointer("/function/arguments").and_then(|v| v.as_str()) {
                entry.2.push_str(text);
            }
        }
        if self
            .calls
            .values()
            .map(|(id, name, args)| id.len() + name.len() + args.len())
            .sum::<usize>()
            > MAX_TOOL_BYTES
        {
            self.error = Some(ToolError::invalid("Model tool stream exceeds 256 KiB"));
        }
    }
    fn take(&mut self) -> ToolResult<Vec<ToolCall>> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        Ok(std::mem::take(&mut self.calls)
            .into_values()
            .map(|(call_id, fn_name, args)| ToolCall {
                call_id,
                fn_name,
                fn_arguments: serde_json::from_str(&args)
                    .unwrap_or(serde_json::Value::String(args)),
                thought_signatures: None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    fn streamed(arguments: &str, finish: bool) -> ToolResult<Turn> {
        streamed_calls(
            vec![
                serde_json::json!([{"index":0,"id":"live-id","type":"function","function":{"name":"chart_get_context","arguments":arguments}}]),
            ],
            finish,
        )
    }
    fn streamed_calls(chunks: Vec<serde_json::Value>, finish: bool) -> ToolResult<Turn> {
        streamed_frames(
            chunks
                .into_iter()
                .map(|calls| {
                    serde_json::json!({
                        "choices":[{"index":0,"delta":{"tool_calls":calls},"finish_reason":null}]
                    })
                })
                .collect(),
            finish,
        )
    }
    fn streamed_frames(frames: Vec<serde_json::Value>, finish: bool) -> ToolResult<Turn> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/", listener.local_addr().unwrap());
        let chunks: String = frames
            .into_iter()
            .map(|frame| format!("data: {frame}\n\n"))
            .collect();
        let body = format!(
            "{chunks}{}data: [DONE]\n\n",
            if finish {
                "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n"
            } else {
                ""
            }
        );
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut received = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let count = socket.read(&mut buf).unwrap();
                assert_ne!(count, 0);
                received.extend_from_slice(&buf[..count]);
                if let Some(index) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&received[..index]).to_lowercase();
                    let length: usize = head
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if received.len() >= index + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&received);
            assert!(request.starts_with("POST /v1/chat/completions"));
            assert!(request.contains("chart_get_context"));
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let settings = Settings {
            base_url: endpoint,
            ..Settings::default()
        };
        let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
        let result = runtime.handle().block_on(async {
            Genai::with_key(&settings, "fixture-key".into())
                .unwrap()
                .complete(
                    ChatRequest::new(vec![ChatMessage::user("check")])
                        .with_tools(super::super::policy::tools(Permissions::READ_ONLY)),
                    &|_| Ok(()),
                )
                .await
        });
        server.join().unwrap();
        runtime.shutdown();
        result
    }
    #[test]
    fn real_genai_stream_assembles_complete_tool_arguments_and_ids() {
        let turn = streamed("{\"schema_version\":1}", true).unwrap();
        let ContentPart::ToolCall(call) = &turn.message.content.parts()[0] else {
            panic!()
        };
        assert_eq!(call.call_id, "live-id");
        assert_eq!(call.fn_arguments["schema_version"], 1);
        assert!(turn.usage.is_none());
    }
    #[test]
    fn done_without_finish_reason_is_not_executable() {
        assert!(streamed("{\"schema_version\":1}", false).is_err());
    }
    #[test]
    fn malformed_json_is_preserved_for_typed_tool_error() {
        let turn = streamed("{\"schema_version\":", true).unwrap();
        let ContentPart::ToolCall(call) = &turn.message.content.parts()[0] else {
            panic!()
        };
        assert!(crate::catalog::parse(&call.fn_name, call.fn_arguments.clone()).is_err());
    }
    #[test]
    fn parallel_fragmented_calls_keep_every_id_and_complete_arguments() {
        let turn = streamed_calls(vec![
            serde_json::json!([
                {"index":0,"id":"first","function":{"name":"chart_get_context","arguments":"{\"schema_"}},
                {"index":1,"id":"second","function":{"name":"chart_get_context","arguments":"{\"schema_"}}
            ]),
            serde_json::json!([
                {"index":0,"function":{"arguments":"version\":1}"}},
                {"index":1,"function":{"arguments":"version\":1}"}}
            ])
        ], true).unwrap();
        let calls: Vec<_> = turn
            .message
            .content
            .parts()
            .iter()
            .filter_map(|p| {
                if let ContentPart::ToolCall(c) = p {
                    Some(c)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].call_id, "first");
        assert_eq!(calls[1].call_id, "second");
        for call in calls {
            assert_eq!(call.fn_arguments["schema_version"], 1);
        }
    }

    #[test]
    fn small_fragmented_tool_response_with_metadata_does_not_exhaust_wire_budget() {
        let arguments =
            serde_json::json!({"schema_version":1,"summary":"x".repeat(5000)}).to_string();
        let frames: Vec<_> = arguments
            .chars()
            .enumerate()
            .map(|(index, ch)| {
                let mut call =
                    serde_json::json!({"index":0,"function":{"arguments":ch.to_string()}});
                if index == 0 {
                    call["id"] = "live-id".into();
                    call["type"] = "function".into();
                    call["function"]["name"] = "chart_get_context".into();
                }
                serde_json::json!({
                    "id":"chatcmpl-fixture-0123456789abcdef0123456789abcdef",
                    "object":"chat.completion.chunk",
                    "created":1792512000u64,
                    "model":"deepseek/deepseek-v4.1-flash",
                    "system_fingerprint":"fixture-fingerprint",
                    "choices":[{"index":0,"delta":{"tool_calls":[call]},"finish_reason":null}]
                })
            })
            .collect();
        assert!(arguments.len() < 8 * 1024);
        assert!(frames.iter().map(|v| v.to_string().len()).sum::<usize>() > 1024 * 1024);
        let turn = streamed_frames(frames, true).unwrap();
        let ContentPart::ToolCall(call) = &turn.message.content.parts()[0] else {
            panic!("expected complete tool call")
        };
        assert_eq!(call.fn_arguments.to_string(), arguments);
    }

    #[test]
    fn fragmented_reasoning_can_exceed_old_frame_limit_before_a_complete_tool_call() {
        let mut frames: Vec<_> = (0..17_000)
            .map(|_| {
                serde_json::json!({
                    "id":"reasoning-fixture",
                    "model":"deepseek/deepseek-v4.1-flash",
                    "choices":[{"index":0,"delta":{"reasoning_content":"想"},"finish_reason":null}]
                })
            })
            .collect();
        frames.push(serde_json::json!({
            "choices":[{"index":0,"delta":{"tool_calls":[{
                "index":0,"id":"after-reasoning","type":"function",
                "function":{"name":"chart_get_context","arguments":"{\"schema_version\":1}"}
            }]},"finish_reason":null}]
        }));
        let turn = streamed_frames(frames, true).unwrap();
        let ContentPart::ToolCall(call) = &turn.message.content.parts()[0] else {
            panic!("expected complete tool call")
        };
        assert_eq!(call.call_id, "after-reasoning");
        assert_eq!(call.fn_arguments["schema_version"], 1);
    }

    #[test]
    fn excessive_reasoning_still_fails_before_returning_executable_calls() {
        let frames = vec![serde_json::json!({
            "choices":[{"index":0,"delta":{"reasoning_content":"x".repeat(MAX_REASONING_BYTES+1)},"finish_reason":null}]
        })];
        let error = streamed_frames(frames, true)
            .err()
            .expect("must reject oversized reasoning");
        assert!(
            error.message.contains("Model reasoning exceeds 256 KiB"),
            "{error:?}"
        );
    }

    #[test]
    fn larger_wire_allowance_preserves_text_and_tool_content_limits() {
        let error = streamed(&"x".repeat(MAX_TOOL_BYTES + 1), true)
            .err()
            .expect("must reject oversized tool arguments");
        assert!(
            error.message.contains("Model tool stream exceeds 256 KiB"),
            "{error:?}"
        );
        let error = streamed_frames(
            vec![serde_json::json!({
                "choices":[{"index":0,"delta":{"content":"x".repeat(MAX_TEXT_BYTES+1)},"finish_reason":null}]
            })],
            true,
        )
        .err()
        .expect("must reject oversized text");
        assert!(
            error.message.contains("Model response exceeds 64 KiB"),
            "{error:?}"
        );
    }

    fn capture(calls: &mut OpenAiCalls, index: u32, data: &str) {
        calls.capture(RawFrameRef {
            index,
            event: None,
            data,
            elapsed_us: 0,
        });
    }

    #[test]
    fn wire_limits_report_which_limit_failed_and_discard_partial_calls() {
        let initial = serde_json::json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"id":"partial","function":{"name":"chart_get_context","arguments":"{"}
        }]}}]})
        .to_string();
        let mut calls = OpenAiCalls::default();
        capture(&mut calls, 0, &initial);
        let metadata = serde_json::json!({"padding":"x".repeat(1024 * 1024)}).to_string();
        for i in 1..=32 {
            capture(&mut calls, i, &metadata);
        }
        let error = calls.take().unwrap_err();
        assert!(error.message.contains("wire byte limit"), "{error:?}");
        assert!(error.message.contains("33554432 bytes"), "{error:?}");

        let mut calls = OpenAiCalls::default();
        capture(&mut calls, 0, &initial);
        // Frame indices are contiguous and zero-based in the SDK.
        for i in 1..MAX_STREAM_FRAMES as u32 {
            capture(&mut calls, i, "{}");
        }
        assert!(calls.error.is_none());
        capture(&mut calls, MAX_STREAM_FRAMES as u32, "{}");
        let error = calls.take().unwrap_err();
        assert!(error.message.contains("frame limit"), "{error:?}");
        assert!(error.message.contains("65537 > 65536 frames"), "{error:?}");
    }
}
impl Provider for Genai {
    fn complete<'a>(&'a mut self, request: ChatRequest, emit: &'a Emit<'a>) -> Completion<'a> {
        Box::pin(async move {
            let calls = Arc::new(Mutex::new(OpenAiCalls::default()));
            let mut options = ChatOptions::default()
                .with_capture_content(true)
                .with_capture_tool_calls(true)
                .with_capture_usage(true);
            if self.openai {
                options = options.with_raw_frame_fn({
                    let calls = calls.clone();
                    move |_, frame| calls.lock().unwrap().capture(frame)
                });
            }
            let mut response = self
                .client
                .exec_chat_stream(self.target.clone(), request, Some(&options))
                .await
                .map_err(provider_error)?;
            let mut bytes = 0usize;
            let mut reasoning_bytes = 0usize;
            let mut tool_sizes = HashMap::new();
            let mut events = 0usize;
            while let Some(event) = response.stream.next().await {
                if let Some(error) = &calls.lock().unwrap().error {
                    return Err(error.clone());
                }
                events += 1;
                // Start and End are SDK events in addition to the wire frames.
                if events > MAX_STREAM_FRAMES + 2 {
                    return Err(ToolError::invalid(format!(
                        "Model stream exceeds event limit ({} > {} events); no tools executed",
                        events,
                        MAX_STREAM_FRAMES + 2
                    )));
                }
                match event.map_err(provider_error)? {
                    ChatStreamEvent::Chunk(chunk) => {
                        bytes += chunk.content.len();
                        if bytes > MAX_TEXT_BYTES {
                            return Err(ToolError::invalid("Model response exceeds 64 KiB"));
                        }
                        emit(Event::Text(chunk.content))?;
                    }
                    ChatStreamEvent::ReasoningChunk(chunk) => {
                        // Reasoning is not displayed or retained, but must have
                        // its own limit rather than spending the wire allowance.
                        reasoning_bytes += chunk.content.len();
                        if reasoning_bytes > MAX_REASONING_BYTES {
                            return Err(ToolError::invalid(format!(
                                "Model reasoning exceeds 256 KiB ({} bytes); no tools executed",
                                reasoning_bytes
                            )));
                        }
                    }
                    ChatStreamEvent::ToolCallChunk(chunk) => {
                        // genai emits cumulative arguments. Count each call's latest
                        // size rather than charging every prefix a second time.
                        tool_sizes.insert(chunk.tool_call.call_id.clone(), chunk.tool_call.size());
                        if tool_sizes.len() > 64
                            || tool_sizes.values().sum::<usize>() + bytes > MAX_TOOL_BYTES
                        {
                            return Err(ToolError::invalid(
                                "Model tool stream exceeds 256 KiB or 64 calls",
                            ));
                        }
                    }
                    ChatStreamEvent::End(end) => {
                        if !matches!(
                            end.captured_stop_reason,
                            Some(
                                StopReason::Completed(_)
                                    | StopReason::ToolCall(_)
                                    | StopReason::StopSequence(_)
                            )
                        ) {
                            return Err(ToolError::invalid(
                                "Provider output lacks a successful finish reason; no tools executed",
                            ));
                        }
                        let mut content = end.captured_content.unwrap_or_default();
                        if self.openai {
                            let mut parts: Vec<_> = content
                                .into_iter()
                                .filter(|p| !matches!(p, ContentPart::ToolCall(_)))
                                .collect();
                            parts.extend(
                                calls
                                    .lock()
                                    .unwrap()
                                    .take()?
                                    .into_iter()
                                    .map(ContentPart::ToolCall),
                            );
                            content = MessageContent::from_parts(parts);
                        }
                        let message = ChatMessage::assistant(content);
                        if message.size() > super::MAX_CONTEXT_BYTES {
                            return Err(ToolError::invalid(
                                "Model response exceeds 256 KiB; no tools executed",
                            ));
                        }
                        return Ok(Turn {
                            message,
                            usage: end.captured_usage,
                        });
                    }
                    _ => {}
                }
            }
            Err(ToolError::new(
                ErrorCode::CoreUnavailable,
                "Provider stream ended without a complete response; no tools executed",
            ))
        })
    }
}
