use egui_macroquad::egui;

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

    pub fn draw(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        view_start: f32,
        pixels_per_second: f32,
    ) {
        let Some(level) = self
            .levels
            .iter()
            .rev()
            .find(|l| self.duration / l.min_max.len().max(1) as f32 * pixels_per_second < 2.0)
            .or_else(|| self.levels.first())
        else {
            return;
        };
        let bin_duration = self.duration / level.min_max.len().max(1) as f32;
        let first = ((view_start / bin_duration).floor() as isize - 1).max(0) as usize;
        let last =
            (((view_start + rect.width() / pixels_per_second) / bin_duration).ceil() as usize + 1)
                .min(level.min_max.len());
        for index in first..last {
            let x = rect.left() + (index as f32 * bin_duration - view_start) * pixels_per_second;
            if x < rect.left() - 1.0 || x > rect.right() + 1.0 {
                continue;
            }
            let [lo, hi] = level.min_max[index];
            painter.line_segment(
                [
                    egui::pos2(x, rect.center().y - hi * rect.height() * 0.45),
                    egui::pos2(x, rect.center().y - lo * rect.height() * 0.45),
                ],
                egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgba_premultiplied(90, 141, 186, 130),
                ),
            );
        }
    }
}
