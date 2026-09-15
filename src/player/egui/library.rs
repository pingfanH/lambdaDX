use std::path::{Path, PathBuf};

use egui_macroquad::egui::{self, ColorImage, TextureOptions};
use lambda_dx::app::platform;
use lambda_dx::simai_io::{self, DialogImport};

use crate::state::{LibrarySong, PlayerPage, PlayerState};

/// Runtime override for the chart library root.
const CHARTS_DIR_ENV: &str = "MAI2_SONGS_DIR";
/// Library directory created under the user's home on first launch.
const CHARTS_DIR_NAME: &str = ".maichart";
const CHART_FILE_NAME: &str = "maidata.txt";
/// How deep below a chart root song directories are searched. Libraries group
/// songs one level down (`<root>/Original/<song>/maidata.txt`), so the scan
/// descends a few levels instead of only one.
const MAX_CHART_SCAN_DEPTH: usize = 3;

pub fn ensure_song_library(app: &mut PlayerState) {
    if app.song_library_scanned {
        return;
    }
    refresh_song_library(app);
}

/// Prepare the chart library for this launch. The library directory is created
/// when it is missing, and the eligible chart list is always read from disk:
/// nothing is bundled with the build.
pub fn ensure_chart_library(app: &mut PlayerState) {
    let root = charts_directory();
    let existed = root.is_dir();
    if let Err(error) = create_charts_directory() {
        app.song_library.clear();
        app.player_ui.song_error = Some(error);
        return;
    }
    if !existed {
        app.set_status(format!("已创建曲库目录 {}", root.display()));
    }
    ensure_song_library(app);
}

pub fn refresh_song_library(app: &mut PlayerState) {
    app.song_library_scanned = true;
    app.ui_cover_textures.clear();
    app.ui_assets_loaded = false;
    app.player_ui.loaded_song = None;
    match create_charts_directory().and_then(|root| scan_song_directory(&root)) {
        Ok(songs) => {
            app.song_library = songs;
            if app.player_ui.selected_song >= app.song_library.len() {
                app.player_ui.selected_song = 0;
            }
            app.player_ui.song_error = None;
        }
        Err(error) => {
            app.song_library.clear();
            app.player_ui.song_error = Some(error);
        }
    }
}

/// Chart library root: `$MAI2_SONGS_DIR` when set, otherwise `<home>/.maichart`.
fn charts_directory() -> PathBuf {
    std::env::var_os(CHARTS_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(home_charts_directory)
        .unwrap_or_else(|| platform::data_root_dir().join(CHARTS_DIR_NAME))
}

fn home_charts_directory() -> Option<PathBuf> {
    home_directory().map(|home| home.join(CHARTS_DIR_NAME))
}

/// `HOME` is unset in some Windows shells, where `USERPROFILE` is the
/// equivalent. Mobile builds have neither and fall back to the data root.
fn home_directory() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .find_map(std::env::var_os)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Create the chart library directory when missing so a fresh install can be
/// filled in place at `~/.maichart`.
fn create_charts_directory() -> Result<PathBuf, String> {
    let root = charts_directory();
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("无法创建曲库目录 {}: {error}", root.display()))?;
    Ok(root)
}

/// Song folders below `root`, searched recursively. The returned list is the
/// eligible chart list: a directory qualifies when it directly contains a
/// `maidata.txt`, so grouped libraries are supported. An empty library yields
/// an empty list; only an unreadable directory is an error.
pub fn scan_song_directory(root: &Path) -> Result<Vec<LibrarySong>, String> {
    if !root.is_dir() {
        return Err(format!("曲库目录不存在: {}", root.display()));
    }
    let mut folders = Vec::new();
    collect_song_folders(root, 0, &mut folders);
    folders.sort();
    Ok(folders
        .into_iter()
        .map(|folder| song_from_folder(&folder))
        .collect())
}

/// Library root shown in the UI, so an empty library can point at itself.
pub fn charts_directory_display() -> String {
    charts_directory().display().to_string()
}

fn collect_song_folders(dir: &Path, depth: usize, folders: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        // Dot directories hold caches and tooling, never songs.
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
        })
        .collect();
    children.sort();
    for child in children {
        if child.join(CHART_FILE_NAME).is_file() {
            folders.push(child);
        } else if depth + 1 < MAX_CHART_SCAN_DEPTH {
            collect_song_folders(&child, depth + 1, folders);
        }
    }
}

fn song_from_folder(folder: &Path) -> LibrarySong {
    let chart_path = folder.join(CHART_FILE_NAME);
    let fallback_title = folder
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名歌曲")
        .to_owned();
    let raw_text = std::fs::read_to_string(&chart_path).ok();
    let metadata = raw_text
        .as_ref()
        .and_then(|text| simai_io::parse_simai_source(text).ok());
    let title = metadata
        .as_ref()
        .map(|file| file.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or(&fallback_title)
        .to_owned();
    let artist = metadata
        .as_ref()
        .map(|file| file.artist.trim())
        .filter(|artist| !artist.is_empty())
        .unwrap_or("未知艺术家")
        .to_owned();
    // `&des=` is the chart designer (谱师).
    let designer = raw_text
        .as_ref()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("&des="))
                .map(|value| value.trim().to_owned())
        })
        .filter(|designer| !designer.is_empty())
        .unwrap_or_else(|| "未知谱师".to_owned());
    let difficulty_count = metadata
        .as_ref()
        .map(|file| file.chart_count())
        .unwrap_or(0);
    let descriptor = if difficulty_count == 0 {
        "本地谱面".to_owned()
    } else {
        format!("{difficulty_count} 个难度 · 本地谱面")
    };
    let cover_path = ["bg.jpg", "bg.png", "bg.jpeg", "cover.jpg", "cover.png", "jacket.jpg", "jacket.png"]
        .iter()
        .map(|name| folder.join(name))
        .find(|path| path.is_file());

    LibrarySong {
        title,
        artist,
        designer,
        chart_path,
        cover_path,
        descriptor,
    }
}

pub fn ensure_cover_textures(ctx: &egui::Context, app: &mut PlayerState) {
    ensure_song_library(app);
    if app.ui_assets_loaded {
        return;
    }
    app.ui_assets_loaded = true;
    app.ui_logo_texture = decode_texture(
        ctx,
        "player_logo",
        include_bytes!("../../../assets/icon.jpg"),
    );
    app.ui_cover_textures = app
        .song_library
        .iter()
        .enumerate()
        .map(|(index, song)| {
            song.cover_path.as_ref().and_then(|path| {
                std::fs::read(path)
                    .ok()
                    .and_then(|bytes| decode_texture(ctx, &format!("song_cover_{index}"), &bytes))
            })
        })
        .collect();
}

fn decode_texture(ctx: &egui::Context, name: &str, encoded: &[u8]) -> Option<egui::TextureHandle> {
    let image = match macroquad::texture::Image::from_file_with_format(encoded, None) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("[player_ui] failed to decode {name}: {error}");
            return None;
        }
    };
    let color_image = ColorImage::from_rgba_unmultiplied(
        [usize::from(image.width), usize::from(image.height)],
        &image.bytes,
    );
    Some(ctx.load_texture(name, color_image, TextureOptions::LINEAR))
}

pub fn load_song(app: &mut PlayerState, song_index: usize) -> Result<(), String> {
    ensure_song_library(app);
    let chart_path = app
        .song_library
        .get(song_index)
        .ok_or_else(|| "曲库中没有可用谱面".to_owned())?
        .chart_path
        .to_string_lossy()
        .into_owned();
    let import = simai_io::import_from_file_path(&chart_path)?;
    apply_import(app, import, Some(song_index));
    Ok(())
}

pub fn apply_import(app: &mut PlayerState, import: DialogImport, song_index: Option<usize>) {
    let note_count = import.chart.notes.len();
    let selected_level = import.chart.simai_level.max(
        import
            .levels
            .iter()
            .map(|(level, _)| *level)
            .max()
            .unwrap_or(0),
    );

    app.import_levels = import.levels.clone();
    app.imported_simai = Some(import.simai_file);
    app.import_selected_level = selected_level;
    app.reload_judge_engine();
    app.set_chart(import.chart);
    app.set_selected_note(None);
    app.set_editing_slide_path(None);
    if let (Some(bytes), Some(ext)) = (&import.audio_bytes, &import.audio_ext) {
        if let Some(pcm) = lambda_dx::app::audio::load_audio_from_bytes(bytes, ext) {
            app.audio_source_name = Some(import.title.clone());
            app.audio_wav_pcm = Some(pcm);
            app.audio_cache.clear();
        }
    }
    app.player_ui.loaded_song = song_index;
    app.player_ui.using_custom_song = song_index.is_none();
    app.player_ui.song_error = None;
    app.set_status(format!("已载入 {} · {note_count} notes", import.title));
}

pub fn select_difficulty(app: &mut PlayerState, level: u32) -> Result<(), String> {
    let simai = app
        .imported_simai
        .as_ref()
        .ok_or_else(|| "当前歌曲没有可切换的难度".to_owned())?;
    let chart = simai_io::convert_simai_level(simai, level)?;
    app.import_selected_level = level;
    app.reload_judge_engine();
    app.set_chart(chart);
    app.set_status(format!("难度已切换至 Lv.{level}"));
    Ok(())
}

/// Copy an imported song (maidata + audio + cover) into the chart library,
/// refresh the list and return the new song's index.
pub fn import_song_to_library(
    app: &mut PlayerState,
    import: &simai_io::DialogImport,
) -> Result<usize, String> {
    let source_dir = import
        .source_dir
        .as_ref()
        .ok_or_else(|| "导入来源目录不可用".to_owned())?;
    let root = create_charts_directory()?;

    let mut folder = sanitize_folder_name(&import.title);
    let mut dest = root.join(&folder);
    let mut counter = 1;
    while dest.is_dir() {
        folder = format!("{} ({})", sanitize_folder_name(&import.title), counter);
        dest = root.join(&folder);
        counter += 1;
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("创建歌曲目录失败: {e}"))?;

    // Copy maidata.txt (fall back to re-exporting the parsed file).
    let maidata = source_dir.join(CHART_FILE_NAME);
    if maidata.is_file() {
        std::fs::copy(&maidata, dest.join(CHART_FILE_NAME))
            .map_err(|e| format!("复制 {CHART_FILE_NAME} 失败: {e}"))?;
    } else {
        let text = simai_io::export_simai_file(&import.simai_file);
        std::fs::write(dest.join(CHART_FILE_NAME), text)
            .map_err(|e| format!("写入 {CHART_FILE_NAME} 失败: {e}"))?;
    }

    // Write audio.
    if let (Some(bytes), Some(ext)) = (&import.audio_bytes, &import.audio_ext) {
        std::fs::write(dest.join(format!("track.{ext}")), bytes)
            .map_err(|e| format!("写入音频失败: {e}"))?;
    }

    // Copy the cover if present (also normalize it to bg.jpg for detection).
    let cover_candidates = [
        "bg.jpg",
        "bg.png",
        "bg.jpeg",
        "cover.jpg",
        "cover.png",
        "jacket.jpg",
        "jacket.png",
    ];
    for name in cover_candidates {
        let src = source_dir.join(name);
        if src.is_file() {
            let dest_name = if name.starts_with("bg.") {
                name
            } else {
                "bg.jpg"
            };
            let _ = std::fs::copy(&src, dest.join(dest_name));
            break;
        }
    }

    refresh_song_library(app);
    let index = app
        .song_library
        .iter()
        .position(|song| song.chart_path.starts_with(&dest))
        .unwrap_or_else(|| app.song_library.len().saturating_sub(1));
    app.player_ui.selected_song = index;
    app.set_status(format!("已导入「{}」到曲库", import.title));
    Ok(index)
}

fn sanitize_folder_name(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '・' | '（' | '）') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.trim().is_empty() {
        s = "Imported Song".to_owned();
    }
    s.trim().to_string()
}

pub fn begin_gameplay(app: &mut PlayerState) -> Result<(), String> {
    if app.player_ui.using_custom_song {
        app.toggle_replay();
        app.player_ui.page = PlayerPage::Gameplay;
        return Ok(());
    }

    let selected = app.player_ui.selected_song;
    if app.song_library.is_empty() {
        return Err("曲库中没有可游玩的谱面".to_owned());
    }
    if !app.player_ui.using_custom_song && app.player_ui.loaded_song != Some(selected) {
        load_song(app, selected)?;
    }
    app.toggle_replay();
    app.player_ui.page = PlayerPage::Gameplay;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{MAX_CHART_SCAN_DEPTH, scan_song_directory};

    /// Unique temp directory per test, removed by [`TempLibrary`] on drop.
    struct TempLibrary(PathBuf);

    impl TempLibrary {
        fn new(label: &str) -> Self {
            let unique_suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must be after the Unix epoch")
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("lambda_dx_player_library_{label}_{unique_suffix}"));
            std::fs::create_dir_all(&root).expect("test library must be created");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// Create `<root>/<relative>` holding a minimal parseable `maidata.txt`.
        fn add_song(&self, relative: &str) {
            self.add_song_titled(relative, "Test Track");
        }

        fn add_song_titled(&self, relative: &str, title: &str) {
            let folder = self.0.join(relative);
            std::fs::create_dir_all(&folder).expect("test song directory must be created");
            std::fs::write(
                folder.join("maidata.txt"),
                format!("&title={title}\n&artist=Test Artist\n&inote_5={{8}},1,,,,\n"),
            )
            .expect("test chart must be written");
        }

        fn add_dir(&self, relative: &str) {
            std::fs::create_dir_all(self.0.join(relative)).expect("test directory must be created");
        }
    }

    impl Drop for TempLibrary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scans_song_subdirectories_when_they_contain_maidata() {
        // Given
        let library = TempLibrary::new("flat");
        library.add_song("valid-song");
        library.add_dir("ignored-folder");

        // When
        let songs = scan_song_directory(library.path()).expect("song directory scan must succeed");

        // Then
        assert_eq!(songs.len(), 1);
        assert_eq!(
            songs[0].chart_path,
            library.path().join("valid-song").join("maidata.txt")
        );
    }

    #[test]
    fn scans_grouped_libraries_below_the_root() {
        // Given: real libraries group charts one level down.
        let library = TempLibrary::new("grouped");
        library.add_song("Original/inner-song");
        library.add_song("root-song");

        // When
        let songs = scan_song_directory(library.path()).expect("song directory scan must succeed");

        // Then: both the grouped and the top-level song are eligible.
        let titles: Vec<&str> = songs.iter().map(|song| song.title.as_str()).collect();
        assert_eq!(titles, ["Test Track", "Test Track"]);
        assert!(songs.iter().any(|song| song
            .chart_path
            .ends_with("Original/inner-song/maidata.txt")));
    }

    #[test]
    fn ignores_dot_directories_holding_tool_caches() {
        // Given: the reference implementation stores caches in `.MajdataPlay`.
        let library = TempLibrary::new("dot-dirs");
        library.add_song(".MajdataPlay/hidden-song");
        library.add_song("visible-song");

        // When
        let songs = scan_song_directory(library.path()).expect("song directory scan must succeed");

        // Then
        assert_eq!(songs.len(), 1);
        assert!(songs[0].chart_path.ends_with("visible-song/maidata.txt"));
    }

    #[test]
    fn scans_charts_at_the_depth_limit_but_not_beyond() {
        // Given: one chart exactly at the limit and one nested a level deeper.
        let library = TempLibrary::new("deep");
        let nested = |levels: usize| {
            (0..levels)
                .map(|level| format!("level{level}"))
                .collect::<Vec<_>>()
                .join("/")
        };
        library.add_song_titled(&nested(MAX_CHART_SCAN_DEPTH), "At Limit");
        library.add_song_titled(&nested(MAX_CHART_SCAN_DEPTH + 1), "Beyond Limit");

        // When
        let songs = scan_song_directory(library.path()).expect("song directory scan must succeed");

        // Then: only the chart within the depth limit is eligible.
        let titles: Vec<&str> = songs.iter().map(|song| song.title.as_str()).collect();
        assert_eq!(titles, ["At Limit"]);
    }

    #[test]
    fn empty_library_scans_to_an_empty_list() {
        // Given
        let library = TempLibrary::new("empty");

        // When
        let songs = scan_song_directory(library.path()).expect("empty library must not fail");

        // Then
        assert!(songs.is_empty());
    }

    #[test]
    fn missing_library_is_reported_as_an_error() {
        // Given
        let library = TempLibrary::new("missing");
        let missing = library.path().join("does-not-exist");

        // When
        let result = scan_song_directory(&missing);

        // Then
        assert!(result.is_err());
    }
}
