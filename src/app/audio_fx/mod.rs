//! FX 音频 DSP：把 [`crate::app::fx::ResolvedFx`] 叠加到 BGM 上。
//!
//! 设计目标（见 `docs/FX_NOTE_PLAN.md` §7）：实时切换效果时**保留 BGM 播放位置**。
//! 因此不用 kson 的“嵌套 Source”方案（重建 Source 链会丢状态），而是在一层
//! [`FxSource`] 内做内联 DSP：每约 20ms 轮询一次共享控制，命中则重建内部状态机，
//! `None` 时逐采样干声透传。

mod biquad;

use std::sync::{Arc, Mutex};

use rodio::Source;

use crate::app::fx::ResolvedFx;

pub use self::biquad::{Biquad, BiquadKind};

/// 玩家 → 音频线程的共享效果通道。
#[derive(Debug, Default)]
pub struct FxControl {
    slot: Mutex<Option<Arc<ResolvedFx>>>,
}

impl FxControl {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn set(&self, fx: Option<Arc<ResolvedFx>>) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = fx;
        }
    }

    pub fn clear(&self) {
        self.set(None);
    }

    fn get(&self) -> Option<Arc<ResolvedFx>> {
        self.slot.lock().ok().and_then(|slot| slot.clone())
    }
}

/// 把 BGM Source 包一层 FX。`None` 时逐采样透传，不重建 Sink。
pub struct FxSource<S: Source<Item = f32>> {
    input: S,
    control: Arc<FxControl>,
    current: Option<Arc<ResolvedFx>>,
    active: Option<Dsp>,
    channels: u16,
    sample_rate: u32,
    channel_pos: u16,
    poll_countdown: u32,
    poll_interval: u32,
}

impl<S: Source<Item = f32>> FxSource<S> {
    pub fn new(input: S, control: Arc<FxControl>) -> Self {
        let channels = input.channels().max(1);
        let sample_rate = input.sample_rate().max(1);
        let poll_interval = (sample_rate / 50).max(1) * channels as u32;
        Self {
            input,
            control,
            current: None,
            active: None,
            channels,
            sample_rate,
            channel_pos: 0,
            poll_countdown: 1,
            poll_interval,
        }
    }

    fn poll(&mut self) {
        let new = self.control.get();
        let same = match (&self.current, &new) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.current = new.clone();
        self.active = new.map(|r| Dsp::new(&r, self.sample_rate, self.channels));
    }
}

impl<S: Source<Item = f32>> Iterator for FxSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.poll_countdown = self.poll_countdown.saturating_sub(1);
        if self.poll_countdown == 0 {
            self.poll();
            self.poll_countdown = self.poll_interval;
        }

        let sample = self.input.next()?;
        let ch = self.channel_pos;
        self.channel_pos = (self.channel_pos + 1) % self.channels;

        Some(match &mut self.active {
            Some(dsp) => dsp.process(sample, ch == 0),
            None => sample,
        })
    }
}

impl<S: Source<Item = f32>> Source for FxSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.input.total_duration()
    }
}

// ---------------------------------------------------------------------------
// DSP 状态机
// ---------------------------------------------------------------------------

enum Dsp {
    Gate(GateState),
    BitCrusher(BitCrushState),
    Wobble(WobbleState),
    Sidechain(SidechainState),
    HighPass(HighPassState),
}

impl Dsp {
    fn new(r: &ResolvedFx, sample_rate: u32, channels: u16) -> Self {
        match *r {
            ResolvedFx::Gate { wave_length, rate, mix } => {
                Self::Gate(GateState::new(wave_length, rate, mix, sample_rate))
            }
            ResolvedFx::BitCrusher { reduction, mix } => {
                Self::BitCrusher(BitCrushState::new(reduction, mix, channels))
            }
            ResolvedFx::Wobble { wave_length, lo_freq, hi_freq, q, mix } => {
                Self::Wobble(WobbleState::new(
                    wave_length,
                    lo_freq,
                    hi_freq,
                    q,
                    mix,
                    sample_rate,
                    channels,
                ))
            }
            ResolvedFx::Sidechain { period, hold_time, attack_time, release_time, ratio } => {
                Self::Sidechain(SidechainState::new(
                    period,
                    hold_time,
                    attack_time,
                    release_time,
                    ratio,
                    sample_rate,
                ))
            }
            ResolvedFx::HighPassFilter { freq, q, mix } => {
                Self::HighPass(HighPassState::new(freq, q, mix, sample_rate, channels))
            }
        }
    }

    fn process(&mut self, sample: f32, is_frame_start: bool) -> f32 {
        match self {
            Dsp::Gate(s) => s.process(sample, is_frame_start),
            Dsp::BitCrusher(s) => s.process(sample, is_frame_start),
            Dsp::Wobble(s) => s.process(sample, is_frame_start),
            Dsp::Sidechain(s) => s.process(sample, is_frame_start),
            Dsp::HighPass(s) => s.process(sample),
        }
    }
}

fn lerp(dry: f32, wet: f32, mix: f32) -> f32 {
    dry + (wet - dry) * mix
}

/// 周期性节流：开放 `rate` 比例的时间则原样通过，否则静音。
struct GateState {
    period_frames: u64,
    open_frames: u64,
    cursor: u64,
    mix: f32,
}

impl GateState {
    fn new(wave_length: f32, rate: f32, mix: f32, sample_rate: u32) -> Self {
        let period_frames = (wave_length * sample_rate as f32).round().max(1.0) as u64;
        let open_frames = ((period_frames as f32) * rate.clamp(0.0, 1.0)).round() as u64;
        Self { period_frames, open_frames, cursor: 0, mix }
    }

    fn process(&mut self, sample: f32, is_frame_start: bool) -> f32 {
        if is_frame_start {
            self.cursor = (self.cursor + 1) % self.period_frames;
        }
        let wet = if self.cursor < self.open_frames { sample } else { 0.0 };
        lerp(sample, wet, self.mix)
    }
}

/// 采样保持：每 `reduction` 个采样捕获一次，通道各自保持。
struct BitCrushState {
    reduction: u32,
    counter: u32,
    capturing: bool,
    hold: Vec<f32>,
    current_channel: usize,
    mix: f32,
}

impl BitCrushState {
    fn new(reduction: f32, mix: f32, channels: u16) -> Self {
        Self {
            reduction: reduction.round().max(1.0) as u32,
            counter: 0,
            capturing: true,
            hold: vec![0.0; channels.max(1) as usize],
            current_channel: 0,
            mix,
        }
    }

    fn process(&mut self, sample: f32, is_frame_start: bool) -> f32 {
        let c = self.current_channel;
        if is_frame_start {
            self.capturing = self.counter == 0;
            self.counter = (self.counter + 1) % self.reduction;
        }
        if self.capturing {
            self.hold[c] = sample;
        }
        self.current_channel = (self.current_channel + 1) % self.hold.len();
        lerp(sample, self.hold[c], self.mix)
    }
}

/// 三角波驱动低通中心频率，在 `lo_freq..hi_freq` 间对数摆动。
struct WobbleState {
    phase: f32,
    phase_step: f32,
    lo_freq: f32,
    hi_freq: f32,
    q: f32,
    mix: f32,
    filter: Biquad,
}

impl WobbleState {
    fn new(
        wave_length: f32,
        lo_freq: f32,
        hi_freq: f32,
        q: f32,
        mix: f32,
        sample_rate: u32,
        channels: u16,
    ) -> Self {
        let lo = lo_freq.max(1.0);
        let hi = hi_freq.max(lo);
        let filter = Biquad::new(BiquadKind::LowPass, q, lo, sample_rate, channels);
        Self {
            phase: 0.0,
            phase_step: 1.0 / (wave_length.max(1e-3) * sample_rate as f32),
            lo_freq: lo,
            hi_freq: hi,
            q,
            mix,
            filter,
        }
    }

    fn process(&mut self, sample: f32, is_frame_start: bool) -> f32 {
        if is_frame_start {
            self.phase = (self.phase + self.phase_step) % 1.0;
            let tri = if self.phase < 0.5 {
                4.0 * self.phase - 1.0
            } else {
                3.0 - 4.0 * self.phase
            };
            let u = tri * 0.5 + 0.5;
            let freq = self.lo_freq * (self.hi_freq / self.lo_freq).powf(u);
            self.filter.set(BiquadKind::LowPass, self.q, freq);
        }
        let wet = self.filter.process(sample);
        lerp(sample, wet, self.mix)
    }
}

/// 侧链压缩：`period` 内 attack/hold/release 包络把音量压到 `1/ratio`。
struct SidechainState {
    period_frames: u64,
    attack_frames: u64,
    hold_frames: u64,
    release_frames: u64,
    ratio: f32,
    mix: f32,
    time: u64,
}

impl SidechainState {
    fn new(
        period: f32,
        hold: f32,
        attack: f32,
        release: f32,
        ratio: f32,
        sample_rate: u32,
    ) -> Self {
        let frames = |secs: f32| (secs.max(0.0) * sample_rate as f32).round() as u64;
        Self {
            period_frames: frames(period).max(1),
            attack_frames: frames(attack),
            hold_frames: frames(hold),
            release_frames: frames(release),
            ratio: ratio.max(1.0),
            mix: 1.0,
            time: 0,
        }
    }

    fn process(&mut self, sample: f32, is_frame_start: bool) -> f32 {
        let a = self.attack_frames;
        let h = a + self.hold_frames;
        let r = h + self.release_frames;
        let vol = if self.time < a {
            1.0 - self.time as f32 / a.max(1) as f32
        } else if self.time < h {
            0.0
        } else if self.time < r {
            (self.time - h) as f32 / (r - h).max(1) as f32
        } else {
            1.0
        };
        if is_frame_start {
            self.time = (self.time + 1) % self.period_frames;
        }
        let gain = (1.0 / self.ratio) + (1.0 - 1.0 / self.ratio) * vol;
        lerp(sample, sample * gain, self.mix)
    }
}

struct HighPassState {
    filter: Biquad,
    mix: f32,
}

impl HighPassState {
    fn new(freq: f32, q: f32, mix: f32, sample_rate: u32, channels: u16) -> Self {
        let filter = Biquad::new(BiquadKind::HighPass, q, freq, sample_rate, channels);
        Self { filter, mix }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let wet = self.filter.process(sample);
        lerp(sample, wet, self.mix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fx::{parse_fx_effect, resolve};
    use rodio::buffer::SamplesBuffer;

    fn src(samples: Vec<f32>) -> FxSource<SamplesBuffer<f32>> {
        FxSource::new(SamplesBuffer::new(2, 44_100, samples), FxControl::new())
    }

    #[test]
    fn dry_passthrough_is_exact() {
        let samples: Vec<f32> = (0..64).map(|i| (i as f32) * 0.01 - 0.3).collect();
        let mut s = src(samples.clone());
        for expected in samples {
            assert_eq!(s.next(), Some(expected));
        }
        assert_eq!(s.next(), None);
    }

    #[test]
    fn control_takes_effect_after_poll() {
        let control = FxControl::new();
        let mut s = FxSource::new(SamplesBuffer::new(1, 44_100, vec![1.0; 4096]), control.clone());
        // First sample polls an empty control → dry.
        assert_eq!(s.next(), Some(1.0));
        let fx = Arc::new(resolve(&parse_fx_effect("hpf(mix=100%;freq=20hz)").unwrap(), 120.0));
        control.set(Some(fx));
        // Poll interval is (44100/50)=882 samples; drain past it and expect attenuation.
        let mut any_changed = false;
        for _ in 0..2000 {
            if let Some(v) = s.next() {
                if (v - 1.0).abs() > 1e-4 {
                    any_changed = true;
                    break;
                }
            }
        }
        assert!(any_changed, "hpf should alter a DC-ish signal after activation");
    }

    #[test]
    fn bitcrusher_holds_samples() {
        let r = resolve(&parse_fx_effect("bc(reduction=4samples;mix=100%)").unwrap(), 120.0);
        let mut dsp = Dsp::new(&r, 44_100, 1);
        // Feed a ramp; with 4-sample hold the output should repeat in groups.
        let outs: Vec<f32> = (0..8).map(|i| dsp.process(i as f32, true)).collect();
        assert_eq!(outs[0], outs[1]);
        assert_eq!(outs[1], outs[2]);
        assert_eq!(outs[2], outs[3]);
        assert_ne!(outs[3], outs[4]);
    }

    #[test]
    fn hpf_attenuates_dc() {
        let r = resolve(&parse_fx_effect("hpf(mix=100%;freq=2khz)").unwrap(), 120.0);
        let mut dsp = Dsp::new(&r, 44_100, 1);
        let mut last = 0.0;
        for _ in 0..2000 {
            last = dsp.process(1.0, true);
        }
        assert!(last.abs() < 0.1, "HPF should reject DC, got {last}");
    }
}
