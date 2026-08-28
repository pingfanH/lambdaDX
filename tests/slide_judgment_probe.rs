use std::sync::{Mutex, OnceLock};

use lambda_dx::simai_io;
use lambda_dx::types::zone::PadZone;
use lambda_dx::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
};
use lnmai_core_ffi::api;
use lnmai_core_ffi::session::{self, Empty, Session};
use lnmai_core_ffi::types::{JudgeEvent, JudgeEventKind, JudgeGrade, TimedInputBatch};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn ensure_runtime() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| unsafe { session::initialize_runtime().unwrap() });
}

fn sample_slide_chart_text() -> String {
    let chart = ChartDoc {
        version: "1.0".to_string(),
        title: "slide-probe".to_string(),
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

#[test]
fn parsed_slide_chart_has_real_runtime_queues() {
    let _guard = test_guard();
    ensure_runtime();

    let chart_text = sample_slide_chart_text();
    eprintln!("[probe] exported simai:\n{chart_text}");

    let lowered = api::parse_lowered_chart(&chart_text, 6).unwrap();
    eprintln!(
        "[probe] parsed lowered: slide_heads={} slides={} first_queue_tracks={} first_total_queue_len={}",
        lowered.slide_heads.len(),
        lowered.slides.len(),
        lowered
            .slides
            .first()
            .map(|slide| slide.judge_queues.len())
            .unwrap_or(0),
        lowered
            .slides
            .first()
            .map(|slide| slide.total_judge_queue_len)
            .unwrap_or(0),
    );

    assert_eq!(lowered.slide_heads.len(), 1);
    assert_eq!(lowered.slides.len(), 1);
    assert!(!lowered.slides[0].judge_queues.is_empty());
    assert!(lowered.slides[0].total_judge_queue_len > 0);
    assert!(lowered.slides[0].track_count > 0);
}

#[test]
fn parsed_slide_body_fails_without_sensor_input() {
    let _guard = test_guard();
    ensure_runtime();

    let chart_text = sample_slide_chart_text();
    let lowered = api::parse_lowered_chart(&chart_text, 6).unwrap();
    let slide = &lowered.slides[0];
    let too_late_time = slide.start_timing + slide.length + 700_000;

    let empty = Session::<Empty>::create().unwrap();
    let (mut loaded, _) = empty.load_chart_text(&chart_text, 6).unwrap();

    let step0 = loaded
        .advance_frame_light(
            &serde_json::to_string(&TimedInputBatch {
                current_time: 0,
                events: vec![],
            })
            .unwrap(),
        )
        .unwrap();
    eprintln!("[probe] parsed no-input step0 json={}", step0.json);

    let step1 = loaded
        .advance_frame_light(
            &serde_json::to_string(&TimedInputBatch {
                current_time: too_late_time,
                events: vec![],
            })
            .unwrap(),
        )
        .unwrap();
    eprintln!("[probe] parsed no-input step1 json={}", step1.json);

    let step1_json: serde_json::Value = serde_json::from_str(&step1.json).unwrap();
    let events: Vec<JudgeEvent> = serde_json::from_value(
        step1_json
            .get("result")
            .and_then(|result| result.get("events"))
            .cloned()
            .unwrap_or_default(),
    )
    .unwrap();

    let slide_events: Vec<_> = events
        .iter()
        .filter(|evt| evt.kind == JudgeEventKind::Slide)
        .collect();
    assert_eq!(slide_events.len(), 1);
    assert_eq!(slide_events[0].grade, JudgeGrade::Miss);
}

#[test]
fn empty_slide_queues_auto_judge_perfect_without_input() {
    let _guard = test_guard();
    ensure_runtime();

    let chart_text = sample_slide_chart_text();
    let mut lowered = api::parse_lowered_chart(&chart_text, 6).unwrap();
    assert_eq!(lowered.slides.len(), 1);

    lowered.slide_heads.clear();
    lowered.slides[0].judge_queues.clear();
    lowered.slides[0].total_judge_queue_len = 0;
    lowered.slides[0].track_count = 0;

    eprintln!(
        "[probe] mutated lowered: slide_heads={} slides={} first_queue_tracks={} first_total_queue_len={}",
        lowered.slide_heads.len(),
        lowered.slides.len(),
        lowered.slides[0].judge_queues.len(),
        lowered.slides[0].total_judge_queue_len,
    );

    let slide = &lowered.slides[0];
    let judge_time = slide.judge_at.unwrap_or(slide.start_timing + slide.length);
    let chart_json = serde_json::to_string(&lowered).unwrap();
    let empty = Session::<Empty>::create().unwrap();
    let (mut loaded, _) = empty.load_chart_json(&chart_json).unwrap();

    let step0 = loaded
        .advance_frame_light(
            &serde_json::to_string(&TimedInputBatch {
                current_time: judge_time,
                events: vec![],
            })
            .unwrap(),
        )
        .unwrap();
    let step0_json: serde_json::Value = serde_json::from_str(&step0.json).unwrap();
    eprintln!("[probe] step0 json={step0_json}");

    let step1 = loaded
        .advance_frame_light(
            &serde_json::to_string(&TimedInputBatch {
                current_time: judge_time + 1,
                events: vec![],
            })
            .unwrap(),
        )
        .unwrap();
    let step1_json: serde_json::Value = serde_json::from_str(&step1.json).unwrap();
    eprintln!("[probe] step1 json={step1_json}");

    let slide_events: Vec<JudgeEvent> = serde_json::from_value(
        step1_json
            .get("result")
            .and_then(|result| result.get("events"))
            .cloned()
            .unwrap_or_default(),
    )
    .unwrap();
    let slide_events: Vec<_> = slide_events
        .into_iter()
        .filter(|evt| evt.kind == JudgeEventKind::Slide)
        .collect();
    assert_eq!(slide_events.len(), 1);
    assert_eq!(slide_events[0].grade, JudgeGrade::Perfect);
}
