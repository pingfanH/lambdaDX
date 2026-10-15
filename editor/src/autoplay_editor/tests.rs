use super::*;
use std::collections::HashSet;

#[test]
fn autoplay_zoom_stays_centered_during_playback_and_after_wheel_release() {
    for mode in [Mode::Idle, Mode::Playing, Mode::Recording] {
        let ctx = egui::Context::default();
        let mut editor = EditorState::default();
        editor.workspace = crate::state::Workspace::Autoplay;
        let mut app = PadPreviewState::new_at(chart("1"), None, None, 0.0, None);
        app.mode = mode;
        let mut layout = Layout::default();
        let command = egui::Modifiers {
            command: true,
            mac_cmd: true,
            ..Default::default()
        };
        let pointer = Pos2::new(640.0, 680.0);
        for frame in 0..18 {
            let playing = matches!(mode, Mode::Playing | Mode::Recording);
            let now = 30.0 + if playing { frame as f32 / 60.0 } else { 0.0 };
            app.forced_time = Some(now);
            app.timeline_view_time = now;
            if playing {
                crate::runtime::follow_playhead(&mut editor, now);
            }
            let modifiers = if frame == 4 { command } else { egui::Modifiers::NONE };
            let mut events = vec![egui::Event::PointerMoved(pointer)];
            if matches!(frame, 3 | 4 | 8 | 16) {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: match frame {
                        4 => Vec2::new(0.0, 30.0),
                        8 => Vec2::new(0.0, -30.0),
                        _ => Vec2::new(60.0, 0.0),
                    },
                    modifiers: if frame == 4 || frame == 8 {
                        command
                    } else {
                        egui::Modifiers::NONE
                    },
                });
            }
            let old_pps = editor.pixels_per_second;
            let wheel = match frame { 4 => 1.0, 8 => -1.0, _ => 0.0 };
            let output = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                time: Some(frame as f64 / 60.0),
                events,
                modifiers,
                ..Default::default()
            }, |ctx| draw(ctx, &mut editor, &mut app, &mut layout, true, wheel));
            if wheel != 0.0 {
                assert_ne!(editor.pixels_per_second, old_pps, "zoom ignored on frame {frame}");
            }
            if frame >= 4 {
                let lines: Vec<_> = output.shapes.iter().filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == Color32::from_rgb(255, 73, 92) =>
                            Some((points[0].x, shape.clip_rect)),
                    _ => None,
                }).collect();
                assert!(!lines.is_empty(), "missing playhead on frame {frame}");
                let pan = if !playing && frame >= 16 { 60.0 } else { 0.0 };
                for (x, plot) in lines {
                    assert!((x - plot.center().x - pan).abs() <= 1.0,
                        "{mode:?}, frame {frame}: playhead x={x}, plot={plot:?}, view={}, pps={}",
                        editor.view_start, editor.pixels_per_second);
                }
            }
        }
    }
}

#[test]
fn autoplay_zoom_centers_playhead() {
    let mut editor = EditorState::default();
    for width in [180.0, 640.0, 1600.0] {
        for playhead in [0.0, 0.5, 30.0] {
            for pps in [10.0, 120.0, 360.0, 4800.0] {
                editor.view_start = playhead + 100.0;
                set_centered_zoom(&mut editor, pps, playhead, width);
                let x = (playhead - editor.view_start) * editor.pixels_per_second;
                assert!((x - width * 0.5).abs() < 0.01,
                    "playhead at {x} instead of center for {width} px, {pps} px/s, {playhead} s");
            }
        }
    }
}

#[test]
fn requested_timeline_slide_endpoints_have_separate_hit_areas() {
    let events = vec![
        make_event("A1", 1, 0),
        make_event("A1", 2, 100_000),
        make_event("B1", 1, 100_000),
        make_event("B1", 2, 200_000),
        make_event("A2", 1, 100_000),
        make_event("A2", 2, 100_000),
    ];
    for pps in [10.0, 360.0, 4800.0] {
        let layout = event_layout(&events, pps);
        let hits: Vec<_> = events.iter().enumerate().map(|(index, event)| {
            Rect::from_center_size(
                event_center(event, index, &layout, 0.0, timed_input_tp(event) as f32 / 1e6 * pps),
                Vec2::splat(14.0),
            )
        }).collect();
        for (index, hit) in hits.iter().enumerate() {
            for other in &hits[index + 1..] {
                assert!(!hit.intersects(*other), "overlapping endpoints at {pps} px/s");
            }
        }
    }
    let mut state = AutoplayEditor::default();
    state.selected = Some((0, 1));
    state.selected_row = Some(0);
    state.clear_selection();
    assert!(state.selected.is_none() && state.selected_row.is_none());
    assert_eq!(state.new_zone, "A1");
    assert_eq!(state.new_kind, 0);
}

fn chart(source: &str) -> ChartDoc {
    let mut chart =
        crate::app::maidata::from_maidata_level(&format!("&inote_6=(120){{4}}{source},E"), 6)
            .unwrap();
    for (index, note) in chart.notes.iter_mut().enumerate() {
        note.id = 100 + index as u64;
    }
    chart
}

fn player(chart: ChartDoc) -> PadPreviewState {
    let mut app = crate::runtime::initialize_player(
        PadPreviewState::new_at(chart, None, None, 0.0, None),
        None,
        None,
    );
    app.forced_time = Some(0.0);
    assert!(app.has_engine(), "{}", app.status);
    app
}

#[test]
fn imported_autoplay_keeps_touch_and_slide_event_groups() {
    for (source, body_count) in [
        ("&inote_6=(120){4}B2,1-5[4:1],E", 1),
        (
            "&inote_6=(120){4}1,2h[4:1],B3/B4,4-6-1[0.5##2],Ch[4:1],E",
            2,
        ),
    ] {
        for (native, inner_screen) in [(false, false), (false, true), (true, false), (true, true)] {
            let chart = crate::app::maidata::from_maidata_level(source, 6).unwrap();
            let mut app = crate::runtime::initialize_player(
                PadPreviewState::new_at(chart, None, None, 0.0, None),
                (!native).then(|| source.to_owned()),
                Some(6),
            );
            app.params.autoplay_inner_screen = inner_screen;
            assert!(app.has_engine(), "{}", app.status);
            let mut editor = AutoplayEditor::default();
            editor.sync(&app);
            for note in &app.playback_chart().notes {
                assert!(
                    editor.tracks.iter().any(|track| {
                        track.key.note_id == note.id && !track.events.is_empty()
                    }),
                    "native={native}, inner_screen={inner_screen}: missing {:?} #{}",
                    note.note_type,
                    note.id
                );
            }
            let bodies: Vec<_> = editor
                .tracks
                .iter()
                .filter(|track| track.key.role == 2)
                .collect();
            assert_eq!(
                bodies.len(),
                body_count,
                "every slide segment needs an event group"
            );
            assert!(bodies.iter().all(|track| !track.events.is_empty()));
            let part_count = app
                .default_autoplay_plan
                .as_ref()
                .unwrap()
                .groups
                .iter()
                .map(|group| group.parts.len())
                .sum::<usize>();
            assert_eq!(
                editor.tracks.len(),
                part_count,
                "no input part should be dropped from the editor"
            );
            for note in app.playback_chart().notes.iter().filter(|note| {
                matches!(
                    note.note_type,
                    crate::app::types::NoteType::Touch | crate::app::types::NoteType::TouchHold
                )
            }) {
                let track = editor
                    .tracks
                    .iter()
                    .find(|track| track.key.note_id == note.id && track.key.role == 0)
                    .unwrap();
                assert!(
                    track.events.iter().all(|event| match event {
                        TimedInputEvent::SensorClick { area, .. }
                        | TimedInputEvent::SensorHold { area, .. } => {
                            crate::player::engine::zone_for_sensor(*area).to_id() == note.lane
                        }
                        _ => false,
                    }),
                    "touch inputs must remain on their owning sensor"
                );
            }
            let packed: HashSet<_> = editor
                .lanes
                .iter()
                .flatten()
                .map(|group| group.row)
                .collect();
            assert_eq!(packed.len(), editor.tracks.len());
        }
    }
}

#[test]
fn ownership_distinguishes_same_head_slides_conslide_segments_and_wifi_body() {
    let mut same_head = chart("1-3[4:1]*-5[4:1]");
    let second = same_head.notes.remove(1);
    same_head.notes[0].slide.extend(second.slide);
    let mut editor = AutoplayEditor::default();
    editor.sync(&player(same_head));
    let slides: HashSet<_> = editor
        .tracks
        .iter()
        .filter(|t| t.key.role == 2)
        .map(|t| t.key.slide)
        .collect();
    assert_eq!(slides, HashSet::from([0, 1]));
    assert!(editor.tracks.iter().all(|t| t.key.note_id == 100));
    assert_eq!(editor.tracks.iter().filter(|t| t.key.role == 1).count(), 1);

    editor = AutoplayEditor::default();
    editor.sync(&player(chart("3-5-8[0.5##2]")));
    let segments: HashSet<_> = editor
        .tracks
        .iter()
        .filter(|t| t.key.role == 2)
        .map(|t| t.key.segment)
        .collect();
    assert_eq!(segments, HashSet::from([0, 1]));

    editor = AutoplayEditor::default();
    editor.sync(&player(chart("1w5[0.25##1]")));
    let bodies: Vec<_> = editor.tracks.iter().filter(|t| t.key.role == 2).collect();
    assert_eq!(
        bodies.len(),
        1,
        "the core owns all Wi-Fi branches in one body part"
    );
    let zones: HashSet<_> = bodies[0].events.iter().map(event_zone).collect();
    assert!(
        zones.len() >= 3,
        "the body track must retain all Wi-Fi sensor events"
    );
    assert_eq!(editor.tracks.iter().filter(|t| t.key.role == 1).count(), 1);
    let unique: HashSet<_> = editor.tracks.iter().map(|t| &t.key).collect();
    assert_eq!(unique.len(), editor.tracks.len());
}

#[test]
fn event_add_delete_reset_and_undo_preserve_default_plan() {
    let app = player(chart("1"));
    let mut chart = app.chart.clone();
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let defaults = state.tracks[0].defaults.clone();
    let mut events = defaults.clone();
    events.push(make_event("B3", 1, 250_000));
    let mut editor = EditorState::default();
    state.commit(&mut editor, &mut chart, 0, events.clone());
    assert_eq!(editor.undo.len(), 1);
    assert_eq!(chart.autoplay_overrides[0].events, events);
    assert_eq!(state.tracks[0].defaults, defaults);
    state.commit(&mut editor, &mut chart, 0, events);
    assert_eq!(
        editor.undo.len(),
        1,
        "unchanged events must not add history"
    );
    state.selected = Some((0, defaults.len()));
    state.delete(&mut editor, &mut chart);
    assert!(
        chart.autoplay_overrides.is_empty(),
        "returning to defaults removes the override"
    );
    editor.undo(&mut chart);
    assert_eq!(chart.autoplay_overrides.len(), 1);
    state.reset(&mut editor, &mut chart, 0);
    assert!(chart.autoplay_overrides.is_empty());
    assert_eq!(state.tracks[0].events, defaults);
    state.selected = Some((0, 0));
    state.delete(&mut editor, &mut chart);
    assert!(
        chart.autoplay_overrides[0].events.is_empty(),
        "an empty override disables this track"
    );
    editor.undo(&mut chart);
    assert!(chart.autoplay_overrides.is_empty());
    editor.redo(&mut chart);
    assert!(chart.autoplay_overrides[0].events.is_empty());
}

#[test]
fn inspector_transactions_cancel_or_commit_as_one_step() {
    let app = player(chart("1"));
    let mut chart = app.chart.clone();
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let original = state.tracks[0].events.clone();
    let mut editor = EditorState::default();
    state.inspector_edit = Some(InspectorEdit {
        row: 0,
        original: original.clone(),
    });
    state.tracks[0].events[0] = make_event("B2", 0, 150_000);
    state.cancel();
    assert_eq!(state.tracks[0].events, original);
    assert!(chart.autoplay_overrides.is_empty());
    assert!(editor.undo.is_empty());
    state.inspector_edit = Some(InspectorEdit {
        row: 0,
        original: original.clone(),
    });
    for time in [100_000, 200_000, 300_000] {
        state.tracks[0].events[0] = make_event("B2", 0, time);
    }
    state.finish_inspector(&mut editor, &mut chart);
    state.finish_inspector(&mut editor, &mut chart);
    assert_eq!(editor.undo.len(), 1);
    assert_eq!(
        chart.autoplay_overrides[0].events,
        [make_event("B2", 0, 300_000)]
    );
    editor.undo(&mut chart);
    assert!(chart.autoplay_overrides.is_empty());
    state.drag = Some(Drag {
        row: 0,
        index: 0,
        original: original[0].clone(),
        origin_x: 0.0,
    });
    state.tracks[0].events[0] = make_event("B3", 2, 400_000);
    state.cancel();
    assert_eq!(state.tracks[0].events, original);
}

#[test]
fn relative_edits_and_selected_event_follow_note_moves_and_core_reordering() {
    let app = player(chart("1,2"));
    let mut edited = app.chart.clone();
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let row = state
        .tracks
        .iter()
        .position(|t| t.key.note_id == 100)
        .unwrap();
    let key = state.tracks[row].key.clone();
    state.selected_row = Some(row);
    state.selected = Some((row, 0));
    state.commit(
        &mut EditorState::default(),
        &mut edited,
        row,
        vec![make_event("K1", 0, 125_000)],
    );
    edited.notes[0].time = 3.0;
    edited.notes.reverse();
    let next = player(edited);
    state.sync(&next);
    let row = state.tracks.iter().position(|t| t.key == key).unwrap();
    assert_eq!(state.selected_row, Some(row));
    assert_eq!(state.selected, Some((row, 0)));
    assert_eq!(state.tracks[row].events, [make_event("K1", 0, 4_125_000)]);
    let part = next
        .autoplay_plan
        .as_ref()
        .unwrap()
        .groups
        .iter()
        .flat_map(|g| &g.parts)
        .find(|part| {
            autoplay_edit::key(
                next.playback_chart(),
                next.judge_engine.as_ref().unwrap(),
                part,
            ) == Some(key.clone())
        })
        .unwrap();
    assert_eq!(part.events, state.tracks[row].events);
    assert_eq!(
        next.chart.autoplay_overrides[0].events,
        [make_event("K1", 0, 125_000)]
    );
}

#[test]
fn event_zone_and_action_roundtrip_all_input_channels() {
    for zone in zone_names() {
        for kind in 0..=2 {
            let event = make_event(&zone, kind, -123_456);
            assert_eq!(event_zone(&event), zone);
            assert_eq!(event_kind(&event), kind);
            assert_eq!(timed_input_tp(&event), -123_456);
        }
    }
}

#[test]
fn event_snap_uses_chart_measures_and_can_be_disabled() {
    let chart = chart("1");
    let mut editor = EditorState::default();
    editor.snap_division = 16;
    assert_eq!(snap_time(0.19, &editor, &chart), 0.25);
    editor.snap_enabled = false;
    assert!((snap_time(0.19, &editor, &chart) - 0.19).abs() < 1e-6);
}
