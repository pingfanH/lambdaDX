use crate::{bridge::*, catalog, contract::*};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Default)]
struct Host(usize);
impl EditorHost for Host {
    fn execute(&mut self, _: Request) -> ToolResult<Value> {
        self.0 += 1;
        Ok(json!({"snapshot_token": "host-created"}))
    }
}
fn request(client: &Client) -> Pending {
    client
        .call(
            "chart_get_context",
            json!({"schema_version": 1}),
            Permissions::READ_ONLY,
            Duration::from_secs(1),
        )
        .unwrap()
}
fn query() -> Value {
    json!({"schema_version":1,"target":{"document_id":"document","snapshot_token":"snapshot"},"note_ids":[u64::MAX.to_string()],"limit":128})
}
#[test]
fn catalog_is_strict_and_keeps_u64_ids_as_strings() {
    let Request::QueryNotes(parsed) = catalog::parse("chart_query_notes", query()).unwrap() else {
        panic!()
    };
    assert_eq!(parsed.note_ids, [u64::MAX.to_string()]);
    let mut invalid = query();
    invalid["target"]["invented"] = json!(true);
    assert!(catalog::parse("chart_query_notes", invalid).is_err());
    for id in [
        json!(9007199254740993_u64),
        json!("01"),
        json!("18446744073709551616"),
    ] {
        let mut args = query();
        args["note_ids"] = json!([id]);
        assert!(catalog::parse("chart_query_notes", args).is_err());
    }
    for limit in [0, 129] {
        let mut args = query();
        args["limit"] = json!(limit);
        assert!(catalog::parse("chart_query_notes", args).is_err());
    }
    assert!(catalog::parse("chart_get_context", json!({"schema_version":2})).is_err());
    assert!(catalog::parse("save", json!({})).is_err());
    for tool in catalog::tools() {
        assert_eq!(tool.input_schema["additionalProperties"], false);
        assert!(tool.output_schema.is_object());
    }
}
#[test]
fn queue_is_bounded_and_drains_one_per_frame() {
    let (client, inbox) = channel();
    let pending: Vec<_> = (0..QUEUE_CAPACITY).map(|_| request(&client)).collect();
    assert_eq!(
        client
            .call(
                "chart_get_context",
                json!({"schema_version":1}),
                Permissions::READ_ONLY,
                Duration::from_secs(1)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::QueueFull
    );
    let mut host = Host::default();
    assert_eq!(inbox.drain(&mut host), 1);
    assert_eq!(host.0, 1);
    assert!(pending[0].poll().unwrap().is_ok());
    assert!(pending[1].poll().is_none());
}
#[test]
fn cancelled_dropped_and_expired_calls_never_execute() {
    let (client, inbox) = channel();
    let cancelled = request(&client);
    cancelled.cancel();
    drop(request(&client));
    let expired = client
        .call(
            "chart_get_context",
            json!({"schema_version":1}),
            Permissions::READ_ONLY,
            Duration::from_millis(1),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(5));
    let mut host = Host::default();
    for _ in 0..3 {
        inbox.drain(&mut host);
    }
    assert_eq!(host.0, 0);
    assert_eq!(
        cancelled.poll().unwrap().unwrap_err().code,
        ErrorCode::Cancelled
    );
    assert_eq!(
        expired.poll().unwrap().unwrap_err().code,
        ErrorCode::DeadlineExceeded
    );
}
#[test]
fn permissions_deadlines_and_shutdown_are_explicit() {
    let (client, inbox) = channel();
    let args = json!({"schema_version":1,"target":{"document_id":"d","snapshot_token":"s"},"range":{"start_measure":1,"end_measure":2}});
    assert_eq!(
        client
            .call(
                "editor_focus_range",
                args,
                Permissions::READ_ONLY,
                Duration::from_secs(1)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::PermissionDenied
    );
    assert!(
        client
            .call(
                "chart_get_context",
                json!({"schema_version":1}),
                Permissions::READ_ONLY,
                Duration::ZERO
            )
            .is_err()
    );
    let pending = request(&client);
    drop(inbox);
    assert_eq!(
        pending.poll().unwrap().unwrap_err().code,
        ErrorCode::Cancelled
    );
    assert_eq!(
        client
            .call(
                "chart_get_context",
                json!({"schema_version":1}),
                Permissions::READ_ONLY,
                Duration::from_secs(1)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::Cancelled
    );
}
#[test]
fn output_size_limit_is_explicit() {
    struct Large;
    impl EditorHost for Large {
        fn execute(&mut self, _: Request) -> ToolResult<Value> {
            Ok(json!("x".repeat(MAX_RESULT_BYTES)))
        }
    }
    let (client, inbox) = channel();
    let pending = request(&client);
    inbox.drain(&mut Large);
    assert_eq!(
        pending.poll().unwrap().unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn argument_sizes_and_numeric_boundaries_are_rejected() {
    for range in [
        json!({"start_measure":0,"end_measure":2}),
        json!({"start_measure":2,"end_measure":2}),
        json!({"start_measure":1,"end_measure":1e100}),
    ] {
        let mut args = query();
        args["range"] = range;
        assert!(catalog::parse("chart_query_notes", args).is_err());
    }
    let mut args = query();
    args["note_ids"] = json!(vec!["1"; MAX_IDS + 1]);
    assert!(catalog::parse("chart_query_notes", args).is_err());
    let mut args = query();
    args["target"]["snapshot_token"] = json!("x".repeat(MAX_INPUT_BYTES));
    assert!(catalog::parse("chart_query_notes", args).is_err());
    assert_eq!(catalog::tools().len(), 10);
    assert!(std::ptr::eq(catalog::tools(), catalog::tools()));
}

#[cfg(any(feature = "mcp", feature = "agent"))]
#[test]
fn background_runtime_runs_and_shuts_down_without_an_editor_or_network() {
    let runtime = crate::runtime::BackgroundRuntime::new().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    runtime.handle().spawn(async move {
        sender.send(42).unwrap();
    });
    assert_eq!(receiver.recv_timeout(Duration::from_secs(5)).unwrap(), 42);
    runtime.shutdown();
}

#[test]
fn consumed_reply_keeps_session_alive_and_delivered_errors_win() {
    struct Scoped(Option<Session>, bool);
    impl EditorHost for Scoped {
        fn execute(&mut self, _: Request) -> ToolResult<Value> { unreachable!() }
        fn execute_scoped(&mut self, _: Request, session: Session) -> ToolResult<Value> {
            self.0 = Some(session);
            if self.1 { Err(ToolError::new(ErrorCode::DocumentChanged, "real error")) } else { Ok(json!({})) }
        }
    }
    let (client, inbox) = channel();
    let mut host = Scoped(None, false);
    let pending = request(&client);
    inbox.drain(&mut host);
    assert!(pending.poll().unwrap().is_ok());
    drop(pending);
    assert!(host.0.as_ref().unwrap().active());
    client.session().cancel();
    assert!(!host.0.as_ref().unwrap().active());
    let (client, inbox) = channel();
    let pending = request(&client);
    host.1 = true;
    inbox.drain(&mut host);
    pending.cancel();
    assert_eq!(pending.poll().unwrap().unwrap_err().code, ErrorCode::DocumentChanged);
}
