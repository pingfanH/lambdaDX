pub mod audio;
mod beat_format;
pub mod chart;
pub mod egui_components;
pub mod egui_style;
pub mod egui_ui;
pub mod input;
pub mod pad_svg;
pub mod platform;
pub mod sfx;
pub mod simai_io;
pub mod slide;
pub mod slide_match;
pub mod slide_render;
pub mod state;
pub mod template;
pub mod toast;
pub mod types;
pub mod ui;

use macroquad::file::set_pc_assets_folder;
use macroquad::prelude::{Color, clear_background, next_frame};

use state::AppState;

use crate::egui_ui::draw_egui_ui;
pub use ui::window_conf;

/// Main app loop.
/// `bin` only keeps the macroquad entry function and delegates to here.
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
    match load_mask_material() {
        Ok(m) => app.mask_material = Some(m),
        Err(e) => app.set_status(format!("Shader: {e}")),
    }

    ui::load_note_textures(&mut app).await;
    // Prime egui state on first frame to avoid mouse event issues on macOS
    // egui_macroquad::ui(|egui_ctx| { egui_ctx.set_pixels_per_point(2.0); });
    // egui_macroquad::draw();
    loop {
        clear_background(Color::from_rgba(30, 30, 30, 255));

        // Layout: timeline (left) + pad (right), below toolbar
        let layout = ui::compute_layout(&app);
        // Pad touch geom centered in upper 60% of viewport (matches visual rendering)
        let pad_area_h = layout.pad.h * 0.6;
        let pad_area = types::RectF {
            x: layout.pad.x,
            y: layout.pad.y,
            w: layout.pad.w,
            h: pad_area_h,
        };
        let pad_geom = ui::compute_pad_geom(pad_area);
        let buttons: Vec<types::UiButton> = Vec::new(); // buttons via egui

        // Draw timeline + pad (native macroquad first)
        ui::draw_layout(&app, layout, pad_geom, &buttons);

        // Input
        input::handle_global_hotkeys(&mut app);
        input::handle_lane_input(&mut app);
        audio::service_audio(&mut app).await;
        app.update_playback();
        app.service_hit_sounds();
        app.tick_feedback();

        let pointer_events = input::collect_pointer_events();
        input::handle_touch_controls(&mut app, pad_geom, &buttons, &pointer_events);
        input::handle_timeline_editing(&mut app, layout.timeline);

        // Egui on top (build UI + draw)
        egui_macroquad::ui(|egui_ctx| {
            draw_egui_ui(egui_ctx, &mut app);
        });
        egui_macroquad::draw();

        // Handle pending file dialog import (from main loop, not egui frame)
        if app.pending_import {
            app.pending_import = false;
            match simai_io::dialog_import() {
                Ok(import) => {
                    let n = import.chart.notes.len();
                    app.import_levels = import.levels.clone();
                    app.imported_simai = Some(import.simai_file);
                    // Find default (max) level
                    app.import_selected_level =
                        import.levels.iter().map(|(lv, _)| *lv).max().unwrap_or(0);
                    app.set_chart(import.chart);
                    app.set_selected_note(None);
                    app.set_editing_slide_path(None);
                    if let (Some(bytes), Some(ext)) = (&import.audio_bytes, &import.audio_ext) {
                        if let Some(pcm) = audio::load_audio_from_bytes(bytes, ext) {
                            app.audio_source_name = Some(import.title.clone());
                            app.audio_wav_pcm = Some(pcm);
                            app.audio_cache.clear();
                            app.request_audio_start();
                        }
                    }
                    app.set_status(format!("Opened {} ({n} notes)", import.title));
                }
                Err(e) if e == "cancelled" => {}
                Err(e) => app.set_status(format!("Import: {e}")),
            }
        }

        // Update and draw toast notifications
        app.toasts.update();
        app.toasts.draw();

        next_frame().await;
    }
}

pub fn load_mask_material() -> Result<macroquad::material::Material, String> {
    use macroquad::material::{MaterialParams, load_material};
    use macroquad::prelude::{ShaderSource, UniformDesc, UniformType};

    let vertex = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying vec2 uv;
varying vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
    color = color0;
}"#;

    load_material(
        ShaderSource::Glsl {
            vertex,
            fragment: include_str!("mask.frag"),
        },
        MaterialParams {
            uniforms: vec![UniformDesc::new("progress", UniformType::Float1)],
            pipeline_params: macroquad::miniquad::PipelineParams {
                color_blend: Some(macroquad::miniquad::BlendState::new(
                    macroquad::miniquad::Equation::Add,
                    macroquad::miniquad::BlendFactor::Value(
                        macroquad::miniquad::BlendValue::SourceAlpha,
                    ),
                    macroquad::miniquad::BlendFactor::OneMinusValue(
                        macroquad::miniquad::BlendValue::SourceAlpha,
                    ),
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .map_err(|e| format!("{e:?}"))
}
