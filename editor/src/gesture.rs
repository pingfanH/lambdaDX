//! Trackpad pinch-to-zoom gesture source (macOS).
//!
//! macroquad/miniquad's macOS backend handles mouse/keyboard/scroll but never
//! installs a `magnifyWithEvent:` selector, so trackpad pinches are dropped
//! before they reach the app. This module installs a native `NSEvent` local
//! monitor for the magnify gesture and accumulates the magnification into a
//! global that the timeline pulls once per frame (`take`).
//!
//! On non-macOS targets everything is a no-op, so the call sites stay
//! platform-agnostic.

#[cfg(target_os = "macos")]
mod imp {
    use std::ptr::NonNull;
    use std::sync::{Mutex, Once};

    /// Accumulated magnification since the last `take`.
    static MAGNIFICATION: Mutex<f32> = Mutex::new(0.0);
    static INSTALL: Once = Once::new();

    /// Install the monitor once, for the process lifetime. Must run on the main
    /// thread after the app (NSApplication) is running.
    pub fn install() {
        INSTALL.call_once(|| unsafe {
            use block2::RcBlock;
            use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};

            // The monitor block sees the event before dispatch; returning it
            // lets it continue (we only observe, never consume). Matching the
            // whole gesture mask (Begin/End as well as Magnify) keeps the event
            // stream flowing for the entire unbroken two-finger gesture, so a
            // continuous spread/pinch zooms without lifting to re-recognise.
            let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
                let ev = event.as_ref();
                if ev.r#type() == NSEventType::Magnify {
                    add(ev.magnification() as f32);
                }
                event.as_ptr()
            });
            let mask = NSEventMask::Magnify | NSEventMask::SmartMagnify | NSEventMask::Gesture;
            let token = NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block);
            // Keep the token and the block alive for the whole process.
            std::mem::forget(token);
            std::mem::forget(block);
        });
    }

    fn add(delta: f32) {
        if let Ok(mut m) = MAGNIFICATION.lock() {
            *m += delta;
        }
    }

    /// Consume and return the accumulated magnification (0.0 when none).
    pub fn take() -> f32 {
        MAGNIFICATION
            .lock()
            .map(|mut m| std::mem::take(&mut *m))
            .unwrap_or(0.0)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn install() {}
    pub fn take() -> f32 {
        0.0
    }
}

pub use imp::{install, take};
