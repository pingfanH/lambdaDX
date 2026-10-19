use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EffectDefinition {
    pub name: String,
    pub systems: Vec<SystemDefinition>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SystemDefinition {
    pub name: String,
    #[serde(rename = "m")]
    pub matrix: [f32; 6],
    pub rot: f32,
    pub duration: f32,
    pub looping: bool,
    #[serde(rename = "simSpeed")]
    pub sim_speed: f32,
    pub delay: Value,
    pub lifetime: Value,
    pub speed: Value,
    pub size: Value,
    pub rotation: Value,
    #[serde(rename = "startColor")]
    pub start_color: Value,
    #[serde(rename = "maxParticles")]
    pub max_particles: usize,
    pub shape: Option<ShapeDefinition>,
    pub rate: Value,
    pub bursts: Vec<BurstDefinition>,
    #[serde(rename = "sizeOL")]
    pub size_over_lifetime: Value,
    #[serde(rename = "rotOL")]
    pub rotation_over_lifetime: Value,
    #[serde(rename = "colorOL")]
    pub color_over_lifetime: Value,
    pub uv: Option<UvDefinition>,
    pub limit: Option<LimitDefinition>,
    pub render: Option<RenderDefinition>,
}

impl Default for SystemDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            matrix: [100.0, 0.0, 0.0, 100.0, 0.0, 0.0],
            rot: 0.0,
            duration: 1.0,
            looping: false,
            sim_speed: 1.0,
            delay: Value::Null,
            lifetime: Value::Null,
            speed: Value::Null,
            size: Value::Null,
            rotation: Value::Null,
            start_color: Value::Null,
            max_particles: 64,
            shape: None,
            rate: Value::Null,
            bursts: Vec::new(),
            size_over_lifetime: Value::Null,
            rotation_over_lifetime: Value::Null,
            color_over_lifetime: Value::Null,
            uv: None,
            limit: None,
            render: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LimitDefinition {
    pub magnitude: Value,
    pub dampen: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ShapeDefinition {
    #[serde(rename = "type")]
    pub shape_type: i32,
    pub radius: f32,
    #[serde(rename = "radiusThickness")]
    pub radius_thickness: f32,
    pub arc: f32,
    #[serde(rename = "arcMode")]
    pub arc_mode: i32,
    #[serde(rename = "arcSpread")]
    pub arc_spread: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct BurstDefinition {
    pub time: f32,
    pub count: Value,
    pub cycles: usize,
    pub interval: f32,
    pub probability: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct UvDefinition {
    #[serde(rename = "tilesX")]
    pub tiles_x: u32,
    #[serde(rename = "tilesY")]
    pub tiles_y: u32,
    pub frame: Value,
    #[serde(rename = "startFrame")]
    pub start_frame: Value,
    pub cycles: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RenderDefinition {
    pub mode: u32,
    pub align: i32,
    #[serde(rename = "lengthScale")]
    pub length_scale: f32,
    #[serde(rename = "velocityScale")]
    pub velocity_scale: f32,
    #[serde(rename = "maxSize")]
    pub max_size: f32,
    pub order: i32,
    pub name: String,
    pub shader: String,
    pub kind: String,
    pub texture: Option<String>,
    #[serde(rename = "texture2")]
    pub texture2: Option<String>,
    pub st: [f32; 4],
    pub st2: [f32; 4],
    pub color: [f32; 4],
    pub exposure: f32,
    pub opacity: f32,
    pub rainbow: Option<Value>,
}

impl Default for RenderDefinition {
    fn default() -> Self {
        Self {
            mode: 0,
            align: 0,
            length_scale: 1.0,
            velocity_scale: 0.0,
            max_size: 100.0,
            order: 0,
            name: String::new(),
            shader: String::new(),
            kind: String::new(),
            texture: None,
            texture2: None,
            st: [1.0, 1.0, 0.0, 0.0],
            st2: [1.0, 1.0, 0.0, 0.0],
            color: [1.0; 4],
            exposure: 0.0,
            opacity: 1.0,
            rainbow: None,
        }
    }
}

pub fn parse_effects(json: &str) -> Result<Vec<EffectDefinition>, String> {
    let definitions: std::collections::BTreeMap<String, EffectDefinition> =
        serde_json::from_str(json).map_err(|error| format!("invalid effects.json: {error}"))?;
    Ok(definitions.into_values().collect())
}

pub fn scalar_value(node: &Value, t: f32, random: f32) -> f32 {
    let mode = node.get("s").and_then(Value::as_i64).unwrap_or(0);
    let max = node.get("k").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let curve = |key| {
        node.get(key)
            .and_then(Value::as_array)
            .map(|keys| sample_scalar_curve(keys, t))
            .unwrap_or(1.0)
    };
    match mode {
        1 => curve("c") * max,
        2 => (curve("cm") + (curve("c") - curve("cm")) * random) * max,
        3 => {
            let min = node.get("mn").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            min + (max - min) * random
        }
        _ => max,
    }
}

pub fn sample_scalar_curve(keys: &[Value], t: f32) -> f32 {
    let Some(first) = keys.first() else {
        return 1.0;
    };
    if t <= key_number(first, 0) {
        return key_number(first, 1);
    }
    if let Some(last) = keys.last()
        && t >= key_number(last, 0)
    {
        return key_number(last, 1);
    }
    for pair in keys.windows(2) {
        let (left, right) = (&pair[0], &pair[1]);
        let t0 = key_number(left, 0);
        let t1 = key_number(right, 0);
        if t <= t1 {
            let dt = (t1 - t0).max(1e-5);
            let u = ((t - t0) / dt).clamp(0.0, 1.0);
            let v0 = key_number(left, 1);
            let v1 = key_number(right, 1);
            if left.get(3).and_then(Value::as_f64).is_none()
                || right.get(2).and_then(Value::as_f64).is_none()
            {
                return v0;
            }
            let m0 = key_number(left, 3);
            let m1 = key_number(right, 2);
            let u2 = u * u;
            let u3 = u2 * u;
            return (2.0 * u3 - 3.0 * u2 + 1.0) * v0
                + (u3 - 2.0 * u2 + u) * dt * m0
                + (-2.0 * u3 + 3.0 * u2) * v1
                + (u3 - u2) * dt * m1;
        }
    }
    keys.last().map(|key| key_number(key, 1)).unwrap_or(0.0)
}

pub fn color_value(node: &Value, t: f32, random: f32) -> [f32; 4] {
    let constant = |key| {
        node.get(key)
            .and_then(Value::as_array)
            .map(|values| rgba_array(values))
            .unwrap_or([1.0; 4])
    };
    let gradient = |key, time| sample_gradient(&node[key], time);
    match node.get("s").and_then(Value::as_i64).unwrap_or(0) {
        0 => constant("c1"),
        1 => gradient("g1", t),
        2 => mix_color(constant("c0"), constant("c1"), random),
        3 => mix_color(gradient("g0", t), gradient("g1", t), random),
        4 => gradient("g1", random),
        _ => [1.0; 4],
    }
}

fn mix_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn sample_gradient(gradient: &Value, t: f32) -> [f32; 4] {
    let fixed = gradient
        .get("fixed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let rgb = gradient
        .get("c")
        .and_then(Value::as_array)
        .map(|keys| std::array::from_fn(|i| sample_gradient_channel(keys, t, i + 1, fixed)))
        .unwrap_or([1.0, 1.0, 1.0]);
    let alpha = gradient
        .get("a")
        .and_then(Value::as_array)
        .map(|keys| sample_gradient_channel(keys, t, 1, fixed))
        .unwrap_or(1.0);
    [rgb[0], rgb[1], rgb[2], alpha]
}

fn sample_gradient_channel(keys: &[Value], t: f32, channel: usize, fixed: bool) -> f32 {
    let Some(first) = keys.first() else {
        return 1.0;
    };
    if t <= key_number(first, 0) {
        return key_number(first, channel);
    }
    for pair in keys.windows(2) {
        let (left, right) = (&pair[0], &pair[1]);
        let t0 = key_number(left, 0);
        let t1 = key_number(right, 0);
        if t <= t1 {
            let p = ((t - t0) / (t1 - t0).max(1e-5)).clamp(0.0, 1.0);
            let a = key_number(left, channel);
            let b = key_number(right, channel);
            return if fixed { b } else { a + (b - a) * p };
        }
    }
    key_number(keys.last().unwrap_or(first), channel)
}

fn rgba_array(values: &[Value]) -> [f32; 4] {
    [
        value_number(values.first(), 1.0),
        value_number(values.get(1), 1.0),
        value_number(values.get(2), 1.0),
        value_number(values.get(3), 1.0),
    ]
}

fn key_number(key: &Value, index: usize) -> f32 {
    key.as_array()
        .and_then(|values| values.get(index))
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32
}

fn value_number(value: Option<&Value>, default: f32) -> f32 {
    value
        .and_then(Value::as_f64)
        .map(|n| n as f32)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extracted_library_and_all_systems() {
        let path = format!(
            "{}/assets/extracted/lambdadx_replay_1600_master/effects/effects.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let json = std::fs::read_to_string(path).expect("extracted effects.json");
        let effects = parse_effects(&json).expect("parse effects");
        assert_eq!(effects.len(), 29);
        assert_eq!(
            effects
                .iter()
                .map(|effect| effect.systems.len())
                .sum::<usize>(),
            147
        );
        assert!(
            effects
                .iter()
                .any(|effect| effect.name == "FX_GAM_Notes_Tap_00")
        );
    }

    #[test]
    fn scalar_nodes_sample_constant_ranges_and_curves() {
        assert_eq!(
            scalar_value(&serde_json::json!({"s":0,"k":2.0}), 0.5, 0.2),
            2.0
        );
        assert_eq!(
            scalar_value(&serde_json::json!({"s":3,"mn":1.0,"k":3.0}), 0.0, 0.25),
            1.5
        );
        let curve = serde_json::json!([[0.0, 0.0, 0.0, 0.0], [1.0, 2.0, 0.0, 0.0]]);
        assert!((sample_scalar_curve(curve.as_array().unwrap(), 0.5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn curve_multiplier_and_two_curve_range_match_source_renderer() {
        let node = serde_json::json!({
            "s":2, "k":10.0,
            "cm":[[0,0.2,0,0],[1,0.2,0,0]],
            "c":[[0,1,0,0],[1,1,0,0]]
        });
        assert!((scalar_value(&node, 0.5, 0.25) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn gradient_alpha_uses_linear_interpolation() {
        let node = serde_json::json!({"s":1,"g1":{
            "a":[[0,1],[1,0]], "c":[[0,1,1,1],[1,1,1,1]]
        }});
        assert!((color_value(&node, 0.25, 0.5)[3] - 0.75).abs() < 1e-5);
    }
}
