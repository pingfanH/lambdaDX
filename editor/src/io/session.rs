//! Remembers the chart the editor last worked on, in the per-user cache.

use crate::app::platform;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE: &str = "editor_session.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    /// Chart folder or file the editor last loaded.
    pub chart: Option<PathBuf>,
    /// Simai `inote` slot to reopen with it.
    pub level: Option<u32>,
}

pub fn load() -> Session {
    let Ok(text) = platform::read_cache_text(FILE) else {
        return Session::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// The chart to reopen when no path is given on the command line.
pub fn remembered_chart() -> Option<PathBuf> {
    load().chart.filter(|path| path.exists())
}

pub fn remember_chart(path: &Path, level: Option<u32>) {
    let session = Session {
        chart: Some(path.to_path_buf()),
        level,
    };
    let result = serde_json::to_string_pretty(&session)
        .map_err(|error| error.to_string())
        .and_then(|text| platform::write_cache_text(FILE, &text).map(|_| ()));
    if let Err(error) = result {
        eprintln!("[session] {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_cache(dir: &Path, body: impl FnOnce()) {
        // SAFETY: single-threaded test that restores the variable afterwards.
        unsafe { std::env::set_var("MAI2_CACHE_DIR", dir) };
        body();
        unsafe { std::env::remove_var("MAI2_CACHE_DIR") };
    }

    #[test]
    fn session_roundtrip_keeps_chart_and_level() {
        let dir = std::env::temp_dir().join("lambdadx_session_test");
        let _ = std::fs::remove_dir_all(&dir);
        with_cache(&dir, || {
            remember_chart(Path::new("/tmp/chart/maidata.txt"), Some(6));
            let saved = load();
            assert_eq!(saved.chart.as_deref(), Some(Path::new("/tmp/chart/maidata.txt")));
            assert_eq!(saved.level, Some(6));
            // A missing path is not remembered as something to reopen.
            assert!(remembered_chart().is_none());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
