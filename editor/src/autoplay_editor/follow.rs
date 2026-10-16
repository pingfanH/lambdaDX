const TRANSITION_SECONDS: f32 = 0.2;

#[derive(Debug, Clone, Default)]
pub(super) struct PlayheadFollow {
    transition: Option<Transition>,
}

#[derive(Debug, Clone)]
struct Transition {
    origin: f32,
    elapsed: f32,
    last_time: f32,
}

impl PlayheadFollow {
    pub fn reset(&mut self) {
        self.transition = None;
    }

    pub fn advance(&mut self, view: f32, now: f32, span: f32, dt: f32, speed: f32) -> f32 {
        if span <= 0.0 {
            self.reset();
            return view;
        }
        // A seek or loop wrap starts a new transition, never an old camera jump.
        if self.transition.as_ref().is_some_and(|motion| {
            let delta = now - motion.last_time;
            delta < -0.001 || delta > dt.max(0.0) * speed.abs() + 0.1
        }) {
            self.reset();
        }
        let target = now - span * 0.5;
        let transitioning = self.transition.as_ref().is_some_and(|motion| motion.elapsed < TRANSITION_SECONDS);
        if !transitioning && now >= view && target <= view {
            self.reset();
            return view;
        }
        let motion = self.transition.get_or_insert(Transition {
            origin: view,
            elapsed: 0.0,
            last_time: now,
        });
        motion.last_time = now;
        motion.elapsed = (motion.elapsed + dt.clamp(0.0, 0.1)).min(TRANSITION_SECONDS);
        let progress = motion.elapsed / TRANSITION_SECONDS;
        let ease = progress * progress * (3.0 - 2.0 * progress);
        // Ease into the moving target, then follow exactly without zoom-dependent lag.
        if progress >= 1.0 { target } else { motion.origin + (target - motion.origin) * ease }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_half_stays_fixed_then_eases_into_exact_follow_at_any_scale() {
        for fps in [30.0, 60.0, 144.0] {
            for span in [0.25, 4.0, 100.0] {
                let mut follow = PlayheadFollow::default();
                let dt = 1.0 / fps;
                let mut view = 10.0;
                for fraction in [0.0, 0.25, 0.5] {
                    assert_eq!(follow.advance(view, 10.0 + span * fraction, span, dt, 1.0), view);
                }
                let crossing = 10.0 + span * 0.5;
                view = follow.advance(view, crossing + dt, span, dt, 1.0);
                assert!(view > 10.0 && view < 10.0 + dt);
                for frame in 2..=(fps as usize) {
                    let now = crossing + frame as f32 * dt;
                    let next = follow.advance(view, now, span, dt, 1.0);
                    assert!(next >= view);
                    view = next;
                    if frame as f32 * dt > TRANSITION_SECONDS + dt {
                        assert!((view - (now - span * 0.5)).abs() < 0.0001);
                    }
                }
            }
        }
    }

    #[test]
    fn interruption_and_clock_jumps_restart_from_current_view() {
        let mut follow = PlayheadFollow::default();
        let mut view = 0.0;
        for frame in 0..30 {
            view = follow.advance(view, 3.0 + frame as f32 / 60.0, 4.0, 1.0 / 60.0, 1.0);
        }
        let before = view;
        view = follow.advance(view, 30.0, 4.0, 1.0 / 60.0, 1.0);
        assert!(view > before && view < 28.0);
        follow.reset();
        assert_eq!(follow.advance(28.5, 30.0, 4.0, 1.0 / 60.0, 1.0), 28.5);
        view = 28.5;
        for frame in 0..20 {
            let now = 1.0 + frame as f32 / 60.0;
            let next = follow.advance(view, now, 4.0, 1.0 / 60.0, 1.0);
            if frame == 0 { assert!(next < view && next > now - 2.0); }
            view = next;
        }
        assert!((view - (1.0 + 19.0 / 60.0 - 2.0)).abs() < 0.0001);
    }
}
