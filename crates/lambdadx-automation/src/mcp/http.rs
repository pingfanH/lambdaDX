use super::{Handler, discovery::{Record, Registration}};
use crate::{bridge::{Client, Session}, contract::Permissions};
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Request, Response, StatusCode, service::service_fn};
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::{StreamableHttpService, StreamableHttpServerConfig, session::local::LocalSessionManager};
use std::{convert::Infallible, io, sync::{Arc, Mutex, RwLock}, time::Duration};
use tokio::{sync::Semaphore, task::JoinSet};
use tokio_util::sync::CancellationToken;

pub const MAX_SESSIONS: usize = 8;
pub const MAX_CONNECTIONS: usize = 16;
pub struct Server {
    pub instance_id: String,
    pub endpoint: String,
    permissions: Arc<RwLock<Permissions>>,
    sessions: Arc<Mutex<Vec<Session>>>,
    slots: Arc<Semaphore>,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
    _registration: Registration,
}
impl Server {
    pub async fn start(client: Client) -> io::Result<Self> {
        Self::start_in(client, &super::discovery::directory()?).await
    }
    pub async fn start_in(client: Client, directory: &std::path::Path) -> io::Result<Self> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let instance_id = uuid::Uuid::new_v4().to_string();
        let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let record = Record { instance_id: instance_id.clone(), port, token: token.clone(), pid: std::process::id() };
        let registration = Registration::in_directory(directory, &record)?;
        let endpoint = record.endpoint();
        let permissions = Arc::new(RwLock::new(Permissions::READ_ONLY));
        let sessions = Arc::new(Mutex::new(Vec::<Session>::new()));
        let slots = Arc::new(Semaphore::new(MAX_SESSIONS));
        let stop = CancellationToken::new();
        let factory = { let permissions = permissions.clone(); let sessions = sessions.clone(); let slots = slots.clone(); move || {
            let permit = slots.clone().try_acquire_owned().map_err(|_| io::Error::other("MCP session limit reached"))?;
            let client = client.scoped();
            let mut sessions = sessions.lock().unwrap(); sessions.retain(Session::active); sessions.push(client.session());
            Ok(Handler::new(client, permissions.clone(), permit))
        }};
        let mut config = StreamableHttpServerConfig::default().enforce_origin_validation();
        config.cancellation_token = stop.clone(); config.allowed_hosts = vec![format!("127.0.0.1:{port}")];
        config.max_request_body_bytes = 300 * 1024;
        let service = StreamableHttpService::new(factory, Arc::new(LocalSessionManager::default()), config);
        let shutdown = stop.clone();
        let task = tokio::spawn(async move {
            let connections = Arc::new(Semaphore::new(MAX_CONNECTIONS));
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((socket, peer)) = accepted else { break; };
                        if !peer.ip().is_loopback() { continue; }
                        let Ok(permit) = connections.clone().try_acquire_owned() else { continue; };
                        let service = service.clone(); let bearer = format!("Bearer {token}");
                        tasks.spawn(async move {
                            let _permit = permit;
                            let handler = service_fn(move |request: Request<hyper::body::Incoming>| {
                                let service = service.clone(); let bearer = bearer.clone();
                                async move {
                                    let status = if request.uri().path() != "/mcp" { Some(StatusCode::NOT_FOUND) }
                                        else if request.headers().get(hyper::header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some(bearer.as_str()) { Some(StatusCode::UNAUTHORIZED) } else { None };
                                    if let Some(status) = status {
                                        return Ok::<_, Infallible>(Response::builder().status(status).body(Full::new(Bytes::new()).boxed()).unwrap());
                                    }
                                    Ok(service.handle(request).await)
                                }
                            });
                            let _ = tokio::time::timeout(Duration::from_secs(660), hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(socket), handler)).await;
                        });
                    }
                }
            }
            tasks.abort_all(); while tasks.join_next().await.is_some() {}
        });
        Ok(Self { instance_id, endpoint, permissions, sessions, slots, stop, task, _registration: registration })
    }
    pub fn session_count(&self) -> usize { MAX_SESSIONS - self.slots.available_permits() }
    pub fn permissions(&self) -> Permissions { *self.permissions.read().unwrap() }
    pub fn set_permissions(&self, permissions: Permissions) {
        let mut current = self.permissions.write().unwrap();
        if (current.propose && !permissions.propose) || (current.focus && !permissions.focus) || (current.read && !permissions.read) {
            for session in self.sessions.lock().unwrap().drain(..) { session.cancel(); }
        }
        *current = permissions;
    }
}
impl Drop for Server {
    fn drop(&mut self) { self.stop.cancel(); for s in self.sessions.lock().unwrap().drain(..) { s.cancel(); } self.task.abort(); }
}
