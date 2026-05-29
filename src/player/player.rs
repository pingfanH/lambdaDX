pub mod player_layout;
pub mod egui;
pub mod input;
pub mod state;
pub mod ui;
pub mod audio;

use macroquad::color::Color;
use macroquad::file::set_pc_assets_folder;
use macroquad::prelude::{clear_background, next_frame};
use lambda_dx::app::{egui_ui, pad_svg, sfx, types};

use lambda_dx::{ window_conf};
use crate::state::PlayerState;

#[macroquad::main(window_conf)]
pub async fn main() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    set_pc_assets_folder("assets");

    let chart = lambda_dx::chart::load_generated_chart().await;
    let (audio_source_name, audio_wav_pcm) = lambda_dx::app::audio::load_audio_pcm_from_assets().await;
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
            app.sfx_tap = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/tap_perfect.wav"));
            app.sfx_touch = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/touch.wav"));
            app.sfx_slide = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/slide.wav"));
            app.sfx_touch_riser = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/touch_Hold_riser.wav"));
            app.sfx_break = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/break.wav"));
            app.sfx_break_tap = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/break_tap.wav"));
            app.sfx_tap_ex = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/tap_ex.wav"));
            app.sfx_slide_break_start = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/slide_break_start.wav"));
            app.sfx_break_slide = sfx::SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/break_slide.wav"));
        }
        Err(e) => app.set_status(format!("SFX init failed: {e}")),
    }

    // Load mask shader material
    match lambda_dx::app::load_mask_material() {
        Ok(m) => app.mask_material = Some(m),
        Err(e) => app.set_status(format!("Shader: {e}")),
    }

    ui::load_note_textures(&mut app).await;
    app.init_lnmai();
    // Prime egui state on first frame to avoid mouse event issues on macOS
    egui_macroquad::ui(|egui_ctx| { egui_ctx.set_pixels_per_point(2.0); });
    egui_macroquad::draw();
    loop {
        clear_background(Color::from_rgba(10, 17, 30, 255));

        // Layout: timeline (left) + pad (right), below toolbar
        let layout = player_layout::compute_layout(&app);
        let pad_geom = ui::compute_pad_geom(layout.pad);
        let buttons: Vec<types::UiButton> = Vec::new(); // buttons via egui

        // Draw timeline + pad (native macroquad first)
        player_layout::draw_layout(&app, layout, pad_geom, &buttons);

        // Input
        input::handle_global_hotkeys(&mut app);
        input::handle_lane_input(&mut app);
        let pointer_events = lambda_dx::app::input::collect_pointer_events();
        input::handle_touch_controls(&mut app, pad_geom, &buttons, &pointer_events);
        audio::service_audio(&mut app).await;
        input::collect_lnmai_input_events(&mut app);
        app.advance_lnmai_frame();
        app.tick_judge_texts();
        app.tick_feedback();
        //lambda_dx::app::input::handle_timeline_editing(&mut app, layout.timeline);

        // Egui on top (build UI + draw)
        egui_macroquad::ui(|egui_ctx| {
            egui_ctx.set_pixels_per_point(2.0);
            egui::draw_egui_ui(egui_ctx, &mut app);
        });
        egui_macroquad::draw();

        next_frame().await;
    }
}
