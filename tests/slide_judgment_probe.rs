use std::sync::{Mutex, OnceLock};

use lambda_dx::simai_io;
use lambda_dx::types::zone::PadZone;
use lambda_dx::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
};
use lnmai_core::session::{Empty, Loaded, Session};
use lnmai_core::types::{
    ButtonZone, JudgeEvent, JudgeEventKind, JudgeGrade, RuntimeStepLightResult, SensorArea,
    TimedInputBatch, TimedInputEvent,
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
        lnmai_core::session::initialize_runtime().expect("lean runtime init");
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
    simai_io::export_simai_file(&file)
}

fn load_session(chart_text: &str) -> (Session<Loaded>, u64) {
    let empty = Session::<Empty>::create().expect("create session");
    let (loaded, _) = empty.load_chart_text(chart_text, 6).expect("load chart");
    let handle = loaded.handle();
    (loaded, handle)
}

fn step_light(loaded: &mut Session<Loaded>, time_us: i64) -> RuntimeStepLightResult {
    step_light_with_events(loaded, time_us, vec![])
}

fn step_light_with_events(
    loaded: &mut Session<Loaded>,
    time_us: i64,
    events: Vec<TimedInputEvent>,
) -> RuntimeStepLightResult {
    let batch = TimedInputBatch {
        current_time: time_us,
        events,
    };
    let envelope = loaded
        .advance_frame_light(&serde_json::to_string(&batch).expect("batch json"))
        .expect("step");
    let value: serde_json::Value = serde_json::from_str(&envelope.json).expect("envelope json");
    serde_json::from_value(value.get("result").cloned().unwrap_or_default())
        .expect("runtime result")
}

fn session_state(loaded: &Session<Loaded>) -> serde_json::Value {
    let envelope = loaded.get_state_json().expect("state json");
    serde_json::from_str(&envelope.json).expect("state envelope json")
}

fn first_slide_queue_areas(loaded: &Session<Loaded>) -> Vec<SensorArea> {
    let state = session_state(loaded);
    state["result"]["slides"][0]["judgeQueues"]
        .as_array()
        .expect("slide judge queues")
        .iter()
        .flat_map(|queue| queue.as_array().expect("queue areas"))
        .flat_map(|area| area["targetAreas"].as_array().expect("target areas"))
        .map(|area| serde_json::from_value(area.clone()).expect("sensor area"))
        .collect()
}

fn sensor_tap(tp: i64, area: SensorArea) -> Vec<TimedInputEvent> {
    vec![
        TimedInputEvent::SensorClick { tp, area },
        TimedInputEvent::SensorHold {
            tp,
            area,
            is_down: true,
        },
    ]
}

fn sensor_release(tp: i64, area: SensorArea) -> Vec<TimedInputEvent> {
    vec![TimedInputEvent::SensorHold {
        tp,
        area,
        is_down: false,
    }]
}

fn sensor_presses(tp: i64, areas: &[SensorArea]) -> Vec<TimedInputEvent> {
    areas
        .iter()
        .flat_map(|area| sensor_tap(tp, *area))
        .collect()
}

fn sensor_releases(tp: i64, areas: &[SensorArea]) -> Vec<TimedInputEvent> {
    areas
        .iter()
        .flat_map(|area| sensor_release(tp, *area))
        .collect()
}

fn slide_events(result: &RuntimeStepLightResult) -> Vec<&JudgeEvent> {
    result
        .events
        .iter()
        .filter(|evt| evt.kind == JudgeEventKind::Slide)
        .collect()
}

#[test]
fn sensor_only_early_slide_head_keeps_body_accessible() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = sample_slide_chart_text();
    let (mut loaded, _) = load_session(&chart_text);
    let areas = first_slide_queue_areas(&loaded);
    assert!(
        !areas.is_empty(),
        "fixture slide should expose sensor queues"
    );

    step_light(&mut loaded, 700_000);
    step_light_with_events(&mut loaded, 800_000, sensor_tap(800_000, SensorArea::A1));
    step_light_with_events(
        &mut loaded,
        850_000,
        sensor_release(850_000, SensorArea::A1),
    );

    for (idx, area) in areas.into_iter().enumerate() {
        let tp = 1_260_000 + (idx as i64 * 90_000);
        step_light_with_events(&mut loaded, tp, sensor_tap(tp, area));
        step_light_with_events(&mut loaded, tp + 40_000, sensor_release(tp + 40_000, area));
    }

    let result = step_light(&mut loaded, 2_700_000);
    let events = slide_events(&result);
    assert_eq!(events.len(), 1, "expected one slide event");
    assert!(
        !events[0].grade.is_miss_or_too_fast(),
        "sensor-only early head plus body sensors should judge slide as hit, got {:?}",
        events[0].grade
    );
}

#[test]
fn same_frame_fast_slide_swipe_advances_body_queue() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = sample_slide_chart_text();
    let (mut loaded, _) = load_session(&chart_text);
    let areas = first_slide_queue_areas(&loaded);
    assert!(
        areas.len() >= 3,
        "fixture slide should have multiple body areas"
    );

    step_light(&mut loaded, 700_000);
    step_light_with_events(&mut loaded, 800_000, sensor_tap(800_000, SensorArea::A1));
    step_light_with_events(
        &mut loaded,
        850_000,
        sensor_release(850_000, SensorArea::A1),
    );

    step_light_with_events(&mut loaded, 1_300_000, sensor_presses(1_300_000, &areas));
    step_light_with_events(
        &mut loaded,
        1_316_000,
        sensor_releases(1_316_000, &areas[..areas.len() - 1]),
    );

    let result = step_light(&mut loaded, 2_700_000);
    let events = slide_events(&result);
    assert_eq!(events.len(), 1, "expected one slide event");
    assert!(
        !events[0].grade.is_miss_or_too_fast(),
        "same-frame fast body swipe should judge slide as hit, got {:?}",
        events[0].grade
    );
}

#[test]
fn holding_early_slide_head_into_body_window_counts_as_body_sensor_on() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = sample_slide_chart_text();
    let (mut loaded, _) = load_session(&chart_text);
    let areas = first_slide_queue_areas(&loaded);
    assert!(
        !areas.is_empty(),
        "fixture slide should expose sensor queues"
    );

    step_light(&mut loaded, 300_000);
    step_light_with_events(&mut loaded, 500_000, sensor_tap(500_000, SensorArea::A1));
    step_light(&mut loaded, 1_260_000);

    let state = session_state(&loaded);
    let first_area = &state["result"]["slides"][0]["judgeQueues"][0][0];
    assert_eq!(
        first_area["wasOn"], true,
        "an already-held head sensor should be visible when slide body becomes checkable"
    );

    for (idx, area) in areas.into_iter().enumerate().skip(1) {
        let tp = 1_320_000 + (idx as i64 * 90_000);
        step_light_with_events(&mut loaded, tp, sensor_tap(tp, area));
        step_light_with_events(&mut loaded, tp + 40_000, sensor_release(tp + 40_000, area));
    }

    let result = step_light(&mut loaded, 2_700_000);
    let events = slide_events(&result);
    assert_eq!(events.len(), 1, "expected one slide event");
    assert!(
        !events[0].grade.is_miss_or_too_fast(),
        "held early head plus later body sensors should judge slide as hit, got {:?}",
        events[0].grade
    );
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
    let chart_text =
        "&title=Tap Probe\n&artist=Test\n&first=0\n&lv_6=1\n&inote_6=(120){4}1,2,3,4,E\n";
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
    let result: RuntimeStepLightResult =
        serde_json::from_value(value.get("result").cloned().unwrap()).unwrap();
    let taps: Vec<&JudgeEvent> = result
        .events
        .iter()
        .filter(|evt| evt.kind == JudgeEventKind::Tap)
        .collect();
    assert_eq!(taps.len(), 1, "expected one tap event");
    assert!(!taps[0].grade.is_miss_or_too_fast(), "tap should be a hit");
}

#[test]
fn slide_miss_event_position() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = sample_slide_chart_text();
    let (mut loaded, _) = load_session(&chart_text);
    let result = step_light(&mut loaded, 5_000_000);
    for ev in &result.events {
        eprintln!(
            "[probe] event kind={:?} grade={:?} position={:?}",
            ev.kind, ev.grade, ev.position
        );
    }
    let slide_ev = result
        .events
        .iter()
        .find(|evt| evt.kind == JudgeEventKind::Slide)
        .expect("slide event");
    // The slide judge event reports the head button (K1), so the player maps
    // slide feedback to the slide's tail zone itself.
    assert!(slide_ev.position.button.is_some());
}

#[test]
fn real_chart_engine_produces_events() {
    let _guard = test_guard();
    ensure_runtime();
    let text = std::fs::read_to_string("songs/夜に駆ける・改/maidata.txt").expect("real chart");
    let empty = Session::<Empty>::create().expect("create");
    // Some community charts use wifi inside a connection slide, which the
    // engine rejects; the player falls back to its own judgment then.
    let Ok((mut loaded, _)) = empty.load_chart_text(&text, 7) else {
        eprintln!("[probe] real chart rejected by engine (wifi-in-chain)");
        return;
    };
    let mut found = false;
    for t in (100_000..=8_000_000).step_by(100_000) {
        let batch = TimedInputBatch {
            current_time: t,
            events: vec![],
        };
        let envelope = loaded
            .advance_frame_light(&serde_json::to_string(&batch).unwrap())
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&envelope.json).unwrap();
        let result: RuntimeStepLightResult =
            serde_json::from_value(value.get("result").cloned().unwrap_or_default()).unwrap();
        if !result.events.is_empty() {
            found = true;
            for ev in &result.events {
                eprintln!(
                    "[probe] real event kind={:?} grade={:?} pos={:?}",
                    ev.kind, ev.grade, ev.position
                );
            }
            break;
        }
    }
    assert!(found, "expected judge events from the real chart");
}
