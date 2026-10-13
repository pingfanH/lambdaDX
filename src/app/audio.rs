use minimp3::{Decoder as Mp3Decoder, Frame as Mp3Frame};
use rodio::buffer::SamplesBuffer;
use rodio::dynamic_mixer::{self, DynamicMixer, DynamicMixerController};
use rodio::{OutputStream, Sink, Source};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Arc;

use super::platform;
use super::types::WavPcm;
use crate::player::state::PadPreviewState;

// ---------------------------------------------------------------------------
// PCM decoding
// ---------------------------------------------------------------------------

fn load_wav_pcm_from_bytes(bytes: &[u8]) -> Result<WavPcm, String> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("open wav bytes: {e}"))?;
    let spec = reader.spec();
    if spec.bits_per_sample != 16 {
        return Err("only 16-bit wav is supported for speed shift".to_string());
    }
    let src_ch = spec.channels.max(1) as usize;
    let mut raw = Vec::new();
    for s in reader.samples::<i16>() {
        raw.push(s.map_err(|e| format!("read wav sample: {e}"))?);
    }
    let mut samples = Vec::with_capacity((raw.len() / src_ch) * 2);
    match src_ch {
        1 => {
            for &s in &raw {
                samples.push(s);
                samples.push(s);
            }
        }
        2 => samples.extend_from_slice(&raw),
        _ => {
            for frame in raw.chunks(src_ch) {
                let l = *frame.first().unwrap_or(&0);
                let r = *frame.get(1).unwrap_or(&l);
                samples.push(l);
                samples.push(r);
            }
        }
    }
    Ok(WavPcm {
        sample_rate: spec.sample_rate,
        channels: 2,
        samples: Arc::new(samples),
    })
}

fn load_mp3_pcm_from_bytes(bytes: &[u8]) -> Result<WavPcm, String> {
    let mut decoder = Mp3Decoder::new(Cursor::new(bytes.to_vec()));
    let mut all_samples: Vec<i16> = Vec::new();
    let mut sample_rate: Option<u32> = None;

    loop {
        match decoder.next_frame() {
            Ok(Mp3Frame {
                data,
                sample_rate: sr,
                channels: ch,
                ..
            }) => {
                if sample_rate.is_none() {
                    sample_rate = Some(sr as u32);
                }
                match ch {
                    1 => {
                        for s in data {
                            all_samples.push(s);
                            all_samples.push(s);
                        }
                    }
                    2 => all_samples.extend_from_slice(&data),
                    _ => {
                        for chunk in data.chunks(ch) {
                            let l = *chunk.first().unwrap_or(&0);
                            let r = *chunk.get(1).unwrap_or(&l);
                            all_samples.push(l);
                            all_samples.push(r);
                        }
                    }
                }
            }
            Err(minimp3::Error::Eof) => break,
            Err(e) => return Err(format!("decode mp3 frame: {e}")),
        }
    }

    if all_samples.is_empty() {
        return Err("mp3 decode produced empty pcm".to_string());
    }

    // LAME-style encoder delay (~1764 stereo frames ≈ 40 ms @ 44.1 kHz).
    let encoder_delay_samples = 1764 * 2;
    if all_samples.len() > encoder_delay_samples {
        all_samples.drain(..encoder_delay_samples);
    }

    Ok(WavPcm {
        sample_rate: sample_rate.unwrap_or(44100),
        channels: 2,
        samples: Arc::new(all_samples),
    })
}

/// Load the first supported BGM from assets. The bundled default song's track
/// wins, then the generic demo tracks.
pub async fn load_audio_pcm_from_assets() -> (Option<String>, Option<WavPcm>) {
    let candidates = ["charts/jack_ripper/track.mp3", "demo.wav", "demo.mp3"];
    for name in candidates {
        if let Ok(bytes) = platform::load_asset_bytes(name).await {
            let parsed = if name.ends_with(".wav") {
                load_wav_pcm_from_bytes(&bytes)
            } else {
                load_mp3_pcm_from_bytes(&bytes)
            };
            if let Ok(pcm) = parsed.map(normalize_to_44100) {
                return (Some(name.to_string()), Some(pcm));
            }
        }
    }
    (None, None)
}

/// Decode an audio byte buffer by extension (`.wav` vs anything else = mp3),
/// normalized to 44.1 kHz.
pub fn decode_audio_bytes(bytes: &[u8], ext: &str) -> Option<WavPcm> {
    let pcm = if ext.eq_ignore_ascii_case("wav") {
        load_wav_pcm_from_bytes(bytes)
    } else {
        load_mp3_pcm_from_bytes(bytes)
    };
    pcm.map(normalize_to_44100).ok()
}

/// Load a BGM from a local file path. Returns the display name and PCM.
pub fn load_audio_from_path(path: &std::path::Path) -> (Option<String>, Option<WavPcm>) {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .or_else(|| Some(path.to_string_lossy().to_string()));
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    match std::fs::read(path) {
        Ok(bytes) => (name, decode_audio_bytes(&bytes, &ext)),
        Err(_) => (name, None),
    }
}

/// Resample arbitrary-rate PCM to 44.1 kHz (linear interpolation).
fn normalize_to_44100(src: WavPcm) -> WavPcm {
    if src.sample_rate == 44_100 {
        return src;
    }
    let ch = src.channels.max(1) as usize;
    let in_frames = src.samples.len() / ch;
    if in_frames == 0 {
        return WavPcm {
            sample_rate: 44_100,
            channels: src.channels.max(1),
            samples: Arc::new(vec![0; ch]),
        };
    }
    let ratio = 44_100.0_f32 / src.sample_rate.max(1) as f32;
    let out_frames = ((in_frames as f32) * ratio).max(1.0).round() as usize;
    let max_src = in_frames.saturating_sub(1);
    let mut out = vec![0_i16; out_frames * ch];
    for out_i in 0..out_frames {
        let src_pos = ((out_i as f32) / ratio).min(max_src as f32);
        let i0 = src_pos.floor() as usize;
        let i1 = (i0 + 1).min(max_src);
        let frac = src_pos - i0 as f32;
        for c in 0..ch {
            let a = src.samples[i0 * ch + c] as f32;
            let b = src.samples[i1 * ch + c] as f32;
            let v = a + (b - a) * frac;
            out[out_i * ch + c] = v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }
    WavPcm {
        sample_rate: 44_100,
        channels: src.channels.max(1),
        samples: Arc::new(out),
    }
}

// ---------------------------------------------------------------------------
// BGM playback (rodio)
// ---------------------------------------------------------------------------

/// Shared PCM cursor; seeking never copies or converts the remaining track.
#[derive(Clone)]
pub struct BgmPcm {
    pub samples: Arc<Vec<i16>>,
    pub channels: u16,
    pub sample_rate: u32,
    cursor: usize,
    silence: usize,
}

impl Iterator for BgmPcm {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.silence > 0 {
            self.silence -= 1;
            return Some(0.0);
        }
        let sample = *self.samples.get(self.cursor)?;
        self.cursor += 1;
        Some(sample as f32 / 32768.0)
    }
}

impl Source for BgmPcm {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.silence + self.samples.len().saturating_sub(self.cursor))
    }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs_f64(
            self.current_frame_len().unwrap_or(0) as f64
                / f64::from(self.channels) / f64::from(self.sample_rate),
        ))
    }
}

/// Pre-decoded one-shot sound effect (e.g. `Sfx/answer.wav`).
#[derive(Clone)]
pub struct SfxBuffer {
    samples: Arc<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
}

impl SfxBuffer {
    /// Decode a WAV file from raw bytes into pre-decoded f32 samples.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let decoder = rodio::Decoder::new(Cursor::new(bytes.to_vec())).ok()?;
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let samples: Vec<f32> = decoder.convert_samples::<f32>().collect();
        Some(Self {
            samples: Arc::new(samples),
            channels,
            sample_rate,
        })
    }
}

/// Load the bundled `Sfx/answer.wav` cue sound.
pub async fn load_answer_sfx() -> Option<SfxBuffer> {
    let bytes = platform::load_asset_bytes("Sfx/answer.wav").await.ok()?;
    SfxBuffer::from_bytes(&bytes)
}

/// Load the first of `candidates` (asset-relative paths) that decodes.
pub async fn load_sfx(candidates: &[&str]) -> Option<SfxBuffer> {
    for path in candidates {
        if let Ok(bytes) = platform::load_asset_bytes(path).await {
            if let Some(buf) = SfxBuffer::from_bytes(&bytes) {
                return Some(buf);
            }
        }
    }
    None
}

const OUTPUT_RATE: u32 = 44_100;
const OUTPUT_CEILING: f32 = 0.95;
const LOOKAHEAD_FRAMES: usize = OUTPUT_RATE as usize * 3 / 1000;

/// Peak limiting happens after all sources are summed. A shared stereo gain
/// preserves balance, with a 3 ms lookahead attack and a 50 ms release.
struct LimitedOutput {
    mixer: DynamicMixer<f32>,
    frames: VecDeque<[f32; 2]>,
    ended: bool,
    right: Option<f32>,
    gain: f32,
    release: f32,
}

impl Iterator for LimitedOutput {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        if !self.ended {
            if let Some(left) = self.mixer.next() {
                let right = self.mixer.next().unwrap_or(0.0);
                self.frames.push_back([left, right]);
            } else {
                self.ended = true;
            }
        }

        let mut gain = self.gain + (1.0 - self.gain) * self.release;
        // Every future peak sets a deadline: ramp down over the frames still
        // available instead of abruptly ducking the background at the hit.
        for (ahead, frame) in self.frames.iter().enumerate() {
            let peak = frame[0].abs().max(frame[1].abs());
            if peak > OUTPUT_CEILING {
                let target = OUTPUT_CEILING / peak;
                gain = gain.min(self.gain + (target - self.gain) / (ahead + 1) as f32);
            }
        }
        let [left, right] = self.frames.pop_front()?;
        self.gain = gain;
        self.right = Some(right * gain);
        Some(left * gain)
    }
}

impl Source for LimitedOutput {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        OUTPUT_RATE
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

fn output_mixer() -> (Arc<DynamicMixerController<f32>>, LimitedOutput) {
    let (controller, mixer) = dynamic_mixer::mixer(2, OUTPUT_RATE);
    let mut frames = VecDeque::with_capacity(LOOKAHEAD_FRAMES + 1);
    frames.resize(LOOKAHEAD_FRAMES, [0.0, 0.0]);
    (
        controller,
        LimitedOutput {
            mixer,
            frames,
            ended: false,
            right: None,
            gain: 1.0,
            release: 1.0 - (-1.0 / (0.05 * OUTPUT_RATE as f32)).exp(),
        },
    )
}

/// Minimal rodio-backed BGM player. One stoppable sink at a time.
pub struct BgmPlayer {
    _stream: OutputStream,
    mixer: Arc<DynamicMixerController<f32>>,
    sink: Option<Sink>,
}

impl BgmPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("rodio output: {e}"))?;
        let (mixer, output) = output_mixer();
        // Keep the bus alive between sounds, including before the first BGM.
        mixer.add(rodio::source::Zero::<f32>::new(2, OUTPUT_RATE));
        handle
            .play_raw(output)
            .map_err(|e| format!("rodio mixer: {e}"))?;
        Ok(Self {
            _stream: stream,
            mixer,
            sink: None,
        })
    }

    pub fn play(&mut self, pcm: BgmPcm, speed: f32) {
        self.stop();
        let source = pcm.speed(speed);
        let (sink, output) = Sink::new_idle();
        sink.append(source);
        self.mixer.add(output);
        self.sink = Some(sink);
    }

    /// Fire a one-shot effect on the shared output without touching the BGM.
    /// The bus mixes it independently of the BGM sink, so it can overlap.
    pub fn play_once(&self, buf: &SfxBuffer, volume: f32) {
        let source =
            SamplesBuffer::new(buf.channels, buf.sample_rate, buf.samples.as_ref().clone())
                .amplify(volume);
        self.mixer.add(source);
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.sink.as_ref().is_some_and(|s| !s.empty())
    }
}

// ---------------------------------------------------------------------------
// Audio servicing
// ---------------------------------------------------------------------------

/// Build and play the BGM at the current speed if a start was requested.
pub async fn service_audio(app: &mut PadPreviewState) {
    if !app.pending_audio_start {
        return;
    }
    app.pending_audio_start = false;

    app.stop_audio_if_any();

    // The song clock stayed frozen while the start was pending; release it now,
    // at the moment playback (or the silent fallback) actually begins, so the
    // timeline does not drift ahead during the audio load and then snap back.
    app.audio_start_pending = false;
    app.mode_wall_anchor = macroquad::prelude::get_time();

    if !app.audio_enabled {
        return;
    }

    let speed = app.current_speed();
    if speed <= 0.0 {
        app.audio_seek_offset = None;
        return;
    }

    if app.audio_wav_pcm.is_some() {
        match load_audio_for_speed(app) {
            Ok(bgm) => {
                if let Some(player) = &mut app.bgm_player {
                    app.mode_wall_anchor = macroquad::prelude::get_time();
                    player.play(bgm, speed);
                }
                app.audio_seek_offset = None;
                app.set_status(format!("Audio speed applied: {:.1}x", speed));
            }
            Err(err) => {
                app.audio_seek_offset = None;
                app.set_status(format!("Audio load failed @ {:.1}x: {err}", speed));
            }
        }
        return;
    }

    if let Some(src) = &app.audio_source_name {
        app.set_status(format!(
            "Audio source loaded: {src} @ {:.1}x",
            app.current_speed()
        ));
    } else {
        app.set_status("Audio disabled: put demo.wav or demo.mp3 in assets/".to_string());
    }
}

fn load_audio_for_speed(app: &mut PadPreviewState) -> Result<BgmPcm, String> {
    let chart_seek = app.audio_seek_offset.unwrap_or(0.0);
    let audio_offset = app.chart.audio_offset;
    let audio_time = chart_seek + audio_offset;
    let effective_seek = audio_time.max(0.0);
    let wav = app
        .audio_wav_pcm
        .as_ref()
        .ok_or_else(|| "pcm source missing".to_string())?;
    let channels = wav.channels.max(1);
    let ch = channels as usize;
    let skip_frames = (effective_seek * wav.sample_rate as f32) as usize;
    let start = (skip_frames * ch).min(wav.samples.len());
    let bgm = BgmPcm {
        samples: Arc::clone(&wav.samples),
        channels,
        sample_rate: wav.sample_rate.max(1),
        cursor: start,
        silence: ((-audio_time).max(0.0) * wav.sample_rate as f32) as usize * ch,
    };
    Ok(bgm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeking_before_audio_start_preserves_chart_alignment() {
        let mut chart = crate::app::chart::fallback_chart();
        chart.audio_offset = -0.5;
        let mut pad = PadPreviewState::new_at(
            chart, None, Some(WavPcm {
                channels: 2, sample_rate: 4, samples: Arc::new(vec![16384; 8]),
            }), 0.0, None,
        );
        pad.audio_seek_offset = Some(0.0);
        let from_start = load_audio_for_speed(&mut pad).unwrap();
        assert_eq!(from_start.take(4).collect::<Vec<_>>(), vec![0.0; 4]);
        pad.audio_seek_offset = Some(0.5);
        let from_audio_start = load_audio_for_speed(&mut pad).unwrap();
        assert_eq!(from_audio_start.collect::<Vec<_>>(), vec![0.5; 8]);
    }

    #[test]
    fn repeated_seek_uses_shared_pcm_and_frame_aligned_cursors() {
        let mut pad = PadPreviewState::new_at(
            crate::app::chart::fallback_chart(), None, Some(WavPcm {
                channels: 2, sample_rate: 4,
                samples: Arc::new(vec![0, 1, 2, 3, 4, 5, 6, 7]),
            }), 0.0, None,
        );
        for offset in [-0.5, 0.0, 0.5] {
            pad.chart.audio_offset = offset;
            for t in [0.0, 0.25, 0.75, 1.0, 0.0] {
                pad.audio_seek_offset = Some(t);
                let mut source = load_audio_for_speed(&mut pad).unwrap();
                assert!(Arc::ptr_eq(&source.samples, &pad.audio_wav_pcm.as_ref().unwrap().samples));
                assert_eq!(source.cursor % 2, 0);
                let expected = (t + offset).max(0.0) * 4.0;
                if t + offset < 0.0 {
                    assert_eq!(source.next(), Some(0.0));
                } else if expected >= 4.0 {
                    assert_eq!(source.next(), None);
                } else {
                    assert_eq!(source.next(), Some((expected as usize * 2) as f32 / 32768.0));
                }
            }
        }
    }

    #[test]
    fn simultaneous_answer_and_hit_sounds_stay_below_full_scale() {
        let answer = SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/answer.wav")).unwrap();
        let hit =
            SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/tap_perfect.wav")).unwrap();
        let (mixer, output) = output_mixer();
        for buf in [&answer, &hit] {
            mixer.add(SamplesBuffer::new(
                buf.channels,
                buf.sample_rate,
                buf.samples.as_ref().clone(),
            ));
        }
        let peak = output
            .take(44_100 * 2)
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        assert!(peak > 0.01, "effects must remain audible");
        assert!(
            peak <= 1.0,
            "overlapping effects overload the output: peak={peak}"
        );
    }

    #[test]
    fn limiter_preserves_quiet_audio_and_links_stereo_gain() {
        let (mixer, output) = output_mixer();
        let quiet = vec![0.1, -0.2, 0.4, -0.3];
        mixer.add(SamplesBuffer::new(2, OUTPUT_RATE, quiet.clone()));
        assert_eq!(output.skip(LOOKAHEAD_FRAMES * 2).collect::<Vec<_>>(), quiet);

        let (mixer, mut output) = output_mixer();
        mixer.add(SamplesBuffer::new(
            2,
            OUTPUT_RATE,
            vec![1.2, -2.4, 0.2, -0.4],
        ));
        for _ in 0..LOOKAHEAD_FRAMES * 2 {
            assert_eq!(output.next(), Some(0.0));
        }
        let left = output.next().unwrap();
        let right = output.next().unwrap();
        assert!((left / right + 0.5).abs() < 1e-6);
        assert!((right + OUTPUT_CEILING).abs() < 1e-6);
        let next_left = output.next().unwrap();
        let next_right = output.next().unwrap();
        assert!(next_left > 0.2 * (OUTPUT_CEILING / 2.4));
        assert!(
            next_left < 0.2,
            "gain must recover smoothly rather than jump"
        );
        assert!((next_left / next_right + 0.5).abs() < 1e-6);
    }

    #[test]
    fn dense_chords_with_bgm_remain_bounded_and_mixer_survives_silence() {
        let (mixer, mut output) = output_mixer();
        mixer.add(rodio::source::Zero::<f32>::new(2, OUTPUT_RATE));
        assert_eq!(
            output.by_ref().take(100).collect::<Vec<_>>(),
            vec![0.0; 100]
        );
        mixer.add(SamplesBuffer::new(2, OUTPUT_RATE, vec![0.8; 2000]));
        let answer = SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/answer.wav")).unwrap();
        let hit =
            SfxBuffer::from_bytes(include_bytes!("../../assets/Sfx/tap_perfect.wav")).unwrap();
        for _ in 0..8 {
            for buf in [&answer, &hit] {
                mixer.add(SamplesBuffer::new(
                    buf.channels,
                    buf.sample_rate,
                    buf.samples.as_ref().clone(),
                ));
            }
        }
        let peak = output
            .take(OUTPUT_RATE as usize * 2)
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        assert!(peak > 0.8);
        assert!(peak <= OUTPUT_CEILING + 1e-6, "peak={peak}");
    }

    #[test]
    fn overlapping_transient_does_not_click_the_background_channel() {
        let (mixer, output) = output_mixer();
        let background: Vec<f32> = (0..2000).flat_map(|_| [0.4, 0.0]).collect();
        let hit: Vec<f32> = (0..2000)
            .flat_map(|frame| [0.0, if frame == 1000 { 8.0 } else { 0.0 }])
            .collect();
        mixer.add(SamplesBuffer::new(2, OUTPUT_RATE, background));
        mixer.add(SamplesBuffer::new(2, OUTPUT_RATE, hit));
        let left: Vec<f32> = output.step_by(2).collect();
        let jump = left[500..1500]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            jump < 0.01,
            "a hit makes the background click: sample jump={jump}"
        );
    }

    #[test]
    fn staggered_signed_peaks_remain_bounded_and_flush_the_tail() {
        let (mixer, output) = output_mixer();
        let samples: Vec<f32> = (0..2000)
            .flat_map(|frame| {
                let peak = match frame {
                    500 => 32.0,
                    550 => -64.0,
                    1999 => 8.0,
                    _ => 0.0,
                };
                [0.4, peak]
            })
            .collect();
        mixer.add(SamplesBuffer::new(2, OUTPUT_RATE, samples));
        let samples: Vec<f32> = output.collect();
        assert_eq!(samples.len(), (2000 + LOOKAHEAD_FRAMES) * 2);
        assert!(
            samples
                .iter()
                .all(|s| s.is_finite() && s.abs() <= OUTPUT_CEILING + 1e-6)
        );
        assert!(samples[2 * (LOOKAHEAD_FRAMES + 550) + 1] < 0.0);
        assert!(
            samples.last().unwrap() > &0.0,
            "the last hit must not be lost"
        );
    }
}
