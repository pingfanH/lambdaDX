//! Chart library: scan a songs directory for `maidata.txt`, read metadata,
//! decode cover art and convert a chosen difficulty into a [`ChartDoc`].
//!
//! Mirrors the original player's `egui/library.rs`, trimmed to what the
//! macroquad parser in this crate can provide (no Simai *import* / file dialogs).

use std::collections::VecDeque;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use macroquad::prelude::Texture2D;

use crate::app::maidata::from_maidata;
use crate::app::types::ChartDoc;
use crate::simai::parse_file;

const SONGS_DIR_ENV: &str = "MAI2_SONGS_DIR";
const MAX_DEPTH: usize = 3;
/// Covers larger than this (on the long edge) are downscaled before upload.
const COVER_MAX: u32 = 512;

/// A decoded cover on its way from the worker thread to the GPU.
struct CoverRgba {
    index: usize,
    w: u16,
    h: u16,
    rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LibrarySong {
    pub title: String,
    pub artist: String,
    pub designer: String,
    pub descriptor: String,
    pub folder: PathBuf,
    pub chart_path: PathBuf,
    pub cover_path: Option<PathBuf>,
    /// `&lv_N=` style entries `(slot, display level)`.
    pub levels: Vec<(u32, String)>,
}

pub struct Library {
    pub root: PathBuf,
    pub songs: Vec<LibrarySong>,
    pub covers: Vec<Option<Texture2D>>,
    /// Whether a cover decode has already been attempted for each song.
    attempted: Vec<bool>,
    /// Covers queued for background decoding.
    pending: VecDeque<usize>,
    /// In-flight background decode (results are uploaded on the main thread).
    cover_rx: Option<Receiver<CoverRgba>>,
    cover_inflight: bool,
}

impl Library {
    /// An empty library (no scan).
    pub fn empty() -> Self {
        Self {
            root: PathBuf::new(),
            songs: Vec::new(),
            covers: Vec::new(),
            attempted: Vec::new(),
            pending: VecDeque::new(),
            cover_rx: None,
            cover_inflight: false,
        }
    }

    /// Resolve the songs directory (`MAI2_SONGS_DIR` > `~/.maichart` > given
    /// fallback) and scan it. Never fails: an unreadable root yields no songs.
    pub fn load(fallback: PathBuf) -> Self {
        let root = resolve_root(fallback);
        let mut lib = Self::empty();
        lib.root = root;
        lib.refresh();
        lib
    }

    pub fn refresh(&mut self) {
        let t = std::time::Instant::now();
        self.songs = scan(&self.root).unwrap_or_default();
        let n = self.songs.len();
        self.covers = vec![None; n];
        // Songs without a cover are "done"; the rest queue for background decode.
        self.attempted = self
            .songs
            .iter()
            .map(|s| s.cover_path.is_none())
            .collect();
        self.pending = (0..n).filter(|i| self.songs[*i].cover_path.is_some()).collect();
        self.cover_rx = None;
        self.cover_inflight = false;
        if crate::player_ui::perf::enabled() {
            eprintln!(
                "[perf] library.scan ({} songs from {}): {:.2}ms",
                n,
                self.root.display(),
                t.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    /// Return `index`'s cover if it is already uploaded (never blocks).
    pub fn cover(&self, index: usize) -> Option<&Texture2D> {
        self.covers.get(index).and_then(|c| c.as_ref())
    }

    /// Kept for call-site symmetry; retains the cover if present, otherwise the
    /// background pump will fill it in a frame or two.
    pub fn ensure_cover(&mut self, index: usize) -> Option<&Texture2D> {
        self.cover(index)
    }

    /// Upload finished covers and start the next background decode. Called once
    /// per frame; decoding itself runs off the main thread, so this never stalls.
    pub fn pump_covers(&mut self, _budget_ms: f32) {
        if let Some(rx) = &self.cover_rx {
            match rx.try_recv() {
                Ok(r) => {
                    self.covers[r.index] =
                        Some(Texture2D::from_rgba8(r.w, r.h, &r.rgba));
                    self.attempted[r.index] = true;
                    self.cover_rx = None;
                    self.cover_inflight = false;
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.cover_rx = None;
                    self.cover_inflight = false;
                }
            }
        }
        if self.cover_inflight {
            return;
        }
        while let Some(index) = self.pending.pop_front() {
            let Some(path) = self.songs.get(index).and_then(|s| s.cover_path.clone()) else {
                continue;
            };
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                if let Some((w, h, rgba)) = decode_cover_rgba(&path) {
                    let _ = tx.send(CoverRgba { index, w, h, rgba });
                }
            });
            self.cover_rx = Some(rx);
            self.cover_inflight = true;
            break;
        }
    }

    pub fn covers_ready(&self) -> usize {
        self.attempted.iter().filter(|a| **a).count()
    }

    pub fn chart_text(&self, index: usize) -> Result<String, String> {
        let song = self.songs.get(index).ok_or("曲库中没有这首歌")?;
        std::fs::read_to_string(&song.chart_path)
            .map_err(|e| format!("读取 {}: {e}", song.chart_path.display()))
    }

    pub fn audio_path(&self, index: usize) -> Option<PathBuf> {
        let song = self.songs.get(index)?;
        crate::app::chart::find_audio_in_dir(&song.folder)
    }
}

/// Decode a cover and downscale it to `COVER_MAX`, returning raw RGBA. Runs on
/// a worker thread — the GPU upload happens back on the main thread.
///
/// Downscaling uses [`image::DynamicImage::thumbnail`] (fast, and its resampler
/// is monomorphised inside the `image` crate) rather than the generic
/// `imageops::resize`, which would be compiled unoptimized into this crate.
fn decode_cover_rgba(path: &Path) -> Option<(u16, u16, Vec<u8>)> {
    let bytes = std::fs::read(path).ok()?;
    let img = crate::player_ui::perf::time("    cover.decode", || {
        image::load_from_memory(&bytes).ok()
    })?;
    let img = if img.width().max(img.height()) > COVER_MAX {
        crate::player_ui::perf::time("    cover.resize", || {
            img.thumbnail(COVER_MAX, COVER_MAX)
        })
    } else {
        img
    };
    let rgba = crate::player_ui::perf::time("    cover.rgba", || img.to_rgba8());
    let (w, h) = rgba.dimensions();
    Some((w as u16, h as u16, rgba.into_raw()))
}

fn resolve_root(fallback: PathBuf) -> PathBuf {    if let Some(dir) = std::env::var_os(SONGS_DIR_ENV) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let candidate = PathBuf::from(home).join(".maichart");
        if candidate.is_dir() {
            return candidate;
        }
    }
    fallback
}

/// Recursively collect folders containing `maidata.txt` up to `MAX_DEPTH`.
pub fn scan(root: &Path) -> Result<Vec<LibrarySong>, String> {
    if !root.is_dir() {
        return Err(format!("{} 不是目录", root.display()));
    }
    let mut folders = Vec::new();
    collect(root, 0, &mut folders);
    folders.sort();
    Ok(folders.iter().filter_map(|f| song_from_folder(f)).collect())
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.join("maidata.txt").is_file() {
            out.push(path.clone());
        }
        collect(&path, depth + 1, out);
    }
}

fn song_from_folder(folder: &Path) -> Option<LibrarySong> {
    let chart_path = folder.join("maidata.txt");
    let text = std::fs::read_to_string(&chart_path).ok()?;
    let file = parse_file(&text).ok();
    let (title, artist, levels) = match &file {
        Some(f) => (
            if f.title.trim().is_empty() {
                folder_name(folder)
            } else {
                f.title.clone()
            },
            if f.artist.trim().is_empty() {
                "未知艺术家".to_string()
            } else {
                f.artist.clone()
            },
            {
                let chart_keys: HashSet<u32> = f.charts.iter().map(|(key, _)| *key).collect();
                f.levels
                    .iter()
                    .filter(|(key, _)| chart_keys.contains(key))
                    .cloned()
                    .collect()
            },
        ),
        None => (folder_name(folder), "未知艺术家".to_string(), Vec::new()),
    };
    let designer = read_designer(&text).unwrap_or_else(|| "未知谱师".to_string());
    let diff_count = file.map(|f| f.charts.len()).unwrap_or(0);
    let descriptor = if diff_count > 1 {
        format!("{diff_count} 个难度 · 本地谱面")
    } else {
        "本地谱面".to_string()
    };
    Some(LibrarySong {
        title,
        artist,
        designer,
        descriptor,
        folder: folder.to_path_buf(),
        chart_path,
        cover_path: find_cover(folder),
        levels,
    })
}

fn folder_name(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未命名歌曲")
        .to_string()
}

fn read_designer(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("&des=").or_else(|| line.strip_prefix("&des: ")) {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn find_cover(folder: &Path) -> Option<PathBuf> {
    for name in [
        "bg.jpg", "bg.png", "bg.jpeg", "cover.jpg", "cover.png", "jacket.jpg", "jacket.png",
    ] {
        let p = folder.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Build a [`ChartDoc`] for `level_key` (an `&inote_N=` slot). `None` picks the
/// hardest available difficulty.
pub fn chart_for_level(text: &str, level_key: Option<u32>) -> Result<ChartDoc, String> {
    let diff = match level_key {
        Some(key) => {
            let file = crate::player_ui::perf::time("chart.parse_file(index)", || {
                parse_file(text).map_err(|e| e.to_string())
            })?;
            let mut keys: Vec<u32> = file.charts.iter().map(|(k, _)| *k).collect();
            keys.sort_unstable();
            keys.dedup();
            keys.iter()
                .position(|k| *k == key)
                .map(|p| p as i32 + 1)
        }
        None => None,
    };
    crate::player_ui::perf::time("chart.from_maidata(reparse+convert)", || {
        from_maidata(text, diff)
    })
}

/// A sensible default difficulty key for a freshly-loaded song: the first
/// `&inote_N=` slot that exists, preferring the hardest.
pub fn default_level(levels: &[(u32, String)]) -> Option<u32> {
    levels.iter().map(|(k, _)| *k).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_and_converts_bundled_charts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/charts");
        let songs = scan(&root).expect("scan");
        assert!(!songs.is_empty(), "no bundled charts found");
        for song in &songs {
            let text = std::fs::read_to_string(&song.chart_path).expect("read maidata");
            let key = default_level(&song.levels);
            let chart = chart_for_level(&text, key).expect("convert");
            assert!(!chart.notes.is_empty(), "{} has no notes", song.title);
        }
    }

    #[test]
    fn level_selection_matches_slot() {
        let text = "&title=T\n&inote_3=(120){4}1,2,3,4\n&inote_5=(120){4}1,2,3,4,5,6,7,8\n";
        let easy = chart_for_level(text, Some(3)).unwrap();
        let hard = chart_for_level(text, Some(5)).unwrap();
        assert!(easy.notes.len() < hard.notes.len());
    }

    #[test]
    fn default_level_ignores_orphan_level_metadata() {
        let text = "&title=T\n&lv_2=2.0\n&lv_6=\n&inote_2=(120){4}1,2,3,4\n";
        let file = parse_file(text).expect("file");
        let chart_keys: HashSet<u32> = file.charts.iter().map(|(key, _)| *key).collect();
        let levels: Vec<_> = file
            .levels
            .iter()
            .filter(|(key, _)| chart_keys.contains(key))
            .cloned()
            .collect();
        assert_eq!(default_level(&levels), Some(2));
    }
}
