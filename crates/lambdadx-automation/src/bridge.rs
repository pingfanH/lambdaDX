//! A bounded, nonblocking frame-thread bridge. No Tokio or network is required.
use crate::{catalog, contract::*};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

pub const QUEUE_CAPACITY: usize = 16;
pub const MAX_DEADLINE: Duration = Duration::from_secs(30);
pub const MAX_REQUESTS_PER_FRAME: usize = 1;

pub trait EditorHost {
    fn execute(&mut self, request: Request) -> ToolResult<Value>;
    fn execute_scoped(&mut self, request: Request, _session: Session) -> ToolResult<Value> { self.execute(request) }
}
#[derive(Clone)]
pub struct Session { id: u64, active: Arc<AtomicBool>, request: Option<Arc<AtomicU8>> }
impl Default for Session {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self { id: NEXT.fetch_add(1, Ordering::Relaxed), active: Arc::new(AtomicBool::new(true)), request: None }
    }
}
impl Session {
    pub fn id(&self) -> u64 { self.id }
    pub fn active(&self) -> bool { self.active.load(Ordering::Acquire) && self.request.as_ref().is_none_or(|r| r.load(Ordering::Acquire) != 2) }
    pub fn cancel(&self) { self.active.store(false, Ordering::Release); }
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::SyncSender<Envelope>,
    library: mpsc::SyncSender<Envelope>,
    session: Session,
}
pub struct Inbox {
    receiver: mpsc::Receiver<Envelope>,
}
pub struct Pending {
    receiver: mpsc::Receiver<ToolResult<Value>>,
    cancelled: Arc<AtomicU8>,
    deadline: Instant,
}
struct Envelope {
    session: Session,
    request: Request,
    reply: mpsc::SyncSender<ToolResult<Value>>,
    cancelled: Arc<AtomicU8>,
    deadline: Instant,
}
pub fn channel() -> (Client, Inbox) {
    channel_inner(None)
}
/// Override the shared library root (useful to embedders and isolated tests).
pub fn channel_with_library(root: std::path::PathBuf) -> (Client, Inbox) {
    channel_inner(Some(root))
}
fn channel_inner(root: Option<std::path::PathBuf>) -> (Client, Inbox) {
    let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
    let (library, requests) = mpsc::sync_channel::<Envelope>(QUEUE_CAPACITY);
    // Separate bounded worker: no index scans or reference reads on the UI
    // thread or Tokio executor. Dropping all clients terminates the worker.
    let _ = std::thread::Builder::new().name("note-design-library".into()).spawn(move || {
        let mut store = root.map(crate::library::Library::new).unwrap_or_else(crate::library::Library::discover);
        for envelope in requests {
            let result = active(&envelope.cancelled, envelope.deadline).and_then(|()| {
                if !envelope.session.active() { return Err(ToolError::new(ErrorCode::Cancelled, "Session ended")); }
                let value = store.execute(envelope.request.clone())?;
                if !envelope.session.active() { return Err(ToolError::new(ErrorCode::Cancelled, "Session ended")); }
                Ok(value)
            });
            deliver(envelope, result);
        }
    });
    (Client { sender, library, session: Session::default() }, Inbox { receiver })
}
fn active(cancelled: &AtomicU8, deadline: Instant) -> ToolResult<()> {
    if cancelled.load(Ordering::Acquire) == 2 {
        return Err(ToolError::new(ErrorCode::Cancelled, "Request cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(ToolError::new(
            ErrorCode::DeadlineExceeded,
            "Request deadline elapsed",
        ));
    }
    Ok(())
}
impl Client {
    pub fn scoped(&self) -> Self { Self { sender: self.sender.clone(), library: self.library.clone(), session: Session::default() } }
    pub fn session(&self) -> Session { self.session.clone() }
    pub fn call(
        &self,
        name: &str,
        arguments: Value,
        permissions: Permissions,
        timeout: Duration,
    ) -> ToolResult<Pending> {
        let request = catalog::parse(name, arguments)?;
        if !self.session.active() { return Err(ToolError::new(ErrorCode::Cancelled, "Session ended")); }
        if !permissions.allows(request.permission()) {
            return Err(ToolError::new(
                ErrorCode::PermissionDenied,
                "Tool permission denied",
            ));
        }
        if timeout.is_zero() || timeout > MAX_DEADLINE {
            return Err(ToolError::invalid(
                "Deadline must be greater than zero and at most 30 seconds",
            ));
        }
        let deadline = Instant::now() + timeout;
        // 0 = awaiting delivery, 1 = consumed, 2 = cancelled/abandoned.
        let cancelled = Arc::new(AtomicU8::new(0));
        let (reply, receiver) = mpsc::sync_channel(1);
        let envelope = Envelope {
            session: Session { request: Some(cancelled.clone()), ..self.session.clone() },
            request,
            reply,
            cancelled: cancelled.clone(),
            deadline,
        };
        let sender = if matches!(envelope.request, Request::LibraryResolve(_) | Request::LibrarySearch(_) | Request::LibraryRead(_)) { &self.library } else { &self.sender };
        sender
            .try_send(envelope)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => ToolError::new(
                    ErrorCode::QueueFull,
                    "Editor request queue is full; retry later",
                ),
                mpsc::TrySendError::Disconnected(_) => {
                    ToolError::new(ErrorCode::Cancelled, "Editor host has stopped")
                }
            })?;
        Ok(Pending {
            receiver,
            cancelled,
            deadline,
        })
    }
}
impl Pending {
    pub fn cancel(&self) {
        self.cancelled.store(2, Ordering::Release);
    }
    pub fn poll(&self) -> Option<ToolResult<Value>> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.cancelled.store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
                Some(result)
            },
            Err(mpsc::TryRecvError::Empty) => {
                if let Err(error) = active(&self.cancelled, self.deadline) {
                    self.cancel();
                    Some(Err(error))
                } else {
                    None
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(ToolError::new(
                ErrorCode::Cancelled,
                "Editor host has stopped",
            ))),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        let _ = self.cancelled.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
    }
}
impl Inbox {
    /// Process at most one request per frame; never wait for a caller. Dropped
    /// or expired callers cannot cause a late host operation.
    pub fn drain(&self, host: &mut impl EditorHost) -> usize {
        let mut count = 0;
        for envelope in self.receiver.try_iter().take(MAX_REQUESTS_PER_FRAME) {
            count += 1;
            let result = active(&envelope.cancelled, envelope.deadline)
                .and_then(|()| {
                    if !envelope.session.active() { return Err(ToolError::new(ErrorCode::Cancelled, "Session ended")); }
                    host.execute_scoped(envelope.request, envelope.session)
                });
            // Host consumes request/session; delivery only needs the reply state.
            deliver_parts(envelope.reply, envelope.cancelled, envelope.deadline, result);
        }
        count
    }
}
fn deliver(envelope: Envelope, result: ToolResult<Value>) {
    deliver_parts(envelope.reply, envelope.cancelled, envelope.deadline, result);
}
fn deliver_parts(reply: mpsc::SyncSender<ToolResult<Value>>, cancelled: Arc<AtomicU8>, deadline: Instant, result: ToolResult<Value>) {
    let result = result.and_then(|value| {
        active(&cancelled, deadline)?;
        if serde_json::to_vec(&value).map_err(|e| ToolError::invalid(e.to_string()))?.len() > MAX_RESULT_BYTES {
            return Err(ToolError::invalid("Tool result exceeds 64 KiB; reduce the range or page size"));
        }
        Ok(value)
    });
    if result.is_err() { cancelled.store(2, Ordering::Release); }
    if reply.try_send(result).is_err() { cancelled.store(2, Ordering::Release); }
}
