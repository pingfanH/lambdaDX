//! Chart library: scan a songs directory for `maidata.txt`, read metadata,
//! decode cover art and convert a chosen difficulty into a [`ChartDoc`].
//!
//! Mirrors the original player's `egui/library.rs`, trimmed to what the
//! macroquad parser in this crate can provide (no Simai *import* / file dialogs).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use macroquad::prelude::Texture2D;

use crate::app::maidata::from_maidata;
use crate::app::types::ChartDoc;

const SONGS_DIR_ENV: &str = "MAI2_SONGS_DIR";
const MAX_DEPTH: usize = 3;
/// Covers larger than this (on the long edge) are downscaled before upload.
const COVER_MAX: u32 = 512;
/// At most 48 MiB of RGBA cover pixels, independent of library size.
const COVER_CACHE_BYTES: usize = 48 * 1024 * 1024;

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
    additional_roots: Vec<PathBuf>,
    covers: Vec<Option<Texture2D>>,
    cover_lru: VecDeque<usize>,
    cover_bytes: Vec<usize>,
    cover_cache_bytes: usize,
    /// Whether a cover decode has already been attempted for each song.
    attempted: Vec<bool>,
    /// Covers queued for background decoding.
    pending: VecDeque<usize>,
    /// In-flight background decode (results are uploaded on the main thread).
    cover_rx: Option<(usize, Receiver<Option<CoverRgba>>)>,
}

impl Library {
    /// An empty library (no scan).
    pub fn empty() -> Self {
        Self {
            root: PathBuf::new(),
            songs: Vec::new(),
            additional_roots: Vec::new(),
            covers: Vec::new(),
            cover_lru: VecDeque::new(),
            cover_bytes: Vec::new(),
            cover_cache_bytes: 0,
            attempted: Vec::new(),
            pending: VecDeque::new(),
            cover_rx: None,
        }
    }

    /// Resolve the songs directory and scan it. An explicit `MAI2_SONGS_DIR`
    /// overrides discovery. Android combines external `files/chart` with the
    /// bundled fallback; desktop prefers `~/.maichart` over the fallback.
    /// Unreadable roots are skipped.
    pub fn load(fallback: PathBuf) -> Self {
        let (root, additional_roots) = resolve_roots(fallback);
        let mut lib = Self::empty();
        lib.root = root;
        lib.additional_roots = additional_roots;
        lib.refresh();
        lib
    }

    pub fn refresh(&mut self) {
        let t = std::time::Instant::now();
        self.songs = std::iter::once(&self.root)
            .chain(self.additional_roots.iter())
            .filter_map(|root| scan(root).ok())
            .flatten()
            .collect();
        let n = self.songs.len();
        self.covers = vec![None; n];
        self.cover_lru.clear();
        self.cover_bytes = vec![0; n];
        self.cover_cache_bytes = 0;
        // Only visible/requested covers are decoded, never the entire library.
        self.attempted = self
            .songs
            .iter()
            .map(|s| s.cover_path.is_none())
            .collect();
        self.pending.clear();
        self.cover_rx = None;
        if crate::player_ui::perf::enabled() {
            eprintln!(
                "[perf] library.scan ({} songs from {:?}): {:.2}ms",
                n,
                std::iter::once(&self.root)
                    .chain(self.additional_roots.iter())
                    .collect::<Vec<_>>(),
                t.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    /// Return `index`'s cover if it is already uploaded (never blocks).
    pub fn cover(&self, index: usize) -> Option<&Texture2D> {
        self.covers.get(index).and_then(|c| c.as_ref())
    }

    /// Queue a visible cover and keep recently used ones in a bounded GPU cache.
    pub fn ensure_cover(&mut self, index: usize) -> Option<&Texture2D> {
        if self.cover(index).is_some() {
            self.touch_cover(index);
        } else if self.attempted.get(index) == Some(&false)
            && !self.cover_rx.as_ref().is_some_and(|(i, _)| *i == index)
            && !self.pending.contains(&index)
        {
            self.pending.push_back(index);
        }
        self.cover(index)
    }

    fn touch_cover(&mut self, index: usize) {
        self.cover_lru.retain(|i| *i != index);
        self.cover_lru.push_back(index);
    }

    fn evict_cover(&mut self, index: usize) {
        self.cover_lru.retain(|i| *i != index);
        self.cover_cache_bytes = self.cover_cache_bytes.saturating_sub(self.cover_bytes[index]);
        self.cover_bytes[index] = 0;
        self.covers[index] = None;
        self.attempted[index] = false;
    }

    fn make_cover_room(&mut self, incoming_bytes: usize) {
        while self.cover_cache_bytes.saturating_add(incoming_bytes) > COVER_CACHE_BYTES {
            let Some(index) = self.cover_lru.front().copied() else {
                break;
            };
            self.evict_cover(index);
        }
    }

    /// Upload finished covers and start the next background decode. Called once
    /// per frame; decoding itself runs off the main thread, so this never stalls.
    pub fn pump_covers(&mut self) {
        if let Some((_, rx)) = &self.cover_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.cover_rx = None;
                    if let Some(r) = result {
                        if r.index >= self.covers.len() {
                            return;
                        }
                        self.evict_cover(r.index);
                        self.make_cover_room(r.rgba.len());
                        self.covers[r.index] = Some(Texture2D::from_rgba8(r.w, r.h, &r.rgba));
                        self.cover_bytes[r.index] = r.rgba.len();
                        self.cover_cache_bytes = self.cover_cache_bytes.saturating_add(r.rgba.len());
                        self.attempted[r.index] = true;
                        self.touch_cover(r.index);
                    }
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.cover_rx = None;
                }
            }
        }
        while let Some(index) = self.pending.pop_front() {
            let Some(path) = self.songs.get(index).and_then(|s| s.cover_path.clone()) else {
                continue;
            };
            let (tx, rx) = mpsc::channel();
            self.attempted[index] = true;
            std::thread::spawn(move || {
                let result = decode_cover_rgba(&path)
                    .map(|(w, h, rgba)| CoverRgba { index, w, h, rgba });
                let _ = tx.send(result);
            });
            self.cover_rx = Some((index, rx));
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

fn resolve_roots(fallback: PathBuf) -> (PathBuf, Vec<PathBuf>) {
    if let Some(dir) = std::env::var_os(SONGS_DIR_ENV) {
        if !dir.is_empty() {
            return (PathBuf::from(dir), Vec::new());
        }
    }
    if let Some(dir) = std::env::var_os("MAI2_EXTERNAL_DATA_DIR").filter(|dir| !dir.is_empty()) {
        // Keep the path even if storage is currently absent: Refresh should
        // discover charts added or mounted after startup.
        let root = PathBuf::from(dir).join("chart");
        let additional = if fallback == root {
            Vec::new()
        } else {
            vec![fallback]
        };
        return (root, additional);
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let candidate = PathBuf::from(home).join(".maichart");
        if candidate.is_dir() {
            return (candidate, Vec::new());
        }
    }
    (fallback, Vec::new())
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
    let meta = crate::app::maidata::metadata(&text);
    let title = if meta.title.trim().is_empty() {
        folder_name(folder)
    } else {
        meta.title.clone()
    };
    let artist = if meta.artist.trim().is_empty() {
        "未知艺术家".to_string()
    } else {
        meta.artist.clone()
    };
    let levels = meta.levels.clone();
    let designer = read_designer(&text).unwrap_or_else(|| "未知谱师".to_string());
    let descriptor = if meta.chart_count > 1 {
        format!("{} 个难度 · 本地谱面", meta.chart_count)
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
        cover_path: crate::app::chart::find_cover_in_dir(folder),
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

/// Build a [`ChartDoc`] for `level_key` (an `&inote_N=` slot). `None` picks the
/// hardest available difficulty.
pub fn chart_for_level(text: &str, level_key: Option<u32>) -> Result<ChartDoc, String> {
    let keys = crate::app::maidata::inote_keys(text);
    let diff = match level_key {
        Some(key) => keys
            .iter()
            .position(|k| *k == key)
            .map(|position| position as i32 + 1),
        None => None,
    };
    crate::player_ui::perf::time("chart.from_lean(reparse+convert)", || {
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
    fn android_external_charts_join_bundled_library_and_refresh() {
        const FIXTURE_ENV: &str = "LDX_ANDROID_LIBRARY_TEST_DIR";
        if let Some(root) = std::env::var_os(FIXTURE_ENV).map(PathBuf::from) {
            let external = root.join("Android/data/io.lambdadx.lambda_dx_player_ui/files/chart");
            let bundled = root.join("private/player-assets/charts");
            let mut library = Library::load(bundled.clone());
            assert!(
                library.songs.iter().any(|song| song.title == "Chronomia"),
                "Android files/chart/ChronomiaLime was omitted from the song list: {:?}",
                library.songs
            );
            assert!(library.songs.iter().any(|song| song.title == "Bundled"));
            assert_eq!(library.songs.len(), 2);
            assert_eq!(library.root, external);
            // An identical fallback must not list the external songs twice.
            assert_eq!(Library::load(external.clone()).songs.len(), 1);

            std::fs::remove_dir_all(&external).unwrap();
            library.refresh();
            assert_eq!(library.songs.len(), 1);
            assert_eq!(library.songs[0].title, "Bundled");
            let mut missing_at_startup = Library::load(bundled);
            assert_eq!(missing_at_startup.songs.len(), 1);
            assert_eq!(missing_at_startup.root, external);

            let added = external.join("Added");
            std::fs::create_dir_all(&added).unwrap();
            std::fs::write(added.join("maidata.txt"), "&title=Added\n&inote_5=(120){4}1,E")
                .unwrap();
            library.refresh();
            assert_eq!(library.songs.len(), 2);
            assert!(library.songs.iter().any(|song| song.title == "Added"));
            missing_at_startup.refresh();
            assert_eq!(missing_at_startup.songs.len(), 2);
            assert!(missing_at_startup.songs.iter().any(|song| song.title == "Added"));
            return;
        }

        // A child process keeps Android's startup environment isolated from
        // other tests, which may be loading the desktop library concurrently.
        let root = std::env::temp_dir().join(format!(
            "ldx-android-library-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let files = root.join("Android/data/io.lambdadx.lambda_dx_player_ui/files");
        for (folder, title) in [
            (files.join("chart/ChronomiaLime"), "Chronomia"),
            (root.join("private/player-assets/charts/Bundled"), "Bundled"),
        ] {
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join("maidata.txt"),
                format!("&title={title}\n&artist=Lime\n&inote_5=(227){{4}}1,2,3,4,E"),
            )
            .unwrap();
        }
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "player_ui::library::tests::android_external_charts_join_bundled_library_and_refresh",
                "--nocapture",
            ])
            .env(FIXTURE_ENV, &root)
            .env("MAI2_EXTERNAL_DATA_DIR", &files)
            .env("HOME", &root)
            .env_remove("USERPROFILE")
            .env_remove(SONGS_DIR_ENV)
            .output()
            .unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn scanning_does_not_queue_the_entire_cover_library() {
        let mut library = Library::empty();
        library.root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/charts");
        library.refresh();
        assert!(!library.songs.is_empty());
        assert!(library.pending.is_empty(), "covers must load only when requested");
    }

    #[test]
    fn cover_cache_evicts_oldest_entries_by_bytes() {
        const MIB: usize = 1024 * 1024;
        let mut library = Library::empty();
        library.covers = vec![None, None, None];
        library.cover_bytes = vec![16 * MIB; 3];
        library.cover_cache_bytes = COVER_CACHE_BYTES;
        library.cover_lru = VecDeque::from([0, 1, 2]);
        library.attempted = vec![true, true, true];

        library.touch_cover(0);
        library.make_cover_room(24 * MIB);

        assert_eq!(library.cover_cache_bytes, 16 * MIB);
        assert_eq!(library.cover_lru, VecDeque::from([0]));
        assert_eq!(library.cover_bytes, vec![16 * MIB, 0, 0]);
        assert_eq!(library.attempted, vec![true, false, false]);
    }

    #[test]
    fn cover_cache_keeps_entries_when_upload_fits_exactly() {
        let mut library = Library::empty();
        library.covers = vec![None];
        library.cover_bytes = vec![COVER_CACHE_BYTES / 2];
        library.cover_cache_bytes = COVER_CACHE_BYTES / 2;
        library.cover_lru.push_back(0);
        library.attempted = vec![true];

        library.make_cover_room(COVER_CACHE_BYTES / 2);

        assert_eq!(library.cover_cache_bytes, COVER_CACHE_BYTES / 2);
        assert_eq!(library.cover_lru, VecDeque::from([0]));
        assert!(library.attempted[0]);
    }

    #[test]
    fn visible_cover_requests_are_deduplicated() {
        let mut library = Library::empty();
        library.covers = vec![None, None];
        library.attempted = vec![false, false];

        library.ensure_cover(0);
        library.ensure_cover(0);
        library.ensure_cover(2);
        assert_eq!(library.pending, VecDeque::from([0]));

        library.pending.clear();
        let (_tx, rx) = mpsc::channel();
        library.cover_rx = Some((0, rx));
        library.ensure_cover(0);
        library.ensure_cover(1);
        assert_eq!(library.pending, VecDeque::from([1]));
    }

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
}
