mod audio;
mod beat_format;
mod chart;
mod egui_ui;
mod input;
mod pad_svg;
mod platform;
mod simai_io;
mod slide_match;
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
    match load_sound_from_bytes(include_bytes!("../../assets/Sfx/tap_perfect.wav")).await {
        Ok(s) => { app.hit_sound = Some(s); }
        Err(e) => app.status = format!("Failed to load tap_perfect.wav: {e:?}"),
    }
    match load_sound_from_bytes(include_bytes!("../../assets/Sfx/touch.wav")).await {
        Ok(s) => { app.touch_sound = Some(s); }
        Err(e) => app.status = format!("Failed to load touch.wav: {e:?}"),
    }
    match load_sound_from_bytes(include_bytes!("../../assets/Sfx/slide.wav")).await {
        Ok(s) => { app.slide_sound = Some(s); }
        Err(e) => app.status = format!("Failed to load slide.wav: {e:?}"),
    }
    match load_sound_from_bytes(include_bytes!("../../assets/Sfx/touch_Hold_riser.wav")).await {
        Ok(s) => { app.touch_riser_sound = Some(s); }
        Err(e) => app.status = format!("Failed to load touch_Hold_riser.wav: {e:?}"),
    }

    // Load mask shader material
    match load_mask_material() {
        Ok(m) => app.mask_material = Some(m),
        Err(e) => app.status = format!("Shader: {e}"),
    }

    ui::load_note_textures(&mut app).await;
    audio::warm_audio_cache(&mut app, 1.0).await;

    loop {
        clear_background(Color::from_rgba(10, 17, 30, 255));

        // Layout: timeline (left) + pad (right), below toolbar
        let layout = ui::compute_layout(&app);
        let pad_geom = ui::compute_pad_geom(layout.pad);
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
            egui_ctx.set_pixels_per_point(2.0);
            egui_ui::draw_egui_ui(egui_ctx, &mut app);
        });
        egui_macroquad::draw();

        next_frame().await;
    }
}

fn load_mask_material() -> Result<macroquad::material::Material, String> {
    use macroquad::material::{load_material, MaterialParams};
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
                    macroquad::miniquad::BlendFactor::Value(macroquad::miniquad::BlendValue::SourceAlpha),
                    macroquad::miniquad::BlendFactor::OneMinusValue(macroquad::miniquad::BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .map_err(|e| format!("{e:?}"))
}
