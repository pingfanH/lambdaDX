use minimp3::{Decoder as Mp3Decoder, Frame as Mp3Frame};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::sync::Arc;

use super::platform;
use super::types::{SPEED_MAX, SPEED_MIN, WavPcm};
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
        samples,
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
        samples: all_samples,
    })
}

/// Load the first supported BGM from assets. The bundled default song's track
/// wins, then the generic demo tracks.
pub async fn load_audio_pcm_from_assets() -> (Option<String>, Option<WavPcm>) {
    let candidates = [
        "charts/jack_ripper/track.mp3",
        "demo.wav",
        "demo.mp3",
    ];
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
            samples: vec![0; ch],
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
        samples: out,
    }
}

// ---------------------------------------------------------------------------
// BGM playback (rodio)
// ---------------------------------------------------------------------------

/// Pre-decoded f32 BGM samples ready for instant playback.
#[derive(Clone)]
pub struct BgmPcm {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
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

/// Minimal rodio-backed BGM player. One stoppable sink at a time.
pub struct BgmPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
}

impl BgmPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("rodio output: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
        })
    }

    pub fn play(&mut self, samples: &[f32], channels: u16, sample_rate: u32) {
        self.stop();
        let source = SamplesBuffer::new(channels, sample_rate, samples.to_vec());
        if let Ok(sink) = Sink::try_new(&self.handle) {
            sink.set_volume(1.0);
            sink.append(source);
            self.sink = Some(sink);
        }
    }

    /// Fire a one-shot effect on the shared output without touching the BGM.
    /// `play_raw` mixes it independently of the BGM sink, so it can overlap.
    pub fn play_once(&self, buf: &SfxBuffer, volume: f32) {
        let source =
            SamplesBuffer::new(buf.channels, buf.sample_rate, buf.samples.as_ref().clone())
                .amplify(volume);
        let _ = self.handle.play_raw(source.convert_samples());
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

fn speed_cache_key(speed: f32) -> i32 {
    (speed.clamp(SPEED_MIN, SPEED_MAX) * 10.0).round() as i32
}

/// Build and play the BGM at the current speed if a start was requested.
pub async fn service_audio(app: &mut PadPreviewState) {
    if !app.pending_audio_start {
        return;
    }
    app.pending_audio_start = false;

    app.stop_audio_if_any();

    if !app.audio_enabled {
        return;
    }

    let speed = app.current_speed();
    if speed <= 0.0 {
        app.audio_seek_offset = None;
        return;
    }

    if app.audio_wav_pcm.is_some() {
        match load_cached_audio_for_speed(app, speed) {
            Ok(bgm) => {
                if let Some(player) = &mut app.bgm_player {
                    app.mode_wall_anchor = macroquad::prelude::get_time();
                    player.play(&bgm.samples, bgm.channels, bgm.sample_rate);
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

fn load_cached_audio_for_speed(app: &mut PadPreviewState, speed: f32) -> Result<BgmPcm, String> {
    let key = speed_cache_key(speed);
    let chart_seek = app.audio_seek_offset.unwrap_or(0.0);
    let audio_offset = app.chart.audio_offset;
    let effective_seek = (chart_seek + audio_offset).max(0.0);

    if chart_seek <= 0.0 {
        if let Some(cached) = app.audio_cache.get(&key) {
            return Ok(cached.clone());
        }
    }
    let wav = app
        .audio_wav_pcm
        .as_ref()
        .ok_or_else(|| "pcm source missing".to_string())?;
    let (samples_i16, channels) = build_speed_pcm(wav, speed, effective_seek);
    let samples_f32: Vec<f32> = samples_i16.iter().map(|&s| s as f32 / 32768.0).collect();
    let bgm = BgmPcm {
        samples: samples_f32,
        channels,
        sample_rate: wav.sample_rate,
    };
    if chart_seek <= 0.0 {
        app.audio_cache.insert(key, bgm.clone());
    }
    Ok(bgm)
}

/// Build speed-adjusted raw PCM i16 samples. Returns (samples, channels).
/// When `speed == 1.0` this is a plain copy from `seek_offset` onward, which is
/// also what keeps pitch unchanged at the common case.
fn build_speed_pcm(wav: &WavPcm, speed: f32, seek_offset: f32) -> (Vec<i16>, u16) {
    let speed = speed.clamp(SPEED_MIN, SPEED_MAX);
    let channels = wav.channels.max(1);
    let ch = channels as usize;
    let sample_rate = wav.sample_rate as f32;
    let skip_frames = (seek_offset.max(0.0) * sample_rate) as usize;
    let total_frames = wav.samples.len() / ch;
    let in_frames = total_frames.saturating_sub(skip_frames);

    if in_frames == 0 {
        return (vec![0; ch], channels);
    }

    if (speed - 1.0).abs() < 0.001 {
        let start = skip_frames * ch;
        return (wav.samples[start..].to_vec(), channels);
    }

    let out_frames = ((in_frames as f32) / speed).max(1.0).round() as usize;
    let mut out = vec![0_i16; out_frames * ch];
    let max_src_i = total_frames.saturating_sub(1);

    for out_i in 0..out_frames {
        let src_pos = skip_frames as f32 + (out_i as f32 * speed).min(in_frames as f32);
        let src_i0 = (src_pos.floor() as usize).min(max_src_i);
        let src_i1 = (src_i0 + 1).min(max_src_i);
        let frac = src_pos - src_i0 as f32;
        for c in 0..ch {
            let a = wav.samples[src_i0 * ch + c] as f32;
            let b = wav.samples[src_i1 * ch + c] as f32;
            let v = a + (b - a) * frac;
            out[out_i * ch + c] = v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }

    (out, channels)
}
