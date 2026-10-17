use super::*;
use crate::app::autoplay_edit::{AutoplayKey, AutoplayOverride};
use crate::core::types::{SensorArea, TimedInputEvent};

fn fixture() -> (EditorState, PadPreviewState) {
    let mut chart = crate::app::chart::fallback_chart();
    chart.notes = vec![
        Note {
            id: 11,
            time: 3.0,
            lane: 1,
            ..Default::default()
        },
        Note {
            id: 12,
            time: 4.0,
            lane: 3,
            hold_duration: 0.5,
            ..Default::default()
        },
        Note {
            id: 13,
            time: 3.0,
            lane: 1,
            ..Default::default()
        },
    ];
    chart.templates.clear();
    chart.template_instances.clear();
    chart.autoplay_overrides.clear();
    let mut editor = EditorState::default();
    editor.selected.extend([11, 12]);
    let mut app = PadPreviewState::new_at(chart, None, None, 0.0, None);
    app.forced_time = Some(0.75);
    app.timeline_view_time = 0.75;
    (editor, app)
}

fn authored(note_id: u64) -> AutoplayOverride {
    AutoplayOverride {
        curve: None,
        key: AutoplayKey {
            note_id,
            role: 0,
            slide: 0,
            segment: 0,
            track: None,
        },
        events: vec![TimedInputEvent::SensorClick {
            tp: 100,
            area: SensorArea::B1,
        }],
    }
}

#[test]
fn selection_preserves_coincident_unselected_notes_and_remaps_authored_events() {
    let (mut editor, mut app) = fixture();
    app.chart.autoplay_overrides.push(authored(11));
    apply(&mut editor, &mut app, Action::CreateSelection);
    assert_eq!(
        app.chart.notes.iter().map(|n| n.id).collect::<Vec<_>>(),
        [13]
    );
    let template = &app.chart.templates[0];
    assert_eq!(
        template.notes.iter().map(|n| n.time).collect::<Vec<_>>(),
        [1.0, 2.0]
    );
    assert_eq!(template.duration, 1.5);
    assert_eq!(app.chart.template_instances[0].anchor_time, 3.0);
    assert_eq!(app.timeline_view_time, 0.75);
    assert_eq!(app.playback_chart().notes.len(), 3);
    let virtual_id = app
        .playback_chart()
        .notes
        .iter()
        .find(|n| {
            n.template_source
                .as_ref()
                .is_some_and(|s| s.source_note_id == 1)
        })
        .unwrap()
        .id;
    assert_ne!(virtual_id, 11);
    assert_eq!(app.chart.autoplay_overrides[0].key.note_id, virtual_id);
    assert_eq!(app.chart.autoplay_overrides[0].events, authored(11).events);
    editor.undo(&mut app.chart);
    assert!(app.chart.templates.is_empty());
    assert_eq!(app.chart.notes.len(), 3);
    assert_eq!(app.chart.autoplay_overrides[0].key.note_id, 11);
    editor.redo(&mut app.chart);
    assert_eq!(app.chart.autoplay_overrides[0].key.note_id, virtual_id);
}

#[test]
fn isolation_uses_definition_or_instance_offset_and_propagates_one_undo_step() {
    for instance in [false, true] {
        let (mut editor, mut app) = fixture();
        apply(&mut editor, &mut app, Action::CreateSelection);
        app.chart.template_instances.push(TemplateInstance {
            instance_id: "second".into(),
            template_id: "tpl_1".into(),
            template_version: 1,
            anchor_time: 6.0,
            hidden: false,
        });
        app.refresh_playback_chart();
        let parent = app.chart.clone();
        let history = editor.undo.len();
        let action = if instance {
            Action::EnterInstance("second".into())
        } else {
            Action::EnterDefinition("tpl_1".into())
        };
        apply(&mut editor, &mut app, action);
        let offset = if instance { 5.0 } else { 0.0 };
        assert_eq!(editor.template_session.as_ref().unwrap().offset, offset);
        assert_eq!(app.chart.notes[0].time, 1.0 + offset);
        assert!(app.chart.template_instances.is_empty());
        assert!(editor.undo.is_empty());
        editor.push_history(&app.chart);
        app.chart.notes[0].lane = 7;
        apply(&mut editor, &mut app, Action::Exit);
        assert_eq!(editor.undo.len(), history + 1);
        assert_eq!(app.chart.templates[0].version, 2);
        assert_eq!(app.chart.templates[0].notes[0].time, 1.0);
        assert_eq!(app.chart.templates[0].notes[0].lane, 7);
        assert!(
            app.chart
                .template_instances
                .iter()
                .all(|i| i.template_version == 2)
        );
        assert_eq!(app.chart.notes, parent.notes);
        assert_eq!(
            app.playback_chart()
                .notes
                .iter()
                .filter(|n| n.lane == 7)
                .count(),
            2
        );
        editor.undo(&mut app.chart);
        assert_eq!(app.chart.templates, parent.templates);
        editor.redo(&mut app.chart);
        assert_eq!(app.chart.templates[0].notes[0].lane, 7);
    }
}

#[test]
fn exit_restores_parent_transport_and_queues_seek_after_scene_replacement() {
    for playing in [false, true] {
        let (mut editor, mut app) = fixture();
        apply(&mut editor, &mut app, Action::CreateSelection);
        if playing {
            app.toggle_play();
        }
        apply(
            &mut editor,
            &mut app,
            Action::EnterInstance("inst_1".into()),
        );
        assert_eq!(app.mode, Mode::Idle);
        app.timeline_view_time = 12.0;
        app.toggle_play();
        apply(&mut editor, &mut app, Action::Exit);
        assert_eq!(editor.pending_seek, Some(0.75));
        assert_eq!(app.mode == Mode::Playing, playing);
        assert!(editor.template_session.is_none());
    }
}

#[test]
fn saved_isolation_then_reverting_to_original_still_needs_saving() {
    let (mut editor, mut app) = fixture();
    apply(&mut editor, &mut app, Action::CreateSelection);
    editor.dirty = false;
    apply(
        &mut editor,
        &mut app,
        Action::EnterDefinition("tpl_1".into()),
    );
    editor.push_history(&app.chart);
    app.chart.notes[0].lane = 7;
    mark_saved(&mut editor);
    editor.undo(&mut app.chart);
    apply(&mut editor, &mut app, Action::Exit);
    assert_eq!(app.chart.templates[0].notes[0].lane, 1);
    assert!(
        editor.dirty,
        "disk contains lane 7 although the parent originally had lane 1"
    );
}

#[test]
fn new_empty_template_enters_isolation_with_original_duration() {
    let (mut editor, mut app) = fixture();
    apply(&mut editor, &mut app, Action::New);
    assert!(app.chart.notes.is_empty());
    let session = editor.template_session.as_ref().unwrap();
    assert_eq!(session.parent.templates[0].duration, 1.0);
    assert!(session.parent.template_instances.is_empty());
    apply(&mut editor, &mut app, Action::Exit);
    assert_eq!(app.chart.notes.len(), 3);
    assert_eq!(app.chart.templates[0].duration, 1.0);
    editor.undo(&mut app.chart);
    assert!(app.chart.templates.is_empty());
}

#[test]
fn deleting_instance_prunes_only_its_authored_events_and_is_undoable() {
    let (mut editor, mut app) = fixture();
    apply(&mut editor, &mut app, Action::CreateSelection);
    app.chart.template_instances.push(TemplateInstance {
        instance_id: "second".into(),
        template_id: "tpl_1".into(),
        template_version: 1,
        anchor_time: 6.0,
        hidden: false,
    });
    app.refresh_playback_chart();
    let ids: Vec<_> = app
        .playback_chart()
        .notes
        .iter()
        .filter(|n| n.template_source.is_some())
        .map(|n| n.id)
        .collect();
    app.chart.autoplay_overrides = ids.iter().copied().map(authored).collect();
    app.chart.autoplay_overrides.push(authored(13));
    apply(
        &mut editor,
        &mut app,
        Action::DeleteInstance("inst_1".into()),
    );
    assert_eq!(app.chart.template_instances.len(), 1);
    assert_eq!(app.chart.autoplay_overrides.len(), 3);
    assert!(
        app.chart
            .autoplay_overrides
            .iter()
            .any(|item| item.key.note_id == 13)
    );
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.template_instances.len(), 2);
    assert_eq!(app.chart.autoplay_overrides.len(), 5);
    app.refresh_playback_chart();
    apply(
        &mut editor,
        &mut app,
        Action::DeleteDefinition("tpl_1".into()),
    );
    assert!(app.chart.templates.is_empty());
    assert!(app.chart.template_instances.is_empty());
    assert_eq!(app.chart.autoplay_overrides, [authored(13)]);
}
