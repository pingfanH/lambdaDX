mod audio;
mod chart;
mod input;
mod pad_svg;
mod platform;
mod state;
mod types;
mod ui;

use macroquad::audio::load_sound_from_bytes;
use macroquad::file::set_pc_assets_folder;
use macroquad::prelude::{clear_background, next_frame, Color};

use state::AppState;

pub use ui::window_conf;

/// Main app loop.
/// `main.rs` only keeps the macroquad entry function and delegates to here.
pub async fn run_app() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    set_pc_assets_folder("assets");

    let chart = chart::load_generated_chart().await;
    let (audio_source_name, audio_wav_pcm) = audio::load_audio_pcm_from_assets().await;
    let mut app = AppState::new(chart, audio_source_name, audio_wav_pcm);

    // Parse the SVG pad definition
    match pad_svg::PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")) {
        Ok(def) => {
            app.pad_svg = Some(def);
        }
        Err(e) => {
            app.status = format!("Failed to parse pad.svg: {e}");
        }
    }

    // Load hit sounds from embedded bytes
    match load_sound_from_bytes(include_bytes!("../../assets/Sfx/answer.wav")).await {
        Ok(s) => { app.hit_sound = Some(s); }
        Err(e) => app.status = format!("Failed to load answer.wav: {e:?}"),
    }
    match load_sound_from_bytes(include_bytes!("../../assets/Sfx/touch.wav")).await {
        Ok(s) => { app.touch_sound = Some(s); }
        Err(e) => app.status = format!("Failed to load touch.wav: {e:?}"),
    }

    ui::load_note_textures(&mut app).await;
    audio::warm_audio_cache(&mut app, 1.0).await;

    loop {
        clear_background(Color::from_rgba(10, 17, 30, 255));

        let layout = ui::compute_layout(&app);
        let pad_geom = ui::compute_pad_geom(layout.pad);
        let buttons = ui::build_ui_buttons(layout, &app);
        let pointer_events = input::collect_pointer_events();

        input::handle_global_hotkeys(&mut app);
        input::handle_touch_controls(&mut app, pad_geom, &buttons, &pointer_events);
        input::handle_lane_input(&mut app);
        audio::service_audio(&mut app).await;
        app.update_playback();
        app.service_hit_sounds();
        app.tick_feedback();

        ui::draw_layout(&app, layout, pad_geom, &buttons);

        next_frame().await;
    }
}
