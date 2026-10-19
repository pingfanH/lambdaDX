// Video-derived correction absent from the website's exported movement data.
// This only changes the short four-star group, not the long-lived eight stars.
const CONTRACTION_SECONDS: f32 = 0.06;
const START_OFFSET: f32 = 0.3;
const EXIT_ALIGNED_LIFE: f32 = 2.0 / 3.0;
const FOUR_STAR_GROUP_SCALE: f32 = 0.9;

fn is_four_star_group(effect: &str, system: &str) -> bool {
    effect == "FX_GAM_Notes_Hold_Release_00" && system == "Star_Round"
}

pub(super) fn group_scale(effect: &str, system: &str) -> f32 {
    if is_four_star_group(effect, system) {
        FOUR_STAR_GROUP_SCALE
    } else {
        1.0
    }
}

pub(super) fn radial_offset(effect: &str, system: &str, age: f32) -> f32 {
    if !is_four_star_group(effect, system) {
        return 0.0;
    }
    let remaining = (1.0 - age / CONTRACTION_SECONDS).clamp(0.0, 1.0);
    START_OFFSET * remaining * remaining
}

// Reconstruct the observed group constraint, not four independent random spins.
// This is a video-fitted phase: the exact Unity rotation metadata is unavailable.
pub(super) fn velocity_basis_rotation(
    effect: &str,
    system: &str,
    age: f32,
    lifetime: f32,
    angular_velocity: &serde_json::Value,
) -> Option<f32> {
    if !is_four_star_group(effect, system) {
        return None;
    }
    let start = std::f32::consts::FRAC_PI_2;
    if angular_velocity.is_null() {
        return Some(start);
    }
    let begins = CONTRACTION_SECONDS.min(lifetime * EXIT_ALIGNED_LIFE);
    let ends = lifetime * EXIT_ALIGNED_LIFE;
    let integral = |until: f32| {
        let mut value = 0.0;
        let mut time = begins;
        while time < until {
            let step = (1.0_f32 / 60.0).min(until - time);
            // Use the authored upper curve's easing, shared by the entire burst.
            value += super::data::scalar_value(angular_velocity, time / lifetime, 1.0).abs() * step;
            time += step;
        }
        value
    };
    let total = integral(ends);
    let phase = if total > f32::EPSILON {
        integral(age.clamp(begins, ends)) / total
    } else {
        0.0
    };
    // Settle early enough that the texture's inner notch does not expose a
    // center hole during the connected cross phase.
    let phase = 1.0 - (1.0 - phase).powi(2);
    // Unity UV +Y is the single top tip of FX_hy_18_060; PI turns it inward.
    Some(start + std::f32::consts::FRAC_PI_2 * phase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contraction_only_affects_four_stars_and_ends_before_late_phase() {
        assert_eq!(
            radial_offset("FX_GAM_Notes_Hold_Release_00", "Star_Round", 0.0),
            0.3
        );
        assert_eq!(
            radial_offset("FX_GAM_Notes_Hold_Release_00", "Star_Round", 0.06),
            0.0
        );
        assert_eq!(
            radial_offset("FX_GAM_Notes_Hold_Release_00", "Star_Round_Longlife", 0.0),
            0.0
        );
        assert_eq!(radial_offset("FX_GAM_Notes_Tap_00", "Star_Round", 0.0), 0.0);
    }

    #[test]
    fn shared_phase_holds_during_contraction_and_settles_without_a_snap() {
        let curve = serde_json::json!({"s":0,"k":3.0});
        let sample = |age| {
            velocity_basis_rotation(
                "FX_GAM_Notes_Hold_Release_00",
                "Star_Round",
                age,
                0.35,
                &curve,
            )
            .unwrap()
        };
        assert_eq!(sample(0.0), sample(0.06));
        assert!(sample(0.125) > sample(0.06));
        assert!((sample(0.24) - std::f32::consts::PI).abs() < 1e-6);
        for frame in 1..350 {
            assert!(
                (sample(frame as f32 / 1000.0) - sample((frame - 1) as f32 / 1000.0)).abs() < 0.02
            );
        }
        assert!(
            velocity_basis_rotation(
                "FX_GAM_Notes_Hold_Release_00",
                "Star_Round_Longlife",
                0.2,
                0.5,
                &curve
            )
            .is_none()
        );
        assert!(
            velocity_basis_rotation("FX_GAM_Notes_Tap_00", "Star_Round", 0.2, 0.35, &curve)
                .is_none()
        );
    }
}
