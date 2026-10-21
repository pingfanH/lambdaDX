use super::*;
use crate::bridge::{self, EditorHost};
use rmcp::{ClientLifecycleMode, ClientServiceExt, transport::{StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig}};
use serde_json::json;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};

struct Host(Arc<AtomicUsize>);
impl EditorHost for Host {
    fn execute(&mut self, request: crate::contract::Request) -> ToolResult<serde_json::Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        match request {
            crate::contract::Request::Context(_) => Ok(json!({"document_id": "fixture", "snapshot_token": "snapshot"})),
            _ => Err(ToolError::new(crate::contract::ErrorCode::NoDocument, "No fixture document")),
        }
    }
}
fn runtime() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap() }
fn params(name: &str, args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name.to_owned()).with_arguments(args.as_object().unwrap().clone())
}
async fn connect(record: &discovery::Record) -> rmcp::service::RunningService<rmcp::RoleClient, ClientConfig> {
    ClientConfig::default().serve_with_lifecycle(
        StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(record.endpoint()).auth_header(record.token.clone())),
        ClientLifecycleMode::Initialize,
    ).await.unwrap()
}

#[test]
fn real_http_client_and_stdio_relay_use_the_bounded_host() {
    runtime().block_on(async {
        let directory = std::env::temp_dir().join(format!("lambdadx-mcp-test-{}", uuid::Uuid::new_v4()));
        let (client, inbox) = bridge::channel();
        let server = http::Server::start_in(client, &directory).await.unwrap();
        let record = discovery::read_in(&directory, &server.instance_id).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let pump = std::thread::spawn({ let count = count.clone(); let stop = stop.clone(); move || {
            let mut host = Host(count);
            while !stop.load(Ordering::Acquire) { inbox.drain(&mut host); std::thread::sleep(Duration::from_millis(1)); }
        }});
        let client = connect(&record).await;
        assert_eq!(client.list_tools(None).await.unwrap().tools.len(), 5);
        assert_eq!(client.list_resources(None).await.unwrap().resources.len(), 2);
        let resource = client.read_resource(ReadResourceRequestParams::new(UNITS_URI)).await.unwrap();
        assert!(!resource.contents.is_empty());
        let result = client.call_tool(params("chart_get_context", json!({"schema_version":1}))).await.unwrap();
        assert_eq!(result.structured_content.unwrap()["document_id"], "fixture");
        assert_ne!(result.is_error, Some(true));
        assert!(client.call_tool(params("chart_get_context", json!({"schema_version":1, "unknown":true}))).await.is_err());
        let denied = client.call_tool(params("editor_focus_range", json!({"schema_version":1,"target":{"document_id":"fixture","snapshot_token":"snapshot"},"range":{"start_measure":1,"end_measure":2}}))).await.unwrap();
        assert_eq!(denied.is_error, Some(true));
        assert_eq!(denied.structured_content.unwrap()["code"], "PERMISSION_DENIED");
        server.set_permissions(Permissions { read:true, focus:true, propose:true });
        assert_eq!(client.list_tools(None).await.unwrap().tools.len(), 7);
        let error = client.call_tool(params("chart_query_notes", json!({"schema_version":1,"target":{"document_id":"fixture","snapshot_token":"snapshot"},"limit":128}))).await.unwrap();
        assert_eq!(error.is_error, Some(true));
        assert_eq!(error.structured_content.unwrap()["code"], "NO_DOCUMENT");
        client.cancel().await.unwrap();

        // A genuine rmcp stdio client talks through exactly the proxy relay.
        let (a, b) = tokio::io::duplex(512 * 1024);
        let (input, output) = tokio::io::split(a);
        let relay = tokio::spawn(async move { proxy::relay(rmcp::transport::async_rw::AsyncRwTransport::new_server(input, output), &record).await });
        let stdio_client = ClientConfig::default().serve_with_lifecycle(b, ClientLifecycleMode::Initialize).await.unwrap();
        assert_eq!(stdio_client.list_tools(None).await.unwrap().tools.len(), 7);
        let result = stdio_client.call_tool(params("chart_get_context", json!({"schema_version":1}))).await.unwrap();
        assert_eq!(result.structured_content.unwrap()["snapshot_token"], "snapshot");
        stdio_client.cancel().await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(3), relay).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3);
        stop.store(true, Ordering::Release); pump.join().unwrap();
        drop(server);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
        std::fs::remove_dir(directory).unwrap();
    });
}

#[test]
fn http_auth_host_origin_limits_and_request_cancellation() {
    runtime().block_on(async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let directory = std::env::temp_dir().join(format!("lambdadx-mcp-test-{}", uuid::Uuid::new_v4()));
        let (client, inbox) = bridge::channel();
        let server = http::Server::start_in(client, &directory).await.unwrap();
        let record = discovery::read_in(&directory, &server.instance_id).unwrap();
        for (auth, host, origin, expected) in [(false, format!("127.0.0.1:{}", record.port), "", "401"), (true, "attacker.invalid".into(), "", "403"), (true, format!("127.0.0.1:{}",record.port), "Origin: https://attacker.invalid\r\n", "403")] {
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", record.port)).await.unwrap();
            let auth = if auth { format!("Authorization: Bearer {}\r\n", record.token) } else { String::new() };
            stream.write_all(format!("POST /mcp HTTP/1.1\r\nHost: {host}\r\n{auth}{origin}Content-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            let mut response = String::new(); stream.read_to_string(&mut response).await.unwrap();
            assert!(response.starts_with(&format!("HTTP/1.1 {expected}")), "{response}");
        }
        let client = connect(&record).await;
        assert_eq!(server.session_count(), 1);
        let mut clients = Vec::new();
        for _ in 1..http::MAX_SESSIONS { clients.push(connect(&record).await); }
        assert_eq!(server.session_count(), http::MAX_SESSIONS);
        let transport = StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(record.endpoint()).auth_header(record.token.clone()));
        assert!(ClientConfig::default().serve_with_lifecycle(transport, ClientLifecycleMode::Initialize).await.is_err());
        let result = tokio::time::timeout(Duration::from_millis(70), client.call_tool(params("chart_get_context", json!({"schema_version":1})))).await;
        assert!(result.is_err());
        tokio::time::sleep(Duration::from_millis(70)).await;
        let count = Arc::new(AtomicUsize::new(0));
        inbox.drain(&mut Host(count.clone()));
        assert_eq!(count.load(Ordering::SeqCst), 0, "cancelled HTTP request must never execute later");
        client.cancel().await.unwrap();
        for client in clients { client.cancel().await.unwrap(); }
        drop(server); std::fs::remove_dir(directory).unwrap();
    });
}

#[cfg(unix)]
#[test]
fn discovery_rejects_public_or_symlink_records() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = std::env::temp_dir().join(format!("lambdadx-discovery-test-{}", uuid::Uuid::new_v4()));
    let record = discovery::Record { instance_id: "test".into(), port:1234, token:"a".repeat(64), pid:1 };
    let registration = discovery::Registration::in_directory(&directory, &record).unwrap();
    let path = directory.join("test.json");
    assert_eq!(std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(discovery::read_in(&directory, "test").is_err());
    std::fs::remove_file(&path).unwrap();
    symlink("other.json", &path).unwrap();
    assert!(discovery::read_in(&directory, "test").is_err());
    assert!(discovery::read_in(&directory, "../test").is_err());
    drop(registration); std::fs::remove_dir(directory).unwrap();
}
