//! Audio waveform pyramid used by the timeline background.
//!
//! Kept independent of rendering: it builds a multi-resolution min/max pyramid
//! once per track and exposes it to whichever timeline renderer needs it.

use crate::player::state::PadPreviewState;

#[derive(Debug, Clone)]
pub struct WaveformLevel {
    pub samples_per_bin: usize,
    pub min_max: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Default)]
pub struct WaveformPyramid {
    pub levels: Vec<WaveformLevel>,
    pub duration: f32,
}

impl WaveformPyramid {
    /// Build the pyramid from the currently loaded track.
    pub fn build(app: &PadPreviewState) -> Self {
        let Some(pcm) = &app.audio_wav_pcm else {
            return Self::default();
        };
        let channels = pcm.channels.max(1) as usize;
        let frames = pcm.samples.len() / channels;
        let duration = frames as f32 / pcm.sample_rate.max(1) as f32;
        let mut levels = Vec::new();
        for samples_per_bin in [64, 256, 1024, 4096] {
            let mut min_max = Vec::with_capacity(frames.div_ceil(samples_per_bin));
            for chunk in pcm.samples.chunks(channels * samples_per_bin) {
                let mut lo = 1.0_f32;
                let mut hi = -1.0_f32;
                for frame in chunk.chunks(channels) {
                    let value =
                        frame.iter().map(|v| *v as f32 / 32768.0).sum::<f32>() / frame.len() as f32;
                    lo = lo.min(value);
                    hi = hi.max(value);
                }
                min_max.push([lo, hi]);
            }
            levels.push(WaveformLevel {
                samples_per_bin,
                min_max,
            });
        }
        Self { levels, duration }
    }

    /// Pick the coarsest level whose bins are still finer than ~2px at the given
    /// zoom, falling back to the finest level.
    pub fn level_for(&self, pixels_per_second: f32) -> Option<&WaveformLevel> {
        self.levels
            .iter()
            .rev()
            .find(|l| self.duration / l.min_max.len().max(1) as f32 * pixels_per_second < 2.0)
            .or_else(|| self.levels.first())
    }
}
