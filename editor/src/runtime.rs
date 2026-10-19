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

mod sync;
pub use sync::ChartSync;

/// A fully-loaded chart: parsed document plus its decoded track.
pub struct ChartBundle {
    pub chart: ChartDoc,
    pub audio_name: Option<String>,
    pub audio_pcm: Option<WavPcm>,
    pub simai: Option<String>,
    pub level: Option<u32>,
    pub source: Option<crate::io::ChartSource>,
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
    } else {
        initialize_native(&mut app);
    }
    app
}

fn initialize_native(app: &mut PadPreviewState) {
    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    {
        let result = (|| {
            app.refresh_playback_chart();
            let chart = app.playback_chart();
            let level = chart.simai_level.max(1);
            let compiled = crate::core_chart::compile(chart, level)?;
            let mut engine = crate::player::engine::JudgeEngine::load(&compiled.text, level)?;
            engine.bind_editor_chart(compiled.note_ids, compiled.slide_positions)?;
            crate::player::state::PreparedEditedEngine::new(
                chart,
                engine,
                compiled.text,
                level,
                app.edited_engine_context(),
            )
        })();
        match result {
            Ok(prepared) => app.install_edited_engine(prepared),
            Err(error) => app.set_status(format!("engine: {error}")),
        }
    }
}

/// Load a chart folder (or file) into a [`ChartBundle`]. `want_slot` selects a
/// Simai `&inote_N` difficulty when present, else the highest available.
pub fn load_chart_bundle(dir: &Path, want_slot: Option<u32>) -> ChartBundle {
    try_load_chart_bundle(dir, want_slot).unwrap_or_else(|error| {
        eprintln!("editor: load failed: {error}");
        ChartBundle {
            chart: app::chart::fallback_chart(),
            audio_name: None,
            audio_pcm: None,
            simai: None,
            level: None,
            source: None,
        }
    })
}

pub fn try_load_chart_bundle(dir: &Path, want_slot: Option<u32>) -> Result<ChartBundle, String> {
    let mut source = crate::io::ChartSource::resolve(dir, None);
    let simai = if source.is_simai() {
        Some(
            std::fs::read_to_string(&source.path)
                .map_err(|e| format!("read {}: {e}", source.path.display()))?,
        )
    } else {
        None
    };
    let (chart, level) = match &simai {
        Some(text) => {
            let keys = app::maidata::inote_keys(text);
            let slot = want_slot
                .filter(|s| keys.contains(s))
                .or_else(|| keys.last().copied());
            let chart =
                app::maidata::from_maidata_level(text, slot.ok_or("No Simai difficulties")?)?;
            (chart, slot)
        }
        None => {
            let chart = app::chart::load_chart_from_path(&source.path, None)?;
            (chart, None)
        }
    };
    source.level = level;
    let audio_dir = source.path.parent().unwrap_or(dir);
    let (audio_name, audio_pcm) = app::chart::find_audio_in_dir(audio_dir)
        .map(|p| app::audio::load_audio_from_path(&p))
        .unwrap_or((None, None));
    Ok(ChartBundle {
        chart,
        audio_name,
        audio_pcm,
        simai,
        level,
        source: Some(source),
    })
}

/// (Re)load a chart folder into a live [`PadPreviewState`], keeping the already
/// loaded textures/materials/SFX. `want_slot` selects a Simai difficulty.
pub fn load_chart_into(
    app: &mut PadPreviewState,
    dir: &Path,
    want_slot: Option<u32>,
) -> Result<crate::io::ChartSource, String> {
    let bundle = try_load_chart_bundle(dir, want_slot)?;
    let source = bundle.source.ok_or("Missing loaded source")?;
    let native = bundle.simai.is_none();
    app.load_chart(
        bundle.chart,
        bundle.audio_name,
        bundle.audio_pcm,
        bundle.simai,
        bundle.level,
    );
    if native {
        initialize_native(app);
    }
    Ok(source)
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

/// Follow the chart playhead. Autoplay centers itself after laying out its plot.
pub fn follow_playhead(editor: &mut EditorState, current_time: f32) {
    if editor.follow_playhead && !editor.keyboard.detached && !editor.follow_zoom_detached {
        editor.view_start = crate::timeline::geometry::view_reference(editor, current_time);
    }
}

/// Resume a zoom-suspended chart follow once playback is active.
pub fn follow_playhead_during_playback(editor: &mut EditorState, current_time: f32, playing: bool) {
    if playing && editor.follow_playhead && !editor.keyboard.detached {
        editor.follow_zoom_detached = false;
    }
    follow_playhead(editor, current_time);
}

pub fn loop_range(editor: &EditorState) -> Option<(f32, f32)> {
    editor.loop_range.filter(|&(start, end)| {
        editor.loop_enabled
            && start.is_finite()
            && end.is_finite()
            && start >= 0.0
            && end - start >= 0.01
    })
}

fn playback_start(editor: &EditorState, time: f32) -> f32 {
    loop_range(editor).map_or(time, |(start, end)| {
        if time < start || time >= end {
            start
        } else {
            time
        }
    })
}

pub fn toggle_play(editor: &EditorState, app: &mut PadPreviewState) {
    if !matches!(
        app.mode,
        app::types::Mode::Playing | app::types::Mode::Recording
    ) {
        let time = app.timeline_time();
        let start = playback_start(editor, time);
        if start != time {
            app.seek_to(start);
        }
    }
    app.toggle_play();
}

pub fn restart_playback(editor: &EditorState, app: &mut PadPreviewState) {
    app.start_playback_at(loop_range(editor).map_or(0.0, |(start, _)| start));
}

pub fn rewind(editor: &EditorState, app: &mut PadPreviewState) {
    let start = if matches!(
        app.mode,
        app::types::Mode::Playing | app::types::Mode::Recording
    ) {
        loop_range(editor).map_or(0.0, |(start, _)| start)
    } else {
        0.0
    };
    app.seek_to(start);
}

pub fn tick_loop(editor: &EditorState, app: &mut PadPreviewState) {
    if !editor.loop_enabled
        || editor.loop_drag.is_some()
        || editor.scrubbing
        || app.scrubbing
        || !matches!(
            app.mode,
            app::types::Mode::Playing | app::types::Mode::Recording
        )
    {
        return;
    }
    if let Some((start, end)) = loop_range(editor)
        && (app.timeline_time() < start || app.timeline_time() >= end)
    {
        app.seek_to(start);
    }
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;

    #[test]
    fn loop_playback_starts_inside_the_range_and_preserves_inside_positions() {
        let mut editor = EditorState {
            loop_range: Some((2.0, 6.0)),
            loop_enabled: true,
            ..Default::default()
        };
        for (time, expected) in [(0.0, 2.0), (2.0, 2.0), (3.5, 3.5), (6.0, 2.0), (9.0, 2.0)] {
            assert_eq!(playback_start(&editor, time), expected);
        }
        editor.loop_enabled = false;
        assert_eq!(playback_start(&editor, 9.0), 9.0);
        editor.loop_enabled = true;
        editor.loop_range = Some((6.0, 2.0));
        assert_eq!(playback_start(&editor, 9.0), 9.0);
    }

    fn settle(sync: &mut ChartSync, pad: &mut PadPreviewState) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if sync.update(pad, false).unwrap() {
                return;
            }
            sync.schedule(pad, false).unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "core update did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

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
        assert!(!sync.update(&mut pad, false).unwrap());
        settle(&mut sync, &mut pad);
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
        assert!(
            !pad.has_engine(),
            "a gesture must not keep stale native state on the UI thread"
        );
        sync.schedule(&mut pad, true).unwrap();
        settle(&mut sync, &mut pad);
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
        settle(&mut sync, &mut pad);
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
        settle(&mut sync, &mut pad);
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

    #[test]
    fn authored_events_survive_background_rebuild_seek_and_both_playback_modes() {
        use crate::app::autoplay_edit::{self, AutoplayOverride};
        use crate::core::types::{SensorArea, TimedInputEvent};
        for direct in [false, true] {
            let chart = app::maidata::from_maidata_level("&inote_6=(120){4}1h[2],,,E", 6).unwrap();
            let mut pad = initialize_player(
                PadPreviewState::new_at(chart, None, None, 0.0, None),
                None,
                None,
            );
            pad.forced_time = Some(0.0);
            pad.autoplay = true;
            pad.autoplay_direct_core = direct;
            let mut sync = ChartSync::new(&mut pad.chart);
            let part = &pad.default_autoplay_plan.as_ref().unwrap().groups[0].parts[0];
            let key = autoplay_edit::key(
                pad.playback_chart(),
                pad.judge_engine.as_ref().unwrap(),
                part,
            )
            .unwrap();
            let events = vec![
                TimedInputEvent::SensorHold {
                    tp: 100_000,
                    area: SensorArea::A1,
                    is_down: true,
                },
                TimedInputEvent::SensorHold {
                    tp: 1_500_000,
                    area: SensorArea::A1,
                    is_down: false,
                },
            ];
            pad.chart.autoplay_overrides.push(AutoplayOverride {
                key,
                events: events.clone(),
                curve: None,
            });
            settle(&mut sync, &mut pad);
            assert!(pad.external_autoplay);
            assert_eq!(pad.autoplay_plan.as_ref().unwrap().events(), events);
            for target in [0.75, 0.0, 0.75] {
                pad.try_seek_to(target).unwrap();
                assert_eq!(pad.autoplay_plan.as_ref().unwrap().events(), events);
            }
            assert_eq!(pad.autoplay_tactic_cursor, 1);
            if direct {
                assert_eq!(pad.autoplay_direct_held, events[..1]);
            } else {
                assert!(pad.autoplay_explicit_held.contains(&SensorArea::A1));
            }
            pad.forced_time = Some(1.5);
            pad.audio_start_pending = false;
            autoplay::tick(&mut pad);
            assert_eq!(pad.engine_events, events[1..]);
        }
    }
}
