use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::sync::Arc;

/// Pre-decoded audio samples ready for instant playback.
#[derive(Clone)]
pub struct SfxBuffer {
    samples: Arc<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
}

impl SfxBuffer {
    /// Decode a WAV file from raw bytes into pre-decoded f32 samples.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let cursor = Cursor::new(bytes.to_vec());
        let decoder = rodio::Decoder::new(cursor).ok()?;
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

/// Low-latency sound effect player backed by rodio/cpal.
pub struct SfxPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    /// Looping sink for touch-hold riser (needs stop support)
    riser_sink: Option<Sink>,
    /// BGM sink (stoppable)
    bgm_sink: Option<Sink>,
}

impl SfxPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("rodio output: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            riser_sink: None,
            bgm_sink: None,
        })
    }

    /// Play a one-shot sound at the given volume (0.0–1.0) via play_raw (no Sink overhead).
    pub fn play(&self, buf: &SfxBuffer, volume: f32) {
        let source =
            SamplesBuffer::new(buf.channels, buf.sample_rate, buf.samples.as_ref().clone())
                .amplify(volume);
        let _ = self.handle.play_raw(source.convert_samples());
    }

    /// Start looping a sound. Only one riser can be active at a time.
    pub fn play_looped(&mut self, buf: &SfxBuffer, volume: f32) {
        self.stop_looped();
        let source =
            SamplesBuffer::new(buf.channels, buf.sample_rate, buf.samples.as_ref().clone())
                .amplify(volume)
                .repeat_infinite();
        let sink = Sink::try_new(&self.handle).unwrap();
        sink.append(source);
        self.riser_sink = Some(sink);
    }

    /// Stop the currently looping sound.
    pub fn stop_looped(&mut self) {
        if let Some(sink) = self.riser_sink.take() {
            sink.stop();
        }
    }

    /// Play BGM from pre-decoded f32 samples (zero-copy, no WAV roundtrip).
    pub fn play_bgm(&mut self, samples: &[f32], channels: u16, sample_rate: u32) {
        self.stop_bgm();
        let source = SamplesBuffer::new(channels, sample_rate, samples.to_vec());
        let sink = Sink::try_new(&self.handle).unwrap();
        sink.set_volume(1.0);
        sink.append(source);
        self.bgm_sink = Some(sink);
    }

    /// Stop BGM.
    pub fn stop_bgm(&mut self) {
        if let Some(sink) = self.bgm_sink.take() {
            sink.stop();
        }
    }
}
