mod ffi_test;

use macroquad::prelude::*;
use macroquad_sim::app::types::zone::PadZone;

#[macroquad::main("Mask")]
async fn main() {
    let tex = load_texture("assets/Skins/classic/touchhold_border.png")
        .await
        .unwrap();

    let material = load_material(
        ShaderSource::Glsl {
            vertex: r#"
                #version 100

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
                }
            "#,
            fragment: include_str!("../mask.frag"),
        },
        MaterialParams {
            pipeline_params: macroquad::miniquad::PipelineParams {
                color_blend: Some(macroquad::miniquad::BlendState::new(
                    macroquad::miniquad::Equation::Add,
                    macroquad::miniquad::BlendFactor::Value(macroquad::miniquad::BlendValue::SourceAlpha),
                    macroquad::miniquad::BlendFactor::OneMinusValue(macroquad::miniquad::BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("progress", UniformType::Float1),
            ],
            textures: vec![],
            ..Default::default()
        },
    )
        .unwrap();

    let mut progress = 0.0f32;

    loop {
        clear_background(Color::from_rgba(15, 23, 42, 255));

        progress += get_frame_time() * 0.2;
        if progress > 1.0 {
            progress = 0.0;
        }

        let x = 100.0;
        let y = 100.0;
        let size = 300.0;

        // Ghost ring: always visible, normal alpha blending via default material
        draw_texture_ex(
            &tex,
            x,
            y,
            Color::from_rgba(255, 255, 255, 0),
            DrawTextureParams {
                dest_size: Some(vec2(size, size)),
                ..Default::default()
            },
        );

        // Progress sweep: shader-based clockwise reveal with alpha blending
        gl_use_material(&material);
        material.set_uniform("progress", progress);
        draw_texture_ex(
            &tex,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(size, size)),
                ..Default::default()
            },
        );
        gl_use_default_material();

        draw_text(
            &format!("Progress: {:.2}", progress),
            x + 10.0,
            y + size + 30.0,
            24.0,
            WHITE,
        );

        next_frame().await;
    }
}

#[test]
fn test() {
    let zone:i8 = 3;
    let fin = (zone - 1).rem_euclid(8) as u8 +9;
    let zone =  PadZone::num_to_b(zone);
    println!("{}", zone);
}
