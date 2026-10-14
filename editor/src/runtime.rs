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

/// The last compiled content, independent of the editor's saved/dirty flag.
pub struct ChartSync {
    notes: Vec<app::types::Note>,
    bpms: Vec<app::types::BpmChange>,
    current: bool,
}

impl ChartSync {
    pub fn new(chart: &mut ChartDoc) -> Self {
        app::maichart::recompute_each(&mut chart.notes);
        Self {
            notes: chart.notes.clone(),
            bpms: chart.bpms.clone(),
            current: true,
        }
    }

    pub fn update(
        &mut self,
        app: &mut PadPreviewState,
        gesture_active: bool,
    ) -> Result<bool, String> {
        app::maichart::recompute_each(&mut app.chart.notes);
        if self.notes == app.chart.notes && self.bpms == app.chart.bpms {
            return Ok(self.current);
        }
        if gesture_active {
            return Ok(false);
        }
        let result = (|| {
            #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
            {
                let level = app.chart.simai_level.max(1);
                let compiled = super::core_chart::compile(&app.chart, level)?;
                let mut engine = crate::player::engine::JudgeEngine::load(&compiled.text, level)?;
                engine.bind_editor_chart(compiled.note_ids, compiled.slide_positions)?;
                app.replace_edited_engine(engine, compiled.text, level)
            }
            #[cfg(not(any(feature = "backend-lean", feature = "backend-rust")))]
            {
                let mut cues = crate::player::cues::CueTrack::from_chart(&app.chart);
                cues.reset(app.timeline_time().max(0.0));
                app.cue_track = Some(cues);
                Ok(())
            }
        })();
        self.current = result.is_ok();
        self.notes = app.chart.notes.clone();
        self.bpms = app.chart.bpms.clone();
        result.map(|()| true)
    }
}

/// A fully-loaded chart: parsed document plus its decoded track.
pub struct ChartBundle {
    pub chart: ChartDoc,
    pub audio_name: Option<String>,
    pub audio_pcm: Option<WavPcm>,
    pub simai: Option<String>,
    pub level: Option<u32>,
}

/// Finish chart initialization before the editor accepts preview-mode input.
pub fn initialize_player(
    mut app: PadPreviewState,
    simai: Option<String>,
    level: Option<u32>,
) -> PadPreviewState {
    if let Some(text) = simai {
        let level = level
            .or_else(|| app::maidata::inote_key(&text, None))
            .unwrap_or(app.chart.simai_level);
        match app.load_engine(&text, level) {
            Ok(()) => app.set_status(format!("lnmai-core ready (levelIndex={level})")),
            Err(error) => {
                eprintln!("editor: engine load failed: {error}");
                app.set_status(format!("engine: {error}"));
            }
        }
    }
    app
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
                    app::chart::load_chart_from_path(dir, None).unwrap_or_else(|e| {
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
pub async fn tick_player(app: &mut PadPreviewState, chart_current: bool) {
    app::audio::service_audio(app).await;
    if chart_current {
        app.tick_cues();
        autoplay::tick(app);
        crate::player::engine::step_judge_engine(app);
    }
}

/// Map the editor's preview modes onto the shared player state.
///
/// * `0` autoplay **without** judgment
/// * `1` manual play **with** judgment
/// * `2` autoplay **with** judgment
/// * `3` autoplay events sent directly to core without simulated touches
pub fn apply_preview_mode(app: &mut PadPreviewState, mode: u8) {
    let (autoplay, no_core) = match mode % 4 {
        0 => (true, true),
        1 => (false, false),
        _ => (true, false),
    };
    autoplay::set_on(app, autoplay);
    autoplay::set_direct_core(app, mode % 4 == 3);
    let mut params = app::params::get();
    if params.no_core != no_core {
        params.no_core = no_core;
        app::params::set(params.clone());
        app.params = params;
    }
}

/// Format the preview mode for the toolbar.
pub fn preview_mode_label(mode: u8) -> &'static str {
    match mode % 4 {
        0 => "Autoplay · no judge",
        1 => "Manual · judge",
        2 => "Autoplay · judge",
        _ => "Autoplay · core direct",
    }
}

/// Follow the playhead while playing so the timeline does not scroll off-screen.
pub fn follow_playhead(editor: &mut EditorState, current_time: f32) {
    editor.view_start = (current_time - 2.0).max(0.0);
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;

    #[test]
    fn editing_reloads_native_core_without_changing_player_clock_or_ids() {
        let source = "&inote_6=(120){4}1,2,E\n";
        let chart = app::maidata::from_maidata_level(source, 6).unwrap();
        let mut pad = initialize_player(
            PadPreviewState::new_at(chart, None, None, 0.0, None),
            Some(source.into()),
            Some(6),
        );
        pad.forced_time = Some(0.25);
        pad.mode = app::types::Mode::Playing;
        let mut sync = ChartSync::new(&mut pad.chart);
        let id = pad.chart.notes[0].id;
        pad.chart.notes[0].time = 2.0;
        pad.chart.notes[0].lane = 7;
        assert!(sync.update(&mut pad, false).unwrap());
        let spec = pad.judge_engine.as_ref().unwrap().chart_spec().unwrap();
        assert!(
            spec.taps
                .iter()
                .any(|n| n.slot == crate::core::types::OuterSlot::S7 && n.timing == 2_000_000)
        );
        assert_eq!(pad.chart.notes[0].id, id);
        assert_eq!(pad.song_time(), 0.25);
        assert_eq!(pad.mode, app::types::Mode::Playing);
    }

    #[test]
    fn gestures_defer_core_reload_and_delete_undo_refresh_each() {
        let source = "&inote_6=(120){4}1/2,E\n";
        let chart = app::maidata::from_maidata_level(source, 6).unwrap();
        let mut pad = initialize_player(
            PadPreviewState::new_at(chart, None, None, 0.0, None),
            Some(source.into()),
            Some(6),
        );
        pad.forced_time = Some(0.0);
        let mut sync = ChartSync::new(&mut pad.chart);
        let original = pad.chart.notes.clone();
        pad.chart.notes[0].lane = 7;
        assert!(!sync.update(&mut pad, true).unwrap());
        assert_eq!(
            pad.judge_engine
                .as_ref()
                .unwrap()
                .chart_spec()
                .unwrap()
                .taps[0]
                .slot,
            crate::core::types::OuterSlot::S1
        );
        assert!(sync.update(&mut pad, false).unwrap());
        assert_eq!(
            pad.judge_engine
                .as_ref()
                .unwrap()
                .chart_spec()
                .unwrap()
                .taps[0]
                .slot,
            crate::core::types::OuterSlot::S7
        );
        pad.chart.notes.pop();
        assert!(sync.update(&mut pad, false).unwrap());
        assert!(!pad.chart.notes[0].is_each);
        assert_eq!(
            pad.judge_engine
                .as_ref()
                .unwrap()
                .chart_spec()
                .unwrap()
                .taps
                .len(),
            1
        );
        pad.chart.notes = original;
        assert!(sync.update(&mut pad, false).unwrap());
        assert!(pad.chart.notes.iter().all(|note| note.is_each));
        assert_eq!(
            pad.judge_engine
                .as_ref()
                .unwrap()
                .chart_spec()
                .unwrap()
                .taps
                .len(),
            2
        );
    }

    #[test]
    fn initial_chart_supports_autoplay_and_judged_preview_modes() {
        let source = "&title=Preview\n&lv_6=1\n&inote_6=(120){4}1,2,E\n";
        let chart = app::maidata::from_maidata_level(source, 6).unwrap();
        let pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
        let mut pad = initialize_player(pad, Some(source.into()), Some(6));
        pad.forced_time = Some(0.0);
        assert!(
            pad.has_engine(),
            "initial editor chart must load its judgment core"
        );
        assert!(!pad.autoplay_tactic.is_empty());
        for (mode, autoplay_on, core_on) in [
            (0, true, false),
            (1, false, true),
            (2, true, true),
            (3, true, true),
        ] {
            apply_preview_mode(&mut pad, mode);
            assert_eq!(pad.autoplay, autoplay_on);
            assert_eq!(pad.use_core(), core_on, "preview mode {mode}");
            assert_eq!(pad.autoplay_direct_core, mode == 3);
        }
        autoplay::set_on(&mut pad, false);
        assert!(!pad.autoplay);
        assert!(pad.use_core());
        autoplay::set_on(&mut pad, true);
        assert!(pad.autoplay);
        assert!(pad.use_core());
    }
}
