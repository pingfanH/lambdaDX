use std::sync::{Mutex, OnceLock};
use serde_json::json;

use lnmai_core::session::{self, Session, Empty, FfiEnvelope};
use lnmai_core_rs::ffi::RuntimeStepLightResult;
use lnmai_core_rs::types::{AudioCommand, JudgeEventKind, RenderCommand};
use lnmai_core_rs::chart_loader::ChartSpec;
use lnmai_core_rs::areas::SensorArea;
use lnmai_core_rs::input_model::TimedInputBatch;

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poison| poison.into_inner())
}

fn ensure_runtime() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| unsafe { session::initialize_runtime().unwrap() });
}

fn decode_result<T: serde::de::DeserializeOwned>(env: &FfiEnvelope) -> T {
    let v: serde_json::Value = serde_json::from_str(&env.json).unwrap();
    serde_json::from_value(v["result"].clone()).unwrap()
}

/// Demonstrates how slide judgment data flows from the RS backend.
///
/// This test loads a real chart, steps frames with sensor input that
/// triggers slide progress, and parses the resulting JSON into typed
/// Rust structs. It shows exactly how:
///
/// - `RenderCommand::HideSlideBars { note_index, end_index }` marks
///   individual slide areas as completed (the player's hand passed through).
///
/// - `RenderCommand::UpdateSlideProgress { note_index, remaining }` is
///   emitted only when the remaining area count changes (not every frame).
///
/// - `JudgeEvent { kind: Slide, grade, diff, note_index }` is emitted
///   once when the slide finishes judgment (after the wait countdown).
///
/// - `note_index` is the sole identifier linking all commands to a
///   specific slide. The Rust side must use it to map back to the
///   chart's slide data.
#[test]
fn slide_judgment_parse_instance() {
    let _guard = test_guard();
    let chart_text = include_str!("../../lnmai-core-rs/lnmai-core-ffi/assets/24_Sun Dance/maidata.txt");
    ensure_runtime();
    let empty = Session::<Empty>::create().unwrap();
    let (mut loaded, _load_info) = empty.load_chart_text(chart_text, 6).unwrap();

    let lowered_env = loaded.get_lowered_chart_json().unwrap();
    let lowered_chart: ChartSpec = decode_result(&lowered_env);
    assert!(!lowered_chart.slides.is_empty());

    let state_env = loaded.get_state_json().unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_env.json).unwrap();
    assert_eq!(state["result"]["currentTime"]["ticks"], 0);

    // Step 1: Advance at time 0 with no input.
    let step0 = loaded.advance_frame_light(
        &json!({ "current_time": {"ticks": 0}, "events": [] }).to_string(),
    ).unwrap();

    let v0: serde_json::Value = serde_json::from_str(&step0.json).unwrap();
    assert_eq!(v0["ok"], true);

    // Step 2: Simulate a slide being touched.
    // Hold sensor A1 at t=500ms (500000 microseconds) to start progressing a slide.
    let batch = json!({
        "current_time": {"ticks": 500_000},
        "events": [{
            "SensorHold": {
                "tp": {"ticks": 500_000},
                "area": "A1",
                "is_down": true
            }
        }]
    });
    let step1 = loaded.advance_frame_light(&batch.to_string()).unwrap();

    let result1: RuntimeStepLightResult = decode_result(&step1);

    // Inspect render commands — these tell the renderer what changed.
    for cmd in &result1.render_commands {
        match cmd {
            RenderCommand::HideSlideBars { note_index, end_index } => {
                eprintln!(
                    "slide noteIndex={} area completed, hide bar up to arrow {}",
                    note_index, end_index
                );
            }
            RenderCommand::UpdateSlideProgress { note_index, remaining } => {
                eprintln!(
                    "slide noteIndex={} progress updated, {} areas remaining",
                    note_index, remaining
                );
            }
            RenderCommand::UpdateSlideTrackProgress { note_index, track_index, remaining } => {
                eprintln!(
                    "slide noteIndex={} track={} progress, {} areas remaining",
                    note_index, track_index, remaining
                );
            }
            RenderCommand::HideAllSlideBars { note_index } => {
                eprintln!("slide noteIndex={} all bars hidden (ended)", note_index);
            }
            _ => {}
        }
    }

    // Inspect judge events — a Slide event appears only once per slide.
    for evt in &result1.events {
        if evt.kind == JudgeEventKind::Slide {
            eprintln!(
                "SLIDE JUDGED: noteIndex={} grade={:?} diff={}μs",
                evt.note_index, evt.grade, evt.diff
            );
            eprintln!("  position={:?}", evt.position);
        }
    }

    // Inspect audio commands — PlaySlideCue fires when a new track area activates.
    for cmd in &result1.audio_commands {
        if let AudioCommand::PlaySlideCue { note_index, track_index, at_time } = cmd {
            eprintln!(
                "slide noteIndex={} track={} cue at t={}μs",
                note_index, track_index, at_time
            );
        }
    }

    let (_empty, _unload_info) = loaded.unload_chart().unwrap();
}
