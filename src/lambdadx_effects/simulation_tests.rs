use super::super::data::{BurstDefinition, RenderDefinition, UvDefinition};
use super::*;
use serde_json::json;

fn constant(value: f32) -> Value {
    json!({"s":0,"k":value})
}

fn system() -> SystemDefinition {
    SystemDefinition {
        lifetime: constant(1.0),
        size: constant(2.0),
        bursts: vec![BurstDefinition {
            count: constant(1.0),
            probability: 1.0,
            ..Default::default()
        }],
        render: Some(RenderDefinition {
            texture: Some("test.png".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn instance(system: SystemDefinition) -> EffectInstance {
    EffectInstance::new(
        &EffectDefinition {
            systems: vec![system],
            ..Default::default()
        },
        1,
    )
}

fn library() -> Vec<EffectDefinition> {
    super::super::data::parse_effects(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/extracted/lambdadx_replay_1600_master/effects/effects.json"
    )))
    .unwrap()
}

#[test]
fn particle_velocity_uses_system_scale_without_translation() {
    assert_eq!(
        transform_vector([100.0, 0.0, 0.0, 100.0, 300.0, 400.0], vec2(1.0, 2.0)),
        vec2(100.0, 200.0)
    );
}

#[test]
fn delayed_burst_does_not_emit_before_start_delay() {
    let mut system = system();
    system.delay = constant(0.5);
    let mut effect = instance(system);
    effect.update(0.1);
    assert!(effect.sample_system(0).is_empty());
    effect.elapsed = 0.5;
    assert_eq!(effect.sample_system(0).len(), 1);
    effect.elapsed = 1.5;
    assert!(effect.sample_system(0).is_empty());
}

#[test]
fn repeated_bursts_observe_interval_and_rate_starts_at_zero() {
    let mut system = system();
    system.bursts[0].cycles = 3;
    system.bursts[0].interval = 0.2;
    assert_eq!(emission_events(&system, 0.1, 1, 0).len(), 1);
    assert_eq!(emission_events(&system, 0.21, 1, 0).len(), 2);
    assert_eq!(emission_events(&system, 0.41, 1, 0).len(), 3);
    system.bursts.clear();
    system.rate = constant(10.0);
    assert_eq!(emission_events(&system, 0.0, 1, 0).len(), 1);
    assert_eq!(emission_events(&system, 0.21, 1, 0).len(), 3);
    assert!(emission_events(&system, 2.1, 1, 0).is_empty());
    system.looping = true;
    assert!(!emission_events(&system, 2.1, 1, 0).is_empty());
}

#[test]
fn constant_uv_frame_does_not_advance_with_age_or_cycles() {
    let mut system = system();
    system.uv = Some(UvDefinition {
        tiles_x: 4,
        tiles_y: 2,
        frame: constant(0.5),
        start_frame: constant(-1.0),
        cycles: 5.0,
    });
    for life in [0.0, 0.1, 0.5, 0.99] {
        assert_eq!(uv_rect(&system, life, 0.0, 0.0), [0.75, 0.5, 0.25, 0.5]);
    }
}

#[test]
fn material_atlas_aspect_does_not_stretch_particle_geometry() {
    let mut system = system();
    system.render.as_mut().unwrap().st = [1.0, 0.25, 0.0, 0.0];
    let quad = &instance(system).sample_system(0)[0];
    assert_eq!(quad.points[0], vec2(-100.0, -100.0));
    assert_eq!(quad.points[1].distance(quad.points[0]), 200.0);
    assert_eq!(quad.points[2].distance(quad.points[0]), 200.0);
}

#[test]
fn stretched_billboard_length_uses_y_size_along_velocity() {
    let mut system = system();
    system.size_over_lifetime = json!({"x":constant(1.0),"y":constant(0.25)});
    let render = system.render.as_mut().unwrap();
    render.mode = 1;
    render.length_scale = 3.0;
    let quad = &instance(system).sample_system(0)[0];
    assert!((quad.points[1].distance(quad.points[0]) - 200.0).abs() < 0.001);
    assert!((quad.points[2].distance(quad.points[0]) - 150.0).abs() < 0.001);
    assert!(
        (quad.points[2] - quad.points[0])
            .normalize()
            .distance(Vec2::Y)
            < 0.001
    );
    let front = (quad.points[2] + quad.points[3]) * 0.5;
    assert!(
        front.distance(vec2(0.0, 25.0)) < 0.001,
        "stretching must not move the unstretched leading edge: {front:?}"
    );
}

#[test]
fn stretched_billboard_rotates_star_before_stretch_without_rotating_motion_axis() {
    let mut system = system();
    system.rotation = constant(std::f32::consts::FRAC_PI_4);
    let render = system.render.as_mut().unwrap();
    render.mode = 1;
    render.length_scale = 3.0;
    let effect = instance(system);
    let quad = &effect.sample_system(0)[0];
    let horizontal = quad.points[1] - quad.points[0];
    let vertical = quad.points[2] - quad.points[0];
    let diagonal = std::f32::consts::FRAC_1_SQRT_2 * 200.0;
    assert!(
        horizontal.distance(vec2(diagonal, -3.0 * diagonal)) < 0.001,
        "the star's X edge must tilt before vertical stretching: {horizontal:?}"
    );
    assert!(
        vertical.distance(vec2(diagonal, 3.0 * diagonal)) < 0.001,
        "the star's Y edge must tilt before vertical stretching: {vertical:?}"
    );
    let center = (quad.points[0] + quad.points[3]) * 0.5;
    assert!(
        center.distance(vec2(0.0, -200.0)) < 0.001,
        "the trailing offset must remain on the motion axis: {center:?}"
    );
}

#[test]
fn stretched_billboard_keeps_rotation_over_lifetime() {
    let mut system = system();
    system.render.as_mut().unwrap().mode = 1;
    system.render.as_mut().unwrap().length_scale = 3.0;
    system.rotation_over_lifetime = constant(std::f32::consts::PI);
    let mut effect = instance(system);
    let start = effect.sample_system(0);
    effect.elapsed = 0.25;
    let later = effect.sample_system(0);
    let start_edge = start[0].points[1] - start[0].points[0];
    let later_edge = later[0].points[1] - later[0].points[0];
    assert!(
        start_edge.distance(later_edge) > 100.0,
        "stretched billboard discarded its angular velocity"
    );
}

#[test]
fn stretched_billboard_keeps_world_sprite_angle_instead_of_radial_lock() {
    let mut system = system();
    system.shape = Some(super::super::data::ShapeDefinition {
        arc: 360.0,
        arc_mode: 3,
        ..Default::default()
    });
    system.bursts[0].count = constant(4.0);
    system.render.as_mut().unwrap().mode = 1;
    system.render.as_mut().unwrap().length_scale = 1.0;
    let effect = instance(system);
    let quads = effect.sample_system(0);
    assert_eq!(quads.len(), 4);
    for quad in quads {
        assert!(
            (quad.points[1] - quad.points[0]).distance(vec2(200.0, 0.0)) < 0.001,
            "an unrotated star should not rotate just because it travels in a different direction"
        );
    }
}

#[test]
fn hold_release_authored_spin_changes_outline_without_changing_particle_centers() {
    let effects = library();
    let mut spinning = EffectInstance::new(
        effects
            .iter()
            .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
            .unwrap(),
        1,
    );
    spinning.elapsed = 0.125;
    let mut locked = spinning.clone();
    for system in &mut locked.definition.systems {
        system.rotation_over_lifetime = Value::Null;
    }
    for index in [0, 1] {
        let rotated = spinning.sample_system(index);
        let unrotated = locked.sample_system(index);
        let mut changed = 0;
        for (rotated, unrotated) in rotated.iter().zip(&unrotated) {
            let center = |quad: &ParticleQuad| (quad.points[0] + quad.points[3]) * 0.5;
            assert!(center(rotated).distance(center(unrotated)) < 0.001);
            if rotated.points[0].distance(unrotated.points[0]) > 0.01 {
                changed += 1;
            }
            assert_eq!(rotated.uv, unrotated.uv);
            assert_eq!(rotated.color, unrotated.color);
        }
        assert!(
            changed > 0,
            "Hold Release system {index} discarded its authored rotation"
        );
    }
}

#[test]
fn hold_release_eight_stars_recover_shape_while_four_stars_shrink() {
    let effects = library();
    let mut effect = EffectInstance::new(
        effects
            .iter()
            .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
            .unwrap(),
        1,
    );
    effect.elapsed = 0.3;
    let eight = effect.sample_system(0);
    let four = effect.sample_system(1);
    assert_eq!(eight.len(), 8);
    assert_eq!(four.len(), 4);
    for star in eight {
        let width = star.points[1].distance(star.points[0]);
        let length = star.points[2].distance(star.points[0]);
        assert!(
            (0.8..1.3).contains(&(length / width)),
            "eight-star aspect: {}",
            length / width
        );
        assert!(width > 30.0, "eight stars should regain their full width");
    }
    for star in four {
        assert!(star.points[1].distance(star.points[0]) < 10.0);
    }
    effect.elapsed = 0.36;
    assert!(
        effect.sample_system(1).is_empty(),
        "four stars expire before eight stars"
    );
    assert_eq!(effect.sample_system(0).len(), 8);
    effect.elapsed = 0.5;
    assert!(effect.sample_system(0).is_empty());
}

#[test]
fn hold_release_four_stars_share_quarter_turn_symmetry() {
    let effects = library();
    let definition = effects
        .iter()
        .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
        .unwrap();
    for seed in [1, 7, 0x11ce5eed] {
        let mut effect = EffectInstance::new(definition, seed);
        for time in [0.0, 0.03, 0.08, 0.125, 0.2, 0.3, 0.34] {
            effect.elapsed = time;
            let stars = effect.sample_system(1);
            assert_eq!(stars.len(), 4);
            for index in 1..4 {
                for corner in 0..4 {
                    let mut expected = stars[0].points[corner];
                    for _ in 0..index {
                        expected = vec2(-expected.y, expected.x);
                    }
                    assert!(
                        stars[index].points[corner].distance(expected) < 0.002,
                        "four-star symmetry broke at seed {seed}, time {time}, star {index}, corner {corner}"
                    );
                }
                assert_eq!(stars[index].uv, stars[0].uv);
            }
        }
    }
}

#[test]
fn hold_release_four_stars_exit_with_texture_tip_facing_center() {
    // The extracted texture's single upper tip is at Unity UV (0.5, 0.93).
    let image = image::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/extracted/lambdadx_replay_1600_master/effects/particle_textures/FX_hy_18_060.png"
    ))
    .unwrap()
    .to_rgba8();
    assert!(
        image.get_pixel(128, 24)[3] > 100,
        "fixture must have its tip at the top"
    );
    assert!(image.get_pixel(96, 24)[3] < 10);
    assert!(image.get_pixel(160, 24)[3] < 10);
    let effects = library();
    let definition = effects
        .iter()
        .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
        .unwrap();
    for seed in [1, 7, 0x11ce5eed] {
        let mut effect = EffectInstance::new(definition, seed);
        for time in [0.24, 0.28, 0.32, 0.34] {
            effect.elapsed = time;
            for (index, star) in effect.sample_system(1).iter().enumerate() {
                let center = (star.points[0] + star.points[3]) * 0.5;
                let texture_tip = (star.points[2] + star.points[3]) * 0.5 - center;
                let inward = -center.normalize();
                let alignment = texture_tip.normalize().dot(inward);
                assert!(
                    alignment > 0.999,
                    "exit tip misses center: seed {seed}, time {time}, star {index}, dot {alignment}"
                );
            }
        }
    }
}

#[test]
fn hold_release_four_stars_still_cover_center_during_stretch_phase() {
    let effects = library();
    let mut effect = EffectInstance::new(
        effects
            .iter()
            .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
            .unwrap(),
        1,
    );
    effect.elapsed = 0.125;
    let stars = effect.sample_system(1);
    assert_eq!(stars.len(), 4);
    let star = &stars[0];
    let inner = star
        .points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    assert!(
        inner <= 0.0,
        "reference four-star cross is filled at this phase; inner edge is {}",
        inner
    );
    let texture = image::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/extracted/lambdadx_replay_1600_master/effects/particle_textures/FX_hy_18_060.png"
    ))
    .unwrap()
    .to_rgba8();
    for point in [Vec2::ZERO, vec2(0.0, 2.0), vec2(2.0, 0.0)] {
        let mut covered = false;
        for star in &stars {
            let x_axis = star.points[1] - star.points[0];
            let y_axis = star.points[2] - star.points[0];
            let local = point - star.points[0];
            let det = x_axis.perp_dot(y_axis);
            let u = local.perp_dot(y_axis) / det;
            let v = x_axis.perp_dot(local) / det;
            if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                let alpha = texture.get_pixel((u * 255.0) as u32, ((1.0 - v) * 255.0) as u32)[3];
                covered |= alpha > 100;
            }
        }
        assert!(covered, "four-star texture leaves a hole at {point:?}");
    }
}

#[test]
fn hold_release_four_stars_contract_then_expand_without_changing_eight_stars() {
    let effects = library();
    let mut effect = EffectInstance::new(
        effects
            .iter()
            .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
            .unwrap(),
        1,
    );
    let center_radius = |quad: &ParticleQuad| ((quad.points[0] + quad.points[3]) * 0.5).length();
    let start = center_radius(&effect.sample_system(1)[0]);
    effect.elapsed = 0.03;
    let inward = center_radius(&effect.sample_system(1)[0]);
    effect.elapsed = 0.1;
    let outward = center_radius(&effect.sample_system(1)[0]);
    assert!(
        inward + 3.0 < start,
        "four stars should contract first: {start} -> {inward}"
    );
    assert!(
        outward > inward + 5.0,
        "four stars should expand after contraction"
    );
}

#[test]
fn rotation_over_lifetime_is_integrated_as_angular_velocity() {
    let mut system = system();
    system.rotation_over_lifetime = constant(std::f32::consts::PI);
    let mut effect = instance(system);
    effect.elapsed = 0.5;
    let quad = &effect.sample_system(0)[0];
    assert!(quad.points[0].distance(vec2(-100.0, 100.0)) < 0.001);
}

#[test]
fn source_hash_and_hex_vertices_match_javascript_reference() {
    assert!((random(1, 0, 0, 1) - 0.531129746).abs() < 1e-7);
    assert!((random(0x11ce5eed, 5, 1025, 6) - 0.926810058).abs() < 1e-7);
    let effects = library();
    let mut effect = EffectInstance::new(
        effects
            .iter()
            .find(|e| e.name == "FX_GAM_Notes_Tap_01")
            .unwrap(),
        1,
    );
    effect.elapsed = 0.12;
    assert_eq!(effect.particle_count(), 5);
    assert_eq!(effect.sample_system(3)[0].points[0], vec2(-124.0, -125.0));
    assert!(effect.sample_system(4)[0].points[0].distance(vec2(31.732314, -111.441545)) < 0.001);
    effect.judgment = Judgment::Good;
    assert_eq!(effect.particle_count(), 3);
    assert!(effect.sample_system(3).is_empty());
    effect.judgment = Judgment::Great;
    assert_eq!(effect.particle_count(), 2);
}

#[test]
fn all_effects_are_finite_and_independent_of_update_steps() {
    for definition in library() {
        for judgment in Judgment::ALL {
            let mut direct = EffectInstance::new(&definition, 1);
            direct.judgment = judgment;
            let mut stepped = direct.clone();
            direct.update(0.237);
            stepped.update(0.101);
            stepped.update(0.136);
            for index in 0..direct.system_count() {
                let a = direct.sample_system(index);
                let b = stepped.sample_system(index);
                assert_eq!(a.len(), b.len(), "{} / {}", definition.name, index);
                for (a, b) in a.iter().zip(b.iter()) {
                    for i in 0..4 {
                        assert!(a.points[i].is_finite());
                        assert!(a.color[i].is_finite());
                        assert!(
                            a.points[i].distance(b.points[i]) < 0.005,
                            "{} / {}",
                            definition.name,
                            index
                        );
                        assert!((a.uv[i] - b.uv[i]).abs() < 1e-5);
                    }
                }
            }
        }
    }
}

#[test]
fn judgments_have_distinct_colors_and_critical_reuses_perfect_geometry() {
    assert_eq!(Judgment::default(), Judgment::CriticalPerfect);
    assert_eq!(
        Judgment::parse("critical-perfect"),
        Some(Judgment::CriticalPerfect)
    );
    assert_eq!(Judgment::parse("CP"), Some(Judgment::CriticalPerfect));
    assert_eq!(Judgment::parse("perfect"), Some(Judgment::Perfect));
    assert_eq!(Judgment::Raw.color(), None);
    assert_eq!(
        Judgment::CriticalPerfect.color(),
        Some([1.0, 1.0, 0.0, 1.0])
    );
    assert_eq!(
        Judgment::Perfect.palette_shift(),
        Judgment::CriticalPerfect.palette_shift()
    );
    assert!(Judgment::Perfect.palette_green_scale() < 1.0);
    assert_eq!(Judgment::Great.palette_shift(), 0.25);
    assert_eq!(Judgment::Good.palette_shift(), 0.75);
    assert_eq!(Judgment::Raw.palette_shift(), -1.0);
    let definitions = library();
    for name in [
        "FX_GAM_Notes_Tap_00",
        "FX_GAM_Notes_Tap_01",
        "FX_GAM_Notes_Hold_Release_00",
    ] {
        let definition = definitions.iter().find(|e| e.name == name).unwrap();
        let mut effect = EffectInstance::new(definition, 1);
        effect.elapsed = 0.12;
        let critical = (0..effect.system_count())
            .map(|index| effect.sample_system(index))
            .collect::<Vec<_>>();
        effect.judgment = Judgment::Perfect;
        for (index, expected) in critical.iter().enumerate() {
            let perfect = effect.sample_system(index);
            assert_eq!(perfect.len(), expected.len(), "{name} / {index}");
            for (actual, expected) in perfect.iter().zip(expected) {
                assert_eq!(actual.points, expected.points);
                assert_eq!(actual.uv, expected.uv);
                assert_eq!(actual.color[3], expected.color[3]);
                assert!(actual.color[1] < expected.color[1]);
            }
        }
    }
}

#[test]
fn white_hold_release_particles_receive_all_four_judgment_colors() {
    let definitions = library();
    let definition = definitions
        .iter()
        .find(|e| e.name == "FX_GAM_Notes_Hold_Release_00")
        .unwrap();
    let mut effect = EffectInstance::new(definition, 1);
    effect.elapsed = 0.12;
    for grade in Judgment::ALL {
        effect.judgment = grade;
        let stars = effect.sample_system(1);
        assert_eq!(stars.len(), 4);
        let rgb = grade.color().unwrap_or([1.0; 4]);
        assert_eq!(stars[0].color[..3], rgb[..3]);
    }
}

#[test]
#[ignore = "requires Node.js and LAMBDADX_REFERENCE_BUNDLE pointing to the original site bundle"]
fn source_reference_all_effects() {
    let bundle = std::env::var("LAMBDADX_REFERENCE_BUNDLE").expect("set LAMBDADX_REFERENCE_BUNDLE");
    // Evaluate the original site's unmodified CPU particle renderer as an independent oracle.
    let script = r#"
        const fs = require('fs');
        const source = fs.readFileSync(process.argv[1], 'utf8');
        const code = source.slice(source.indexOf('function q('), source.indexOf('var Nt='));
        const {Mt,Ct} = new Function('const Ct=[1,1,0,1];' + code + ';return {Mt,Ct};')();
        // Explicit judgment palette is a caller input; the source replay only used yellow.
        const palette = {
            good:[47/255,202/255,76/255,1], great:[1,114/255,178/255,1],
            perfect:[1,157/255,0,1], 'critical-perfect':[1,1,0,1]
        };
        const effects = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
        const result = [];
        for (const effect of Object.values(effects)) {
            for (const system of effect.systems) {
                system.judge = system.name.match(/(Good|Great|Perfect)/)?.[1].toLowerCase() ?? null;
                system.starBreak = /_Star_Break/.test(system.render.name);
            }
            for (const time of [.013,.12,.237,.43,.91,1.73,3.17]) {
                for (const color of ['good','great','perfect','critical-perfect','none']) {
                    const r = {
                        fx: {data:new Float64Array(1000000),length:0,reserve(){}},
                        fxRuns: [], get fxQuads(){return this.fx.length / 32;}
                    };
                    if (palette[color]) Ct.splice(0,4,...palette[color]);
                    const geometryGrade = color === 'critical-perfect' ? 'perfect' : color;
                    Mt(effect,time,{x:0,y:0,rot:0,color:geometryGrade,seed:1},r);
                    const systems = effect.systems.map(() => []);
                    for (const run of r.fxRuns) {
                        for (let i=0; i<run.count; i++) {
                            systems[effect.systems.indexOf(run.sys)].push(
                                Array.from(r.fx.data.slice((run.first+i)*32,(run.first+i+1)*32)));
                        }
                    }
                    result.push({name:effect.name,time,color,systems});
                }
            }
        }
        process.stdout.write(JSON.stringify(result));
    "#;
    let output = std::process::Command::new("node")
        .args([
            "-e",
            script,
            &bundle,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/extracted/lambdadx_replay_1600_master/effects/effects.json"
            ),
        ])
        .output()
        .expect("run Node.js reference");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let references: Vec<Value> = serde_json::from_slice(&output.stdout).expect("reference JSON");
    let effects = library();
    let mut quads = 0;
    for reference in &references {
        let name = reference["name"].as_str().unwrap();
        let mut effect = EffectInstance::new(effects.iter().find(|e| e.name == name).unwrap(), 1);
        effect.elapsed = reference["time"].as_f64().unwrap() as f32;
        effect.judgment = Judgment::parse(reference["color"].as_str().unwrap()).unwrap();
        for (index, expected) in reference["systems"].as_array().unwrap().iter().enumerate() {
            let actual = effect.sample_system(index);
            let expected = expected.as_array().unwrap();
            assert_eq!(
                actual.len(),
                expected.len(),
                "{name} / {}s / {:?} / system {index}",
                effect.elapsed,
                effect.judgment
            );
            for (particle, expected) in actual.iter().zip(expected) {
                quads += 1;
                let stretched = effect.definition.systems[index]
                    .render
                    .as_ref()
                    .unwrap()
                    .mode
                    == 1;
                if stretched {
                    // Undo the corrected trailing stretch before checking source particle motion.
                    let group_scale = super::super::reference_motion::group_scale(
                        name,
                        &effect.definition.systems[index].name,
                    );
                    let center = (particle.points[0] + particle.points[3]) * 0.5 / group_scale;
                    let direction = vec2(
                        (expected[8].as_f64().unwrap() - expected[0].as_f64().unwrap()) as f32,
                        (expected[9].as_f64().unwrap() - expected[1].as_f64().unwrap()) as f32,
                    )
                    .normalize_or_zero();
                    let x_edge = (particle.points[1] - particle.points[0]) / group_scale;
                    let y_edge = (particle.points[2] - particle.points[0]) / group_scale;
                    // Sprite rotation precedes stretch, so neither edge alone is the length axis.
                    let length = vec2(x_edge.dot(direction), y_edge.dot(direction)).length();
                    let length_scale = effect.definition.systems[index]
                        .render
                        .as_ref()
                        .unwrap()
                        .length_scale;
                    let original_length = length / length_scale;
                    let mut unshifted_center =
                        center + direction * (length - original_length) * 0.5;
                    let system = &effect.definition.systems[index];
                    let age = effect.elapsed * system.sim_speed;
                    let offset =
                        super::super::reference_motion::radial_offset(name, &system.name, age);
                    let m = system.matrix;
                    let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                    unshifted_center -= direction * offset * scale;
                    let source_center = vec2(
                        (expected[0].as_f64().unwrap() + expected[24].as_f64().unwrap()) as f32
                            * 0.5,
                        (expected[1].as_f64().unwrap() + expected[25].as_f64().unwrap()) as f32
                            * 0.5,
                    );
                    assert!(
                        unshifted_center.distance(source_center) < 0.03,
                        "{name} / system {index} center"
                    );
                }
                for corner in 0..4 {
                    let vertex = [
                        particle.points[corner].x,
                        particle.points[corner].y,
                        particle.uv[0] + if corner & 1 != 0 { particle.uv[2] } else { 0.0 },
                        particle.uv[1] + if corner & 2 != 0 { particle.uv[3] } else { 0.0 },
                        particle.color[0],
                        particle.color[1],
                        particle.color[2],
                        particle.color[3],
                    ];
                    for (channel, actual) in vertex.into_iter().enumerate() {
                        if stretched && channel < 2 {
                            continue;
                        }
                        let expected = expected[corner * 8 + channel].as_f64().unwrap() as f32;
                        let tolerance = if channel < 2 { 0.03 } else { 0.0001 };
                        assert!(
                            (actual - expected).abs() < tolerance,
                            "{name} / {}s / {:?} / system {index} / corner {corner} / channel {channel}: {actual} != {expected}",
                            effect.elapsed,
                            effect.judgment
                        );
                    }
                }
            }
        }
    }
    println!(
        "Compared {} frames / {quads} quads with the original renderer (corrected stretch axes, trailing offset, and pre-stretch rotation covered by regression tests)",
        references.len()
    );
}
