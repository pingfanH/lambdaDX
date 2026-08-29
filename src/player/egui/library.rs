use std::path::{Path, PathBuf};

use egui_macroquad::egui::{self, ColorImage, TextureOptions};
use lambda_dx::simai_io::{self, DialogImport};

use crate::state::{LibrarySong, PlayerPage, PlayerState};

const SONGS_DIR_ENV: &str = "MAI2_SONGS_DIR";
const DEFAULT_SONGS_DIR: &str = "songs";

pub fn ensure_song_library(app: &mut PlayerState) {
    if app.song_library_scanned {
        return;
    }
    refresh_song_library(app);
}

pub fn refresh_song_library(app: &mut PlayerState) {
    app.song_library_scanned = true;
    app.ui_cover_textures.clear();
    app.ui_assets_loaded = false;
    app.player_ui.loaded_song = None;
    match scan_song_directory(&songs_directory()) {
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

fn songs_directory() -> PathBuf {
    std::env::var_os(SONGS_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SONGS_DIR))
}

pub fn scan_song_directory(root: &Path) -> Result<Vec<LibrarySong>, String> {
    let entries = std::fs::read_dir(root)
        .map_err(|error| format!("无法读取曲库目录 {}: {error}", root.display()))?;
    let mut folders: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("maidata.txt").is_file())
        .collect();
    folders.sort();

    Ok(folders
        .into_iter()
        .map(|folder| song_from_folder(&folder))
        .collect())
}

fn song_from_folder(folder: &Path) -> LibrarySong {
    let chart_path = folder.join("maidata.txt");
    let fallback_title = folder
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名歌曲")
        .to_owned();
    let metadata = std::fs::read_to_string(&chart_path)
        .ok()
        .and_then(|text| maisimai::parse_file(&text).ok());
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
    let difficulty_count = metadata.as_ref().map(|file| file.charts.len()).unwrap_or(0);
    let descriptor = if difficulty_count == 0 {
        "本地谱面".to_owned()
    } else {
        format!("{difficulty_count} 个难度 · 本地谱面")
    };
    let cover_path = ["bg.jpg", "bg.png", "cover.jpg", "cover.png"]
        .iter()
        .map(|name| folder.join(name))
        .find(|path| path.is_file());

    LibrarySong {
        title,
        artist,
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::scan_song_directory;

    #[test]
    fn scans_song_subdirectories_when_they_contain_maidata() {
        // Given
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("lambda_dx_player_library_{unique_suffix}"));
        let valid_song = root.join("valid-song");
        let ignored_folder = root.join("ignored-folder");
        std::fs::create_dir_all(&valid_song).expect("test song directory must be created");
        std::fs::create_dir_all(&ignored_folder).expect("ignored directory must be created");
        std::fs::write(
            valid_song.join("maidata.txt"),
            "&title=Test Track\n&artist=Test Artist\n",
        )
        .expect("test chart must be written");

        // When
        let songs = scan_song_directory(&root).expect("song directory scan must succeed");

        // Then
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].chart_path, valid_song.join("maidata.txt"));

        std::fs::remove_dir_all(root).expect("test song directory must be removed");
    }
}
