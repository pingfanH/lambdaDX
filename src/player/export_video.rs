//! Offscreen video export via the `ffmpeg` sidecar (encoder side).
//!
//! Symmetric to [`super::video`] (which decodes a background video): frames are
//! rendered offscreen at `render_w × render_h` and piped as raw RGBA into
//! `ffmpeg`'s stdin. ffmpeg flips (GL render targets read back bottom-up),
//! optionally upscales to the output size, encodes H.264 and muxes the track.
//!
//! ```text
//! ffmpeg -y -f rawvideo -pix_fmt rgba -s <render> -r FPS -i - \
//!        [-ss <first> -i track.mp3] \
//!        -vf vflip[,scale=<out>] -c:v <encoder> ... out.mp4
//! ```
//!
//! Encoder defaults to VideoToolbox on macOS (hardware, much faster) and
//! libx264 elsewhere; override with [`Encoder`].

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;

/// Which H.264 encoder to use.
#[derive(Debug, Clone)]
pub enum Encoder {
    Auto,
    /// `libx264` with `-preset` / `-crf`.
    X264,
    /// Hardware VideoToolbox (`-b:v`).
    VideoToolbox,
    /// Any ffmpeg encoder name (`-b:v`).
    Named(String),
}

impl Encoder {
    pub fn parse(s: &str) -> Encoder {
        match s {
            "auto" => Encoder::Auto,
            "libx264" | "x264" => Encoder::X264,
            "h264_videotoolbox" | "videotoolbox" => Encoder::VideoToolbox,
            other => Encoder::Named(other.to_string()),
        }
    }
}

/// Everything needed to start an export.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub output: PathBuf,
    /// Source audio file to mux (skipped when `None`).
    pub audio: Option<PathBuf>,
    /// Seconds to skip in the audio (`&first`, so audio t=0 aligns with song 0).
    pub audio_offset: f32,
    /// Size we render frames at (the rawvideo input size).
    pub render_w: u32,
    pub render_h: u32,
    /// Final video size (upscaled from the render size when they differ).
    pub out_w: u32,
    pub out_h: u32,
    pub fps: u32,
    pub encoder: Encoder,
    /// Bitrate for hardware/`Named` encoders (e.g. `"12M"`).
    pub bitrate: String,
    pub crf: u32,
    pub preset: String,
}

impl ExportConfig {
    /// Frame size in bytes (RGBA8) at the render resolution.
    pub fn frame_bytes(&self) -> usize {
        (self.render_w as usize) * (self.render_h as usize) * 4
    }
}

/// A running `ffmpeg` encoder. A background thread feeds frames to its stdin so
/// rendering and encoding overlap instead of serialising on a blocking pipe.
pub struct VideoEncoder {
    tx: Option<SyncSender<Vec<u8>>>,
    handle: Option<JoinHandle<Result<(), String>>>,
    frame_bytes: usize,
}

impl VideoEncoder {
    /// Spawn `ffmpeg`. Errors clearly if the binary is missing.
    pub fn spawn(cfg: &ExportConfig) -> Result<Self, String> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{}x{}", cfg.render_w, cfg.render_h),
            "-r",
            &cfg.fps.to_string(),
            "-i",
            "-",
        ]);
        if let Some(audio) = &cfg.audio {
            if cfg.audio_offset > 1e-4 {
                cmd.args(["-ss", &format!("{:.6}", cfg.audio_offset)]);
            }
            cmd.arg("-i").arg(audio);
        }

        // `vflip`: GL render targets read back bottom-up. Upscale if needed.
        let filter = if (cfg.render_w, cfg.render_h) != (cfg.out_w, cfg.out_h) {
            format!("vflip,scale={}:{}", cfg.out_w, cfg.out_h)
        } else {
            "vflip".to_string()
        };
        cmd.args(["-vf", &filter]);

        match &cfg.encoder {
            Encoder::X264 => {
                cmd.args([
                    "-c:v",
                    "libx264",
                    "-preset",
                    &cfg.preset,
                    "-crf",
                    &cfg.crf.to_string(),
                ]);
            }
            Encoder::VideoToolbox => {
                cmd.args(["-c:v", "h264_videotoolbox", "-b:v", &cfg.bitrate]);
            }
            Encoder::Named(name) => {
                cmd.args(["-c:v", name, "-b:v", &cfg.bitrate]);
            }
            Encoder::Auto => {
                // `resolve_encoder` should have picked a concrete one.
                cmd.args(["-c:v", "libx264", "-preset", &cfg.preset, "-crf", &cfg.crf.to_string()]);
            }
        }
        cmd.args(["-pix_fmt", "yuv420p"]);
        if cfg.audio.is_some() {
            cmd.args(["-c:a", "aac", "-b:a", "192k", "-shortest"]);
        }
        cmd.arg(&cfg.output);

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot start `ffmpeg` ({e}); install it to export video"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "ffmpeg stdin unavailable".to_string())?;

        // Bounded queue: rendering keeps going while ffmpeg encodes; if ffmpeg
        // falls behind we apply backpressure instead of buffering the whole clip.
        let (tx, rx) = sync_channel::<Vec<u8>>(8);
        let handle = std::thread::spawn(move || -> Result<(), String> {
            while let Ok(frame) = rx.recv() {
                stdin
                    .write_all(&frame)
                    .map_err(|e| format!("ffmpeg write failed: {e}"))?;
            }
            drop(stdin);
            wait_child(&mut child)
        });

        Ok(Self {
            tx: Some(tx),
            handle: Some(handle),
            frame_bytes: cfg.frame_bytes(),
        })
    }

    /// Hand one RGBA frame (exactly `render_w * render_h * 4` bytes).
    pub fn write_frame(&mut self, rgba: Vec<u8>) -> Result<(), String> {
        if rgba.len() != self.frame_bytes {
            return Err(format!(
                "frame size mismatch: got {}, expected {}",
                rgba.len(),
                self.frame_bytes
            ));
        }
        let tx = self.tx.as_ref().ok_or("encoder already finished")?;
        tx.send(rgba)
            .map_err(|_| "encoder thread stopped".to_string())
    }

    /// Flush and wait for `ffmpeg` to finish muxing.
    pub fn finish(mut self) -> Result<(), String> {
        self.tx.take(); // close the channel
        match self.handle.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "encoder thread panicked".to_string())?,
            None => Ok(()),
        }
    }
}

fn wait_child(child: &mut Child) -> Result<(), String> {
    let status = child
        .wait()
        .map_err(|e| format!("ffmpeg wait failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited with {status}"))
    }
}

/// Turn [`Encoder::Auto`] into a concrete encoder (VideoToolbox on macOS).
pub fn resolve_encoder(encoder: &Encoder) -> Encoder {
    match encoder {
        Encoder::Auto if cfg!(target_os = "macos") => Encoder::VideoToolbox,
        Encoder::Auto => Encoder::X264,
        other => other.clone(),
    }
}

/// Parse a `WxH` size string (e.g. `1920x1080`).
pub fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("invalid size `{s}` (expected WxH)"))?;
    let w = w.trim().parse::<u32>().map_err(|_| format!("bad width in `{s}`"))?;
    let h = h.trim().parse::<u32>().map_err(|_| format!("bad height in `{s}`"))?;
    if w == 0 || h == 0 {
        return Err(format!("invalid size `{s}`"));
    }
    // H.264 needs even dimensions.
    Ok((w & !1, h & !1))
}
