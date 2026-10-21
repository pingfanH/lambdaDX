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
            std::env::var(&settings.credential_env).map_err(|_| {
                ToolError::invalid("Configured credential environment variable is unavailable")
            })?
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

/// genai 0.7's OpenAI streamer only captures the first call in a regular
/// delta array. Its public frame hook lets us assemble every call without
/// retaining raw frames, while genai still owns HTTP, text, usage and finish.
#[derive(Default)]
struct OpenAiCalls {
    calls: BTreeMap<u64, (String, String, String)>,
    bytes: usize,
    error: Option<ToolError>,
}
impl OpenAiCalls {
    fn capture(&mut self, frame: RawFrameRef<'_>) {
        if self.error.is_some() {
            return;
        }
        self.bytes += frame.data.len();
        if self.bytes > 1024 * 1024 || frame.index >= 16384 {
            self.error = Some(ToolError::invalid("Provider stream exceeds wire budget"));
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
            > 256 * 1024
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
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/", listener.local_addr().unwrap());
        let chunks: String = chunks.into_iter().map(|calls| format!("data: {}\n\n", serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":calls},"finish_reason":null}]}))).collect();
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
            let mut tool_sizes = HashMap::new();
            let mut events = 0usize;
            while let Some(event) = response.stream.next().await {
                if let Some(error) = &calls.lock().unwrap().error {
                    return Err(error.clone());
                }
                events += 1;
                if events > 16384 {
                    return Err(ToolError::invalid("Model stream exceeds event limit"));
                }
                match event.map_err(provider_error)? {
                    ChatStreamEvent::Chunk(chunk) => {
                        bytes += chunk.content.len();
                        if bytes > 64 * 1024 {
                            return Err(ToolError::invalid("Model response exceeds 64 KiB"));
                        }
                        emit(Event::Text(chunk.content))?;
                    }
                    ChatStreamEvent::ToolCallChunk(chunk) => {
                        // genai emits cumulative arguments. Count each call's latest
                        // size rather than charging every prefix a second time.
                        tool_sizes.insert(chunk.tool_call.call_id.clone(), chunk.tool_call.size());
                        if tool_sizes.len() > 64
                            || tool_sizes.values().sum::<usize>() + bytes > 256 * 1024
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
