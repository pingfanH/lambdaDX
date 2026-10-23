use super::*;
use crate::app::types::{Mode, Note};
use lambdadx_automation::agent::cursor;
use serde_json::json;
use std::time::{Duration, Instant};

fn fixture() -> (Service, EditorState, PadPreviewState, crate::runtime::ChartSync) {
    let mut chart = crate::app::chart::fallback_chart();
    chart.notes = vec![Note { id: 1, time: 1.0, lane: 1, hi_speed: 1.0, ..Default::default() }];
    chart.templates.clear();
    chart.template_instances.clear();
    chart.autoplay_overrides.clear();
    let sync = crate::runtime::ChartSync::new(&mut chart);
    let text = format!("|| preview preserves source\r\n{}", crate::io::simai::editable_body(&chart).unwrap());
    let mut editor = EditorState::default();
    editor.simai_editor.load(&chart, text);
    editor.keyboard.cursor.measure = 4.0;
    editor.snap_division = 16;
    editor.selected.insert(1);
    let mut app = PadPreviewState::new_at(chart, None, None, 0.0, None);
    app.forced_time = Some(0.0);
    (Service::new(), editor, app, sync)
}

fn generate(service: &mut Service, editor: &mut EditorState, app: &mut PadPreviewState, sync: &mut crate::runtime::ChartSync) -> String {
    let client = service.client();
    let context = service.prepare_cursor(&client, editor, app).unwrap();
    let request = cursor::proposal(json!({
        "operation_id": "cursor-preview", "summary": "four-note pattern",
        "notes": [
            {"note_type":"tap", "offset_measures":0, "ring_lane":3},
            {"note_type":"hold", "offset_measures":0.25, "ring_lane":4, "properties":{"hold_duration_measures":0.25}},
            {"note_type":"touch", "offset_measures":0.5, "touch_zone":"B1"},
            {"note_type":"slide", "offset_measures":0.75, "ring_lane":1, "properties":{"slides":[{"delay_measures":0.25, "motion_duration_measures":0.5, "is_break":false, "segments":[{"shape":"line", "points":[{"zone":"A5", "stored_beat_offset":0}]}]}]}}
        ]
    }), &context).unwrap();
    let result = service.proposals.create(request, client.session(), &service.snapshots, editor, app, true).unwrap();
    let start = Instant::now();
    loop {
        service.maintenance(editor, app, sync, true);
        if app.has_chart_preview() { break; }
        let status = service.proposals.status(&result.proposal_id, &client.session()).unwrap();
        assert!(status.error.is_none(), "{:?}", status.error);
        assert!(start.elapsed() < Duration::from_secs(5), "preview stalled: {:?}", status.state);
        std::thread::sleep(Duration::from_millis(1));
    }
    result.proposal_id
}

#[test]
fn cursor_preview_is_playable_but_not_part_of_document_or_history() {
    let (mut service, mut editor, mut app, mut sync) = fixture();
    let before = app.chart.clone();
    let source = editor.simai_editor.text.clone();
    let cursor = editor.keyboard.cursor.measure;
    let selection = editor.selected.clone();
    generate(&mut service, &mut editor, &mut app, &mut sync);
    assert_eq!(app.chart, before);
    assert_eq!(editor.simai_editor.text, source);
    assert!(editor.simai_editor.automation_source(&app.chart).1);
    assert!(!editor.dirty);
    assert!(editor.undo.is_empty() && editor.redo.is_empty());
    assert_eq!(editor.keyboard.cursor.measure, cursor);
    assert_eq!(editor.selected, selection);
    assert_eq!(app.preview_notes().len(), 4);
    assert_eq!(app.playback_chart().notes.len(), 5);
    assert_eq!(crate::templates::document(&editor, &app.chart), before, "save uses canonical document");
    service.reject_requested = true;
    app.mode = Mode::Playing;
    service.maintenance(&mut editor, &mut app, &mut sync, true);
    assert!(!app.has_chart_preview());
    assert_eq!(app.playback_chart(), &before);
    assert_eq!(app.chart, before);
    assert_eq!(editor.simai_editor.text, source);
    assert!(!editor.dirty);
    assert!(editor.undo.is_empty());
    assert!(service.proposals.pending().is_none());
}

#[test]
fn single_response_generation_reaches_ghost_preview_and_rejects_stale_documents() {
    use lambdadx_automation::agent::{self, Message, Run, State, provider::{Completion, Provider, Turn}};
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    struct Once(Arc<AtomicUsize>);
    impl Provider for Once {
        fn complete<'a>(&'a mut self, request: agent::ChatRequest, _: &'a agent::Emit<'a>) -> Completion<'a> {
            Box::pin(async move {
                assert_eq!(self.0.fetch_add(1, Ordering::SeqCst), 0);
                assert!(request.tools.is_none());
                assert!(format!("{request:?}").contains("左右手交替"));
                Ok(Turn { message: Message::assistant(json!({"summary":"four-hit interaction", "notes":[
                    {"offset_measures":0,"ring_lane":3}, {"offset_measures":0.0625,"ring_lane":6},
                    {"offset_measures":0.125,"ring_lane":3}, {"offset_measures":0.1875,"ring_lane":6}
                ]}).to_string()), usage: None })
            })
        }
    }
    for stale in [false, true] {
        let (mut service, mut editor, mut app, mut sync) = fixture();
        let client = service.client().scoped();
        let context = service.prepare_cursor(&client, &mut editor, &app).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let run = Run::start_at_cursor(&service.background().unwrap(), Once(calls.clone()), client,
            vec![Message::user("生成4个交互")], Permissions { read: true, focus: false, propose: true }, context);
        if stale { editor.place_note(&mut app.chart, 8.0, 8); }
        let before = app.chart.clone();
        let source = editor.simai_editor.text.clone();
        let start = Instant::now();
        let state = loop {
            service.tick(&mut editor, &app, true, true);
            service.maintenance(&mut editor, &mut app, &mut sync, true);
            if let Ok(agent::Event::Finished { state, error, .. }) = run.finished.try_recv() {
                if stale { assert_eq!(error.unwrap().code, ErrorCode::DocumentChanged); }
                else { assert!(error.is_none()); }
                break state;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(state, if stale { State::Failed } else { State::AwaitingApproval });
        if stale { assert!(!app.has_chart_preview()); }
        else {
            while !app.has_chart_preview() {
                service.maintenance(&mut editor, &mut app, &mut sync, true);
                assert!(start.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(app.preview_notes().len(), 4);
            assert_eq!(app.playback_chart().notes.len(), 5);
            assert_eq!(app.preview_notes()[0].time, 4.0);
        }
        assert_eq!(app.chart, before);
        assert_eq!(editor.simai_editor.text, source);
    }
}

#[test]
fn accept_selects_advances_and_undo_redo_preserves_source() {
    let (mut service, mut editor, mut app, mut sync) = fixture();
    let before = app.chart.clone();
    let source = editor.simai_editor.text.clone();
    let id = generate(&mut service, &mut editor, &mut app, &mut sync);
    let added: std::collections::HashSet<_> = app.preview_notes().iter().map(|n| n.id).collect();
    let last = app.preview_notes().iter().map(|n| n.time).max_by(f32::total_cmp).unwrap();
    app.mode = Mode::Playing;
    assert_eq!(service.proposals.approve(&mut editor, &mut app, &mut service.snapshots, true, false).unwrap_err().code, ErrorCode::EditorBusy);
    assert!(app.has_chart_preview());
    app.mode = Mode::Idle;
    service.approve_requested = true;
    service.maintenance(&mut editor, &mut app, &mut sync, true);
    assert!(!app.has_chart_preview());
    assert_eq!(app.chart.notes.len(), 5);
    assert_eq!(editor.selected, added);
    assert_eq!(editor.keyboard.cursor.measure, last + 1.0 / 16.0);
    assert!(editor.dirty);
    assert_eq!(editor.undo.len(), 1);
    assert_eq!(service.proposals.status(&id, &service.client.session()).unwrap().state, ProposalState::Applied);
    let after = app.chart.clone();
    let after_text = editor.simai_editor.text.clone();
    assert!(after_text.contains("|| preview preserves source\r\n"));
    editor.undo(&mut app.chart);
    assert_eq!(app.chart, before);
    assert_eq!(editor.simai_editor.text, source);
    editor.redo(&mut app.chart);
    assert_eq!(app.chart, after);
    assert_eq!(editor.simai_editor.text, after_text);
}

#[test]
fn document_edits_drafts_stop_and_reload_remove_only_the_preview() {
    for reason in ["chart", "draft", "stop", "reload"] {
        let (mut service, mut editor, mut app, mut sync) = fixture();
        generate(&mut service, &mut editor, &mut app, &mut sync);
        match reason {
            "chart" => { editor.place_note(&mut app.chart, 7.0, 8); }
            "draft" => { editor.simai_editor.text.push_str("\n|| new user draft"); editor.simai_editor.changed(); }
            "stop" => service.client.session().cancel(),
            "reload" => { app.load_chart(app.chart.clone(), None, None, None, None); }
            _ => unreachable!(),
        }
        let expected = app.chart.clone();
        let text = editor.simai_editor.text.clone();
        service.maintenance(&mut editor, &mut app, &mut sync, true);
        assert!(!app.has_chart_preview(), "{reason}");
        assert!(service.proposals.pending().is_none(), "{reason}");
        assert_eq!(app.chart, expected, "{reason}");
        assert_eq!(editor.simai_editor.text, text, "{reason}");
    }
}
