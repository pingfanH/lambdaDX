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
mod player;
mod simai;

use macroquad::color::Color;
use macroquad::file::set_pc_assets_folder;
use macroquad::prelude::{clear_background, next_frame};
use macroquad::Window;

use app::cli::LaunchArgs;
use app::{audio, chart, pad_svg, platform};
use player::state::PadPreviewState;

fn main() {
    match app::cli::parse_env() {
        Ok(None) => {
            print!("{}", app::cli::help());
        }
        Ok(Some(args)) => {
            Window::from_config(app::window_conf(), run(args));
        }
        Err(e) => {
            eprintln!("error: {e}\n");
            eprint!("{}", app::cli::help());
            std::process::exit(2);
        }
    }
}

async fn run(args: LaunchArgs) {
    // Assets are still used for pad.svg, skins, the mask shader and the cue
    // sound, even when a chart is loaded from elsewhere.
    set_pc_assets_folder(&platform::asset_dir().to_string_lossy());

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

    // Cue sound played at tap / hold head / hold tail / slide star head.
    app.answer_sfx = audio::load_answer_sfx().await;
    if app.answer_sfx.is_none() {
        app.set_status("Cue sound missing: assets/Sfx/answer.wav".to_string());
    }

    loop {
        clear_background(Color::from_rgba(30, 30, 30, 255));

        let layout = player::layout::compute_layout(&app);
        let pad_geom = player::layout::compute_pad_geom(layout.pad);

        player::input::handle_global_hotkeys(&mut app);
        player::input::handle_lane_input(&mut app);
        let pointer_events = player::input::collect_pointer_events();
        player::input::handle_touch_controls(&mut app, pad_geom, &pointer_events);

        audio::service_audio(&mut app).await;

        // Fire cue sounds (tap / hold head / hold tail / slide star head).
        app.tick_cues();

        player::render::draw_pad_panel(&app, layout.pad, pad_geom);

        app.tick_feedback();

        // Release the frozen song clock once the first frame is on screen.
        if app.playback_pending {
            app.finalize_playback_start();
        }

        next_frame().await;
    }
}
