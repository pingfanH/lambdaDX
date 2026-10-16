//! LambdaDX editor — binary entry point.
//!
//! The editor is the standalone pad preview in a resizable window, with an egui
//! chrome (toolbar, tools, inspector, progress) layered over a macroquad-rendered
//! pad and timeline. All editing logic lives in the sibling modules:
//!
//! * [`state`] / [`edit_ops`] — document + selection state and mutations.
//! * [`runtime`] — loading charts and driving the shared player.
//! * [`ui`] — the egui chrome (one submodule per region).
//! * [`timeline`] — timeline geometry, input and macroquad renderers.
//! * [`io`] — saving and choosing charts.

// The editor re-includes the crate's shared `app` / `player` / `core` trees, so
// (like the sibling bins in `src/`) dead code and unused items from those
// modules are expected per-binary.
#![allow(dead_code, unused_variables, unused_imports)]

// The gameplay/player modules live in the crate's `src/` tree and are included
// here by path so the editor is a self-contained binary under `editor/src/`.
#[path = "../../src/app/mod.rs"]
mod app;
#[path = "../../src/core/mod.rs"]
mod core;
#[path = "../../src/player/mod.rs"]
mod player;

mod audio;
mod appearance;
mod core_chart;
mod edit_ops;
mod gesture;
mod io;
mod simai_editor;
mod keyboard;
mod runtime;
mod state;
mod theme;
mod timeline;
mod templates;
mod autoplay_editor;
mod ui;

use egui_macroquad::egui;
use macroquad::Window;
use macroquad::prelude::*;

use app::pad_svg;
use player::state::PadPreviewState;
use state::{EditorState, PendingAction, Workspace};
use ui::EditorUi;

fn editor_window_conf() -> Conf {
    let mut conf = app::window_conf();
    conf.window_title = "LambdaDX Editor".to_string();
    // Leave room for the main chart and the docked preview/inspector.
    conf.window_width = 1600;
    conf.window_height = 900;
    conf.window_resizable = true;
    if let Some((width, height)) = std::env::var("MAI2_EDITOR_WINDOW").ok().and_then(|size| {
        let (width, height) = size.split_once('x')?;
        Some((width.parse::<i32>().ok()?, height.parse::<i32>().ok()?))
    }).filter(|(w, h)| *w >= 640 && *h >= 480) {
        conf.window_width = width;
        conf.window_height = height;
    }
    conf
}

fn main() {
    Window::from_config(editor_window_conf(), run());
}

async fn run() {
    let t_timing = get_time();
    let timing = std::env::var("MAI2_EDITOR_TIMING").is_ok();
    let mark = |label: &str| {
        if timing {
            eprintln!("[timing] {label}: {:.2}s", get_time() - t_timing);
        }
    };

    set_pc_assets_folder(&app::platform::asset_dir().to_string_lossy());
    mark("assets folder");
    let mut file_dialogs = io::dialogs::Dialogs::default();
    file_dialogs.warm_up();
    // Let the main queue initialize native panels before the editing UI is live.
    clear_background(theme::to_mq(theme::BG_WINDOW));
    next_frame().await;

    // Load a chart *folder* (Simai source + track) so the judgment engine and
    // autoplay modes work. Override with `MAI2_EDITOR_CHART`.
    let chart_dir = std::env::var("MAI2_EDITOR_CHART")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| app::platform::asset_dir().join("charts/jack_ripper"));

    let bundle = runtime::load_chart_bundle(&chart_dir, None);
    let source = bundle.source.clone();
    mark("chart load");
    let mut app = runtime::initialize_player(
        PadPreviewState::new(bundle.chart, bundle.audio_name, bundle.audio_pcm),
        bundle.simai,
        bundle.level,
    );
    let audio_ms = get_time();
    app.params = app::params::load();
    app::params::set(app.params.clone());
    app.note_speed = app.params.note_speed_default;
    app.touch_speed = app.params.touch_speed_default;
    app.slide_fade_in = app.params.slide_fade_in;
    app.set_play_speed(app.params.play_speed_default);
    mark("state new + params");
    let _ = audio_ms;

    player::autoplay::rebuild(&mut app);
    if let Ok(def) = pad_svg::PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")) {
        app.pad_svg = Some(def);
    }
    mark("pad svg");
    player::render::load_note_textures(&mut app).await;
    mark("note textures");
    if let Ok(m) = app::ui::load_mask_material() {
        app.mask_material = Some(m);
    }
    if let Ok(m) = app::ui::load_break_shine_material() {
        app.break_shine_material = Some(m);
    }
    mark("materials");
    player::sfx::load_pad_sfx(&mut app).await;
    mark("sfx");
    if timing {
        std::process::exit(0);
    }

    let mut ui = EditorUi::new();
    ui.appearance.load();
    let mut editor = EditorState::default();
    // Trackpad pinch-to-zoom (macOS); a no-op elsewhere.
    gesture::install();
    if std::env::var("MAI2_EDITOR_LAYOUT")
        .map(|v| v.eq_ignore_ascii_case("horizontal"))
        .unwrap_or(false)
    {
        editor.layout = state::EditorLayout::Horizontal;
    }
    editor.chart_dir = chart_dir.to_string_lossy().to_string();
    editor.source = source;
    editor.level_slots = app::chart::read_simai_source(&chart_dir)
        .map(|t| app::maidata::inote_keys(&t))
        .unwrap_or_default();
    editor.level_index = editor.level_slots.len().saturating_sub(1);
    if std::env::var("MAI2_EDITOR_WORKSPACE").is_ok_and(|v| v.eq_ignore_ascii_case("autoplay")) {
        editor.workspace = Workspace::Autoplay;
        editor.autoplay_mode = 2;
        runtime::apply_preview_mode(&mut app, 2);
    }
    if std::env::var_os("MAI2_EDITOR_TEMPLATES").is_some() { editor.template_panel = true; }
    if let Ok(id) = std::env::var("MAI2_EDITOR_TEMPLATE") {
        templates::apply(&mut editor, &mut app, templates::Action::EnterDefinition(id));
    }
    // Screenshot overrides do not modify saved preferences.
    if std::env::var_os("MAI2_EDITOR_JUDGE_WINDOWS").is_some() {
        app.params.show_judge_windows = true;
        app::params::set(app.params.clone());
    }
    app.show_params = std::env::var_os("MAI2_EDITOR_SHOW_SETTINGS").is_some();
    if let Some(time) = std::env::var("MAI2_EDITOR_TIME").ok()
        .and_then(|value| value.parse::<f32>().ok()).filter(|time| time.is_finite()) {
        app.seek_to(time.max(0.0));
        runtime::follow_playhead(&mut editor, time.max(0.0));
    }

    let mut waveform = audio::WaveformPyramid::default();
    let shot_path = std::env::var("MAI2_EDITOR_SHOT").ok();
    let shot_at = std::env::var("MAI2_EDITOR_SHOT_AT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.6);
    let start_time = get_time();
    let mut chart_sync = runtime::ChartSync::new(&mut app.chart);
    let mut auto_save = io::autosave::AutoSave::default();
    let mut cover_art = appearance::CoverArt::default();

    // `MAI2_EDITOR_FPS=1` prints a once-per-second frame-time summary to stderr
    // (a lightweight perf read-out; no-op otherwise).
    let fps_dbg = std::env::var("MAI2_EDITOR_FPS").is_ok();
    let mut fps_last = get_time();
    let mut fps_acc = 0.0f32;
    let mut fps_n = 0u32;
    let mut fps_worst = 0.0f32;

    loop {
        // OS close requests are deferred while there are unsaved changes: with a
        // dirty document we cancel the quit and raise a confirm action instead
        // (resolved by the Save / Discard / Cancel dialog).
        if editor.has_unsaved_changes() || editor.file_dialog_active {
            macroquad::input::prevent_quit();
        }
        if macroquad::input::is_quit_requested() && !editor.file_dialog_active {
            timeline::transform::finish(&mut editor, &mut app.chart, false);
            editor.pending_action = Some(PendingAction::Quit);
        }

        app::anim::tick();
        ui.appearance.update();

        // Preview-mode shortcuts are routed through egui so text fields and
        // slide shape keys keep ownership of their keyboard input.
        if editor.mode_dirty {
            editor.mode_dirty = false;
            runtime::apply_preview_mode(&mut app, editor.autoplay_mode);
        }

        runtime::tick_loop(&editor, &mut app);
        let current_time = player_time(&app);
        runtime::follow_playhead(&mut editor, current_time);

        let events = player::input::collect_pointer_events();
        let pointer = player::input::pointer::ui_pointer(&events);
        app::egui_bridge::pointer(pointer.pos, pointer.down, pointer.pressed, pointer.released);

        let mut egui_screen_w = 1.0;
        app::egui_bridge::ui(|ctx| {
            if waveform.levels.is_empty() && app.audio_wav_pcm.is_some() {
                waveform = audio::WaveformPyramid::build(&app);
            }
            editor.glyph_to_points = ctx.screen_rect().width() / screen_width().max(1.0);
            ui.draw(ctx, &mut editor, &mut app, current_time, &waveform, mouse_wheel().1);
            let preview_keys_enabled = editor.workspace == Workspace::Chart && !ui.input_blocked && !ctx.wants_keyboard_input() && !ctx.input(|i| i.modifiers.any())
                && !editor.is_slide_edit() && editor.selection_transform.is_none()
                && !editor.keyboard.owns_input() && !editor.keyboard.active
                && ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| ui.layout.preview.contains(p)));
            player::input::keyboard::handle_lane_input_when(&mut app, preview_keys_enabled);
            ctx.request_repaint();
            egui_screen_w = ctx.screen_rect().width();
        });
        let egui_wants_pointer = app::egui_bridge::wants_pointer();
        ui.finish_layout();

        if let Some(action) = editor.template_action.take() {
            templates::apply(&mut editor, &mut app, action);
        }
        apply_io_requests(&mut editor, &mut app, &mut file_dialogs, &mut ui.appearance);
        ui.input_blocked |= editor.file_dialog_active;
        if let Some(error) = cover_art.update(editor.source.as_ref(), &mut app) {
            editor.status = Some(error);
        }
        let current_time = player_time(&app);

        // egui points → macroquad logical units (normally 1.0; kept explicit for
        // HiDPI displays where the two DPI scales can differ).
        let to_mq = if egui_screen_w > 1.0 {
            screen_width() / egui_screen_w
        } else {
            1.0
        };
        let (fb_w, _fb_h) = macroquad::miniquad::window::screen_size();
        let s_phys = if egui_screen_w > 1.0 {
            fb_w / egui_screen_w
        } else {
            1.0
        };

        // A timeline / overview scrub seeks the shared player clock.
        let pending_seek = editor.pending_seek.take();
        if let Some(t) = pending_seek {
            // Scene changes must detach old bindings before seeking the new chart.
            if let Err(error) = chart_sync.update(&mut app, true) {
                editor.status = Some(format!("core update: {error}"));
            }
            let t = t.max(0.0);
            if editor.scrubbing {
                app.scrub_to(t);
            } else {
                app.seek_to(t);
            }
        }
        if app.scrubbing && !editor.scrubbing && pending_seek.is_none() {
            app.seek_to(app.timeline_view_time);
        }

        app::viewport::set(0.0, 0.0, 1.0);
        let mut pad = ui.layout.pad;
        pad.cx *= to_mq;
        pad.cy *= to_mq;
        pad.outer_r *= to_mq;
        pad.outer_r /= app::params::pad_circle_scale().max(1.0);
        let preview = ui.layout.preview;
        let pad_visible = preview.width() > 1.0 && preview.height() > 1.0;
        let pad_input = pad_visible && editor.workspace == Workspace::Chart && !ui.input_blocked
            && !egui_wants_pointer
            && !editor.keyboard.owns_input() && !editor.keyboard.suppress_pointer;
        let pad_events: Vec<_> = events
            .into_iter()
            .filter(|e| {
                let p = e.position;
                let release = matches!(e.phase, TouchPhase::Ended | TouchPhase::Cancelled)
                    && app.active_pointer_zones.contains_key(&e.id);
                release || (pad_input && preview.contains(egui_macroquad::egui::pos2(p.x / to_mq, p.y / to_mq))
                    && (p.x - pad.cx).hypot(p.y - pad.cy) <= pad.outer_r)
            })
            .collect();
        let scale = ui::layout::preview_note_scale(
            pad.outer_r,
            player::render::ui_scale(&app),
        );
        app::params::set_pad_scale(scale * app::params::pad_zoom());
        if !pad_events.is_empty() {
            player::input::handle_touch_controls(&mut app, pad, &pad_events);
        }

        // Slide-edit mode: a pad press on an A-zone (1-8) completes a pending
        // shape key, otherwise the pressed zone is appended as a waypoint on the
        // editing slide's last segment. Chrome-owned presses never reach the pad.
        if pad_input && editor.is_slide_edit() {
            let zone = {
                let svg = app.pad_svg.as_ref();
                pad_events
                    .iter()
                    .filter(|e| e.phase == macroquad::prelude::TouchPhase::Started)
                    .find_map(|e| {
                        svg.and_then(|s| {
                            s.hit_test(e.position, &pad).or_else(|| {
                                s.hit_test_range(e.position, &pad, 0.0).into_iter().next()
                            })
                        })
                    })
            };
            if let Some(z) = zone {
                let id = z.to_id();
                if let (Some(shape), true) = (editor.pending_shape, (1..=8).contains(&id)) {
                    editor.pending_shape = None;
                    editor.apply_slide_shape(&mut app.chart, shape, id);
                } else {
                    editor.append_slide_waypoint(&mut app.chart, z);
                }
            }
        }

        // Same audio / cue / autoplay / judgment pipeline as the preview.
        app.scrubbing = editor.scrubbing;
        // Star (slide) edit mode shows only the slide being edited, with its
        // whole trail (no time culling); everything else is hidden.
        app.edit_solo_slide = (editor.workspace == Workspace::Chart).then_some(editor.editing_slide)
            .flatten().map(|id| (id, editor.editing_slide_idx));
        let gesture_active = editor.dragging.is_some() || editor.selection_transform.is_some()
            || editor.handle_grab.is_some() || editor.keyboard.transaction.is_some()
            || editor.autoplay_editor.dragging() || ui.templates.dragging();
        let document_idle = !gesture_active && !ui.input_blocked && editor.pending_action.is_none()
            && editor.pointer_press.is_none() && editor.star_placement.is_none()
            && editor.hold_placement.is_none() && !editor.transform_pointer_down
            && editor.keyboard.dialog.is_none() && editor.template_action.is_none();
        simai_editor::tick(&mut editor, &mut app.chart, document_idle);
        auto_save.tick(&mut editor, &app.chart, app.document_revision(), document_idle);
        let chart_current = match chart_sync.update(&mut app, gesture_active) {
            Ok(current) => current,
            Err(error) => {
                editor.status = Some(format!("core update: {error}"));
                false
            }
        };
        runtime::tick_player(&mut app, chart_current).await;
        let current_time = player_time(&app);
        runtime::follow_playhead(&mut editor, current_time);

        clear_background(theme::to_mq(theme::BG_WINDOW));
        ui.appearance.draw();

        if pad_visible {
            draw_rectangle(
                preview.left() * to_mq,
                preview.top() * to_mq,
                preview.width() * to_mq,
                preview.height() * to_mq,
                theme::to_mq(theme::surface(theme::BG_OVERVIEW)),
            );
        }

        // Pad preview, scissored to its region.
        let preview = ui.layout.preview;
        if preview.width() > 1.0 && preview.height() > 1.0 {
            let scissor = to_scissor(preview, s_phys, fb_w);
            unsafe { get_internal_gl() }.quad_gl.scissor(Some(scissor));
            player::render::pad::draw_pad_sim_transparent(
                &app, pad.cx, pad.cy, pad.outer_r, scale,
                |pad| ui.appearance.draw_pad_surround(
                    pad, theme::to_mq(theme::surface(theme::BG_OVERVIEW)),
                ),
            );
            unsafe { get_internal_gl() }.quad_gl.scissor(None);
        }

        // Timeline (macroquad, behind the transparent central egui panel).
        let timeline = ui.layout.timeline;
        if timeline.width() > 1.0 && timeline.height() > 1.0 {
            let scissor = to_scissor(timeline, s_phys, fb_w);
            unsafe { get_internal_gl() }.quad_gl.scissor(Some(scissor));
            let rect = Rect::new(
                timeline.left() * to_mq,
                timeline.top() * to_mq,
                timeline.width() * to_mq,
                timeline.height() * to_mq,
            );
            timeline::draw(
                editor.layout,
                &app,
                &editor,
                app.playback_chart(),
                rect,
                current_time,
                &waveform,
                // Geometry scale (egui points → macroquad logical). The timeline
                // renderer uses this for positions so it matches the egui-space
                // input hit-testing; it keeps content (glyph) sizes at logical
                // 1.0 internally so notes don't shrink on HiDPI.
                to_mq,
            );
            unsafe { get_internal_gl() }.quad_gl.scissor(None);
        }

        app.tick_feedback();
        app::egui_bridge::draw();
        app.finalize_playback_start();

        if fps_dbg {
            let now = get_time();
            let dt = (now - fps_last) as f32;
            fps_last = now;
            fps_acc += dt;
            fps_n += 1;
            if dt > fps_worst {
                fps_worst = dt;
            }
            if fps_acc >= 1.0 {
                eprintln!(
                    "[fps] avg={:.1} worst={:.1}ms ({:.0} fps)",
                    fps_acc / fps_n as f32 * 1000.0,
                    fps_worst * 1000.0,
                    fps_n as f32 / fps_acc,
                );
                fps_acc = 0.0;
                fps_n = 0;
                fps_worst = 0.0;
            }
        }

        if let Some(path) = &shot_path {
            if get_time() - start_time > shot_at {
                get_screen_data().export_png(path);
                println!("editor: wrote {path}");
                std::process::exit(0);
            }
        }
        next_frame().await;
        if let Err(error) = chart_sync.schedule(&mut app, gesture_active) {
            editor.status = Some(format!("core update: {error}"));
        }
    }
}

/// The player clock shown on the timeline: the scrub time when idle, else the
/// song time.
fn player_time(app: &PadPreviewState) -> f32 {
    match app.mode {
        app::types::Mode::Idle => app.timeline_view_time,
        _ => app.song_time(),
    }
}

/// Convert an egui-points rect to a macroquad scissor (raw framebuffer pixels).
fn to_scissor(rect: egui::Rect, s_phys: f32, fb_w: f32) -> (i32, i32, i32, i32) {
    let left = (rect.left() * s_phys).floor().clamp(0.0, fb_w) as i32;
    let top = (rect.top() * s_phys).floor().clamp(0.0, fb_w) as i32;
    let right = (rect.right() * s_phys).ceil().clamp(0.0, fb_w) as i32;
    let bottom = (rect.bottom() * s_phys).ceil().clamp(0.0, fb_w) as i32;
    (left, top, (right - left).max(0), (bottom - top).max(0))
}

/// Apply editor-raised IO requests (native picker / reload / export) before the
/// next frame draws. Load/open/quit are gated on the unsaved-changes prompt:
/// when the document is dirty they are deferred to `pending_action` and run
/// later, once the confirm dialog resolves (Save / Discard).
fn apply_io_requests(editor: &mut EditorState, app: &mut PadPreviewState, dialogs: &mut io::dialogs::Dialogs, appearance: &mut appearance::Appearance) {
    use io::dialogs::Request;
    if let Some((request, result)) = dialogs.poll() {
        match result {
            Ok(Some(path)) => match request {
                Request::Open => load_editor_chart(editor, app, &path, None),
                Request::Save { save_as, .. } => io::finish_export(editor, &app.chart, save_as, &path),
                Request::Background => appearance.request_image(path.to_string_lossy().into_owned()),
            },
            Ok(None) => editor.status = Some("File selection cancelled".into()),
            Err(error) => editor.status = Some(format!("File dialog failed: {error}")),
        }
    }
    editor.file_dialog_active = dialogs.busy();
    if editor.file_dialog_active { return; }

    // Resolve a confirmed deferred action first (Save/Discard was chosen).
    if editor.pending_confirmed {
        editor.pending_confirmed = false;
        match editor.pending_action.take() {
            Some(PendingAction::Reload) => { reload_chart(editor, app); }
            Some(PendingAction::Open) => open_chart(editor, dialogs),
            Some(PendingAction::Quit) => quit_editor(),
            None => {}
        }
    }
    if dialogs.busy() { return; }

    if editor.open_dialog {
        editor.open_dialog = false;
        if !editor.defer_if_dirty(PendingAction::Open) {
            open_chart(editor, dialogs);
        }
        return;
    }
    if editor.export_request {
        editor.export_request = false;
        export_chart(editor, app, dialogs, false);
        return;
    }
    if editor.save_as_request {
        editor.save_as_request = false;
        export_chart(editor, app, dialogs, true);
        return;
    }
    if std::mem::take(&mut appearance.browse_requested) {
        start_file_dialog(editor, dialogs, Request::Background, Some(appearance.image_path()));
        return;
    }
    if editor.reload_request {
        editor.reload_request = false;
        if !editor.defer_if_dirty(PendingAction::Reload) {
            reload_chart(editor, app);
        }
    }
}

fn start_file_dialog(editor: &mut EditorState, dialogs: &mut io::dialogs::Dialogs, request: io::dialogs::Request, hint: Option<std::path::PathBuf>) {
    let hint = hint.or_else(|| editor.source.as_ref().map(|source| source.path.clone()))
        .or_else(|| Some(std::path::PathBuf::from(&editor.chart_dir)));
    match dialogs.start(request, hint) {
        Ok(()) => {
            editor.file_dialog_active = true;
            editor.status = Some("Waiting for file selection...".into());
        }
        Err(error) => editor.status = Some(format!("File dialog failed: {error}")),
    }
}

/// The caller has already resolved unsaved changes. Present asynchronously and
/// install the result on the frame thread after the sheet completes.
fn open_chart(editor: &mut EditorState, dialogs: &mut io::dialogs::Dialogs) {
    start_file_dialog(editor, dialogs, io::dialogs::Request::Open, None);
}

fn export_chart(editor: &mut EditorState, app: &mut PadPreviewState, dialogs: &mut io::dialogs::Dialogs, save_as: bool) {
    match io::prepare_export(editor, &mut app.chart, save_as) {
        Ok(request) => start_file_dialog(editor, dialogs, request, None),
        Err(error) => editor.status = Some(error),
    }
}

/// (Re)load the chart currently named by [`EditorState::chart_dir`]. A successful
/// load clears the dirty flag (the on-disk chart now matches the buffer).
fn reload_chart(editor: &mut EditorState, app: &mut PadPreviewState) {
    let dir = std::path::PathBuf::from(&editor.chart_dir);
    let want = editor.level_slots.get(editor.level_index).copied();
    load_editor_chart(editor, app, &dir, want);
}

fn load_editor_chart(editor: &mut EditorState, app: &mut PadPreviewState, path: &std::path::Path, want: Option<u32>) {
    let source = match runtime::load_chart_into(app, path, want) {
        Ok(source) => source,
        Err(error) => {
            editor.cancel_file_action();
            editor.status = Some(format!("Load failed: {error}"));
            return;
        }
    };
    editor.chart_dir = path.to_string_lossy().into_owned();
    editor.source = Some(source.clone());
    editor.level_slots = app::chart::read_simai_source(path)
        .map(|t| app::maidata::inote_keys(&t))
        .unwrap_or_default();
    editor.level_index = source.level.and_then(|level| editor.level_slots.iter().position(|slot| *slot == level)).unwrap_or(0);
    editor.undo.clear();
    editor.redo.clear();
    editor.selected.clear();
    editor.keyboard.reset_document();
    editor.selection_transform = None;
    editor.last_edit = None;
    editor.placement_sequence.clear();
    editor.transform_pointer_down = false;
    editor.star_placement = None;
    editor.hold_placement = None;
    editor.pointer_press = None;
    editor.timeline_pan = None;
    editor.dragging = None;
    editor.handle_grab = None;
    editor.marquee_start = None;
    editor.editing_slide = None;
    editor.pending_shape = None;
    editor.template_session = None;
    editor.template_action = None;
    editor.selected_template = None;
    editor.selected_instance = None;
    editor.autoplay_editor = Default::default();
    editor.simai_editor.reset();
    editor.view_start = 0.0;
    editor.dirty = false;
    editor.status = Some(format!("Loaded {}", editor.chart_dir));
}

/// Exit the editor (used by the confirm dialog / a clean quit request).
fn quit_editor() -> ! {
    std::process::exit(0);
}
