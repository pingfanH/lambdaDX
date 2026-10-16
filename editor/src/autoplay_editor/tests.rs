use super::*;
use std::collections::HashSet;

struct NavigationFixture {
    ctx: egui::Context,
    state: AutoplayEditor,
    editor: EditorState,
    app: PadPreviewState,
    time: f64,
}

impl NavigationFixture {
    fn new(mode: Mode) -> Self {
        let mut app = PadPreviewState::new_at(chart("1"), None, None, 0.0, None);
        app.mode = mode;
        let mut f = Self { ctx: egui::Context::default(), state: AutoplayEditor::default(),
            editor: EditorState::default(), app, time: 0.0 };
        f.editor.snap_enabled = false;
        f.frame(Vec::new(), 0.0);
        f.frame(Vec::new(), 0.0);
        f
    }

    fn frame(&mut self, events: Vec<egui::Event>, wheel: f32) -> Viewport {
        self.time += 1.0 / 60.0;
        let now = 30.0 + if matches!(self.app.mode, Mode::Playing | Mode::Recording) { self.time as f32 } else { 0.0 };
        self.app.forced_time = Some(now);
        self.app.timeline_view_time = now;
        let mut result = None;
        let _ = self.ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(self.time), events, ..Default::default()
        }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                result = Some(viewport(ui, &mut self.state, &mut self.editor, &mut self.app, wheel));
            });
        });
        result.unwrap()
    }

    fn button(&mut self, pos: Pos2, button: egui::PointerButton, pressed: bool) {
        self.frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
            pos, button, pressed, modifiers: egui::Modifiers::NONE,
        }], 0.0);
    }
}

#[test]
fn timeline_grid_subdivisions_contrast_with_track_backgrounds() {
    for raw in [false, true] {
        let mut f = NavigationFixture::new(Mode::Idle);
        let plot = f.frame(Vec::new(), 0.0);
        let x = plot.grid.iter().find(|(_, major)| !major).unwrap().0;
        let output = f.ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if raw { tracks_ui(ui, &mut f.state, &mut f.editor, &mut f.app, false, 0.0); }
                else { clips::draw(ui, &mut f.state, &mut f.editor, &mut f.app, false, 0.0); }
            });
        });
        let strokes: Vec<_> = output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, stroke }
                if (points[0].x - x).abs() < 0.01 && points[0].x == points[1].x
                    && points[0].y >= plot.ruler.bottom() => Some(stroke),
            _ => None,
        }).collect();
        assert!(!strokes.is_empty(), "missing subdivisions: raw={raw}");
        for stroke in strokes {
            assert!(stroke.color.r() >= 53, "subdivision blends into the 39/41 background: raw={raw}, {stroke:?}");
        }
    }
}

#[test]
fn timeline_grid_zoom_out_keeps_coarser_subdivisions() {
    let mut f = NavigationFixture::new(Mode::Idle);
    f.state.pixels_per_second = 30.0;
    for division in [16, 32, 48, 192] {
        f.editor.snap_division = division;
        let plot = f.frame(Vec::new(), 0.0);
        assert!(plot.grid.iter().any(|(_, major)| !major), "1/{division} dropped every subdivision");
    }
}

#[test]
fn timeline_navigation_browsing_retains_follow_until_playback_resumes() {
    let mut f = NavigationFixture::new(Mode::Idle);
    let pos = Pos2::new(500.0, 250.0);
    f.frame(vec![egui::Event::PointerMoved(pos), egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -60.0),
        modifiers: egui::Modifiers::SHIFT,
    }], -1.0);
    assert!(f.state.follow, "browsing must not switch follow off");
    let view = f.state.view_start;
    f.frame(Vec::new(), 0.0);
    assert_eq!(f.state.view_start, view, "paused browsing stays in place");
    f.app.mode = Mode::Playing;
    let plot = f.frame(Vec::new(), 0.0);
    let play_x = (f.app.timeline_time() - f.state.view_start) * f.state.pixels_per_second;
    assert!((play_x - (plot.ruler.right() - plot.left) * 0.5).abs() < 0.01);
}

#[test]
fn timeline_navigation_zoom_keeps_pointer_time_during_playback() {
    for mode in [Mode::Idle, Mode::Playing, Mode::Recording] {
        let mut f = NavigationFixture::new(mode);
        let pointer = Pos2::new(340.0, 680.0);
        for frame in 0..18 {
            let old_view = f.state.view_start;
            let old_pps = f.state.pixels_per_second;
            let wheel = match frame { 0 => 1.0, 4 => -1.0, _ => 0.0 };
            let mut events = vec![egui::Event::PointerMoved(pointer)];
            if wheel != 0.0 {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, wheel * 30.0),
                    modifiers: egui::Modifiers { command: true, mac_cmd: true, ..Default::default() },
                });
            }
            let plot = f.frame(events, wheel);
            if wheel != 0.0 {
                assert_ne!(f.state.pixels_per_second, old_pps);
                let before = old_view + (pointer.x - plot.left) / old_pps;
                let after = f.state.view_start + (pointer.x - plot.left) / f.state.pixels_per_second;
                assert!((before - after).abs() < 0.0001, "pointer time shifted: {mode:?}");
            } else if mode == Mode::Idle {
                assert_eq!(f.state.view_start, old_view, "follow overwrote manual zoom");
            } else {
                let x = (f.app.timeline_time() - f.state.view_start) * f.state.pixels_per_second;
                assert!((x - (plot.ruler.right() - plot.left) * 0.5).abs() < 0.01);
            }
            assert!(f.state.follow);
            assert!(f.editor.pending_seek.is_none());
        }
        f.state.return_current();
        let plot = f.frame(Vec::new(), 0.0);
        assert!(f.state.follow);
        let x = (f.app.timeline_time() - f.state.view_start) * f.state.pixels_per_second;
        assert!((x - (plot.ruler.right() - plot.left) * 0.5).abs() < 0.01);
    }
}

#[test]
fn timeline_navigation_zoom_preserves_anchor_at_scale_limits() {
    for offset in [0.0, 32.0, 640.0, 1600.0] {
        for pps in [1.0, 10.0, 120.0, 360.0, 4800.0, 9600.0] {
            let mut state = AutoplayEditor { view_start: -2.5, ..Default::default() };
            let anchor = state.view_start + offset / state.pixels_per_second;
            set_anchored_zoom(&mut state, pps, offset);
            let after = state.view_start + offset / state.pixels_per_second;
            assert!((anchor - after).abs() < 0.0001);
            assert!(state.follow && !state.center_requested);
        }
    }
}

#[test]
fn timeline_navigation_middle_rate_scroll_and_shift_wheel_do_not_seek() {
    for y in [21.0, 250.0] {
        let mut f = NavigationFixture::new(Mode::Playing);
        let pos = Pos2::new(500.0, y);
        f.frame(vec![egui::Event::PointerMoved(pos)], 0.0);
        let before = f.state.view_start;
        let pps = f.state.pixels_per_second;
        f.button(pos, egui::PointerButton::Middle, true);
        let moved = pos + Vec2::new(75.0, 5.0);
        f.frame(vec![egui::Event::PointerMoved(moved)], 0.0);
        assert!(f.state.view_start > before);
        let before = f.state.view_start;
        f.frame(Vec::new(), 0.0);
        let slow_step = f.state.view_start - before;
        assert!(slow_step > 0.0, "stationary pointer away from origin keeps scrolling");
        let outside = Pos2::new(1400.0, y);
        let before = f.state.view_start;
        f.frame(vec![egui::Event::PointerMoved(outside)], 0.0);
        assert!(f.state.view_start - before > slow_step, "distance controls speed");
        let before = f.state.view_start;
        f.frame(vec![egui::Event::PointerMoved(pos - Vec2::new(75.0, 0.0))], 0.0);
        assert!(f.state.view_start < before, "crossing the origin reverses scroll");
        let before = f.state.view_start;
        f.frame(vec![egui::Event::PointerMoved(pos)], 0.0);
        f.frame(Vec::new(), 0.0);
        assert_eq!(f.state.view_start, before, "returning to origin stops scroll");
        f.button(outside, egui::PointerButton::Middle, false);
        assert!(!f.state.pointer_dragging() && f.state.follow);
        assert!(!f.editor.scrubbing && f.editor.pending_seek.is_none());
        let plot = f.frame(Vec::new(), 0.0);
        let x = (f.app.timeline_time() - f.state.view_start) * pps;
        assert!((x - (plot.ruler.right() - plot.left) * 0.5).abs() < 0.01, "playback resumes following");
        f.app.mode = Mode::Idle;
        let view = f.state.view_start;
        f.frame(vec![egui::Event::PointerMoved(pos), egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -60.0),
            modifiers: egui::Modifiers { shift: true, ..Default::default() },
        }], -1.0);
        assert!((f.state.view_start - (view + 60.0 / pps)).abs() < 0.0001);
        let view = f.state.view_start;
        f.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -60.0),
            modifiers: egui::Modifiers::NONE,
        }], -1.0);
        assert_eq!(f.state.view_start, view, "vertical wheel is reserved for lanes");
        assert!(f.editor.pending_seek.is_none());
    }
}

#[test]
fn timeline_navigation_locator_keeps_panning_outside_until_release() {
    for direction in [-1.0, 1.0] {
        let mut f = NavigationFixture::new(Mode::Playing);
        let plot = f.frame(Vec::new(), 0.0);
        let start = Pos2::new((plot.left + plot.ruler.right()) * 0.5, plot.ruler.center().y);
        f.frame(vec![egui::Event::PointerMoved(start)], 0.0);
        f.button(start, egui::PointerButton::Primary, true);
        let edge = if direction < 0.0 { plot.left } else { plot.ruler.right() };
        let outside = Pos2::new(edge + direction * 50.0, start.y);
        f.frame(vec![egui::Event::PointerMoved(outside)], 0.0);
        assert!(f.editor.scrubbing);
        for _ in 0..4 {
            let view = f.state.view_start;
            let seek = f.editor.pending_seek.unwrap();
            f.frame(Vec::new(), 0.0);
            assert!((f.state.view_start - view) * direction > 0.0);
            assert!((f.editor.pending_seek.unwrap() - seek) * direction > 0.0);
        }
        f.frame(vec![egui::Event::PointerMoved(start)], 0.0);
        let view = f.state.view_start;
        f.frame(Vec::new(), 0.0);
        assert_eq!(f.state.view_start, view, "returning inside stops edge pan");
        f.frame(vec![egui::Event::PointerMoved(outside)], 0.0);
        f.button(outside, egui::PointerButton::Primary, false);
        assert!(!f.editor.scrubbing && f.state.follow);
        let plot = f.frame(Vec::new(), 0.0);
        let x = (f.app.timeline_time() - f.state.view_start) * f.state.pixels_per_second;
        assert!((x - (plot.ruler.right() - plot.left) * 0.5).abs() < 0.01, "playback resumes following");
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
