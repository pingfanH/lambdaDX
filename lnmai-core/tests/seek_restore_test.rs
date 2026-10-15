use lnmai_core::session::{Empty, Loaded, Session};
use lnmai_core::types::{RenderCommand, TimedInputBatch, TimedInputEvent};

fn event_time(event: &TimedInputEvent) -> i64 {
    match event {
        TimedInputEvent::ButtonClick { tp, .. }
        | TimedInputEvent::ButtonHold { tp, .. }
        | TimedInputEvent::SensorClick { tp, .. }
        | TimedInputEvent::SensorHold { tp, .. } => *tp,
    }
}

fn state(session: &Session<Loaded>) -> serde_json::Value {
    let envelope = session.get_state_json().unwrap();
    serde_json::from_str::<serde_json::Value>(&envelope.json).unwrap()["result"].clone()
}

#[test]
fn crossing_state_matches_playback_without_replaying_unrelated_history() {
    for (note, offset) in [
        ("1h[1:2]", 1_000_000),
        ("C1h[1:2]", 1_000_000),
        ("1-5[1:2]", 1_000_000),
        ("1-5[1:1]-8[1:1]", 3_500_000),
        ("1w5[1:2]", 1_000_000),
        ("1h[1:2]/2h[1:2]/B3h[1:2]/B4h[1:2]", 1_000_000),
        ("1/1h[1:2]", 1_000_000),
    ] {
        let source = format!("&inote_6=(120){{4}}{}{},E\n", "1/B3,".repeat(16), note);
        let load = || {
            Session::<Empty>::create_native()
                .unwrap()
                .load_chart_text_native(&source, 6)
                .unwrap()
        };
        let mut seek = load();
        let mut playback = load();
        let events = seek.autoplay_plan().unwrap().events();
        let target = 8_000_000 + offset;
        for event in events.iter().filter(|event| event_time(event) < target) {
            playback
                .step_native(&TimedInputBatch {
                    current_time: event_time(event),
                    events: vec![event.clone()],
                })
                .unwrap();
        }
        playback
            .step_native(&TimedInputBatch {
                current_time: target - 1,
                events: vec![],
            })
            .unwrap();
        let reset = seek.seek_with_events(target, &events, true).unwrap();
        assert!(
            reset
                .render_commands
                .iter()
                .all(|command| !matches!(command, RenderCommand::ShowJudgeResult { .. }))
        );
        let expected = state(&playback);
        let actual = state(&seek);
        for field in [
            "activeHolds",
            "activeTouchHolds",
            "prevButton",
            "prevSensor",
        ] {
            assert_eq!(actual[field], expected[field], "{note}: {field}");
        }
        for slide in expected["slides"].as_array().unwrap() {
            let id = &slide["params"]["noteIndex"];
            let restored = actual["slides"]
                .as_array()
                .unwrap()
                .iter()
                .find(|candidate| candidate["params"]["noteIndex"] == *id)
                .unwrap();
            if slide["startTiming"].as_i64().unwrap() + slide["length"].as_i64().unwrap() < target {
                // Past chain parts must not emit their pending result after reset.
                assert_eq!(restored["state"], "Ended", "{note}: expired slide {id}");
                assert!(
                    restored["judgeQueues"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|queue| queue.as_array().unwrap().is_empty())
                );
            } else {
                assert_eq!(restored, slide, "{note}: crossing slide {id}");
            }
        }
    }
}

#[test]
fn exact_hold_tail_is_scored_by_the_resumed_frame() {
    for note in ["1h[1:2]", "C1h[1:2]"] {
        let source = format!("&inote_6=(120){{4}}{note},E\n");
        let mut session = Session::<Empty>::create_native()
            .unwrap()
            .load_chart_text_native(&source, 6)
            .unwrap();
        let chart = session.chart_spec_native().unwrap();
        let target = chart
            .holds
            .first()
            .map(|hold| hold.timing + hold.length)
            .or_else(|| {
                chart
                    .touch_holds
                    .first()
                    .map(|hold| hold.timing + hold.length)
            })
            .unwrap();
        let events = session.autoplay_plan().unwrap().events();
        let reset = session.seek_with_events(target, &events, true).unwrap();
        assert_eq!(reset.score.earned_base, 0);
        assert!(reset.score.total_base > 0);
        let result = session
            .step_native(&TimedInputBatch {
                current_time: target,
                events: events
                    .into_iter()
                    .filter(|event| event_time(event) == target)
                    .collect(),
            })
            .unwrap();
        assert_eq!(result.score.lost_base, 0, "{note}");
        assert_eq!(result.score.earned_base, result.score.total_base, "{note}");
    }
}
