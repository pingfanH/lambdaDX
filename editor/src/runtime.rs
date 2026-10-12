//! Editor runtime glue: loading charts/audio and driving the shared player.
//!
//! The editor is a thin shell around the same [`PadPreviewState`] the standalone
//! preview uses. Everything in here translates editor intent ("load this folder",
//! "switch to autoplay") into calls on that shared state.

use std::path::Path;

use crate::app;
use crate::app::types::{ChartDoc, WavPcm};
use crate::player::autoplay;
use crate::player::state::PadPreviewState;

use super::state::EditorState;

/// A fully-loaded chart: parsed document plus its decoded track.
pub struct ChartBundle {
    pub chart: ChartDoc,
    pub audio_name: Option<String>,
    pub audio_pcm: Option<WavPcm>,
    pub simai: Option<String>,
    pub level: Option<u32>,
}

/// Load a chart folder (or file) into a [`ChartBundle`]. `want_slot` selects a
/// Simai `&inote_N` difficulty when present, else the highest available.
pub fn load_chart_bundle(dir: &Path, want_slot: Option<u32>) -> ChartBundle {
    let simai = app::chart::read_simai_source(dir);
    let (chart, level) = match &simai {
        Some(text) => {
            let keys = app::maidata::inote_keys(text);
            let slot = want_slot
                .filter(|s| keys.contains(s))
                .or_else(|| keys.last().copied());
            let chart = slot
                .and_then(|s| app::maidata::from_maidata_level(text, s).ok())
                .unwrap_or_else(|| {
                    app::chart::load_chart_from_path(dir, None)
                        .unwrap_or_else(|e| {
                            eprintln!("editor: load failed: {e}");
                            app::chart::fallback_chart()
                        })
                });
            (chart, slot)
        }
        None => {
            let chart = app::chart::load_chart_from_path(dir, None).unwrap_or_else(|e| {
                eprintln!("editor: load failed: {e}");
                app::chart::fallback_chart()
            });
            (chart, None)
        }
    };
    let (audio_name, audio_pcm) = app::chart::find_audio_in_dir(dir)
        .map(|p| app::audio::load_audio_from_path(&p))
        .unwrap_or((None, None));
    ChartBundle {
        chart,
        audio_name,
        audio_pcm,
        simai,
        level,
    }
}

/// (Re)load a chart folder into a live [`PadPreviewState`], keeping the already
/// loaded textures/materials/SFX. `want_slot` selects a Simai difficulty.
pub fn load_chart_into(app: &mut PadPreviewState, dir: &Path, want_slot: Option<u32>) {
    let bundle = load_chart_bundle(dir, want_slot);
    app.load_chart(
        bundle.chart,
        bundle.audio_name,
        bundle.audio_pcm,
        bundle.simai,
        bundle.level,
    );
}

/// Feed the shared player one frame: audio, cue sounds, autoplay and judgment.
///
/// While the editor is scrubbing the playhead it suppresses cue sounds and feeds
/// autoplay straight to the judgment backend (no sensor-zone mirroring) — both
/// handled by the player modules reading [`PadPreviewState::scrubbing`].
pub async fn tick_player(app: &mut PadPreviewState) {
    app::audio::service_audio(app).await;
    app.tick_cues();
    autoplay::tick(app);
    crate::player::engine::step_judge_engine(app);
}

/// Map the editor's 3-way preview mode onto the shared player state.
///
/// * `0` autoplay **without** judgment
/// * `1` manual play **with** judgment
/// * `2` autoplay **with** judgment
pub fn apply_preview_mode(app: &mut PadPreviewState, mode: u8) {
    let (autoplay, no_core) = match mode % 3 {
        0 => (true, true),
        1 => (false, false),
        _ => (true, false),
    };
    autoplay::set_on(app, autoplay);
    let mut params = app::params::get();
    if params.no_core != no_core {
        params.no_core = no_core;
        app::params::set(params.clone());
        app.params = params;
    }
}

/// Format the 3-way preview mode for the toolbar.
pub fn preview_mode_label(mode: u8) -> &'static str {
    match mode % 3 {
        0 => "Autoplay · no judge",
        1 => "Manual · judge",
        _ => "Autoplay · judge",
    }
}

/// Follow the playhead while playing so the timeline does not scroll off-screen.
pub fn follow_playhead(editor: &mut EditorState, current_time: f32) {
    editor.view_start = (current_time - 2.0).max(0.0);
}
