use macroquad::audio::{load_sound_from_bytes, play_sound_once, Sound};
use minimp3::{Decoder as Mp3Decoder, Frame as Mp3Frame};
use std::io::Cursor;

use super::platform;
use super::state::AppState;
use super::types::{WavPcm, SPEED_MAX, SPEED_MIN};

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
        2 => {
            samples.extend_from_slice(&raw);
        }
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

                // Normalize to stereo PCM16.
                match ch {
                    1 => {
                        for s in data {
                            all_samples.push(s);
                            all_samples.push(s);
                        }
                    }
                    2 => {
                        all_samples.extend_from_slice(&data);
                    }
                    _ => {
                        // Fallback: take first 2 channels stride.
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

    Ok(WavPcm {
        sample_rate: sample_rate.unwrap_or(44100),
        channels: 2,
        samples: all_samples,
    })
}

/// Load first supported BGM from assets.
pub(crate) async fn load_audio_pcm_from_assets() -> (Option<String>, Option<WavPcm>) {
    let candidates = ["demo.wav", "demo.mp3"];
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

pub(crate) async fn service_audio(app: &mut AppState) {
    if !app.pending_audio_start {
        return;
    }
    app.pending_audio_start = false;

    app.stop_audio_if_any();

    if !app.audio_enabled {
        return;
    }

    let speed = app.current_speed();

    if app.audio_wav_pcm.is_some() {
        match load_cached_audio_for_speed(app, speed).await {
            Ok(sound) => {
                app.audio = Some(sound);
                if let Some(s) = &app.audio {
                    play_sound_once(s);
                }
                app.audio_seek_offset = None;
                // Build waveform data on first load
                if app.waveform_data.is_empty() {
                    build_waveform(app);
                }
                app.status = format!("Audio speed applied: {:.1}x", speed);
            }
            Err(err) => {
                app.audio_seek_offset = None;
                app.status = format!("Audio load failed @ {:.1}x: {err}", speed);
            }
        }
        return;
    }

    if let Some(src) = &app.audio_source_name {
        app.status = format!("Audio source loaded: {src} @ {:.1}x", app.current_speed());
    } else {
        app.status =
            "Audio disabled: put demo.wav or demo.mp3 in assets and ensure packaging includes assets/".to_string();
    }
}

fn speed_cache_key(speed: f32) -> i32 {
    (speed.clamp(SPEED_MIN, SPEED_MAX) * 10.0).round() as i32
}

pub(crate) async fn warm_audio_cache(app: &mut AppState, speed: f32) {
    if app.audio_wav_pcm.is_none() {
        return;
    }
    let _ = load_cached_audio_for_speed(app, speed).await;
}

async fn load_cached_audio_for_speed(app: &mut AppState, speed: f32) -> Result<Sound, String> {
    let key = speed_cache_key(speed);
    if let Some(sound) = app.audio_cache.get(&key) {
        return Ok(sound.clone());
    }
    let wav = app
        .audio_wav_pcm
        .as_ref()
        .ok_or_else(|| "pcm source missing".to_string())?;
    let seek = app.audio_seek_offset.unwrap_or(0.0);
    let bytes = build_speed_wav_bytes_impl(wav, speed, seek);
    let sound = load_sound_from_bytes(&bytes)
        .await
        .map_err(|e| format!("{e}"))?;
    app.audio_cache.insert(key, sound.clone());
    Ok(sound)
}

pub(crate) fn build_waveform(app: &mut super::state::AppState) {
    let Some(pcm) = &app.audio_wav_pcm else { return };
    let ch = pcm.channels.max(1) as usize;
    let sr = pcm.sample_rate.max(1) as usize;
    let total_frames = pcm.samples.len() / ch;

    let fft_size = 1024;
    let hop = 512;
    let mut planner = rustfft::FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);

    let mut window: Vec<f32> = (0..fft_size).map(|i| {
        0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos()
    }).collect();

    let time_bins = (total_frames.saturating_sub(fft_size)) / hop + 1;
    let freq_bins = fft_size / 2; // positive frequencies only
    app.waveform_data.clear();
    // Store as flat array: time_bin * freq_bins
    let mut pos = 0;
    while pos + fft_size <= total_frames {
        let mut real: Vec<f32> = (0..fft_size).map(|i| {
            let s = pcm.samples[(pos + i) * ch] as f32 / 32768.0;
            s * window[i]
        }).collect();
        let mut imag = vec![0.0_f32; fft_size];
        // interleave to complex
        let mut complex: Vec<rustfft::num_complex::Complex<f32>> = real.iter().zip(imag.iter())
            .map(|(&r, &i)| rustfft::num_complex::Complex::new(r, i)).collect();
        fft.process(&mut complex);
        // Magnitudes (positive frequencies only)
        for i in 0..freq_bins {
            let mag = complex[i].norm().ln().max(0.0);
            app.waveform_data.push(mag);
        }
        pos += hop;
    }
    // Store metadata for rendering
    app.waveform_freq_bins = freq_bins as u32;
    app.waveform_time_res = hop as f32 / sr as f32;
}

fn build_speed_wav_bytes_impl(wav: &WavPcm, speed: f32, seek_offset: f32) -> Vec<u8> {
    let speed = speed.clamp(SPEED_MIN, SPEED_MAX);
    let channels = wav.channels.max(1);
    let ch = channels as usize;
    let sample_rate = wav.sample_rate as f32;
    let skip_frames = (seek_offset.max(0.0) * sample_rate) as usize;
    let total_frames = wav.samples.len() / ch;
    let in_frames = total_frames.saturating_sub(skip_frames);
    if in_frames == 0 {
        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        let spec = hound::WavSpec {
            channels,
            // quad-snd has an OOB bug in its non-44100 resampler path.
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, spec).expect("wav writer");
        for _ in 0..ch {
            writer.write_sample(0_i16).expect("wav sample write");
        }
        writer.finalize().expect("wav finalize");
        return cursor.into_inner();
    }

    let out_frames = ((in_frames as f32) / speed).max(1.0).round() as usize;
    let mut out_samples = vec![0_i16; out_frames * ch];
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
            out_samples[out_i * ch + c] = v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }

    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    let spec = hound::WavSpec {
        channels,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut cursor, spec).expect("wav writer");
    for s in out_samples {
        writer.write_sample(s).expect("wav sample write");
    }
    writer.finalize().expect("wav finalize");
    cursor.into_inner()
}
