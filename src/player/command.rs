use std::path::PathBuf;

use lambda_dx::simai_io;

use crate::state::{PlayerPage, PlayerState};

const HELP: &str = r#"LambdaDX player

Usage:
  nix run -L .#player <path> [label value]...
  ./lambdaDX player <path> [label value]...
  lambda_dx_player <path> [label value]...

Path may be a maidata.txt file or a song directory containing maidata.txt.

Settings:
  level <n>             Select Simai difficulty level.
  note-speed <n>        Set note flight speed, e.g. 7.5.
  slide-fade-in <sec>   Set slide trail fade-in lead time in seconds.
  play-speed <n>        Set playback speed, e.g. 1.0.
  audio <on|off>        Enable or disable music and judge sound.
  pad-only <on|off>     Show pad-focused layout.
  mobile-ui <on|off>    Use mobile layout.
  ui-scale <n>          Override UI scale.
  start <sec>           Start playback from this song time.

Examples:
  nix run -L .#player songs/MySong/maidata.txt level 13 note-speed 8.0
  ./lambdaDX player songs/MySong play-speed 0.75 audio off
"#;

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchArgs {
    Library,
    Help,
    Command(CommandMode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandMode {
    pub chart_path: PathBuf,
    pub level: Option<u32>,
    pub note_speed: Option<f32>,
    pub slide_fade_in: Option<f32>,
    pub play_speed: Option<f32>,
    pub audio_enabled: Option<bool>,
    pub show_pad_only: Option<bool>,
    pub mobile_ui: Option<bool>,
    pub ui_scale: Option<f32>,
    pub start_time: Option<f32>,
}

impl CommandMode {
    fn new(chart_path: PathBuf) -> Self {
        Self {
            chart_path,
            level: None,
            note_speed: None,
            slide_fade_in: None,
            play_speed: None,
            audio_enabled: None,
            show_pad_only: None,
            mobile_ui: None,
            ui_scale: None,
            start_time: None,
        }
    }
}

pub fn help() -> &'static str {
    HELP
}

pub fn parse_env() -> Result<LaunchArgs, String> {
    parse(std::env::args().skip(1))
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<LaunchArgs, String> {
    let mut args: Vec<String> = args.into_iter().collect();
    if args.first().is_some_and(|arg| arg == "player") {
        args.remove(0);
    }
    if args.is_empty() {
        return Ok(LaunchArgs::Library);
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h" | "help"))
    {
        return Ok(LaunchArgs::Help);
    }

    let chart_path = chart_path_from_arg(&args[0]);
    let mut command = CommandMode::new(chart_path);
    let tail = &args[1..];
    if tail.len() % 2 != 0 {
        return Err("settings must be supplied as label/value pairs".to_owned());
    }
    for pair in tail.chunks_exact(2) {
        let label = normalize_label(&pair[0]);
        let value = pair[1].as_str();
        match label.as_str() {
            "level" | "lv" | "difficulty" => command.level = Some(parse_u32(value, &pair[0])?),
            "note-speed" | "note_speed" | "notespeed" => {
                command.note_speed = Some(parse_f32(value, &pair[0])?)
            }
            "slide-fade-in" | "slide_fade_in" | "fade-in" | "fade_in" => {
                command.slide_fade_in = Some(parse_f32(value, &pair[0])?)
            }
            "play-speed" | "play_speed" | "playback-speed" | "playback_speed" => {
                command.play_speed = Some(parse_f32(value, &pair[0])?)
            }
            "audio" | "sound" => command.audio_enabled = Some(parse_bool(value, &pair[0])?),
            "pad-only" | "pad_only" => command.show_pad_only = Some(parse_bool(value, &pair[0])?),
            "mobile-ui" | "mobile_ui" | "mobile" => {
                command.mobile_ui = Some(parse_bool(value, &pair[0])?)
            }
            "ui-scale" | "ui_scale" | "scale" => {
                command.ui_scale = Some(parse_f32(value, &pair[0])?)
            }
            "start" | "start-time" | "start_time" | "seek" | "offset" => {
                command.start_time = Some(parse_f32(value, &pair[0])?)
            }
            _ => return Err(format!("unknown setting label '{}'", pair[0])),
        }
    }
    Ok(LaunchArgs::Command(command))
}

pub fn apply_to_app(app: &mut PlayerState, command: &CommandMode) -> Result<(), String> {
    let chart_path = command.chart_path.to_string_lossy();
    let mut import = simai_io::import_from_file_path(&chart_path)?;
    let selected_level = match command.level {
        Some(level) => {
            import.chart = simai_io::convert_simai_level(&import.simai_file, level)?;
            level
        }
        None => import.chart.simai_level.max(
            import
                .levels
                .iter()
                .map(|(level, _)| *level)
                .max()
                .unwrap_or(0),
        ),
    };

    app.import_levels = import.levels.clone();
    app.imported_simai = Some(import.simai_file);
    app.import_selected_level = selected_level;
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

    if let Some(note_speed) = command.note_speed {
        app.note_speed = note_speed.clamp(1.0, 20.0);
    }
    if let Some(slide_fade_in) = command.slide_fade_in {
        app.slide_fade_in = slide_fade_in.max(0.0);
    }
    if let Some(play_speed) = command.play_speed {
        app.set_play_speed(play_speed);
    }
    if let Some(audio_enabled) = command.audio_enabled {
        app.audio_enabled = audio_enabled;
    }
    if let Some(show_pad_only) = command.show_pad_only {
        app.show_pad_only = show_pad_only;
    }
    if let Some(mobile_ui) = command.mobile_ui {
        app.mobile_ui = mobile_ui;
    }
    if let Some(ui_scale) = command.ui_scale {
        app.ui_scale_override = Some(ui_scale.clamp(0.7, 2.4));
    }

    app.player_ui.loaded_song = None;
    app.player_ui.using_custom_song = true;
    app.player_ui.song_error = None;
    app.reload_judge_engine();
    app.mode_song_offset = command.start_time.unwrap_or(0.0).max(0.0);
    app.timeline_view_time = app.mode_song_offset;
    app.audio_seek_offset = Some(app.mode_song_offset);
    app.mode = lambda_dx::app::types::Mode::Playing;
    app.mode_wall_anchor = macroquad::prelude::get_time();
    app.playback_cursor = 0;
    app.clear_active_screen_inputs();
    app.slide_progress.clear();
    app.request_audio_start();
    app.player_ui.page = PlayerPage::Gameplay;
    app.set_status(format!("Command mode: {} - Lv.{selected_level}", import.title));
    Ok(())
}

fn chart_path_from_arg(arg: &str) -> PathBuf {
    let path = PathBuf::from(arg);
    if path.is_dir() {
        path.join("maidata.txt")
    } else {
        path
    }
}

fn normalize_label(label: &str) -> String {
    label.trim_start_matches('-').to_ascii_lowercase()
}

fn parse_f32(value: &str, label: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("{label} expects a number, got '{value}'"))
}

fn parse_u32(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("{label} expects an integer, got '{value}'"))
}

fn parse_bool(value: &str, label: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "enable" | "enabled" => Ok(true),
        "0" | "false" | "no" | "off" | "disable" | "disabled" => Ok(false),
        _ => Err(format!("{label} expects on/off, got '{value}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandMode, LaunchArgs, parse};
    use std::path::PathBuf;

    #[test]
    fn parse_empty_args_opens_library() {
        assert_eq!(parse([]).unwrap(), LaunchArgs::Library);
    }

    #[test]
    fn parse_help_without_command_mode() {
        assert_eq!(parse(["--help".to_owned()]).unwrap(), LaunchArgs::Help);
    }

    #[test]
    fn parse_wrapper_style_command() {
        let parsed = parse([
            "player".to_owned(),
            "songs/foo/maidata.txt".to_owned(),
            "level".to_owned(),
            "13".to_owned(),
            "note-speed".to_owned(),
            "8.0".to_owned(),
            "audio".to_owned(),
            "off".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            parsed,
            LaunchArgs::Command(CommandMode {
                chart_path: PathBuf::from("songs/foo/maidata.txt"),
                level: Some(13),
                note_speed: Some(8.0),
                slide_fade_in: None,
                play_speed: None,
                audio_enabled: Some(false),
                show_pad_only: None,
                mobile_ui: None,
                ui_scale: None,
                start_time: None,
            })
        );
    }

    #[test]
    fn parse_rejects_unpaired_setting() {
        assert!(parse(["maidata.txt".to_owned(), "level".to_owned()]).is_err());
    }
}
