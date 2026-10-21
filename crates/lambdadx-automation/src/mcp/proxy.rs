//! Fixed local transport relay. rmcp owns framing, HTTP sessions and cancellation.
use super::discovery;
use rmcp::{RoleServer, transport::{Transport, async_rw::AsyncRwTransport, streamable_http_client::{StreamableHttpClientTransport, StreamableHttpClientTransportConfig}}};
use std::{io, path::PathBuf};
use tokio::task::JoinSet;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut instance = None;
    let mut directory = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--instance" if instance.is_none() => instance = args.next(),
            "--discovery-dir" if directory.is_none() => directory = args.next().map(PathBuf::from),
            _ => return Err(io::Error::other("Usage: lambdadx_mcp --instance <id> [--discovery-dir <private directory>]").into()),
        }
    }
    let instance = instance.ok_or_else(|| io::Error::other("Select an editor with --instance <id>"))?;
    let record = match directory { Some(dir) => discovery::read_in(&dir, &instance)?, None => discovery::read(&instance)? };
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let result = runtime.block_on(async {
        let (input, output) = rmcp::transport::stdio_pipes();
        relay(AsyncRwTransport::new_server(input, output).with_max_line_length(300 * 1024), &record).await
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    result.map_err(Into::into)
}

pub async fn relay(mut stdio: impl Transport<RoleServer> + 'static, record: &discovery::Record) -> io::Result<()> {
    let config = StreamableHttpClientTransportConfig::with_uri(record.endpoint())
        .auth_header(record.token.clone()).max_concurrent_requests(16);
    let mut http = StreamableHttpClientTransport::from_config(config);
    let mut sends = JoinSet::new();
    let result = loop {
        tokio::select! {
            Some(result) = sends.join_next(), if !sends.is_empty() => {
                if !matches!(result, Ok(Ok(()))) { break Err(io::Error::other("Editor MCP transport failed")); }
            }
            message = stdio.receive(), if sends.len() < 16 => {
                let Some(message) = message else { break Ok(()); };
                // The upstream is the fixed editor server, which exposes only
                // the shared Tools/Resources catalog. No third-party endpoint.
                let send = http.send(message);
                sends.spawn(async move { send.await.map_err(|_| ()) });
            }
            message = http.receive() => {
                let Some(message) = message else { break Err(io::Error::other("Editor MCP session ended")); };
                if stdio.send(message).await.is_err() { break Err(io::Error::other("MCP stdout closed")); }
            }
        }
    };
    sends.abort_all();
    let _ = http.close().await;
    let _ = stdio.close().await;
    result
}
