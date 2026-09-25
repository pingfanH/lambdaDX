//! Lightweight load-time instrumentation.
//!
//! Every [`Scope`] prints `[perf] <label>: <ms>` on drop (stderr). Enabled by
//! default; set `MAI2_PERF=0` to silence. Use [`time`] for sync work and hold a
//! [`Scope`] manually across `await` points.

use std::time::Instant;

pub fn enabled() -> bool {
    std::env::var("MAI2_PERF")
        .map(|v| v != "0")
        .unwrap_or(true)
}

pub struct Scope {
    label: String,
    start: Instant,
}

impl Scope {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            start: Instant::now(),
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if enabled() {
            let ms = self.start.elapsed().as_secs_f64() * 1000.0;
            eprintln!("[perf] {}: {:.2}ms", self.label, ms);
        }
    }
}

/// Time a synchronous closure.
pub fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let _s = Scope::new(label);
    f()
}
