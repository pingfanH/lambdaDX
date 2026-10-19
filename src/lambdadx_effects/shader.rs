use macroquad::material::{Material, MaterialParams, load_material};
use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};
use macroquad::prelude::{ShaderSource, UniformDesc, UniformType, Vec3};

use super::data::RenderDefinition;
use super::simulation::Judgment;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 normal;
varying vec2 vUV;
varying vec4 vColor;
varying vec2 vScreen;
uniform mat4 Model;
uniform mat4 Projection;
uniform vec3 u_center_zoom;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    vUV = texcoord;
    vColor = normal;
    vec2 world = (position.xy - u_center_zoom.xy) / u_center_zoom.z;
    vScreen = vec2((world.x + 540.0) / 1080.0, (-world.y + 540.0) / 1920.0);
}"#;

const COMMON: &str = r#"#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying vec2 vUV;
varying vec4 vColor;
varying vec2 vScreen;
uniform sampler2D Texture;
uniform sampler2D Texture2;
uniform vec4 uST;
uniform vec4 uST2;
uniform vec4 uColor;
uniform float uExposure;
uniform float uOpacity;
"#;

const RATE: &str = r#"
void main() {
    vec4 t = texture2D(Texture, vUV * uST.xy + uST.zw) * vColor;
    gl_FragColor = vec4(t.rgb * uColor.rgb, t.a * uColor.a * exp2(uExposure) * uOpacity);
}"#;

const UVSHIFT: &str = r#"
uniform float uPaletteShift;
uniform float uPaletteGreenScale;
void main() {
    vec3 c = vColor.rgb;
    vec4 p = c.g >= c.b ? vec4(c.g, c.b, 0.0, -1.0 / 3.0) : vec4(c.b, c.g, -1.0, 2.0 / 3.0);
    vec4 q = c.r >= p.x ? vec4(c.r, p.y, p.z, p.x) : vec4(p.x, p.y, p.w, c.r);
    float d = q.x - min(q.w, q.y);
    float sat = q.x > 0.0 ? d / q.x : 0.0;
    float h = d > 0.0 ? abs(q.z + (q.w - q.y) / (6.0 * d)) : 0.0;
    float a = h >= 0.25 ? 0.0 : 1.0;
    float y = (h >= 0.152778 ? 1.0 : 0.0) + a;
    float z = (0.833333 >= h ? 0.0 : 1.0) + (h >= 0.152778 ? 0.0 : 1.0);
    float shift = 0.0 >= z ? -0.75 : -0.25;
    shift = y == 2.0 ? -0.5 : shift;
    shift = sat >= 0.25 ? shift : 0.0;
    // Judgment is explicit: orange Perfect must not enter the pink hue bucket.
    // Raw retains the original color-driven atlas selection.
    shift = uPaletteShift >= 0.0 ? -uPaletteShift : shift;
    vec4 t = texture2D(Texture, vUV * uST.xy + uST.zw + vec2(0.0, -shift));
    // Recolor only the yellow component; neutral highlights remain white.
    t.g = mix(t.b, t.g, uPaletteGreenScale);
    float e = exp2(uExposure);
    float alpha = t.a * vColor.a * uColor.a * uOpacity;
    gl_FragColor = vec4(t.rgb * uColor.rgb * min(e, 1.0), min(max(e, 1.0), 1048576.0) * alpha);
}"#;

const SCREEN: &str = r#"
void main() {
    vec4 t = texture2D(Texture, vUV * uST.xy + uST.zw) * vColor * uColor;
    float k = t.a * exp2(uExposure) * uOpacity;
    gl_FragColor = vec4(t.rgb * k, 1.0);
}"#;

const ADD2: &str = r#"
void main() {
    vec4 t0 = texture2D(Texture, vUV * uST.xy + uST.zw);
    vec4 t1 = texture2D(Texture2, vUV * uST2.xy + uST2.zw);
    vec3 rgb = t0.rgb * t1.rgb * vColor.rgb * uColor.rgb;
    rgb *= t0.a * t1.a * vColor.a * uColor.a * exp2(uExposure) * uOpacity;
    gl_FragColor = vec4(rgb, 1.0);
}"#;

const RAINBOW: &str = r#"
uniform vec3 uCol[6];
uniform float uRepeat;
uniform float uCount;
uniform float uRotateSpeed;
uniform float uRotateFloor;
uniform float uInKey;
uniform float uOutKey;
uniform float uFalloff;
uniform float uTime;
void main() {
    vec2 p = vUV * 2.0 - 1.0;
    float ang = (atan(p.y, p.x) + 3.141593) * uRepeat;
    float fy = fract(ang * 0.159155);
    float fz = fract(ang * 0.795775);
    float fx = fract(ang * 0.795775 + 0.03);
    float rot = uRotateSpeed * uTime;
    if (uRotateFloor > 0.5) rot = floor(rot);
    float inv = 1.0 / floor(uCount);
    float t = fract(rot * inv + fy);
    vec3 col = uCol[5];
    if (t < 5.0 * inv) col = uCol[4];
    if (t < 4.0 * inv) col = uCol[3];
    if (t < 3.0 * inv) col = uCol[2];
    if (t < 2.0 * inv) col = uCol[1];
    if (t < inv) col = uCol[0];
    bool stripeX = 0.56 >= fx;
    bool stripeZ = 0.5 >= fz;
    vec2 sp = fract((vScreen * 2.0 - 1.0) * 64.0) - 0.5;
    float dotv = 1.0 - length(sp);
    dotv *= dotv;
    float va = vColor.a;
    float inK = clamp((1.0 - va) / uInKey, 0.0, 1.0);
    float r = length(vUV - 0.5);
    float w = fract(r * 4.0 - 2.0 * (1.0 - va));
    float m = 0.8 >= w ? w * w * 1.125 + 0.1 : (w - 0.8) * -25.0 + 1.0;
    m = 0.82 >= w ? m : (w - 0.82) * -2.777778 + 0.5;
    m *= 1.0 >= w ? 1.0 : 0.0;
    float pat = 1.0 - clamp((dotv * m - 0.1) * 24.0 + 0.1, 0.0, 1.0);
    float mixv = stripeX ? 1.0 : pat;
    float f = abs(r - 0.25) * 3.0 + 0.25;
    vec3 c1 = mix(col, col * f, mixv);
    vec3 rgb = stripeZ ? vec3(f) : c1;
    float a1 = clamp((r - inK) * -3.0 - 0.5, 0.0, 1.0);
    float oz = clamp(va / uOutKey, 0.0, 1.0);
    a1 *= clamp((oz + r) * 2.0 - 1.0, 0.0, 1.0);
    a1 *= 0.5 >= r ? (stripeZ ? 0.5 : 1.0) : 0.0;
    float fo = uFalloff * 0.2499 + 0.25;
    a1 *= clamp(-(r - fo) / (0.5 - fo) + 1.0, 0.0, 1.0);
    gl_FragColor = vec4(rgb, a1 * 0.75);
}"#;

pub struct EffectShaders {
    rate: Material,
    screen: Material,
    uvshift: Material,
    add2: Material,
    rainbow: Material,
}

impl EffectShaders {
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            rate: make_material(RATE, false, false, BlendMode::Alpha)?,
            screen: make_material(SCREEN, false, false, BlendMode::Screen)?,
            uvshift: make_material(UVSHIFT, false, false, BlendMode::Alpha)?,
            add2: make_material(ADD2, true, false, BlendMode::Additive)?,
            rainbow: make_material(RAINBOW, false, true, BlendMode::Alpha)?,
        })
    }

    pub fn configure(&self, render: &RenderDefinition, age: f32, judgment: Judgment) -> &Material {
        let material = match render.kind.as_str() {
            "rainbow" => &self.rainbow,
            "uvshift" => &self.uvshift,
            "add2" => &self.add2,
            "screen" => &self.screen,
            _ => &self.rate,
        };
        material.set_uniform("uExposure", render.exposure);
        material.set_uniform("uOpacity", render.opacity);
        material.set_uniform("uST", render.st);
        material.set_uniform("uColor", render.color);
        if render.kind == "uvshift" {
            material.set_uniform("uPaletteShift", judgment.palette_shift());
            material.set_uniform("uPaletteGreenScale", judgment.palette_green_scale());
        }
        if render.kind == "add2" {
            material.set_uniform("uST2", render.st2);
        }
        if render.kind == "rainbow" {
            let params = render.rainbow.as_ref();
            let number = |key: &str, default: f32| {
                params
                    .and_then(|p| p[key].as_f64())
                    .map(|n| n as f32)
                    .unwrap_or(default)
            };
            let colors = std::array::from_fn::<_, 6, _>(|i| {
                Vec3::from_array(std::array::from_fn(|c| {
                    params
                        .and_then(|p| p["colors"][i][c].as_f64())
                        .unwrap_or(1.0) as f32
                }))
            });
            material.set_uniform_array("uCol", &colors);
            material.set_uniform("uRepeat", number("repeat", 3.0));
            material.set_uniform("uCount", number("count", 5.0));
            material.set_uniform("uRotateSpeed", number("rotateSpeed", 9.0));
            material.set_uniform(
                "uRotateFloor",
                if params
                    .and_then(|p| p["rotateFloor"].as_bool())
                    .unwrap_or(true)
                {
                    1.0f32
                } else {
                    0.0
                },
            );
            material.set_uniform("uInKey", number("inKey", 0.2));
            material.set_uniform("uOutKey", number("outKey", 0.4));
            material.set_uniform("uFalloff", number("falloff", 0.66));
            material.set_uniform("uTime", age);
        }
        material
    }
}

#[derive(Clone, Copy)]
enum BlendMode {
    Alpha,
    Screen,
    Additive,
}

fn make_material(
    body: &str,
    second_texture: bool,
    rainbow: bool,
    blend: BlendMode,
) -> Result<Material, String> {
    let (source, destination) = match blend {
        BlendMode::Alpha => (
            BlendFactor::Value(BlendValue::SourceAlpha),
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        ),
        BlendMode::Screen => (
            BlendFactor::One,
            BlendFactor::OneMinusValue(BlendValue::SourceColor),
        ),
        BlendMode::Additive => (BlendFactor::One, BlendFactor::One),
    };
    let mut uniforms = vec![
        UniformDesc::new("u_center_zoom", UniformType::Float3),
        UniformDesc::new("uExposure", UniformType::Float1),
        UniformDesc::new("uOpacity", UniformType::Float1),
        UniformDesc::new("uST", UniformType::Float4),
        UniformDesc::new("uColor", UniformType::Float4),
    ];
    let mut textures = Vec::new();
    if body == UVSHIFT {
        uniforms.push(UniformDesc::new("uPaletteShift", UniformType::Float1));
        uniforms.push(UniformDesc::new("uPaletteGreenScale", UniformType::Float1));
    }
    if second_texture {
        uniforms.push(UniformDesc::new("uST2", UniformType::Float4));
        textures.push("Texture2".to_string());
    }
    if rainbow {
        uniforms.push(UniformDesc::new("uCol", UniformType::Float3).array(6));
        for name in [
            "uRepeat",
            "uCount",
            "uRotateSpeed",
            "uRotateFloor",
            "uInKey",
            "uOutKey",
            "uFalloff",
            "uTime",
        ] {
            uniforms.push(UniformDesc::new(name, UniformType::Float1));
        }
    }
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: &format!("{COMMON}{body}"),
        },
        MaterialParams {
            uniforms,
            textures,
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(Equation::Add, source, destination)),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                ..Default::default()
            },
        },
    )
    .map_err(|error| format!("cannot create effect shader: {error:?}"))
}
