use minimp3::{Decoder as Mp3Decoder, Frame as Mp3Frame};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink, Source};
use std::io::Cursor;
use std::thread;
use std::time::{Duration, Instant};

// ── Chart helpers (copied from app) ────────────────────────────────
fn measure_to_secs(m: f32, bpm: f32) -> f32 {
    (m - 1.0) * 240.0 / bpm
}

const TICKS_PER_MEASURE: i32 = 384;
const TICKS_PER_BEAT: i32 = 96;

fn beat_pos_to_measure(measure: i32, beat: i32, division: i32, offset: i32) -> f32 {
    let tick = (measure - 1) * TICKS_PER_MEASURE
        + (beat - 1) * TICKS_PER_BEAT
        + if division > 0 {
            offset * TICKS_PER_BEAT / division
        } else {
            0
        };
    1.0 + tick as f32 / TICKS_PER_MEASURE as f32
}

// ── Audio helpers ──────────────────────────────────────────────────
fn decode_mp3(bytes: &[u8]) -> (Vec<i16>, u32) {
    let mut decoder = Mp3Decoder::new(Cursor::new(bytes.to_vec()));
    let mut all: Vec<i16> = Vec::new();
    let mut sr = 44100u32;
    loop {
        match decoder.next_frame() {
            Ok(Mp3Frame {
                data,
                sample_rate,
                channels,
                ..
            }) => {
                sr = sample_rate as u32;
                let ch = channels.max(1) as usize;
                if ch == 2 {
                    all.extend_from_slice(&data);
                } else {
                    for &s in &data {
                        all.push(s);
                        all.push(s);
                    }
                }
            }
            Err(minimp3::Error::Eof) => break,
            Err(_) => break,
        }
    }
    (all, sr)
}

fn resample_to_44100(samples: &[i16], src_rate: u32) -> Vec<i16> {
    if src_rate == 44100 {
        return samples.to_vec();
    }
    let ch = 2usize;
    let in_frames = samples.len() / ch;
    let ratio = 44100.0f32 / src_rate as f32;
    let out_frames = ((in_frames as f32) * ratio).round() as usize;
    let max_src = in_frames.saturating_sub(1);
    let mut out = vec![0i16; out_frames * ch];
    for i in 0..out_frames {
        let src_pos = (i as f32 / ratio).min(max_src as f32);
        let i0 = src_pos.floor() as usize;
        let i1 = (i0 + 1).min(max_src);
        let frac = src_pos - i0 as f32;
        for c in 0..ch {
            let a = samples[i0 * ch + c] as f32;
            let b = samples[i1 * ch + c] as f32;
            out[i * ch + c] = (a + (b - a) * frac).round().clamp(-32768.0, 32767.0) as i16;
        }
    }
    out
}

fn pcm_to_wav_bytes(samples: &[i16], channels: u16, sample_rate: u32) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let mut buf = Vec::with_capacity(44 + data_size as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

// ── Parse note times from chart JSON ───────────────────────────────
fn load_note_times(json: &str) -> (f32, f32, Vec<f32>) {
    let v: serde_json::Value = serde_json::from_str(json).expect("bad json");
    let bpm = v["bpm"].as_f64().unwrap() as f32;
    let audio_offset = v["audio_offset"].as_f64().unwrap_or(0.0) as f32;
    let notes = v["notes"].as_array().unwrap();
    let mut times: Vec<f32> = Vec::new();
    for n in notes {
        let measure = n["measure"].as_i64().unwrap() as i32;
        let beat = n["beat"].as_i64().unwrap() as i32;
        let division = n["division"].as_i64().unwrap_or(1) as i32;
        let offset = n["offset"].as_i64().unwrap_or(0) as i32;
        let m = beat_pos_to_measure(measure, beat, division, offset);
        times.push(measure_to_secs(m, bpm));
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    (bpm, audio_offset, times)
}

/// Detect the time (in seconds) of the first significant audio onset.
fn detect_onset(samples: &[i16], sample_rate: u32) -> f32 {
    let ch = 2usize;
    let frames = samples.len() / ch;
    // RMS over 10ms windows
    let window = (sample_rate as usize / 100).max(1);
    let threshold = 500.0f32; // ~1.5% of i16 max
    for start in (0..frames).step_by(window / 4) {
        let end = (start + window).min(frames);
        let mut sum = 0.0f64;
        for i in start..end {
            for c in 0..ch {
                let s = samples[i * ch + c] as f64;
                sum += s * s;
            }
        }
        let rms = (sum / ((end - start) * ch) as f64).sqrt() as f32;
        if rms > threshold {
            return start as f32 / sample_rate as f32;
        }
    }
    0.0
}

/// Play BGM with note clicks at a given audio_offset, for `duration` seconds.
fn play_test(
    handle: &rodio::OutputStreamHandle,
    bgm_wav: &[u8],
    tap_ch: u16,
    tap_sr: u32,
    tap_samples: &[f32],
    note_times: &[f32],
    audio_offset: f32,
    duration: f32,
) {
    let bgm_sink = Sink::try_new(handle).unwrap();
    let bgm_source = rodio::Decoder::new(Cursor::new(bgm_wav.to_vec())).unwrap();
    bgm_sink.append(bgm_source);

    let start = Instant::now();
    let mut next_idx = 0;
    let mut played = 0u32;

    loop {
        let elapsed = start.elapsed().as_secs_f32();
        if elapsed > duration {
            break;
        }

        while next_idx < note_times.len() {
            let trigger_t = note_times[next_idx] + audio_offset;
            if trigger_t > duration {
                next_idx = note_times.len();
                break;
            }
            if elapsed >= trigger_t {
                let buf = SamplesBuffer::new(tap_ch, tap_sr, tap_samples.to_vec());
                let _ = handle.play_raw(buf.amplify(0.5).convert_samples());
                played += 1;
                next_idx += 1;
            } else {
                break;
            }
        }
        thread::sleep(Duration::from_millis(1));
    }

    bgm_sink.stop();
    println!(
        "  Played {} clicks with offset={:.3}s",
        played, audio_offset
    );
}

fn main() {
    // ── Load assets ────────────────────────────────────────────────
    let mp3_bytes = include_bytes!("../../assets/demo.mp3");
    let tap_wav = include_bytes!("../../assets/Sfx/tap_perfect.wav");
    let chart_json = include_str!("../../output/latest_chart.json");

    println!("Decoding MP3...");
    let (pcm, src_rate) = decode_mp3(mp3_bytes);
    println!(
        "  MP3 decoded: {} stereo samples, {}Hz",
        pcm.len() / 2,
        src_rate
    );

    let pcm44 = resample_to_44100(&pcm, src_rate);
    let bgm_duration = pcm44.len() as f32 / 2.0 / 44100.0;
    println!("  44100Hz, duration: {:.2}s", bgm_duration);

    let (bpm, _chart_offset, note_times) = load_note_times(chart_json);
    println!("Chart: bpm={}, {} unique note times", bpm, note_times.len());
    println!(
        "  First note at {:.3}s, last at {:.3}s",
        note_times.first().unwrap_or(&0.0),
        note_times.last().unwrap_or(&0.0)
    );

    // Detect actual audio onset
    let onset = detect_onset(&pcm44, 44100);
    println!("Audio onset detected at: {:.3}s", onset);

    // Beat interval
    let beat_sec = 60.0 / bpm;
    println!("Beat interval: {:.3}s ({} BPM)", beat_sec, bpm);

    // Pre-decode tap
    let cursor = Cursor::new(tap_wav.to_vec());
    let dec = rodio::Decoder::new(cursor).expect("tap decode");
    let tap_sr = dec.sample_rate();
    let tap_ch = dec.channels();
    let tap_samples: Vec<f32> = dec.convert_samples::<f32>().collect();

    // Build BGM WAV
    let bgm_wav = pcm_to_wav_bytes(&pcm44, 2, 44100);

    let (_stream, handle) = OutputStream::try_default().expect("audio output");

    // ── Get offset from CLI or auto-scan ───────────────────────────
    let args: Vec<String> = std::env::args().collect();
    if let Some(offset_str) = args.get(1) {
        // Single test with explicit offset
        let offset: f32 = offset_str
            .parse()
            .expect("usage: sfx_test <offset_seconds>");
        println!(
            "\n=== Testing with audio_offset = {:.3}s (full song) ===",
            offset
        );
        println!("Ctrl+C to stop\n");
        play_test(
            &handle,
            &bgm_wav,
            tap_ch,
            tap_sr,
            &tap_samples,
            &note_times,
            offset,
            bgm_duration,
        );
    } else {
        // Auto-scan: try offsets near the onset
        let test_duration = 15.0; // play 15s per test
        let candidates = [
            onset - 0.2,
            onset,
            onset + 0.1,
            onset + 0.2,
            onset + 0.3,
            onset + 0.5,
        ];
        for (i, &offset) in candidates.iter().enumerate() {
            let offset = offset.max(0.0);
            println!(
                "\n=== Test {} / {}: audio_offset = {:.3}s ===",
                i + 1,
                candidates.len(),
                offset
            );
            println!(
                "  (first {}s, listen if clicks land on beats)",
                test_duration
            );
            thread::sleep(Duration::from_millis(800));
            play_test(
                &handle,
                &bgm_wav,
                tap_ch,
                tap_sr,
                &tap_samples,
                &note_times,
                offset,
                test_duration,
            );
            thread::sleep(Duration::from_millis(1500));
        }
        println!("\n========================================");
        println!("Pick the test number where clicks aligned best,");
        println!("then re-run with that offset:");
        println!("  cargo run --release --bin sfx_test -- <offset>");
        println!("Or set it in the app's UI (Offset +/- buttons).");
    }
}
