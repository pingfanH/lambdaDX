//! Owned once by the editor session, never by a dock pane or an individual call.
pub use tokio::runtime::Handle;
pub struct BackgroundRuntime(tokio::runtime::Runtime);
impl BackgroundRuntime {
    pub fn new() -> std::io::Result<Self> {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("editor-automation")
            .enable_all()
            .build()
            .map(Self)
    }
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.0.handle().clone()
    }
    /// Cancel adapters first, drop their bridge handles, then bound shutdown.
    pub fn shutdown(self) {
        self.0.shutdown_timeout(std::time::Duration::from_secs(1));
    }
}
