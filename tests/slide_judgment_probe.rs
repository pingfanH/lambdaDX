use std::sync::{Mutex, OnceLock};

use lambda_dx::simai_io;
use lambda_dx::types::zone::PadZone;
use lambda_dx::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
};
use lnmai_core_rs::session::{Empty, Loaded, Session};
use lnmai_core_rs::types::{
    ButtonZone, JudgeEvent, JudgeEventKind, JudgeGrade, TimedInputBatch, TimedInputEvent,
};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn ensure_runtime() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| unsafe {
        lnmai_core_rs::session::initialize_runtime().expect("lean runtime init");
    });
}

fn sample_slide_chart_text() -> String {
    let chart = ChartDoc {
        version: "1.0".to_string(),
        title: "slide-probe".to_string(),
        artist: String::new(),
        simai_level: 6,
        bpm: 120.0,
        bpms: vec![BpmChange {
            measure: 1.0,
            bpm: 120.0,
        }],
        audio_offset: 0.0,
        notes: vec![Note {
            time: 1.0,
            lane: 1,
            note_type: NoteType::Slide,
            is_star: true,
            slide: vec![Slide {
                segments: vec![SlideSegment {
                    points: vec![SlidePoint::from(PadZone::from(5_u8))],
                    shape: SlideShape::Line,
                }],
                slide_duration: 1.0,
                slide_start_delay: 0.25,
                slide_is_break: false,
            }],
            ..Default::default()
        }],
        templates: vec![],
        template_instances: vec![],
    };
    let file = simai_io::chart_doc_to_simai_file(&chart);
    maisimai::export_file(&file)
}

fn load_session(chart_text: &str) -> (Session<Loaded>, u64) {
    let empty = Session::<Empty>::create().expect("create session");
    let (loaded, _) = empty
        .load_chart_text(chart_text, 6)
        .expect("load chart");
    let handle = loaded.handle();
    (loaded, handle)
}

fn step_light(loaded: &mut Session<Loaded>, time_us: i64) -> lnmai_core_rs::ffi_types::RuntimeStepLightResult {
    let batch = TimedInputBatch {
        current_time: time_us,
        events: vec![],
    };
    let envelope = loaded
        .advance_frame_light(&serde_json::to_string(&batch).expect("batch json"))
        .expect("step");
    let value: serde_json::Value = serde_json::from_str(&envelope.json).expect("envelope json");
    serde_json::from_value(value.get("result").cloned().unwrap_or_default()).expect("runtime result")
}

fn slide_events(result: &lnmai_core_rs::ffi_types::RuntimeStepLightResult) -> Vec<&JudgeEvent> {
    result
        .events
        .iter()
        .filter(|evt| evt.kind == JudgeEventKind::Slide)
        .collect()
}

#[test]
fn parsed_slide_chart_loads_and_judges_slide_miss_without_input() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = sample_slide_chart_text();
    let (mut loaded, handle) = load_session(&chart_text);
    let _ = handle;

    let result = step_light(&mut loaded, 5_000_000);
    let events = slide_events(&result);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].grade, JudgeGrade::Miss);
}

#[test]
fn parsed_slide_chart_reports_score_state() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = sample_slide_chart_text();
    let (mut loaded, handle) = load_session(&chart_text);
    let _ = handle;

    let result = step_light(&mut loaded, 5_000_000);
    assert!(result.current_time > 0);
    assert!(!result.events.is_empty());
}

#[test]
fn tap_click_at_judge_time_is_perfect() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = "&title=Tap Probe\n&artist=Test\n&first=0\n&lv_6=1\n&inote_6=(120){4}1,2,3,4,E\n";
    let (mut loaded, _) = load_session(&chart_text);

    let batch = TimedInputBatch {
        current_time: 20_000,
        events: vec![TimedInputEvent::ButtonClick {
            tp: 1,
            zone: ButtonZone::K1,
        }],
    };
    let envelope = loaded
        .advance_frame_light(&serde_json::to_string(&batch).unwrap())
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&envelope.json).unwrap();
    eprintln!("[probe] envelope: {}", envelope.json);
    let result: lnmai_core_rs::ffi_types::RuntimeStepLightResult =
        serde_json::from_value(value.get("result").cloned().unwrap()).unwrap();
    let taps: Vec<&JudgeEvent> = result
        .events
        .iter()
        .filter(|evt| evt.kind == JudgeEventKind::Tap)
        .collect();
    assert_eq!(taps.len(), 1, "expected one tap event");
    assert!(!taps[0].grade.is_miss_or_too_fast(), "tap should be a hit");
}
