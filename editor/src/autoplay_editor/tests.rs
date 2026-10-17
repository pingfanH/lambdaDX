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
        f.editor.autoplay_snap_enabled = false;
        f.state.return_current();
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
        f.editor.autoplay_snap_division = division;
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
    f.frame(Vec::new(), 0.0);
    assert_eq!(f.state.view_start, view, "playback leaves a visible left-half playhead alone");
    for _ in 0..30 { f.frame(Vec::new(), 0.0); }
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
                let target = f.app.timeline_time() - (plot.ruler.right() - plot.left) * 0.5 / f.state.pixels_per_second;
                if f.app.timeline_time() >= old_view && target <= old_view {
                    assert_eq!(f.state.view_start, old_view, "follow starts only at the midpoint");
                } else {
                    assert!(f.state.view_start >= old_view.min(target) - 0.0001);
                    assert!(f.state.view_start <= old_view.max(target) + 0.0001);
                }
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
fn timeline_navigation_midpoint_follow_eases_and_pauses_without_recentering() {
    let mut f = NavigationFixture::new(Mode::Playing);
    let span = f.state.plot_width / f.state.pixels_per_second;
    f.state.view_start = f.app.timeline_time() - span * 0.25;
    f.state.follow_motion.reset();
    let view = f.state.view_start;
    for _ in 0..10 { f.frame(Vec::new(), 0.0); }
    assert_eq!(f.state.view_start, view);

    f.state.view_start = f.app.timeline_time() - span * 0.8;
    let view = f.state.view_start;
    f.frame(Vec::new(), 0.0);
    let target = f.app.timeline_time() - span * 0.5;
    assert!(f.state.view_start > view && f.state.view_start < target);
    f.app.mode = Mode::Idle;
    let view = f.state.view_start;
    for _ in 0..20 { f.frame(Vec::new(), 0.0); }
    assert_eq!(f.state.view_start, view, "pause stops the transition immediately");

    f.state.toggle_follow();
    f.state.toggle_follow();
    f.frame(Vec::new(), 0.0);
    assert_eq!(f.state.view_start, view, "enabling follow is not return-to-playhead");
    f.app.mode = Mode::Recording;
    f.frame(Vec::new(), 0.0);
    assert!(f.state.view_start < f.app.timeline_time() - span * 0.5);
    for _ in 0..20 { f.frame(Vec::new(), 0.0); }
    assert!((f.state.view_start - (f.app.timeline_time() - span * 0.5)).abs() < 0.0001);
    assert!(f.editor.pending_seek.is_none());
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
        for _ in 0..20 { f.frame(Vec::new(), 0.0); }
        let plot = f.frame(Vec::new(), 0.0);
        let x = (f.app.timeline_time() - f.state.view_start) * pps;
        assert!(x >= 0.0 && x <= (plot.ruler.right() - plot.left) * 0.5 + 0.01, "playback resumes midpoint following");
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
        for _ in 0..20 { f.frame(Vec::new(), 0.0); }
        let plot = f.frame(Vec::new(), 0.0);
        let x = (f.app.timeline_time() - f.state.view_start) * f.state.pixels_per_second;
        assert!(x >= 0.0 && x <= (plot.ruler.right() - plot.left) * 0.5 + 0.01, "playback resumes midpoint following");
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
fn workspace_upgrade_curve_motion_and_sensor_bake() {
    use crate::app::autoplay_edit::{AutoplayCurve, CurveTiming};
    let mut source = AutoplayCurve { points: vec![[-0.8, 0.0], [-0.7, 0.0], [0.8, 0.0]],
        smooth: false, speed: 2.0, ..Default::default() };
    let path = curve::Path::new(&source);
    assert!(path.at(0.5).distance(Pos2::ZERO) < 1e-5, "motion must use arc length");
    assert!((path.duration(&source) - 0.8).abs() < 1e-5);
    source.points.push([1.6, 0.0]);
    assert!((curve::Path::new(&source).duration(&source) - 1.2).abs() < 1e-5);
    source.timing = CurveTiming::FixedDuration;
    source.duration = 0.8;
    assert_eq!(curve::Path::new(&source).duration(&source), 0.8);
    source.points.pop();
    let svg = crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg")).unwrap();
    let events = curve::bake(&source, &svg).unwrap();
    let mut held = HashSet::new();
    for event in &events {
        match event_kind(event) {
            1 => assert!(held.insert(event_zone(event))),
            2 => assert!(held.remove(&event_zone(event))),
            _ => panic!("curve must bake paired sensor holds"),
        }
        assert!((0..=800_000).contains(&timed_input_tp(event)));
    }
    assert!(held.is_empty());
    assert!(events.windows(2).all(|pair| timed_input_tp(&pair[0]) <= timed_input_tp(&pair[1])));
}

#[test]
fn workspace_upgrade_conslide_maps_one_segment_per_part() {
    let mut app = player(chart("1-5-8[0.5##2]"));
    app.pad_svg = Some(
        crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
            .unwrap(),
    );
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let rows: Vec<_> = state
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.key.role == 2)
        .map(|(row, _)| row)
        .collect();
    assert!(rows.len() >= 2, "a conslide has one part per segment");
    let mut curves: Vec<_> = rows
        .iter()
        .map(|&row| {
            let curve = curve::from_slide(&state.tracks[row].key, &app).unwrap();
            (state.tracks[row].key.segment, curve)
        })
        .collect();
    curves.sort_by_key(|(segment, _)| *segment);
    assert_ne!(
        curves[0].1.points, curves[1].1.points,
        "each part must draw only its own segment"
    );
    let joint = Pos2::from(*curves[0].1.points.last().unwrap());
    let start = Pos2::from(curves[1].1.points[0]);
    assert!(
        joint.distance(start) < 0.01,
        "consecutive segments meet at the joint: {joint:?} vs {start:?}"
    );
}

#[test]
fn workspace_upgrade_new_curve_ignores_the_selected_clip() {
    let mut app = player(chart("1-5-8[0.5##2]"));
    app.pad_svg = Some(
        crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
            .unwrap(),
    );
    let mut state = AutoplayEditor::default();
    let mut editor = EditorState::default();
    state.sync(&app);
    let slide = state.tracks.iter().position(|track| track.key.role == 2).unwrap();
    let before = state.tracks.len();
    state.select_clip(slide, &app);
    state.new_curve(&mut editor, &mut app);
    assert!(state.curve_editing(), "the pen must start a draft");
    let draft_row = state.selected_row.expect("the draft names its clip");
    assert!(
        state.tracks[draft_row].key.is_standalone(),
        "a drawn curve must be its own clip, not the selected one"
    );
    assert_eq!(state.tracks.len(), before + 1);
    let overrides = app.chart.autoplay_overrides.clone();
    state.cancel_curve();
    assert_eq!(state.tracks.len(), before, "cancelling drops only the new clip");
    assert!(draft_row >= state.tracks.len() || !state.tracks[draft_row].key.is_standalone());
    assert_eq!(state.tracks[slide].key.role, 2, "the selected group survives");
    assert_eq!(app.chart.autoplay_overrides, overrides);
}

#[test]
fn workspace_upgrade_wifi_curve_maps_three_straight_legs() {
    let mut app = player(chart("1w5[0.25##1]"));
    app.pad_svg = Some(
        crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
            .unwrap(),
    );
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let rows: Vec<_> = state
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.key.role == 2)
        .map(|(row, _)| row)
        .collect();
    assert_eq!(rows.len(), 1, "the core reports one combined Wi-Fi body");
    let mapped = curve::from_slide(&state.tracks[rows[0]].key, &app).unwrap();
    assert_eq!(mapped.points.len(), 6, "three straight legs through the head");
    assert!(mapped.speed > 0.0 && mapped.duration > 0.0);
    let events = curve::bake(&mapped, app.pad_svg.as_ref().unwrap()).unwrap();
    assert!(!events.is_empty(), "the fan must press the sensors it crosses");
}

#[test]
fn workspace_upgrade_elements_stay_separate_and_resize_by_band() {
    let mut app = player(chart("1-5-8[0.5##2]"));
    app.pad_svg = Some(
        crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
            .unwrap(),
    );
    let mut state = AutoplayEditor::default();
    let mut editor = EditorState::default();
    editor.autoplay_snap_enabled = false;
    state.sync(&app);
    assert_eq!(
        state.clip_layout.clips.len(),
        state.tracks.len(),
        "every real owner is its own element by default"
    );
    assert!(state.clip_layout.clips.iter().all(|clip| clip.rows.len() == 1));
    let row = state
        .tracks
        .iter()
        .position(|track| track.key.role == 2)
        .unwrap();
    state.select_clip(row, &app);
    assert!(
        state.element_curve(row, &app).is_some(),
        "a Slide element shows its curve without mapping it"
    );
    assert!(app.chart.autoplay_overrides.is_empty(), "showing a curve authors nothing");
    let (start, end) = state.element_span(row, &app);
    let duration = end - start;
    assert!(duration > 0.05, "the element spans its own duration: {duration}");
    clips::begin_band(&mut state, &mut editor, &mut app, row, clips::BandEdge::End, 0.0, end);
    let pps = state.pixels_per_second;
    clips::update_band(&mut state, &editor, &app, duration * pps);
    let (_, stretched) = state.element_span(row, &app);
    assert!(stretched > end + 0.05, "the right grip lengthens the element: {stretched} vs {end}");
    clips::finish_band(&mut state, &mut editor, &mut app, true);
    let saved = app.chart.autoplay_overrides[0]
        .curve
        .clone()
        .expect("resizing authors the curve");
    assert!(
        saved.duration > duration as f64 * 1.5,
        "the saved duration follows the grip: {:?} vs {duration}",
        saved.duration
    );
    assert_eq!(editor.undo.len(), 1, "one step per resize");
}

#[test]
fn workspace_upgrade_curve_element_follows_a_live_drag() {
    let mut app = player(chart("1-5-8[0.5##2]"));
    app.pad_svg = Some(
        crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
            .unwrap(),
    );
    let mut state = AutoplayEditor::default();
    let mut editor = EditorState::default();
    editor.autoplay_snap_enabled = false;
    state.sync(&app);
    let row = state
        .tracks
        .iter()
        .position(|track| track.key.role == 2)
        .unwrap();
    state.select_clip(row, &app);
    let (start, _) = state.element_span(row, &app);
    let curve = state
        .element_curve(row, &app)
        .expect("a Slide element shows a curve");
    let index = state.tracks[row]
        .events
        .iter()
        .enumerate()
        .min_by_key(|(_, event)| timed_input_tp(event))
        .map(|(index, _)| index)
        .unwrap();
    state.selected = Some((row, index));
    state.selected_pair = None;
    clips::begin_selected(&mut state, &egui::Context::default(), false);
    assert!(state.clip_drag.is_some(), "the press starts an element drag");
    // The frame loop moves the dragged events; the box and its band must
    // follow immediately instead of waiting for the confirming release.
    let shift = 500_000;
    for event in state.tracks[row].events.iter_mut() {
        *event = autoplay_edit::with_time(event, timed_input_tp(event) + shift);
    }
    let (moved, _) = state.element_span(row, &app);
    assert!((moved - start - 0.5).abs() < 1e-4, "{moved} vs {start}");
    assert!(
        (state.element_curve(row, &app).unwrap().start - curve.start - 0.5).abs() < 1e-6,
        "the previewed curve must shift with the drag"
    );
    state.clip_drag = None;
    assert!((state.element_curve(row, &app).unwrap().start - curve.start).abs() < 1e-9);
}

#[test]
fn workspace_upgrade_slide_lanes_follow_taps_and_view_options() {
    let lane_of = |state: &AutoplayEditor, row: usize| {
        state
            .clip_layout
            .lanes
            .iter()
            .position(|lane| {
                lane.iter()
                    .any(|&index| state.clip_layout.clips[index].rows.contains(&row))
            })
            .expect("every element has a lane")
    };
    // The tap sits inside the slide's travel, so their boxes really overlap.
    let app = player(chart("1-5[4:2],2"));
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let tap = state
        .tracks
        .iter()
        .position(|track| track.key.role == 0)
        .expect("tap");
    let slide = state
        .tracks
        .iter()
        .position(|track| track.key.role == 2)
        .expect("slide body");
    assert!(
        lane_of(&state, tap) < lane_of(&state, slide),
        "a Slide sharing time with a tap must stay on the lower lane"
    );
    state.slide_new_lane = true;
    state.lanes_dirty = true;
    state.repack();
    assert!(lane_of(&state, slide) >= 1, "the option lifts every slide");
    let auto = lane_of(&state, slide);
    state.pin_lane(slide, 2);
    assert_eq!(lane_of(&state, slide), 2, "a vertical drag pins the lane");
    state.slide_new_lane = false;
    state.selected_row = Some(slide);
    state.clear_lane_pins();
    assert_eq!(lane_of(&state, slide), auto, "auto placement takes the pin back");
}

#[test]
fn workspace_upgrade_multi_selection_deletes_every_picked_element() {
    let mut app = player(chart("1-5-8[0.5##2]"));
    let mut state = AutoplayEditor::default();
    let mut editor = EditorState::default();
    state.sync(&app);
    let rows: Vec<_> = state
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.key.role == 2)
        .map(|(row, _)| row)
        .collect();
    assert!(rows.len() >= 2, "conslide segments stay apart");
    state.select_clip(rows[0], &app);
    assert_eq!(state.selected_elements(), vec![rows[0]], "a plain click picks one element");
    state.toggle_row(rows[1]);
    assert_eq!(state.selected_elements(), rows[..2].to_vec());
    state.delete_elements(&mut editor, &mut app.chart);
    assert!(state.selection.is_empty());
    for row in &rows[..2] {
        let key = state.tracks[*row].key.clone();
        let item = app
            .chart
            .autoplay_overrides
            .iter()
            .find(|item| item.key == key)
            .expect("a disabled element keeps its explicit empty override");
        assert!(item.events.is_empty());
    }
    assert_eq!(editor.undo.len(), 1, "the whole pick is one undo step");
}

#[test]
fn workspace_upgrade_curve_handles_and_shape_preserving_insertion() {
    use crate::app::autoplay_edit::AutoplayCurve;
    let source = AutoplayCurve { points: vec![[-0.8, 0.0], [0.0, 0.6], [0.8, 0.0]],
        ..Default::default() };
    let legacy = serde_json::to_value(&source).unwrap();
    assert!(legacy.get("handles").is_none());
    let mut source: AutoplayCurve = serde_json::from_value(legacy).unwrap();
    curve::ensure_handles(&mut source);
    let old = source.handles[1].clone();
    curve::move_point(&mut source, 1, [0.1, 0.7]);
    for (before, after) in [(old.incoming, source.handles[1].incoming),
        (old.outgoing, source.handles[1].outgoing)] {
        assert!((Pos2::from(after) - Pos2::from(before) - Vec2::splat(0.1)).length() < 1e-6);
    }
    let center = Pos2::from(source.points[1]);
    let opposite_length = center.distance(Pos2::from(source.handles[1].incoming));
    curve::move_handle(&mut source, 1, false, [0.4, 1.0], false);
    let incoming = Pos2::from(source.handles[1].incoming) - center;
    let outgoing = Pos2::from(source.handles[1].outgoing) - center;
    assert!((incoming.length() - opposite_length).abs() < 1e-6);
    assert!(incoming.normalized().dot(outgoing.normalized()) < -0.9999);
    let opposite = source.handles[1].incoming;
    curve::move_handle(&mut source, 1, false, [0.5, 0.5], true);
    assert_eq!(source.handles[1].incoming, opposite);
    assert!(!source.handles[1].aligned);
    let before = curve::Path::new(&source);
    let count = source.points.len();
    let inserted = curve::insert_nearest(&mut source, before.at(0.3), 0.003).unwrap();
    assert_eq!(source.points.len(), count + 1);
    assert_eq!(source.handles.len(), source.points.len());
    let after = curve::Path::new(&source);
    assert!((after.length - before.length).abs() < 0.003);
    for i in 0..=100 {
        assert!(before.at(i as f32 / 100.0).distance(after.at(i as f32 / 100.0)) < 0.003);
    }
    let roundtrip: AutoplayCurve = serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
    assert_eq!(roundtrip, source);
    curve::remove_point(&mut source, inserted);
    assert_eq!(source.handles.len(), count);
}

#[test]
fn workspace_upgrade_slide_curve_commit_save_move_and_undo() {
    let mut app = player(chart("1-5-8[0.5##2]"));
    app.pad_svg = Some(crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg")).unwrap());
    let mut state = AutoplayEditor::default();
    let mut editor = EditorState::default();
    state.sync(&app);
    let rows: Vec<_> = state.tracks.iter().enumerate().filter(|(_, t)| t.key.role == 2).map(|(r, _)| r).collect();
    assert!(!rows.is_empty());
    for &row in &rows {
        let mapped = curve::from_slide(&state.tracks[row].key, &app).unwrap();
        assert!(mapped.duration > 0.0 && mapped.points.len() >= 2);
        assert!(!curve::bake(&mapped, app.pad_svg.as_ref().unwrap()).unwrap().is_empty());
    }
    let row = rows[0];
    let before = app.chart.autoplay_overrides.clone();
    let generated = state.tracks[row].events.clone();
    state.select_clip(row, &app);
    assert!(state.curve_view(row, &app.chart));
    assert_eq!(state.detail_row, Some(row));
    assert_eq!(app.chart.autoplay_overrides, before, "selection must not author events");
    assert_eq!(state.tracks[row].events, generated);
    state.raw_events = true;
    state.edit_curve(&mut editor, &mut app, row, false);
    assert!(state.curve_editing());
    assert!(!state.raw_events);
    state.apply_curve(&mut editor, &mut app);
    assert!(!state.curve_editing());
    let saved = app.chart.autoplay_overrides[0].clone();
    assert!(!saved.curve.as_ref().unwrap().handles.is_empty());
    assert!(state.curve_view(row, &app.chart));
    let json = serde_json::to_string(&app.chart).unwrap();
    let reopened: ChartDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(reopened.autoplay_overrides, app.chart.autoplay_overrides);
    state.edit_curve(&mut editor, &mut app, row, false);
    assert_eq!(state.curve_for(row, &app).unwrap(), saved.curve.clone().unwrap());
    state.cancel_curve();
    let translated: Vec<_> = state.tracks[row].events.iter().map(|event|
        autoplay_edit::with_time(event, timed_input_tp(event) + 250_000)).collect();
    state.commit(&mut editor, &mut app.chart, row, translated);
    let moved = app.chart.autoplay_overrides[0].clone();
    assert!((moved.curve.as_ref().unwrap().start - saved.curve.as_ref().unwrap().start - 0.25).abs() < 1e-9);
    let mut raw = state.tracks[row].events.clone();
    raw[0] = autoplay_edit::with_time(&raw[0], timed_input_tp(&raw[0]) + 1);
    state.commit(&mut editor, &mut app.chart, row, raw);
    assert!(app.chart.autoplay_overrides[0].curve.is_none());
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.autoplay_overrides, vec![moved]);
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.autoplay_overrides, vec![saved]);
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.autoplay_overrides, before);
    state.sync(&app);
    state.raw_events = true;
    let track_count = state.tracks.len();
    state.new_curve(&mut editor, &mut app);
    assert_eq!(state.tracks.len(), track_count + 1);
    assert_eq!(state.layouts.len(), state.tracks.len(), "draft must have a raw-view layout");
    state.cancel_curve();
    assert_eq!(state.tracks.len(), track_count);
    assert_eq!(app.chart.autoplay_overrides, before);
    state.raw_events = false;
    state.select_clip(row, &app);
    let translated = generated.iter().map(|event|
        autoplay_edit::with_time(event, timed_input_tp(event) + 250_000)).collect();
    state.commit(&mut editor, &mut app.chart, row, translated);
    let moved = app.chart.autoplay_overrides.clone();
    assert!(moved[0].curve.is_some(), "moving a generated curve keeps its geometry");
    state.reset_rows(&mut editor, &mut app.chart, &rows);
    assert!(app.chart.autoplay_overrides.is_empty(), "reset removes the curve override as well");
    assert_eq!(state.tracks[row].events, generated);
    editor.undo(&mut app.chart);
    assert_eq!(app.chart.autoplay_overrides, moved);
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
fn sensor_edit_pad_changes_both_hold_ends_and_undo_restores_owner() {
    let mut app = player(chart("1h[4:2],B3"));
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let row = state.tracks.iter().position(|t| t.key.note_id == 100).unwrap();
    let original = state.tracks[row].events.clone();
    let others: Vec<_> = state.tracks.iter().enumerate().filter(|(r, _)| *r != row)
        .map(|(_, track)| track.events.clone()).collect();
    let span = clips::intervals(&original).into_iter().find(|s| s.release.is_some()).unwrap();
    let release = span.release.unwrap();
    state.selected_row = Some(row);
    state.selected = Some((row, release));
    let mut editor = EditorState::default();
    state.pick_zone(&mut editor, &mut app.chart, "K2", 0.0);
    for index in [span.press, release] {
        assert_eq!(event_zone(&state.tracks[row].events[index]), "K2");
        assert_eq!(timed_input_tp(&state.tracks[row].events[index]), timed_input_tp(&original[index]));
        assert_eq!(event_kind(&state.tracks[row].events[index]), event_kind(&original[index]));
    }
    assert_eq!(state.pad_zone(), "K2", "explicit buttons survive the inner-screen default");
    assert_eq!(editor.undo.len(), 1);
    state.pick_zone(&mut editor, &mut app.chart, "K2", 0.0);
    assert_eq!(editor.undo.len(), 1, "picking the same sensor is not an edit");
    assert_eq!(state.tracks.iter().enumerate().filter(|(r, _)| *r != row)
        .map(|(_, t)| t.events.clone()).collect::<Vec<_>>(), others);
    let edited = state.tracks[row].events.clone();
    state.delete(&mut editor, &mut app.chart);
    assert_eq!(state.tracks[row].events.len(), original.len() - 2, "Release selection deletes the whole interval");
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks[row].events, edited);
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks[row].events, original);
    assert!(app.chart.autoplay_overrides.is_empty());
}

#[test]
fn event_type_toolbar_converts_selection_without_changing_drawing_events() {
    let ctx = egui::Context::default();
    crate::ui::style::apply(&ctx);
    let mut time = 0.0;
    let mut choose = |hold: bool, state: &mut AutoplayEditor, editor: &mut EditorState, app: &mut PadPreviewState| {
        let mut pos = Pos2::ZERO;
        for pressed in [None, Some(true), Some(false)] {
            time += 1.0 / 60.0;
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary,
                    pressed, modifiers: egui::Modifiers::NONE });
            }
            let _ = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 200.0))),
                time: Some(time), events, ..Default::default()
            }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        pos = ui.cursor().min + Vec2::new(
                            (22.0 + ui.spacing().item_spacing.x) * if hold { 3.0 } else { 2.0 } + 11.0, 11.0);
                        drawing_tools(ui, state, editor, app);
                    });
                });
            });
        }
        assert_eq!(state.new_hold, hold, "the toolbar button must receive the click");
    };
    for standalone in [false, true] {
        let mut app = player(chart("1,2"));
        let mut state = AutoplayEditor::default();
        state.sync(&app);
        let key = if standalone { AutoplayKey::standalone(1) } else { state.tracks[1].key.clone() };
        let head = if standalone { 0 } else { state.tracks[1].head };
        let zone = if standalone { "K2" } else { "B3" };
        let start = head + 123_457;
        let original = vec![make_event(zone, 2, start + 900_000), make_event(zone, 0, start),
            make_event(zone, 1, start + 500_000)];
        app.chart.autoplay_overrides.push(AutoplayOverride {
            curve: None,
            key: key.clone(), events: autoplay_edit::relative(&original, head),
        });
        state.sync(&app);
        let row = state.tracks.iter().position(|track| track.key == key).unwrap();
        let other_events = state.tracks[0].events.clone();
        state.selected_row = Some(row);
        state.selected = Some((row, 1));
        state.new_hold = false;
        let mut editor = EditorState::default();

        choose(true, &mut state, &mut editor, &mut app);
        let mut held = original.clone();
        held[1] = make_event(zone, 1, start);
        held.push(make_event(zone, 2, start + 125_000));
        assert_eq!(state.tracks[row].events, held, "Click becomes a one-subdivision Hold without snapping its start");
        assert_eq!(state.selected_pair, Some(3));
        assert_eq!(editor.undo.len(), 1);
        state.sync(&app);
        assert_eq!(state.selected, Some((row, 1)));
        assert_eq!(state.selected_pair, Some(3));
        choose(true, &mut state, &mut editor, &mut app);
        assert_eq!(editor.undo.len(), 1, "same type is not another edit");

        state.selected = Some((row, 3));
        state.selected_pair = None;
        choose(false, &mut state, &mut editor, &mut app);
        assert_eq!(state.tracks[row].events, original, "Release selection converts the whole Hold at its start");
        assert_eq!(state.selected, Some((row, 1)));
        assert_eq!(state.selected_pair, None);
        assert_eq!(editor.undo.len(), 2);

        state.selected = Some((row, 0));
        choose(true, &mut state, &mut editor, &mut app);
        assert_eq!(state.tracks[row].events, original, "existing custom duration is preserved");
        assert_eq!(editor.undo.len(), 2);
        choose(false, &mut state, &mut editor, &mut app);
        assert_eq!(state.tracks[row].events, [make_event(zone, 0, start), make_event(zone, 0, start + 500_000)]);
        assert_eq!(state.selected, Some((row, 1)), "selection survives removing an earlier vector index");
        assert_eq!(editor.undo.len(), 3);
        editor.undo(&mut app.chart);
        state.sync(&app);
        assert_eq!(state.tracks[row].events, original);
        assert_eq!(app.chart.autoplay_overrides[0].key, key);
        assert_eq!(app.chart.autoplay_overrides[0].events, autoplay_edit::relative(&original, head));
        assert_eq!(state.tracks[0].events, other_events);

        state.start_drawing();
        state.new_hold = true;
        state.pick_zone(&mut editor, &mut app.chart, zone, 2.0);
        let drawn = state.tracks[row].events.clone();
        let history = editor.undo.len();
        choose(false, &mut state, &mut editor, &mut app);
        assert_eq!(state.tracks[row].events, drawn, "drawing choice must not convert the last-created interval");
        assert_eq!(editor.undo.len(), history);
    }
}

#[test]
fn sensor_edit_pad_draws_snapped_hold_or_click_as_one_undo_step() {
    let mut app = player(chart("1"));
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    state.selected_row = Some(0);
    state.selected = Some((0, 0));
    state.start_drawing();
    assert!(state.selected.is_none());
    assert!(state.detail_row.is_none(), "drawing must not open a second timeline");
    let original = state.tracks[0].events.clone();
    let mut editor = EditorState::default();
    state.pick_zone(&mut editor, &mut app.chart, "C", 0.19);
    assert_eq!(&state.tracks[0].events[original.len()..],
        &[make_event("C", 1, 250_000), make_event("C", 2, 375_000)]);
    assert_eq!(state.selected_pair, Some(original.len() + 1));
    assert!(state.detail_row.is_none(), "Pad additions stay in the main track");
    assert_eq!(editor.undo.len(), 1);
    state.new_hold = false;
    state.pick_zone(&mut editor, &mut app.chart, "B8", 0.62);
    assert_eq!(state.tracks[0].events.last(), Some(&make_event("B8", 0, 625_000)));
    assert_eq!(state.selected_pair, None);
    assert_eq!(editor.undo.len(), 2);
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks[0].events.len(), original.len() + 2);
    state.suppress_pointer = true;
    state.pick_zone(&mut editor, &mut app.chart, "D1", 0.0);
    assert_eq!(editor.undo.len(), 1, "undo must not also create a Pad event");
    assert_eq!(state.tracks[0].events.len(), original.len() + 2);
}

#[test]
fn sensor_edit_blank_track_creates_without_selecting_a_note() {
    let mut app = player(chart("1"));
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    state.follow = false;
    state.view_start = 0.0;
    state.pixels_per_second = 200.0;
    state.new_zone = "B3".into();
    state.start_drawing();
    let original = app.chart.clone();
    let mut editor = EditorState::default();
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |state: &mut AutoplayEditor, editor: &mut EditorState, app: &mut PadPreviewState,
        pos: Pos2, pressed: Option<bool>| {
        time += 1.0 / 60.0;
        state.sync(app);
        let mut events = vec![egui::Event::PointerMoved(pos)];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary,
                pressed, modifiers: egui::Modifiers::NONE });
        }
        let mut origin = Pos2::ZERO;
        let _ = ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(time), events, ..Default::default()
        }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                origin = ui.cursor().min + Vec2::new(HEADER, 26.0 + ui.spacing().item_spacing.y);
                clips::draw(ui, state, editor, app, true, 0.0);
            });
        });
        origin
    };
    frame(&mut state, &mut editor, &mut app, Pos2::ZERO, None);
    let origin = frame(&mut state, &mut editor, &mut app, Pos2::ZERO, None);
    let point = |seconds: f32| origin + Vec2::new(seconds * 200.0, 17.0);
    frame(&mut state, &mut editor, &mut app, point(0.5), Some(true));
    frame(&mut state, &mut editor, &mut app, point(0.5), Some(false));
    assert_eq!(app.chart.autoplay_overrides.len(), 1, "drawing on blank track requires no note selection");
    assert_eq!(app.chart.notes, original.notes, "a standalone event must not create a chart note");
    assert_eq!(app.chart.autoplay_overrides[0].events,
        [make_event("B3", 1, 500_000), make_event("B3", 2, 625_000)]);
    assert!(state.detail_row.is_none());
    assert_eq!(editor.undo.len(), 1);

    frame(&mut state, &mut editor, &mut app, point(1.0), None);
    frame(&mut state, &mut editor, &mut app, point(1.0), Some(true));
    frame(&mut state, &mut editor, &mut app, point(2.0), None);
    let row = state.selected_row.unwrap();
    assert!(state.tracks[row].key.is_standalone());
    assert!(state.clip_layout.clips.iter().any(|clip| clip.rows.contains(&row)), "new interval is visible while drawing");
    assert_eq!(app.chart.autoplay_overrides.len(), 1, "draft does not edit the document");
    state.cancel();
    frame(&mut state, &mut editor, &mut app, point(2.0), Some(false));
    assert_eq!(state.tracks.len(), 2, "cancel removes the temporary standalone track");
    assert_eq!(editor.undo.len(), 1);

    frame(&mut state, &mut editor, &mut app, point(1.0), None);
    frame(&mut state, &mut editor, &mut app, point(1.0), Some(true));
    frame(&mut state, &mut editor, &mut app, point(2.0), None);
    frame(&mut state, &mut editor, &mut app, point(2.0), Some(false));
    assert_eq!(app.chart.autoplay_overrides.len(), 2);
    assert_eq!(app.chart.autoplay_overrides[1].events,
        [make_event("B3", 1, 1_000_000), make_event("B3", 2, 2_000_000)]);
    assert_eq!(editor.undo.len(), 2, "release commits exactly one interval");
}

#[test]
fn sensor_edit_standalone_events_persist_play_and_undo_without_notes() {
    let mut app = player(chart(""));
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    let mut editor = EditorState::default();
    state.start_drawing();
    state.pick_zone(&mut editor, &mut app.chart, "B3", 0.5);
    state.sync(&app);
    state.new_hold = false;
    state.pick_zone(&mut editor, &mut app.chart, "K2", 1.0);
    state.sync(&app);
    assert!(app.chart.notes.is_empty());
    assert_eq!(state.tracks.len(), 2, "continuous drawing creates independent clips");
    assert!(state.tracks.iter().all(|track| track.key.is_standalone()));
    assert_eq!(editor.undo.len(), 2);

    let saved = serde_json::to_string(&crate::templates::document(&editor, &app.chart)).unwrap();
    let mut restored = player(serde_json::from_str(&saved).unwrap());
    let mut reopened = AutoplayEditor::default();
    reopened.sync(&restored);
    assert_eq!(reopened.tracks.len(), 2);
    let expected = vec![make_event("B3", 1, 500_000), make_event("B3", 2, 625_000), make_event("K2", 0, 1_000_000)];
    assert_eq!(restored.autoplay_tactic, expected);
    let mut plan = restored.autoplay_plan.clone().unwrap();
    assert_eq!(plan.retained(&[]).events(), expected, "seek retains independent inputs");
    autoplay_edit::apply(&restored.chart, restored.judge_engine.as_ref().unwrap(), &mut plan);
    assert_eq!(plan.events(), expected, "reapplying overrides must not duplicate independent events");
    assert_eq!(restored.song_duration(), 2.0, "standalone events extend silent preview");
    restored.try_seek_to(1.5).unwrap();
    assert_eq!(restored.mode_song_offset, 1.5, "seek is not clamped to the empty chart duration");
    assert_eq!(restored.autoplay_tactic, expected);

    state.delete(&mut editor, &mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks.len(), 1);
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks.len(), 2);
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks.len(), 1);
    editor.redo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks.len(), 2);
    state.clear_selection();
    state.add_at_playhead(&mut editor, &mut app.chart, 2.0);
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks.len(), 2, "undo before the next UI sync removes the draft track");
}

#[test]
fn sensor_edit_main_track_adds_and_resizes_holds_without_detail() {
    let mut app = player(chart("1"));
    let mut state = AutoplayEditor::default();
    state.sync(&app);
    state.follow = false;
    state.center_requested = false;
    state.view_start = 0.0;
    state.pixels_per_second = 200.0;
    state.selected_row = Some(0);
    state.new_zone = "B3".into();
    state.start_drawing();
    let original = state.tracks[0].events.clone();
    let mut editor = EditorState::default();
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |state: &mut AutoplayEditor, editor: &mut EditorState, app: &mut PadPreviewState,
        pos: Pos2, pressed: Option<bool>| {
        time += 1.0 / 60.0;
        state.sync(app);
        let mut events = vec![egui::Event::PointerMoved(pos)];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary,
                pressed, modifiers: egui::Modifiers::NONE });
        }
        let mut origin = Pos2::ZERO;
        let _ = ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(time), events, ..Default::default()
        }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                origin = ui.cursor().min + Vec2::new(HEADER, 26.0 + ui.spacing().item_spacing.y);
                clips::draw(ui, state, editor, app, true, 0.0);
            });
        });
        origin
    };
    frame(&mut state, &mut editor, &mut app, Pos2::ZERO, None);
    let origin = frame(&mut state, &mut editor, &mut app, Pos2::ZERO, None);
    let point = |seconds: f32, y: f32| origin + Vec2::new(seconds * 200.0, y);
    let click = point(0.5, 17.0);
    frame(&mut state, &mut editor, &mut app, click, Some(true));
    frame(&mut state, &mut editor, &mut app, click, Some(false));
    assert_eq!(&state.tracks[0].events[original.len()..],
        &[make_event("B3", 1, 500_000), make_event("B3", 2, 625_000)]);
    assert_eq!(editor.undo.len(), 1);
    assert!(state.detail_row.is_none());

    frame(&mut state, &mut editor, &mut app, point(1.0, 17.0), None);
    frame(&mut state, &mut editor, &mut app, point(1.0, 17.0), Some(true));
    frame(&mut state, &mut editor, &mut app, point(2.0, 17.0), None);
    assert!(app.chart.autoplay_overrides[0].events.len() == original.len() + 2, "drag remains a draft");
    frame(&mut state, &mut editor, &mut app, point(2.0, 17.0), Some(false));
    assert_eq!(&state.tracks[0].events[original.len() + 2..],
        &[make_event("B3", 1, 1_000_000), make_event("B3", 2, 2_000_000)]);
    assert_eq!(editor.undo.len(), 2, "release commits once without adding an extra interval");
    assert!(state.detail_row.is_none());

    frame(&mut state, &mut editor, &mut app, point(2.0, 42.0), None);
    frame(&mut state, &mut editor, &mut app, point(2.0, 42.0), Some(true));
    frame(&mut state, &mut editor, &mut app, point(2.5, 42.0), None);
    frame(&mut state, &mut editor, &mut app, point(2.5, 42.0), Some(false));
    assert_eq!(&state.tracks[0].events[original.len() + 2..],
        &[make_event("B3", 1, 1_000_000), make_event("B3", 2, 2_500_000)]);
    assert_eq!(&state.tracks[0].events[..original.len()], original.as_slice());
    assert_eq!(editor.undo.len(), 3);
    assert!(state.detail_row.is_none());
    frame(&mut state, &mut editor, &mut app, point(2.5, 42.0), None);
    editor.undo(&mut app.chart);
    state.sync(&app);
    assert_eq!(state.tracks[0].events.last(), Some(&make_event("B3", 2, 2_000_000)));
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
    editor.autoplay_snap_division = 16;
    assert_eq!(snap_time(0.19, &editor, &chart), 0.25);
    editor.autoplay_snap_enabled = false;
    assert!((snap_time(0.19, &editor, &chart) - 0.19).abs() < 1e-6);
}
