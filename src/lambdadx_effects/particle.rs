use std::collections::HashMap;
use std::path::Path;

use macroquad::prelude::*;

use super::data::EffectDefinition;
use super::shader::EffectShaders;
use super::simulation::EffectInstance;

pub struct EffectLibrary {
    effects: Vec<EffectDefinition>,
    textures: HashMap<String, Texture2D>,
    shaders: EffectShaders,
}

impl EffectLibrary {
    pub async fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        Self::load_with(&root.as_ref().to_string_lossy(), |path| async move {
            std::fs::read(&path).map_err(|error| format!("cannot read {path}: {error}"))
        })
        .await
    }

    #[allow(dead_code)] // Shared PadView entry point; the standalone preview uses filesystem paths.
    pub async fn load_assets(root: &str) -> Result<Self, String> {
        Self::load_with(root, |path| async move {
            macroquad::file::load_file(&path)
                .await
                .map_err(|error| error.to_string())
        })
        .await
    }

    async fn load_with<F, Fut>(root: &str, load: F) -> Result<Self, String>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
    {
        let json = String::from_utf8(load(format!("{root}/effects.json")).await?)
            .map_err(|error| error.to_string())?;
        let effects = super::data::parse_effects(&json)?;
        let mut names = effects
            .iter()
            .flat_map(|effect| &effect.systems)
            .filter_map(|system| system.render.as_ref())
            .flat_map(|render| [render.texture.clone(), render.texture2.clone()])
            .flatten()
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        let mut textures = HashMap::new();
        for name in names {
            let bytes = load(format!("{root}/particle_textures/{name}")).await?;
            let mut image = Image::from_file_with_format(&bytes, None)
                .map_err(|error| format!("cannot decode particle texture {name}: {error}"))?;
            // Match WebGL UNPACK_FLIP_Y_WEBGL before uploading Unity UVs.
            let stride = image.width as usize * 4;
            for y in 0..image.height as usize / 2 {
                let other = image.height as usize - 1 - y;
                for x in 0..stride {
                    image.bytes.swap(y * stride + x, other * stride + x);
                }
            }
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Linear);
            let wrap = if image.width.is_power_of_two() && image.height.is_power_of_two() {
                miniquad::TextureWrap::Repeat
            } else {
                miniquad::TextureWrap::Clamp
            };
            let id = texture.raw_miniquad_id();
            // Only mutate this freshly uploaded texture; no queued draw depends on it.
            unsafe {
                get_internal_gl()
                    .quad_context
                    .texture_set_wrap(id, wrap, wrap);
            }
            textures.insert(name, texture);
        }
        Ok(Self {
            effects,
            textures,
            shaders: EffectShaders::load()?,
        })
    }

    pub fn effect(&self, index: usize) -> Option<&EffectDefinition> {
        self.effects.get(index)
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn new_instance(&self, index: usize) -> Option<EffectInstance> {
        self.effects
            .get(index)
            .map(|effect| EffectInstance::new(effect, preview_seed(index)))
    }

    pub fn draw(&self, instance: &EffectInstance, center: Vec2, zoom: f32) {
        self.draw_oriented(instance, center, zoom, 0.0);
    }

    pub fn draw_oriented(&self, instance: &EffectInstance, center: Vec2, zoom: f32, rotation: f32) {
        self.draw_placed(instance, center, zoom, rotation, 1.0);
    }

    pub fn draw_placed(
        &self,
        instance: &EffectInstance,
        center: Vec2,
        zoom: f32,
        rotation: f32,
        opacity: f32,
    ) {
        // effects.json already contains the source renderer's system order.
        for (index, system) in instance.definition.systems.iter().enumerate() {
            let Some(render) = system.render.as_ref() else {
                continue;
            };
            let particles = instance.sample_system_oriented(index, -rotation);
            if particles.is_empty() {
                continue;
            }
            let material = self
                .shaders
                .configure(render, instance.elapsed, instance.judgment);
            material.set_uniform("uOpacity", render.opacity * opacity.clamp(0.0, 1.0));
            let texture = render
                .texture
                .as_ref()
                .and_then(|name| self.textures.get(name))
                .cloned();
            if render.kind == "add2" {
                let Some(second) = render
                    .texture2
                    .as_ref()
                    .and_then(|name| self.textures.get(name))
                else {
                    continue;
                };
                material.set_texture("Texture2", second.clone());
            }
            material.set_uniform("u_center_zoom", [center.x, center.y, zoom]);
            gl_use_material(material);
            for particle in particles {
                let vertices = (0..4)
                    .map(|i| {
                        let world = particle.points[i];
                        let screen = center + vec2(world.x, -world.y) * zoom;
                        let uv = vec2(
                            particle.uv[0] + if i & 1 != 0 { particle.uv[2] } else { 0.0 },
                            particle.uv[1] + if i & 2 != 0 { particle.uv[3] } else { 0.0 },
                        );
                        // Keep color in float attributes: vertex RGB is also a shader selector.
                        Vertex {
                            position: vec3(screen.x, screen.y, 0.0),
                            uv,
                            color: [255; 4],
                            normal: Vec4::from_array(particle.color),
                        }
                    })
                    .collect();
                let mesh = Mesh {
                    vertices,
                    indices: vec![0, 1, 2, 2, 1, 3],
                    texture: texture.clone(),
                };
                draw_mesh(&mesh);
            }
            gl_use_default_material();
        }
    }
}

pub(super) fn preview_seed(index: usize) -> u64 {
    0xA11CE5EEDu64 + index as u64 * 104729
}
