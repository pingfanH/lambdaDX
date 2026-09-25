//! Background video decoding via an external `ffmpeg` sidecar.
//!
//! Macroquad has no video support and pulling in ffmpeg's C bindings would make
//! the build heavy, so the pad preview pipes raw RGBA frames out of the system
//! `ffmpeg` binary instead:
//!
//! ```text
//! ffmpeg -v error -ss <local> -i bg.mp4 -f rawvideo -pix_fmt rgba -s WxH -r FPS -
//! ```
//!
//! A worker thread reads frames off `ffmpeg`'s stdout into a small bounded
//! channel; [`VideoBg::sync`] picks the frame matching the current song time and
//! uploads it to a [`Texture2D`]. Seeking (song scrub, loop wrap, forward jump)
//! restarts `ffmpeg` at the target offset. If the binary is missing the video is
//! simply skipped.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::thread;

use macroquad::prelude::Texture2D;

/// Frame distance ahead of the decoded output that counts as a real forward
/// seek (and thus a decoder restart) rather than routine fast-forwarding.
const FORWARD_RESTART_FRAMES: u64 = 32;
/// Bounded frame queue; the decoder blocks when it is full, pacing itself.
/// A few frames of slack absorb brief decode hiccups without over-buffering.
const QUEUE_CAP: usize = 8;

/// Per-frame video settings (read from [`crate::app::params`]).
#[derive(Debug, Clone)]
pub struct VideoConfig {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub start: f32,
    pub fps: f32,
    pub height: usize,
    pub looping: bool,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            start: 0.0,
            fps: 0.0,
            height: 1080,
            looping: true,
        }
    }
}

struct Frame {
    index: u64,
    data: Vec<u8>,
}

/// Streaming background video decoder. Owns the ffmpeg child + frame texture.
pub struct VideoBg {
    tex: Option<Texture2D>,
    rx: Option<Receiver<Vec<u8>>>,
    child: Option<Child>,
    buffer: VecDeque<Frame>,

    path: Option<PathBuf>,
    fps: f32,
    /// Native source resolution and frame rate (from ffprobe).
    src_w: usize,
    src_h: usize,
    native_fps: f32,
    /// Decode output resolution (never upscaled past native).
    out_w: usize,
    out_h: usize,
    duration: f64,

    /// Index of the next frame the worker will emit.
    next_index: u64,
    /// `next_index` at the moment the current decoder was (re)started.
    base_index: u64,
    /// Index currently uploaded to `tex` (`-1` = none).
    shown_index: i64,
    /// `want` from the previous sync, used to detect backward seeks.
    last_want: Option<u64>,
    /// Set once a spawn failed so we stop retrying every frame.
    unavailable: bool,
}

impl Default for VideoBg {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoBg {
    pub fn new() -> Self {
        Self {
            tex: None,
            rx: None,
            child: None,
            buffer: VecDeque::new(),
            path: None,
            fps: 30.0,
            src_w: 0,
            src_h: 0,
            native_fps: 0.0,
            out_w: 0,
            out_h: 0,
            duration: 0.0,
            next_index: 0,
            base_index: 0,
            shown_index: -1,
            last_want: None,
            unavailable: false,
        }
    }

    /// The texture for the current frame, if any has been decoded.
    pub fn texture(&self) -> Option<&Texture2D> {
        self.tex.as_ref()
    }

    /// A frame's display is only meaningful once we have a texture.
    pub fn is_ready(&self) -> bool {
        self.tex.is_some()
    }

    /// Advance the video to match `song_time` (called once per frame, before
    /// drawing).
    pub fn sync(&mut self, cfg: &VideoConfig, song_time: f32) {
        if !cfg.enabled {
            self.stop();
            return;
        }
        let Some(path) = cfg.path.clone() else {
            self.stop();
            return;
        };
        if self.unavailable {
            return;
        }

        // Probe once per path; then the output size is derived from the native
        // resolution so we never upscale a low-res decode (which looked blurry).
        let req_h = cfg.height.max(2);
        let path_changed = self.path.as_deref() != Some(path.as_path());
        if path_changed {
            match probe(&path) {
                Some((dur, w, h, fps)) => {
                    self.unavailable = false;
                    self.path = Some(path.clone());
                    self.src_w = w;
                    self.src_h = h;
                    self.native_fps = fps;
                    self.duration = dur;
                }
                None => {
                    self.unavailable = true;
                    return;
                }
            }
        }

        // `bg_video_fps == 0` means "follow the source" (no resample downsample).
        let eff_fps = if cfg.fps > 0.0 {
            cfg.fps
        } else if self.native_fps > 0.0 {
            self.native_fps
        } else {
            30.0
        };

        let out_h = req_h.min(self.src_h.max(2));
        let out_w = ((out_h as f32 * self.src_w as f32 / self.src_h.max(1) as f32).round() as usize)
            .max(2)
            & !1;
        let size_changed = self.out_h != out_h || self.out_w != out_w;
        let fps_changed = (self.fps - eff_fps).abs() > 1e-3;
        if path_changed || size_changed || fps_changed {
            self.fps = eff_fps.max(1.0);
            self.out_h = out_h;
            self.out_w = out_w;
            self.stop_decoder();
            self.base_index = 0;
            self.next_index = 0;
            self.shown_index = -1;
            self.last_want = None;
            self.tex = None;
            self.buffer.clear();
        }
        if self.out_w == 0 {
            return;
        }

        self.pull();

        let dur_s = self.duration as f32;
        // `bg_video_start` is the video time (seconds) shown at song `t = 0`.
        let video_time = cfg.start + song_time;
        let local = if cfg.looping && self.duration > 0.0 {
            video_time.rem_euclid(dur_s)
        } else {
            video_time.clamp(0.0, dur_s.max(0.0))
        };
        let want = (local * self.fps).floor().max(0.0) as u64;

        // Fast-forward through buffered frames older than `want`.
        while let Some(front) = self.buffer.front() {
            if front.index < want {
                self.buffer.pop_front();
            } else {
                break;
            }
        }

        // Restart the decoder only when the timeline actually moved: a backward
        // seek/loop wrap (want decreased) or a forward jump past what has been
        // decoded. A decoder that is merely *ahead* of `want` is the normal
        // state — treating it as a seek would respawn ffmpeg every frame.
        let backward = self.last_want.is_some_and(|lw| want + 1 < lw);
        // Only a *real* forward seek restarts: judge it against the decoder's
        // output once that output has started moving (`next_index > base`),
        // otherwise the post-spawn gap would trigger it every frame.
        let decoded_something = self.next_index > self.base_index;
        let forward_seek =
            decoded_something && want > self.next_index + FORWARD_RESTART_FRAMES;
        let need_restart = self.child.is_none() || backward || forward_seek;
        if need_restart && self.spawn_at(&path, want).is_err() {
            self.unavailable = true;
            return;
        }
        self.last_want = Some(want);

        if self.buffer.front().map(|f| f.index) == Some(want) {
            if let Some(frame) = self.buffer.pop_front() {
                self.upload(&frame.data);
                self.shown_index = want as i64;
            }
        }
    }

    /// Upload a frame, reusing the existing texture when the size matches.
    /// Allocating a fresh texture every frame (as `from_rgba8` does) causes
    /// visible stutter, so only the first frame creates one.
    fn upload(&mut self, data: &[u8]) {
        match &self.tex {
            Some(tex) => tex.update_from_bytes(self.out_w as u32, self.out_h as u32, data),
            None => {
                self.tex = Some(Texture2D::from_rgba8(
                    self.out_w as u16,
                    self.out_h as u16,
                    data,
                ));
            }
        }
    }

    /// Drain everything currently available from the worker.
    fn pull(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(data) => {
                    let index = self.next_index;
                    self.next_index += 1;
                    self.buffer.push_back(Frame { index, data });
                    // Keep the in-memory queue small: at 1080p each frame is
                    // several MB, so we do not want to buffer dozens.
                    if self.buffer.len() >= QUEUE_CAP {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            }
        }
    }

    /// Kill the current decoder and start one positioned at `index`.
    fn spawn_at(&mut self, path: &Path, index: u64) -> Result<(), ()> {
        self.stop_decoder();
        let seek = index as f32 / self.fps;
        let (child, rx) = spawn_ffmpeg(path, seek, self.out_w, self.out_h, self.fps).map_err(|_| ())?;
        self.child = Some(child);
        self.rx = Some(rx);
        self.buffer.clear();
        self.base_index = index;
        self.next_index = index;
        Ok(())
    }

    fn stop_decoder(&mut self) {
        self.rx = None;
        self.buffer.clear();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Stop the decoder and drop the texture.
    pub fn stop(&mut self) {
        self.stop_decoder();
        self.tex = None;
        self.shown_index = -1;
        self.last_want = None;
    }
}

impl Drop for VideoBg {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawn `ffmpeg` decoding `path` from `seek` seconds into `w`×`h` RGBA frames.
fn spawn_ffmpeg(
    path: &Path,
    seek: f32,
    w: usize,
    h: usize,
    fps: f32,
) -> std::io::Result<(Child, Receiver<Vec<u8>>)> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-ss",
            &format!("{:.4}", seek.max(0.0)),
            "-i",
        ])
        .arg(path)
        .args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-an",
            "-s",
            &format!("{w}x{h}"),
            "-r",
            &format!("{:.4}", fps.max(1.0)),
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE_CAP);
    let frame_size = w * h * 4;
    thread::spawn(move || {
        let mut frame = vec![0u8; frame_size];
        loop {
            if stdout.read_exact(&mut frame).is_err() {
                break;
            }
            if tx.send(frame.clone()).is_err() {
                break;
            }
        }
    });
    Ok((child, rx))
}

/// Probe `(duration_seconds, width, height, fps)` for the first video stream.
fn probe(path: &Path) -> Option<(f64, usize, usize, f32)> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let stream = v.get("streams")?.get(0)?;
    let w = stream.get("width")?.as_u64()? as usize;
    let h = stream.get("height")?.as_u64()? as usize;
    if w == 0 || h == 0 {
        return None;
    }
    let fps = stream
        .get("r_frame_rate")
        .and_then(|f| f.as_str())
        .and_then(parse_rational)
        .unwrap_or(0.0);
    let dur = v
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    Some((dur, w, h, fps))
}

/// Parse an ffmpeg rational frame rate like `"60000/1001"`.
fn parse_rational(s: &str) -> Option<f32> {
    let (n, d) = s.split_once('/')?;
    let n: f32 = n.trim().parse().ok()?;
    let d: f32 = d.trim().parse().ok()?;
    (d != 0.0 && n > 0.0).then_some(n / d)
}

/// Resolve the configured video path (empty = `<assets>/bg.mp4`).
pub fn resolve_path(explicit: &str) -> Option<PathBuf> {
    if explicit.trim().is_empty() {
        let p = crate::app::platform::asset_dir().join("bg.mp4");
        p.exists().then_some(p)
    } else {
        let p = PathBuf::from(explicit);
        p.exists().then_some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_index_maps_song_time_to_video_time() {
        // Pure helper: floor((start + song_time) * fps) with loop wrap.
        let fps = 30.0_f32;
        let dur = 10.0_f32;
        let idx = |t: f32, start: f32, looping: bool| {
            let v = start + t;
            let local = if looping { v.rem_euclid(dur) } else { v.clamp(0.0, dur) };
            (local * fps).floor().max(0.0) as u64
        };
        assert_eq!(idx(0.0, 0.0, true), 0);
        assert_eq!(idx(1.0, 0.0, true), 30);
        // The video starts `start` seconds in when the song is at 0.
        assert_eq!(idx(0.0, 0.5, true), 15);
        // Loop wraps at the end.
        assert_eq!(idx(10.5, 0.0, true), 15);
        // Non-loop clamps at the end.
        assert_eq!(idx(99.0, 0.0, false), 300);
    }

    #[test]
    fn resolve_path_missing_is_none() {
        assert!(resolve_path("definitely/not/here.mp4").is_none());
    }

    /// End-to-end decode of the bundled `bg.mp4` (skips if ffmpeg is absent).
    #[test]
    fn decodes_frames_from_bundled_video() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let path = crate::app::platform::asset_dir().join("bg.mp4");
        let Some((dur, w, h, fps)) = probe(&path) else {
            return;
        };
        assert!(dur > 0.0, "probe should report a duration");
        assert!(fps > 0.0, "probe should report a frame rate");
        let out_h = 180usize;
        let out_w =
            ((out_h as f32 * w as f32 / h.max(1) as f32).round() as usize).max(2) & !1;
        let (mut child, rx) = spawn_ffmpeg(&path, 0.0, out_w, out_h, 10.0).expect("spawn ffmpeg");
        let mut got = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while got < 3 && std::time::Instant::now() < deadline {
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(frame) => {
                    assert_eq!(frame.len(), out_w * out_h * 4);
                    got += 1;
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(got >= 1, "expected at least one decoded frame");
    }
}
