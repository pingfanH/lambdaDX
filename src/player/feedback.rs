//! Shared judgment assets and transient feedback data for every PadView host.

use std::collections::HashMap;

use macroquad::prelude::*;
use serde::Deserialize;

use super::effects::{EffectInstance, EffectLibrary, Judgment};
use super::state::PadPreviewState;
use crate::app::types::zone::PadZone;
use crate::core::types::JudgeGrade;

pub const RESOURCE_ROOT: &str = "extracted/lambdadx_replay_1600_master";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    CriticalPerfect,
    Perfect,
    Great,
    Good,
    Miss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    Fast,
    Late,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackGrade {
    pub grade: Grade,
    pub timing: Option<Timing>,
}

impl From<JudgeGrade> for FeedbackGrade {
    fn from(grade: JudgeGrade) -> Self {
        use JudgeGrade::*;
        let timing = match grade {
            FastPerfect2nd | FastPerfect3rd | FastGreat | FastGreat2nd | FastGreat3rd
            | FastGood | TooFast => Some(Timing::Fast),
            LatePerfect2nd | LatePerfect3rd | LateGreat | LateGreat2nd | LateGreat3rd
            | LateGood => Some(Timing::Late),
            _ => None,
        };
        let grade = match grade {
            Perfect => Grade::CriticalPerfect,
            Miss | TooFast => Grade::Miss,
            FastGood | LateGood => Grade::Good,
            FastGreat | FastGreat2nd | FastGreat3rd | LateGreat | LateGreat2nd | LateGreat3rd => {
                Grade::Great
            }
            _ => Grade::Perfect,
        };
        Self { grade, timing }
    }
}

impl FeedbackGrade {
    pub fn from_label(label: &str) -> Self {
        let label = label.to_ascii_lowercase();
        let grade = if label.contains("critical") {
            Grade::CriticalPerfect
        } else if label.contains("perfect") {
            Grade::Perfect
        } else if label.contains("great") {
            Grade::Great
        } else if label.contains("good") {
            Grade::Good
        } else {
            Grade::Miss
        };
        Self {
            grade,
            timing: None,
        }
    }

    pub fn label(self) -> &'static str {
        match self.grade {
            Grade::CriticalPerfect => "Critical Perfect",
            Grade::Perfect => "Perfect",
            Grade::Great => "Great",
            Grade::Good => "Good",
            Grade::Miss => "Miss",
        }
    }

    pub fn judgment(self) -> Option<Judgment> {
        Some(match self.grade {
            Grade::CriticalPerfect => Judgment::CriticalPerfect,
            Grade::Perfect => Judgment::Perfect,
            Grade::Great => Judgment::Great,
            Grade::Good => Judgment::Good,
            Grade::Miss => return None,
        })
    }

    pub fn color(self) -> Color {
        self.judgment()
            .and_then(|grade| grade.color())
            .map(|c| Color::new(c[0], c[1], c[2], c[3]))
            .unwrap_or(Color::from_rgba(200, 90, 90, 255))
    }

    pub fn sprite_suffix(self, slide: bool) -> String {
        let grade = match self.grade {
            Grade::CriticalPerfect => "Critical",
            Grade::Perfect => "Perfect",
            Grade::Great => "Great",
            Grade::Good => "Good",
            Grade::Miss => return "Miss".into(),
        };
        if let Some(timing) = self.timing {
            let timing = match timing {
                Timing::Fast => "Fast",
                Timing::Late => "Late",
            };
            format!("{timing}{grade}{}", if slide { "_01" } else { "" })
        } else {
            grade.into()
        }
    }
}

#[derive(Debug, Clone)]
pub struct JudgeFeedback {
    pub zone: PadZone,
    pub grade: FeedbackGrade,
    pub is_touch: bool,
    pub is_break: bool,
    pub started: f64,
    pub until: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitEffect {
    Tap,
    ExTap,
    Break,
    Touch,
    Hold,
    HoldRelease,
    Center,
}

impl HitEffect {
    pub fn size_scale(self) -> f32 {
        if self == Self::Center {
            crate::app::params::hanabi_fx_scale().max(0.05)
        } else {
            (crate::app::params::hit_fx_size() / 46.0).max(0.05)
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Tap => "FX_GAM_Notes_Tap_00",
            Self::ExTap => "FX_GAM_Notes_EXTap_00",
            Self::Break => "FX_GAM_Notes_Break_00",
            Self::Touch => "FX_GAM_Notes_Touch_00",
            Self::Hold => "FX_GAM_Notes_Hold_00",
            Self::HoldRelease => "FX_GAM_Notes_Hold_Release_00",
            Self::Center => "FX_GAM_Notes_Center_Back_00",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HitFx {
    pub zone: PadZone,
    pub is_touch: bool,
    pub note_id: Option<u64>,
    pub kind: HitEffect,
    pub started: f64,
    pub duration: f32,
    pub color: Color,
    pub seed: f32,
    pub instance: Option<EffectInstance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpriteMeta {
    pub tex: [f32; 2],
    pub rect: [f32; 4],
    pub pivot: [f32; 2],
}

impl SpriteMeta {
    pub fn origin(&self) -> Vec2 {
        vec2(
            self.rect[0] + self.pivot[0] * self.rect[2],
            self.tex[1] - self.rect[1] - self.pivot[1] * self.rect[3],
        )
    }
}

pub struct FeedbackResources {
    pub effects: Option<EffectLibrary>,
    judgments: macroanimate::XflAsset,
    sprites: HashMap<String, SpriteMeta>,
    additive: Material,
}

impl FeedbackResources {
    pub async fn load() -> Result<Self, String> {
        let root = format!("{RESOURCE_ROOT}/judgement");
        let bytes =
            crate::app::platform::load_asset_bytes(&format!("{root}/pad_sprite_manifest.json"))
                .await?;
        let sprites: HashMap<String, SpriteMeta> =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let base = crate::app::platform::asset_dir().join("effects/lambdadx_replay_1600_master");
        let fla = base.with_extension("fla");
        let path = if fla.is_file() { fla } else { base };
        let judgments = macroanimate::XflAsset::load(path).map_err(|error| error.to_string())?;
        let effects = match EffectLibrary::load_assets(&format!("{RESOURCE_ROOT}/effects")).await {
            Ok(effects) => Some(effects),
            Err(error) => {
                eprintln!("LambdaDX particle assets: {error}");
                None
            }
        };
        Ok(Self {
            effects,
            judgments,
            sprites,
            additive: additive_material()?,
        })
    }

    pub fn instance(&self, kind: HitEffect) -> Option<EffectInstance> {
        let effects = self.effects.as_ref()?;
        (0..effects.len())
            .find(|&i| effects.effect(i).is_some_and(|e| e.name == kind.name()))
            .and_then(|index| effects.new_instance(index))
    }

    pub fn draw_sprite(
        &self,
        key: &str,
        anchor: Vec2,
        size: Vec2,
        rotation: f32,
        alpha: f32,
        elapsed: f32,
    ) -> bool {
        let Some(meta) = self.sprites.get(key) else {
            return false;
        };
        let name = format!("Judgement/{key}");
        let Some(clip) = self.judgments.get(&name) else {
            return false;
        };
        let frame = ((elapsed.max(0.0) * clip.fps()) as usize).min(clip.frames() - 1);
        let matrix = judgment_matrix(meta, anchor, size, rotation);
        self.judgments.draw_matrix(&name, frame, &matrix, alpha);
        true
    }

    pub fn draw_sprite_additive(
        &self,
        key: &str,
        anchor: Vec2,
        size: Vec2,
        rotation: f32,
        alpha: f32,
        elapsed: f32,
    ) -> bool {
        gl_use_material(&self.additive);
        let drawn = self.draw_sprite(key, anchor, size, rotation, alpha, elapsed);
        gl_use_default_material();
        drawn
    }
}

fn additive_material() -> Result<Material, String> {
    use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
    load_material(
        ShaderSource::Glsl {
            vertex: r#"#version 100
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
    color = color0 / 255.0;
}"#,
            fragment: r#"#version 100
precision mediump float;
varying vec2 uv;
varying vec4 color;
uniform sampler2D Texture;
void main() {
    gl_FragColor = texture2D(Texture, uv) * color;
}"#,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::One,
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Zero,
                    BlendFactor::One,
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .map_err(|error| format!("judgment additive shader: {error}"))
}

fn judgment_matrix(meta: &SpriteMeta, anchor: Vec2, size: Vec2, rotation: f32) -> [f32; 6] {
    // FLA symbols are centered; the source anchor is registered relative to its trimmed rect.
    let offset = (Vec2::from_array(meta.tex) * 0.5 - meta.origin()) * size;
    let axis = Vec2::from_angle(rotation);
    let origin = anchor + axis.rotate(offset);
    [
        axis.x * size.x,
        axis.y * size.x,
        -axis.y * size.y,
        axis.x * size.y,
        origin.x,
        origin.y,
    ]
}

impl PadPreviewState {
    pub fn push_hanabi_feedback(
        &mut self, zone: PadZone, grade: FeedbackGrade, is_touch: bool, note_id: Option<u64>,
    ) {
        // Audio remains enabled when visual hit effects are hidden.
        self.play_sfx(super::sfx::hanabi_for_hit(self, grade));
        self.push_note_hit_fx(zone, grade, HitEffect::Center, is_touch, None, note_id);
    }

    pub fn push_note_judgement(
        &mut self,
        zone: PadZone,
        grade: FeedbackGrade,
        is_touch: bool,
        is_break: bool,
        duration: f64,
    ) {
        let now = self.fx_clock();
        self.judge_feedback
            .retain(|feedback| feedback.zone != zone || feedback.is_touch != is_touch);
        self.judge_feedback.push(JudgeFeedback {
            zone,
            grade,
            is_touch,
            is_break,
            started: now,
            until: now + duration,
        });
    }

    pub fn push_note_hit_fx(
        &mut self,
        zone: PadZone,
        grade: FeedbackGrade,
        kind: HitEffect,
        is_touch: bool,
        duration: Option<f32>,
        note_id: Option<u64>,
    ) {
        let Some(judgment) = grade.judgment() else {
            return;
        };
        if !crate::app::params::hit_fx() {
            return;
        }
        let now = self.fx_clock();
        let mut instance = self
            .feedback_resources
            .as_ref()
            .and_then(|r| r.instance(kind));
        if let Some(instance) = &mut instance {
            instance.judgment = if kind == HitEffect::Center {
                Judgment::Raw
            } else {
                judgment
            };
            instance.reset(note_id.unwrap_or((now * 1e6) as u64));
        }
        let duration = duration.unwrap_or_else(|| {
            instance
                .as_ref()
                .map(|i| i.duration())
                .unwrap_or_else(|| crate::app::params::hit_fx_duration().max(0.05))
        });
        self.hit_fx.push(HitFx {
            zone,
            is_touch,
            note_id,
            kind,
            started: now,
            duration,
            color: grade.color(),
            seed: now.fract() as f32 * std::f32::consts::TAU,
            instance,
        });
    }

    pub fn stop_hold_fx(&mut self, note_id: u64) {
        self.hit_fx
            .retain(|fx| fx.kind != HitEffect::Hold || fx.note_id != Some(note_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hanabi_size_is_independent_of_other_hit_effects() {
        let previous = crate::app::params::get();
        let mut params = previous.clone();
        params.hit_fx_size = 92.0;
        params.hanabi_fx_scale = 0.65;
        crate::app::params::set(params.clone());
        assert_eq!(HitEffect::Center.size_scale(), 0.65);
        for kind in [HitEffect::Tap, HitEffect::Touch, HitEffect::Hold, HitEffect::HoldRelease, HitEffect::Break, HitEffect::ExTap] {
            assert_eq!(kind.size_scale(), 2.0);
        }
        params.hit_fx_size = 23.0;
        crate::app::params::set(params);
        assert_eq!(HitEffect::Center.size_scale(), 0.65);
        assert_eq!(HitEffect::Touch.size_scale(), 0.5);
        crate::app::params::set(previous);
    }

    #[test]
    fn critical_and_ordinary_perfect_are_distinct() {
        let critical = FeedbackGrade::from(JudgeGrade::Perfect);
        let perfect = FeedbackGrade::from(JudgeGrade::FastPerfect2nd);
        assert_eq!(critical.grade, Grade::CriticalPerfect);
        assert_eq!(critical.sprite_suffix(false), "Critical");
        assert_eq!(perfect.grade, Grade::Perfect);
        assert_eq!(perfect.sprite_suffix(true), "FastPerfect_01");
        assert_ne!(critical.color(), perfect.color());
    }

    #[test]
    fn misses_do_not_spawn_particles_and_hold_tail_stops_only_its_loop() {
        let mut app = super::super::render::scale::tests::app();
        app.forced_time = Some(1.0);
        app.push_note_hit_fx(
            PadZone::A1,
            JudgeGrade::Miss.into(),
            HitEffect::Tap,
            false,
            None,
            Some(1),
        );
        assert!(app.hit_fx.is_empty());
        for (id, kind) in [
            (1, HitEffect::Hold),
            (2, HitEffect::Hold),
            (1, HitEffect::Tap),
        ] {
            app.push_note_hit_fx(
                PadZone::A1,
                JudgeGrade::Perfect.into(),
                kind,
                false,
                Some(2.0),
                Some(id),
            );
        }
        app.stop_hold_fx(1);
        assert_eq!(app.hit_fx.len(), 2);
        assert!(
            app.hit_fx
                .iter()
                .any(|fx| fx.note_id == Some(2) && fx.kind == HitEffect::Hold)
        );
        app.forced_time = Some(4.0);
        app.mode = crate::app::types::Mode::Playing;
        app.tick_feedback();
        assert!(app.hit_fx.is_empty());
    }

    #[test]
    fn same_sensor_note_and_touch_judgments_have_independent_slots() {
        let mut app = super::super::render::scale::tests::app();
        app.forced_time = Some(1.0);
        for is_touch in [false, true, false] {
            app.push_note_judgement(
                PadZone::A1,
                JudgeGrade::Perfect.into(),
                is_touch,
                false,
                0.3,
            );
        }
        assert_eq!(app.judge_feedback.len(), 2);
        assert_ne!(
            app.judge_feedback[0].is_touch,
            app.judge_feedback[1].is_touch
        );
        app.try_seek_to(0.0).unwrap();
        assert!(app.judge_feedback.is_empty());
        assert!(app.hit_fx.is_empty());
    }

    #[test]
    fn every_grade_and_slide_family_has_an_asset() {
        let atlas = macroanimate::XflAtlas::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/effects/lambdadx_replay_1600_master.fla"
        ))
        .unwrap();
        let manifest: HashMap<String, SpriteMeta> = serde_json::from_str(include_str!(
            "../../assets/extracted/lambdadx_replay_1600_master/judgement/pad_sprite_manifest.json"
        ))
        .unwrap();
        for core_grade in [
            JudgeGrade::Perfect,
            JudgeGrade::FastPerfect2nd,
            JudgeGrade::LatePerfect3rd,
            JudgeGrade::FastGreat,
            JudgeGrade::LateGreat2nd,
            JudgeGrade::FastGood,
            JudgeGrade::LateGood,
            JudgeGrade::Miss,
            JudgeGrade::TooFast,
        ] {
            let grade = FeedbackGrade::from(core_grade);
            assert!(manifest.contains_key(&format!("judge/UI_GAM_{}", grade.sprite_suffix(false))));
            if grade.grade == Grade::Miss {
                continue;
            }
            for prefix in [
                "slidejudge/UI_GAM_Slide_L",
                "slidejudge/UI_GAM_Slide_R",
                "slidecirclejudge/UI_GAM_SlideCircle_L",
                "slidecirclejudge/UI_GAM_SlideCircle_R",
                "slidefanjudge/UI_GAM_SlideFan_U",
                "slidefanjudge/UI_GAM_SlideFan_D",
            ] {
                assert!(manifest.contains_key(&format!("{prefix}_{}", grade.sprite_suffix(true))));
            }
        }
        for (key, meta) in manifest {
            assert!(
                atlas.animations.contains_key(&format!("Judgement/{key}")),
                "{key}"
            );
            assert!(
                !atlas.parts(&format!("Judgement/{key}"), 0).is_empty(),
                "{key}"
            );
            assert!(meta.origin().is_finite(), "{key}");
            let path = format!(
                "{}/assets/{RESOURCE_ROOT}/judgement/{key}.png",
                env!("CARGO_MANIFEST_DIR")
            );
            let image = image::open(path).unwrap();
            assert_eq!(
                [image.width() as f32, image.height() as f32],
                meta.tex,
                "{key}"
            );
            let name = format!("Judgement/{key}");
            let parts = atlas.parts(&name, 0);
            assert_eq!(parts.len(), 1, "{key}");
            let part = &parts[0];
            let macroanimate::xfl::PartContent::Bitmap(bitmap) = &part.content else {
                panic!("{key} must preserve the source bitmap");
            };
            let embedded = image::load_from_memory(atlas.bitmaps[bitmap].bytes.as_ref().unwrap()).unwrap();
            assert_eq!(embedded.to_rgba8(), image.to_rgba8(), "{key} bitmap differs");
            assert_eq!(part.matrix, [1.0, 0.0, 0.0, 1.0, -meta.tex[0] * 0.5, -meta.tex[1] * 0.5], "{key} must not stretch the sprite");
        }
    }

    #[test]
    fn flash_matrix_keeps_manifest_anchor_under_rotation_and_mirroring() {
        let meta = SpriteMeta {
            tex: [428.0, 140.0],
            rect: [22.0, 1.0, 379.0, 108.0],
            pivot: [-0.058, 0.639],
        };
        let registered = meta.origin() - Vec2::from_array(meta.tex) * 0.5;
        let anchor = vec2(120.0, 350.0);
        for scale in [vec2(0.5, 0.5), vec2(-0.5, 0.5), vec2(0.5, -0.5)] {
            for rotation in [0.0, 0.3, 1.4, 3.8] {
                let m = judgment_matrix(&meta, anchor, scale, rotation);
                let point = vec2(
                    m[0] * registered.x + m[2] * registered.y + m[4],
                    m[1] * registered.x + m[3] * registered.y + m[5],
                );
                assert!(point.distance(anchor) < 0.0001);
            }
        }
    }

    #[test]
    fn fla_sprite_corners_match_all_source_rendered_slide_quads() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../assets/extracted/lambdadx_replay_1600_master/slide_judgment_reference.json"
        )).unwrap();
        let manifest: HashMap<String, SpriteMeta> = serde_json::from_str(include_str!(
            "../../assets/extracted/lambdadx_replay_1600_master/judgement/pad_sprite_manifest.json"
        )).unwrap();
        for value in reference["poses"].as_object().unwrap().values() {
            let pose: super::super::render::slide_judgment::Pose = serde_json::from_value(value.clone()).unwrap();
            let meta = &manifest[&format!("{}_Critical", pose.key)];
            let (anchor, size, rotation) = pose.screen(Vec2::ZERO, 480.0);
            let m = judgment_matrix(meta, anchor, size, rotation);
            for i in 0..4 {
                let corner = vec2(if i % 2 == 0 { 0.0 } else { meta.tex[0] }, if i < 2 { 0.0 } else { meta.tex[1] }) - Vec2::from_array(meta.tex) * 0.5;
                let actual = vec2(m[0] * corner.x + m[2] * corner.y + m[4], m[1] * corner.x + m[3] * corner.y + m[5]);
                let expected = vec2(pose.corners[i * 2], pose.corners[i * 2 + 1]);
                assert!(actual.distance(expected) < 0.001, "{}: {actual:?} != {expected:?}", pose.key);
            }
        }
    }
}
