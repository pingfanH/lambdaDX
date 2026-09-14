pub mod audio;
pub mod command;
pub mod egui;
pub mod engine;
pub mod input;
pub mod perf;
pub mod player_layout;
pub mod state;
#[cfg(target_os = "linux")]
pub mod touch_evdev;
pub mod ui;

use lambda_dx::app::{pad_svg, platform, sfx, types};
use macroquad::Window;
use macroquad::color::Color;
use macroquad::file::set_pc_assets_folder;
use macroquad::prelude::{clear_background, next_frame};
use std::time::Instant;

use crate::command::LaunchArgs;
use crate::state::PlayerState;
use lambda_dx::window_conf;

pub fn main() {
    match command::parse_env() {
        Ok(LaunchArgs::Help) => {
            print!("{}", command::help());
        }
        Ok(launch_args) => {
            Window::from_config(window_conf(), run(launch_args));
        }
        Err(error) => {
            eprintln!("error: {error}\n");
            eprint!("{}", command::help());
            std::process::exit(2);
        }
    }
}

async fn run(launch_args: LaunchArgs) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let asset_dir = platform::asset_dir();
        set_pc_assets_folder(&asset_dir.to_string_lossy());
    }

    let chart = lambda_dx::chart::load_generated_chart().await;
    let (audio_source_name, audio_wav_pcm) =
        lambda_dx::app::audio::load_audio_pcm_from_assets().await;
    let mut app = PlayerState::new(chart, audio_source_name, audio_wav_pcm);

    // Parse the SVG pad definition
    match pad_svg::PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")) {
        Ok(def) => {
            app.pad_svg = Some(def);
        }
        Err(e) => {
            app.set_status(format!("Failed to parse pad.svg: {e}"));
        }
    }

    // Initialize low-latency SFX player (rodio/cpal)
    match sfx::SfxPlayer::new() {
        Ok(player) => {
            app.sfx_player = Some(player);
            app.sfx_tap =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/tap_perfect.wav"));
            app.sfx_touch =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/touch.wav"));
            app.sfx_slide =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/slide.wav"));
            app.sfx_touch_riser =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/touch_Hold_riser.wav"));
            app.sfx_break =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/break.wav"));
            app.sfx_break_tap =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/break_tap.wav"));
            app.sfx_tap_ex =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/tap_ex.wav"));
            app.sfx_slide_break_start = sfx::SfxBuffer::from_bytes(include_bytes!(
                "../../assets/Sfx/slide_break_start.wav"
            ));
            app.sfx_break_slide =
                sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/break_slide.wav"));
        }
        Err(e) => app.set_status(format!("SFX init failed: {e}")),
    }

    // Load mask shader material
    match lambda_dx::app::load_mask_material() {
        Ok(m) => app.mask_material = Some(m),
        Err(e) => app.set_status(format!("Shader: {e}")),
    }

    ui::load_note_textures(&mut app).await;

    if let LaunchArgs::Command(command) = &launch_args {
        if let Err(error) = command::apply_to_app(&mut app, command) {
            app.set_status(format!("Command mode: {error}"));
            app.player_ui.song_error = Some(error);
        }
    }

    #[cfg(target_os = "linux")]
    let mut evdev_touch = touch_evdev::EvdevTouch::start(None);
    #[cfg(target_os = "linux")]
    if let Some(t) = &evdev_touch {
        match t.last_error() {
            Some(e) => app.set_status(format!("Multi-touch: {e}")),
            None => app.set_status(format!("Multi-touch: {}", t.device_name())),
        }
    }

    // Prime egui state on first frame to avoid mouse event issues on macOS.
    egui_macroquad::ui(|_| {});
    egui_macroquad::draw();
    loop {
        let frame_start = Instant::now();
        clear_background(Color::from_rgba(30, 30, 30, 255));

        let layout = player_layout::compute_layout(&app);
        let pad_geom = ui::compute_pad_geom(layout.pad);
        let buttons: Vec<types::UiButton> = Vec::new(); // buttons via egui

        let draw_start = Instant::now();
        input::handle_global_hotkeys(&mut app);
        if app.player_ui.page == state::PlayerPage::Gameplay {
            input::handle_lane_input(&mut app);

            let mut pointer_events = lambda_dx::app::input::collect_pointer_events();
            #[cfg(target_os = "linux")]
            if evdev_touch
                .as_ref()
                .is_some_and(|touch| touch.last_error().is_some())
            {
                evdev_touch = None;
            }
            if let Some(t) = evdev_touch.as_mut() {
                pointer_events.extend(t.collect_pointer_events(macroquad::math::vec2(
                    macroquad::prelude::screen_width(),
                    macroquad::prelude::screen_height(),
                )));
            }

            input::handle_touch_controls(&mut app, pad_geom, &buttons, &pointer_events);
        }
        audio::service_audio(&mut app).await;

        if app.player_ui.page == state::PlayerPage::Gameplay
            && app.mode == lambda_dx::app::types::Mode::Playing
        {
            engine::step_judge_engine(&mut app);
        }

        if app.player_ui.shows_gameplay_background() {
            player_layout::draw_layout(&app, layout, pad_geom, &buttons);
        }

        app.tick_feedback();
        let frontend_elapsed = frame_start.elapsed();
        let draw_input_elapsed = draw_start.elapsed();

        let egui_start = Instant::now();
        egui_macroquad::ui(|egui_ctx| {
            egui::draw_egui_ui(egui_ctx, &mut app);
        });
        egui_macroquad::draw();
        let egui_elapsed = egui_start.elapsed();

        perf::record("frontend.draw_input", draw_input_elapsed);
        perf::record("frontend", frontend_elapsed);
        perf::record("egui", egui_elapsed);
        perf::record("total", frame_start.elapsed());
        perf::frame_done();

        if app.pending_import {
            app.pending_import = false;
            match lambda_dx::simai_io::dialog_import() {
                Ok(import) => egui::finish_dialog_import(&mut app, import),
                Err(e) if e == "cancelled" => {}
                Err(e) => app.set_status(format!("Import: {e}")),
            }
        }

        next_frame().await;
    }
}
