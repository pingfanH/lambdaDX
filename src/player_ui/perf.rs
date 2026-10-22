//! Lightweight load-time instrumentation.
//!
//! Every [`Scope`] prints `[perf] <label>: <ms>` on drop (stderr). Enabled by
//! default; set `MAI2_PERF=0` to silence. Use [`time`] for sync work and hold a
//! [`Scope`] manually across `await` points.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub fn enabled() -> bool {
    std::env::var("MAI2_PERF")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Enable lightweight per-frame profiling with MAI2_FRAME_PROFILE=1.
///
/// Android applications do not inherit an interactive shell environment, so
/// the same profiler can be enabled on a device by creating the marker file
/// /sdcard/lambdadx-frame-profile before launching the player. The marker is
/// checked once at startup and the profiler reports one aggregate line every
/// 60 frames, rather than logging on every frame.
pub fn frame_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MAI2_FRAME_PROFILE")
            .map(|v| v != "0")
            .unwrap_or(false)
            || std::path::Path::new("/sdcard/lambdadx-frame-profile").is_file()
    })
}

struct FrameStats {
    number: u64,
    started: Instant,
    sections: BTreeMap<&'static str, (Duration, Duration)>,
}

struct FrameWindow {
    frames: u32,
    total: Duration,
    max_total: Duration,
    sections: BTreeMap<&'static str, (Duration, Duration)>,
}

thread_local! {
    static FRAME_STATS: RefCell<Option<FrameStats>> = const { RefCell::new(None) };
    static FRAME_WINDOW: RefCell<FrameWindow> = RefCell::new(FrameWindow {
        frames: 0,
        total: Duration::ZERO,
        max_total: Duration::ZERO,
        sections: BTreeMap::new(),
    });
}

/// Start collecting timings for the current CPU frame. This is a no-op when
/// profiling is disabled, so the normal player loop has no timing map cost.
pub fn frame_begin(number: u64) {
    if !frame_profile_enabled() {
        return;
    }
    FRAME_STATS.with(|slot| {
        *slot.borrow_mut() = Some(FrameStats {
            number,
            started: Instant::now(),
            sections: BTreeMap::new(),
        });
    });
}

/// Measure one named part of the current frame.
pub fn frame_section<T>(label: &'static str, f: impl FnOnce() -> T) -> T {
    if !frame_profile_enabled() {
        return f();
    }
    let started = Instant::now();
    let result = f();
    let elapsed = started.elapsed();
    FRAME_STATS.with(|slot| {
        if let Some(stats) = slot.borrow_mut().as_mut() {
            let entry = stats
                .sections
                .entry(label)
                .or_insert((Duration::ZERO, Duration::ZERO));
            entry.0 += elapsed;
            entry.1 = entry.1.max(elapsed);
        }
    });
    result
}

/// Finish the current frame and print a compact average/maximum summary every
/// 60 frames. The summary is useful on Android logcat while keeping logging
/// itself out of the hot path for almost every frame.
pub fn frame_end() {
    if !frame_profile_enabled() {
        return;
    }
    let Some(stats) = FRAME_STATS.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    FRAME_WINDOW.with(|slot| {
        let mut window = slot.borrow_mut();
        window.frames += 1;
        let total = stats.started.elapsed();
        window.total += total;
        window.max_total = window.max_total.max(total);
        for (label, (sum, max)) in stats.sections {
            let entry = window
                .sections
                .entry(label)
                .or_insert((Duration::ZERO, Duration::ZERO));
            entry.0 += sum;
            entry.1 = entry.1.max(max);
        }
        if window.frames < 60 {
            return;
        }
        let frames = window.frames as f64;
        let sections = window
            .sections
            .iter()
            .map(|(label, (sum, max))| {
                format!(
                    "{label} avg={:.2} max={:.2}",
                    sum.as_secs_f64() * 1000.0 / frames,
                    max.as_secs_f64() * 1000.0
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!(
            "[frame-profile] frame={} total_avg={:.2} total_max={:.2} {}",
            stats.number,
            window.total.as_secs_f64() * 1000.0 / frames,
            window.max_total.as_secs_f64() * 1000.0,
            sections
        );
        *window = FrameWindow {
            frames: 0,
            total: Duration::ZERO,
            max_total: Duration::ZERO,
            sections: BTreeMap::new(),
        };
    });
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
