//! Frame-time probe for isolating the flaky-performance root cause.
//!
//! Records named per-frame stage durations, then prints a 1s summary (p50/p95/
//! max per stage) to stderr and logs any individual slow frame with its full
//! stage breakdown. Set `MAI2_PERF=0` to silence it; it is on by default.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Frames slower than this (in ms) are logged individually.
const SLOW_FRAME_MS: f64 = 12.0;

struct Probe {
    enabled: bool,
    window_start: Instant,
    frames: u32,
    samples: Vec<(&'static str, Vec<f64>)>,
    /// Names of the current frame's stages, in insertion order.
    frame_order: Vec<&'static str>,
    frame_values: Vec<f64>,
}

impl Probe {
    fn new() -> Self {
        let enabled = std::env::var("MAI2_PERF")
            .map(|v| v != "0" && v != "off")
            .unwrap_or(true);
        Probe {
            enabled,
            window_start: Instant::now(),
            frames: 0,
            samples: Vec::new(),
            frame_order: Vec::new(),
            frame_values: Vec::new(),
        }
    }

    fn record(&mut self, name: &'static str, d: Duration) {
        if !self.enabled {
            return;
        }
        let ms = d.as_secs_f64() * 1000.0;
        self.frame_order.push(name);
        self.frame_values.push(ms);
    }

    fn frame_done(&mut self) {
        if !self.enabled {
            return;
        }
        self.frames += 1;

        // Feed this frame's stages into the windowed sample buckets.
        for (name, ms) in self.frame_order.iter().zip(self.frame_values.iter()) {
            let bucket = match self.samples.iter_mut().find(|(n, _)| n == name) {
                Some((_, v)) => v,
                None => {
                    self.samples.push((name, Vec::new()));
                    &mut self.samples.last_mut().unwrap().1
                }
            };
            bucket.push(*ms);
        }

        let total = self.frame_values.last().copied().unwrap_or(0.0);
        if total > SLOW_FRAME_MS {
            let parts: Vec<String> = self
                .frame_order
                .iter()
                .zip(self.frame_values.iter())
                .map(|(n, v)| format!("{n}={v:.2}ms"))
                .collect();
            eprintln!("[perf] slow frame: {}", parts.join(" "));
        }

        self.frame_order.clear();
        self.frame_values.clear();

        if self.window_start.elapsed().as_secs_f64() >= 1.0 {
            self.flush();
        }
    }

    fn flush(&mut self) {
        let mut line = format!("[perf] {} frames", self.frames);
        for (name, samples) in &self.samples {
            if samples.is_empty() {
                continue;
            }
            let (p50, p95, max) = stats(samples);
            line.push_str(&format!(" | {name} p50={p50:.2} p95={p95:.2} max={max:.2}"));
        }
        eprintln!("{line}");
        self.samples.clear();
        self.frames = 0;
        self.window_start = Instant::now();
    }
}

fn stats(samples: &[f64]) -> (f64, f64, f64) {
    let mut s = samples.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = s[s.len() / 2];
    let p95 = s[(((s.len() as f64) * 0.95) as usize).min(s.len() - 1)];
    let max = *s.last().unwrap();
    (p50, p95, max)
}

static PROBE: Mutex<Option<Probe>> = Mutex::new(None);

fn with_probe(f: impl FnOnce(&mut Probe)) {
    if let Ok(mut guard) = PROBE.lock() {
        if guard.is_none() {
            *guard = Some(Probe::new());
        }
        if let Some(p) = guard.as_mut() {
            f(p);
        }
    }
}

pub fn record(name: &'static str, d: Duration) {
    with_probe(|p| p.record(name, d));
}

pub fn frame_done() {
    with_probe(|p| p.frame_done());
}
