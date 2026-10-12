//! Chart I/O helpers: saving the edited chart and choosing a chart folder.

use crate::app::types::ChartDoc;

use super::state::EditorState;

/// Save the current chart as pretty JSON under `output/editor_chart.json` and
/// record the outcome on [`EditorState::status`].
pub fn save_chart(editor: &mut EditorState, chart: &ChartDoc) {
    let result = crate::app::beat_format::chart_to_json(chart)
        .and_then(|json| crate::app::platform::write_output_text("editor_chart.json", &json));
    editor.status = Some(match result {
        Ok(path) => format!("Saved {}", path.display()),
        Err(e) => format!("Save failed: {e}"),
    });
}

/// Native folder picker (macOS via `osascript`; no-op elsewhere).
pub fn pick_chart_path() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"Select chart folder\")",
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout)
            .trim()
            .trim_end_matches('/')
            .to_string();
        (!s.is_empty()).then_some(s)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}
