//! Standalone LambdaDX pad preview.
//!
//! Extracted from `macroquad_sim`'s `lambda_dx_player` bin. It keeps the pad
//! rendering (zones, touch highlights, note flight for tap/hold/touch/slide) and
//! audio-driven playback, dropping the editor, song library, judgment engine and
//! egui front-end. See `docs/PAD_PREVIEW.md`.
//!
//! A chart folder or JSON file can be passed on the command line; see
//! `--help` / `app/cli.rs`.

// The `app` modules are near-verbatim copies of a much larger crate, so they
// intentionally carry items the preview does not use (editor, judgment, save,
// alternate render paths). Silencing those keeps the extraction diff small.
#![allow(dead_code, unused_variables, unused_imports)]

mod app;
mod core;
mod player;

use macroquad::color::Color;
use macroquad::file::set_pc_assets_folder;
use macroquad::prelude::{
    Camera2D, FilterMode, Rect, clear_background, next_frame, render_target, set_camera,
    set_default_camera,
};
use macroquad::Window;

use app::cli::LaunchArgs;
use app::types::MOUSE_POINTER_ID;
use app::{audio, chart, pad_svg, platform};
use player::state::PadPreviewState;

use std::path::PathBuf;

fn main() {
    match app::cli::parse_env() {
        Ok(None) => {
            print!("{}", app::cli::help());
        }
        Ok(Some(args)) => {
            if args.dump {
                dump_and_exit(&args);
            }
            if let Some(path) = &args.slides_svg {
                dump_slides_svg(path);
            }
            if let Some(out) = args.export_video.clone() {
                let n = args.export_parallel.unwrap_or(1).max(1);
                if n > 1 {
                    // Orchestrate in this process (no window) and exit.
                    export_parallel(&args, out, n);
                    return;
                }
            }
            let mut conf = app::window_conf();
            if args.export_video.is_some() {
                // Export renders offscreen; don't let vsync throttle it, and keep
                // the on-screen window as small/unobtrusive as possible.
                conf.platform.swap_interval = Some(0);
                conf.window_width = 1;
                conf.window_height = 1;
                conf.high_dpi = false;
                conf.sample_count = 1;
            }
            Window::from_config(conf, run(args));
        }
        Err(e) => {
            eprintln!("error: {e}\n");
            eprint!("{}", app::cli::help());
            std::process::exit(2);
        }
    }
}

/// `--dump`: load the chart synchronously, print it, and exit (no window).
fn dump_and_exit(args: &LaunchArgs) -> ! {
    let path = args
        .chart
        .clone()
        .unwrap_or_else(|| platform::asset_dir().join("charts/jack_ripper/chart.json"));
    match chart::load_chart_from_path(&path, args.diff) {
        Ok(c) => {
            dump_chart(&c);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("error: failed to load chart from {}: {e}", path.display());
            std::process::exit(2);
        }
    }
}

/// `--dump-slides-svg`: write every slide curve to an SVG and exit.
fn dump_slides_svg(path: &std::path::Path) -> ! {
    let svg = app::slide::export::all_paths_svg();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(path, svg) {
        Ok(()) => {
            println!("wrote {}", path.display());
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("error: failed to write {}: {e}", path.display());
            std::process::exit(2);
        }
    }
}

fn dump_chart(c: &app::types::ChartDoc) {
    use app::types::{NoteType, measure_to_secs};
    let mut taps = 0;
    let mut holds = 0;
    let mut touches = 0;
    for n in &c.notes {
        match n.note_type {
            NoteType::Tap => taps += 1,
            NoteType::Hold => holds += 1,
            NoteType::Touch => touches += 1,
            NoteType::Slide => {}
        }
    }
    println!("chart: {} — {}", c.title, c.artist);
    println!("notes: {} (taps {taps}, holds {holds}, touches {touches})", c.notes.len());
    println!("bpms: {:?}", c.bpms);
    println!("slides (measure / seconds / lane / shape / end / wait / travel):");
    for (i, n) in c.notes.iter().enumerate() {
        if !matches!(n.note_type, NoteType::Slide) {
            continue;
        }
        for (si, s) in n.slide.iter().enumerate() {
            let seg = &s.segments[0];
            let end = seg
                .points
                .last()
                .map(|p| p.zone.to_string())
                .unwrap_or_else(|| "-".into());
            let shapes: Vec<String> = s.segments.iter().map(|x| format!("{:?}", x.shape)).collect();
            println!(
                "  #{i}.{si} m={:.4} s={:.4} lane={} shapes=[{}] end={} wait={:.4} travel={:.4}",
                n.time,
                measure_to_secs(n.time, &c.bpms),
                n.lane,
                shapes.join(","),
                end,
                s.slide_start_delay,
                s.slide_duration - s.slide_start_delay,
            );
        }
    }
}

async fn run(args: LaunchArgs) {
    // Assets are still used for pad.svg, skins, the mask shader and the cue
    // sound, even when a chart is loaded from elsewhere.
    set_pc_assets_folder(&platform::asset_dir().to_string_lossy());

    // Load the Animate project (movies referenced by name, e.g. `ui.get(..)`).
    app::anim::init();

    // ── Chart ──────────────────────────────────────────────────────────
    let chart = match &args.chart {
        Some(path) => match chart::load_chart_from_path(path, args.diff) {
            Ok(c) => {
                println!(
                    "Loaded chart '{}' from {} ({} notes)",
                    c.title,
                    path.display(),
                    c.notes.len()
                );
                c
            }
            Err(e) => {
                eprintln!("error: failed to load chart from {}: {e}", path.display());
                std::process::exit(2);
            }
        },
        None => chart::load_generated_chart(args.diff).await,
    };

    // ── Audio: explicit path > chart-folder track > bundled assets ──────
    let (audio_source_name, audio_wav_pcm) = if let Some(path) = &args.audio {
        let (name, pcm) = audio::load_audio_from_path(path);
        if pcm.is_none() {
            eprintln!("warning: could not decode audio from {}", path.display());
        }
        (name, pcm)
    } else if let Some(track) = args
        .chart
        .as_deref()
        .filter(|p| p.is_dir())
        .and_then(chart::find_audio_in_dir)
    {
        audio::load_audio_from_path(&track)
    } else {
        audio::load_audio_pcm_from_assets().await
    };

    let mut app = PadPreviewState::new(chart, audio_source_name, audio_wav_pcm);

    // ── lnmai-core judgment engine (only for Simai-sourced charts) ─────
    // Skipped during a (non-core) export: the export uses local autoplay.
    let load_engine = args.export_video.is_none() || args.export_core;
    if load_engine {
        match args.chart.as_deref().and_then(chart::read_simai_source) {
            Some(text) => {
                let level =
                    app::maidata::inote_key(&text, args.diff).unwrap_or(app.chart.simai_level);
                match app.load_engine(&text, level) {
                    Ok(()) => {
                        println!("lnmai-core engine loaded (levelIndex={level})");
                        app.set_status("lnmai-core engine ready".to_string());
                    }
                    Err(e) => {
                        eprintln!("warning: lnmai-core engine load failed: {e}");
                        app.set_status(format!("engine: {e}"));
                    }
                }
            }
            None => {
                eprintln!(
                    "note: no Simai source (pass a maidata.txt / chart folder); \
                     lnmai-core judgment is disabled for this chart"
                );
            }
        }
    }

    // Tunable visual params (override JSON > bundled JSON > built-in defaults).
    app.params = app::params::load();
    app::params::set(app.params.clone());
    // Apply the persisted default speeds / fade-in to this session.
    app.note_speed = app.params.note_speed_default;
    app.touch_speed = app.params.touch_speed_default;
    app.slide_fade_in = app.params.slide_fade_in;
    // Persisted default playback speed.
    app.set_play_speed(app.params.play_speed_default);
    // Autoplay schedule for the initial chart (toggle with `O` or the panel).
    player::autoplay::rebuild(&mut app);
    if std::env::var("MAI2_AUTOPLAY").is_ok() || std::env::var("MAI2_UI_AUTOPLAY").is_ok() {
        player::autoplay::set_on(&mut app, true);
    }

    // Parse the SVG pad definition.
    match pad_svg::PadSvgDef::from_svg_str(include_str!("../assets/pad.svg")) {
        Ok(def) => app.pad_svg = Some(def),
        Err(e) => app.set_status(format!("Failed to parse pad.svg: {e}")),
    }

    // Note skins + touch-hold border shader.
    player::render::load_note_textures(&mut app).await;
    match app::ui::load_mask_material() {
        Ok(m) => app.mask_material = Some(m),
        Err(e) => app.set_status(format!("Shader: {e}")),
    }

    // Desktop Linux: announce the multi-touch touchscreen (if any).
    #[cfg(target_os = "linux")]
    if let Some(status) = player::input::touch_evdev::status() {
        app.set_status(status);
    }

    // Pad cue / judgment SFX (shared with the UI player so both sound the same).
    player::sfx::load_pad_sfx(&mut app).await;
    if app.answer_sfx.is_none() {
        app.set_status("Cue sound missing: assets/Sfx/answer.wav".to_string());
    }

    // ── Offscreen video export (no window loop) ────────────────────────
    if let Some(output) = args.export_video.clone() {
        run_export(&mut app, &args, output).await;
        return;
    }

    loop {
        clear_background(Color::from_rgba(30, 30, 30, 255));
        app::anim::tick();

        let layout = player::layout::compute_layout(&app);
        let pad_geom = player::layout::compute_pad_geom(layout.pad);

        player::input::handle_global_hotkeys(&mut app);
        // F5: re-read the chart from disk and reload (picks up edits live).
        if macroquad::input::is_key_pressed(macroquad::input::KeyCode::F5) {
            if let Some(text) = args.chart.as_deref().and_then(chart::read_simai_source) {
                let level =
                    app::maidata::inote_key(&text, args.diff).unwrap_or(app.chart.simai_level);
                match app::maidata::from_maidata_level(&text, level) {
                    Ok(mut chart) => {
                        app::maichart::assign_note_ids(&mut chart.notes);
                        app.chart = chart;
                        app.cue_track =
                            Some(player::cues::CueTrack::from_chart(&app.chart));
                        player::autoplay::rebuild(&mut app);
                        let _ = app.load_engine(&text, level);
                        app.start_playback_at(0.0);
                        app.set_status("谱面已重新载入".to_string());
                    }
                    Err(e) => app.set_status(format!("reload: {e}")),
                }
            }
        }
        player::input::handle_lane_input(&mut app);
        let pointer_events = player::input::collect_pointer_events();
        let ui_scale = player::render::ui_scale(&app);
        // The HUD progress bar / AUTO button take priority over pad touches.
        let hud_consumed = player::hud::handle_input(&mut app, layout.header, ui_scale);
        let pointer_events: Vec<_> = if hud_consumed {
            pointer_events
                .into_iter()
                .filter(|e| e.id != MOUSE_POINTER_ID)
                .collect()
        } else {
            pointer_events
        };
        player::input::handle_touch_controls(&mut app, pad_geom, &pointer_events);

        audio::service_audio(&mut app).await;

        // Fire cue sounds (tap / hold head / hold tail / slide star head).
        app.tick_cues();
        // Chart-driven autoplay (synthetic touches) for this frame.
        player::autoplay::tick(&mut app);
        // Step the lnmai-core judgment engine and apply its feedback/sfx.
        player::engine::step_judge_engine(&mut app);

        // Background video (bg.mp4) via the ffmpeg sidecar.
        let video_cfg = player::video::VideoConfig {
            enabled: app::params::bg_video(),
            path: player::video::resolve_path(&app::params::bg_video_path()),
            start: app::params::bg_video_start(),
            fps: app::params::bg_video_fps(),
            height: app::params::bg_video_height().max(2.0) as usize,
            looping: app::params::bg_video_loop(),
        };
        let song_t = app.song_time();
        app.video_bg.sync(&video_cfg, song_t);

        // Shared themed surface; a bg.mp4 video, when enabled, replaces it.
        let bg_a = app::params::pad_bg_alpha().clamp(0.0, 255.0) / 255.0;
        player::render::draw_pad_panel(
            &app,
            layout.pad,
            pad_geom,
            player::render::PadSurface::themed(bg_a),
        );
        player::hud::draw(&app, layout.header, ui_scale);

        app.tick_feedback();

        // egui params panel on top (F1).
        egui_macroquad::ui(|ctx| player::params_panel::draw(ctx, &mut app));
        egui_macroquad::draw();

        // Release the frozen song clock once the first frame is on screen.
        if app.playback_pending {
            app.finalize_playback_start();
        }

        next_frame().await;
    }
}


/// Keep an audio path only when it is a real file (guards against a stray
/// positional argument / typo becoming an ffmpeg input).
fn valid_audio(path: Option<PathBuf>) -> Option<PathBuf> {
    match path {
        Some(p) if p.is_file() => Some(p),
        Some(p) => {
            eprintln!("warning: ignoring audio path `{}` (not a file)", p.display());
            None
        }
        None => None,
    }
}

/// Chart end time in seconds (last hold/slide tail), for the export length.
fn chart_end_secs(chart: &app::types::ChartDoc) -> f32 {    use app::types::{NoteType, hold_tail_time, mdur_to_secs, note_secs};
    let mut end = 0.0_f32;
    for n in &chart.notes {
        let e = match n.note_type {
            NoteType::Hold => hold_tail_time(n, &chart.bpms),
            NoteType::Slide => {
                let dur = n
                    .slide
                    .iter()
                    .map(|s| s.slide_duration)
                    .fold(0.0_f32, f32::max);
                note_secs(n, &chart.bpms) + mdur_to_secs(dur, n.time, &chart.bpms)
            }
            _ => note_secs(n, &chart.bpms),
        };
        end = end.max(e);
    }
    end.max(1.0)
}

/// Deterministic offscreen export: render each frame at a fixed virtual time and
/// pipe RGBA into `ffmpeg` (`--export-video`).
async fn run_export(app: &mut PadPreviewState, args: &LaunchArgs, output: PathBuf) {
    use player::export_video::{Encoder, ExportConfig, VideoEncoder, resolve_encoder};

    let fps = args.export_fps.unwrap_or(60).max(1);
    let (out_w, out_h) = args.export_size.unwrap_or((1920, 1080));
    let (width, height) = args.export_render_size.unwrap_or((out_w, out_h));
    let crf = args.export_crf.unwrap_or(18);
    let preset = args
        .export_preset
        .clone()
        .unwrap_or_else(|| "veryfast".to_string());
    let bitrate = args
        .export_bitrate
        .clone()
        .unwrap_or_else(|| "12M".to_string());
    let encoder = resolve_encoder(
        &args
            .export_encoder
            .as_deref()
            .map(Encoder::parse)
            .unwrap_or(Encoder::Auto),
    );

    let audio_path = if args.export_no_audio {
        None
    } else {
        valid_audio(
            args.export_audio
                .clone()
                .or_else(|| args.audio.clone())
                .or_else(|| {
                    args.chart
                        .as_deref()
                        .filter(|p| p.is_dir())
                        .and_then(chart::find_audio_in_dir)
                }),
        )
    };
    let duration = args
        .export_duration
        .unwrap_or_else(|| chart_end_secs(&app.chart) + 2.0)
        .max(0.1);
    let start = args.export_start.unwrap_or(0.0).max(0.0);

    let cfg = ExportConfig {
        output,
        audio: audio_path,
        audio_offset: app.chart.audio_offset,
        render_w: width,
        render_h: height,
        out_w,
        out_h,
        fps,
        encoder,
        bitrate,
        crf,
        preset,
    };

    let mut encoder = match VideoEncoder::spawn(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    // Deterministic run: virtual clock, no audio device, autoplay on.
    app.audio_enabled = false;
    app.forced_time = Some(0.0);
    app.mode = app::types::Mode::Playing;
    app.playback_pending = false;
    app.autoplay = true;
    // Skipping the core makes the export far faster: no per-frame JSON FFI.
    if !args.export_core {
        let mut p = app::params::get();
        p.no_core = true;
        app.params = p.clone();
        app::params::set(p);
    }
    player::autoplay::rebuild(app);
    // Match the visual scale to the export resolution.
    app.ui_scale_override = Some((width.min(height) as f32 / player::render::scale::DESIGN_MIN_SIDE).max(0.1));

    let video_cfg = player::video::VideoConfig {
        enabled: app::params::bg_video(),
        path: player::video::resolve_path(&app::params::bg_video_path()),
        start: app::params::bg_video_start(),
        fps: app::params::bg_video_fps(),
        height: app::params::bg_video_height().max(2.0) as usize,
        looping: app::params::bg_video_loop(),
    };

    let target = render_target(width, height);
    target.texture.set_filter(FilterMode::Linear);
    let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, width as f32, height as f32));
    camera.render_target = Some(target.clone());

    let total = (duration * fps as f32).ceil() as usize;
    let mut t_draw = std::time::Duration::ZERO;
    let mut t_read = std::time::Duration::ZERO;
    println!(
        "exporting {} @ render {}x{} -> {}x{} {}fps, {:?}, {:.1}s ({} frames){}",
        cfg.output.display(),
        width,
        height,
        out_w,
        out_h,
        fps,
        cfg.encoder,
        duration,
        total,
        cfg.audio
            .as_ref()
            .map(|a| format!(" + audio {}", a.display()))
            .unwrap_or_default()
    );

    for frame in 0..total {
        let t = start + frame as f32 / fps as f32;
        app.forced_time = Some(t);

        player::autoplay::tick(app);
        player::engine::step_judge_engine(app);
        app.tick_feedback();
        app.video_bg.sync(&video_cfg, t);

        let layout = player::layout::compute_layout_sized(app, width as f32, height as f32);
        let pad_geom = player::layout::compute_pad_geom(layout.pad);
        let ui_scale = player::render::ui_scale(app);

        let t0 = std::time::Instant::now();
        set_camera(&camera);
        clear_background(Color::from_rgba(30, 30, 30, 255));
        let bg_a = app::params::pad_bg_alpha().clamp(0.0, 255.0) / 255.0;
        player::render::draw_pad_panel(
            app,
            layout.pad,
            pad_geom,
            player::render::PadSurface::themed(bg_a),
        );
        player::hud::draw(app, layout.header, ui_scale);
        set_default_camera();
        t_draw += t0.elapsed();

        let t1 = std::time::Instant::now();
        let image = target.texture.get_texture_data();
        if let Err(e) = encoder.write_frame(image.bytes) {
            eprintln!("export error: {e}");
            std::process::exit(2);
        }
        t_read += t1.elapsed();
        if frame % fps as usize == 0 {
            println!("  {:.1}s / {:.1}s", t, duration);
        }
        // Present only occasionally: offscreen rendering + readback does not
        // need a buffer swap per frame, and presenting throttles the loop.
        if frame % (fps as usize).max(1) == 0 {
            next_frame().await;
        }
    }
    println!(
        "  timing: draw {:.1}ms/frame, read+write {:.1}ms/frame",
        t_draw.as_secs_f64() * 1000.0 / total as f64,
        t_read.as_secs_f64() * 1000.0 / total as f64,
    );

    match encoder.finish() {
        Ok(()) => println!("wrote {}", cfg.output.display()),
        Err(e) => {
            eprintln!("export failed: {e}");
            std::process::exit(2);
        }
    }
}

/// Multi-process export: split `[0, duration]` across `n` child processes (each
/// its own GL context / all cores), then concat with ffmpeg. Runs in `main`
/// before any window is created, so the orchestrator itself is headless.
fn export_parallel(args: &LaunchArgs, output: PathBuf, n: u32) {
    use std::process::{Command, Stdio};

    // Load the chart (no window needed) for the length + audio offset.
    let chart = args
        .chart
        .as_deref()
        .and_then(|p| chart::load_chart_from_path(p, args.diff).ok());
    let total = args
        .export_duration
        .unwrap_or_else(|| chart.as_ref().map(|c| chart_end_secs(c) + 2.0).unwrap_or(30.0))
        .max(0.1);
    let audio_offset = chart.as_ref().map(|c| c.audio_offset).unwrap_or(0.0);

    let fps = args.export_fps.unwrap_or(60);
    let (ow, oh) = args.export_size.unwrap_or((1920, 1080));
    let (rw, rh) = args.export_render_size.unwrap_or((ow, oh));
    let exe = std::env::current_exe().expect("current exe");

    let seg = total / n as f32;
    let parts: Vec<PathBuf> = (0..n)
        .map(|i| output.with_extension(format!("part{i}.mp4")))
        .collect();

    println!(
        "parallel export: {n} processes, {total:.1}s total ({seg:.1}s each), render {rw}x{rh} -> {ow}x{oh} {fps}fps"
    );
    println!(
        "note: macroquad cannot render without a window, so {n} small windows will open."
    );

    let mut children = Vec::new();
    for i in 0..n {
        let start = i as f32 * seg;
        let dur = if i + 1 == n { (total - start).max(0.1) } else { seg };
        let mut cmd = Command::new(&exe);
        if let Some(c) = &args.chart {
            cmd.arg(c);
        }
        if let Some(d) = args.diff {
            cmd.args(["--diff", &d.to_string()]);
        }
        cmd.args(["--export-video", &parts[i as usize].to_string_lossy()]);
        cmd.args(["--export-start", &format!("{start:.6}")]);
        cmd.args(["--export-duration", &format!("{dur:.6}")]);
        cmd.args(["--export-fps", &fps.to_string()]);
        cmd.args(["--export-size", &format!("{ow}x{oh}")]);
        cmd.args(["--export-render-size", &format!("{rw}x{rh}")]);
        if let Some(e) = &args.export_encoder {
            cmd.args(["--export-encoder", e]);
        }
        if let Some(b) = &args.export_bitrate {
            cmd.args(["--export-bitrate", b]);
        }
        if let Some(p) = &args.export_preset {
            cmd.args(["--export-preset", p]);
        }
        if let Some(c) = args.export_crf {
            cmd.args(["--export-crf", &c.to_string()]);
        }
        cmd.args(["--export-parallel", "1", "--export-no-audio"]);
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

        match cmd.spawn() {
            Ok(child) => children.push((i, child)),
            Err(e) => {
                eprintln!("error: cannot spawn export segment {i}: {e}");
                std::process::exit(2);
            }
        }
    }

    let mut ok = true;
    for (i, mut child) in children {
        match child.wait() {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("error: export segment {i} exited with {s}");
                ok = false;
            }
            Err(e) => {
                eprintln!("error: waiting for segment {i}: {e}");
                ok = false;
            }
        }
    }
    if !ok {
        std::process::exit(2);
    }

    // Concat the segments and mux the full track.
    let list = output.with_extension("parts.txt");
    let mut text = String::new();
    for p in &parts {
        text.push_str(&format!("file '{}'\n", p.display()));
    }
    if let Err(e) = std::fs::write(&list, text) {
        eprintln!("error: cannot write concat list: {e}");
        std::process::exit(2);
    }

    let audio = if args.export_no_audio {
        None
    } else {
        valid_audio(
            args.export_audio
                .clone()
                .or_else(|| args.audio.clone())
                .or_else(|| {
                    args.chart
                        .as_deref()
                        .filter(|p| p.is_dir())
                        .and_then(chart::find_audio_in_dir)
                }),
        )
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-loglevel",
        "error",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        &list.to_string_lossy(),
    ]);
    if let Some(a) = &audio {
        if audio_offset > 1e-4 {
            cmd.args(["-ss", &format!("{audio_offset:.6}")]);
        }
        cmd.arg("-i").arg(a);
    }
    cmd.args(["-c:v", "copy"]);
    if audio.is_some() {
        cmd.args(["-c:a", "aac", "-b:a", "192k", "-shortest"]);
    }
    cmd.arg(&output);

    let status = cmd.status();
    for p in &parts {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(&list);

    match status {
        Ok(s) if s.success() => println!("wrote {}", output.display()),
        _ => {
            eprintln!("error: ffmpeg concat failed");
            std::process::exit(2);
        }
    }
}
