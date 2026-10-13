//! Waveform backdrop rendering (macroquad), one variant per orientation.

use macroquad::prelude as mq;

use crate::audio::WaveformPyramid;

use super::notes::ca;

/// Horizontal timeline: draw the waveform as vertical min/max bars behind the
/// lane rows.
#[allow(clippy::too_many_arguments)]
pub fn draw_horizontal(
    waveform: &WaveformPyramid,
    plot_left: f32,
    plot_right: f32,
    lanes_top: f32,
    lanes_h: f32,
    view: f32,
    pps: f32,
    scale: f32,
    audio_offset: f32,
) {
    let Some(level) = waveform.level_for(pps) else {
        return;
    };
    let bins = level.min_max.len().max(1);
    let bin_duration = waveform.duration / bins as f32;
    if bin_duration <= 0.0 {
        return;
    }
    let center = lanes_top + lanes_h * 0.5;
    let first = (((view + audio_offset) / bin_duration).floor() as isize - 1).max(0) as usize;
    let last =
        (((view + audio_offset + (plot_right - plot_left) / pps) / bin_duration).max(0.0).ceil() as usize + 1).min(bins);
    for index in first..last {
        let x = plot_left + (index as f32 * bin_duration - audio_offset - view) * pps;
        if x < plot_left - 1.0 || x > plot_right + 1.0 {
            continue;
        }
        let [lo, hi] = level.min_max[index];
        mq::draw_line(
            x,
            center - hi * lanes_h * 0.45,
            x,
            center - lo * lanes_h * 0.45,
            1.0 * scale,
            ca(90, 141, 186, 130),
        );
    }
}

/// Vertical timeline: draw the waveform as horizontal min/max bars behind the
/// lane columns. `judge_y` is the "now" position.
#[allow(clippy::too_many_arguments)]
pub fn draw_vertical(
    waveform: &WaveformPyramid,
    plot_left: f32,
    plot_right: f32,
    plot_top: f32,
    plot_bottom: f32,
    judge_y: f32,
    now: f32,
    pps: f32,
    scale: f32,
    audio_offset: f32,
) {
    let Some(level) = waveform.level_for(pps) else {
        return;
    };
    let bins = level.min_max.len().max(1);
    let bin_duration = waveform.duration / bins as f32;
    if bin_duration <= 0.0 {
        return;
    }
    let center = (plot_left + plot_right) * 0.5;
    let half = (plot_right - plot_left) * 0.5 * 0.45;
    let t_top = now + audio_offset + (judge_y - plot_top) / pps;
    let t_bottom = now + audio_offset + (judge_y - plot_bottom) / pps;
    let first = ((t_bottom.min(t_top) / bin_duration).floor() as isize - 1).max(0) as usize;
    let last = ((t_bottom.max(t_top) / bin_duration).max(0.0).ceil() as usize + 1).min(bins);
    for index in first..last {
        let y = judge_y - (index as f32 * bin_duration - audio_offset - now) * pps;
        if y < plot_top - 1.0 || y > plot_bottom + 1.0 {
            continue;
        }
        let [lo, hi] = level.min_max[index];
        mq::draw_line(
            center - hi * half,
            y,
            center - lo * half,
            y,
            1.0 * scale,
            ca(90, 141, 186, 130),
        );
    }
}
