use macroquad::miniquad::conf::{LinuxBackend, Platform};
use macroquad::prelude::*;
use macroquad::texture::{DrawTextureParams, Texture2D};

use super::types::RectF;

/// Window / backend configuration for the standalone pad preview.
pub fn window_conf() -> Conf {
    Conf {
        window_title: "LambdaDX Pad Preview".to_string(),
        window_width: 1280,
        window_height: 760,
        high_dpi: true,
        sample_count: 4,
        platform: Platform {
            linux_backend: LinuxBackend::WaylandWithX11Fallback,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Load the circular-sweep mask shader used by the touch-hold progress border.
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

/// Point-in-rect test for screen-space UI buttons.
pub fn rect_contains(r: RectF, p: Vec2) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

/// Draw a 9-slice hold body from `from` (head end) to `to` (tail end). Caps are
/// always kept at their natural size, so a zero-length segment (head == tail at
/// spawn, body 0) still renders the head cap as a blob scaling with `width`
/// instead of a squeezed point. `fallback_dir` (the outward ray) orients the
/// caps when the segment is degenerate during the approach.
pub fn draw_hold_9slice_segment(
    tex: &Texture2D,
    from: Vec2,
    to: Vec2,
    width: f32,
    tint: Color,
    fallback_dir: Vec2,
) {
    let delta = to - from;
    let len = delta.length();
    let dir = if len > 1e-3 {
        delta / len
    } else {
        (-fallback_dir).normalize_or_zero()
    };
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_len = (cap_h * (width / tex_w)).max(1.0);

    let min_cap = 4.0;
    let natural_cap_len = cap_len.max(min_cap);
    let (head_len, tail_len) = (natural_cap_len, natural_cap_len);
    let head_start = from - dir * head_len;
    let tail_start = to;
    let body_start = head_start + dir * head_len;
    let body_delta = tail_start - body_start;
    let body_len = body_delta.dot(dir).max(0.0);

    let draw_part = |start: Vec2, part_len: f32, src_y: f32, src_h: f32| {
        if part_len <= 0.0 {
            return;
        }
        let center = start + dir * (part_len * 0.5);
        draw_texture_ex(
            tex,
            center.x - width * 0.5,
            center.y - part_len * 0.5,
            tint,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: src_y,
                    w: tex_w,
                    h: src_h,
                }),
                dest_size: Some(vec2(width, part_len)),
                rotation: angle,
                pivot: Some(center),
                ..Default::default()
            },
        );
    };

    draw_part(tail_start, tail_len, tex_h - cap_h, cap_h);
    draw_part(body_start, body_len, cap_h, body_src_h);
    draw_part(head_start, head_len, 0.0, cap_h);
}
