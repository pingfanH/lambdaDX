use macroquad::prelude::{Vec2, vec2};
use serde_json::Value;

use super::data::{EffectDefinition, SystemDefinition, color_value, scalar_value};

const STEP: f32 = 1.0 / 60.0;

#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Judgment {
    Good,
    Great,
    Perfect,
    #[default]
    CriticalPerfect,
    Raw,
}

impl Judgment {
    pub const ALL: [Self; 5] = [
        Self::Good,
        Self::Great,
        Self::Perfect,
        Self::CriticalPerfect,
        Self::Raw,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "Good",
            Self::Great => "Great",
            Self::Perfect => "Perfect",
            Self::CriticalPerfect => "Critical Perfect",
            Self::Raw => "Raw",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "good" => Some(Self::Good),
            "great" => Some(Self::Great),
            "perfect" => Some(Self::Perfect),
            "critical" | "critical-perfect" | "critical_perfect" | "critical perfect" | "cp" => {
                Some(Self::CriticalPerfect)
            }
            "raw" | "none" => Some(Self::Raw),
            _ => None,
        }
    }

    pub fn color(self) -> Option<[f32; 4]> {
        // Perfect/Great/Good RGBs match the corresponding UI_GAM judgment sprites.
        // Keep the existing replay yellow unchanged for Critical Perfect.
        match self {
            Self::CriticalPerfect => Some([1.0, 1.0, 0.0, 1.0]),
            Self::Perfect => Some([1.0, 157.0 / 255.0, 0.0, 1.0]),
            Self::Great => Some([1.0, 114.0 / 255.0, 178.0 / 255.0, 1.0]),
            Self::Good => Some([47.0 / 255.0, 202.0 / 255.0, 76.0 / 255.0, 1.0]),
            Self::Raw => None,
        }
    }

    pub(super) fn palette_shift(self) -> f32 {
        match self {
            Self::Good => 0.75,
            Self::Great => 0.25,
            Self::Perfect | Self::CriticalPerfect => 0.5,
            Self::Raw => -1.0,
        }
    }

    pub(super) fn palette_green_scale(self) -> f32 {
        if self == Self::Perfect {
            157.0 / 255.0
        } else {
            1.0
        }
    }

    fn includes(self, system: &SystemDefinition) -> bool {
        let grade = if self == Self::CriticalPerfect {
            Self::Perfect
        } else {
            self
        };
        self == Self::Raw
            || system
                .render
                .as_ref()
                .is_some_and(|render| render.name.contains("_Star_Break"))
            || [Self::Good, Self::Great, Self::Perfect]
                .into_iter()
                .find(|judge| system.name.contains(judge.label()))
                .is_none_or(|judge| judge == grade)
    }
}

#[derive(Clone, Debug)]
pub struct EffectInstance {
    pub(super) definition: EffectDefinition,
    seed: u32,
    pub judgment: Judgment,
    pub elapsed: f32,
}

impl EffectInstance {
    pub(crate) fn new(effect: &EffectDefinition, seed: u64) -> Self {
        Self {
            definition: effect.clone(),
            seed: seed as u32,
            judgment: Judgment::default(),
            elapsed: 0.0,
        }
    }

    pub fn reset(&mut self, seed: u64) {
        self.elapsed = 0.0;
        self.seed = seed as u32;
    }

    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt.max(0.0);
    }

    pub fn system_count(&self) -> usize {
        self.definition.systems.len()
    }

    pub fn particle_count(&self) -> usize {
        (0..self.system_count())
            .map(|index| self.sample_system(index).len())
            .sum()
    }

    pub fn duration(&self) -> f32 {
        self.definition
            .systems
            .iter()
            .map(|system| {
                if system.looping {
                    return f32::INFINITY;
                }
                (maximum(&system.delay) + system.duration + maximum(&system.lifetime))
                    / if system.sim_speed == 0.0 {
                        1.0
                    } else {
                        system.sim_speed
                    }
            })
            .fold(0.25, f32::max)
    }

    pub(super) fn sample_system(&self, index: usize) -> Vec<ParticleQuad> {
        let system = &self.definition.systems[index];
        let Some(render) = system.render.as_ref() else {
            return Vec::new();
        };
        if !self.judgment.includes(system) || (render.texture.is_none() && render.kind != "rainbow")
        {
            return Vec::new();
        }
        let random = |id, property| random(self.seed, index as u32, id, property);
        let time = self.elapsed * system.sim_speed - scalar_value(&system.delay, 0.0, random(7, 7));
        if time < 0.0 {
            return Vec::new();
        }
        emission_events(system, time, self.seed, index as u32)
            .into_iter()
            .filter_map(|event| {
                let age = time - event.time;
                let sample =
                    |node: &Value, t, property| scalar_value(node, t, random(event.id, property));
                let lifetime = sample(&system.lifetime, 0.0, 1);
                if age < 0.0 || age >= lifetime {
                    return None;
                }
                let life = age / lifetime;
                let (mut local, direction) = if let Some(shape) = &system.shape {
                    let angle = (shape.arc
                        * if shape.arc_mode == 3 {
                            event.index as f32 / event.count.max(1) as f32
                        } else {
                            random(event.id, 2)
                        })
                    .to_radians();
                    let direction = vec2(angle.cos(), angle.sin());
                    (
                        direction
                            * shape.radius
                            * (1.0 - shape.radius_thickness * random(event.id, 3)),
                        direction,
                    )
                } else {
                    (Vec2::ZERO, Vec2::Y)
                };
                let mut speed = sample(&system.speed, 0.0, 4);
                let distance = if let Some(limit) = &system.limit {
                    let damping = 1.0 - (1.0 - limit.dampen).powf(STEP * 30.0);
                    integrate(age, |time| {
                        let magnitude = sample(&limit.magnitude, time / lifetime, 5);
                        if speed.abs() > magnitude {
                            speed += (speed.signum() * magnitude - speed) * damping;
                        }
                        speed
                    })
                } else {
                    speed * age
                };
                let reference_offset = super::reference_motion::radial_offset(
                    &self.definition.name,
                    &system.name,
                    age,
                );
                local += direction * (distance + reference_offset);
                let size = sample(&system.size, 0.0, 6);
                let mut dimensions = Vec2::splat(size);
                if !system.size_over_lifetime.is_null() {
                    let x = sample(&system.size_over_lifetime["x"], life, 8);
                    let y = if system.size_over_lifetime["y"].is_null() {
                        x
                    } else {
                        sample(&system.size_over_lifetime["y"], life, 8)
                    };
                    dimensions *= vec2(x, y);
                }
                if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
                    return None;
                }
                let mut rotation = sample(&system.rotation, 0.0, 9);
                if !system.rotation_over_lifetime.is_null() {
                    rotation += match system.rotation_over_lifetime["s"].as_i64() {
                        Some(0 | 3) => sample(&system.rotation_over_lifetime, 0.0, 10) * age,
                        _ => integrate(age, |t| {
                            sample(&system.rotation_over_lifetime, t / lifetime, 10)
                        }),
                    };
                }
                let mut color = self.judgment.color().unwrap_or_else(|| {
                    color_value(
                        &system.start_color,
                        event.time / system.duration.max(1e-6),
                        random(event.id, 11),
                    )
                });
                if !system.color_over_lifetime.is_null() {
                    let over = color_value(&system.color_over_lifetime, life, random(event.id, 12));
                    for i in 0..4 {
                        color[i] *= over[i];
                    }
                }
                if color[3] <= 0.001 {
                    return None;
                }
                let m = system.matrix;
                let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                let mut position = transform_vector(m, local) + vec2(m[4], m[5]);
                dimensions = (dimensions * scale).min(Vec2::splat(render.max_size * 1920.0));
                let angle = if render.mode == 1 {
                    let velocity = transform_vector(m, direction);
                    // Unity stretches local Y along velocity. The website stretches X,
                    // which prevents Hold Release's eight stars from regaining their shape.
                    let unstretched_length = dimensions.y;
                    dimensions.y = dimensions.y * render.length_scale
                        + speed.abs() * scale * render.velocity_scale;
                    // Keep the leading edge fixed and put the extra length behind it.
                    // Centered stretching pulls Hold Release's four arms apart too early.
                    position -=
                        velocity.normalize_or_zero() * (dimensions.y - unstretched_length) * 0.5;
                    velocity.y.atan2(velocity.x) - std::f32::consts::FRAC_PI_2
                } else {
                    (if render.align == 2 {
                        system.rot.to_radians()
                    } else {
                        0.0
                    }) - rotation
                };
                // Scale the entire four-star group about the effect origin, including
                // particle positions, so this also reduces the spacing between stars.
                let group_scale =
                    super::reference_motion::group_scale(&self.definition.name, &system.name);
                position *= group_scale;
                dimensions *= group_scale;
                let uv = uv_rect(system, life, random(event.id, 13), random(event.id, 14));
                let stretched_rotation = if render.mode == 1 {
                    super::reference_motion::velocity_basis_rotation(
                        &self.definition.name,
                        &system.name,
                        age,
                        lifetime,
                        &system.rotation_over_lifetime,
                    )
                    .unwrap_or(rotation + angle)
                } else {
                    0.0
                };
                let points = std::array::from_fn(|i| {
                    let mut corner = vec2(
                        if i & 1 != 0 { 0.5 } else { -0.5 },
                        if i & 2 != 0 { 0.5 } else { -0.5 },
                    );
                    if render.mode == 1 {
                        // Four-star Hold Release uses a shared radial phase; other sprites
                        // retain their authored world-space rotation in the velocity basis.
                        let rotation = stretched_rotation;
                        corner = vec2(
                            corner.x * rotation.cos() + corner.y * rotation.sin(),
                            -corner.x * rotation.sin() + corner.y * rotation.cos(),
                        );
                    }
                    let Vec2 { x, y } = corner * dimensions;
                    position
                        + vec2(
                            x * angle.cos() - y * angle.sin(),
                            x * angle.sin() + y * angle.cos(),
                        )
                });
                Some(ParticleQuad { points, uv, color })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParticleQuad {
    pub points: [Vec2; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

#[derive(Debug)]
struct Emission {
    time: f32,
    id: u32,
    index: usize,
    count: usize,
}

fn emission_events(system: &SystemDefinition, time: f32, seed: u32, index: u32) -> Vec<Emission> {
    let duration = system.duration.max(1e-6);
    let start = time - maximum(&system.lifetime);
    let first = (start / duration).floor().max(0.0) as u32;
    let last = if system.looping {
        (time.max(0.0) / duration).floor() as u32
    } else {
        0
    };
    let mut events = Vec::new();
    for cycle in first..=last {
        let base = cycle as f32 * duration;
        for (burst_index, burst) in system.bursts.iter().enumerate() {
            for repeat in 0..burst.cycles.max(1) {
                let event_time = base + burst.time + repeat as f32 * burst.interval;
                let event_key = cycle.wrapping_mul(64).wrapping_add(repeat as u32);
                if event_time < start || event_time > time {
                    continue;
                }
                if event_time >= base + duration
                    || (burst.probability < 1.0
                        && random(seed, index, 1000 + burst_index as u32, event_key)
                            >= burst.probability)
                {
                    continue;
                }
                let count = (scalar_value(
                    &burst.count,
                    0.0,
                    random(seed, index, 2000 + burst_index as u32, event_key),
                )
                .round()
                .max(0.0) as usize)
                    .min(system.max_particles);
                for particle in 0..count {
                    let id = ((cycle.wrapping_mul(16).wrapping_add(burst_index as u32))
                        .wrapping_mul(64)
                        .wrapping_add(repeat as u32))
                    .wrapping_mul(1024)
                    .wrapping_add(particle as u32);
                    events.push(Emission {
                        time: event_time,
                        id,
                        index: particle,
                        count,
                    });
                }
            }
        }
        let rate = scalar_value(&system.rate, 0.0, 0.5);
        if rate > 0.0 {
            let first = ((start - base) * rate).ceil().max(0.0) as i64;
            let last = ((time.min(base + duration - 1e-6) - base) * rate).floor() as i64;
            for i in first..=last {
                events.push(Emission {
                    time: base + i as f32 / rate,
                    id: 100_000_000u32
                        .wrapping_add(cycle.wrapping_mul(100_000))
                        .wrapping_add(i as u32),
                    index: 0,
                    count: 1,
                });
            }
        }
    }
    events
}

fn maximum(node: &Value) -> f32 {
    if node["s"].as_i64() == Some(3) {
        scalar_value(node, 0.0, 0.0).max(scalar_value(node, 0.0, 1.0))
    } else {
        node["k"].as_f64().unwrap_or(0.0) as f32
    }
}

fn integrate(age: f32, mut sample: impl FnMut(f32) -> f32) -> f32 {
    let mut result = 0.0;
    let mut time = 0.0;
    while time < age {
        result += sample(time) * STEP.min(age - time);
        time += STEP;
    }
    result
}

fn uv_rect(system: &SystemDefinition, life: f32, random_frame: f32, random_start: f32) -> [f32; 4] {
    let Some(uv) = &system.uv else {
        return [0.0, 0.0, 1.0, 1.0];
    };
    let x = uv.tiles_x.max(1);
    let y = uv.tiles_y.max(1);
    let total = (x * y) as f32;
    let frame = ((scalar_value(&uv.frame, life, random_frame) * total)
        .floor()
        .min(total - 1.0)
        + scalar_value(&uv.start_frame, 0.0, random_start).floor())
    .rem_euclid(total) as u32;
    [
        (frame % x) as f32 / x as f32,
        1.0 - (frame / x + 1) as f32 / y as f32,
        1.0 / x as f32,
        1.0 / y as f32,
    ]
}

fn transform_vector(m: [f32; 6], v: Vec2) -> Vec2 {
    vec2(m[0] * v.x + m[2] * v.y, m[1] * v.x + m[3] * v.y)
}

// Property-keyed hashing matches the source renderer and is independent of frame stepping.
fn random(seed: u32, system: u32, id: u32, property: u32) -> f32 {
    let mut value = (seed ^ 2654435769).wrapping_mul(2246822507);
    value = (value ^ system.wrapping_add(1663821227)).wrapping_mul(3266489909);
    value = (value ^ id.wrapping_add(668265263)).wrapping_mul(374761393);
    value = (value ^ property.wrapping_add(2135587861)).wrapping_mul(2654435761);
    value ^= value >> 15;
    value = value.wrapping_mul(739982445);
    value ^= value >> 12;
    (value as f64 / 4294967296.0) as f32
}
