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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LaunchArgs {
    /// Chart folder or JSON file to load instead of the bundled default.
    pub chart: Option<PathBuf>,
    /// Audio file to use instead of the asset / auto-detected track.
    pub audio: Option<PathBuf>,
    /// 1-based difficulty number to pick from the chart.
    pub diff: Option<i32>,
    /// Print the parsed chart (bpms + slides) and exit without opening a window.
    pub dump: bool,
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
    let mut it = args.into_iter();

    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "-c" | "--chart" => {
                out.chart = Some(PathBuf::from(next_value(&mut it, &arg)?));
            }
            "-a" | "--audio" => {
                out.audio = Some(PathBuf::from(next_value(&mut it, &arg)?));
            }
            "-d" | "--diff" => {
                let v = next_value(&mut it, &arg)?;
                out.diff = Some(
                    v.parse::<i32>()
                        .map_err(|_| format!("invalid --diff value: {v}"))?,
                );
            }
            "--dump" => out.dump = true,
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
    }

    Ok(Some(out))
}

fn next_value<I: Iterator<Item = String>>(it: &mut I, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
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
    -h, --help           Print this help.

KEYS:
    Space  play/pause   R  restart   Home  0:00   ←/→  seek ±1s
    ↑/↓    speed        A  toggle audio        1-8/T  lane hits

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
