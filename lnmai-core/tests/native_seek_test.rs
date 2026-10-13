use lnmai_core::session::{Empty, Session};
use lnmai_core::types::{ChartSpec, RuntimeStepLightResult, TimedInputBatch};

#[test]
fn native_transport_matches_json_and_seek_is_repeatable() {
    let source = "&title=Native\n&artist=Test\n&first=0\n&lv_6=1\n&inote_6=(120){4}1,2,3h[4:2],A1,C1h[4:2],1-3[4:1],E\n";
    let mut native = Session::<Empty>::create_native()
        .unwrap()
        .load_chart_text_native(source, 6)
        .unwrap();
    let (mut legacy, _) = Session::<Empty>::create()
        .unwrap()
        .load_chart_text(source, 6)
        .unwrap();
    let chart = native.chart_spec_native().unwrap();
    let json_chart: ChartSpec = legacy
        .get_lowered_chart_json()
        .unwrap()
        .decode_result()
        .unwrap();
    assert_eq!(chart.taps, json_chart.taps);
    assert_eq!(chart.holds, json_chart.holds);
    assert_eq!(chart.touches, json_chart.touches);
    assert_eq!(chart.touch_holds, json_chart.touch_holds);
    assert_eq!(chart.slide_heads, json_chart.slide_heads);
    let mut json_slides = json_chart.slides.clone();
    for slide in &mut json_slides {
        slide.debug_simai = None;
    }
    assert_eq!(chart.slides, json_slides);
    let plan = native.autoplay_plan().unwrap();
    assert!(!plan.groups.is_empty());
    for group in &plan.groups {
        for id in &group.runtime_note_indices {
            assert_eq!(plan.group_for_note(*id).unwrap().id, group.id);
        }
    }
    let mut events = plan.events();
    events.retain(|event| match event {
        lnmai_core::types::TimedInputEvent::ButtonClick { tp, .. }
        | lnmai_core::types::TimedInputEvent::ButtonHold { tp, .. }
        | lnmai_core::types::TimedInputEvent::SensorClick { tp, .. }
        | lnmai_core::types::TimedInputEvent::SensorHold { tp, .. } => *tp <= 2_000_000,
    });
    let batch = TimedInputBatch {
        current_time: 2_000_000,
        events,
    };
    let direct = native.step_native(&batch).unwrap();
    let encoded: RuntimeStepLightResult = legacy
        .advance_frame_light(&serde_json::to_string(&batch).unwrap())
        .unwrap()
        .decode_result()
        .unwrap();
    assert_eq!(direct, encoded);
    assert!(
        native
            .step_native(&TimedInputBatch {
                current_time: 0,
                events: vec![]
            })
            .is_err()
    );
    let reset = native.seek(0).unwrap();
    assert_eq!(reset.current_time, 0);
    assert_eq!(reset.score.combo, 0);
    assert_eq!(reset.score.earned_base, 0);
    assert_eq!(reset.score.lost_base, 0);
    assert_eq!(reset.score.dx_score, 0);
    assert_eq!(reset, native.seek(0).unwrap());
    assert!(native.seek(-1).is_err());
    assert_eq!(direct, native.step_native(&batch).unwrap());
    let segment = native.seek(300_000).unwrap();
    assert!(!segment.skipped_note_indices.is_empty());
    assert!(segment.score.max_dx_score < reset.score.max_dx_score);
    for group in plan.retained(&segment.retained_note_indices).groups {
        assert!(
            group
                .runtime_note_indices
                .iter()
                .all(|id| segment.retained_note_indices.contains(id))
        );
    }
    let end = native.seek(20_000_000).unwrap();
    assert!(end.retained_note_indices.is_empty());
    assert_eq!(end.score.max_dx_score, 0);

    // Rebuild queue frontiers as well as totals: the retained same-lane taps
    // must remain judgeable after their earlier queue entries are removed.
    let source = "&title=Queue\n&lv_6=1\n&inote_6=(120){4}1,1,1,E\n";
    let mut queued = Session::<Empty>::create_native()
        .unwrap()
        .load_chart_text_native(source, 6)
        .unwrap();
    let queue_chart = queued.chart_spec_native().unwrap();
    let target = queue_chart.taps[1].timing;
    let plan = queued.autoplay_plan().unwrap();
    let reset = queued.seek(target).unwrap();
    assert_eq!(reset.retained_note_indices.len(), 2);
    assert_eq!(reset.score.max_dx_score, 6);
    let events = plan.retained(&reset.retained_note_indices).events();
    for event in events {
        let time = match &event {
            lnmai_core::types::TimedInputEvent::ButtonClick { tp, .. }
            | lnmai_core::types::TimedInputEvent::ButtonHold { tp, .. }
            | lnmai_core::types::TimedInputEvent::SensorClick { tp, .. }
            | lnmai_core::types::TimedInputEvent::SensorHold { tp, .. } => *tp,
        };
        let result = queued
            .step_native(&TimedInputBatch {
                current_time: time,
                events: vec![event],
            })
            .unwrap();
        assert!(
            result
                .events
                .iter()
                .all(|event| reset.retained_note_indices.contains(&event.note_index))
        );
    }
    let finished = queued
        .step_native(&TimedInputBatch {
            current_time: 5_000_000,
            events: vec![],
        })
        .unwrap();
    assert_eq!(finished.score.combo, 2);
    assert_eq!(finished.score.lost_base, 0);
    assert_eq!(finished.score.earned_base, finished.score.total_base);

    // A long note crossing the target is skipped whole, not re-entered halfway.
    for (id, time) in chart
        .holds
        .iter()
        .map(|note| (note.note_index, note.timing))
        .chain(
            chart
                .touch_holds
                .iter()
                .map(|note| (note.note_index, note.timing)),
        )
    {
        let crossing = native.seek(time + 1).unwrap();
        assert!(crossing.skipped_note_indices.contains(&id));
        assert!(!crossing.retained_note_indices.contains(&id));
    }

    // Connected chains and wifi tracks must stay in one ownership group.
    for simai in ["1-3[4:1]-5[4:1]", "1w5[4:1]"] {
        let source = format!("&title=Group\n&lv_6=1\n&inote_6=(120){{4}}{simai},E\n");
        let mut session = Session::<Empty>::create_native()
            .unwrap()
            .load_chart_text_native(&source, 6)
            .unwrap();
        let chart = session.chart_spec_native().unwrap();
        let plan = session.autoplay_plan().unwrap();
        let slide_group = plan.group_for_note(chart.slides[0].note_index).unwrap();
        assert_eq!(slide_group.kind, 4);
        for slide in &chart.slides {
            assert_eq!(
                plan.group_for_note(slide.note_index).unwrap().id,
                slide_group.id
            );
        }
        plan.validate_for(&plan).unwrap();
        let target = chart.slides[0].head_timing + 1;
        let seek = session.seek(target.max(0)).unwrap();
        assert!(
            chart
                .slides
                .iter()
                .all(|slide| seek.skipped_note_indices.contains(&slide.note_index))
        );
        assert!(plan.retained(&seek.retained_note_indices).groups.is_empty());
        assert!(!session.seek(0).unwrap().retained_note_indices.is_empty());
    }
}
