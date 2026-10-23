//! Thread-safe lifecycle handoff. Android may suspend rendering before another frame.

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Default)]
pub struct Lifecycle {
    pub muted: Arc<AtomicBool>,
    background: AtomicBool,
    pending_pause: Mutex<Option<Instant>>,
}

impl Lifecycle {
    pub fn suspend(&self) {
        let mut pending = self.pending_pause.lock().unwrap();
        self.muted.store(true, Ordering::Release);
        self.background.store(true, Ordering::Release);
        pending.get_or_insert_with(Instant::now);
    }

    pub fn restore(&self) {
        let _pending = self.pending_pause.lock().unwrap();
        self.background.store(false, Ordering::Release);
    }
    pub fn is_background(&self) -> bool { self.background.load(Ordering::Acquire) }
    pub fn take_pause(&self) -> Option<Instant> { self.pending_pause.lock().unwrap().take() }

    /// Only reopen the audio gate after the main thread has stopped playback.
    pub fn acknowledge_pause(&self) {
        let pending = self.pending_pause.lock().unwrap();
        if pending.is_none() && !self.is_background() {
            self.muted.store(false, Ordering::Release);
        }
    }
}

pub fn global() -> &'static Lifecycle {
    static LIFECYCLE: OnceLock<Lifecycle> = OnceLock::new();
    LIFECYCLE.get_or_init(Lifecycle::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returning_to_foreground_cannot_resume_audio_before_pause_is_handled() {
        let lifecycle = Lifecycle::default();
        lifecycle.suspend();
        let first = *lifecycle.pending_pause.lock().unwrap();
        lifecycle.suspend();
        assert_eq!(*lifecycle.pending_pause.lock().unwrap(), first);
        lifecycle.restore();
        assert!(lifecycle.muted.load(Ordering::Acquire));
        assert_eq!(lifecycle.take_pause(), first);
        lifecycle.acknowledge_pause();
        assert!(!lifecycle.muted.load(Ordering::Acquire));
    }
}
