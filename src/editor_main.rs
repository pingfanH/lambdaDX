//! LambdaDX editor: egui layout and text with the shared live pad renderer.

#![allow(dead_code, unused_variables, unused_imports)]

#[path = "app/mod.rs"]
mod app;
#[path = "core/mod.rs"]
mod core;
#[path = "../editor/audio.rs"]
mod editor_audio;
#[path = "../editor/egui/mod.rs"]
mod editor_egui;
#[path = "../editor/state.rs"]
mod editor_state;
#[path = "../editor/timeline.rs"]
mod editor_timeline;
#[path = "player/mod.rs"]
mod player;

use macroquad::Window;
use macroquad::prelude::*;

use app::pad_svg;
use editor_egui::{DESIGN_H, DESIGN_W, State as EguiState};
use player::state::PadPreviewState;

fn editor_window_conf() -> Conf {
    let mut conf = app::window_conf();
    conf.window_title = "LambdaDX Editor".to_string();
    // Wider default so the vertical layout's timeline (2× the info panel) has
    // room. The layout is responsive, so any window size keeps the 2:1 ratio.
    conf.window_width = 1600;
    conf.window_height = 900;
    conf.window_resizable = true;
    conf
}

fn main() {
    Window::from_config(editor_window_conf(), run());
}

/// (Re)load a chart folder/file into a live `PadPreviewState`, keeping the
/// already-loaded textures/materials/SFX. `want_slot` selects a Simai
/// `&inote_N` difficulty when present.
fn load_chart_into(app: &mut PadPreviewState, dir: &std::path::Path, want_slot: Option<u32>) {
    let simai = app::chart::read_simai_source(dir);
    let slot = match &simai {
        Some(text) => {
            let keys = app::maidata::inote_keys(text);
            want_slot
                .filter(|s| keys.contains(s))
                .or_else(|| keys.last().copied())
        }
        None => None,
    };
    let chart = match (&simai, slot) {
        (Some(text), Some(s)) => app::maidata::from_maidata_level(text, s).unwrap_or_else(|e| {
            eprintln!("editor: parse failed: {e}");
            app::chart::fallback_chart()
        }),
        _ => app::chart::load_chart_from_path(dir, None).unwrap_or_else(|e| {
            eprintln!("editor: load failed: {e}");
            app::chart::fallback_chart()
        }),
    };
    let (name, pcm) = app::chart::find_audio_in_dir(dir)
        .map(|p| app::audio::load_audio_from_path(&p))
        .unwrap_or((None, None));
    app.load_chart(chart, name, pcm, simai, slot);
}

/// Native folder picker (macOS via `osascript`; no-op elsewhere).
fn pick_chart_path() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"Select chart folder\")",
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout)
            .trim()
            .trim_end_matches('/')
            .to_string();
        (!s.is_empty()).then_some(s)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
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
    // Load a chart *folder* (Simai source + track), like the standalone preview
    // with a chart argument, so the judgment engine and autoplay modes work.
    // Override with `MAI2_EDITOR_CHART`.
    let chart_dir = std::env::var("MAI2_EDITOR_CHART")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| app::platform::asset_dir().join("charts/jack_ripper"));
    let chart = app::chart::load_chart_from_path(&chart_dir, None).unwrap_or_else(|e| {
        eprintln!("editor: failed to load {}: {e}", chart_dir.display());
        app::chart::fallback_chart()
    });
    mark("chart load");
    let (audio_source_name, audio_wav_pcm) = app::chart::find_audio_in_dir(&chart_dir)
        .map(|p| app::audio::load_audio_from_path(&p))
        .unwrap_or((None, None));
    mark("audio decode");
    let mut app = PadPreviewState::new(chart, audio_source_name, audio_wav_pcm);
    app.params = app::params::load();
    app::params::set(app.params.clone());
    app.note_speed = app.params.note_speed_default;
    app.touch_speed = app.params.touch_speed_default;
    app.slide_fade_in = app.params.slide_fade_in;
    app.set_play_speed(app.params.play_speed_default);
    mark("state new + params");
    // lnmai-core judgment engine (only for Simai-sourced charts, like preview).
    if let Some(text) = app::chart::read_simai_source(&chart_dir) {
        let level = app::maidata::inote_key(&text, None).unwrap_or(app.chart.simai_level);
        match app.load_engine(&text, level) {
            Ok(()) => app.set_status("lnmai-core engine ready".into()),
            Err(e) => eprintln!("editor: engine load failed: {e}"),
        }
    }
    mark("engine load");
    player::autoplay::rebuild(&mut app);
    if let Ok(def) = pad_svg::PadSvgDef::from_svg_str(include_str!("../assets/pad.svg")) {
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
    // Pad cue / judgment SFX so the editor's pad sounds like the preview.
    player::sfx::load_pad_sfx(&mut app).await;
    mark("sfx");
    if timing {
        std::process::exit(0);
    }

    let mut ui = EguiState::new();
    let mut editor = editor_state::EditorState::default();
    if std::env::var("MAI2_EDITOR_LAYOUT")
        .map(|v| v.eq_ignore_ascii_case("vertical"))
        .unwrap_or(false)
    {
        editor.layout = editor_state::EditorLayout::Vertical;
    }
    editor.chart_dir = chart_dir.to_string_lossy().to_string();
    editor.level_slots = app::chart::read_simai_source(&chart_dir)
        .map(|t| app::maidata::inote_keys(&t))
        .unwrap_or_default();
    editor.level_index = editor.level_slots.len().saturating_sub(1);
    let mut waveform = editor_audio::WaveformPyramid::default();
    let shot_path = std::env::var("MAI2_EDITOR_SHOT").ok();
    let shot_at = std::env::var("MAI2_EDITOR_SHOT_AT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.6);
    let start_time = get_time();

    loop {
        app::anim::tick();
        // Same hotkeys / lane input as the standalone preview (Space, F1, R, …).
        player::input::handle_global_hotkeys(&mut app);
        player::input::handle_lane_input(&mut app);
        // Preview mode: autoplay-no-judge / manual-judge / autoplay-judge.
        // `P` cycles it; the header shows a clickable label too.
        if is_key_pressed(KeyCode::P) {
            editor.autoplay_mode = (editor.autoplay_mode + 1) % 3;
            editor.mode_dirty = true;
        }
        if editor.mode_dirty {
            editor.mode_dirty = false;
            apply_preview_mode(&mut app, editor.autoplay_mode);
        }
        let current_time = match app.mode {
            app::types::Mode::Idle => app.timeline_view_time,
            _ => app.song_time(),
        };
        if matches!(
            app.mode,
            app::types::Mode::Playing | app::types::Mode::Recording
        ) {
            // Follow the playhead while playing.
            editor.view_start = (current_time - 2.0).max(0.0);
        }
        let events = player::input::collect_pointer_events();
        let pointer = player::input::pointer::ui_pointer(&events);
        app::egui_bridge::pointer(pointer.pos, pointer.down, pointer.pressed, pointer.released);
        let mut egui_screen_w = 1.0;
        app::egui_bridge::ui(|ctx| {
            if waveform.levels.is_empty() && app.audio_wav_pcm.is_some() {
                waveform = editor_audio::WaveformPyramid::build(&app);
            }
            ui.draw(ctx, &mut editor, &mut app.chart, current_time, &waveform);
            ui.finish_layout(ctx);
            // F1 params window, same content as the standalone preview.
            player::params_panel::draw_embedded(ctx, &mut app);
            // Repaint every frame so a newly opened overlay (F1) shows at once.
            ctx.request_repaint();
            egui_screen_w = ctx.screen_rect().width();
        });
        let egui_wants_pointer = app::egui_bridge::wants_pointer();

        // Editor chart I/O requests raised by the sidebar (native picker / reload
        // / difficulty cycle). Applied before drawing so the new chart shows
        // this frame.
        if editor.open_dialog {
            editor.open_dialog = false;
            if let Some(path) = pick_chart_path() {
                editor.chart_dir = path;
                editor.reload_request = true;
            }
        }
        if editor.reload_request {
            editor.reload_request = false;
            let dir = std::path::PathBuf::from(&editor.chart_dir);
            let want = editor.level_slots.get(editor.level_index).copied();
            load_chart_into(&mut app, &dir, want);
            editor.level_slots = app::chart::read_simai_source(&dir)
                .map(|t| app::maidata::inote_keys(&t))
                .unwrap_or_default();
            if editor.level_index >= editor.level_slots.len() {
                editor.level_index = editor.level_slots.len().saturating_sub(1);
            }
            editor.selected.clear();
            editor.view_start = 0.0;
            editor.status = Some(format!("Loaded {}", editor.chart_dir));
        }
        let current_time = match app.mode {
            app::types::Mode::Idle => app.timeline_view_time,
            _ => app.song_time(),
        };

        // egui points → macroquad's logical coordinate space. egui renders at
        // `framebuffer / egui_screen_width`, macroquad at
        // `framebuffer / screen_width()`; the ratio is normally 1, but computing
        // it keeps the pad/timeline aligned even if the DPI scales disagree.
        let to_mq = if egui_screen_w > 1.0 {
            screen_width() / egui_screen_w
        } else {
            1.0
        };

        // Frame-buffer pixels and the egui-point→frame-buffer-pixel scale.
        // macroquad's `scissor` is in raw frame-buffer pixels, while its draw
        // coordinates are logical (framebuffer / dpi_scale).
        let (fb_w, fb_h) = macroquad::miniquad::window::screen_size();
        let s_phys = if egui_screen_w > 1.0 {
            fb_w / egui_screen_w
        } else {
            1.0
        };

        // A timeline / overview scrub seeks the shared player clock, so the
        // pad preview and the red line stay in sync.
        if let Some(t) = editor.pending_seek.take() {
            let t = t.max(0.0);
            app.timeline_view_time = t;
            app.mode_song_offset = t;
            app.mode_wall_anchor = get_time();
        }

        // egui owns the window coordinate system. The pad uses the same screen
        // coordinates, so no fixed 1570x856 camera or letterbox is involved.
        app::viewport::set(0.0, 0.0, 1.0);
        let mut pad = ui.layout.pad;
        pad.cx *= to_mq;
        pad.cy *= to_mq;
        pad.outer_r *= to_mq;
        pad.outer_r /= app::params::pad_circle_scale().max(1.0);
        let pad_events: Vec<_> = {
            events
                .into_iter()
                .filter(|e| {
                    let p = e.position;
                    (p.x - pad.cx).hypot(p.y - pad.cy) <= pad.outer_r
                        || app.active_pointer_zones.contains_key(&e.id)
                })
                .collect()
        };
        let scale = (pad.outer_r / (DESIGN_H * 0.42)).max(0.1);
        app.ui_scale_override = Some(scale);
        app::params::set_pad_scale(scale * app::params::pad_zoom());
        if !egui_wants_pointer {
            player::input::handle_touch_controls(&mut app, pad, &pad_events);
        }
        // Same audio / cue / autoplay / judgment pipeline as the preview.
        // While the editor is scrubbing the playhead, suppress cue sounds and
        // feed autoplay straight to the backend (no sensor-zone mirroring).
        app.scrubbing = editor.scrubbing;
        app::audio::service_audio(&mut app).await;
        app.tick_cues();
        player::autoplay::tick(&mut app);
        player::engine::step_judge_engine(&mut app);

        clear_background(Color::from_rgba(29, 29, 30, 255));
        let preview = ui.layout.preview;
        // Sidebar background follows the panel (left in horizontal, right in
        // vertical layout).
        let sidebar = ui.layout.sidebar;
        if sidebar.width() > 1.0 && sidebar.height() > 1.0 {
            draw_rectangle(
                sidebar.left() * to_mq,
                sidebar.top() * to_mq,
                sidebar.width() * to_mq,
                sidebar.height() * to_mq,
                Color::from_rgba(34, 34, 34, 255),
            );
        }
        let left = (preview.left() * s_phys)
            .floor()
            .clamp(0.0, fb_w) as i32;
        let top = (preview.top() * s_phys)
            .floor()
            .clamp(0.0, fb_h) as i32;
        let right = (preview.right() * s_phys)
            .ceil()
            .clamp(0.0, fb_w) as i32;
        let bottom = (preview.bottom() * s_phys)
            .ceil()
            .clamp(0.0, fb_h) as i32;
        unsafe { get_internal_gl() }.quad_gl.scissor(Some((
            left,
            top,
            (right - left).max(0),
            (bottom - top).max(0),
        )));
        player::render::pad::draw_pad_sim(&app, pad.cx, pad.cy, pad.outer_r, scale);
        unsafe { get_internal_gl() }.quad_gl.scissor(None);

        // Timeline (macroquad, behind the transparent central egui panel).
        let timeline = ui.layout.timeline;
        if timeline.width() > 1.0 && timeline.height() > 1.0 {
            // Draw rect in macroquad's logical coordinates, scissor in raw
            // frame-buffer pixels.
            let mq_left = timeline.left() * to_mq;
            let mq_top = timeline.top() * to_mq;
            let mq_w = timeline.width() * to_mq;
            let mq_h = timeline.height() * to_mq;
            let left = (timeline.left() * s_phys).floor().clamp(0.0, fb_w) as i32;
            let top = (timeline.top() * s_phys).floor().clamp(0.0, fb_h) as i32;
            let right = (timeline.right() * s_phys).ceil().clamp(0.0, fb_w) as i32;
            let bottom = (timeline.bottom() * s_phys).ceil().clamp(0.0, fb_h) as i32;
            unsafe { get_internal_gl() }.quad_gl.scissor(Some((
                left,
                top,
                (right - left).max(0),
                (bottom - top).max(0),
            )));
            let timeline_rect = Rect::new(mq_left, mq_top, mq_w, mq_h);
            match editor.layout {
                editor_state::EditorLayout::Horizontal => editor_timeline::draw_macroquad(
                    &app,
                    &editor,
                    &app.chart,
                    timeline_rect,
                    current_time,
                    &waveform,
                    to_mq,
                ),
                editor_state::EditorLayout::Vertical => {
                    editor_timeline::draw_macroquad_vertical(
                        &app,
                        &editor,
                        &app.chart,
                        timeline_rect,
                        current_time,
                        &waveform,
                        to_mq,
                    )
                }
            }
            unsafe { get_internal_gl() }.quad_gl.scissor(None);
        }

        app.tick_feedback();
        app::egui_bridge::draw();
        app.finalize_playback_start();

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

/// Apply the 3-way preview mode to the shared player state.
///
/// * `0` autoplay **without** judgment (legacy autoplay + `no_core`)
/// * `1` manual play **with** judgment (core)
/// * `2` autoplay **with** judgment (core)
fn apply_preview_mode(app: &mut PadPreviewState, mode: u8) {
    let (autoplay, no_core) = match mode % 3 {
        0 => (true, true),
        1 => (false, false),
        _ => (true, false),
    };
    player::autoplay::set_on(app, autoplay);
    let mut p = app::params::get();
    if p.no_core != no_core {
        p.no_core = no_core;
        app::params::set(p.clone());
        app.params = p;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn egui_editor_uses_responsive_design_constants() {
        assert_eq!(DESIGN_W, 1570.0);
        assert_eq!(DESIGN_H, 856.0);
        assert!(editor_egui::SIDEBAR_MIN < editor_egui::SIDEBAR_DEFAULT);
    }
}
