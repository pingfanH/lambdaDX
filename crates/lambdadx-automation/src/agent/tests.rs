use super::*;
use crate::bridge::{self, EditorHost};
use genai::chat::{ContentPart, ToolCall};
use provider::{Completion, Turn};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct Fake {
    turns: VecDeque<ChatMessage>,
    requests: Arc<Mutex<Vec<ChatRequest>>>,
    delay: Duration,
}
impl Provider for Fake {
    fn complete<'a>(&'a mut self, request: ChatRequest, emit: &'a Emit<'a>) -> Completion<'a> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request);
            tokio::time::sleep(self.delay).await;
            emit(Event::Text("streamed".into()))?;
            Ok(Turn {
                message: self
                    .turns
                    .pop_front()
                    .expect("unexpected extra model request"),
                usage: None,
            })
        })
    }
}
fn fake(turns: Vec<ChatMessage>) -> Fake {
    Fake {
        turns: turns.into(),
        requests: Default::default(),
        delay: Duration::ZERO,
    }
}
fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        call_id: id.into(),
        fn_name: name.into(),
        fn_arguments: args,
        thought_signatures: None,
    }
}
fn context(id: &str) -> ToolCall {
    call(id, "chart_get_context", json!({"schema_version":1}))
}
#[derive(Default)]
struct Host(Vec<Request>);
impl EditorHost for Host {
    fn execute(&mut self, r: Request) -> ToolResult<Value> {
        self.0.push(r);
        Ok(json!({"snapshot_token":"actual-host-snapshot"}))
    }
}
fn exercise(provider: Fake, budget: Budget) -> (Report, Host, Vec<Event>) {
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, inbox) = bridge::channel();
    let mut host = Host::default();
    let events = Mutex::new(Vec::new());
    let emit = |e| {
        events.lock().unwrap().push(e);
        Ok(())
    };
    let result = runtime.handle().block_on(async {
        tokio::select! {
            result = run(provider, client, vec![ChatMessage::user("check notes")], Permissions { read:true, focus:true, propose:true }, budget, CancellationToken::new(), &emit) => result,
            _ = async { loop { inbox.drain(&mut host); tokio::time::sleep(Duration::from_millis(1)).await; } } => unreachable!(),
        }
    });
    runtime.shutdown();
    (result, host, events.into_inner().unwrap())
}
fn responses(report: &Report) -> Vec<&genai::chat::ToolResponse> {
    report
        .messages
        .iter()
        .flat_map(|m| m.content.parts())
        .filter_map(|p| {
            if let ContentPart::ToolResponse(r) = p {
                Some(r)
            } else {
                None
            }
        })
        .collect()
}
fn cursor_context() -> cursor::Context {
    cursor::Context {
        target: SnapshotRef { document_id: "pinned-document".into(), snapshot_token: "pinned-snapshot".into() },
        measure: 5.0, ring_lane: 3, touch_zone: None, step_measures: 0.0625, bpm: 180.0,
    }
}
fn exercise_cursor(provider: impl Provider, permissions: Permissions) -> (Report, Host, Vec<Event>) {
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, inbox) = bridge::channel();
    let mut host = Host::default();
    let events = Mutex::new(Vec::new());
    let emit = |e| { events.lock().unwrap().push(e); Ok(()) };
    let report = runtime.handle().block_on(async {
        tokio::select! {
            report = run_inner(provider, client, vec![ChatMessage::user("生成32个交互")], permissions, Budget::default(),
                CancellationToken::new(), &emit, Mode::Cursor(cursor_context())) => report,
            _ = async { loop { inbox.drain(&mut host); tokio::time::sleep(Duration::from_millis(1)).await; } } => unreachable!(),
        }
    });
    runtime.shutdown();
    (report, host, events.into_inner().unwrap())
}

#[test]
fn cursor_generates_with_one_model_request_and_one_local_proposal() {
    let notes: Vec<_> = (0..32).map(|i| json!({"offset_measures":i as f64 / 16.0,"ring_lane":if i % 2 == 0 { 3 } else { 6 }})).collect();
    let provider = fake(vec![ChatMessage::assistant(json!({"summary":"32-note interaction","notes":notes}).to_string())]);
    let requests = provider.requests.clone();
    let (report, host, events) = exercise_cursor(provider, Permissions { read: true, focus: false, propose: true });
    assert_eq!(report.state, State::AwaitingApproval, "{:?}", report.error);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].tools.is_none(), "direct mode has no model tool loop");
    let request = format!("{:?}", requests[0]);
    assert!(request.contains("左右手交替") && request.contains("output_schema") && request.contains("generation_request"));
    assert!(!request.contains("chart_get_context") && !request.contains("chart_read_simai"));
    let [Request::Propose(proposal)] = host.0.as_slice() else { panic!("only one local proposal is allowed") };
    assert_eq!(proposal.target, cursor_context().target);
    assert!(proposal.operation_id.starts_with("cursor-"));
    let [Edit::AddNotes { notes }] = proposal.edits.as_slice() else { panic!() };
    assert_eq!(notes.len(), 32);
    assert_eq!(notes[0].measure, 5.0);
    assert_eq!(notes[31].measure, 5.0 + 31.0 / 16.0);
    assert_eq!(events.iter().filter(|e| matches!(e, Event::Tool { .. })).count(), 1);
    assert!(!events.iter().any(|e| matches!(e, Event::Text(t) if t == "streamed")));
    assert!(events.iter().any(|e| matches!(e, Event::Text(t) if t.contains("32 个音符"))));
    assert!(!format!("{:?}", report.messages).contains("generation_request"), "definitions are transient");
}

#[test]
#[ignore = "Explicit network smoke; reads configured model credentials from the environment/.env"]
fn live_direct_generation_smoke() {
    struct Counted { inner: provider::Genai, calls: Arc<Mutex<usize>> }
    impl Provider for Counted {
        fn complete<'a>(&'a mut self, request: ChatRequest, emit: &'a Emit<'a>) -> Completion<'a> {
            *self.calls.lock().unwrap() += 1;
            self.inner.complete(request, emit)
        }
    }
    let settings = provider::Settings::default().with_environment().unwrap();
    let calls = Arc::new(Mutex::new(0));
    let provider = Counted { inner: provider::Genai::new(&settings).unwrap(), calls: calls.clone() };
    let started = Instant::now();
    let (report, host, _) = exercise_cursor(provider, Permissions { read: true, focus: false, propose: true });
    eprintln!("Direct generation: model={}, requests={}, elapsed={:?}", settings.model, calls.lock().unwrap(), started.elapsed());
    assert_eq!(report.state, State::AwaitingApproval, "{:?}", report.error);
    assert_eq!(*calls.lock().unwrap(), 1);
    let [Request::Propose(p)] = host.0.as_slice() else { panic!("missing local proposal") };
    let [Edit::AddNotes { notes }] = p.edits.as_slice() else { panic!() };
    assert_eq!(notes.len(), 32);
}

#[test]
fn invalid_direct_output_never_retries_the_model_or_delivers_an_edit() {
    for message in [
        ChatMessage::assistant("not JSON"),
        ChatMessage::assistant(context("disallowed-tool")),
        ChatMessage::assistant(r#"{"summary":"invalid offset","notes":[{"offset_measures":-1}]}"#),
        ChatMessage::assistant(r#"{"summary":"empty","notes":[]}"#),
        ChatMessage::assistant(r#"{"summary":"bad lane","notes":[{"offset_measures":0,"ring_lane":9}]}"#),
        ChatMessage::assistant(r#"{"summary":"extra fields","notes":[{"offset_measures":0}],"apply":true}"#),
    ] {
        let provider = fake(vec![message]);
        let requests = provider.requests.clone();
        let (report, host, _) = exercise_cursor(provider, Permissions { read: true, focus: false, propose: true });
        assert_eq!(report.state, State::Failed);
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert!(host.0.is_empty());
    }
    let provider = fake(vec![]);
    let requests = provider.requests.clone();
    let (report, host, _) = exercise_cursor(provider, Permissions::READ_ONLY);
    assert_eq!(report.error.unwrap().code, ErrorCode::PermissionDenied);
    assert!(requests.lock().unwrap().is_empty() && host.0.is_empty());
}

#[test]
fn stopping_direct_generation_during_network_cannot_create_a_late_proposal() {
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, inbox) = bridge::channel();
    let mut provider = fake(vec![]);
    provider.delay = Duration::from_secs(30);
    let requests = provider.requests.clone();
    let run = Run::start_at_cursor(&runtime.handle(), provider, client, vec![ChatMessage::user("交互")],
        Permissions { read: true, focus: false, propose: true }, cursor_context());
    let started = Instant::now();
    while requests.lock().unwrap().is_empty() {
        assert!(started.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(1));
    }
    run.stop();
    assert!(matches!(run.finished.recv_timeout(Duration::from_secs(2)).unwrap(), Event::Finished { state: State::Cancelled, .. }));
    let mut host = Host::default();
    inbox.drain(&mut host);
    assert!(host.0.is_empty());
    assert_eq!(requests.lock().unwrap().len(), 1);
    drop(run);
    runtime.shutdown();
}
#[test]
fn streams_text_and_reports_unknown_usage() {
    let (report, host, events) = exercise(
        fake(vec![ChatMessage::assistant("done")]),
        Budget::default(),
    );
    assert_eq!(report.state, State::Completed);
    assert!(host.0.is_empty());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Text(t) if t == "streamed"))
    );
    assert!(events.iter().any(|e| matches!(e, Event::Usage(None))));
}
#[test]
fn multiple_sequential_tools_preserve_results_and_continuation() {
    let provider = fake(vec![
        ChatMessage::assistant(vec![context("a"), context("b")]),
        ChatMessage::assistant("done"),
    ]);
    let requests = provider.requests.clone();
    let (report, host, _) = exercise(provider, Budget::default());
    assert_eq!(report.state, State::Completed);
    assert_eq!(host.0.len(), 2);
    assert_eq!(
        responses(&report)
            .iter()
            .map(|r| r.call_id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    let request = &requests.lock().unwrap()[1];
    assert!(format!("{request:?}").contains("actual-host-snapshot"));
    assert!(!format!("{request:?}").contains("approve_edit"));
}
#[test]
fn note_design_library_is_available_to_agent_without_editor_requests() {
    let provider = fake(vec![
        ChatMessage::assistant(vec![call(
            "lookup",
            "library_search",
            json!({"schema_version":1,"query":"交互","limit":3}),
        )]),
        ChatMessage::assistant("done"),
    ]);
    let requests = provider.requests.clone();
    let (report, host, _) = exercise(provider, Budget::default());
    assert_eq!(report.state, State::Completed);
    assert!(
        host.0.is_empty(),
        "Library reads must bypass the editor frame thread"
    );
    let results = responses(&report);
    assert_eq!(results.len(), 1);
    assert!(format!("{:?}", results[0]).contains("interaction"));
    let request = format!("{:?}", requests.lock().unwrap()[0]);
    assert!(request.contains("library_search") && request.contains("library_read"));
    assert!(request.contains("左右手逐击交替"));
}
#[test]
fn invalid_unknown_duplicate_and_malformed_calls_each_get_a_result() {
    let calls = vec![
        context("a"),
        context("a"),
        call("b", "invented", json!({})),
        call("c", "chart_get_context", json!("{half")),
        call(
            "d",
            "chart_get_context",
            json!({"schema_version":1,"unknown":true}),
        ),
    ];
    let (report, host, _) = exercise(
        fake(vec![
            ChatMessage::assistant(calls),
            ChatMessage::assistant("done"),
        ]),
        Budget::default(),
    );
    assert_eq!(host.0.len(), 1);
    let results = responses(&report);
    assert_eq!(results.len(), 5);
    for r in &results[1..] {
        assert!(format!("{r:?}").contains("isError"));
    }
    assert_eq!(report.state, State::Completed);
}
#[test]
fn proposal_stops_without_automatic_model_resumption() {
    let proposal = call(
        "p",
        "chart_propose_edit",
        json!({"schema_version":1,"target":{"document_id":"doc","snapshot_token":"snap"},"operation_id":"op","summary":"move","edits":[{"kind":"transform_notes","note_ids":["1"],"delta_measures":0.25,"lane_rotation":0}]}),
    );
    let provider = fake(vec![ChatMessage::assistant(vec![
        proposal,
        context("skipped"),
    ])]);
    let requests = provider.requests.clone();
    let (report, host, _) = exercise(provider, Budget::default());
    assert_eq!(report.state, State::AwaitingApproval);
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(host.0.len(), 1);
    assert_eq!(responses(&report).len(), 2);
}
#[test]
fn tool_round_network_and_total_budgets_stop() {
    let (report, host, _) = exercise(
        fake(vec![ChatMessage::assistant(vec![
            context("a"),
            context("b"),
        ])]),
        Budget {
            tools: 1,
            ..Budget::default()
        },
    );
    assert_eq!(report.state, State::Failed);
    assert_eq!(host.0.len(), 1);
    assert_eq!(responses(&report).len(), 2);
    let (report, _, _) = exercise(
        fake(vec![ChatMessage::assistant(vec![context("a")])]),
        Budget {
            rounds: 1,
            ..Budget::default()
        },
    );
    assert_eq!(report.error.unwrap().code, ErrorCode::DeadlineExceeded);
    for budget in [
        Budget {
            network: Duration::from_millis(2),
            ..Budget::default()
        },
        Budget {
            total: Duration::from_millis(2),
            ..Budget::default()
        },
    ] {
        let mut provider = fake(vec![]);
        provider.delay = Duration::from_secs(1);
        let (report, host, _) = exercise(provider, budget);
        assert_eq!(report.error.unwrap().code, ErrorCode::DeadlineExceeded);
        assert!(host.0.is_empty());
    }
}
#[test]
fn stop_cancels_queued_tools_before_host_delivery() {
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, inbox) = bridge::channel();
    let stop = CancellationToken::new();
    let token = stop.clone();
    let report = runtime.handle().block_on(async {
        tokio::select! {
            report = run(fake(vec![ChatMessage::assistant(vec![context("a"),context("b")])]), client, vec![], Permissions::READ_ONLY, Budget::default(), stop, &|_| Ok(())) => report,
            _ = async { tokio::time::sleep(Duration::from_millis(20)).await; token.cancel(); std::future::pending::<()>().await; } => unreachable!(),
        }
    });
    assert_eq!(report.state, State::Cancelled);
    assert_eq!(responses(&report).len(), 2);
    let mut host = Host::default();
    inbox.drain(&mut host);
    assert!(host.0.is_empty());
    runtime.shutdown();
}
#[test]
fn prefetched_run_can_query_broader_context_before_proposing() {
    let target = json!({"document_id":"doc","snapshot_token":"snap"});
    let provider = fake(vec![
        ChatMessage::assistant(vec![
            call(
                "neighbors",
                "chart_query_notes",
                json!({"schema_version":1,"target":target,"range":{"start_measure":1.0,"end_measure":17.0},"limit":128}),
            ),
            call(
                "check",
                "chart_check",
                json!({"schema_version":1,"target":target,"level":"structure","limit":128}),
            ),
        ]),
        ChatMessage::assistant(call(
            "edit",
            "chart_propose_edit",
            json!({"schema_version":1,"target":target,"operation_id":"broader","summary":"Use surrounding chart before editing selection","edits":[{"kind":"transform_notes","note_ids":["1"],"delta_measures":0.25,"lane_rotation":0}]}),
        )),
    ]);
    let requests = provider.requests.clone();
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, inbox) = bridge::channel();
    let mut host = Host::default();
    let report = runtime.handle().block_on(async {
        tokio::select! {
            report = run_inner(provider, client, vec![ChatMessage::user("Improve the selection in the context of the surrounding section")],
                Permissions { read:true, focus:false, propose:true }, Budget::default(), CancellationToken::new(), &|_| Ok(()), true) => report,
            _ = async { loop { inbox.drain(&mut host); tokio::time::sleep(Duration::from_millis(1)).await; } } => unreachable!(),
        }
    });
    assert_eq!(report.state, State::AwaitingApproval, "{:?}", report.error);
    assert!(matches!(
        host.0.as_slice(),
        [
            Request::Context(_),
            Request::QueryNotes(_),
            Request::Check(_),
            Request::Propose(_)
        ]
    ));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(format!("{:?}", requests[0]).contains("editor_context"));
    assert!(format!("{:?}", requests[1]).contains("neighbors"));
    for request in requests.iter() {
        for name in [
            "chart_get_context",
            "chart_query_notes",
            "chart_read_simai",
            "chart_check",
            "library_search",
            "library_read",
            "chart_propose_edit",
        ] {
            assert!(format!("{request:?}").contains(name), "lost tool {name}");
        }
    }
    runtime.shutdown();
}

#[test]
fn prefetch_failures_do_not_request_a_model_or_leave_queued_work() {
    struct Busy;
    impl EditorHost for Busy {
        fn execute(&mut self, _: Request) -> ToolResult<Value> {
            Err(ToolError::new(ErrorCode::EditorBusy, "Finish active input"))
        }
    }
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    for (permissions, drain, expected) in [
        (Permissions::READ_ONLY, true, ErrorCode::EditorBusy),
        (
            Permissions {
                read: false,
                focus: false,
                propose: false,
            },
            true,
            ErrorCode::PermissionDenied,
        ),
        (Permissions::READ_ONLY, false, ErrorCode::DeadlineExceeded),
    ] {
        let (client, inbox) = bridge::channel();
        let session = client.session();
        let provider = fake(vec![]);
        let requests = provider.requests.clone();
        let report = runtime.handle().block_on(async {
            tokio::select! {
                report = run_inner(provider, client, vec![ChatMessage::user("move selection")], permissions,
                    Budget { total:Duration::from_millis(20), ..Budget::default() },
                    CancellationToken::new(), &|_| Ok(()), true) => report,
                _ = async {
                    loop {
                        if drain { inbox.drain(&mut Busy); }
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                } => unreachable!(),
            }
        });
        assert_eq!(report.state, State::Failed);
        assert_eq!(report.error.unwrap().code, expected);
        assert!(requests.lock().unwrap().is_empty());
        assert!(!session.active());
        let mut host = Host::default();
        inbox.drain(&mut host);
        assert!(host.0.is_empty(), "failed prefetch cannot execute late");
    }
    runtime.shutdown();
}

#[test]
fn stop_during_prefetch_cancels_before_model_or_host_execution() {
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, inbox) = bridge::channel();
    let provider = fake(vec![]);
    let requests = provider.requests.clone();
    let run = Run::start_with_context(
        &runtime.handle(),
        provider,
        client,
        vec![],
        Permissions::READ_ONLY,
    );
    assert!(matches!(
        run.events.recv_timeout(Duration::from_secs(2)).unwrap(),
        Event::State(State::ExecutingTools)
    ));
    run.stop();
    assert!(matches!(
        run.finished.recv_timeout(Duration::from_secs(2)).unwrap(),
        Event::Finished {
            state: State::Cancelled,
            ..
        }
    ));
    let mut host = Host::default();
    inbox.drain(&mut host);
    assert!(host.0.is_empty());
    assert!(requests.lock().unwrap().is_empty());
    drop(run);
    runtime.shutdown();
}

#[test]
fn terminal_event_survives_stream_queue_overflow() {
    struct Flood;
    impl Provider for Flood {
        fn complete<'a>(&'a mut self, _: ChatRequest, emit: &'a Emit<'a>) -> Completion<'a> {
            Box::pin(async move {
                for _ in 0..200 {
                    emit(Event::Text("x".into()))?;
                }
                unreachable!()
            })
        }
    }
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (client, _) = bridge::channel();
    let run = Run::start(
        &runtime.handle(),
        Flood,
        client,
        vec![],
        Permissions::READ_ONLY,
    );
    let event = run.finished.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        event,
        Event::Finished {
            state: State::Failed,
            ..
        }
    ));
    drop(run);
    runtime.shutdown();
}
#[test]
fn configuration_is_nonsecret_and_rejects_unsafe_endpoints() {
    let mut settings = provider::Settings::default();
    for endpoint in [
        "http://example.com/",
        "https://user:password@example.com/",
        "https://example.com/?key=x",
        "file:///tmp/model",
    ] {
        settings.base_url = endpoint.into();
        assert!(settings.validate().is_err());
    }
    settings.base_url = "http://127.0.0.1:11434/v1/".into();
    assert!(settings.validate().is_ok());
    let serialized = serde_json::to_value(settings).unwrap();
    assert!(serialized.get("credential_env").is_some());
    assert!(serialized.get("secret").is_none());
}
