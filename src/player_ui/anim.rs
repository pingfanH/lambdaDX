//! Easing, staggered reveals and frame-rate-independent smoothing.

/// Clamped progress in `0..=1` for a thing born at `born`, delayed `delay`
/// seconds and taking `dur` seconds.
pub fn progress(now: f64, born: f64, delay: f64, dur: f64) -> f32 {
    if dur <= 0.0 {
        return 1.0;
    }
    (((now - born - delay) / dur) as f32).clamp(0.0, 1.0)
}

/// Ease-out cubic: fast start, gentle settle.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Ease-in-out cubic.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Ease-out back: overshoots slightly then settles. Mirrors the reference
/// site's `cubic-bezier(.2,.9,.3,1.2)` pop.
pub fn ease_out_back(t: f32) -> f32 {
    ease_out_back_s(t, 1.20)
}

pub fn ease_out_back_s(t: f32, s: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c3 = s + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + s * (t - 1.0).powi(2)
}

/// Exponential approach that is stable across frame rates.
pub fn approach(current: &mut f32, target: f32, rate: f32, dt: f32) -> f32 {
    let k = 1.0 - (-rate * dt).exp();
    *current += (target - *current) * k;
    *current
}

/// A single smoothed scalar with a target.
#[derive(Debug, Clone, Copy, Default)]
pub struct Smooth {
    pub value: f32,
}

impl Smooth {
    pub fn new(value: f32) -> Self {
        Self { value }
    }

    pub fn update(&mut self, target: f32, rate: f32, dt: f32) -> f32 {
        approach(&mut self.value, target, rate, dt)
    }
}

/// Linear interpolation.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
