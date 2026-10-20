//! A bounded, nonblocking frame-thread bridge. No Tokio or network is required.
use crate::{catalog, contract::*};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

pub const QUEUE_CAPACITY: usize = 16;
pub const MAX_DEADLINE: Duration = Duration::from_secs(30);
pub const MAX_REQUESTS_PER_FRAME: usize = 1;

pub trait EditorHost {
    fn execute(&mut self, request: Request) -> ToolResult<Value>;
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::SyncSender<Envelope>,
}
pub struct Inbox {
    receiver: mpsc::Receiver<Envelope>,
}
pub struct Pending {
    receiver: mpsc::Receiver<ToolResult<Value>>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
struct Envelope {
    request: Request,
    reply: mpsc::SyncSender<ToolResult<Value>>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
pub fn channel() -> (Client, Inbox) {
    let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
    (Client { sender }, Inbox { receiver })
}
fn active(cancelled: &AtomicBool, deadline: Instant) -> ToolResult<()> {
    if cancelled.load(Ordering::Acquire) {
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
    pub fn call(
        &self,
        name: &str,
        arguments: Value,
        permissions: Permissions,
        timeout: Duration,
    ) -> ToolResult<Pending> {
        let request = catalog::parse(name, arguments)?;
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
        let cancelled = Arc::new(AtomicBool::new(false));
        let (reply, receiver) = mpsc::sync_channel(1);
        let envelope = Envelope {
            request,
            reply,
            cancelled: cancelled.clone(),
            deadline,
        };
        self.sender
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
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn poll(&self) -> Option<ToolResult<Value>> {
        if let Err(error) = active(&self.cancelled, self.deadline) {
            self.cancel();
            return Some(Err(error));
        }
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(ToolError::new(
                ErrorCode::Cancelled,
                "Editor host has stopped",
            ))),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancel();
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
                .and_then(|()| host.execute(envelope.request));
            let result = result.and_then(|value| {
                active(&envelope.cancelled, envelope.deadline)?;
                if serde_json::to_vec(&value)
                    .map_err(|e| ToolError::invalid(e.to_string()))?
                    .len()
                    > MAX_RESULT_BYTES
                {
                    return Err(ToolError::invalid(
                        "Tool result exceeds 64 KiB; reduce the range or page size",
                    ));
                }
                Ok(value)
            });
            let _ = envelope.reply.try_send(result);
        }
        count
    }
}
