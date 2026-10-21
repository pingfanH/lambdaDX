//! The Agent uses the real editor host with no MCP listener and no GPU window.
use super::*;
use lambdadx_automation::agent::{
    self, Event, Message, Run, State,
    provider::{self, Completion, Provider, Turn},
};
use serde_json::json;
use std::time::{Duration, Instant};

fn fixture() -> (Service, EditorState, PadPreviewState) {
    let mut chart = crate::app::chart::fallback_chart();
    chart.notes = vec![
        crate::app::types::Note {
            id: 11,
            time: 1.0,
            lane: 1,
            hi_speed: 1.0,
            ..Default::default()
        },
        crate::app::types::Note {
            id: 12,
            time: 2.0,
            lane: 2,
            hi_speed: 1.0,
            ..Default::default()
        },
    ];
    chart.templates.clear();
    chart.template_instances.clear();
    chart.autoplay_overrides.clear();
    let mut editor = EditorState::default();
    editor.selected.extend([11, 12]);
    (
        Service::new(),
        editor,
        PadPreviewState::new_at(chart, None, None, 0.0, None),
    )
}
fn tool(id: &str, name: &str, args: Value) -> Message {
    Message::assistant(agent::ToolCall {
        call_id: id.into(),
        fn_name: name.into(),
        fn_arguments: args,
        thought_signatures: None,
    })
}
struct Fake(usize);
impl Provider for Fake {
    fn complete<'a>(
        &'a mut self,
        request: agent::ChatRequest,
        _: &'a agent::Emit<'a>,
    ) -> Completion<'a> {
        Box::pin(async move {
            let message = if self.0 == 0 {
                tool("ctx", "chart_get_context", json!({"schema_version":1}))
            } else {
                let target = request
                    .messages
                    .iter()
                    .flat_map(|m| m.content.parts())
                    .find_map(|p| {
                        let agent::ContentPart::ToolResponse(response) = p else {
                            return None;
                        };
                        let result: Value = serde_json::from_str(&response.content).ok()?;
                        result.get("result")?.get("target").cloned()
                    })
                    .expect("actual host context");
                tool(
                    "move",
                    "chart_propose_edit",
                    json!({"schema_version":1,"target":target,"operation_id":"move","summary":"Move explicit selection by one quarter measure","edits":[{"kind":"transform_notes","note_ids":["11","12"],"delta_measures":0.25,"lane_rotation":0}]}),
                )
            };
            self.0 += 1;
            Ok(Turn {
                message,
                usage: None,
            })
        })
    }
}
fn drive(provider: impl Provider + 'static) -> (Service, EditorState, PadPreviewState, Run) {
    let (mut host, mut editor, app) = fixture();
    let runtime = host.background().unwrap();
    let run = Run::start(
        &runtime,
        provider,
        host.client(),
        vec![Message::user(
            "Read chart_get_context and query the selected note IDs 11 and 12, then propose moving only these two notes one beat later. A beat is exactly 0.25 measures. Use chart_propose_edit; do not apply anything. No follow-up clarification is needed.",
        )],
        Permissions {
            read: true,
            focus: false,
            propose: true,
        },
    );
    let start = Instant::now();
    loop {
        host.tick(&mut editor, &app, true, false);
        for event in run.events.try_iter() {
            if let Event::Tool { name, result, .. } = event {
                println!("tool={name} success={}", result.is_ok());
            }
        }
        if let Ok(Event::Finished { state, error, .. }) = run.finished.try_recv() {
            assert_eq!(state, State::AwaitingApproval, "run error: {error:?}");
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(125),
            "agent timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(app.chart.notes[0].time, 1.0);
    assert!(!editor.dirty);
    assert_eq!(
        host.proposals.pending().unwrap().result.state,
        ProposalState::AwaitingApproval
    );
    (host, editor, app, run)
}
#[test]
fn agent_without_mcp_proposes_through_real_host_and_stop_revokes_review() {
    let (mut host, mut editor, app, run) = drive(Fake(0));
    run.stop();
    host.tick(&mut editor, &app, true, false);
    assert!(host.proposals.pending().is_none());
    assert_eq!(app.chart.notes[0].time, 1.0);
    assert!(!editor.dirty);
}
#[test]
fn agent_proposal_requires_user_approval_and_is_one_undo_step() {
    let (mut host, mut editor, mut app, _run) = drive(Fake(0));
    host.proposals
        .approve(&mut editor, &mut app, &mut host.snapshots, true, false)
        .unwrap();
    assert_eq!(app.chart.notes[0].time, 1.25);
    assert_eq!(app.chart.notes[1].time, 2.25);
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.notes[0].time, 1.0);
    assert_eq!(app.chart.notes[1].time, 2.0);
}
#[test]
#[ignore = "Explicit network smoke; requires LDX_AGENT_PROVIDER URL, LDX_AGENT_MODEL and LDX_AGENT_SECRET"]
fn live_agent_smoke() {
    let settings = provider::Settings {
        provider: provider::ProviderKind::OpenAICompatible,
        credential_env: "LDX_AGENT_SECRET".into(),
        allow_focus: false,
        allow_proposals: true,
        ..provider::Settings::default()
    }
    .with_environment()
    .unwrap();
    let provider = provider::Genai::new(&settings).unwrap();
    let (host, _, _, run) = drive(provider);
    let proposal = &host.proposals.pending().unwrap().result;
    assert_eq!(proposal.changes.len(), 2);
    assert!(
        proposal
            .changes
            .iter()
            .all(|change| ["11", "12"].contains(&change.id.as_str()))
    );
    for change in &proposal.changes {
        let before = change.before.as_ref().unwrap();
        let after = change.after.as_ref().unwrap();
        assert_eq!(after.measure - before.measure, 0.25);
    }
    // Real provider never approves, applies or writes this in-memory fixture.
    run.stop();
}

#[cfg(all(feature = "editor-mcp", unix))]
#[test]
fn direct_http_mcp_queries_and_proposes_to_the_same_editor_host() {
    use std::io::{BufRead, BufReader, Write};
    // A wire client keeps this editor test independent of rmcp's client-only
    // features. The shared crate separately tests the genuine rmcp client.
    fn post(port: u16, token: &str, session: &mut String, body: Value) -> Value {
        let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let body = body.to_string();
        let session_header = if session.is_empty() {
            String::new()
        } else {
            format!("Mcp-Session-Id: {session}\r\n")
        };
        write!(socket, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-03-26\r\n{session_header}Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut reader = BufReader::new(socket);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("HTTP/1.1 200"), "{line}");
        loop {
            line.clear();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("mcp-session-id") {
                    *session = value.trim().into();
                }
            }
        }
        loop {
            line.clear();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(value) = serde_json::from_str::<Value>(data.trim()) {
                    if value.get("id").is_some() {
                        assert!(value.get("error").is_none(), "{value}");
                        return value["result"].clone();
                    }
                }
            }
        }
    }
    let (mut host, mut editor, app) = fixture();
    let runtime = host.background().unwrap();
    let directory = std::env::temp_dir().join(format!("ldx-editor-mcp-{}", std::process::id()));
    let server = runtime
        .block_on(lambdadx_automation::mcp::http::Server::start_in(
            host.client(),
            &directory,
        ))
        .unwrap();
    server.set_permissions(Permissions {
        read: true,
        focus: false,
        propose: true,
    });
    let record =
        lambdadx_automation::mcp::discovery::read_in(&directory, &server.instance_id).unwrap();
    let thread = std::thread::spawn(move || {
        let mut session = String::new();
        post(
            record.port,
            &record.token,
            &mut session,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"editor-test","version":"1"}}}),
        );
        let context = post(
            record.port,
            &record.token,
            &mut session,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"chart_get_context","arguments":{"schema_version":1}}}),
        );
        let target = &context["structuredContent"]["target"];
        assert!(target["snapshot_token"].is_string());
        let notes = post(
            record.port,
            &record.token,
            &mut session,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"chart_query_notes","arguments":{"schema_version":1,"target":target,"note_ids":["11","12"],"limit":128}}}),
        );
        assert_ne!(notes["isError"], true);
        assert_eq!(
            notes["structuredContent"]["notes"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let proposal = post(
            record.port,
            &record.token,
            &mut session,
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"chart_propose_edit","arguments":{"schema_version":1,"target":target,"operation_id":"http-move","summary":"Move explicit notes by a quarter measure","edits":[{"kind":"transform_notes","note_ids":["11","12"],"delta_measures":0.25,"lane_rotation":0}]}}}),
        );
        assert_ne!(proposal["isError"], true);
        assert_eq!(
            proposal["structuredContent"]["changes"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    });
    let start = Instant::now();
    while !thread.is_finished() {
        host.tick(&mut editor, &app, true, false);
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    thread.join().unwrap();
    assert_eq!(
        host.proposals.pending().unwrap().result.state,
        ProposalState::AwaitingApproval
    );
    assert_eq!(app.chart.notes[0].time, 1.0);
    assert!(!editor.dirty);
    drop(server);
    host.tick(&mut editor, &app, true, false);
    assert!(host.proposals.pending().is_none());
    std::fs::remove_dir(directory).unwrap();
}
