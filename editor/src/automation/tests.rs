use super::*;
use crate::app::types::{self, Note as ChartNote, NoteType};
use lambdadx_automation::bridge::Pending;
use serde_json::json;
use std::time::{Duration, Instant};

fn fixture() -> (Service, EditorState, PadPreviewState) {
    let mut chart = crate::app::chart::fallback_chart();
    chart.notes = vec![
        ChartNote {
            id: 12,
            time: 2.0,
            lane: 3,
            ..Default::default()
        },
        ChartNote {
            id: 11,
            time: 1.0,
            lane: 1,
            ..Default::default()
        },
        ChartNote {
            id: 10,
            time: 2.0,
            lane: 17,
            note_type: NoteType::TouchHold,
            hold_duration: 0.5,
            ..Default::default()
        },
    ];
    chart.templates.clear();
    chart.template_instances.clear();
    chart.autoplay_overrides.clear();
    chart.bpm = 120.0;
    chart.bpms = vec![types::BpmChange {
        measure: 1.0,
        bpm: 120.0,
    }];
    let mut editor = EditorState::default();
    editor.source = Some(crate::io::ChartSource {
        path: "/unused/maidata.txt".into(),
        level: Some(5),
    });
    editor.selected.extend([11, 12]);
    editor.auto_save = true;
    editor.simai_editor.load(&chart, "(120){4}1,2,3,E".into());
    let mut app = PadPreviewState::new_at(chart, None, None, 0.0, None);
    app.forced_time = Some(0.75);
    app.timeline_view_time = 0.75;
    (Service::new(), editor, app)
}
fn enqueue(service: &Service, name: &str, args: Value) -> Pending {
    service
        .client()
        .call(
            name,
            args,
            Permissions {
                read: true,
                focus: true,
                propose: false,
            },
            Duration::from_secs(30),
        )
        .unwrap()
}
fn call(
    service: &mut Service,
    editor: &mut EditorState,
    app: &PadPreviewState,
    name: &str,
    args: Value,
) -> ToolResult<Value> {
    let pending = enqueue(service, name, args);
    service.tick(editor, app, true, false);
    pending.poll().expect("one request completed in this frame")
}
fn context(service: &mut Service, editor: &mut EditorState, app: &PadPreviewState) -> Context {
    serde_json::from_value(
        call(
            service,
            editor,
            app,
            "chart_get_context",
            json!({"schema_version":1}),
        )
        .unwrap(),
    )
    .unwrap()
}
fn note_args(target: &SnapshotRef, offset: usize, limit: usize) -> Value {
    json!({"schema_version":1,"target":target,"offset":offset,"limit":limit})
}
#[test]
fn graphical_delete_without_simai_pane_allows_agent_proposal() {
    let (mut service, mut editor, mut app) = fixture();
    let text = "|| 保留注释\r\n(120){4}1,2,3,E\r\n";
    app.chart = crate::simai_editor::parse_draft(text, 120.0, 5).unwrap();
    crate::app::maichart::assign_note_ids(&mut app.chart.notes);
    let mut sync = crate::runtime::ChartSync::new(&mut app.chart);
    editor.simai_editor.load(&app.chart, text.into());
    editor.auto_save = false;
    let deleted = app.chart.notes[0].id;
    let remaining = app.chart.notes[1].id;
    let old = context(&mut service, &mut editor, &app);

    // Use the same edit operation as the chart canvas, without drawing Simai.
    editor.delete_note(&mut app.chart, deleted);
    assert!(!editor.simai_editor.modified());
    service.maintenance(&mut editor, &mut app, &mut sync, true);
    let ctx = context(&mut service, &mut editor, &app);
    let source = editor.simai_editor.text.clone();
    let generation = editor.simai_editor.generation;
    assert!(context(&mut service, &mut editor, &app).simai_matches_chart);
    assert_eq!(editor.simai_editor.text, source, "repeated reads do not rewrite source");
    assert_eq!(editor.simai_editor.generation, generation);
    let old_source: SimaiSlice = serde_json::from_value(call(
        &mut service, &mut editor, &app, "chart_read_simai",
        json!({"schema_version":1,"target":old.target,
            "range":{"start_char":0,"end_char":text.chars().count()}}),
    ).unwrap()).unwrap();
    assert_eq!(old_source.text, text, "old snapshots stay immutable");
    let stale: ProposeRequest = serde_json::from_value(json!({
        "schema_version":1,"target":old.target,"operation_id":"stale-delete","summary":"Stale proposal",
        "edits":[{"kind":"delete_notes","note_ids":[remaining.to_string()]}],
    })).unwrap();
    let error = service.proposals.create(
        stale, service.client.session(), &service.snapshots, &editor, &app, true,
    ).unwrap_err();
    assert_eq!(error.code, ErrorCode::DocumentChanged);
    let pending = service.client().call(
        "chart_propose_edit",
        json!({"schema_version":1,"target":ctx.target,"operation_id":"after-delete",
            "summary":"Delete another explicitly selected note",
            "edits":[{"kind":"delete_notes","note_ids":[remaining.to_string()]}]}),
        Permissions { read: true, focus: false, propose: true },
        Duration::from_secs(30),
    ).unwrap();
    service.maintenance(&mut editor, &mut app, &mut sync, true);
    service.tick(&mut editor, &app, true, false);
    let result = pending.poll().expect("proposal request completed");
    assert!(result.is_ok(), "proposal after graphical deletion: {result:?}");
    assert!(ctx.simai_matches_chart);
    assert!(!ctx.unapplied_draft);
    assert!(!ctx.draft_conflict);
    assert!(editor.simai_editor.error.is_none());
    assert!(editor.simai_editor.text.contains("|| 保留注释\r\n"));
    let parsed = crate::simai_editor::parse_draft(&editor.simai_editor.text, 120.0, 5).unwrap();
    assert!(crate::simai_editor::format::equivalent(&parsed, &app.chart));
    assert_eq!(app.chart.notes.len(), 2, "proposal has not been applied");
    let start = Instant::now();
    loop {
        service.maintenance(&mut editor, &mut app, &mut sync, true);
        let proposal = service.proposals.pending().expect("proposal remains reviewable");
        if proposal.result.state == ProposalState::AwaitingApproval { break; }
        assert!(start.elapsed() < Duration::from_secs(5), "preflight timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(editor.undo.len(), 1, "only the graphical deletion is in history");
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.notes.len(), 3);
    assert_eq!(editor.simai_editor.text, text, "undo restores exact source");
}
#[test]
fn context_preserves_simai_draft_after_graphical_delete_and_proposals_stay_blocked() {
    let (mut service, mut editor, mut app) = fixture();
    let text = "(120){4}1,2,3,E";
    app.chart = crate::simai_editor::parse_draft(text, 120.0, 5).unwrap();
    crate::app::maichart::assign_note_ids(&mut app.chart.notes);
    editor.simai_editor.load(&app.chart, text.into());
    editor.simai_editor.text.push_str(" || 未应用草稿\r\n");
    editor.simai_editor.changed();
    let draft = editor.simai_editor.text.clone();
    let deleted = app.chart.notes[0].id;
    editor.delete_note(&mut app.chart, deleted);
    let ctx = context(&mut service, &mut editor, &app);
    assert!(ctx.unapplied_draft);
    assert!(ctx.draft_conflict);
    assert!(!ctx.simai_matches_chart);
    assert_eq!(editor.simai_editor.text, draft);
    let pending = service.client().call(
        "chart_propose_edit",
        json!({"schema_version":1,"target":ctx.target,"operation_id":"keep-draft","summary":"Blocked by draft",
            "edits":[{"kind":"delete_notes","note_ids":[app.chart.notes[0].id.to_string()]}]}),
        Permissions { read: true, focus: false, propose: true }, Duration::from_secs(30),
    ).unwrap();
    service.tick(&mut editor, &app, true, false);
    assert_eq!(pending.poll().unwrap().unwrap_err().code, ErrorCode::UnappliedDraft);
    assert_eq!(editor.simai_editor.text, draft);
    assert_eq!(app.chart.notes.len(), 2);
}
#[test]
fn queries_use_the_open_document_without_mutating_document_or_transport() {
    let (mut service, mut editor, app) = fixture();
    let chart = app.chart.clone();
    let state = format!("{editor:?}");
    let ctx = context(&mut service, &mut editor, &app);
    assert_eq!(ctx.difficulty_slot, Some(5));
    assert_eq!(ctx.authored_note_count, 3);
    assert_eq!(ctx.selected_note_ids, ["11", "12"]);
    assert!(ctx.simai_matches_chart);
    assert!(!ctx.core_available);
    assert!(!ctx.core_current);
    assert!(ctx.auto_save);
    let page: NotesPage = serde_json::from_value(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_query_notes",
            note_args(&ctx.target, 0, 128),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        page.notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["11", "10", "12"]
    );
    assert_eq!(page.notes[1].touch_zone.as_deref(), Some("C"));
    assert_eq!(page.notes[1].note_type, NoteKind::TouchHold);
    assert_eq!(page.notes[1].ring_lane, None);
    call(
        &mut service,
        &mut editor,
        &app,
        "chart_read_simai",
        json!({"schema_version":1,"target":ctx.target,"range":{"start_char":0,"end_char":4}}),
    )
    .unwrap();
    let report: CheckReport = serde_json::from_value(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_check",
            json!({"schema_version":1,"target":ctx.target,"level":"structure","limit":128}),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(report.diagnostics.is_empty());
    assert_eq!(app.chart, chart);
    assert_eq!(format!("{editor:?}"), state);
    assert_eq!(app.timeline_view_time, 0.75);
    assert_eq!(app.forced_time, Some(0.75));
    assert_eq!(app.mode, types::Mode::Idle);
    assert_eq!(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_check",
            json!({"schema_version":1,"target":ctx.target,"level":"core","limit":10})
        )
        .unwrap_err()
        .code,
        ErrorCode::CoreUnavailable
    );
}
#[test]
fn paging_is_immutable_but_live_focus_rejects_changed_content() {
    let (mut service, mut editor, mut app) = fixture();
    let ctx = context(&mut service, &mut editor, &app);
    app.chart.notes[0].time = 1.0;
    app.chart.notes.reverse();
    let page: NotesPage = serde_json::from_value(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_query_notes",
            note_args(&ctx.target, 1, 1),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(page.notes[0].id, "10");
    assert_eq!(page.total, 3);
    assert_eq!(page.next_offset, Some(2));
    let focus = json!({"schema_version":1,"target":ctx.target,"range":{"start_measure":2.0,"end_measure":3.0}});
    let view = editor.view_start;
    assert_eq!(
        call(&mut service, &mut editor, &app, "editor_focus_range", focus)
            .unwrap_err()
            .code,
        ErrorCode::DocumentChanged
    );
    assert_eq!(editor.view_start, view);
    let current = context(&mut service, &mut editor, &app);
    assert!(!current.simai_matches_chart);
    assert_ne!(ctx.target.snapshot_token, current.target.snapshot_token);
    assert_eq!(ctx.target.document_id, current.target.document_id);
}
#[test]
fn exact_unicode_crlf_draft_is_not_parsed_or_normalized() {
    let (mut service, mut editor, mut app) = fixture();
    let text: String = [
        '注',
        '释',
        char::from(13),
        char::from(10),
        '😀',
        '1',
        ',',
        'E',
    ]
    .into_iter()
    .collect();
    editor.simai_editor.text = text.clone();
    editor.simai_editor.changed();
    app.chart.notes[0].lane = 8;
    let ctx = context(&mut service, &mut editor, &app);
    assert!(ctx.unapplied_draft);
    assert!(ctx.draft_conflict);
    assert!(!ctx.simai_matches_chart);
    let args =
        json!({"schema_version":1,"target":ctx.target,"range":{"start_char":4,"end_char":7}});
    let slice: SimaiSlice = serde_json::from_value(
        call(&mut service, &mut editor, &app, "chart_read_simai", args).unwrap(),
    )
    .unwrap();
    assert_eq!(slice.text, "😀1,");
    assert_eq!(slice.start.line, 2);
    assert_eq!(slice.start.column, 1);
    assert_eq!(slice.end.column, 4);
    assert_eq!(slice.total_chars, 8);
    assert!(slice.unapplied_draft);
    editor.simai_editor.text.push('X');
    editor.simai_editor.changed();
    let old: SimaiSlice = serde_json::from_value(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_read_simai",
            json!({"schema_version":1,"target":ctx.target,"range":{"start_char":0,"end_char":8}}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(old.text, text);
}
#[test]
fn identities_expire_on_reload_source_difficulty_and_isolation_changes() {
    let (mut service, mut editor, mut app) = fixture();
    for change in 0..3 {
        let ctx = context(&mut service, &mut editor, &app);
        match change {
            0 => app.load_chart(app.chart.clone(), None, None, None, None),
            1 => editor.source.as_mut().unwrap().path = "/other/maidata.txt".into(),
            _ => editor.source.as_mut().unwrap().level = Some(4),
        }
        assert_eq!(
            call(
                &mut service,
                &mut editor,
                &app,
                "chart_query_notes",
                note_args(&ctx.target, 0, 10)
            )
            .unwrap_err()
            .code,
            ErrorCode::DocumentChanged
        );
        assert_ne!(
            ctx.target.document_id,
            context(&mut service, &mut editor, &app).target.document_id
        );
    }
    let ctx = context(&mut service, &mut editor, &app);
    editor.template_session = Some(Box::new(crate::templates::Session {
        parent: app.chart.clone(),
        editor: editor.clone(),
        template_id: "isolated".into(),
        offset: 0.0,
        time: 0.0,
        playing: false,
        saved: true,
        backdrop: std::sync::Arc::new(app.chart.clone()),
    }));
    assert_eq!(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_get_context",
            json!({"schema_version":1})
        )
        .unwrap_err()
        .code,
        ErrorCode::UnsupportedOperation
    );
    editor.template_session = None;
    assert_eq!(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_query_notes",
            note_args(&ctx.target, 0, 10)
        )
        .unwrap_err()
        .code,
        ErrorCode::DocumentChanged
    );
}
#[test]
fn focus_scrolls_without_seeking_selecting_or_editing() {
    let (mut service, mut editor, app) = fixture();
    let ctx = context(&mut service, &mut editor, &app);
    let chart = app.chart.clone();
    let selected = editor.selected.clone();
    call(&mut service, &mut editor, &app, "editor_focus_range", json!({"schema_version":1,"target":ctx.target,"range":{"start_measure":2.5,"end_measure":3.0}})).unwrap();
    assert_eq!(editor.view_start, 3.0);
    assert!(!editor.follow_playhead);
    assert_eq!(app.chart, chart);
    assert_eq!(editor.selected, selected);
    assert!(!editor.dirty);
    assert!(editor.undo.is_empty());
    assert!(editor.redo.is_empty());
    assert_eq!(app.forced_time, Some(0.75));
    assert_eq!(app.timeline_view_time, 0.75);
    assert!(editor.pending_seek.is_none());
    let pending = enqueue(&service, "chart_get_context", json!({"schema_version":1}));
    service.tick(&mut editor, &app, false, false);
    assert_eq!(
        pending.poll().unwrap().unwrap_err().code,
        ErrorCode::EditorBusy
    );
}
#[test]
fn diagnostics_are_stable_paginated_and_non_finite_values_never_become_null_notes() {
    let (mut service, mut editor, mut app) = fixture();
    app.chart.notes[0].lane = 0;
    app.chart.notes[0].hold_duration = -1.0;
    app.chart.notes[1].time = f32::NAN;
    let ctx = context(&mut service, &mut editor, &app);
    let args = json!({"schema_version":1,"target":ctx.target,"level":"structure","limit":1});
    let first: CheckReport = serde_json::from_value(
        call(&mut service, &mut editor, &app, "chart_check", args.clone()).unwrap(),
    )
    .unwrap();
    let repeated: CheckReport =
        serde_json::from_value(call(&mut service, &mut editor, &app, "chart_check", args).unwrap())
            .unwrap();
    assert_eq!(
        first.diagnostics[0].rule_id,
        repeated.diagnostics[0].rule_id
    );
    assert!(first.total >= 3);
    assert_eq!(first.next_offset, Some(1));
    let mut args = note_args(&ctx.target, 0, 128);
    args["note_ids"] = json!(["11"]);
    assert_eq!(
        call(&mut service, &mut editor, &app, "chart_query_notes", args)
            .unwrap_err()
            .code,
        ErrorCode::ValidationFailed
    );
}
#[test]
fn snapshot_capacity_and_utf8_size_limits_do_not_silently_truncate() {
    let (mut service, mut editor, app) = fixture();
    let oldest = context(&mut service, &mut editor, &app);
    for _ in 0..8 {
        context(&mut service, &mut editor, &app);
    }
    assert_eq!(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_query_notes",
            note_args(&oldest.target, 0, 10)
        )
        .unwrap_err()
        .code,
        ErrorCode::DocumentChanged
    );
    editor.simai_editor.text = "😀".repeat(5000);
    editor.simai_editor.changed();
    let ctx = context(&mut service, &mut editor, &app);
    assert_eq!(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_read_simai",
            json!({"schema_version":1,"target":ctx.target,"range":{"start_char":0,"end_char":5000}})
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
}
#[test]
fn large_chart_capture_and_queries_report_main_thread_cost() {
    let (mut service, mut editor, mut app) = fixture();
    app.chart.notes = (0..20_000)
        .rev()
        .map(|i| ChartNote {
            id: i + 1,
            time: 1.0 + i as f32 / 16.0,
            lane: (i % 8 + 1) as u8,
            ..Default::default()
        })
        .collect();
    editor.simai_editor.load(&app.chart, "E".into());
    let start = Instant::now();
    let ctx = context(&mut service, &mut editor, &app);
    let capture = start.elapsed();
    let start = Instant::now();
    let page: NotesPage = serde_json::from_value(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_query_notes",
            note_args(&ctx.target, 10_000, 128),
        )
        .unwrap(),
    )
    .unwrap();
    let query = start.elapsed();
    assert_eq!(page.total, 20_000);
    assert_eq!(page.notes[0].id, "10001");
    let start = Instant::now();
    call(
        &mut service,
        &mut editor,
        &app,
        "chart_check",
        json!({"schema_version":1,"target":ctx.target,"level":"structure","limit":128}),
    )
    .unwrap();
    let check = start.elapsed();
    let start = Instant::now();
    for _ in 0..1000 {
        service.tick(&mut editor, &app, true, false);
    }
    let idle = start.elapsed();
    eprintln!(
        "automation 20000 notes: capture={capture:?}, query={query:?}, check={check:?}, idle_1000_frames={idle:?}"
    );
}

#[test]
fn filters_exclude_expansion_and_preserve_slide_storage_units() {
    let (mut service, mut editor, mut app) = fixture();
    app.chart.notes.push(ChartNote {
        id: 20,
        time: 2.5,
        lane: 1,
        note_type: NoteType::Slide,
        slide: vec![types::Slide {
            slide_duration: 1.0,
            slide_start_delay: 0.25,
            slide_is_break: true,
            runtime_parts: 1,
            segments: vec![types::SlideSegment {
                shape: types::SlideShape::Line,
                points: vec![types::SlidePoint {
                    zone: types::zone::id_to_zone(5).unwrap(),
                    beat_offset: 0.5,
                }],
            }],
        }],
        ..Default::default()
    });
    app.chart.notes.push(ChartNote {
        id: 21,
        time: 2.5,
        lane: 4,
        template_source: Some(types::NoteTemplateSource::default()),
        ..Default::default()
    });
    app.chart.templates.push(types::TemplateDef {
        id: "template".into(),
        name: "Example".into(),
        version: 1,
        notes: vec![ChartNote {
            id: 99,
            time: 1.0,
            lane: 8,
            ..Default::default()
        }],
        duration: 1.0,
    });
    editor.selected.extend([20, 21, 99]);
    let ctx = context(&mut service, &mut editor, &app);
    assert_eq!(ctx.authored_note_count, 4);
    assert_eq!(ctx.template_count, 1);
    assert_eq!(ctx.selected_note_ids, ["11", "12", "20"]);
    let mut args = note_args(&ctx.target, 0, 128);
    args["range"] = json!({"start_measure":2.5,"end_measure":3.0});
    args["note_types"] = json!(["slide"]);
    args["note_ids"] = json!(["20", "21", "99"]);
    let page: NotesPage = serde_json::from_value(
        call(&mut service, &mut editor, &app, "chart_query_notes", args).unwrap(),
    )
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.notes[0].id, "20");
    let slide = &page.notes[0].slides[0];
    assert_eq!(slide.delay_measures, 0.25);
    assert_eq!(slide.motion_duration_measures, 0.75);
    assert_eq!(slide.segments[0].points[0].stored_beat_offset, 0.5);
    let mut args = note_args(&ctx.target, 0, 128);
    args["range"] = json!({"start_measure":1.0,"end_measure":2.0});
    let page: NotesPage = serde_json::from_value(
        call(&mut service, &mut editor, &app, "chart_query_notes", args).unwrap(),
    )
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.notes[0].id, "11");
}

#[test]
fn missing_simai_is_not_synthesized_and_invalid_tempo_is_inspectable() {
    let (mut service, mut editor, mut app) = fixture();
    editor.simai_editor = Default::default();
    app.chart.bpm = 0.0;
    app.chart.notes[2].hold_duration = 0.0;
    let ctx = context(&mut service, &mut editor, &app);
    assert!(!ctx.has_simai_source);
    assert!(!ctx.simai_matches_chart);
    let error = call(
        &mut service,
        &mut editor,
        &app,
        "chart_read_simai",
        json!({"schema_version":1,"target":ctx.target,"range":{"start_char":0,"end_char":0}}),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::UnsupportedOperation);
    let report: CheckReport = serde_json::from_value(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_check",
            json!({"schema_version":1,"target":ctx.target,"level":"structure","limit":128}),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "chart.tempo")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "hold.duration")
    );
    assert_eq!(call(&mut service, &mut editor, &app, "editor_focus_range", json!({"schema_version":1,"target":ctx.target,"range":{"start_measure":1.0,"end_measure":2.0}})).unwrap_err().code, ErrorCode::ValidationFailed);
    app.chart.bpm = f32::NAN;
    assert_eq!(
        call(
            &mut service,
            &mut editor,
            &app,
            "chart_get_context",
            json!({"schema_version":1})
        )
        .unwrap_err()
        .code,
        ErrorCode::ValidationFailed
    );
}
