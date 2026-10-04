//! 双二阶（BiQuad）滤波器，供 wobble / high_pass_filter 使用。
//!
//! 移植自参考实现 `kson-rodio-sources/src/biquad.rs`，改用 rodio 0.19 的
//! `channels()->u16` / `sample_rate()->u32`，并去掉 mpsc 控制器（参数在
//! [`super::FxSource`] 里直接更新）。

use std::f32::consts::SQRT_2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiquadKind {
    LowPass,
    HighPass,
}

/// 立体声（任意通道数）双二阶滤波器，Direct Form II Transposed。
#[derive(Debug, Clone)]
pub struct Biquad {
    kind: BiquadKind,
    b0: f32,
    b1: f32,
    b2: f32,
    a0: f32,
    a1: f32,
    a2: f32,
    z1: Vec<f32>,
    z2: Vec<f32>,
    channels: usize,
    current_channel: usize,
    sample_rate: f32,
    q: f32,
    freq: f32,
}

impl Biquad {
    pub fn new(kind: BiquadKind, q: f32, freq: f32, sample_rate: u32, channels: u16) -> Self {
        let channels = channels.max(1) as usize;
        let mut b = Self {
            kind,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a0: 1.0,
            a1: 0.0,
            a2: 0.0,
            z1: vec![0.0; channels],
            z2: vec![0.0; channels],
            channels,
            current_channel: 0,
            sample_rate: sample_rate.max(1) as f32,
            q: q.max(0.01),
            freq: freq.max(1e-3),
        };
        b.update();
        b
    }

    pub fn set(&mut self, kind: BiquadKind, q: f32, freq: f32) {
        self.kind = kind;
        self.q = q.max(0.01);
        // Nyquist 之下，避免系数发散。
        self.freq = freq.clamp(1e-3, self.sample_rate * 0.49);
        self.update();
    }

    pub fn reset(&mut self) {
        self.z1.iter_mut().for_each(|z| *z = 0.0);
        self.z2.iter_mut().for_each(|z| *z = 0.0);
        self.current_channel = 0;
    }

    fn update(&mut self) {
        let w0 = 2.0 * std::f32::consts::PI * self.freq / self.sample_rate;
        let cw0 = w0.cos();
        let alpha = w0.sin() / (2.0 * self.q);
        match self.kind {
            BiquadKind::LowPass => {
                self.b0 = (1.0 - cw0) / 2.0;
                self.b1 = 1.0 - cw0;
                self.b2 = (1.0 - cw0) / 2.0;
            }
            BiquadKind::HighPass => {
                self.b0 = (1.0 + cw0) / 2.0;
                self.b1 = -(1.0 + cw0);
                self.b2 = (1.0 + cw0) / 2.0;
            }
        }
        self.a0 = 1.0 + alpha;
        self.a1 = -2.0 * cw0;
        self.a2 = 1.0 - alpha;
    }

    /// 处理单个采样，内部按通道轮转。
    pub fn process(&mut self, sample: f32) -> f32 {
        let c = self.current_channel;
        self.current_channel = (self.current_channel + 1) % self.channels;

        let (b0, b1, b2, a0, a1, a2) = (self.b0, self.b1, self.b2, self.a0, self.a1, self.a2);
        let out = (b0 / a0) * sample + self.z1[c];
        self.z1[c] = (b1 / a0) * sample - (a1 / a0) * out + self.z2[c];
        self.z2[c] = (b2 / a0) * sample - (a2 / a0) * out;
        out
    }
}

pub fn default_q() -> f32 {
    SQRT_2
}
