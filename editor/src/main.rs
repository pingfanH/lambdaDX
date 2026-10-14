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
mod core_chart;
mod edit_ops;
mod gesture;
mod io;
mod runtime;
mod state;
mod theme;
mod timeline;
mod ui;

use egui_macroquad::egui;
use macroquad::Window;
use macroquad::prelude::*;

use app::pad_svg;
use player::state::PadPreviewState;
use state::{EditorState, PendingAction};
use ui::EditorUi;

fn editor_window_conf() -> Conf {
    let mut conf = app::window_conf();
    conf.window_title = "LambdaDX Editor".to_string();
    // Wide default so the design's ~1:1 timeline/panel split has room. The
    // layout is fully responsive, so any window size stays usable.
    conf.window_width = 1600;
    conf.window_height = 900;
    conf.window_resizable = true;
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

    // Load a chart *folder* (Simai source + track) so the judgment engine and
    // autoplay modes work. Override with `MAI2_EDITOR_CHART`.
    let chart_dir = std::env::var("MAI2_EDITOR_CHART")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| app::platform::asset_dir().join("charts/jack_ripper"));

    let bundle = runtime::load_chart_bundle(&chart_dir, None);
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
    editor.level_slots = app::chart::read_simai_source(&chart_dir)
        .map(|t| app::maidata::inote_keys(&t))
        .unwrap_or_default();
    editor.level_index = editor.level_slots.len().saturating_sub(1);

    let mut waveform = audio::WaveformPyramid::default();
    let shot_path = std::env::var("MAI2_EDITOR_SHOT").ok();
    let shot_at = std::env::var("MAI2_EDITOR_SHOT_AT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.6);
    let start_time = get_time();
    let mut chart_sync = runtime::ChartSync::new(&mut app.chart);

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
        if editor.dirty {
            macroquad::input::prevent_quit();
        }
        if macroquad::input::is_quit_requested() {
            timeline::transform::finish(&mut editor, &mut app.chart, false);
            editor.pending_action = Some(PendingAction::Quit);
        }

        app::anim::tick();

        // Preview-mode shortcuts are routed through egui so text fields and
        // slide shape keys keep ownership of their keyboard input.
        if editor.mode_dirty {
            editor.mode_dirty = false;
            runtime::apply_preview_mode(&mut app, editor.autoplay_mode);
        }

        let current_time = player_time(&app);
        if matches!(
            app.mode,
            app::types::Mode::Playing | app::types::Mode::Recording
        ) {
            runtime::follow_playhead(&mut editor, current_time);
        }

        let events = player::input::collect_pointer_events();
        let pointer = player::input::pointer::ui_pointer(&events);
        app::egui_bridge::pointer(pointer.pos, pointer.down, pointer.pressed, pointer.released);

        let mut egui_screen_w = 1.0;
        app::egui_bridge::ui(|ctx| {
            if waveform.levels.is_empty() && app.audio_wav_pcm.is_some() {
                waveform = audio::WaveformPyramid::build(&app);
            }
            ui.draw(ctx, &mut editor, &mut app, current_time, &waveform);
            let preview_keys_enabled = !ctx.wants_keyboard_input() && !ctx.input(|i| i.modifiers.any())
                && !editor.is_slide_edit() && editor.selection_transform.is_none()
                && ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| ui.layout.preview.contains(p)));
            player::input::keyboard::handle_lane_input_when(&mut app, preview_keys_enabled);
            player::params_panel::draw_embedded(ctx, &mut app);
            ctx.request_repaint();
            egui_screen_w = ctx.screen_rect().width();
        });
        let egui_wants_pointer = app::egui_bridge::wants_pointer();
        ui.finish_layout();

        apply_io_requests(&mut editor, &mut app);
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
        let pad_events: Vec<_> = events
            .into_iter()
            .filter(|e| {
                let p = e.position;
                (p.x - pad.cx).hypot(p.y - pad.cy) <= pad.outer_r
                    || app.active_pointer_zones.contains_key(&e.id)
            })
            .collect();
        let scale = (pad.outer_r / (ui::layout::DESIGN_PAD_REFERENCE)).max(0.1);
        app.ui_scale_override = Some(scale);
        app::params::set_pad_scale(scale * app::params::pad_zoom());
        if !egui_wants_pointer {
            player::input::handle_touch_controls(&mut app, pad, &pad_events);
        }

        // Slide-edit mode: a pad press on an A-zone (1-8) completes a pending
        // shape key, otherwise the pressed zone is appended as a waypoint on the
        // editing slide's last segment. Not gated on `egui_wants_pointer`: while
        // editing the pad is the interaction surface, and the sidebar merely
        // hovers it (egui reports `wants_pointer_input` for any covered area).
        if editor.is_slide_edit() {
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
        app.edit_solo_slide = editor.editing_slide.map(|id| (id, editor.editing_slide_idx));
        let chart_current = match chart_sync.update(&mut app,
            editor.dragging.is_some() || editor.selection_transform.is_some() || editor.handle_grab.is_some()) {
            Ok(current) => current,
            Err(error) => {
                editor.status = Some(format!("core update: {error}"));
                false
            }
        };
        runtime::tick_player(&mut app, chart_current).await;
        let current_time = player_time(&app);

        clear_background(theme::to_mq(theme::BG_WINDOW));

        // Sidebar background (the egui sidebar frame is transparent so the pad
        // macroquad layer can show through).
        let sidebar = ui.layout.sidebar;
        if sidebar.width() > 1.0 && sidebar.height() > 1.0 {
            draw_rectangle(
                sidebar.left() * to_mq,
                sidebar.top() * to_mq,
                sidebar.width() * to_mq,
                sidebar.height() * to_mq,
                theme::to_mq(theme::BG_PANEL),
            );
        }

        // Pad preview, scissored to its region.
        let preview = ui.layout.preview;
        if preview.width() > 1.0 && preview.height() > 1.0 {
            let scissor = to_scissor(preview, s_phys, fb_w);
            unsafe { get_internal_gl() }.quad_gl.scissor(Some(scissor));
            player::render::pad::draw_pad_sim(&app, pad.cx, pad.cy, pad.outer_r, scale);
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
                &app.chart,
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
fn apply_io_requests(editor: &mut EditorState, app: &mut PadPreviewState) {
    // Resolve a confirmed deferred action first (Save/Discard was chosen).
    if editor.pending_confirmed {
        editor.pending_confirmed = false;
        match editor.pending_action.take() {
            Some(PendingAction::Reload) => reload_chart(editor, app),
            Some(PendingAction::Open) => open_chart(editor, app),
            Some(PendingAction::Quit) => quit_editor(),
            None => {}
        }
    }

    if editor.open_dialog {
        editor.open_dialog = false;
        if !editor.defer_if_dirty(PendingAction::Open) {
            open_chart(editor, app);
        }
    }
    if editor.export_request {
        editor.export_request = false;
        editor.status = Some("Simai export not implemented yet".into());
    }
    if editor.reload_request {
        editor.reload_request = false;
        if !editor.defer_if_dirty(PendingAction::Reload) {
            reload_chart(editor, app);
        }
    }
}

/// Native folder picker, then load the chosen folder (no dirty guard — the
/// caller has already resolved any unsaved changes).
fn open_chart(editor: &mut EditorState, app: &mut PadPreviewState) {
    if let Some(path) = io::pick_chart_path() {
        editor.chart_dir = path;
        reload_chart(editor, app);
    }
}

/// (Re)load the chart currently named by [`EditorState::chart_dir`]. A successful
/// load clears the dirty flag (the on-disk chart now matches the buffer).
fn reload_chart(editor: &mut EditorState, app: &mut PadPreviewState) {
    let dir = std::path::PathBuf::from(&editor.chart_dir);
    let want = editor.level_slots.get(editor.level_index).copied();
    runtime::load_chart_into(app, &dir, want);
    editor.level_slots = app::chart::read_simai_source(&dir)
        .map(|t| app::maidata::inote_keys(&t))
        .unwrap_or_default();
    if editor.level_index >= editor.level_slots.len() {
        editor.level_index = editor.level_slots.len().saturating_sub(1);
    }
    editor.selected.clear();
    editor.selection_transform = None;
    editor.last_edit = None;
    editor.placement_sequence.clear();
    editor.transform_pointer_down = false;
    editor.star_placement = None;
    editor.pointer_press = None;
    editor.dragging = None;
    editor.handle_grab = None;
    editor.marquee_start = None;
    editor.editing_slide = None;
    editor.pending_shape = None;
    editor.view_start = 0.0;
    editor.dirty = false;
    editor.status = Some(format!("Loaded {}", editor.chart_dir));
}

/// Exit the editor (used by the confirm dialog / a clean quit request).
fn quit_editor() -> ! {
    std::process::exit(0);
}
