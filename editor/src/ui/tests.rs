use super::*;
use crate::app::types::Mode;
use crate::state::Tool;

struct Fixture {
    ctx: egui::Context,
    ui: EditorUi,
    editor: EditorState,
    app: PadPreviewState,
    time: f64,
}

impl Fixture {
    fn new() -> Self {
        let chart = crate::app::maidata::from_maidata("&inote_1=(120){4}1,,E", None).unwrap();
        let mut app = PadPreviewState::new_at(chart, None, None, 0.0, None);
        app.forced_time = Some(0.0);
        let mut fixture = Self {
            ctx: egui::Context::default(), ui: EditorUi::new(),
            editor: EditorState::default(), app, time: 0.0,
        };
        fixture.frame(Vec::new());
        fixture.frame(Vec::new());
        fixture
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.frame_with_modifiers(events, egui::Modifiers::NONE)
    }

    fn frame_with_modifiers(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) -> egui::FullOutput {
        self.time += 0.1;
        let now = if self.app.mode == Mode::Idle { self.app.timeline_view_time } else { self.app.song_time() };
        self.ctx.run(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 800.0))),
            time: Some(self.time), events, modifiers, ..Default::default()
        }, |ctx| self.ui.draw(ctx, &mut self.editor, &mut self.app, now, &WaveformPyramid::default(), 0.0))
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) -> egui::FullOutput {
        self.frame(vec![
            egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
        ])
    }

    fn click(&mut self, position: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(position)]);
        self.frame(vec![egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        self.frame(vec![egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }])
    }
}

fn text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() * 0.5),
        _ => None,
    }).unwrap_or_else(|| panic!("missing visible label: {label}"))
}

mod template_interactions {
    use super::*;
    use crate::app::types::{Note, measure_to_secs};
    use crate::state::EditorLayout;
    use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER, RULER_H};

    fn fixture(layout: EditorLayout) -> Fixture {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.pixels_per_second = 100.0;
        f.app.chart.notes[0].time = 1.5;
        f.editor.selected.insert(f.app.chart.notes[0].id);
        crate::templates::apply(&mut f.editor, &mut f.app, crate::templates::Action::CreateSelection);
        f.frame(Vec::new());
        f.frame(Vec::new());
        f
    }

    fn position(f: &Fixture, measure: f32, lane: f32) -> egui::Pos2 {
        let rect = f.ui.layout.timeline;
        let time = measure_to_secs(measure, &f.app.chart.bpms);
        match f.editor.layout {
            EditorLayout::Vertical => egui::pos2(
                rect.left() + LANE_GUTTER + lane * (rect.width() - LANE_GUTTER) / 9.0,
                rect.bottom() - JUDGE_MARGIN - time * f.editor.pixels_per_second,
            ),
            EditorLayout::Horizontal => egui::pos2(
                rect.left() + LANE_GUTTER + (time - f.editor.view_start) * f.editor.pixels_per_second,
                rect.top() + RULER_H + lane * (rect.height() - RULER_H) / 9.0,
            ),
        }
    }

    fn pointer(f: &mut Fixture, pos: egui::Pos2, pressed: bool) {
        let shift = egui::Modifiers { shift: true, ..Default::default() };
        f.frame_with_modifiers(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: shift },
        ], shift);
    }

    #[test]
    fn template_shift_drag_commits_once_outside_and_escape_restores_the_anchor() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            let history = f.editor.undo.len();
            let start = position(&f, 1.625, 6.5);
            let end = position(&f, 2.125, 6.5);
            pointer(&mut f, start, true);
            assert!(f.ui.templates.dragging());
            f.frame_with_modifiers(vec![egui::Event::PointerMoved(end)], egui::Modifiers { shift: true, ..Default::default() });
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 2.0);
            pointer(&mut f, egui::pos2(-20.0, -20.0), false);
            assert!(!f.ui.templates.dragging());
            assert_eq!(f.editor.undo.len(), history + 1);
            assert!(f.app.chart.notes.is_empty());
            assert!(f.editor.template_action.is_none());
            f.key(egui::Key::Z, egui::Modifiers { command: true, ..Default::default() });
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 1.5);

            let history = f.editor.undo.len();
            let start = position(&f, 1.625, 6.5);
            let end = position(&f, 1.875, 6.5);
            pointer(&mut f, start, true);
            f.frame_with_modifiers(vec![egui::Event::PointerMoved(end)], egui::Modifiers { shift: true, ..Default::default() });
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 1.75);
            f.key(egui::Key::Escape, egui::Modifiers::NONE);
            assert!(!f.ui.templates.dragging());
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 1.5);
            assert_eq!(f.editor.undo.len(), history);
            pointer(&mut f, egui::pos2(-20.0, -20.0), false);
        }
    }

    #[test]
    fn template_bounds_do_not_intercept_coincident_ordinary_note_heads() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            f.app.chart.notes.push(Note { id: 77, time: 1.5, lane: 1, ..Default::default() });
            f.app.refresh_playback_chart();
            let head = position(&f, 1.5, 0.5);
            f.click(head);
            assert_eq!(f.editor.selected.iter().copied().collect::<Vec<_>>(), [77]);
            assert!(!f.ui.templates.dragging());
            assert!(f.editor.template_action.is_none());
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 1.5);
        }
    }
}

fn tool_fill(output: &egui::FullOutput, ui: &EditorUi) -> egui::Color32 {
    let center = ui.layout.tools.min + egui::vec2(19.0, 19.0);
    output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Rect(rect) if rect.rect.size() == egui::vec2(30.0, 30.0) && rect.rect.contains(center) => Some(rect.fill),
        _ => None,
    }).expect("palette button must remain painted")
}

#[test]
fn space_runs_transport_once_through_full_ui() {
    let mut f = Fixture::new();
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
    assert!(f.editor.keyboard.transport.is_empty());
    f.frame(Vec::new());
    assert_eq!(f.app.mode, Mode::Playing);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Idle);
}

#[test]
fn file_shortcuts_request_save_as_export_and_open_without_placing_notes() {
    let mut f = Fixture::new();
    f.editor.dirty = true;
    let primary = egui::Modifiers { command: true, ..Default::default() };
    let shifted = egui::Modifiers { shift: true, ..primary };
    f.key(egui::Key::S, primary);
    assert!(f.editor.save_as_request, "an unsourced document must request Save As");
    assert!(f.editor.dirty);
    f.editor.save_as_request = false;
    f.key(egui::Key::S, shifted);
    assert!(f.editor.save_as_request);
    assert!(!f.editor.export_request);
    f.editor.save_as_request = false;
    f.key(egui::Key::E, shifted);
    assert!(f.editor.export_request);
    assert!(!f.editor.save_as_request);
    f.key(egui::Key::O, primary);
    assert!(f.editor.open_dialog);
    assert_eq!(f.app.chart.notes.len(), 1);
    assert_eq!(f.app.mode, Mode::Idle);
}

#[test]
fn file_menu_actions_reach_the_document_io_requests() {
    for (label, request) in [("打开谱面...", "open"), ("保存", "save_as"), ("另存为...", "save_as"), ("导出 Simai...", "export")] {
        let mut f = Fixture::new();
        f.editor.dirty = true;
        let baseline = f.frame(Vec::new());
        f.click(text_position(&baseline, "文件"));
        let menu = f.frame(Vec::new());
        f.click(text_position(&menu, label));
        assert!(match request {
            "open" => f.editor.open_dialog,
            "save_as" => f.editor.save_as_request,
            "export" => f.editor.export_request,
            _ => false,
        }, "{label}");
        assert!(f.editor.dirty);
        assert_eq!(f.app.chart.notes.len(), 1);
    }
}

#[test]
fn command_search_exposes_file_actions_and_does_not_leak_enter_to_placement() {
    for (search, request) in [("Open chart", "open"), ("Save chart", "save_as"), ("Save As", "save_as"), ("Export Simai", "export")] {
        let mut f = Fixture::new();
        f.key(egui::Key::F3, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text(search.into())]);
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(f.editor.keyboard.dialog.is_none());
        assert!(match request {
            "open" => f.editor.open_dialog,
            "save_as" => f.editor.save_as_request,
            "export" => f.editor.export_request,
            _ => false,
        }, "{search}");
        assert_eq!(f.app.chart.notes.len(), 1);
    }
}

#[test]
fn discard_does_not_mark_the_document_saved_before_open_succeeds() {
    let mut f = Fixture::new();
    f.editor.dirty = true;
    f.editor.pending_action = Some(crate::state::PendingAction::Open);
    f.key(egui::Key::ArrowRight, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.pending_confirmed);
    assert!(f.editor.dirty, "cancelled or failed open must retain unsaved changes");
}

#[test]
fn unsaved_prompt_waits_for_save_as_before_running_the_pending_action() {
    let mut f = Fixture::new();
    f.editor.dirty = true;
    f.editor.pending_action = Some(crate::state::PendingAction::Quit);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.save_as_request);
    assert!(f.editor.dirty);
    assert!(!f.editor.pending_confirmed);
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(f.editor.pending_action.is_none());
    assert!(f.editor.dirty);
}

#[test]
fn keyboard_tools_are_visible_and_accept_arrows_and_enter() {
    let mut f = Fixture::new();
    f.key(egui::Key::F4, egui::Modifiers::NONE);
    let output = f.frame(Vec::new());
    text_position(&output, "Tools");
    f.key(egui::Key::ArrowDown, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(f.editor.tool, Tool::Tap);
    assert!(f.editor.keyboard.dialog.is_none());
    assert!(f.app.chart.notes.len() == 1, "dialog confirmation must not place a note");
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.active);
    assert!(f.editor.keyboard.rect.is_positive());
}

#[test]
fn clicking_play_does_not_flash_workspace_chrome() {
    let mut f = Fixture::new();
    let baseline = f.frame(Vec::new());
    let color = tool_fill(&baseline, &f.ui);
    let menu = text_position(&baseline, "播放");
    f.click(menu);
    let opened = f.frame(Vec::new());
    let item = opened.shapes.iter().filter_map(|s| match &s.shape {
        egui::Shape::Text(text) if text.galley.text() == "播放" && text.pos.y > menu.y + 5.0 => Some(text.pos + text.galley.size() * 0.5),
        _ => None,
    }).next().expect("play menu item must be visible");
    let clicked = f.click(item);
    assert_eq!(f.app.mode, Mode::Playing);
    assert_eq!(tool_fill(&clicked, &f.ui), color, "closing-click frame must not dim the palette");
    let timeline = f.ui.layout.timeline;
    let next = f.frame(vec![egui::Event::PointerGone]);
    assert_eq!(f.ui.layout.timeline, timeline);
    assert_eq!(tool_fill(&next, &f.ui), color);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Idle, "menu must release keyboard ownership");
}

#[test]
fn held_space_does_not_toggle_playback_repeatedly() {
    let mut f = Fixture::new();
    let event = |repeat| egui::Event::Key {
        key: egui::Key::Space, physical_key: None, pressed: true, repeat, modifiers: egui::Modifiers::NONE,
    };
    f.frame(vec![event(false)]);
    assert_eq!(f.app.mode, Mode::Playing);
    f.frame(vec![event(true)]);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn jump_accepts_typing_and_releases_space_after_confirmation() {
    let mut f = Fixture::new();
    f.app.chart.notes[0].time = 10.0;
    f.key(egui::Key::J, egui::Modifiers::NONE);
    f.frame(Vec::new());
    f.frame(vec![egui::Event::Text("2.5".into())]);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Idle);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.dialog.is_none());
    assert_eq!(f.editor.keyboard.cursor.measure, 2.5);
    assert_eq!(f.app.timeline_view_time, 3.0, "jump must seek playback in the confirmation frame");
    assert_eq!(f.editor.view_start, 3.0);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn jump_aligns_cursor_and_view_while_playing_and_clamps_to_song_end() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 10.0;
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        f.key(egui::Key::J, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text("3s".into())]);
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Playing);
        assert_eq!(f.app.timeline_view_time, 3.0);
        assert_eq!(f.editor.keyboard.cursor.measure, 2.5);
        assert_eq!(f.editor.view_start, 3.0);
        let cursor = crate::keyboard::navigation::position(&f.editor, &f.app.chart, 3.0, f.editor.keyboard.cursor);
        assert!(f.editor.keyboard.rect.contains(cursor));
        f.frame(Vec::new());
        assert_eq!(f.editor.view_start, 3.0, "auto-scroll must not offset the jump target next frame");
        f.key(egui::Key::J, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text("999s".into())]);
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        let end = f.app.song_duration();
        assert_eq!(f.app.timeline_view_time, end);
        assert_eq!(f.editor.view_start, end);
        assert_eq!(crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms), end);
    }
}

#[test]
fn jump_remains_aligned_after_measure_roundtrip_on_tempo_changes() {
    use crate::app::types::BpmChange;
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 100.0;
        f.app.chart.bpms = vec![
            BpmChange { measure: 1.0, bpm: 141.0 },
            BpmChange { measure: 3.0, bpm: 174.0 },
            BpmChange { measure: 6.0, bpm: 215.0 },
        ];
        for target in [0.1, 0.3, 1.1, 3.1, 5.3561645, 7.123, 20.7] {
            f.key(egui::Key::J, egui::Modifiers::NONE);
            f.frame(vec![egui::Event::Text(format!("{target}s"))]);
            f.key(egui::Key::Enter, egui::Modifiers::NONE);
            let actual = f.app.timeline_view_time;
            assert!((actual - target).abs() < 1e-5);
            assert_eq!(f.editor.view_start, actual);
            f.frame(Vec::new());
            assert_eq!(f.editor.view_start, actual, "{layout:?}, target {target}");
        }
    }
}

#[test]
fn tab_visits_all_notes_in_time_order_and_wraps_without_selecting() {
    let mut f = Fixture::new();
    let mut note = f.app.chart.notes[0].clone();
    note.id = 101; note.time = 3.0; note.lane = 2;
    f.app.chart.notes.push(note.clone());
    note.id = 102; note.time = 2.0; note.lane = 5;
    f.app.chart.notes.push(note.clone());
    note.id = 103; note.lane = 3;
    f.app.chart.notes.push(note);
    f.editor.keyboard.cursor.measure = 1.5;
    for id in [103, 102, 101, f.app.chart.notes[0].id, 103] {
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        let note = f.app.chart.notes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(f.editor.keyboard.cursor.measure, note.time);
        assert_eq!(f.editor.keyboard.cursor.zone, note.lane);
        assert!(f.editor.selected.is_empty());
    }
    f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
    assert_eq!(f.editor.keyboard.candidate, Some(f.app.chart.notes[0].id));
    f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
    assert_eq!(f.editor.keyboard.candidate, Some(101));
    assert_eq!(f.app.timeline_view_time, crate::app::types::measure_to_secs(3.0, &f.app.chart.bpms));
}

#[test]
fn held_tab_continues_navigation_in_both_directions_and_keeps_playback_aligned() {
    let mut f = Fixture::new();
    let mut note = f.app.chart.notes[0].clone();
    for (id, time) in [(101, 2.0), (102, 3.0), (103, 4.0)] {
        note.id = id; note.time = time;
        f.app.chart.notes.push(note.clone());
    }
    let event = |repeat, modifiers| egui::Event::Key {
        key: egui::Key::Tab, physical_key: None, pressed: true, repeat, modifiers,
    };
    for (repeat, id) in [(false, 101), (true, 102), (true, 103), (true, f.app.chart.notes[0].id)] {
        f.frame(vec![event(repeat, egui::Modifiers::NONE)]);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        let time = crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert_eq!(f.app.timeline_view_time, time);
        let cursor = crate::keyboard::navigation::position(&f.editor, &f.app.chart, time, f.editor.keyboard.cursor);
        assert!(f.editor.keyboard.rect.expand(0.5).contains(cursor));
    }
    for id in [103, 102] {
        f.frame(vec![event(true, egui::Modifiers::SHIFT)]);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
    }
    assert!(f.editor.selected.is_empty());
    assert!(f.editor.undo.is_empty());
}

#[test]
fn command_arrows_navigate_with_shift_acceleration_in_both_layouts() {
    use crate::state::EditorLayout;
    for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 20.0;
        let (later, earlier, next_lane, prev_lane) = match layout {
            EditorLayout::Vertical => (egui::Key::ArrowUp, egui::Key::ArrowDown, egui::Key::ArrowRight, egui::Key::ArrowLeft),
            EditorLayout::Horizontal => (egui::Key::ArrowRight, egui::Key::ArrowLeft, egui::Key::ArrowDown, egui::Key::ArrowUp),
        };
        let fast = egui::Modifiers { shift: true, ..egui::Modifiers::COMMAND };
        let start = f.editor.keyboard.cursor.measure;
        f.key(later, egui::Modifiers::COMMAND);
        assert_eq!(f.editor.keyboard.cursor.measure, start + 1.0 / f.editor.snap_division as f32);
        let normal = f.editor.keyboard.cursor.measure;
        f.key(later, fast);
        assert_eq!(f.editor.keyboard.cursor.measure, normal + 1.0);
        f.key(earlier, fast);
        assert_eq!(f.editor.keyboard.cursor.measure, normal);
        f.key(earlier, egui::Modifiers::COMMAND);
        assert_eq!(f.editor.keyboard.cursor.measure, start);
        f.key(next_lane, egui::Modifiers::COMMAND);
        assert_eq!(f.editor.keyboard.cursor.zone, 2);
        f.key(next_lane, fast);
        assert_eq!(f.editor.keyboard.cursor.zone, 6);
        f.key(prev_lane, fast);
        assert_eq!(f.editor.keyboard.cursor.zone, 2);
        f.key(prev_lane, egui::Modifiers::COMMAND);
        assert_eq!(f.editor.keyboard.cursor.zone, 1);
        assert!(f.editor.undo.is_empty());
        assert!(!f.editor.dirty);
    }
}

#[test]
fn held_diagonal_arrows_repeat_both_axes_and_release_independently() {
    use crate::state::EditorLayout;
    for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 20.0;
        let (time_key, lane_key) = match layout {
            EditorLayout::Vertical => (egui::Key::ArrowUp, egui::Key::ArrowRight),
            EditorLayout::Horizontal => (egui::Key::ArrowRight, egui::Key::ArrowDown),
        };
        let event = |key, pressed, repeat| egui::Event::Key {
            key, physical_key: None, pressed, repeat, modifiers: egui::Modifiers::NONE,
        };
        f.frame(vec![event(time_key, true, false)]);
        f.frame(vec![event(lane_key, true, false)]);
        let start = f.editor.keyboard.cursor;
        for step in 1..=3 {
            f.frame(vec![event(lane_key, true, true)]);
            assert_eq!(f.editor.keyboard.cursor.measure, start.measure + step as f32 / f.editor.snap_division as f32);
            assert_eq!(f.editor.keyboard.cursor.zone, start.zone + step);
        }
        f.frame(vec![event(lane_key, false, false), event(time_key, true, true)]);
        assert_eq!(f.editor.keyboard.cursor.measure, start.measure + 4.0 / f.editor.snap_division as f32);
        assert_eq!(f.editor.keyboard.cursor.zone, start.zone + 3);
        assert_eq!(f.app.timeline_view_time, crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms));
    }
}

#[test]
fn cursor_properties_and_deletion_target_pointed_note_without_selecting_it() {
    for key in [egui::Key::Delete, egui::Key::Backspace, egui::Key::D] {
        let mut f = Fixture::new();
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        note.id = 101; note.time = 2.0; note.lane = 3;
        f.app.chart.notes.push(note);
        f.editor.select_only(original);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        let output = f.frame(Vec::new());
        text_position(&output, "Cursor · id 101");
        assert_eq!(f.editor.single_selected(), Some(original));
        f.key(egui::Key::F5, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.dialog.as_ref().unwrap().note.as_ref().unwrap().id, 101);
        f.editor.keyboard.dialog.as_mut().unwrap().note.as_mut().unwrap().is_break = true;
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(!f.app.chart.notes[0].is_break);
        assert!(f.app.chart.notes[1].is_break);
        assert_eq!(f.editor.single_selected(), Some(original));
        f.key(key, egui::Modifiers::NONE);
        assert_eq!(f.app.chart.notes.len(), 1);
        assert_eq!(f.app.chart.notes[0].id, original);
        assert_eq!(f.editor.single_selected(), Some(original));
    }
}

#[test]
fn cursor_on_empty_space_does_not_delete_an_unrelated_selection() {
    let mut f = Fixture::new();
    f.editor.select_only(f.app.chart.notes[0].id);
    f.key(egui::Key::Num4, egui::Modifiers::NONE);
    f.key(egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(f.app.chart.notes.len(), 1);
    assert!(f.editor.undo.is_empty());
    f.key(egui::Key::F5, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.dialog.as_ref().unwrap().note.is_none());
}

#[test]
fn arrows_point_to_properties_and_handles_without_changing_selection() {
    let mut f = Fixture::new();
    let original = f.app.chart.notes[0].id;
    let mut note = f.app.chart.notes[0].clone();
    note.id = 101; note.lane = 2;
    note.note_type = crate::app::types::NoteType::Hold; note.hold_duration = 0.5;
    f.app.chart.notes.push(note);
    f.editor.select_only(original);
    f.key(egui::Key::ArrowRight, egui::Modifiers::NONE);
    let output = f.frame(Vec::new());
    text_position(&output, "Cursor · id 101");
    assert_eq!(f.editor.single_selected(), Some(original));
    f.key(egui::Key::F7, egui::Modifiers::NONE);
    assert_eq!(f.editor.keyboard.dialog.as_ref().unwrap().note.as_ref().unwrap().id, 101);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(matches!(f.editor.keyboard.transaction.as_ref().unwrap().kind, crate::keyboard::edit::Draft::Handle { id: 101, .. }));
    assert_eq!(f.editor.single_selected(), Some(original));
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(f.app.chart.notes[1].hold_duration, 0.5);
}

#[test]
fn diagonal_repeat_cancels_opposites_and_counts_each_axis_once_per_frame() {
    let mut f = Fixture::new();
    f.app.chart.notes[0].time = 20.0;
    let mods = egui::Modifiers { shift: true, ..egui::Modifiers::COMMAND };
    let event = |key, pressed, repeat| egui::Event::Key {
        key, physical_key: None, pressed, repeat, modifiers: mods,
    };
    f.frame(vec![event(egui::Key::ArrowUp, true, false), event(egui::Key::ArrowRight, true, false)]);
    let start = f.editor.keyboard.cursor;
    f.frame(vec![event(egui::Key::ArrowUp, true, true), event(egui::Key::ArrowRight, true, true)]);
    assert_eq!(f.editor.keyboard.cursor.measure, start.measure + 1.0);
    assert_eq!(f.editor.keyboard.cursor.zone, crate::timeline::geometry::rotated_zone(start.zone, 4));
    f.frame(vec![event(egui::Key::ArrowLeft, true, false)]);
    let start = f.editor.keyboard.cursor;
    f.frame(vec![event(egui::Key::ArrowLeft, true, true)]);
    assert_eq!(f.editor.keyboard.cursor.measure, start.measure + 1.0);
    assert_eq!(f.editor.keyboard.cursor.zone, start.zone);
}

#[test]
fn navigation_preserves_playback_and_can_edit_past_the_song_end() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        let key = match layout {
            crate::state::EditorLayout::Vertical => egui::Key::ArrowUp,
            crate::state::EditorLayout::Horizontal => egui::Key::ArrowRight,
        };
        f.key(key, egui::Modifiers::SHIFT);
        assert_eq!(f.app.mode, Mode::Playing);
        assert_eq!(f.editor.keyboard.cursor.measure, 2.0);
        let time = crate::app::types::measure_to_secs(2.0, &f.app.chart.bpms);
        assert_eq!(f.editor.view_start, 0.0, "editing beyond the song end still inside the viewport must not scroll");
        assert!(time > f.app.song_duration());
        assert_eq!(f.app.timeline_view_time, f.app.song_duration());
        f.editor.tool = Tool::Tap;
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(f.app.chart.notes.last().unwrap().time, 2.0);
    }
}

#[test]
fn tab_keeps_overlapping_note_identity_across_tempo_changes() {
    let mut f = Fixture::new();
    f.app.chart.bpms = vec![crate::app::types::BpmChange { measure: 1.0, bpm: 141.0 }];
    let mut note = f.app.chart.notes[0].clone();
    note.time = 3.137; note.lane = 4;
    for id in [101, 102, 103] { note.id = id; f.app.chart.notes.push(note.clone()); }
    for id in [101, 102, 103] {
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        assert_eq!(f.editor.keyboard.cursor.measure, 3.137);
        assert!(f.editor.selected.is_empty());
    }
}

#[test]
fn tab_handles_single_empty_and_offscreen_charts_in_both_layouts() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 40.0;
        let id = f.app.chart.notes[0].id;
        f.editor.keyboard.cursor.measure = 45.0;
        f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        let cursor = crate::keyboard::navigation::position(&f.editor, &f.app.chart, 0.0, f.editor.keyboard.cursor);
        assert!(f.editor.keyboard.rect.expand(0.5).contains(cursor), "{layout:?}: {cursor:?}");
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        f.app.chart.notes.clear();
        f.editor.keyboard.candidate = None;
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
        assert_eq!(f.editor.keyboard.candidate, None);
    }
}

#[test]
fn tab_keeps_property_dialog_and_text_field_focus() {
    let mut f = Fixture::new();
    f.editor.select_only(f.app.chart.notes[0].id);
    f.key(egui::Key::F5, egui::Modifiers::NONE);
    f.frame(Vec::new());
    assert!(f.ctx.wants_keyboard_input());
    let cursor = f.editor.keyboard.cursor;
    f.key(egui::Key::Tab, egui::Modifiers::NONE);
    assert_eq!(f.editor.keyboard.cursor, cursor);
    assert!(f.editor.keyboard.dialog.is_some());
    assert!(f.ctx.wants_keyboard_input());
    f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
    assert_eq!(f.editor.keyboard.cursor, cursor);
    assert_eq!(f.editor.keyboard.candidate, None);
    assert!(f.ctx.wants_keyboard_input());
}

#[test]
fn help_and_settings_close_without_leaking_confirmation_to_placement() {
    let mut f = Fixture::new();
    f.editor.tool = Tool::Tap;
    f.key(egui::Key::F1, egui::Modifiers::NONE);
    assert!(f.app.show_params);
    assert!(!f.editor.show_help);
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!f.app.show_params);
    f.key(egui::Key::F12, egui::Modifiers::NONE);
    assert!(f.editor.show_help);
    f.frame(vec![
        egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE },
        egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE },
    ]);
    assert!(!f.editor.show_help);
    assert_eq!(f.app.chart.notes.len(), 1);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn ime_composition_owns_tool_and_transport_keys() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    f.key(egui::Key::H, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Idle);
    assert_eq!(f.editor.tool, Tool::Select);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Disabled)]);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[cfg(feature = "backend-lean")]
mod autoplay {
    use super::*;
    use crate::state::Workspace;

    fn fixture() -> Fixture {
        let mut f = Fixture::new();
        let chart = crate::app::maidata::from_maidata_level("&inote_6=(120){4},1,,E", 6).unwrap();
        f.app = crate::runtime::initialize_player(PadPreviewState::new_at(chart, None, None, 0.0, None), None, None);
        f.app.forced_time = Some(0.0);
        assert!(f.app.has_engine(), "{}", f.app.status);
        f.editor.workspace = Workspace::Autoplay;
        f.frame(Vec::new()); f.frame(Vec::new());
        f
    }

    fn marker(output: &egui::FullOutput) -> egui::Pos2 {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.radius == 5.0 => Some(circle.center),
            _ => None,
        }).expect("event marker must be visible")
    }

    #[test]
    fn event_click_drag_release_outside_and_undo_are_one_transaction() {
        let mut f = fixture();
        f.editor.snap_enabled = false;
        let output = f.frame(Vec::new());
        let position = marker(&output);
        f.click(position);
        let selected = f.editor.autoplay_editor.selected.expect("marker click must not be swallowed by its row");
        assert_eq!(f.editor.autoplay_editor.selected_row, Some(selected.0));
        let original = f.editor.autoplay_editor.tracks[selected.0].events[selected.1].clone();
        let moved = position + egui::vec2(f.editor.pixels_per_second * 0.25, -500.0);
        f.frame(vec![egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        f.frame(vec![egui::Event::PointerMoved(moved)]);
        assert!(f.editor.autoplay_editor.dragging());
        assert!(f.editor.undo.is_empty(), "a pending drag is not committed");
        f.frame(vec![egui::Event::PointerButton { pos: moved, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        assert!(!f.editor.autoplay_editor.dragging());
        assert_eq!(f.editor.undo.len(), 1);
        let event = &f.editor.autoplay_editor.tracks[selected.0].events[selected.1];
        assert_eq!(crate::player::engine::timed_input_tp(event), crate::player::engine::timed_input_tp(&original) + 250_000);
        assert_eq!(f.app.chart.autoplay_overrides.len(), 1);
        f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert!(f.app.chart.autoplay_overrides.is_empty());
        assert_eq!(f.editor.autoplay_editor.tracks[selected.0].events[selected.1], original);
    }

    #[test]
    fn escape_cancels_marker_drag_without_recording_an_edit() {
        let mut f = fixture();
        let output = f.frame(Vec::new());
        let position = marker(&output);
        let original = f.editor.autoplay_editor.tracks[0].events.clone();
        f.frame(vec![egui::Event::PointerMoved(position)]);
        f.frame(vec![egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        f.frame(vec![egui::Event::PointerMoved(position + egui::vec2(75.0, 0.0))]);
        assert!(f.editor.autoplay_editor.dragging());
        f.key(egui::Key::Escape, egui::Modifiers::NONE);
        assert!(!f.editor.autoplay_editor.dragging());
        assert!(f.editor.undo.is_empty());
        assert!(f.app.chart.autoplay_overrides.is_empty());
        assert_eq!(f.editor.autoplay_editor.tracks[0].events, original);
    }

    #[test]
    fn autoplay_uses_configured_transport_and_respects_ime_and_modal_input() {
        let mut f = fixture();
        let binding = f.editor.keyboard.bindings.iter_mut().find(|b| b.command == "play").unwrap();
        binding.key = egui::Key::F9;
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Idle);
        f.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
        f.key(egui::Key::F9, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Idle);
        f.frame(vec![egui::Event::Ime(egui::ImeEvent::Disabled)]);
        f.key(egui::Key::F9, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Playing);
        f.key(egui::Key::F12, egui::Modifiers::NONE);
        assert!(f.editor.show_help);
        let help = f.frame(Vec::new());
        text_position(&help, "Select / move event in its track");
        f.key(egui::Key::F9, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Playing, "modal help owns transport keys");
        f.key(egui::Key::Escape, egui::Modifiers::NONE);
        assert!(!f.editor.show_help);
        f.key(egui::Key::F9, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Idle);
        f.key(egui::Key::S, egui::Modifiers::COMMAND);
        assert!(f.editor.save_as_request);
    }

    #[test]
    fn workspace_switch_leaves_isolation_and_preserves_parent_seek() {
        let mut f = Fixture::new();
        f.editor.select_only(f.app.chart.notes[0].id);
        crate::templates::apply(&mut f.editor, &mut f.app, crate::templates::Action::CreateSelection);
        crate::templates::apply(&mut f.editor, &mut f.app, crate::templates::Action::EnterInstance("inst_1".into()));
        assert!(f.editor.template_session.is_some());
        let output = f.frame(Vec::new());
        f.click(text_position(&output, "Autoplay"));
        assert_eq!(f.editor.workspace, Workspace::Autoplay);
        assert!(f.editor.template_session.is_none());
        assert_eq!(f.editor.pending_seek, Some(0.0));
        assert_eq!(f.app.chart.template_instances.len(), 1);
        assert_eq!(f.editor.autoplay_mode, 2);
        assert!(f.editor.mode_dirty);
    }
}

#[test]
fn keyboard_hold_commits_one_history_entry_and_can_be_cancelled() {
    let mut f = Fixture::new();
    f.key(egui::Key::H, egui::Modifiers::NONE);
    f.key(egui::Key::Num3, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.transaction.is_some());
    assert_eq!(f.app.chart.notes.len(), 2);
    assert!(f.editor.undo.is_empty());
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Idle, "placement must own Space");
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.transaction.is_none());
    assert_eq!(f.app.chart.notes.len(), 2);
    assert_eq!(f.editor.undo.len(), 1);
    let hold = f.app.chart.notes.last().unwrap();
    assert_eq!(hold.lane, 3);
    assert!((hold.hold_duration - 1.0 / f.editor.snap_division as f32).abs() < 1e-6);
    let before = f.app.chart.notes.clone();
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(f.app.chart.notes, before);
    assert_eq!(f.editor.undo.len(), 1);
}

#[test]
fn keyboard_star_confirms_head_delay_and_trajectory() {
    let mut f = Fixture::new();
    f.key(egui::Key::S, egui::Modifiers::NONE);
    f.key(egui::Key::Num1, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.app.chart.notes.last().unwrap().slide[0].segments.is_empty());
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert!(f.editor.undo.is_empty(), "disabled inspector must not commit a Star draft");
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    f.key(egui::Key::Num5, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.transaction.is_none());
    assert_eq!(f.editor.undo.len(), 1);
    assert_eq!(f.app.chart.notes.len(), 2);
    let slide = &f.app.chart.notes.last().unwrap().slide[0];
    assert_eq!(slide.segments.len(), 1);
    assert_eq!(slide.segments[0].points.last().unwrap().zone.to_id(), 5);
    assert!(slide.slide_duration > slide.slide_start_delay);
}

#[test]
fn duplicate_confirmation_click_does_not_place_an_extra_tap() {
    let mut f = Fixture::new();
    let id = f.app.chart.notes[0].id;
    f.editor.select_only(id);
    f.editor.tool = Tool::Tap;
    let pointer = f.ui.layout.timeline.center();
    f.frame(vec![egui::Event::PointerMoved(pointer)]);
    f.key(egui::Key::D, egui::Modifiers::SHIFT);
    assert_eq!(f.app.chart.notes.len(), 2);
    assert!(f.editor.selection_transform.is_some());
    f.click(pointer);
    assert!(f.editor.selection_transform.is_none());
    assert_eq!(f.app.chart.notes.len(), 2, "confirmation must not fall through");
    assert_eq!(f.editor.undo.len(), 1);
}

#[test]
fn keyboard_navigation_seeks_playback_without_scrolling_a_visible_cursor() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 6.0;
        f.app.seek_to(2.0);
        f.editor.keyboard.active = true;
        f.editor.keyboard.detached = true;
        f.editor.keyboard.cursor.measure = 2.0;
        f.editor.view_start = 1.0;
        f.frame(Vec::new());
        let view = f.editor.view_start;
        let key = match layout {
            crate::state::EditorLayout::Vertical => egui::Key::ArrowUp,
            crate::state::EditorLayout::Horizontal => egui::Key::ArrowRight,
        };
        f.key(key, egui::Modifiers::NONE);
        assert_eq!(f.editor.view_start, view, "visible navigation must not scroll the viewport");
        crate::runtime::follow_playhead(&mut f.editor, 20.0);
        assert_eq!(f.editor.view_start, view);
        let time = crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert_eq!(f.app.timeline_view_time, time);
        let cursor = crate::keyboard::navigation::position(&f.editor, &f.app.chart, time, f.editor.keyboard.cursor);
        assert!(f.editor.keyboard.rect.contains(cursor), "{layout:?}: {cursor:?}");
        let lane_key = if layout == crate::state::EditorLayout::Vertical { egui::Key::ArrowRight } else { egui::Key::ArrowDown };
        f.key(lane_key, egui::Modifiers::NONE);
        assert_eq!(f.editor.view_start, view, "lane movement must not scroll time");
        f.frame(Vec::new());
        assert_eq!(f.editor.view_start, view);
    }
}

#[test]
fn keyboard_range_selection_confirms_without_placing_and_preserves_the_tool() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.tool = Tool::Tap;
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        note.id = 101; note.time = 2.0; note.lane = 2;
        f.app.chart.notes.push(note.clone());
        note.id = 102; note.time = 3.0; note.lane = 4;
        f.app.chart.notes.push(note);
        f.editor.select_only(102);
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        assert!(f.editor.keyboard.range.is_some());
        assert!(f.editor.keyboard.owns_input());
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        assert_eq!(f.app.mode, Mode::Idle, "a range owns input until confirmed or cancelled");
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(101));
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(f.editor.keyboard.range.is_none());
        assert_eq!(f.editor.selected, [original, 101].into_iter().collect());
        assert_eq!(f.editor.tool, Tool::Tap);
        assert_eq!(f.app.chart.notes.len(), 3, "range confirmation must not place a Tap");
        assert!(f.editor.undo.is_empty());
        assert!(!f.editor.dirty);
    }
}

#[test]
fn keyboard_navigation_scrolls_only_after_crossing_either_visible_time_edge() {
    use crate::app::types::{measure_to_secs, secs_to_measure};
    use crate::state::EditorLayout;
    for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.snap_enabled = false;
        f.app.chart.notes[0].time = 40.0;
        f.editor.keyboard.active = true;
        f.editor.keyboard.detached = true;
        f.editor.view_start = 6.0;
        let span = crate::keyboard::navigation::page_seconds(&f.editor);
        let (later, earlier) = if layout == EditorLayout::Vertical { (egui::Key::ArrowUp, egui::Key::ArrowDown) }
            else { (egui::Key::ArrowRight, egui::Key::ArrowLeft) };
        f.editor.keyboard.cursor.measure = secs_to_measure(6.0 + span - 0.1875, &f.app.chart.bpms);
        f.key(later, egui::Modifiers::NONE);
        assert_eq!(f.editor.view_start, 6.0, "do not scroll early near the far edge");
        f.key(later, egui::Modifiers::NONE);
        let time = measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert!((f.editor.view_start - (time - span)).abs() < 1e-5);
        let position = crate::keyboard::navigation::position(&f.editor, &f.app.chart, time, f.editor.keyboard.cursor);
        assert!(f.editor.keyboard.rect.expand(0.5).contains(position));
        let view = f.editor.view_start;
        f.frame(Vec::new());
        assert_eq!(f.editor.view_start, view, "an edge-aligned cursor must not drift next frame");
        let bottom = if layout == EditorLayout::Vertical { crate::timeline::geometry::JUDGE_MARGIN / f.editor.pixels_per_second } else { 0.0 };
        f.editor.keyboard.cursor.measure = secs_to_measure(view - bottom + 0.1875, &f.app.chart.bpms);
        f.key(earlier, egui::Modifiers::NONE);
        assert_eq!(f.editor.view_start, view, "the strip below the judgment line is still visible");
        f.key(earlier, egui::Modifiers::NONE);
        let time = measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert!((f.editor.view_start - (time + bottom)).abs() < 1e-5);
        let position = crate::keyboard::navigation::position(&f.editor, &f.app.chart, time, f.editor.keyboard.cursor);
        assert!(f.editor.keyboard.rect.expand(0.5).contains(position));
        assert_eq!(f.app.timeline_view_time, time);
    }
}

#[test]
fn keyboard_tab_keeps_the_view_still_for_an_already_visible_note() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        let mut note = f.app.chart.notes[0].clone();
        note.id = 101; note.time = 2.0; note.lane = 2;
        f.app.chart.notes.push(note);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.view_start, 0.0);
        assert_eq!(f.app.timeline_view_time, 2.0);
        f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
        assert_eq!(f.editor.view_start, 0.0);
    }
}

#[test]
fn keyboard_multiselection_toggles_candidates_including_overlapping_notes() {
    let mut f = Fixture::new();
    let original = f.app.chart.notes[0].id;
    let mut note = f.app.chart.notes[0].clone();
    note.id = 101;
    f.app.chart.notes.push(note);
    f.key(egui::Key::F2, egui::Modifiers::NONE);
    assert_eq!(f.editor.selected, [original].into_iter().collect());
    f.key(egui::Key::Tab, egui::Modifiers::NONE);
    f.key(egui::Key::F2, egui::Modifiers::SHIFT);
    assert_eq!(f.editor.selected, [original, 101].into_iter().collect());
    f.key(egui::Key::F2, egui::Modifiers::SHIFT);
    assert_eq!(f.editor.selected, [original].into_iter().collect());
    f.key(egui::Key::F2, egui::Modifiers::NONE);
    assert_eq!(f.editor.selected, [101].into_iter().collect());
    assert!(f.editor.undo.is_empty());
    assert!(!f.editor.dirty);
}

#[test]
fn keyboard_range_selection_adds_across_screens_and_cancels_without_changing_selection() {
    for layout in [crate::state::EditorLayout::Vertical, crate::state::EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        for (id, time, lane) in [(101, 20.0, 3), (102, 30.0, 8), (103, 1.5, 7)] {
            note.id = id; note.time = time; note.lane = lane;
            f.app.chart.notes.push(note.clone());
        }
        f.editor.select_only(102);
        let additive = egui::Modifiers { shift: true, ..egui::Modifiers::COMMAND };
        f.key(egui::Key::Enter, additive);
        f.click(f.ui.layout.timeline.center());
        assert!(f.editor.keyboard.range.is_some(), "a pointer click must not steal a keyboard range");
        assert!(f.editor.keyboard.active);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(101));
        assert!(f.editor.view_start > 0.0);
        assert_eq!(f.editor.selected, [102].into_iter().collect(), "preview must not commit selection");
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(f.editor.selected, [original, 101, 102].into_iter().collect());
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::Escape, egui::Modifiers::NONE);
        assert!(f.editor.keyboard.range.is_none());
        assert_eq!(f.editor.selected, [original, 101, 102].into_iter().collect());
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        f.key(egui::Key::Home, egui::Modifiers::NONE);
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        assert_eq!(f.editor.selected, [102].into_iter().collect(), "a new normal range replaces the selection");
        assert_eq!(f.app.chart.notes.len(), 4);
        assert!(f.editor.undo.is_empty());
    }
}

#[test]
fn keyboard_primary_delete_removes_the_whole_selection_even_with_a_blank_cursor() {
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        let mut f = Fixture::new();
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        for (id, time) in [(101, 2.0), (102, 3.0)] {
            note.id = id; note.time = time;
            f.app.chart.notes.push(note.clone());
        }
        f.key(egui::Key::F2, egui::Modifiers::NONE);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::F2, egui::Modifiers::SHIFT);
        assert_eq!(f.editor.selected, [original, 101].into_iter().collect());
        f.key(egui::Key::Num8, egui::Modifiers::NONE);
        f.key(key, egui::Modifiers::COMMAND);
        assert_eq!(f.app.chart.notes.iter().map(|n| n.id).collect::<Vec<_>>(), vec![102]);
        assert_eq!(f.editor.undo.len(), 1);
        f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(f.app.chart.notes.len(), 3);
    }
}

#[test]
fn properties_keep_their_draft_between_frames() {
    let mut f = Fixture::new();
    f.editor.select_only(f.app.chart.notes[0].id);
    f.key(egui::Key::F5, egui::Modifiers::NONE);
    f.editor.keyboard.dialog.as_mut().unwrap().note.as_mut().unwrap().is_break = true;
    f.frame(Vec::new());
    assert!(!f.app.chart.notes[0].is_break);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.dialog.is_none());
    assert!(f.app.chart.notes[0].is_break);
    assert_eq!(f.editor.undo.len(), 1);
}

#[test]
fn property_typing_commits_and_releases_keyboard_focus() {
    let mut f = Fixture::new();
    f.app.chart.notes[0].time = 2.5;
    f.editor.select_only(f.app.chart.notes[0].id);
    f.key(egui::Key::F5, egui::Modifiers::NONE);
    let output = f.frame(Vec::new());
    let value = text_position(&output, "2.5");
    f.click(value);
    f.frame(Vec::new());
    assert!(f.ctx.wants_keyboard_input(), "property value must accept typing");
    f.key(egui::Key::A, egui::Modifiers::COMMAND);
    f.frame(vec![egui::Event::Text("3.5".into())]);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.dialog.is_none());
    assert_eq!(f.app.chart.notes[0].time, 3.5);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn cancelling_property_typing_releases_space_without_committing() {
    let mut f = Fixture::new();
    f.app.chart.notes[0].time = 2.5;
    f.editor.select_only(f.app.chart.notes[0].id);
    f.key(egui::Key::F5, egui::Modifiers::NONE);
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "2.5"));
    f.frame(Vec::new());
    assert!(f.ctx.wants_keyboard_input());
    f.key(egui::Key::A, egui::Modifiers::COMMAND);
    f.frame(vec![egui::Event::Text("3.5".into())]);
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.dialog.is_none());
    assert_eq!(f.app.chart.notes[0].time, 2.5);
    assert!(f.editor.undo.is_empty());
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn properties_can_be_typed_without_using_the_pointer() {
    let mut f = Fixture::new();
    f.editor.select_only(f.app.chart.notes[0].id);
    f.key(egui::Key::F5, egui::Modifiers::NONE);
    f.frame(Vec::new());
    assert!(f.ctx.wants_keyboard_input(), "F5 must focus the first property field");
    f.key(egui::Key::A, egui::Modifiers::COMMAND);
    f.frame(vec![egui::Event::Text("2.5".into())]);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(f.app.chart.notes[0].time, 2.5);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn tools_dialog_updates_touch_cursor_when_switching_tools() {
    let mut f = Fixture::new();
    f.key(egui::Key::F4, egui::Modifiers::NONE);
    for _ in 0..3 {
        f.key(egui::Key::ArrowDown, egui::Modifiers::NONE);
    }
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(f.editor.tool, Tool::Touch);
    assert!(f.editor.keyboard.cursor.touch);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.app.chart.notes.last().unwrap().is_touch);
    f.key(egui::Key::F4, egui::Modifiers::NONE);
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(f.editor.tool, Tool::Tap);
    assert!(!f.editor.keyboard.cursor.touch);
}
