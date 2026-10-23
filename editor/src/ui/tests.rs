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
            ctx: egui::Context::default(),
            ui: EditorUi::new(),
            editor: EditorState::default(),
            app,
            time: 0.0,
        };
        fixture.frame(Vec::new());
        fixture.frame(Vec::new());
        fixture
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.frame_with_modifiers(events, egui::Modifiers::NONE)
    }

    fn frame_with_modifiers(
        &mut self,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        self.time += 0.1;
        let now = if self.app.mode == Mode::Idle {
            self.app.timeline_view_time
        } else {
            self.app.song_time()
        };
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 800.0),
                )),
                time: Some(self.time),
                events,
                modifiers,
                ..Default::default()
            },
            |ctx| {
                self.ui.draw(
                    ctx,
                    &mut self.editor,
                    &mut self.app,
                    now,
                    &WaveformPyramid::default(),
                    0.0,
                )
            },
        )
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) -> egui::FullOutput {
        self.frame(vec![
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            },
        ])
    }

    fn click(&mut self, position: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(position)]);
        self.frame(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }])
    }
}

fn text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing visible label: {label}"))
}

#[test]
fn annotation_confirmation_is_visible_without_mairis_pane_and_requires_successful_save() {
    use crate::viewer::AnnotationPhase;
    let mut f = Fixture::new();
    f.editor.mairis.annotation.active = true;
    f.editor.mairis.annotation.set_original(&f.app.chart.notes);
    let id = f.app.chart.notes[0].id;
    f.editor.select_only(id);
    f.editor.nudge_selection(&mut f.app.chart, 0.125, 1);
    f.editor.mairis.annotation.phase = AnnotationPhase::Review;
    f.editor.mairis.annotation.confirm_pending = true;
    f.frame(Vec::new());
    let output = f.frame(Vec::new());
    text_position(&output, "确认标注结果");
    text_position(&output, "删除 0 · 新增 0 · 移动 1");
    f.click(text_position(&output, "返回修改"));
    assert!(f.editor.mairis.annotation.is_editing());
    assert!(!f.editor.mairis.annotation.confirm_pending);

    let dir = std::env::temp_dir().join(format!("mairis-confirm-ui-{}-{}",
        std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    f.editor.chart_dir = dir.join("absent").display().to_string();
    f.editor.mairis.annotation.phase = AnnotationPhase::Review;
    f.editor.mairis.annotation.confirm_pending = true;
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "确认并保存"));
    assert_eq!(f.editor.mairis.annotation.phase, AnnotationPhase::Review);
    assert!(f.editor.mairis.annotation.confirm_pending);
    assert!(f.editor.mairis.annotation.status.as_deref().unwrap().contains("保存失败"));
    f.editor.chart_dir = dir.display().to_string();
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "确认并保存"));
    assert_eq!(f.editor.mairis.annotation.phase, AnnotationPhase::Confirmed);
    assert!(!f.editor.mairis.annotation.confirm_pending);
    assert!(dir.join("feedback.jsonl").is_file());
    assert!(dir.join("annotated.chart.json").is_file());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn annotation_ghost_click_drag_restore_and_undo_work_in_both_layouts() {
    use crate::state::EditorLayout;
    use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER, RULER_H, LANE_COUNT};
    for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.ui.workbench.reset_layout();
        f.frame(Vec::new());
        f.editor.layout = layout;
        f.editor.hide_chart_tools = true;
        f.editor.follow_playhead = false;
        f.editor.view_start = 0.0;
        f.editor.pixels_per_second = 120.0;
        f.app.chart.notes[0].time = 1.5;
        f.app.chart.notes[0].lane = 4;
        let original = f.app.chart.notes[0].clone();
        f.editor.mairis.annotation.active = true;
        f.editor.mairis.annotation.set_original(&f.app.chart.notes);
        f.editor.delete_note(&mut f.app.chart, original.id);
        f.editor.tool = Tool::Tap;
        f.frame(Vec::new());
        let rect = f.ui.layout.timeline;
        let at = match layout {
            EditorLayout::Vertical => egui::pos2(rect.left() + LANE_GUTTER
                + 3.5 * (rect.width() - LANE_GUTTER) / LANE_COUNT as f32,
                rect.bottom() - JUDGE_MARGIN - 120.0),
            EditorLayout::Horizontal => egui::pos2(rect.left() + LANE_GUTTER + 120.0,
                rect.top() + RULER_H + 3.5 * (rect.height() - RULER_H) / LANE_COUNT as f32),
        };
        f.click(at);
        assert_eq!(f.editor.mairis.annotation.selected_ghost, Some(original.id), "{layout:?}");
        assert!(f.editor.selected.is_empty());
        f.frame(vec![egui::Event::PointerButton {
            pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
        }]);
        f.frame(vec![egui::Event::PointerMoved(at + egui::vec2(40.0, 40.0))]);
        assert!(f.app.chart.notes.is_empty(), "a ghost drag must not place or move a note");
        assert!(f.editor.dragging.is_none());
        assert!(!f.editor.scrubbing);
        assert_eq!(f.editor.undo.len(), 1);
        f.frame(vec![egui::Event::PointerButton {
            pos: at + egui::vec2(40.0, 40.0), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
        }]);
        f.key(egui::Key::Delete, egui::Modifiers::NONE);
        assert!(f.app.chart.notes.iter().any(|note| note == &original), "{layout:?}: Delete restores selected ghost");
        f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert!(f.app.chart.notes.is_empty());
        f.key(egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
        assert_eq!(f.app.chart.notes.len(), 1);
    }
}

#[test]
fn pad_display_options_save_from_the_pad_menu() {
    const CHILD: &str = "LAMBDADX_TEST_PAD_SETTINGS_CACHE";
    if std::env::var_os(CHILD).is_none() {
        let dir = std::env::temp_dir().join(format!("lambdadx-pad-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::tests::pad_display_options_save_from_the_pad_menu", "--nocapture"])
            .env(CHILD, &dir)
            .env("MAI2_CACHE_DIR", &dir)
            .env("MAI2_DATA_DIR", &dir)
            .output();
        let _ = std::fs::remove_dir_all(&dir);
        let result = result.unwrap();
        assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        return;
    }
    let mut f = Fixture::new();
    let output = f.frame(Vec::new());
    let preview = f.ui.layout.preview;
    let icon = char::from(lucide_icons::Icon::SlidersHorizontal).to_string();
    let menu = output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == icon
            && text.pos.x >= preview.left() && text.pos.x < preview.right()
            && text.pos.y < preview.top() && text.pos.y > preview.top() - 40.0 => {
                Some(text.pos + text.galley.size() * 0.5)
            }
        _ => None,
    }).expect("Pad settings button must be visible");
    f.click(menu);
    for label in ["隐藏 Notes", "隐藏感应区", "显示判定区间"] {
        let output = f.frame(Vec::new());
        f.click(text_position(&output, label));
    }
    assert!(f.app.params.hide_notes);
    assert!(f.app.params.hide_zones);
    assert!(f.app.params.show_judge_windows);
    let saved = crate::app::params::load();
    assert!(saved.hide_notes, "Pad display toggles must be saved without opening another window");
    assert!(saved.hide_zones);
    assert!(saved.show_judge_windows);
}

#[test]
fn chart_beat_ruler_seeks_without_placing_or_sticking() {
    for orientation in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.ui.workbench.reset_layout();
        f.frame(Vec::new());
        f.editor.layout = orientation;
        f.editor.tool = Tool::Tap;
        f.editor.pixels_per_second = 120.0;
        f.editor.follow_playhead = false;
        f.editor.view_start = 0.0;
        f.frame(Vec::new());
        let rect = f.ui.layout.timeline;
        let at = if orientation == crate::state::EditorLayout::Vertical {
            egui::pos2(rect.left() + 4.0, rect.bottom() - crate::timeline::geometry::JUDGE_MARGIN - 30.0)
        } else {
            egui::pos2(rect.left() + crate::timeline::geometry::LANE_GUTTER + 30.0, rect.top() + 8.0)
        };
        let count = f.app.chart.notes.len();
        f.click(at);
        assert!(f.editor.pending_seek.is_some(), "beat ruler must seek in {orientation:?}");
        assert!((f.editor.pending_seek.unwrap() - 0.25).abs() < 0.001);
        assert!(!f.editor.scrubbing);
        f.frame(vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        let outside = if orientation == crate::state::EditorLayout::Vertical {
            egui::pos2(at.x, rect.top() - 50.0)
        } else {
            egui::pos2(rect.right() + 50.0, at.y)
        };
        f.frame(vec![egui::Event::PointerMoved(outside)]);
        assert!(f.editor.scrubbing);
        assert!(f.editor.pending_seek.unwrap() > 0.25);
        let view = f.editor.view_start;
        f.frame(Vec::new());
        assert!(f.editor.view_start > view, "edge seeking must continue while held still");
        f.frame(vec![egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        f.frame(Vec::new());
        assert!(!f.editor.scrubbing);
        assert_eq!(f.app.chart.notes.len(), count);
        assert!(f.editor.undo.is_empty(), "ruler gestures must not edit the chart");
    }
}

#[test]
fn hidden_chart_tools_reclaim_canvas_area_in_both_layouts() {
    use crate::state::EditorLayout;
    for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.follow_playhead = false;
        f.editor.view_start = 0.0;
        f.editor.pixels_per_second = 120.0;
        f.frame(Vec::new());
        let canvas = f.ui.layout.timeline;
        let palette = f.ui.layout.tools;
        f.editor.hide_chart_tools = true;
        f.editor.tool = Tool::Tap;
        f.frame(Vec::new());
        assert_eq!(f.ui.layout.tools, egui::Rect::NOTHING);
        assert_eq!(f.ui.layout.timeline, canvas);
        let before = f.app.chart.notes.len();
        f.click(egui::pos2(palette.right() - 2.0, palette.bottom() - 12.0));
        assert_eq!(f.app.chart.notes.len(), before + 1, "hidden palette must not block note placement: {layout:?}");
        assert_eq!(f.editor.tool, Tool::Tap);
    }
}

#[test]
fn chart_beat_ruler_loop_gesture_owns_input_until_release() {
    use crate::state::EditorLayout;
    use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER};
    for layout in [EditorLayout::Vertical, EditorLayout::Horizontal] {
        let mut f = Fixture::new();
        f.ui.workbench.reset_layout();
        f.frame(Vec::new());
        f.editor.layout = layout;
        f.editor.tool = Tool::Tap;
        f.editor.follow_playhead = false;
        f.editor.view_start = 0.0;
        f.editor.pixels_per_second = 120.0;
        f.frame(Vec::new());
        let rect = f.ui.layout.timeline;
        let position = |time: f32| match layout {
            EditorLayout::Vertical => egui::pos2(rect.left() + 4.0, rect.bottom() - JUDGE_MARGIN - time * 120.0),
            EditorLayout::Horizontal => egui::pos2(rect.left() + LANE_GUTTER + time * 120.0, rect.top() + 8.0),
        };
        let button = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos, button: egui::PointerButton::Primary, pressed, modifiers,
        };
        let start = position(0.25);
        let end = position(0.75);
        f.frame(vec![egui::Event::PointerMoved(start)]);
        f.frame_with_modifiers(vec![button(start, true, egui::Modifiers::SHIFT)], egui::Modifiers::SHIFT);
        assert!(f.editor.loop_drag.is_some());
        f.frame_with_modifiers(vec![egui::Event::PointerMoved(end)], egui::Modifiers::SHIFT);
        f.frame(vec![button(end, false, egui::Modifiers::NONE)]);
        assert_eq!(f.editor.loop_range, Some((0.25, 0.75)));
        assert!(f.editor.loop_enabled);
        assert!(f.editor.loop_drag.is_none());
        assert!(!f.editor.scrubbing);
        assert_eq!(f.editor.pending_seek, None);
        assert_eq!(f.app.chart.notes.len(), 1);
        assert!(f.editor.undo.is_empty());
    }
}

#[test]
fn sensor_edit_touch_pad_and_properties_preserve_sensor_and_undo() {
    let mut f = Fixture::new();
    f.app.chart = crate::app::maidata::from_maidata("&inote_1=(120){4}B3,E", None).unwrap();
    f.app.pad_svg = Some(
        crate::app::pad_svg::PadSvgDef::from_svg_str(include_str!("../../../assets/pad.svg"))
            .unwrap(),
    );
    f.editor.tool = Tool::Touch;
    f.editor.keyboard.active = false;
    f.editor.selected.insert(f.app.chart.notes[0].id);
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "Firework"));
    assert!(f.app.chart.notes[0].is_hanabi);
    assert_eq!(
        f.app.chart.notes[0].lane, 11,
        "other property edits must not clamp B3 into A8"
    );
    let mut pad = f.ui.layout.pad;
    pad.outer_r /= crate::app::params::pad_circle_scale().max(1.0);
    let center = f
        .app
        .pad_svg
        .as_ref()
        .unwrap()
        .zone_screen_centroid(crate::app::types::zone::id_to_zone(33).unwrap(), &pad)
        .unwrap();
    f.click(egui::pos2(center.x, center.y));
    assert_eq!(f.app.chart.notes[0].lane, 33);
    assert_eq!(f.editor.touch_zone, Some(33));
    assert_eq!(f.editor.undo.len(), 2);
    f.editor.undo(&mut f.app.chart);
    assert_eq!(f.app.chart.notes[0].lane, 11);
    assert!(f.app.chart.notes[0].is_hanabi);
    f.editor.selected.clear();
    f.click(egui::pos2(pad.cx, pad.cy));
    assert_eq!(
        f.editor.touch_zone,
        Some(17),
        "without a target, Pad sets the next placement sensor"
    );
    assert_eq!(f.app.chart.notes[0].lane, 11);
    assert_eq!(f.editor.undo.len(), 1);
}

#[test]
fn timeline_regression_pane_switch_keeps_menu_and_canvas_interactive() {
    let mut f = Fixture::new();
    f.ui.workbench.reset_layout();
    for (icon, label, kind) in [
        (lucide_icons::Icon::Music2, "Autoplay", PaneKind::Autoplay),
        (lucide_icons::Icon::ListVideo, "谱面", PaneKind::Chart),
    ] {
        let output = f.frame(Vec::new());
        f.click(text_position(&output, &char::from(icon).to_string()));
        let menu = f.frame(Vec::new());
        let position = menu
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("pane type menu must remain visible");
        f.click(position);
        f.frame(Vec::new());
        f.frame(Vec::new());
        assert!(
            f.ui.workbench.has(kind),
            "pane menu did not switch to {kind:?}"
        );
        assert!(
            !f.ctx.memory(|m| m.any_popup_open()),
            "pane popup stayed open"
        );
        assert!(
            !f.ui.input_blocked,
            "pane switch left the timeline disabled"
        );
    }
}

mod timeline_navigation {
    use super::*;
    use crate::state::EditorLayout;
    use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER, RULER_H};

    fn fixture(layout: EditorLayout) -> Fixture {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.tool = Tool::Select;
        f.editor.view_start = 19.0;
        f.editor.pixels_per_second = 100.0;
        f.editor.follow_playhead = false;
        f.editor.keyboard.detached = true;
        f.app.forced_time = Some(20.0);
        f.app.timeline_view_time = 20.0;
        f.frame(Vec::new());
        f.frame(Vec::new());
        f
    }

    fn position(f: &Fixture, time: f32) -> egui::Pos2 {
        let rect = f.ui.layout.timeline;
        match f.editor.layout {
            EditorLayout::Horizontal => egui::pos2(
                rect.left()
                    + LANE_GUTTER
                    + (time - f.editor.view_start) * f.editor.pixels_per_second,
                (rect.top() + RULER_H + rect.bottom()) * 0.5,
            ),
            EditorLayout::Vertical => egui::pos2(
                (rect.left() + LANE_GUTTER + rect.right()) * 0.5,
                rect.bottom()
                    - JUDGE_MARGIN
                    - (time
                        - crate::keyboard::navigation::reference_time(
                            &f.editor,
                            f.app.timeline_time(),
                        ))
                        * f.editor.pixels_per_second,
            ),
        }
    }

    fn button(f: &mut Fixture, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) {
        f.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }

    #[test]
    fn middle_scroll_controls_speed_without_editing_and_keeps_slide_click() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            f.click(position(&f, 20.5));
            assert!(
                f.editor.pointer_press.is_none(),
                "click must release navigation guard"
            );
            f.app.chart.notes[0].note_type = crate::app::types::NoteType::Hold;
            f.app.chart.notes[0].time = 11.0;
            f.app.chart.notes[0].lane = 5;
            f.app.chart.notes[0].hold_duration = 0.5;
            f.frame(Vec::new());
            let notes = f.app.chart.notes.clone();
            let origin = position(&f, 21.0);
            let direction = match layout {
                EditorLayout::Horizontal => egui::vec2(1.0, 0.0),
                EditorLayout::Vertical => egui::vec2(0.0, -1.0),
            };
            button(&mut f, origin, egui::PointerButton::Middle, true);
            assert!(f.editor.timeline_pan.is_some());
            f.frame(vec![egui::Event::PointerMoved(origin + direction * 50.0)]);
            let before = f.editor.view_start;
            f.frame(Vec::new());
            let slow_step = f.editor.view_start - before;
            assert!(
                slow_step > 0.0,
                "stationary offset must keep scrolling: {layout:?}"
            );
            let before = f.editor.view_start;
            f.frame(vec![egui::Event::PointerMoved(origin + direction * 100.0)]);
            assert!(f.editor.view_start - before > slow_step);
            let before = f.editor.view_start;
            f.frame(vec![egui::Event::PointerMoved(origin - direction * 50.0)]);
            assert!(f.editor.view_start < before);
            let before = f.editor.view_start;
            f.frame(vec![egui::Event::PointerMoved(origin)]);
            f.frame(Vec::new());
            assert_eq!(f.editor.view_start, before);
            let outside = origin + direction * 1000.0;
            f.frame(vec![egui::Event::PointerMoved(outside)]);
            let before = f.editor.view_start;
            f.frame(Vec::new());
            assert!(f.editor.view_start > before);
            button(&mut f, outside, egui::PointerButton::Middle, false);
            let before = f.editor.view_start;
            f.frame(Vec::new());
            assert_eq!(f.editor.view_start, before);
            assert!(f.editor.timeline_pan.is_none() && f.editor.handle_grab.is_none());
            assert!(f.editor.pending_seek.is_none() && f.editor.undo.is_empty());
            assert_eq!(f.app.chart.notes, notes);

            let mut f = fixture(layout);
            let note = &mut f.app.chart.notes[0];
            note.note_type = crate::app::types::NoteType::Slide;
            note.time = 11.0;
            note.lane = 5;
            note.slide.push(crate::app::types::Slide {
                slide_duration: 1.0,
                slide_start_delay: 0.25,
                segments: Vec::new(),
                slide_is_break: false,
                runtime_parts: 1,
            });
            let id = note.id;
            f.frame(Vec::new());
            let pos = position(&f, 21.0);
            button(&mut f, pos, egui::PointerButton::Middle, true);
            button(&mut f, pos, egui::PointerButton::Middle, false);
            assert_eq!(f.editor.editing_slide, Some(id), "middle click: {layout:?}");
        }
    }

    #[test]
    fn playback_recovery_survives_pane_view_capture() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            for pps in [100.0, 4800.0] {
                let mut f = fixture(layout);
                f.editor.follow_playhead = true;
                f.editor.keyboard.detached = false;
                f.editor.pixels_per_second = pps;
                f.editor.follow_view_offset = Some(-40.0);
                f.editor.follow_zoom_detached = true;
                f.editor.view_start = 60.0;
                f.frame(Vec::new());
                crate::runtime::follow_playhead_during_playback(&mut f.editor, 20.0, false);
                assert_eq!(f.editor.view_start, 60.0);
                crate::runtime::follow_playhead_during_playback(&mut f.editor, 20.0, true);
                f.frame(Vec::new());
                assert!(f.ui.layout.timeline.contains(position(&f, 20.0)), "playhead must stay visible after UI captures the pane: {layout:?}, {pps}");
                assert!(!f.editor.follow_zoom_detached);
                assert!(f.editor.undo.is_empty());
            }
        }
    }

    #[test]
    fn zoom_keeps_pointer_time_and_following_pan_seeks() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            f.editor.follow_playhead = true;
            f.editor.keyboard.detached = false;
            crate::runtime::follow_playhead(&mut f.editor, 20.0);
            let pos = position(&f, 21.0);
            let rect = f.ui.layout.timeline;
            let offset = match layout {
                EditorLayout::Horizontal => pos.x - rect.left() - LANE_GUTTER,
                EditorLayout::Vertical => rect.bottom() - JUDGE_MARGIN - pos.y,
            };
            f.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, 30.0),
                    modifiers: egui::Modifiers {
                        command: true,
                        mac_cmd: true,
                        ..Default::default()
                    },
                },
            ]);
            assert!(f.editor.pixels_per_second > 100.0);
            assert!(
                (f.editor.view_start + offset / f.editor.pixels_per_second - 21.0).abs() < 0.0001
            );
            assert!(f.editor.follow_playhead && !f.editor.keyboard.detached);
            assert!(f.editor.pending_seek.is_none(), "zoom must not seek");
            let before = f.editor.view_start;
            f.app.forced_time = Some(22.0);
            f.app.timeline_view_time = 22.0;
            crate::runtime::follow_playhead(&mut f.editor, 22.0);
            f.frame(Vec::new());
            assert!(
                (f.editor.view_start - before).abs() < 0.0001,
                "paused playback follow stays detached after zoom"
            );
            assert!(f.editor.follow_zoom_detached);
            crate::runtime::follow_playhead_during_playback(&mut f.editor, 22.0, true);
            assert!(!f.editor.follow_zoom_detached);
            let expected = if layout == EditorLayout::Horizontal {
                20.0
            } else {
                22.0
            };
            assert!(
                (f.editor.view_start - expected).abs() < 0.0001,
                "playback resumes the normal follow position: layout={layout:?}, actual={}, expected={expected}",
                f.editor.view_start
            );
            let before = f.editor.view_start;
            f.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -60.0),
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            }]);
            assert!(
                (f.editor.view_start - before - 60.0 / f.editor.pixels_per_second).abs() < 0.0001
            );
            let time = f
                .editor
                .pending_seek
                .take()
                .expect("following pan must move the clock");
            assert!((time - 22.0 - 60.0 / f.editor.pixels_per_second).abs() < 0.0001);
            f.app.forced_time = Some(time);
            f.app.timeline_view_time = time;
            crate::runtime::follow_playhead(&mut f.editor, time);
            assert!(f.editor.follow_playhead && !f.editor.keyboard.detached);
            f.editor.follow_playhead = false;
            f.editor.keyboard.detached = true;
            let before = f.editor.view_start;
            f.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -60.0),
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            }]);
            assert!(
                (f.editor.view_start - before - 60.0 / f.editor.pixels_per_second).abs() < 0.0001
            );
            assert!(f.editor.pending_seek.is_none() && f.editor.undo.is_empty());
        }
    }

    #[test]
    fn following_middle_scroll_moves_clock_and_keeps_follow_enabled() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            super::super::navigation::current_chart(&mut f.editor, 20.0);
            let origin = position(&f, 21.0);
            let direction = if layout == EditorLayout::Vertical {
                egui::vec2(0.0, -1.0)
            } else {
                egui::vec2(1.0, 0.0)
            };
            button(&mut f, origin, egui::PointerButton::Middle, true);
            assert!(f.editor.follow_playhead && !f.editor.keyboard.detached);
            f.frame(vec![egui::Event::PointerMoved(origin + direction * 60.0)]);
            for _ in 0..3 {
                let time = f
                    .editor
                    .pending_seek
                    .take()
                    .expect("middle scroll must seek while following");
                assert!(time > f.app.timeline_time());
                f.app.forced_time = Some(time);
                f.app.timeline_view_time = time;
                let view = f.editor.view_start;
                crate::runtime::follow_playhead(&mut f.editor, time);
                assert!((f.editor.view_start - view).abs() < 0.0001);
                assert!(
                    f.editor.follow_playhead && !f.editor.keyboard.detached && f.editor.scrubbing
                );
                f.frame(Vec::new());
            }
            button(&mut f, origin, egui::PointerButton::Middle, false);
            assert!(f.editor.follow_playhead && !f.editor.keyboard.detached);
            assert!(f.editor.timeline_pan.is_none() && !f.editor.scrubbing);
            assert!(f.editor.undo.is_empty());
        }
    }

    #[test]
    fn playhead_drag_scrolls_beyond_edges_until_release() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            for direction in [-1.0, 1.0] {
                let mut f = fixture(layout);
                let start = position(&f, 20.0);
                let rect = f.ui.layout.timeline;
                let outside = match layout {
                    EditorLayout::Horizontal => egui::pos2(
                        if direction > 0.0 {
                            rect.right() + 60.0
                        } else {
                            rect.left() + LANE_GUTTER - 60.0
                        },
                        start.y,
                    ),
                    EditorLayout::Vertical => egui::pos2(
                        start.x,
                        if direction > 0.0 {
                            rect.top() + RULER_H - 60.0
                        } else {
                            rect.bottom() + 60.0
                        },
                    ),
                };
                button(&mut f, start, egui::PointerButton::Primary, true);
                f.frame(vec![egui::Event::PointerMoved(outside)]);
                assert!(f.editor.scrubbing, "{layout:?}");
                for _ in 0..3 {
                    let view = f.editor.view_start;
                    let seek = f.editor.pending_seek.unwrap();
                    f.frame(Vec::new());
                    assert!((f.editor.view_start - view) * direction > 0.0);
                    assert!((f.editor.pending_seek.unwrap() - seek) * direction > 0.0);
                }
                f.frame(vec![egui::Event::PointerMoved(start)]);
                let view = f.editor.view_start;
                f.frame(Vec::new());
                assert_eq!(f.editor.view_start, view);
                f.frame(vec![egui::Event::PointerMoved(outside)]);
                button(&mut f, outside, egui::PointerButton::Primary, false);
                assert!(!f.editor.scrubbing && f.editor.pointer_press.is_none());
                let view = f.editor.view_start;
                f.frame(Vec::new());
                assert_eq!(f.editor.view_start, view);
                assert!(f.editor.undo.is_empty());
            }
        }
    }
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
        crate::templates::apply(
            &mut f.editor,
            &mut f.app,
            crate::templates::Action::CreateSelection,
        );
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
                rect.left()
                    + LANE_GUTTER
                    + (time - f.editor.view_start) * f.editor.pixels_per_second,
                rect.top() + RULER_H + lane * (rect.height() - RULER_H) / 9.0,
            ),
        }
    }

    fn pointer(f: &mut Fixture, pos: egui::Pos2, pressed: bool) {
        let shift = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        f.frame_with_modifiers(
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: shift,
                },
            ],
            shift,
        );
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
            f.frame_with_modifiers(
                vec![egui::Event::PointerMoved(end)],
                egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            );
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 2.0);
            pointer(&mut f, egui::pos2(-20.0, -20.0), false);
            assert!(!f.ui.templates.dragging());
            assert_eq!(f.editor.undo.len(), history + 1);
            assert!(f.app.chart.notes.is_empty());
            assert!(f.editor.template_action.is_none());
            f.key(
                egui::Key::Z,
                egui::Modifiers {
                    command: true,
                    ..Default::default()
                },
            );
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 1.5);

            let history = f.editor.undo.len();
            let start = position(&f, 1.625, 6.5);
            let end = position(&f, 1.875, 6.5);
            pointer(&mut f, start, true);
            f.frame_with_modifiers(
                vec![egui::Event::PointerMoved(end)],
                egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            );
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
            f.app.chart.notes.push(Note {
                id: 77,
                time: 1.5,
                lane: 1,
                ..Default::default()
            });
            f.app.refresh_playback_chart();
            let head = position(&f, 1.5, 0.5);
            f.click(head);
            assert_eq!(f.editor.selected.iter().copied().collect::<Vec<_>>(), [77]);
            assert!(!f.ui.templates.dragging());
            assert!(f.editor.template_action.is_none());
            assert_eq!(f.app.chart.template_instances[0].anchor_time, 1.5);
        }
    }

    #[test]
    fn template_click_selection_survives_release_and_does_not_toggle_off() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            let at = position(&f, 1.625, 6.5);
            let id = f.app.chart.template_instances[0].instance_id.clone();

            f.frame(vec![egui::Event::PointerMoved(at)]);
            f.frame(vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert_eq!(f.editor.selected_instance.as_deref(), Some(id.as_str()));
            f.frame(vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            f.frame(Vec::new());
            assert_eq!(f.editor.selected_instance.as_deref(), Some(id.as_str()));

            f.click(at);
            assert_eq!(f.editor.selected_instance.as_deref(), Some(id.as_str()));
        }
    }

    #[test]
    fn template_secondary_click_keeps_context_menu_open() {
        for layout in [EditorLayout::Horizontal, EditorLayout::Vertical] {
            let mut f = fixture(layout);
            let at = position(&f, 1.625, 6.5);
            f.frame(vec![egui::Event::PointerMoved(at)]);
            f.frame(vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }]);
            f.frame(vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(f.ctx.memory(|memory| memory.any_popup_open()));
        }
    }
}

fn tool_fill(output: &egui::FullOutput, ui: &EditorUi) -> egui::Color32 {
    let center = ui.layout.tools.min + egui::vec2(16.0, 16.0);
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Rect(rect)
                if rect.rect.size() == egui::vec2(24.0, 24.0) && rect.rect.contains(center) =>
            {
                Some(rect.fill)
            }
            _ => None,
        })
        .expect("palette button must remain painted")
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
    let primary = egui::Modifiers {
        command: true,
        ..Default::default()
    };
    let shifted = egui::Modifiers {
        shift: true,
        ..primary
    };
    f.key(egui::Key::S, primary);
    assert!(
        f.editor.save_as_request,
        "an unsourced document must request Save As"
    );
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
    for (label, request) in [
        ("打开谱面...", "open"),
        ("保存", "save_as"),
        ("另存为...", "save_as"),
        ("导出 Simai...", "export"),
    ] {
        let mut f = Fixture::new();
        f.editor.dirty = true;
        let baseline = f.frame(Vec::new());
        f.click(text_position(&baseline, "文件"));
        let menu = f.frame(Vec::new());
        f.click(text_position(&menu, label));
        assert!(
            match request {
                "open" => f.editor.open_dialog,
                "save_as" => f.editor.save_as_request,
                "export" => f.editor.export_request,
                _ => false,
            },
            "{label}"
        );
        assert!(f.editor.dirty);
        assert_eq!(f.app.chart.notes.len(), 1);
    }
}

#[test]
fn command_search_exposes_file_actions_and_does_not_leak_enter_to_placement() {
    for (search, request) in [
        ("Open chart", "open"),
        ("Save chart", "save_as"),
        ("Save As", "save_as"),
        ("Export Simai", "export"),
    ] {
        let mut f = Fixture::new();
        f.key(egui::Key::F3, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text(search.into())]);
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(f.editor.keyboard.dialog.is_none());
        assert!(
            match request {
                "open" => f.editor.open_dialog,
                "save_as" => f.editor.save_as_request,
                "export" => f.editor.export_request,
                _ => false,
            },
            "{search}"
        );
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
    assert!(
        f.editor.dirty,
        "cancelled or failed open must retain unsaved changes"
    );
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
    assert!(
        f.app.chart.notes.len() == 1,
        "dialog confirmation must not place a note"
    );
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
    let item = opened
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(text)
                if text.galley.text() == "播放" && text.pos.y > menu.y + 5.0 =>
            {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .next()
        .expect("play menu item must be visible");
    let clicked = f.click(item);
    assert_eq!(f.app.mode, Mode::Playing);
    assert_eq!(
        tool_fill(&clicked, &f.ui),
        color,
        "closing-click frame must not dim the palette"
    );
    let timeline = f.ui.layout.timeline;
    let next = f.frame(vec![egui::Event::PointerGone]);
    assert_eq!(f.ui.layout.timeline, timeline);
    assert_eq!(tool_fill(&next, &f.ui), color);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(
        f.app.mode,
        Mode::Idle,
        "menu must release keyboard ownership"
    );
}

#[test]
fn held_space_does_not_toggle_playback_repeatedly() {
    let mut f = Fixture::new();
    let event = |repeat| egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: egui::Modifiers::NONE,
    };
    f.frame(vec![event(false)]);
    assert_eq!(f.app.mode, Mode::Playing);
    f.frame(vec![event(true)]);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn simai_playback_space_blocks_separate_text_events_and_preserves_paste() {
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true);
    assert!(f.editor.simai_editor.options.space_playback);
    f.editor.simai_editor.options.live_apply = false;
    f.frame(Vec::new());
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    f.ctx.memory_mut(|m| m.request_focus(id));
    f.frame(Vec::new());
    let text = f.editor.simai_editor.text.clone();
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
    f.frame(vec![egui::Event::Text(" ".into())]);
    assert_eq!(f.editor.simai_editor.text, text);
    assert_eq!(f.app.mode, Mode::Playing);
    f.frame(vec![egui::Event::Paste(" ".into())]);
    assert_eq!(f.editor.simai_editor.text.len(), text.len() + 1);
    f.editor.simai_editor.options.space_playback = false;
    f.frame(vec![egui::Event::Text(" ".into())]);
    assert_eq!(f.editor.simai_editor.text.len(), text.len() + 2);
}

#[test]
fn simai_undo_redo_restores_visual_edits() {
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true);
    f.frame(Vec::new());
    let original = f.app.chart.clone();
    let original_text = f.editor.simai_editor.text.clone();
    f.editor.push_history(&f.app.chart);
    f.app.chart.notes[0].lane = 4;
    f.frame(Vec::new());
    let edited_text = f.editor.simai_editor.text.clone();
    assert_ne!(edited_text, original_text);
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    f.ctx.memory_mut(|m| m.request_focus(id));
    f.frame(Vec::new());
    f.key(egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(f.app.chart, original, "Simai undo must restore a visual edit");
    assert_eq!(f.editor.simai_editor.text, original_text);
    f.key(egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
    assert_eq!(f.app.chart.notes[0].lane, 4);
    assert_eq!(f.editor.simai_editor.text, edited_text);
    assert!(f.ctx.memory(|m| m.has_focus(id)), "undo keeps Simai keyboard focus");
}

#[test]
fn simai_modal_undo_redo_restores_visual_edits() {
    use crate::simai_editor::modal::{Mode as EditMode, Style};
    for style in [Style::Vim, Style::Helix] {
        let mut f = Fixture::new();
        f.ui.workbench.set_visible(PaneKind::Simai, true);
        f.frame(Vec::new());
        f.editor.simai_editor.options.edit_style = style;
        let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
        f.ctx.memory_mut(|m| m.request_focus(id));
        f.frame(Vec::new());
        let original = f.app.chart.clone();
        let original_text = f.editor.simai_editor.text.clone();
        f.editor.push_history(&f.app.chart);
        f.app.chart.notes[0].lane = 4;
        f.frame(Vec::new());
        let edited_text = f.editor.simai_editor.text.clone();
        assert!(f.ctx.memory(|m| m.has_focus(id)), "visual sync preserves focus");
        f.frame(vec![egui::Event::Text("u".into())]);
        assert_eq!(f.app.chart, original, "{style:?} undo restores the chart");
        assert_eq!(f.editor.simai_editor.text, original_text);
        if style == Style::Vim { f.key(egui::Key::R, egui::Modifiers::CTRL); }
        else { f.frame(vec![egui::Event::Text("U".into())]); }
        assert_eq!(f.app.chart.notes[0].lane, 4);
        assert_eq!(f.editor.simai_editor.text, edited_text);
        assert_eq!(f.editor.simai_editor.input.modal.mode, EditMode::Normal);
        assert!(f.ctx.memory(|m| m.has_focus(id)));
    }
}

#[test]
fn simai_modal_modes_preserve_input_ownership_and_text_history() {
    use crate::simai_editor::modal::{Mode as EditMode, Style};
    for style in [Style::Vim, Style::Helix] {
        let mut f = Fixture::new();
        f.ui.workbench.set_visible(PaneKind::Simai, true);
        f.frame(Vec::new());
        f.editor.simai_editor.options.edit_style = style;
        f.editor.simai_editor.options.live_apply = false;
        f.editor.simai_editor.options.auto_completion = false;
        let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
        f.ctx.memory_mut(|m| m.request_focus(id));
        f.frame(Vec::new());
        let original = f.editor.simai_editor.text.clone();
        let notes = f.app.chart.notes.clone();
        f.key(egui::Key::I, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text("i".into())]);
        assert_eq!(f.editor.simai_editor.input.modal.mode, EditMode::Insert);
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text(" ".into())]);
        assert_eq!(f.app.mode, Mode::Idle, "Insert Space must not start playback");
        assert_eq!(f.editor.simai_editor.text.len(), original.len() + 1);
        f.key(egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(f.editor.simai_editor.input.modal.mode, EditMode::Normal);
        assert!(f.ctx.memory(|m| m.has_focus(id)), "Esc keeps text focus");
        f.frame(vec![egui::Event::Text("u".into())]);
        assert_eq!(f.editor.simai_editor.text, original);
        if style == Style::Vim { f.key(egui::Key::R, egui::Modifiers::CTRL); }
        else { f.frame(vec![egui::Event::Text("U".into())]); }
        assert_eq!(f.editor.simai_editor.text.len(), original.len() + 1);
        assert_eq!(f.app.chart.notes, notes, "modal keys never place chart notes");
    }
}

#[test]
fn simai_completion_accept_escape_and_snippet_tab_are_undoable() {
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true);
    f.frame(Vec::new());
    f.editor.simai_editor.options.live_apply = false;
    f.editor.simai_editor.options.space_playback = false;
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    f.ctx.memory_mut(|m| m.request_focus(id));
    f.frame(Vec::new());
    let original = f.editor.simai_editor.text.clone();
    f.frame(vec![egui::Event::Text("[".into())]);
    assert!(f.editor.simai_editor.input.completion.open());
    f.key(egui::Key::Tab, egui::Modifiers::NONE);
    assert!(f.editor.simai_editor.text.contains("[4:1]"));
    assert!(!f.editor.simai_editor.input.completion.fields.is_empty());
    f.frame(vec![egui::Event::Text("16".into())]);
    f.key(egui::Key::Tab, egui::Modifiers::NONE);
    assert!(f.editor.simai_editor.text.contains("[16:1]"));
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    f.key(egui::Key::Z, egui::Modifiers::COMMAND);
    f.key(egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(f.editor.simai_editor.text, format!("[]{original}"));
    assert!(f.ctx.memory(|m| m.has_focus(id)));
}

#[test]
fn simai_completion_can_be_accepted_with_a_pointer_click() {
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true); f.frame(Vec::new());
    f.editor.simai_editor.options.live_apply = false;
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    f.ctx.memory_mut(|m| m.request_focus(id)); f.frame(Vec::new());
    f.frame(vec![egui::Event::Text("[".into())]);
    let output = f.frame(Vec::new());
    let at = text_position(&output, "[4:1]  时值");
    let before = f.editor.simai_editor.text.clone();
    f.click(at);
    assert!(f.editor.simai_editor.text.contains("[4:1]"));
    assert!(!f.editor.simai_editor.input.completion.open());
    assert!(f.ctx.memory(|m| m.has_focus(id)));
    let completed = f.editor.simai_editor.text.clone();
    f.editor.undo(&mut f.app.chart);
    assert_eq!(f.editor.simai_editor.text, before);
    f.editor.redo(&mut f.app.chart);
    assert_eq!(f.editor.simai_editor.text, completed);
}

#[test]
fn simai_helix_block_cursor_covers_the_selected_character() {
    use crate::simai_editor::modal::Style;
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true);
    f.frame(Vec::new());
    let body = "1234,\n56,E";
    f.editor.simai_editor.load(&f.app.chart, body.into());
    f.editor.simai_editor.options.edit_style = Style::Helix;
    f.editor.simai_editor.options.live_apply = false;
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    let mut state = egui::text_edit::TextEditState::load(&f.ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(0))));
    state.store(&f.ctx, id);
    f.ctx.memory_mut(|m| m.request_focus(id));
    f.frame(Vec::new());
    let output = f.frame(vec![egui::Event::Text("l".into())]);
    assert_eq!(f.editor.simai_editor.input.selection.range(), 1..2);
    let text = output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text() == body => Some(t), _ => None,
    }).unwrap();
    let expected = text.galley.pos_from_ccursor(egui::text::CCursor::new(1)).translate(text.pos.to_vec2());
    let block = output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgba_unmultiplied(120, 195, 235, 100) => Some(r.rect), _ => None,
    }).unwrap();
    assert!((block.left() - expected.left()).abs() < 0.01, "block={block:?}, character={expected:?}");
}

#[test]
fn simai_helix_seek_uses_block_character_not_exclusive_selection_end() {
    use crate::simai_editor::{modal::Style, source};
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true); f.frame(Vec::new());
    let body = "(120){4}1,2,E";
    f.editor.simai_editor.load(&f.app.chart, body.into());
    f.editor.simai_editor.options.edit_style = Style::Helix;
    f.editor.simai_editor.options.live_apply = false;
    f.editor.simai_editor.playback.cells = source::timeline(body, 120.0).unwrap();
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    let mut state = egui::text_edit::TextEditState::load(&f.ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(8), egui::text::CCursor::new(9))));
    state.store(&f.ctx, id); f.ctx.memory_mut(|m| m.request_focus(id));
    f.frame(Vec::new());
    f.key(egui::Key::Enter, egui::Modifiers::ALT);
    assert_eq!(f.app.timeline_view_time, 0.0, "character 1 belongs to the first cell, not the following rest");
}

#[test]
fn simai_reverse_search_highlights_without_editing_and_starts_insert_at_match() {
    use crate::simai_editor::modal::Style;
    let mut f = Fixture::new();
    f.ui.workbench.set_visible(PaneKind::Simai, true); f.frame(Vec::new());
    let body = "1h[4:1],2h[4:1],E";
    f.editor.simai_editor.load(&f.app.chart, body.into());
    f.editor.simai_editor.options.edit_style = Style::Vim;
    f.editor.simai_editor.options.live_apply = false;
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    let mut state = egui::text_edit::TextEditState::load(&f.ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(0))));
    state.store(&f.ctx, id); f.ctx.memory_mut(|m| m.request_focus(id)); f.frame(Vec::new());
    f.frame(vec![egui::Event::Text("?h".into())]);
    let output = f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(f.editor.simai_editor.input.selection.head, 9);
    assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Vec(shapes) if shapes.iter().any(|s|
        matches!(s, egui::Shape::Rect(r) if r.fill == egui::Color32::from_rgba_unmultiplied(210, 170, 50, 90))))));
    assert_eq!(f.editor.simai_editor.text, body);
    f.frame(vec![egui::Event::Text("n".into())]);
    assert_eq!(f.editor.simai_editor.input.selection.head, 1);
    f.frame(vec![egui::Event::Text("i".into())]);
    f.frame(vec![egui::Event::Text("b".into())]);
    assert_eq!(f.editor.simai_editor.text, "1bh[4:1],2h[4:1],E");
}

#[test]
fn cursor_shortcuts_seek_and_toggle_follow_in_chart_and_simai() {
    let mut f = Fixture::new();
    f.app.chart.notes[0].time = 6.0;
    f.editor.keyboard.cursor.measure = 2.0;
    f.key(
        egui::Key::Enter,
        egui::Modifiers::ALT | egui::Modifiers::SHIFT,
    );
    assert!(f.editor.keyboard_seek_on_navigation);
    assert_eq!(f.app.timeline_view_time, 2.0);
    f.key(
        egui::Key::Enter,
        egui::Modifiers::ALT | egui::Modifiers::SHIFT,
    );
    assert!(!f.editor.keyboard_seek_on_navigation);
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(f.app.timeline_view_time, 2.0);
    f.key(egui::Key::Enter, egui::Modifiers::ALT);
    assert_eq!(
        f.app.timeline_view_time,
        crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms)
    );
    assert_eq!(f.app.chart.notes.len(), 1);

    f.ui.workbench.set_visible(PaneKind::Simai, true);
    f.frame(Vec::new());
    let id = egui::Id::new(("simai-text", f.editor.simai_editor.generation));
    f.ctx.memory_mut(|m| m.request_focus(id));
    let text = f.editor.simai_editor.text.clone();
    let cells = crate::simai_editor::source::timeline(&text, f.app.chart.bpm).unwrap();
    let (first, second) = (cells[0].chars.start, cells[1].chars.start);
    let time = cells[1].time;
    f.editor.simai_editor.playback.cells = cells;
    let set_cursor = |f: &mut Fixture, index| {
        let mut state = egui::text_edit::TextEditState::load(&f.ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(index),
            )));
        state.store(&f.ctx, id);
    };
    set_cursor(&mut f, second);
    f.app.seek_to(0.0);
    f.frame(Vec::new());
    assert_eq!(f.app.timeline_view_time, 0.0);
    f.key(egui::Key::Enter, egui::Modifiers::ALT);
    assert_eq!(f.app.timeline_view_time, time);
    assert_eq!(f.app.mode, Mode::Idle);
    f.key(
        egui::Key::Enter,
        egui::Modifiers::ALT | egui::Modifiers::SHIFT,
    );
    assert!(f.editor.simai_editor.options.follow_cursor);
    set_cursor(&mut f, first);
    f.frame(Vec::new());
    assert_eq!(f.app.timeline_view_time, 0.0);
    f.app.seek_to(time);
    f.frame(Vec::new());
    assert_eq!(
        f.app.timeline_view_time, time,
        "stationary caret must not pull playback back"
    );
    assert_eq!(
        f.editor.simai_editor.text, text,
        "shortcuts must not insert newlines"
    );
}

#[test]
fn rewind_uses_loop_start_only_during_loop_playback() {
    for mode in [Mode::Idle, Mode::Playing] {
        for loop_enabled in [false, true] {
            let mut f = Fixture::new();
            f.app.chart.notes[0].time = 20.0;
            f.editor.loop_range = Some((2.0, 6.0));
            f.editor.loop_enabled = loop_enabled;
            f.app.mode = mode;
            f.app.seek_to(3.0);
            f.key(egui::Key::Home, egui::Modifiers::ALT);
            let expected = if loop_enabled && mode == Mode::Playing {
                2.0
            } else {
                0.0
            };
            assert_eq!(f.app.timeline_view_time, expected);
            assert_eq!(f.app.mode, mode);
        }
    }
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
    assert_eq!(
        f.app.timeline_view_time, 3.0,
        "jump must seek playback in the confirmation frame"
    );
    assert_eq!(f.editor.view_start, 3.0);
    f.key(egui::Key::Space, egui::Modifiers::NONE);
    assert_eq!(f.app.mode, Mode::Playing);
}

#[test]
fn jump_aligns_cursor_and_view_while_playing_and_clamps_to_song_end() {
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
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
        let cursor = crate::keyboard::navigation::position(
            &f.editor,
            &f.app.chart,
            3.0,
            f.editor.keyboard.cursor,
        );
        assert!(f.editor.keyboard.rect.contains(cursor));
        f.frame(Vec::new());
        assert_eq!(
            f.editor.view_start, 3.0,
            "auto-scroll must not offset the jump target next frame"
        );
        f.key(egui::Key::J, egui::Modifiers::NONE);
        f.frame(vec![egui::Event::Text("999s".into())]);
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        let end = f.app.song_duration();
        assert_eq!(f.app.timeline_view_time, end);
        assert_eq!(f.editor.view_start, end);
        assert_eq!(
            crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms),
            end
        );
    }
}

#[test]
fn jump_remains_aligned_after_measure_roundtrip_on_tempo_changes() {
    use crate::app::types::BpmChange;
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 100.0;
        f.app.chart.bpms = vec![
            BpmChange {
                measure: 1.0,
                bpm: 141.0,
            },
            BpmChange {
                measure: 3.0,
                bpm: 174.0,
            },
            BpmChange {
                measure: 6.0,
                bpm: 215.0,
            },
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
    note.id = 101;
    note.time = 3.0;
    note.lane = 2;
    f.app.chart.notes.push(note.clone());
    note.id = 102;
    note.time = 2.0;
    note.lane = 5;
    f.app.chart.notes.push(note.clone());
    note.id = 103;
    note.lane = 3;
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
    assert_eq!(f.app.timeline_view_time, 0.0);
}

#[test]
fn held_tab_continues_navigation_in_both_directions_without_seeking() {
    let mut f = Fixture::new();
    let mut note = f.app.chart.notes[0].clone();
    for (id, time) in [(101, 2.0), (102, 3.0), (103, 4.0)] {
        note.id = id;
        note.time = time;
        f.app.chart.notes.push(note.clone());
    }
    let event = |repeat, modifiers| egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    };
    for (repeat, id) in [
        (false, 101),
        (true, 102),
        (true, 103),
        (true, f.app.chart.notes[0].id),
    ] {
        f.frame(vec![event(repeat, egui::Modifiers::NONE)]);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        let time =
            crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert_eq!(f.app.timeline_view_time, 0.0);
        let cursor = crate::keyboard::navigation::position(
            &f.editor,
            &f.app.chart,
            time,
            f.editor.keyboard.cursor,
        );
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
            EditorLayout::Vertical => (
                egui::Key::ArrowUp,
                egui::Key::ArrowDown,
                egui::Key::ArrowRight,
                egui::Key::ArrowLeft,
            ),
            EditorLayout::Horizontal => (
                egui::Key::ArrowRight,
                egui::Key::ArrowLeft,
                egui::Key::ArrowDown,
                egui::Key::ArrowUp,
            ),
        };
        let fast = egui::Modifiers {
            shift: true,
            ..egui::Modifiers::COMMAND
        };
        let start = f.editor.keyboard.cursor.measure;
        f.key(later, egui::Modifiers::COMMAND);
        assert_eq!(
            f.editor.keyboard.cursor.measure,
            start + 1.0 / f.editor.snap_division as f32
        );
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
            key,
            physical_key: None,
            pressed,
            repeat,
            modifiers: egui::Modifiers::NONE,
        };
        f.frame(vec![event(time_key, true, false)]);
        f.frame(vec![event(lane_key, true, false)]);
        let start = f.editor.keyboard.cursor;
        for step in 1..=3 {
            f.frame(vec![event(lane_key, true, true)]);
            assert_eq!(
                f.editor.keyboard.cursor.measure,
                start.measure + step as f32 / f.editor.snap_division as f32
            );
            assert_eq!(f.editor.keyboard.cursor.zone, start.zone + step);
        }
        f.frame(vec![
            event(lane_key, false, false),
            event(time_key, true, true),
        ]);
        assert_eq!(
            f.editor.keyboard.cursor.measure,
            start.measure + 4.0 / f.editor.snap_division as f32
        );
        assert_eq!(f.editor.keyboard.cursor.zone, start.zone + 3);
        assert_eq!(f.app.timeline_view_time, 0.0);
    }
}

#[test]
fn cursor_properties_and_deletion_target_pointed_note_without_selecting_it() {
    for key in [egui::Key::Delete, egui::Key::Backspace, egui::Key::D] {
        let mut f = Fixture::new();
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        note.id = 101;
        note.time = 2.0;
        note.lane = 3;
        f.app.chart.notes.push(note);
        f.editor.select_only(original);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        let output = f.frame(Vec::new());
        text_position(&output, "Cursor · id 101");
        assert_eq!(f.editor.single_selected(), Some(original));
        f.key(egui::Key::F5, egui::Modifiers::NONE);
        assert_eq!(
            f.editor
                .keyboard
                .dialog
                .as_ref()
                .unwrap()
                .note
                .as_ref()
                .unwrap()
                .id,
            101
        );
        f.editor
            .keyboard
            .dialog
            .as_mut()
            .unwrap()
            .note
            .as_mut()
            .unwrap()
            .is_break = true;
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
    note.id = 101;
    note.lane = 2;
    note.note_type = crate::app::types::NoteType::Hold;
    note.hold_duration = 0.5;
    f.app.chart.notes.push(note);
    f.editor.select_only(original);
    f.key(egui::Key::ArrowRight, egui::Modifiers::NONE);
    let output = f.frame(Vec::new());
    text_position(&output, "Cursor · id 101");
    assert_eq!(f.editor.single_selected(), Some(original));
    f.key(egui::Key::F7, egui::Modifiers::NONE);
    assert_eq!(
        f.editor
            .keyboard
            .dialog
            .as_ref()
            .unwrap()
            .note
            .as_ref()
            .unwrap()
            .id,
        101
    );
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(matches!(
        f.editor.keyboard.transaction.as_ref().unwrap().kind,
        crate::keyboard::edit::Draft::Handle { id: 101, .. }
    ));
    assert_eq!(f.editor.single_selected(), Some(original));
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(f.app.chart.notes[1].hold_duration, 0.5);
}

#[test]
fn diagonal_repeat_cancels_opposites_and_counts_each_axis_once_per_frame() {
    let mut f = Fixture::new();
    f.app.chart.notes[0].time = 20.0;
    let mods = egui::Modifiers {
        shift: true,
        ..egui::Modifiers::COMMAND
    };
    let event = |key, pressed, repeat| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers: mods,
    };
    f.frame(vec![
        event(egui::Key::ArrowUp, true, false),
        event(egui::Key::ArrowRight, true, false),
    ]);
    let start = f.editor.keyboard.cursor;
    f.frame(vec![
        event(egui::Key::ArrowUp, true, true),
        event(egui::Key::ArrowRight, true, true),
    ]);
    assert_eq!(f.editor.keyboard.cursor.measure, start.measure + 1.0);
    assert_eq!(
        f.editor.keyboard.cursor.zone,
        crate::timeline::geometry::rotated_zone(start.zone, 4)
    );
    f.frame(vec![event(egui::Key::ArrowLeft, true, false)]);
    let start = f.editor.keyboard.cursor;
    f.frame(vec![event(egui::Key::ArrowLeft, true, true)]);
    assert_eq!(f.editor.keyboard.cursor.measure, start.measure + 1.0);
    assert_eq!(f.editor.keyboard.cursor.zone, start.zone);
}

#[test]
fn navigation_preserves_playback_and_can_edit_past_the_song_end() {
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.pixels_per_second = 180.0;
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        let key = match layout {
            crate::state::EditorLayout::Vertical => egui::Key::ArrowUp,
            crate::state::EditorLayout::Horizontal => egui::Key::ArrowRight,
        };
        f.key(key, egui::Modifiers::SHIFT);
        assert_eq!(f.app.mode, Mode::Playing);
        assert_eq!(f.editor.keyboard.cursor.measure, 2.0);
        let time = crate::app::types::measure_to_secs(2.0, &f.app.chart.bpms);
        assert_eq!(
            f.editor.view_start, 0.0,
            "editing beyond the song end still inside the viewport must not scroll"
        );
        assert!(time > f.app.song_duration());
        assert_eq!(f.app.timeline_view_time, 0.0);
        f.editor.tool = Tool::Tap;
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(f.app.chart.notes.last().unwrap().time, 2.0);
    }
}

#[test]
fn tab_keeps_overlapping_note_identity_across_tempo_changes() {
    let mut f = Fixture::new();
    f.app.chart.bpms = vec![crate::app::types::BpmChange {
        measure: 1.0,
        bpm: 141.0,
    }];
    let mut note = f.app.chart.notes[0].clone();
    note.time = 3.137;
    note.lane = 4;
    for id in [101, 102, 103] {
        note.id = id;
        f.app.chart.notes.push(note.clone());
    }
    for id in [101, 102, 103] {
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        assert_eq!(f.editor.keyboard.cursor.measure, 3.137);
        assert!(f.editor.selected.is_empty());
    }
}

#[test]
fn tab_handles_single_empty_and_offscreen_charts_in_both_layouts() {
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.app.chart.notes[0].time = 40.0;
        let id = f.app.chart.notes[0].id;
        f.editor.keyboard.cursor.measure = 45.0;
        f.key(egui::Key::Tab, egui::Modifiers::SHIFT);
        assert_eq!(f.editor.keyboard.candidate, Some(id));
        let cursor = crate::keyboard::navigation::position(
            &f.editor,
            &f.app.chart,
            0.0,
            f.editor.keyboard.cursor,
        );
        assert!(
            f.editor.keyboard.rect.expand(0.5).contains(cursor),
            "{layout:?}: {cursor:?}"
        );
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
        egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
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
        f.app = crate::runtime::initialize_player(
            PadPreviewState::new_at(chart, None, None, 0.0, None),
            None,
            None,
        );
        f.app.forced_time = Some(0.0);
        assert!(f.app.has_engine(), "{}", f.app.status);
        let output = f.frame(Vec::new());
        f.click(text_position(&output, "Autoplay"));
        assert_eq!(f.editor.workspace, Workspace::Autoplay);
        f.frame(Vec::new());
        f.frame(Vec::new());
        f
    }

    fn marker(output: &egui::FullOutput) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle) if circle.radius == 5.0 => Some(circle.center),
                _ => None,
            })
            .expect("event marker must be visible")
    }

    fn raw_fixture() -> Fixture {
        let mut f = fixture();
        let output = f.frame(Vec::new());
        f.click(text_position(
            &output,
            &char::from(lucide_icons::Icon::ListFilter).to_string(),
        ));
        let menu = f.frame(Vec::new());
        f.click(text_position(&menu, "原始事件视图"));
        f.key(egui::Key::Escape, egui::Modifiers::NONE);
        f.frame(Vec::new());
        f
    }

    #[test]
    fn timeline_regression_slide_group_click_draws_sensor_details() {
        let mut f = fixture();
        f.ui.workbench.reset_layout();
        let chart =
            crate::app::maidata::from_maidata_level("&inote_6=(120){4}1-5[4:2],B2,E", 6).unwrap();
        f.app = crate::runtime::initialize_player(
            PadPreviewState::new_at(chart, None, None, 0.0, None),
            None,
            None,
        );
        f.app.forced_time = Some(0.75);
        assert!(f.app.has_engine(), "{}", f.app.status);
        let output = f.frame(Vec::new());
        let track = f
            .editor
            .autoplay_editor
            .tracks
            .iter()
            .find(|track| track.key.role == 2)
            .expect("slide must have an autoplay group");
        let label = format!("#{} Slide {}.1", track.key.note_id, track.key.slide + 1);
        let sensor = track
            .events
            .iter()
            .find_map(|event| match event {
                crate::core::types::TimedInputEvent::SensorHold { area, .. }
                | crate::core::types::TimedInputEvent::SensorClick { area, .. } => {
                    Some(format!("{} {area:?}", track.key.segment + 1))
                }
                _ => None,
            })
            .expect("slide must have sensor events");
        f.click(text_position(&output, &label));
        assert!(
            f.editor.autoplay_editor.selected_row.is_some(),
            "group click was ignored"
        );
        let output = f.frame(Vec::new());
        f.click(text_position(
            &output,
            &char::from(lucide_icons::Icon::AudioLines).to_string(),
        ));
        let output = f.frame(Vec::new());
        text_position(&output, &sensor);
    }

    #[test]
    fn timeline_regression_subdivision_popup_keeps_controls_interactive() {
        let mut f = fixture();
        f.ui.workbench.reset_layout();
        f.editor.snap_division = 16;
        f.editor.autoplay_snap_division = 16;
        let output = f.frame(Vec::new());
        f.click(text_position(&output, "1/16"));
        let menu = f.frame(Vec::new());
        f.click(text_position(&menu, "细分 1/16"));
        let menu = f.frame(Vec::new());
        f.click(text_position(&menu, "1/8"));
        assert_eq!(
            f.editor.autoplay_snap_division, 8,
            "subdivision popup disabled itself"
        );
        assert_eq!(
            f.editor.snap_division, 16,
            "autoplay subdivision changed the chart pane"
        );
        f.frame(Vec::new());
        let output = f.frame(Vec::new());
        assert!(
            !f.ctx.memory(|m| m.any_popup_open()),
            "subdivision popup stayed open"
        );
        assert!(
            !f.ui.input_blocked,
            "subdivision change left the timeline disabled"
        );
        let position = text_position(&output, "01") + egui::vec2(200.0, 0.0);
        let view = f.editor.autoplay_editor.view_start;
        f.frame(vec![
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -60.0),
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
        ]);
        assert_ne!(f.editor.autoplay_editor.view_start, view);
    }

    #[test]
    fn timeline_regression_combined_autoplay_remains_live_without_focus() {
        let mut f = fixture();
        let output = f.frame(Vec::new());
        f.click(text_position(&output, "组合"));
        f.ui.workbench.reset_layout();
        f.frame(Vec::new());
        let output = f.frame(Vec::new());
        assert_eq!(f.editor.workspace, Workspace::Chart);
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { stroke, .. } if stroke.color == crate::theme::ACCENT_RED
            )),
            "unfocused Autoplay timeline must not be greyed out"
        );
        let position = text_position(&output, "01") + egui::vec2(200.0, 0.0);
        let view = f.editor.autoplay_editor.view_start;
        f.frame(vec![
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -60.0),
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
        ]);
        assert_ne!(
            f.editor.autoplay_editor.view_start, view,
            "visible timeline must remain browsable"
        );
        assert_eq!(
            f.editor.workspace,
            Workspace::Chart,
            "hover must not take keyboard focus"
        );
    }

    #[test]
    fn event_click_drag_release_outside_and_undo_are_one_transaction() {
        let mut f = raw_fixture();
        f.editor.autoplay_snap_enabled = false;
        let output = f.frame(Vec::new());
        let position = marker(&output);
        f.click(position);
        let selected = f
            .editor
            .autoplay_editor
            .selected
            .expect("marker click must not be swallowed by its row");
        assert_eq!(f.editor.autoplay_editor.selected_row, Some(selected.0));
        let original = f.editor.autoplay_editor.tracks[selected.0].events[selected.1].clone();
        let moved =
            position + egui::vec2(f.editor.autoplay_editor.pixels_per_second * 0.25, -500.0);
        f.frame(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        f.frame(vec![egui::Event::PointerMoved(moved)]);
        assert!(f.editor.autoplay_editor.dragging());
        assert!(f.editor.undo.is_empty(), "a pending drag is not committed");
        f.frame(vec![egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(!f.editor.autoplay_editor.dragging());
        assert_eq!(f.editor.undo.len(), 1);
        let event = &f.editor.autoplay_editor.tracks[selected.0].events[selected.1];
        assert_eq!(
            crate::player::engine::timed_input_tp(event),
            crate::player::engine::timed_input_tp(&original) + 250_000
        );
        assert_eq!(f.app.chart.autoplay_overrides.len(), 1);
        f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert!(f.app.chart.autoplay_overrides.is_empty());
        assert_eq!(
            f.editor.autoplay_editor.tracks[selected.0].events[selected.1],
            original
        );
    }

    #[test]
    fn escape_cancels_marker_drag_without_recording_an_edit() {
        let mut f = raw_fixture();
        let output = f.frame(Vec::new());
        let position = marker(&output);
        let original = f.editor.autoplay_editor.tracks[0].events.clone();
        f.frame(vec![egui::Event::PointerMoved(position)]);
        f.frame(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        f.frame(vec![egui::Event::PointerMoved(
            position + egui::vec2(75.0, 0.0),
        )]);
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
        let binding = f
            .editor
            .keyboard
            .bindings
            .iter_mut()
            .find(|b| b.command == "play")
            .unwrap();
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
        text_position(&help, "Edit curve / open sensor detail");
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
        crate::templates::apply(
            &mut f.editor,
            &mut f.app,
            crate::templates::Action::CreateSelection,
        );
        crate::templates::apply(
            &mut f.editor,
            &mut f.app,
            crate::templates::Action::EnterInstance("inst_1".into()),
        );
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
fn slide_timing_draft_undo_cancels_without_undoing_an_earlier_edit() {
    let mut f = Fixture::new();
    f.editor.tool = Tool::Tap;
    f.editor.place_note(&mut f.app.chart, 2.0, 3);
    let before = f.app.chart.clone();
    let history = f.editor.undo.len();
    f.editor.tool = Tool::Slide;
    f.editor.place_star_step(&mut f.app.chart, 3.0, 1);
    f.key(egui::Key::Z, egui::Modifiers::COMMAND);
    assert!(f.editor.star_placement.is_none(), "undo must cancel the timing draft");
    assert_eq!(f.app.chart, before);
    assert_eq!(f.editor.undo.len(), history);
    assert!(f.editor.redo.is_empty());
}

#[test]
fn slide_keyboard_draft_cancels_at_each_stage_without_consuming_history() {
    for trajectory in [false, true] {
        for (key, modifiers) in [
            (egui::Key::Escape, egui::Modifiers::NONE),
            (egui::Key::Z, egui::Modifiers::COMMAND),
            (egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
        ] {
            let mut f = Fixture::new();
            f.editor.tool = Tool::Tap;
            f.editor.place_note(&mut f.app.chart, 2.0, 3);
            if modifiers.shift { f.editor.undo(&mut f.app.chart); }
            f.editor.selected.clear();
            let original = f.app.chart.clone();
            let history = (f.editor.undo.len(), f.editor.redo.len(), f.editor.dirty);
            f.editor.tool = Tool::Slide;
            f.key(egui::Key::Num1, egui::Modifiers::NONE);
            f.key(egui::Key::Enter, egui::Modifiers::NONE);
            f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
            if trajectory {
                f.key(egui::Key::Enter, egui::Modifiers::NONE);
                f.key(egui::Key::Num5, egui::Modifiers::NONE);
            }
            assert!(f.editor.keyboard.transaction.is_some());
            f.key(key, modifiers);
            assert_eq!(f.app.chart, original);
            assert_eq!((f.editor.undo.len(), f.editor.redo.len(), f.editor.dirty), history);
            assert!(f.editor.keyboard.transaction.is_none());
            assert!(f.editor.editing_slide.is_none());
            assert!(f.editor.check_slide_placement_finished().is_ok());
        }
    }
}

#[test]
fn slide_draft_save_reports_pending_in_every_placement_stage() {
    for stage in 0..4 {
        let mut f = Fixture::new();
        f.editor.tool = Tool::Slide;
        if stage == 0 {
            f.editor.place_star_step(&mut f.app.chart, 2.0, 1);
        } else if stage == 1 {
            for time in [2.0, 2.25, 3.0] { f.editor.place_star_step(&mut f.app.chart, time, 1); }
        } else {
            f.key(egui::Key::Num1, egui::Modifiers::NONE);
            f.key(egui::Key::Enter, egui::Modifiers::NONE);
            if stage == 3 {
                f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
                f.key(egui::Key::Enter, egui::Modifiers::NONE);
                f.key(egui::Key::Num5, egui::Modifiers::NONE);
            }
        }
        assert!(f.editor.check_slide_placement_finished().is_err());
        let draft = f.app.chart.clone();
        f.editor.status = None;
        f.key(egui::Key::S, egui::Modifiers::COMMAND);
        assert_eq!(f.editor.status.as_deref(), Some("Finish or cancel the Slide placement before saving"), "stage {stage}");
        assert_eq!(f.app.chart, draft);
        assert!(f.editor.source.is_none());
        assert!(f.editor.undo.is_empty());
        assert!(!f.editor.dirty);
    }
}

#[test]
fn slide_default_opposite_setting_saves_and_reloads_through_settings_ui() {
    const CHILD: &str = "LAMBDADX_TEST_SLIDE_SETTINGS_CACHE";
    if std::env::var_os(CHILD).is_none() {
        let dir = std::env::temp_dir().join(format!("lambdadx-slide-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::tests::slide_default_opposite_setting_saves_and_reloads_through_settings_ui", "--nocapture"])
            .env(CHILD, &dir)
            .env("MAI2_CACHE_DIR", &dir)
            .env("MAI2_DATA_DIR", &dir)
            .output();
        let _ = std::fs::remove_dir_all(&dir);
        let result = result.unwrap();
        assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        return;
    }
    let mut f = Fixture::new();
    assert!(!f.app.params.editor_slide_default_opposite);
    f.key(egui::Key::F1, egui::Modifiers::NONE);
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "视图与判定"));
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "工作区与分区"));
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "Slide 放置后默认直线到对面"));
    assert!(f.app.params.editor_slide_default_opposite);
    assert!(crate::app::params::get().editor_slide_default_opposite);
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "保存设置"));
    assert!(crate::app::params::load().editor_slide_default_opposite);
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "Slide 放置后默认直线到对面"));
    assert!(!f.app.params.editor_slide_default_opposite);
    let output = f.frame(Vec::new());
    f.click(text_position(&output, "重新载入"));
    assert!(f.app.params.editor_slide_default_opposite);
}

#[test]
fn slide_pending_path_confirms_once_or_escape_restores_the_document() {
    for confirm in [false, true] {
        let mut f = Fixture::new();
        let original = f.app.chart.clone();
        f.editor.tool = Tool::Slide;
        for time in [2.0, 2.25, 3.0] { f.editor.place_star_step(&mut f.app.chart, time, 1); }
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(f.editor.slide_draft.is_some(), "empty paths cannot be confirmed");
        f.key(egui::Key::Num5, egui::Modifiers::NONE);
        assert!(f.editor.undo.is_empty());
        f.key(if confirm { egui::Key::Enter } else { egui::Key::Escape }, egui::Modifiers::NONE);
        assert!(f.editor.check_slide_placement_finished().is_ok());
        assert!(f.editor.editing_slide.is_none());
        if confirm {
            assert_eq!(f.app.chart.notes.len(), original.notes.len() + 1);
            assert_eq!(f.editor.undo.len(), 1);
            f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        } else {
            assert!(f.editor.undo.is_empty());
            assert!(!f.editor.dirty);
        }
        assert_eq!(f.app.chart, original);
    }
}

#[test]
fn slide_pending_path_takes_priority_over_autoplay_pad_surface() {
    let mut f = Fixture::new();
    f.editor.pane_pad_surface = crate::state::PadSurface::Autoplay;
    f.editor.tool = Tool::Slide;
    for time in [2.0, 2.25, 3.0] { f.editor.place_star_step(&mut f.app.chart, time, 1); }
    let output = f.frame(Vec::new());
    text_position(&output, "Slide · 放置中");
    assert!(f.editor.slide_draft.is_some());
    f.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(f.editor.slide_draft.is_none());
}

#[test]
fn keyboard_star_confirms_head_delay_and_trajectory() {
    let mut f = Fixture::new();
    f.key(egui::Key::S, egui::Modifiers::NONE);
    f.key(egui::Key::Num1, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(
        f.app.chart.notes.last().unwrap().slide[0]
            .segments
            .is_empty()
    );
    f.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert!(
        f.editor.undo.is_empty(),
        "disabled inspector must not commit a Star draft"
    );
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
fn reopened_slide_shapes_survive_pad_style_append_backspace_and_undo() {
    use crate::app::types::SlideShape;

    for (token, shape) in [
        ("1>3", SlideShape::Right),
        ("1<3", SlideShape::Left),
        ("1^3", SlideShape::Caret),
        ("1p3", SlideShape::P),
        ("1q3", SlideShape::Q),
        ("1s5", SlideShape::S),
        ("1z5", SlideShape::Z),
        ("1pp3", SlideShape::PP),
        ("1qq3", SlideShape::QQ),
        ("1w5", SlideShape::Wifi),
    ] {
        let mut f = Fixture::new();
        f.app.chart = crate::app::maidata::from_maidata(
            &format!("&inote_1=(120){{4}}{token}[4:1],,E"), None,
        ).unwrap();
        let original = f.app.chart.clone();
        let id = original.notes[0].id;
        assert_eq!(original.notes[0].slide[0].segments[0].shape, shape);
        f.editor.select_only(id);
        f.editor.keyboard.active = false;
        f.key(egui::Key::E, egui::Modifiers::NONE);
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        f.key(egui::Key::E, egui::Modifiers::NONE);
        assert_eq!(f.editor.editing_slide, Some(id));

        f.key(egui::Key::Num7, egui::Modifiers::NONE);
        let appended = f.app.chart.clone();
        let segments = &appended.notes[0].slide[0].segments;
        assert_eq!(segments.len(), 2, "{token}");
        assert_eq!(segments[0], original.notes[0].slide[0].segments[0], "{token}");
        assert_eq!(segments[1].shape, SlideShape::Line);
        assert_eq!(segments[1].points[0].zone.to_id(), 7);

        f.key(egui::Key::Backspace, egui::Modifiers::NONE);
        assert_eq!(f.app.chart, original, "{token}");
        f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(f.app.chart, appended, "undo Backspace: {token}");
        f.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(f.app.chart, original, "undo append: {token}");
    }
}

#[test]
fn right_slide_draft_can_be_confirmed_reopened_and_have_its_endpoint_edited() {
    use crate::app::types::SlideShape;

    let mut f = Fixture::new();
    let before = f.app.chart.clone();
    f.editor.tool = Tool::Slide;
    for time in [2.0, 2.25, 3.0] { f.editor.place_star_step(&mut f.app.chart, time, 1); }
    f.key(egui::Key::Period, egui::Modifiers::NONE);
    f.key(egui::Key::Num3, egui::Modifiers::NONE);
    assert_eq!(f.app.chart, before, "unconfirmed paths stay outside the chart");
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    let placed = f.app.chart.clone();
    let note = placed.notes.last().unwrap();
    let id = note.id;
    assert_eq!(note.slide[0].segments[0].shape, SlideShape::Right);
    assert!(crate::core_chart::note_text(note, &placed.bpms).unwrap().contains("1>3"));

    f.editor.keyboard.active = true;
    f.key(egui::Key::E, egui::Modifiers::NONE);
    f.key(egui::Key::Num4, egui::Modifiers::NONE);
    assert!(f.editor.keyboard.transaction.is_some());
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    f.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(f.editor.editing_slide.is_none());
    let note = f.app.chart.notes.iter().find(|note| note.id == id).unwrap();
    assert_eq!(note.slide[0].segments.len(), 1);
    assert_eq!(note.slide[0].segments[0].shape, SlideShape::Right);
    assert_eq!(note.slide[0].segments[0].points[0].zone.to_id(), 4);
    assert!(crate::core_chart::note_text(note, &f.app.chart.bpms).unwrap().contains("1>4"));
    f.key(egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(f.app.chart, placed);
}

#[test]
fn delete_in_slide_edit_removes_the_active_trajectory_and_keeps_the_star() {
    use crate::app::types::{NoteType, Slide, SlidePoint, SlideSegment, SlideShape};
    use crate::app::types::zone::PadZone;

    let mut f = Fixture::new();
    let path = Slide {
        slide_start_delay: 0.0,
        slide_duration: 1.0,
        segments: vec![SlideSegment {
            shape: SlideShape::Line,
            points: vec![SlidePoint::from(PadZone::from(5))],
        }],
        slide_is_break: false,
        runtime_parts: 1,
    };
    let note = &mut f.app.chart.notes[0];
    note.note_type = NoteType::Slide;
    note.slide = vec![path.clone(), path];
    f.editor.select_only(note.id);
    f.editor.editing_slide = Some(note.id);
    f.editor.editing_slide_idx = 1;

    f.key(egui::Key::Delete, egui::Modifiers::COMMAND);

    let note = &f.app.chart.notes[0];
    assert_eq!(note.slide.len(), 1);
    assert_eq!(note.slide[0].segments[0].points[0].zone, PadZone::from(5));
    assert_eq!(f.editor.editing_slide, Some(note.id));
    assert_eq!(f.editor.editing_slide_idx, 0);
    assert_eq!(f.editor.undo.len(), 1);

    f.key(egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(f.app.chart.notes[0].note_type, NoteType::Tap);
    assert!(f.app.chart.notes[0].slide.is_empty());
    assert!(f.editor.editing_slide.is_none());
    assert_eq!(f.editor.undo.len(), 2);
}

#[test]
fn delete_in_slide_edit_is_not_consumed_by_the_global_note_shortcut() {
    use crate::app::types::{NoteType, Slide, SlidePoint, SlideSegment, SlideShape};
    use crate::app::types::zone::PadZone;

    let mut f = Fixture::new();
    let note = &mut f.app.chart.notes[0];
    note.note_type = NoteType::Slide;
    note.slide = vec![Slide {
        slide_start_delay: 0.0,
        slide_duration: 1.0,
        segments: vec![SlideSegment {
            shape: SlideShape::Line,
            points: vec![SlidePoint::from(PadZone::from(5))],
        }],
        slide_is_break: false,
        runtime_parts: 1,
    }];
    let id = note.id;
    f.editor.select_only(id);
    f.editor.editing_slide = Some(id);
    f.editor.editing_slide_idx = 0;

    f.key(egui::Key::Delete, egui::Modifiers::COMMAND);

    assert_eq!(f.app.chart.notes.len(), 1, "the note head must remain");
    assert_eq!(f.app.chart.notes[0].note_type, NoteType::Tap);
    assert!(f.app.chart.notes[0].slide.is_empty());
    assert!(f.editor.editing_slide.is_none());
    assert_eq!(f.editor.undo.len(), 1);
}

#[test]
fn delete_in_slide_edit_works_with_modifiers_and_keyboard_transactions() {
    use crate::app::types::{NoteType, Slide, SlidePoint, SlideSegment, SlideShape};
    use crate::app::types::zone::PadZone;

    for (modifiers, keyboard_transaction) in [
        (egui::Modifiers::NONE, false),
        (egui::Modifiers::COMMAND, false),
        (egui::Modifiers::CTRL, false),
        (egui::Modifiers::COMMAND, true),
    ] {
        let mut f = Fixture::new();
        let note = &mut f.app.chart.notes[0];
        note.note_type = NoteType::Slide;
        note.slide = vec![Slide {
            slide_start_delay: 0.0,
            slide_duration: 1.0,
            segments: vec![SlideSegment {
                shape: SlideShape::Line,
                points: vec![SlidePoint::from(PadZone::from(5))],
            }],
            slide_is_break: false,
            runtime_parts: 1,
        }];
        let id = note.id;
        f.editor.select_only(id);
        f.editor.editing_slide = Some(id);
        f.editor.editing_slide_idx = 0;

        if keyboard_transaction {
            f.editor.keyboard.active = true;
            f.key(egui::Key::Num6, egui::Modifiers::NONE);
            assert!(f.editor.keyboard.transaction.is_some());
        }

        f.key(egui::Key::Delete, modifiers);

        assert_eq!(f.app.chart.notes.len(), 1, "the star head must remain");
        assert_eq!(f.app.chart.notes[0].note_type, NoteType::Tap);
        assert!(f.app.chart.notes[0].slide.is_empty());
        assert!(f.editor.editing_slide.is_none());
        assert!(f.editor.keyboard.transaction.is_none());
        assert_eq!(f.editor.undo.len(), 1);
    }
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
    assert_eq!(
        f.app.chart.notes.len(),
        2,
        "confirmation must not fall through"
    );
    assert_eq!(f.editor.undo.len(), 1);
}

#[test]
fn keyboard_navigation_only_seeks_when_enabled_without_scrolling_a_visible_cursor() {
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
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
        assert_eq!(
            f.editor.view_start, view,
            "visible navigation must not scroll the viewport"
        );
        crate::runtime::follow_playhead(&mut f.editor, 20.0);
        assert_eq!(f.editor.view_start, view);
        let time =
            crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert_eq!(f.app.timeline_view_time, 2.0);
        let cursor = crate::keyboard::navigation::position(
            &f.editor,
            &f.app.chart,
            time,
            f.editor.keyboard.cursor,
        );
        assert!(
            f.editor.keyboard.rect.contains(cursor),
            "{layout:?}: {cursor:?}"
        );
        let lane_key = if layout == crate::state::EditorLayout::Vertical {
            egui::Key::ArrowRight
        } else {
            egui::Key::ArrowDown
        };
        f.key(lane_key, egui::Modifiers::NONE);
        assert_eq!(
            f.editor.view_start, view,
            "lane movement must not scroll time"
        );
        f.frame(Vec::new());
        assert_eq!(f.editor.view_start, view);
        f.editor.keyboard_seek_on_navigation = true;
        f.key(key, egui::Modifiers::NONE);
        assert_eq!(
            f.app.timeline_view_time,
            crate::app::types::measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms)
        );
    }
}

#[test]
fn keyboard_range_selection_confirms_without_placing_and_preserves_the_tool() {
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.tool = Tool::Tap;
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        note.id = 101;
        note.time = 2.0;
        note.lane = 2;
        f.app.chart.notes.push(note.clone());
        note.id = 102;
        note.time = 3.0;
        note.lane = 4;
        f.app.chart.notes.push(note);
        f.editor.select_only(102);
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        assert!(f.editor.keyboard.range.is_some());
        assert!(f.editor.keyboard.owns_input());
        f.key(egui::Key::Space, egui::Modifiers::NONE);
        assert_eq!(
            f.app.mode,
            Mode::Idle,
            "a range owns input until confirmed or cancelled"
        );
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(101));
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(f.editor.keyboard.range.is_none());
        assert_eq!(f.editor.selected, [original, 101].into_iter().collect());
        assert_eq!(f.editor.tool, Tool::Tap);
        assert_eq!(
            f.app.chart.notes.len(),
            3,
            "range confirmation must not place a Tap"
        );
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
        let (later, earlier) = if layout == EditorLayout::Vertical {
            (egui::Key::ArrowUp, egui::Key::ArrowDown)
        } else {
            (egui::Key::ArrowRight, egui::Key::ArrowLeft)
        };
        f.editor.keyboard.cursor.measure = secs_to_measure(6.0 + span - 0.1875, &f.app.chart.bpms);
        f.key(later, egui::Modifiers::NONE);
        assert_eq!(
            f.editor.view_start, 6.0,
            "do not scroll early near the far edge"
        );
        f.key(later, egui::Modifiers::NONE);
        let time = measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert!((f.editor.view_start - (time - span)).abs() < 1e-5);
        let position = crate::keyboard::navigation::position(
            &f.editor,
            &f.app.chart,
            time,
            f.editor.keyboard.cursor,
        );
        assert!(f.editor.keyboard.rect.expand(0.5).contains(position));
        let view = f.editor.view_start;
        f.frame(Vec::new());
        assert_eq!(
            f.editor.view_start, view,
            "an edge-aligned cursor must not drift next frame"
        );
        let bottom = if layout == EditorLayout::Vertical {
            crate::timeline::geometry::JUDGE_MARGIN / f.editor.pixels_per_second
        } else {
            0.0
        };
        f.editor.keyboard.cursor.measure =
            secs_to_measure(view - bottom + 0.1875, &f.app.chart.bpms);
        f.key(earlier, egui::Modifiers::NONE);
        assert_eq!(
            f.editor.view_start, view,
            "the strip below the judgment line is still visible"
        );
        f.key(earlier, egui::Modifiers::NONE);
        let time = measure_to_secs(f.editor.keyboard.cursor.measure, &f.app.chart.bpms);
        assert!((f.editor.view_start - (time + bottom)).abs() < 1e-5);
        let position = crate::keyboard::navigation::position(
            &f.editor,
            &f.app.chart,
            time,
            f.editor.keyboard.cursor,
        );
        assert!(f.editor.keyboard.rect.expand(0.5).contains(position));
        assert_eq!(f.app.timeline_view_time, 0.0);
    }
}

#[test]
fn keyboard_tab_keeps_the_view_still_for_an_already_visible_note() {
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        f.editor.pixels_per_second = 180.0;
        let mut note = f.app.chart.notes[0].clone();
        note.id = 101;
        note.time = 2.0;
        note.lane = 2;
        f.app.chart.notes.push(note);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.view_start, 0.0);
        assert_eq!(f.editor.keyboard.candidate, Some(101));
        assert_eq!(f.app.timeline_view_time, 0.0);
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
    for layout in [
        crate::state::EditorLayout::Vertical,
        crate::state::EditorLayout::Horizontal,
    ] {
        let mut f = Fixture::new();
        f.editor.layout = layout;
        let original = f.app.chart.notes[0].id;
        let mut note = f.app.chart.notes[0].clone();
        for (id, time, lane) in [(101, 20.0, 3), (102, 30.0, 8), (103, 1.5, 7)] {
            note.id = id;
            note.time = time;
            note.lane = lane;
            f.app.chart.notes.push(note.clone());
        }
        f.editor.select_only(102);
        let additive = egui::Modifiers {
            shift: true,
            ..egui::Modifiers::COMMAND
        };
        f.key(egui::Key::Enter, additive);
        f.click(f.ui.layout.timeline.center());
        assert!(
            f.editor.keyboard.range.is_some(),
            "a pointer click must not steal a keyboard range"
        );
        assert!(f.editor.keyboard.active);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(f.editor.keyboard.candidate, Some(101));
        assert!(f.editor.view_start > 0.0);
        assert_eq!(
            f.editor.selected,
            [102].into_iter().collect(),
            "preview must not commit selection"
        );
        f.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(
            f.editor.selected,
            [original, 101, 102].into_iter().collect()
        );
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::Escape, egui::Modifiers::NONE);
        assert!(f.editor.keyboard.range.is_none());
        assert_eq!(
            f.editor.selected,
            [original, 101, 102].into_iter().collect()
        );
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        f.key(egui::Key::Home, egui::Modifiers::NONE);
        f.key(egui::Key::Enter, egui::Modifiers::COMMAND);
        assert_eq!(
            f.editor.selected,
            [102].into_iter().collect(),
            "a new normal range replaces the selection"
        );
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
            note.id = id;
            note.time = time;
            f.app.chart.notes.push(note.clone());
        }
        f.key(egui::Key::F2, egui::Modifiers::NONE);
        f.key(egui::Key::Tab, egui::Modifiers::NONE);
        f.key(egui::Key::F2, egui::Modifiers::SHIFT);
        assert_eq!(f.editor.selected, [original, 101].into_iter().collect());
        f.key(egui::Key::Num8, egui::Modifiers::NONE);
        f.key(key, egui::Modifiers::COMMAND);
        assert_eq!(
            f.app.chart.notes.iter().map(|n| n.id).collect::<Vec<_>>(),
            vec![102]
        );
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
    f.editor
        .keyboard
        .dialog
        .as_mut()
        .unwrap()
        .note
        .as_mut()
        .unwrap()
        .is_break = true;
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
    assert!(
        f.ctx.wants_keyboard_input(),
        "property value must accept typing"
    );
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
    assert!(
        f.ctx.wants_keyboard_input(),
        "F5 must focus the first property field"
    );
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
