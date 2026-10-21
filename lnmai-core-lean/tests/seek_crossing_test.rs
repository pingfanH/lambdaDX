use lnmai_core::session::{Empty, Session};
use lnmai_core::types::{TimedInputBatch, TimedInputEvent};

#[test]
fn seek_retains_crossing_long_notes_and_session_identity() {
    for simai in ["1h[1:2]", "C1h[1:2]", "1-5[1:2]", "1-5[1:1]-8[1:1]"] {
        let source = format!("&lv_6=1\n&inote_6=(120){{4}}{simai},E\n");
        let mut session = Session::<Empty>::create_native()
            .unwrap()
            .load_chart_text_native(&source, 6)
            .unwrap();
        let handle = session.handle();
        let chart = session.chart_spec_native().unwrap();
        let id = chart
            .holds
            .first()
            .map(|n| n.note_index)
            .or_else(|| chart.touch_holds.first().map(|n| n.note_index))
            .or_else(|| chart.slides.first().map(|n| n.note_index))
            .unwrap();
        let result = session.seek(1_000_000).unwrap();
        assert!(
            result.retained_note_indices.contains(&id),
            "{simai}: crossing body was discarded"
        );
        assert_eq!(handle, session.handle());
        assert_eq!(result.score.earned_base, 0);
        assert_eq!(result.score.lost_base, 0);
        assert!(!session.seek(0).unwrap().retained_note_indices.is_empty());
    }
}

#[test]
fn autoplay_continues_crossing_bodies_after_seek() {
    for simai in ["1h[1:2]", "C1h[1:2]", "1-5[1:2]", "1-5[1:1]-8[1:1]"] {
        let source = format!("&inote_6=(120){{4}}{simai},E\n");
        let mut session = Session::<Empty>::create_native()
            .unwrap()
            .load_chart_text_native(&source, 6)
            .unwrap();
        let events = session.autoplay_plan().unwrap().events();
        let target = if simai == "1-5[1:1]-8[1:1]" {
            3_500_000
        } else {
            500_000
        };
        let reset = session.seek_with_events(target, &events, true).unwrap();
        assert_eq!(reset.score.earned_base, 0);
        for event in events {
            let tp = match &event {
                TimedInputEvent::ButtonClick { tp, .. }
                | TimedInputEvent::ButtonHold { tp, .. }
                | TimedInputEvent::SensorClick { tp, .. }
                | TimedInputEvent::SensorHold { tp, .. } => *tp,
            };
            if tp >= target {
                session
                    .step_native(&TimedInputBatch {
                        current_time: tp,
                        events: vec![event],
                    })
                    .unwrap();
            }
        }
        session
            .step_native(&TimedInputBatch {
                current_time: 10_000_000,
                events: vec![],
            })
            .unwrap();
        // Slide judgments emit on the frame after their result timer expires.
        let end = session
            .step_native(&TimedInputBatch {
                current_time: 10_016_667,
                events: vec![],
            })
            .unwrap();
        assert_eq!(
            end.score.lost_base, 0,
            "{simai}: autoplay missed after seek"
        );
        assert_eq!(
            end.score.earned_base, end.score.total_base,
            "{simai}: remaining score mismatch"
        );
    }
}

#[test]
fn seek_past_finished_slide_does_not_score_its_pending_result() {
    for simai in ["1-5[1:2]", "1-5[1:1]-8[1:1]"] {
        let source = format!("&inote_6=(120){{4}}{simai},E\n");
        let mut session = Session::<Empty>::create_native()
            .unwrap()
            .load_chart_text_native(&source, 6)
            .unwrap();
        let reset = session.seek(10_000_000).unwrap();
        assert!(reset.retained_note_indices.is_empty());
        for time in [10_016_667, 11_000_000] {
            let result = session
                .step_native(&TimedInputBatch {
                    current_time: time,
                    events: vec![],
                })
                .unwrap();
            assert!(
                result.events.is_empty(),
                "{simai}: emitted a pre-seek judgment"
            );
            assert_eq!(result.score.earned_base, 0, "{simai}");
            assert_eq!(result.score.lost_base, 0, "{simai}");
        }
    }
}
