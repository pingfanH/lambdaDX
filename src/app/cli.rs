//! Command-line options.
//!
//! ```text
//! lambda_dx_pad_preview [CHART] [AUDIO] [--diff N]
//!   -c, --chart <PATH>   chart folder (with chart.json) or JSON file
//!   -a, --audio <PATH>   audio file (mp3/wav); overrides asset/auto-detect
//!   -d, --diff <N>       difficulty number to load (e.g. 4)
//!   -h, --help           print this help
//! ```
//!
//! Positional arguments are accepted for convenience: the first is the chart
//! path, the second the audio path.

use std::path::PathBuf;

/// Parsed launch options.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LaunchArgs {
    /// Chart folder or JSON file to load instead of the bundled default.
    pub chart: Option<PathBuf>,
    /// Audio file to use instead of the asset / auto-detected track.
    pub audio: Option<PathBuf>,
    /// 1-based difficulty number to pick from the chart.
    pub diff: Option<i32>,
    /// Print the parsed chart (bpms + slides) and exit without opening a window.
    pub dump: bool,
    /// Write every slide curve to this SVG and exit.
    pub slides_svg: Option<PathBuf>,

    // ── Offscreen video export ───────────────────────────────────────
    /// Render the chart to this video file (via `ffmpeg`) and exit.
    pub export_video: Option<PathBuf>,
    /// Audio file to mux into the export (defaults to the chart's track).
    pub export_audio: Option<PathBuf>,
    /// Export frame rate (default 60).
    pub export_fps: Option<u32>,
    /// Export size `WxH` (default 1920x1080).
    pub export_size: Option<(u32, u32)>,
    /// Internal render size `WxH` (default = export size); a smaller value is
    /// rendered and upscaled — much faster.
    pub export_render_size: Option<(u32, u32)>,
    /// Encoder: `auto` | `libx264` | `h264_videotoolbox` | any ffmpeg encoder.
    pub export_encoder: Option<String>,
    /// Bitrate for hardware/`Named` encoders (e.g. `12M`).
    pub export_bitrate: Option<String>,
    /// x264 CRF (default 18).
    pub export_crf: Option<u32>,
    /// x264 preset (default `medium`).
    pub export_preset: Option<String>,
    /// Seconds of song to render (default: chart length + 2s).
    pub export_duration: Option<f32>,
    /// Song time (seconds) to start rendering at (default 0).
    pub export_start: Option<f32>,
    /// Split the export across this many processes (default 1) and concat.
    pub export_parallel: Option<u32>,
    /// Video-only export (used by the parallel children).
    pub export_no_audio: bool,
    /// Drive the export with lnmai-core judging (accurate slide trails, but much
    /// slower). Default off: local autoplay + self-sliding stars.
    pub export_core: bool,
}

/// Parse the process arguments (excluding argv[0]).
///
/// Returns `Ok(None)` when `--help` was requested.
pub fn parse_env() -> Result<Option<LaunchArgs>, String> {
    parse(std::env::args().skip(1))
}

pub fn parse<I>(args: I) -> Result<Option<LaunchArgs>, String>
where
    I: IntoIterator<Item = String>,
{
    let mut out = LaunchArgs::default();
    let args: Vec<String> = args.into_iter().collect();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].clone();
        let mut advance = 1;
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "-c" | "--chart" => {
                out.chart = Some(PathBuf::from(next_value(&args, i, &arg)?));
                advance = 2;
            }
            "-a" | "--audio" => {
                out.audio = Some(PathBuf::from(next_value(&args, i, &arg)?));
                advance = 2;
            }
            "-d" | "--diff" => {
                let v = next_value(&args, i, &arg)?;
                out.diff = Some(
                    v.parse::<i32>()
                        .map_err(|_| format!("invalid --diff value: {v}"))?,
                );
                advance = 2;
            }
            "--dump" => out.dump = true,
            "--export-video" => {
                out.export_video = Some(PathBuf::from(next_value(&args, i, &arg)?));
                advance = 2;
            }
            "--export-audio" => {
                out.export_audio = Some(PathBuf::from(next_value(&args, i, &arg)?));
                advance = 2;
            }
            "--export-fps" => {
                let v = next_value(&args, i, &arg)?;
                out.export_fps = Some(
                    v.parse::<u32>()
                        .map_err(|_| format!("invalid --export-fps: {v}"))?,
                );
                advance = 2;
            }
            "--export-size" => {
                let v = next_value(&args, i, &arg)?;
                out.export_size = Some(crate::player::export_video::parse_size(&v)?);
                advance = 2;
            }
            "--export-render-size" => {
                let v = next_value(&args, i, &arg)?;
                out.export_render_size = Some(crate::player::export_video::parse_size(&v)?);
                advance = 2;
            }
            "--export-encoder" => {
                out.export_encoder = Some(next_value(&args, i, &arg)?);
                advance = 2;
            }
            "--export-bitrate" => {
                out.export_bitrate = Some(next_value(&args, i, &arg)?);
                advance = 2;
            }
            "--export-crf" => {
                let v = next_value(&args, i, &arg)?;
                out.export_crf = Some(
                    v.parse::<u32>()
                        .map_err(|_| format!("invalid --export-crf: {v}"))?,
                );
                advance = 2;
            }
            "--export-preset" => {
                out.export_preset = Some(next_value(&args, i, &arg)?);
                advance = 2;
            }
            "--export-duration" => {
                let v = next_value(&args, i, &arg)?;
                out.export_duration = Some(
                    v.parse::<f32>()
                        .map_err(|_| format!("invalid --export-duration: {v}"))?,
                );
                advance = 2;
            }
            "--export-core" => out.export_core = true,
            "--export-start" => {
                let v = next_value(&args, i, &arg)?;
                out.export_start = Some(
                    v.parse::<f32>()
                        .map_err(|_| format!("invalid --export-start: {v}"))?,
                );
                advance = 2;
            }
            "--export-parallel" => {
                let v = next_value(&args, i, &arg)?;
                out.export_parallel = Some(
                    v.parse::<u32>()
                        .map_err(|_| format!("invalid --export-parallel: {v}"))?,
                );
                advance = 2;
            }
            "--export-no-audio" => out.export_no_audio = true,
            "--dump-slides-svg" => {
                // Optional path (default `output/slide_curves.svg`).
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    out.slides_svg = Some(PathBuf::from(args[i + 1].clone()));
                    advance = 2;
                } else {
                    out.slides_svg = Some(PathBuf::from("output/slide_curves.svg"));
                }
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option: {other}"));
            }
            other => {
                // Positional: chart first, then audio.
                if out.chart.is_none() {
                    out.chart = Some(PathBuf::from(other));
                } else if out.audio.is_none() {
                    out.audio = Some(PathBuf::from(other));
                } else {
                    return Err(format!("unexpected extra argument: {other}"));
                }
            }
        }
        i += advance;
    }

    Ok(Some(out))
}

fn next_value(args: &[String], i: usize, flag: &str) -> Result<String, String> {
    args.get(i + 1)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

pub fn help() -> &'static str {
    "LambdaDX Pad Preview

USAGE:
    lambda_dx_pad_preview [CHART] [AUDIO] [OPTIONS]

ARGS:
    CHART    Chart folder (containing maidata.txt and/or chart.json) or a
             chart file (maidata.txt / *.json). Defaults to the bundled
             assets/charts/jack_ripper.
    AUDIO    Audio file (mp3/wav). Defaults to the chart folder's track,
             then the bundled assets.

OPTIONS:
    -c, --chart <PATH>   Same as the first positional argument.
    -a, --audio <PATH>   Same as the second positional argument.
    -d, --diff <N>       Difficulty number to load (e.g. 4).
    --dump               Print the parsed chart (bpms + slides) and exit.
    --dump-slides-svg [PATH]
                         Write every possible slide curve to an SVG (default
                         output/slide_curves.svg) and exit.
    --export-video <PATH>
                         Render the chart offscreen to a video file via
                         `ffmpeg` (H.264) and exit. Autoplay is enabled.
    --export-audio <PATH>
                         Audio file to mux into the export (default: the
                         chart folder's track).
    --export-fps <N>     Export frame rate (default 60).
    --export-size <WxH>  Export size (default 1920x1080; rounded to even).
    --export-render-size <WxH>
                         Internal render size (default = export size). Smaller
                         renders + upscales, which is much faster.
    --export-encoder <E> auto (VideoToolbox on macOS) | libx264 | h264_videotoolbox
                         | any ffmpeg encoder name.
    --export-bitrate <B> Bitrate for hardware encoders (default 12M).
    --export-crf <N>     x264 CRF, lower = better (default 18).
    --export-preset <P>  x264 preset (default veryfast).
    --export-duration <SECONDS>  Length to render (default: chart + 2s).
    --export-start <SECONDS>     Start song time (default 0).
    --export-parallel <N>        Split the export across N processes and concat
                                 (uses all cores). Default 1.
    --export-core        Use lnmai-core judging during export (accurate slide
                         trails, much slower). Default: local autoplay.
    -h, --help           Print this help.

KEYS:
    Space  play/pause   R  restart   Home  0:00   ←/→  seek ±1s
    ↑/↓    speed        A  toggle audio        1-8/T  lane hits
    F1     params panel (edit note sizes / slide / touch; Save writes JSON)

EXAMPLES:
    lambda_dx_pad_preview ~/.maichart/324_Jack-the-Ripper◆
    lambda_dx_pad_preview ./chart.json ./track.mp3 --diff 4
"
}

#[cfg(test)]
mod tests {
    use super::{LaunchArgs, parse};
    use std::path::PathBuf;

    fn args(list: &[&str]) -> Result<Option<LaunchArgs>, String> {
        parse(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn positional_chart_and_audio() {
        let a = args(&["/songs/foo", "track.mp3"]).unwrap().unwrap();
        assert_eq!(a.chart, Some(PathBuf::from("/songs/foo")));
        assert_eq!(a.audio, Some(PathBuf::from("track.mp3")));
        assert_eq!(a.diff, None);
    }

    #[test]
    fn flags_parse() {
        let a = args(&["-c", "c.json", "-a", "a.wav", "-d", "4"])
            .unwrap()
            .unwrap();
        assert_eq!(a.chart, Some(PathBuf::from("c.json")));
        assert_eq!(a.audio, Some(PathBuf::from("a.wav")));
        assert_eq!(a.diff, Some(4));
    }

    #[test]
    fn help_returns_none() {
        assert_eq!(args(&["--help"]).unwrap(), None);
    }

    #[test]
    fn errors_on_unknown_and_missing_values() {
        assert!(args(&["--nope"]).is_err());
        assert!(args(&["--chart"]).is_err());
        assert!(args(&["a", "b", "c"]).is_err());
        assert!(args(&["--diff", "x"]).is_err());
    }
}
