use std::collections::{HashMap, HashSet};

use macroquad::prelude::{Color, Texture2D, Vec2, get_time};

use crate::app::audio::{BgmPcm, BgmPlayer, SfxBuffer};
use crate::app::pad_svg::PadSvgDef;
use crate::app::params;
use crate::app::types::zone::PadZone;
use crate::app::types::{
    ChartDoc, HitFx, JudgeFeedback, Mode, NOTE_SPEED, PadFeedback, SPEED_MAX, SPEED_MIN, WavPcm,
};
use crate::player::cues::CueTrack;
use crate::player::video::VideoBg;

/// Per-sub-slide visual progress, stored as the consumed fraction (0..1) of each
/// of the sub-slide's segments (runtime arcs). The renderer maps these onto its
/// own trail bars using the path's segment boundaries.
#[derive(Debug, Clone, Default)]
pub struct SlideProgress {
    pub seg_frac: Vec<f32>,
    /// Wifi per-track consumed fractions (track index → 0..1), so the three
    /// tracks hide independently.
    pub track_frac: HashMap<u64, f32>,
    /// Wifi per-track explicit trail-bar cutoffs (`HideSlideTrackBars`), which
    /// take precedence over `track_frac`.
    pub track_hidden_until: HashMap<u64, usize>,
}

/// Seconds a slide `just` overlay stays on screen after its judgment.
pub const SLIDE_JUST_DURATION: f64 = 0.65;

/// Display grade for the slide `just` overlay, mapping lnmai-core's full
/// fast/late grade set onto the shipped `Skins/classic/slideok` sprite variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideJudgeGrade {
    Perfect,
    FastGreat,
    LateGreat,
    FastGood,
    LateGood,
    Miss,
    TooFast,
}

impl SlideJudgeGrade {
    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    pub fn from_grade(grade: crate::core::types::JudgeGrade) -> Self {
        use crate::core::types::JudgeGrade::*;
        match grade {
            Miss => SlideJudgeGrade::Miss,
            TooFast => SlideJudgeGrade::TooFast,
            FastGood => SlideJudgeGrade::FastGood,
            LateGood => SlideJudgeGrade::LateGood,
            FastGreat | FastGreat2nd | FastGreat3rd => SlideJudgeGrade::FastGreat,
            LateGreat | LateGreat2nd | LateGreat3rd => SlideJudgeGrade::LateGreat,
            Perfect | FastPerfect2nd | FastPerfect3rd | LatePerfect2nd | LatePerfect3rd => {
                SlideJudgeGrade::Perfect
            }
        }
    }

    /// Sprite family in `Skins/classic/slideok/` (`just_*`, `miss_*`, `toofast_*`).
    pub fn family(self) -> &'static str {
        match self {
            SlideJudgeGrade::Miss => "miss",
            SlideJudgeGrade::TooFast => "toofast",
            _ => "just",
        }
    }

    /// Grade suffix appended after the shape (`_p`, `_fast_gr`, …).
    pub fn suffix(self) -> &'static str {
        match self {
            SlideJudgeGrade::Perfect => "_p",
            SlideJudgeGrade::FastGreat => "_fast_gr",
            SlideJudgeGrade::LateGreat => "_late_gr",
            SlideJudgeGrade::FastGood => "_fast_gd",
            SlideJudgeGrade::LateGood => "_late_gd",
            SlideJudgeGrade::Miss | SlideJudgeGrade::TooFast => "",
        }
    }

    pub fn tint(self) -> Color {
        match self {
            SlideJudgeGrade::Perfect => Color::from_rgba(255, 244, 179, 255),
            SlideJudgeGrade::FastGreat | SlideJudgeGrade::LateGreat => {
                Color::from_rgba(120, 255, 160, 255)
            }
            SlideJudgeGrade::FastGood | SlideJudgeGrade::LateGood => {
                Color::from_rgba(120, 190, 255, 255)
            }
            SlideJudgeGrade::Miss | SlideJudgeGrade::TooFast => {
                Color::from_rgba(255, 120, 120, 255)
            }
        }
    }
}

/// A slide `just` overlay recorded when lnmai-core reports a slide judgment.
#[derive(Debug, Clone, Copy)]
pub struct SlideJudgeFx {
    pub grade: SlideJudgeGrade,
    pub started: f64,
}

#[derive(Clone, Copy, PartialEq)]
pub struct EditedEngineContext {
    pub time: f32,
    revision: u64,
    autoplay: bool,
    direct: bool,
    inner_screen: bool,
    continuous: bool,
}

/// Chart-derived data prepared away from the graphics/audio thread.
pub struct PreparedEditedEngine {
    pub engine: crate::player::engine::JudgeEngine,
    pub context: EditedEngineContext,
    source: String,
    level: u32,
    plan: crate::core::types::AutoplayPlan,
    inner_plan: crate::core::types::AutoplayPlan,
    default_plan: std::sync::Arc<crate::core::types::AutoplayPlan>,
    events: Vec<crate::core::types::TimedInputEvent>,
    normalized_events: Vec<crate::core::types::TimedInputEvent>,
    tactic: Vec<crate::core::types::TimedInputEvent>,
    tactic_cursor: usize,
    score: Option<crate::core::types::ScoreState>,
    windows: crate::player::judge_windows::Metadata,
    progress: Vec<crate::player::engine::SlideArcProgress>,
    note_ends: Vec<(u64, f32)>,
    hidden_notes: HashSet<u64>,
    cues: CueTrack,
    fragments: Vec<crate::app::maidata::SimaiFragment>,
    authored: bool,
}

/// Move chart-sized allocations with the native session so their destruction
/// cannot delay the first frame containing an edit.
pub struct RetiredEditedEngine {
    _engine: crate::player::engine::JudgeEngine,
    _windows: crate::player::judge_windows::Metadata,
    _tactic: Vec<crate::core::types::TimedInputEvent>,
    _plan: Option<crate::core::types::AutoplayPlan>,
    _default_plan: Option<std::sync::Arc<crate::core::types::AutoplayPlan>>,
    _fragments: Vec<crate::app::maidata::SimaiFragment>,
    _source: Option<String>,
    _progress: HashMap<(u64, usize), SlideProgress>,
    _hidden_notes: HashSet<u64>,
    _hold_head_hits: HashSet<u64>,
}

impl PreparedEditedEngine {
    pub fn new(
        chart: &ChartDoc,
        engine: crate::player::engine::JudgeEngine,
        source: String,
        level: u32,
        context: EditedEngineContext,
    ) -> Result<Self, String> {
        let default_plan = std::sync::Arc::new(engine.autoplay_plan()?);
        let plan = crate::app::autoplay_edit::generated_plan(chart, &engine, &default_plan, false);
        let inner_plan = crate::app::autoplay_edit::generated_plan(chart, &engine, &default_plan, true);
        let events = plan.events();
        let normalized_events = inner_plan.events();
        let windows = engine.judgment_metadata(chart);
        let note_ends = chart.notes.iter()
            .map(|note| (note.id, note_end_seconds(note, &chart.bpms))).collect();
        let cues = CueTrack::from_chart(chart);
        let fragments = crate::app::maidata::simai_timeline(&source, level).unwrap_or_default();
        let mut prepared = Self {
            engine, context, source, level, plan, inner_plan, default_plan, events, normalized_events, windows, note_ends, cues, fragments,
            tactic: Vec::new(), tactic_cursor: 0, score: None, progress: Vec::new(), hidden_notes: HashSet::new(),
            authored: !chart.autoplay_overrides.is_empty(),
        };
        prepared.restore(context)?;
        Ok(prepared)
    }

    pub fn restore(&mut self, context: EditedEngineContext) -> Result<(), String> {
        let (plan, events) = if context.inner_screen {
            (&self.inner_plan, &self.normalized_events)
        } else {
            (&self.plan, &self.events)
        };
        let reset = self.engine.seek_with_events(context.time, events, context.autoplay)?;
        self.progress = self.engine.slide_progress_updates(&reset.render_commands);
        self.tactic = plan.retained(&reset.retained_note_indices).events();
        self.tactic_cursor = self.tactic.partition_point(|event| {
            crate::player::engine::timed_input_tp(event) < (context.time * 1e6) as i64
        });
        self.hidden_notes = self.note_ends.iter()
            .filter(|(_, end)| *end < context.time).map(|(id, _)| *id).collect();
        self.score = Some(reset.score);
        self.cues.reset(context.time);
        self.context = context;
        Ok(())
    }
}

/// All mutable state the standalone pad preview needs.
///
/// This is the trimmed successor of the player's `PlayerState`: it keeps only
/// the fields `draw_pad_panel` + audio playback touch, dropping the editor,
/// judgment engine, song library and template state.
pub struct PadPreviewState {
    // ── Timing / playback ─────────────────────────────────────────────
    pub mode: Mode,
    pub mode_wall_anchor: f64,
    pub mode_song_offset: f32,
    /// Playback requested but the song clock is frozen until the first frame.
    pub playback_pending: bool,
    /// A (re)start was requested and the audio has not begun yet this session:
    /// while set, `song_time()` stays frozen at the offset so the timeline does
    /// not drift ahead during the audio load and then snap back (the "twitch"
    /// when starting playback from the top). Cleared by `service_audio` once the
    /// audio is actually started (or the silent fallback begins).
    pub audio_start_pending: bool,
    pub play_speed: f32,
    pub timeline_view_time: f32,
    /// Video export: when `Some`, `song_time()` returns this instead of the
    /// wall-clock-derived position, so frames render deterministically.
    pub forced_time: Option<f32>,

    // ── Note appearance ──────────────────────────────────────────────
    pub note_speed: f32,
    pub touch_speed: f32,
    pub slide_fade_in: f32,

    // ── Chart ────────────────────────────────────────────────────────
    pub chart: ChartDoc,
    expanded_chart: Option<ChartDoc>,
    expanded_source: Option<ChartDoc>,
    pub hidden_notes: HashSet<u64>,
    /// Hold / touch-hold notes whose **head** already had its hit sound played
    /// (detected locally, since the core reports holds only at their tail).
    pub hold_head_hits: HashSet<u64>,
    pub slide_progress: HashMap<(u64, usize), SlideProgress>,
    /// Per-sub-slide `just`/`miss` overlay shown after a slide judgment.
    pub slide_judge: HashMap<(u64, usize), SlideJudgeFx>,
    /// `Skins/classic/slideok/*` sprites, keyed by file stem
    /// (e.g. `just_curv_r_p`). Empty when the skin lacks the folder.
    pub slideok_tex: HashMap<String, Texture2D>,

    // ── Pad interaction ──────────────────────────────────────────────
    pub pad_svg: Option<PadSvgDef>,
    /// Zones currently held by each pointer. A single pointer can hold several
    /// zones at once when the sensor range trigger is on (and its range circle
    /// crosses a boundary), so this is a set rather than a single zone.
    pub active_pointer_zones: HashMap<u64, Vec<PadZone>>,
    pub prev_pointer_pos: HashMap<u64, Vec2>,
    pub pad_feedback: Vec<PadFeedback>,
    pub judge_feedback: Vec<JudgeFeedback>,
    /// One-shot tap-hit effects, pruned in `tick_feedback`.
    pub hit_fx: Vec<HitFx>,

    // ── Audio ────────────────────────────────────────────────────────
    pub audio_source_name: Option<String>,
    pub audio_wav_pcm: Option<WavPcm>,
    pub audio_cache: HashMap<i32, BgmPcm>,
    pub audio_seek_offset: Option<f32>,
    pub pending_audio_start: bool,
    pub audio_enabled: bool,
    pub bgm_player: Option<BgmPlayer>,
    /// One-shot cue sound (`Sfx/answer.wav`) played at tap / hold head / hold tail.
    pub answer_sfx: Option<SfxBuffer>,
    /// Judgment cue sounds per kind, if present. The mapping from a note/event
    /// to one of these lives in `crate::player::sfx` (a table like the skins).
    pub sfx_tap: Option<SfxBuffer>,
    pub sfx_touch: Option<SfxBuffer>,
    pub sfx_slide: Option<SfxBuffer>,
    pub sfx_hold: Option<SfxBuffer>,
    pub sfx_break: Option<SfxBuffer>,
    /// Ex / break variants (`tap_ex.wav`, `break_tap.wav`, `break_slide.wav`,
    /// `slide_break_start.wav`, `slide_break_slide.wav`).
    pub sfx_ex: Option<SfxBuffer>,
    pub sfx_break_tap: Option<SfxBuffer>,
    pub sfx_break_slide: Option<SfxBuffer>,
    pub sfx_slide_break_start: Option<SfxBuffer>,
    pub sfx_slide_break_slide: Option<SfxBuffer>,
    /// Time-based cue schedule (built from the chart).
    pub cue_track: Option<CueTrack>,

    // ── Note textures ────────────────────────────────────────────────
    pub tap_texture: Option<Texture2D>,
    pub hold_texture: Option<Texture2D>,
    pub touch_tri_tex: Option<Texture2D>,
    pub touch_point_tex: Option<Texture2D>,
    pub tap_each_tex: Option<Texture2D>,
    pub hold_each_tex: Option<Texture2D>,
    pub touch_tri_each_tex: Option<Texture2D>,
    pub touch_point_each_tex: Option<Texture2D>,
    pub touchhold_tex: [Option<Texture2D>; 4],
    pub touchhold_border_tex: Option<Texture2D>,
    pub slide_tex: Option<Texture2D>,
    pub slide_each_tex: Option<Texture2D>,
    pub wifi_tex: [Option<Texture2D>; 11],
    pub star_tex: Option<Texture2D>,
    pub star_each_tex: Option<Texture2D>,
    pub star_break_tex: Option<Texture2D>,
    pub star_double_tex: Option<Texture2D>,
    pub star_double_each_tex: Option<Texture2D>,
    pub tap_break_tex: Option<Texture2D>,
    pub hold_break_tex: Option<Texture2D>,
    pub slide_break_tex: Option<Texture2D>,
    pub star_double_break_tex: Option<Texture2D>,
    pub tap_ex_tex: Option<Texture2D>,
    pub hold_ex_tex: Option<Texture2D>,
    pub star_ex_tex: Option<Texture2D>,
    pub star_double_ex_tex: Option<Texture2D>,
    pub mask_material: Option<macroquad::material::Material>,
    /// Break-shine shader: pulses a break sprite's brightness past white
    /// (MajdataPlay's `_Brightness`), which a `u8` vertex tint cannot do.
    pub break_shine_material: Option<macroquad::material::Material>,
    /// Cover art drawn as the pad's circular background (set by the UI player).
    pub cover_texture: Option<Texture2D>,
    /// Optional guide texture drawn under each tap (aligned with flight).
    pub tap_guide_tex: Option<Texture2D>,
    pub tap_guide_each_tex: Option<Texture2D>,
    pub tap_guide_break_tex: Option<Texture2D>,
    /// Guide under slide stars.
    pub slide_guide_tex: Option<Texture2D>,
    /// Guides under hold tails (normal / each / break).
    pub hold_end_guide_tex: Option<Texture2D>,
    pub hold_end_each_guide_tex: Option<Texture2D>,
    pub hold_end_break_guide_tex: Option<Texture2D>,

    // ── Autoplay ─────────────────────────────────────────────────────
    /// When on, the pad presses itself at each note's hit time.
    pub autoplay: bool,
    /// Submit the configured tactic to core without synthetic sensor touches.
    pub autoplay_direct_core: bool,
    /// Raw held inputs, including button holds in simulated mode.
    pub autoplay_direct_held: Vec<crate::core::types::TimedInputEvent>,
    /// Sensor areas held open by a button-click tactic event, released on the
    /// next tactic frame so the core sees a hold for the click.
    pub autoplay_click_held: Vec<crate::core::types::SensorArea>,
    /// Sensor areas held by explicit tactic hold events. Kept separate from
    /// one-frame click holds so dense slide tactics do not lose their hold
    /// state when the next frame has no click event.
    pub autoplay_explicit_held: Vec<crate::core::types::SensorArea>,
    /// When set, the autoplay tactic is read from this JSON event list (see
    /// `--autoplay-tactic`) instead of the core's default.
    pub autoplay_tactic_path: Option<std::path::PathBuf>,
    /// True when `autoplay_tactic` is an external list: it is replayed verbatim
    /// (no click-hold synthesis / dedup preprocessing).
    pub external_autoplay: bool,
    pub legacy_autoplay: bool,
    /// None for external event streams, which keep their explicit input zones.
    autoplay_plan_inner_screen: Option<bool>,

    // ── Progress / seeking ───────────────────────────────────────────
    /// True while the progress bar is being dragged.
    pub scrubbing: bool,
    /// Committed clock before a preview drag; retained for failure rollback.
    scrub_origin: Option<f32>,
    /// Editor-only: render only this (note ID, sub-slide index) without time
    /// culling or judgment-based hiding.
    pub edit_solo_slide: Option<(u64, usize)>,
    /// Which pointer id is dragging the progress bar (touch or mouse).
    pub scrub_pointer: Option<u64>,

    // ── Background video ─────────────────────────────────────────────
    /// `bg.mp4` decoder (only used by the standalone pad preview).
    pub video_bg: VideoBg,

    // ── Misc ─────────────────────────────────────────────────────────
    pub mobile_ui: bool,
    pub ui_scale_override: Option<f32>,
    pub status: String,

    // ── Tunable params panel ─────────────────────────────────────────
    /// Visual parameters (also mirrored into the global `app::params`).
    pub params: crate::app::params::Params,
    /// Whether the egui params panel is open (toggle with F1).
    pub show_params: bool,

    // ── lnmai-core judgment engine ───────────────────────────────────
    /// Loaded lnmai-core judgment session (None until a chart is loaded).
    pub judge_engine: Option<crate::player::engine::JudgeEngine>,
    pub judge_windows: crate::player::judge_windows::Metadata,
    /// Pending input events (pad presses/releases) for the next engine step.
    pub engine_events: Vec<crate::core::types::TimedInputEvent>,
    /// Autoplay: lnmai-core's default replay tactic, consumed by timestamp.
    pub autoplay_tactic: Vec<crate::core::types::TimedInputEvent>,
    /// Complete ownership plan; seek compiles whole retained groups from this.
    pub autoplay_plan: Option<crate::core::types::AutoplayPlan>,
    pub default_autoplay_plan: Option<std::sync::Arc<crate::core::types::AutoplayPlan>>,
    pub autoplay_tactic_cursor: usize,
    /// Latest lnmai-core score snapshot (combo, DX score, judge counts).
    pub core_score: Option<crate::core::types::ScoreState>,
    /// Raw Simai fragments of the loaded chart, timed in seconds — used by the
    /// HUD to show which fragment is currently playing.
    pub simai_fragments: Vec<crate::app::maidata::SimaiFragment>,
    /// Simai source + `&inote_N` used to (re)build the engine on restart.
    simai_source: Option<String>,
    simai_level: u32,
    runtime_revision: u64,
    document_revision: u64,
}

impl PadPreviewState {
    pub fn playback_chart(&self) -> &ChartDoc {
        self.expanded_chart.as_ref().unwrap_or(&self.chart)
    }

    pub fn refresh_playback_chart(&mut self) {
        if self.chart.template_instances.is_empty() {
            self.expanded_chart = None;
            self.expanded_source = None;
        } else if !self.expanded_source.as_ref().is_some_and(|source|
            crate::app::templates::same_inputs(source, &self.chart)) {
            self.expanded_chart = Some(crate::app::templates::expand(&self.chart));
            self.expanded_source = Some(self.chart.clone());
        }
    }

    pub fn new(
        chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
    ) -> Self {
        Self::new_at(
            chart,
            audio_source_name,
            audio_wav_pcm,
            get_time(),
            BgmPlayer::new().ok(),
        )
    }

    pub(crate) fn new_at(
        mut chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
        wall_anchor: f64,
        bgm_player: Option<BgmPlayer>,
    ) -> Self {
        // Give every note a unique id. Simai-converted notes all default to
        // `id == 0`; autoplay hides judged notes by id, so without this a single
        // judgment would hide *every* note.
        crate::app::maichart::assign_note_ids(&mut chart.notes);

        let mobile_ui = std::env::var("MAI2_MOBILE_UI")
            .map(|v| v == "1")
            .unwrap_or(false);
        let ui_scale_override = std::env::var("MAI2_UI_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v.clamp(0.7, 2.4));

        let expanded_chart = (!chart.template_instances.is_empty()).then(|| crate::app::templates::expand(&chart));
        let expanded_source = expanded_chart.as_ref().map(|_| chart.clone());
        let cue_track = Some(CueTrack::from_chart(expanded_chart.as_ref().unwrap_or(&chart)));

        Self {
            mode: Mode::Idle,
            mode_wall_anchor: wall_anchor,
            mode_song_offset: 0.0,
            playback_pending: false,
            audio_start_pending: false,
            play_speed: 1.0,
            timeline_view_time: 0.0,
            forced_time: None,
            note_speed: NOTE_SPEED,
            touch_speed: NOTE_SPEED,
            slide_fade_in: 3.926_913 / crate::app::types::note_speed_from_setting(NOTE_SPEED),
            chart,
            expanded_chart,
            expanded_source,
            hidden_notes: HashSet::new(),
            hold_head_hits: HashSet::new(),
            slide_progress: HashMap::new(),
            slide_judge: HashMap::new(),
            slideok_tex: HashMap::new(),
            pad_svg: None,
            active_pointer_zones: HashMap::new(),
            prev_pointer_pos: HashMap::new(),
            pad_feedback: Vec::new(),
            judge_feedback: Vec::new(),
            hit_fx: Vec::new(),
            audio_source_name,
            audio_wav_pcm,
            audio_cache: HashMap::new(),
            audio_seek_offset: None,
            pending_audio_start: false,
            audio_enabled: true,
            bgm_player,
            answer_sfx: None,
            sfx_tap: None,
            sfx_touch: None,
            sfx_slide: None,
            sfx_hold: None,
            sfx_break: None,
            sfx_ex: None,
            sfx_break_tap: None,
            sfx_break_slide: None,
            sfx_slide_break_start: None,
            sfx_slide_break_slide: None,
            cue_track,
            tap_texture: None,
            hold_texture: None,
            touch_tri_tex: None,
            touch_point_tex: None,
            tap_each_tex: None,
            hold_each_tex: None,
            touch_tri_each_tex: None,
            touch_point_each_tex: None,
            touchhold_tex: [None, None, None, None],
            touchhold_border_tex: None,
            slide_tex: None,
            slide_each_tex: None,
            wifi_tex: std::array::from_fn(|_| None),
            star_tex: None,
            star_each_tex: None,
            star_break_tex: None,
            star_double_tex: None,
            star_double_each_tex: None,
            tap_break_tex: None,
            hold_break_tex: None,
            slide_break_tex: None,
            star_double_break_tex: None,
            tap_ex_tex: None,
            hold_ex_tex: None,
            star_ex_tex: None,
            star_double_ex_tex: None,
            mask_material: None,
            break_shine_material: None,
            cover_texture: None,
            tap_guide_tex: None,
            tap_guide_each_tex: None,
            tap_guide_break_tex: None,
            slide_guide_tex: None,
            hold_end_guide_tex: None,
            hold_end_each_guide_tex: None,
            hold_end_break_guide_tex: None,
            autoplay: false,
            autoplay_direct_core: false,
            autoplay_direct_held: Vec::new(),
            autoplay_click_held: Vec::new(),
            autoplay_explicit_held: Vec::new(),
            autoplay_tactic_path: None,
            external_autoplay: false,
            legacy_autoplay: false,
            autoplay_plan_inner_screen: None,
            scrubbing: false,
            scrub_origin: None,
            edit_solo_slide: None,
            scrub_pointer: None,
            video_bg: VideoBg::new(),
            mobile_ui,
            ui_scale_override,
            status: "Ready".to_string(),
            params: crate::app::params::Params::default(),
            show_params: false,
            judge_engine: None,
            judge_windows: crate::player::judge_windows::Metadata::default(),
            engine_events: Vec::new(),
            autoplay_tactic: Vec::new(),
            autoplay_plan: None,
            default_autoplay_plan: None,
            autoplay_tactic_cursor: 0,
            core_score: None,
            simai_fragments: Vec::new(),
            runtime_revision: 0,
            document_revision: 0,
            simai_source: None,
            simai_level: 0,
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status = msg;
    }

    /// Effective playback speed. Idle (paused) has no advancing clock.
    pub fn current_speed(&self) -> f32 {
        match self.mode {
            Mode::Playing | Mode::Recording => self.play_speed,
            Mode::Idle => 0.0,
        }
    }

    /// Current song position in seconds, derived from a wall-clock anchor.
    pub fn song_time(&self) -> f32 {
        if self.scrubbing {
            return self.timeline_view_time;
        }
        if let Some(t) = self.forced_time {
            return t;
        }
        if self.playback_pending || self.audio_start_pending {
            return self.mode_song_offset;
        }
        let elapsed_wall = (get_time() - self.mode_wall_anchor) as f32;
        self.mode_song_offset + elapsed_wall * self.current_speed()
    }

    pub fn stop_audio_if_any(&mut self) {
        if let Some(player) = &mut self.bgm_player {
            player.stop();
        }
    }

    pub fn request_audio_start(&mut self) {
        self.pending_audio_start = true;
    }

    pub fn toggle_play(&mut self) {
        if self.mode == Mode::Playing {
            self.mode_song_offset = self.song_time();
            self.mode = Mode::Idle;
            self.mode_wall_anchor = self.now();
            self.playback_pending = false;
            self.audio_start_pending = false;
            self.pending_audio_start = false;
            self.stop_audio_if_any();
            self.timeline_view_time = self.mode_song_offset;
            self.active_pointer_zones.clear();
            self.prev_pointer_pos.clear();
            self.set_status(format!("Paused at {:.2}s", self.mode_song_offset));
        } else {
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.mode = Mode::Playing;
            self.mode_wall_anchor = self.now();
            self.playback_pending = false;
            self.audio_start_pending = true;
            self.reset_cues(self.mode_song_offset);
            self.request_audio_start();
            self.set_status(format!(
                "Resumed @ {:.1}x from {:.2}s",
                self.play_speed, self.mode_song_offset
            ));
        }
    }

    /// Restart playback from `time` (seconds), freezing the clock until the
    /// first frame is presented.
    pub fn start_playback_at(&mut self, time: f32) {
        if let Err(error) = self.try_seek_to(time) {
            self.set_status(format!("seek: {error}"));
            return;
        }
        self.mode = Mode::Playing;
        self.mode_wall_anchor = self.now();
        self.playback_pending = true;
        self.audio_start_pending = true;
        self.audio_seek_offset = Some(self.mode_song_offset);
        self.reset_cues(self.mode_song_offset);
    }

    /// Release the frozen clock and start audio after the first frame is drawn.
    pub fn finalize_playback_start(&mut self) {
        if !self.playback_pending {
            return;
        }
        self.playback_pending = false;
        self.request_audio_start();
    }

    /// Queue an audio seek. While paused this only records the target so a later
    /// `toggle_play` resumes from it (matching the player's pause-scrub fix).
    pub fn seek_audio_to(&mut self, time: f32) {
        self.audio_seek_offset = Some(time);
        self.reset_cues(time);
        if self.mode == Mode::Playing {
            // Freeze until service_audio establishes the new wall anchor.
            // A slow audio load must not rewind an already advanced core.
            self.audio_start_pending = true;
            self.pending_audio_start = true;
        } else {
            self.stop_audio_if_any();
        }
    }

    /// Reposition the cue schedule without firing anything.
    fn reset_cues(&mut self, t: f32) {
        if let Some(track) = &mut self.cue_track {
            track.reset(t);
        }
    }

    /// Song length from PCM, with notes and standalone inputs as a silent-preview fallback.
    pub fn song_duration(&self) -> f32 {
        self.audio_wav_pcm
            .as_ref()
            .map(|pcm| {
                pcm.samples.len() as f32
                    / f32::from(pcm.channels.max(1))
                    / pcm.sample_rate.max(1) as f32
                    - self.chart.audio_offset
            })
            .unwrap_or_else(|| {
                self.playback_chart()
                    .notes
                    .iter()
                    .map(|n| {
                        use crate::app::types::{
                            NoteType, hold_tail_time, note_secs, slide_end_time,
                        };
                        (match n.note_type {
                            NoteType::Hold | NoteType::TouchHold => {
                                hold_tail_time(n, &self.chart.bpms)
                            }
                            NoteType::Slide => slide_end_time(n, &self.chart.bpms),
                            _ => note_secs(n, &self.chart.bpms),
                        }) + 1.0
                    })
                    .chain(self.playback_chart().autoplay_overrides.iter()
                        .filter(|item| item.key.is_standalone())
                        .flat_map(|item| &item.events)
                        .map(|event| crate::player::engine::timed_input_tp(event) as f32 / 1e6 + 1.0))
                    .fold(1.0, f32::max)
            })
            .max(1.0)
    }

    /// Update the visible position while dragging the progress bar (no audio
    /// restart).
    pub fn scrub_to(&mut self, t: f32) {
        if !t.is_finite() {
            return;
        }
        if self.scrub_origin.is_none() {
            let origin = if self.mode == Mode::Playing
                && !self.playback_pending
                && !self.audio_start_pending
            {
                self.mode_song_offset
                    + (get_time() - self.mode_wall_anchor) as f32 * self.current_speed()
            } else {
                self.mode_song_offset
            };
            self.scrub_origin = Some(origin);
            self.pending_audio_start = false;
            self.stop_audio_if_any();
        }
        let t = t.clamp(0.0, self.song_duration());
        if let Err(error) = self.seek_runtime_to(t) {
            self.set_status(format!("seek: {error}"));
            return;
        }
        self.timeline_view_time = t;
    }

    /// Commit a seek: reposition the clock and restart audio if playing.
    pub fn seek_to(&mut self, t: f32) {
        if let Err(error) = self.try_seek_to(t) {
            if let Some(origin) = self.scrub_origin.take() {
                self.mode_song_offset = origin;
                self.mode_wall_anchor = get_time();
                self.seek_audio_to(origin);
                let _ = self.seek_runtime_to(origin);
            }
            self.timeline_view_time = self.mode_song_offset;
            self.set_status(format!("seek: {error}"));
        }
    }

    /// Core commits first. A failed reset never moves the player/audio clock.
    pub fn try_seek_to(&mut self, t: f32) -> Result<(), String> {
        if !t.is_finite() {
            return Err("invalid seek time".into());
        }
        let t = t.clamp(0.0, self.song_duration());
        self.seek_runtime_to(t)?;
        self.seek_audio_to(t);
        self.mode_song_offset = t;
        self.timeline_view_time = t;
        self.mode_wall_anchor = self.now();
        self.scrub_origin = None;
        Ok(())
    }

    pub(crate) fn sync_autoplay_settings(&mut self) {
        if self.autoplay_plan_inner_screen
            .is_some_and(|inner| inner != self.params.autoplay_inner_screen)
            && self.has_engine()
        {
            if let Err(error) = self.seek_runtime_to(self.timeline_time()) {
                self.set_status(format!("autoplay: {error}"));
            }
        }
    }

    fn seek_runtime_to(&mut self, t: f32) -> Result<(), String> {
        let refresh_plan = self.autoplay_plan_inner_screen
            .is_some_and(|inner| inner != self.params.autoplay_inner_screen)
            || self.legacy_autoplay && t > 0.0;
        let updated_plan = if refresh_plan {
            self.default_autoplay_plan.as_ref().zip(self.judge_engine.as_ref())
                .map(|(defaults, engine)| crate::app::autoplay_edit::generated_plan(
                    self.playback_chart(), engine, defaults, self.params.autoplay_inner_screen,
                ))
        } else {
            None
        };
        let plan = updated_plan.as_ref().or(self.autoplay_plan.as_ref());
        let events = if let Some(plan) = plan {
            plan.events()
        } else {
            self.autoplay_tactic.clone()
        };
        let mut progress_updates = Vec::new();
        if let Some(engine) = self.judge_engine.as_mut() {
            let result = engine.seek_with_events(t, &events, self.autoplay)?;
            progress_updates = engine.slide_progress_updates(&result.render_commands);
            if let Some(plan) = plan {
                // Bare legacy streams remain usable until a nonzero seek.
                if !self.legacy_autoplay || t > 0.0 {
                    self.autoplay_tactic = plan.retained(&result.retained_note_indices).events();
                    if self.legacy_autoplay {
                        self.external_autoplay = false;
                        self.legacy_autoplay = false;
                    }
                }
            }
            self.core_score = Some(result.score);
        }
        if let Some(plan) = updated_plan {
            self.autoplay_plan = Some(plan);
            self.autoplay_plan_inner_screen = Some(self.params.autoplay_inner_screen);
        }
        self.clear_judgment_state();
        self.runtime_revision = self.runtime_revision.wrapping_add(1);
        let target_us = (t * 1e6) as i64;
        self.autoplay_tactic_cursor = self
            .autoplay_tactic
            .partition_point(|event| crate::player::engine::timed_input_tp(event) < target_us);
        self.apply_core_slide_progress_updates(&progress_updates);
        if self.use_core() {
            let past = self.playback_chart()
                    .notes
                    .iter()
                    .filter(|note| note_end_seconds(note, &self.chart.bpms) < t)
                    .map(|note| note.id).collect::<Vec<_>>();
            self.hidden_notes.extend(past);
        }
        crate::player::autoplay::restore_at(self, t, &events);
        Ok(())
    }

    fn clear_judgment_state(&mut self) {
        self.engine_events.clear();
        self.autoplay_click_held.clear();
        self.autoplay_explicit_held.clear();
        self.autoplay_direct_held.clear();
        self.active_pointer_zones.clear();
        self.prev_pointer_pos.clear();
        self.pad_feedback.clear();
        self.judge_feedback.clear();
        self.hit_fx.clear();
        self.slide_progress.clear();
        self.slide_judge.clear();
        self.hidden_notes.clear();
        self.hold_head_hits.clear();
    }

    /// Fire the one-shot cue sound (`Sfx/answer.wav`).
    pub fn play_answer(&self) {
        self.play_sfx(self.answer_sfx.as_ref());
    }

    /// Play a one-shot sound effect, if the player and buffer are present.
    pub fn play_sfx(&self, buf: Option<&SfxBuffer>) {
        if let (Some(player), Some(buf)) = (&self.bgm_player, buf) {
            player.play_once(buf, 1.0);
        }
    }

    /// The timeline cue sound for a note kind/variant, via the central
    /// `player::sfx` table.
    fn cue_sfx(
        &self,
        cue: crate::player::cues::Cue,
        is_break: bool,
        is_ex: bool,
        is_touch: bool,
    ) -> Option<&SfxBuffer> {
        use crate::player::cues::Cue;
        use crate::player::render::skin::SkinVariant;
        use crate::player::sfx::{self, SfxKind};
        let kind = match cue {
            Cue::Tap => SfxKind::Tap,
            Cue::Touch => SfxKind::Touch,
            Cue::SlideHead => SfxKind::SlideCue,
            // Touch-holds are touch notes; regular holds have no dedicated
            // sound so they use the tap sound.
            Cue::HoldHead | Cue::HoldTail => {
                if is_touch {
                    SfxKind::Touch
                } else {
                    SfxKind::Tap
                }
            }
        };
        let variant = SkinVariant::of_flags(is_break, false);
        sfx::select(self, kind, variant, is_ex)
    }

    /// Play a cue for every tap/hold head/hold tail crossed this frame.
    pub fn tick_cues(&mut self) {
        if self.mode != Mode::Playing
            || self.playback_pending
            || self.audio_start_pending
            || self.scrubbing
        {
            return;
        }
        let t = self.song_time();
        let due = self
            .cue_track
            .as_mut()
            .map(|track| track.take_due_cues(t))
            .unwrap_or_default();
        let core = self.use_core();
        for ev in due {
            // 正解音 (answer.wav): the generic cue on the note timeline for
            // **every** cue — tap / touch / hold head+tail / touch-hold
            // head+tail / slide head. This is independent of `hit_sfx`.
            if params::answer_sfx() {
                self.play_sfx(self.answer_sfx.as_ref());
            }
            if !params::hit_sfx() {
                continue;
            }
            // 击打音 (tap/touch.wav): only from an actual hit. With a core,
            // hits come from the judge events (`play_audio_command`) plus the
            // locally-detected hold/touch-hold **head** press (`play_hold_head_hits`).
            // Without a core there is no judging, so tap/slide/hold preview
            // their sounds on the timeline. Touch is press-triggered and never
            // timeline-plays.
            let timeline_cue = !core && !ev.is_touch;
            if timeline_cue {
                let buf = self.cue_sfx(ev.cue, ev.is_break, ev.is_ex, ev.is_touch);
                self.play_sfx(buf);
            }
        }
    }

    pub fn set_play_speed(&mut self, new_speed: f32) {
        if !new_speed.is_finite() {
            return;
        }
        let position = self.song_time();
        self.play_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
        if self.scrubbing {
            return;
        }
        if self.mode == Mode::Playing {
            self.mode_song_offset = position;
            self.mode_wall_anchor = self.now();
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.audio_start_pending = true;
            self.request_audio_start();
        }
    }

    pub fn nudge_play_speed(&mut self, delta: f32) {
        self.set_play_speed(self.play_speed + delta);
    }

    pub fn tick_feedback(&mut self) {
        // Pad touch feedback is wall-clock (so a held/tapped zone's pulse still
        // expires while paused); hit fx / judgment text / slide overlay follow
        // the note timeline (`fx_timeline`).
        let wall = self.now();
        let clock = self.fx_clock();
        self.pad_feedback.retain(|f| f.until > wall);
        self.judge_feedback.retain(|f| f.until > clock);
        self.hit_fx
            .retain(|f| clock - f.started < f.duration as f64);
        // Slide results can be produced while paused because the core still
        // steps frames at the frozen current time. Their overlay lifetime must
        // use wall time so the result appears and fades immediately.
        let wall_time = self.now();
        self.slide_judge
            .retain(|_, fx| wall_time - fx.started < SLIDE_JUST_DURATION);
    }

    /// Clock used for transient feedback lifetimes: the deterministic export
    /// clock when exporting, else macroquad's wall clock.
    pub fn now(&self) -> f64 {
        match self.forced_time {
            Some(t) => t as f64,
            None => get_time(),
        }
    }

    /// The time the note renderer uses for the current frame: song time while
    /// playing, the scrubbed view time while idle.
    pub fn timeline_time(&self) -> f32 {
        match self.mode {
            Mode::Playing | Mode::Recording => self.song_time(),
            Mode::Idle => self.timeline_view_time,
        }
    }

    /// Clock for transient effects (hit fx, judgment text, slide `just`
    /// overlay). With `fx_timeline` on (default) it is the note-timeline clock,
    /// so pausing freezes the animations; off, it falls back to the wall clock.
    pub fn fx_clock(&self) -> f64 {
        if crate::app::params::fx_timeline() {
            self.timeline_time() as f64
        } else {
            self.now()
        }
    }

    pub fn push_feedback(&mut self, zone: PadZone, duration: f64) {
        self.pad_feedback.push(PadFeedback {
            zone,
            // Wall clock: the pad-zone pulse must expire even while paused.
            until: self.now() + duration,
        });
    }

    pub fn push_judgement(&mut self, zone: PadZone, label: &str, duration: f64) {
        let now = self.fx_clock();
        self.judge_feedback.push(JudgeFeedback {
            zone,
            label: label.to_string(),
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            started: now,
            until: now + duration,
        });
    }

    /// Spawn a one-shot tap-hit effect at `zone`, tinted by the judge `label`
    /// (break notes are orange; misses grey-red).
    pub fn push_hit_fx(&mut self, zone: PadZone, label: &str, is_break: bool) {
        let now = self.fx_clock();
        self.hit_fx.push(HitFx {
            zone,
            started: now,
            duration: params::hit_fx_duration().max(0.05),
            color: hit_fx_color(label, is_break),
            is_break,
            seed: (now.fract() as f32) * std::f32::consts::TAU,
        });
    }

    /// Record a slide `just` overlay when lnmai-core reports a slide judgment.
    pub fn record_slide_judge(&mut self, note_id: u64, slide_idx: usize, grade: SlideJudgeGrade) {
        self.slide_judge.insert(
            (note_id, slide_idx),
            SlideJudgeFx {
                grade,
                started: self.now(),
            },
        );
    }

    // ── lnmai-core judgment engine ───────────────────────────────────

    /// Load `lnmai-core`'s judgment engine for a chart given as Simai text.
    ///
    /// `level_index` is the `&inote_N` block to select. Also builds the default
    /// replay tactic used by autoplay.
    pub fn load_engine(&mut self, simai_text: &str, level_index: u32) -> Result<(), String> {
        let engine = crate::player::engine::JudgeEngine::load(simai_text, level_index)?;
        if crate::player::engine::debug_slide_enabled() {
            let bpms = self.chart.bpms.clone();
            for note in &self.chart.notes {
                if !matches!(note.note_type, crate::app::types::NoteType::Slide) {
                    continue;
                }
                let head = crate::app::types::note_secs(note, &bpms);
                for (si, slide) in note.slide.iter().enumerate() {
                    eprintln!(
                        "[slide/chart] note={} lane={} head={head:.3}s slide_idx={si} \
                         segments={} runtime_parts={} is_break={} is_ex={} is_star={}",
                        note.id,
                        note.lane,
                        slide.segments.len(),
                        slide.runtime_parts,
                        note.is_break,
                        note.is_ex,
                        note.is_star,
                    );
                }
            }
            engine.debug_dump_slide_bindings();
        }
        // `--autoplay-tactic <file>` / `MAI2_AUTOPLAY_TACTIC=<file>` overrides
        // the core's default tactic with an external JSON event list.
        let external = self
            .autoplay_tactic_path
            .clone()
            .or_else(env_autoplay_tactic_path);
        let default_plan = engine.autoplay_plan().ok();
        let mut selected_plan = default_plan.clone();
        let mut legacy = None;
        self.external_autoplay = false;
        self.legacy_autoplay = false;
        if let Some(path) = external {
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if let Ok(plan) = serde_json::from_str::<crate::core::types::AutoplayPlan>(&text) {
                plan.validate_for(
                    default_plan
                        .as_ref()
                        .ok_or("backend has no grouped autoplay")?,
                )?;
                selected_plan = Some(plan);
            } else {
                self.legacy_autoplay = true;
                legacy = Some(
                    serde_json::from_str::<Vec<crate::core::types::TimedInputEvent>>(&text)
                        .map_err(|e| format!("invalid autoplay file: {e}"))?,
                );
                eprintln!(
                    "autoplay: legacy event list; seeking uses the core ownership plan instead"
                );
            }
            self.external_autoplay = true;
        }
        self.autoplay_plan_inner_screen = if self.external_autoplay {
            None
        } else {
            Some(self.params.autoplay_inner_screen)
        };
        if let Some(plan) = &mut selected_plan {
            if self.external_autoplay {
                crate::app::autoplay_edit::apply(self.playback_chart(), &engine, plan);
            } else {
                *plan = crate::app::autoplay_edit::generated_plan(
                    self.playback_chart(), &engine, plan, self.params.autoplay_inner_screen,
                );
            }
            self.external_autoplay |= !self.chart.autoplay_overrides.is_empty();
        }
        self.autoplay_tactic = legacy.unwrap_or_else(|| {
            selected_plan
                .as_ref()
                .map(|p| p.events())
                .unwrap_or_else(|| engine.default_tactic().unwrap_or_default())
        });
        self.default_autoplay_plan = default_plan.map(std::sync::Arc::new);
        self.autoplay_plan = selected_plan;
        self.autoplay_tactic_cursor = 0;
        self.judge_windows = engine.judgment_metadata(self.playback_chart());
        self.judge_engine = Some(engine);
        self.engine_events.clear();
        self.core_score = None;
        self.slide_progress.clear();
        self.slide_judge.clear();
        self.hidden_notes.clear();
        self.hold_head_hits.clear();
        self.simai_source = Some(simai_text.to_string());
        self.simai_level = level_index;
        self.simai_fragments =
            crate::app::maidata::simai_timeline(simai_text, level_index).unwrap_or_default();
        Ok(())
    }

    /// Seek the existing runtime back to zero.
    pub fn reset_engine(&mut self) {
        self.seek_to(0.0);
    }

    pub fn edited_engine_context(&self) -> EditedEngineContext {
        EditedEngineContext {
            time: self.timeline_time().max(0.0),
            revision: self.runtime_revision,
            autoplay: self.autoplay,
            direct: self.autoplay_direct_core,
            inner_screen: self.params.autoplay_inner_screen,
            continuous: matches!(self.mode, Mode::Playing | Mode::Recording) && self.forced_time.is_none()
                && !self.scrubbing && !self.playback_pending && !self.audio_start_pending,
        }
    }

    pub fn accepts_edited_engine(&self, context: EditedEngineContext) -> bool {
        let current = self.edited_engine_context();
        current == context || (current.revision == context.revision
            && current.autoplay == context.autoplay && current.direct == context.direct
            && current.inner_screen == context.inner_screen
            && current.continuous && context.continuous && current.time >= context.time)
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    /// Detach without freeing: the background worker owns the old native
    /// session while loading its replacement under Lean's global mutex.
    pub fn detach_edited_engine(&mut self) -> Option<RetiredEditedEngine> {
        let retired = RetiredEditedEngine {
            _engine: self.judge_engine.take()?,
            _windows: std::mem::take(&mut self.judge_windows),
            _tactic: std::mem::take(&mut self.autoplay_tactic),
            _plan: self.autoplay_plan.take(),
            _default_plan: self.default_autoplay_plan.take(),
            _fragments: std::mem::take(&mut self.simai_fragments),
            _source: self.simai_source.take(),
            _progress: std::mem::take(&mut self.slide_progress),
            _hidden_notes: std::mem::take(&mut self.hidden_notes),
            _hold_head_hits: std::mem::take(&mut self.hold_head_hits),
        };
        self.autoplay_tactic_cursor = 0;
        self.core_score = None;
        self.clear_judgment_state();
        Some(retired)
    }

    /// Publish already prepared data; no native calls, audio restart or clock
    /// changes are allowed on this path.
    pub fn install_edited_engine(&mut self, prepared: PreparedEditedEngine) {
        assert!(self.judge_engine.is_none());
        let PreparedEditedEngine {
            engine, context, source, level, plan, inner_plan, default_plan, events, normalized_events, tactic, tactic_cursor,
            score, windows, progress, hidden_notes, cues, fragments, authored, ..
        } = prepared;
        let (plan, events) = if context.inner_screen {
            (inner_plan, normalized_events)
        } else {
            (plan, events)
        };
        self.clear_judgment_state();
        self.autoplay_tactic = tactic;
        self.autoplay_plan = Some(plan);
        self.default_autoplay_plan = Some(default_plan);
        self.autoplay_plan_inner_screen = Some(context.inner_screen);
        self.external_autoplay = authored;
        self.legacy_autoplay = false;
        self.autoplay_tactic_cursor = tactic_cursor;
        self.judge_windows = windows;
        self.judge_engine = Some(engine);
        self.core_score = score;
        if self.use_core() {
            self.hidden_notes = hidden_notes;
        }
        self.apply_core_slide_progress_updates(&progress);
        crate::player::autoplay::restore_at(self, context.time, &events);
        self.cue_track = Some(cues);
        self.simai_fragments = fragments;
        self.simai_source = Some(source);
        self.simai_level = level;
    }

    /// Drop the loaded lnmai-core session (e.g. when switching to a chart with
    /// no Simai source). After this [`Self::use_core`] is false again.
    pub fn unload_engine(&mut self) {
        self.judge_windows = crate::player::judge_windows::Metadata::default();
        self.scrub_origin = None;
        self.judge_engine = None;
        self.autoplay_tactic.clear();
        self.autoplay_plan = None;
        self.default_autoplay_plan = None;
        self.autoplay_plan_inner_screen = None;
        self.autoplay_tactic_cursor = 0;
        self.engine_events.clear();
        self.core_score = None;
        self.slide_progress.clear();
        self.slide_judge.clear();
        self.hidden_notes.clear();
        self.hold_head_hits.clear();
        self.simai_source = None;
        self.simai_level = 0;
        self.simai_fragments.clear();
    }

    pub fn has_engine(&self) -> bool {
        self.judge_engine.is_some()
    }

    /// Replace the chart (and audio) in place so the editor can open / switch
    /// charts without a restart. Textures, materials and SFX are chart-independent
    /// and stay loaded; only chart-derived runtime state is rebuilt.
    pub fn load_chart(
        &mut self,
        mut chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
        simai_source: Option<String>,
        simai_level: Option<u32>,
    ) {
        self.document_revision = self.document_revision.wrapping_add(1);
        self.runtime_revision = self.runtime_revision.wrapping_add(1);
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        self.chart = chart;
        self.expanded_source = None;
        self.refresh_playback_chart();
        self.cue_track = Some(CueTrack::from_chart(self.playback_chart()));

        self.audio_source_name = audio_source_name;
        self.audio_wav_pcm = audio_wav_pcm;
        self.audio_cache.clear();
        self.audio_seek_offset = None;
        self.pending_audio_start = false;
        self.stop_audio_if_any();

        self.mode = Mode::Idle;
        self.mode_song_offset = 0.0;
        self.scrub_origin = None;
        self.timeline_view_time = 0.0;
        self.mode_wall_anchor = self.now();
        self.playback_pending = false;
        self.audio_start_pending = false;

        self.hidden_notes.clear();
        self.hold_head_hits.clear();
        self.slide_progress.clear();
        self.slide_judge.clear();
        self.active_pointer_zones.clear();
        self.prev_pointer_pos.clear();
        self.pad_feedback.clear();
        self.judge_feedback.clear();
        self.hit_fx.clear();

        self.unload_engine();
        if let Some(text) = simai_source {
            let level = simai_level
                .or_else(|| crate::app::maidata::inote_key(&text, None))
                .unwrap_or(0);
            if let Err(e) = self.load_engine(&text, level) {
                eprintln!("editor: engine load failed: {e}");
            }
        }
        crate::player::autoplay::rebuild(self);
    }

    /// Whether lnmai-core should actually drive judging / slide rendering. When
    /// the `no_core` option is on, the pad renders without core judging.
    pub fn use_core(&self) -> bool {
        self.has_engine() && !crate::app::params::no_core()
    }

    // ── lnmai-core score read-outs ───────────────────────────────────

    /// Current combo from lnmai-core (0 before the engine reports).
    pub fn combo(&self) -> u64 {
        self.core_score.as_ref().map(|s| s.combo).unwrap_or(0)
    }

    /// Pure combo (Perfect-grade chain) from lnmai-core.
    pub fn p_combo(&self) -> u64 {
        self.core_score.as_ref().map(|s| s.p_combo).unwrap_or(0)
    }

    /// Critical-perfect combo from lnmai-core.
    pub fn c_p_combo(&self) -> u64 {
        self.core_score.as_ref().map(|s| s.c_p_combo).unwrap_or(0)
    }

    pub fn fast_count(&self) -> u64 {
        self.core_score.as_ref().map(|s| s.fast_count).unwrap_or(0)
    }

    pub fn late_count(&self) -> u64 {
        self.core_score.as_ref().map(|s| s.late_count).unwrap_or(0)
    }

    /// Total judged notes per display grade — `(perfect, great, good, miss)` —
    /// aggregated across all note families (tap/hold/slide/touch/break). Grades
    /// collapse like the judgment label: Perfect / Great / Good / Miss+TooFast.
    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    pub fn grade_totals(&self) -> (u64, u64, u64, u64) {
        use crate::core::types::JudgeGrade;
        let Some(score) = self.core_score.as_ref() else {
            return (0, 0, 0, 0);
        };
        let counts = &score.counts;
        let miss = counts.grade_count_where(|g| g.is_miss_or_too_fast());
        let great = counts.grade_count_where(JudgeGrade::is_great_grade);
        let good = counts.grade_count_where(JudgeGrade::is_good_grade);
        let perfect = counts.grade_count_where(|g| {
            !g.is_miss_or_too_fast() && !g.is_great_grade() && !g.is_good_grade()
        });
        (perfect, great, good, miss)
    }

    /// Achieved DX score.
    pub fn dx_score(&self) -> i64 {
        self.core_score
            .as_ref()
            .map(|s| s.dx_score_remaining())
            .unwrap_or(0)
    }

    pub fn max_dx_score(&self) -> u64 {
        self.core_score
            .as_ref()
            .map(|s| s.max_dx_score)
            .unwrap_or(0)
    }

    /// Achievement percentage (lnmai-core's `dxAccMinus101`):
    /// `earnedBase/totalBase*100 + earnedExtra/totalExtra`.
    pub fn achievement(&self) -> Option<f32> {
        let s = self.core_score.as_ref()?;
        if s.total_base == 0 {
            return Some(0.0);
        }
        let base = s.earned_base as f64 / s.total_base as f64 * 100.0;
        let extra = if s.total_extra == 0 {
            0.0
        } else {
            s.earned_extra as f64 / s.total_extra as f64
        };
        Some((base + extra) as f32)
    }

    /// The five lnmai-core accuracy rates (percent), matching its `AccRates`:
    /// classic acc(+), classic acc(-), DX acc101(-), DX acc100(-), DX acc(+).
    pub fn acc_rates(&self) -> Option<[(&'static str, f32); 5]> {
        let s = self.core_score.as_ref()?;
        if s.total_base == 0 {
            return None;
        }
        let tb = s.total_base as f64;
        let te = s.total_extra.max(1) as f64;
        let cb = s.earned_base as f64;
        let ce = s.earned_extra as f64;
        let cc = s.earned_classic_extra as f64;
        let earned_base = tb - s.lost_base as f64;
        let earned_extra = te - s.lost_extra as f64;
        let classic_plus = (cb + cc) / tb * 100.0;
        let classic_minus = (earned_base + cc) / tb * 100.0;
        let dx_101 = (earned_base / tb + earned_extra / (te * 100.0)) * 100.0;
        let dx_100 = (earned_base / tb + ce / (te * 100.0)) * 100.0;
        let dx_plus = (cb / tb + ce / (te * 100.0)) * 100.0;
        Some([
            ("ACC+", classic_plus as f32),
            ("ACC-", classic_minus as f32),
            ("ACC101-", dx_101 as f32),
            ("ACC100-", dx_100 as f32),
            ("ACC(+)", dx_plus as f32),
        ])
    }

    /// Combo category label (FC / AP / …) from lnmai-core.
    pub fn combo_state_label(&self) -> &'static str {
        use crate::core::types::ComboState::*;
        match self.core_score.as_ref().map(|s| s.combo_state()) {
            Some(FC) => "FC",
            Some(FCPlus) => "FC+",
            Some(AP) => "AP",
            Some(APPlus) => "AP+",
            _ => "",
        }
    }

    /// Apply lnmai-core's per-runtime-arc consumed fractions to the chart's
    /// `(note_id, slide_idx)` sub-slides used by the renderer.
    ///
    /// lnmai splits a continuous chain into one runtime arc per chart segment, so
    /// each arc's fraction is slotted into **that segment's** range: an earlier
    /// arc finishing can no longer hide the whole sub-slide.
    pub fn apply_core_slide_progress_updates(
        &mut self,
        updates: &[crate::player::engine::SlideArcProgress],
    ) {
        for update in updates {
            let Some((note_id, slide_idx, seg_idx, seg_count)) = self
                .judge_engine
                .as_ref()
                .and_then(|engine| engine.slide_position(self.playback_chart(), update.runtime_slide_index))
                .or_else(|| {
                    if self.judge_engine.is_none() {
                        crate::player::engine::chart_slide_position(
                            self.playback_chart(),
                            update.runtime_slide_index,
                        )
                    } else {
                        None
                    }
                })
            else {
                continue;
            };
            let progress = self
                .slide_progress
                .entry((note_id, slide_idx))
                .or_insert_with(|| SlideProgress {
                    seg_frac: vec![0.0; seg_count],
                    track_frac: HashMap::new(),
                    track_hidden_until: HashMap::new(),
                });
            // Wifi per-track progress is kept separate so each track consumes on
            // its own; everything else slots into the sub-slide's segment range.
            if let Some(track) = update.track_index {
                if let Some(bar) = update.hidden_until_bar {
                    let slot = progress.track_hidden_until.entry(track).or_insert(0);
                    *slot = (*slot).max(bar);
                }
                let slot = progress.track_frac.entry(track).or_insert(0.0);
                *slot = slot.max(update.frac);
                // Also fold into `seg_frac` so non-wifi slides that emit
                // per-track progress (conn-slides) keep consuming as before.
                if progress.seg_frac.len() < seg_count {
                    progress.seg_frac.resize(seg_count, 0.0);
                }
                let slot = &mut progress.seg_frac[seg_idx];
                *slot = slot.max(update.frac);
                continue;
            }
            if progress.seg_frac.len() < seg_count {
                progress.seg_frac.resize(seg_count, 0.0);
            }
            let slot = &mut progress.seg_frac[seg_idx];
            *slot = slot.max(update.frac);
        }

        // A segment can only be reached once the ones before it are consumed, so
        // any segment ahead of the furthest active arc is fully done.
        for progress in self.slide_progress.values_mut() {
            if let Some(last) = progress.seg_frac.iter().rposition(|f| *f > 0.0) {
                for f in &mut progress.seg_frac[..last] {
                    *f = 1.0;
                }
            }
        }

        if crate::player::engine::debug_slide_enabled() {
            for ((note_id, slide_idx), progress) in &self.slide_progress {
                let total = progress.seg_frac.len();
                let traveled: f32 = progress.seg_frac.iter().sum();
                let key = format!("{note_id},{slide_idx}|{:?}", progress.seg_frac);
                if crate::player::engine::debug_dedup(
                    format!("player_state/{note_id}/{slide_idx}"),
                    &key,
                ) {
                    eprintln!(
                        "[slide/player] star key=({note_id},{slide_idx}) \
                         traveled={traveled:.3}/{total} segments seg_frac={:?}",
                        progress.seg_frac
                    );
                }
            }
        }
    }

    /// Queue an lnmai-core sensor press for `zone` at microsecond time `tp`.
    pub fn queue_engine_press(&mut self, zone: PadZone, tp: i64) {
        if self.judge_engine.is_none() {
            return;
        }
        self.engine_events
            .extend(crate::player::engine::press_events_for_zone(zone, tp));
    }

    /// Queue an lnmai-core sensor release for `zone` at microsecond time `tp`.
    pub fn queue_engine_release(&mut self, zone: PadZone, tp: i64) {
        if self.judge_engine.is_none() {
            return;
        }
        self.engine_events
            .extend(crate::player::engine::release_events_for_zone(zone, tp));
    }

    /// Queue a hold-only sensor press (slide body move, no click) for `zone`.
    pub fn queue_engine_hold(&mut self, zone: PadZone, tp: i64) {
        if self.judge_engine.is_none() {
            return;
        }
        self.engine_events
            .extend(crate::player::engine::hold_events_for_zone(zone, tp));
    }
}

/// `MAI2_AUTOPLAY_TACTIC=<path>` — env fallback for `--autoplay-tactic`.
fn env_autoplay_tactic_path() -> Option<std::path::PathBuf> {
    std::env::var_os("MAI2_AUTOPLAY_TACTIC").map(std::path::PathBuf::from)
}

/// Judge-label → hit-effect tint.
fn hit_fx_color(label: &str, is_break: bool) -> Color {
    let l = label.to_ascii_lowercase();
    if l.contains("miss") {
        return Color::from_rgba(200, 90, 90, 255);
    }
    if is_break {
        return Color::from_rgba(255, 150, 60, 255);
    }
    if l.contains("perfect") {
        Color::from_rgba(255, 214, 76, 255)
    } else if l.contains("great") {
        Color::from_rgba(120, 220, 255, 255)
    } else if l.contains("good") {
        Color::from_rgba(130, 240, 130, 255)
    } else {
        Color::from_rgba(255, 255, 255, 255)
    }
}

fn note_end_seconds(note: &crate::app::types::Note, bpms: &[crate::app::types::BpmChange]) -> f32 {
    use crate::app::types::{NoteType, hold_tail_time, note_secs, slide_end_time};
    match note.note_type {
        NoteType::Hold | NoteType::TouchHold => hold_tail_time(note, bpms),
        NoteType::Slide => slide_end_time(note, bpms),
        _ => note_secs(note, bpms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "backend-lean")]
    #[test]
    #[ignore = "interactive scrub timing budget; run without concurrent tests"]
    fn dense_scrubbing_stays_interactive_in_both_autoplay_modes() {
        use std::time::{Duration, Instant};

        let text = format!(
            "&inote_1=(120){{4}}{}E",
            "1/2h[4:1]/B3/4-8[4:1],".repeat(256)
        );
        let mut worst = Duration::ZERO;
        for direct in [false, true] {
            let chart = crate::app::maidata::from_maidata(&text, None).unwrap();
            let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
            pad.load_engine(&text, 1).unwrap();
            pad.autoplay = true;
            pad.autoplay_direct_core = direct;
            pad.scrubbing = true;
            for seconds in [0, 10, 30, 60, 120, 60, 30, 10, 0] {
                let target = seconds as f32;
                pad.forced_time = Some(target);
                let start = Instant::now();
                pad.scrub_to(target);
                let elapsed = start.elapsed();
                eprintln!(
                    "scrub direct={direct} {seconds:3}s: {:.3} ms",
                    elapsed.as_secs_f64() * 1_000.0
                );
                worst = worst.max(elapsed);
                assert_eq!(pad.timeline_view_time, target);
                assert!(pad.engine_events.is_empty());
                let score = pad.core_score.as_ref().unwrap();
                assert_eq!(score.earned_base, 0);
                assert_eq!(score.lost_base, 0);
            }
        }
        assert!(worst < Duration::from_millis(100), "scrub took {worst:?}");
    }

    #[cfg(feature = "backend-lean")]
    #[test]
    fn scrubbing_restores_crossing_bodies_and_autoplay_holds() {
        for simai in ["1h[1:2]", "C1h[1:2]", "1-5[1:2]", "1-5[1:1]-8[1:1]"] {
            let text = format!("&inote_1=(120){{4}}{simai},E");
            let chart = crate::app::maidata::from_maidata(&text, None).unwrap();
            let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
            pad.load_engine(&text, 1).unwrap();
            pad.autoplay = true;
            pad.scrubbing = true;
            for target in [1.0, 0.0, 1.0] {
                pad.forced_time = Some(target);
                pad.scrub_to(target);
                assert!(
                    !pad.hidden_notes.contains(&pad.chart.notes[0].id),
                    "{simai}"
                );
                assert!(!pad.pending_audio_start);
                if target > 0.0 && !pad.chart.notes[0].slide.is_empty() {
                    assert!(
                        !pad.slide_progress
                            .values()
                            .any(|p| p.seg_frac.iter().all(|f| *f >= 1.0))
                    );
                }
                if target > 0.0 && pad.chart.notes[0].slide.is_empty() {
                    assert!(
                        !pad.autoplay_explicit_held.is_empty(),
                        "{simai}: lost held autoplay input"
                    );
                    assert!(!pad.active_pointer_zones.is_empty());
                }
            }
        }
    }

    fn rgb(c: Color) -> (u8, u8, u8) {
        (
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        )
    }

    #[cfg(feature = "backend-lean")]
    #[test]
    fn autoplay_resumes_crossing_hold_and_next_note_in_both_input_modes() {
        for direct in [false, true] {
            let text = "&inote_1=(120){1}1h[1:2],,,1,E";
            let chart = crate::app::maidata::from_maidata(text, None).unwrap();
            let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
            pad.load_engine(text, 1).unwrap();
            pad.autoplay = true;
            pad.autoplay_direct_core = direct;
            pad.scrub_to(1.0);
            assert!(pad.engine_events.is_empty());
            let remaining = pad.autoplay_tactic.clone();
            for event in remaining {
                let tp = crate::player::engine::timed_input_tp(&event);
                if tp < 1_000_000 {
                    continue;
                }
                let time = tp as f32 / 1e6;
                pad.forced_time = Some(time);
                crate::player::autoplay::tick(&mut pad);
                let inputs = std::mem::take(&mut pad.engine_events);
                for result in crate::player::engine::step_engine_events(pad.judge_engine.as_mut().unwrap(), time, inputs) {
                    result.unwrap();
                }
            }
            let result = pad.judge_engine.as_mut().unwrap().step(10.0, Vec::new()).unwrap();
            assert_eq!(result.score.lost_base, 0, "direct={direct}");
            assert_eq!(result.score.earned_base, result.score.total_base, "direct={direct}");
            assert!(pad.autoplay_explicit_held.is_empty());
            assert!(pad.autoplay_direct_held.is_empty());
        }
    }

    #[test]
    fn hit_fx_color_tracks_grade_and_break() {
        assert_eq!(rgb(hit_fx_color("Perfect", false)), (255, 214, 76));
        assert_eq!(rgb(hit_fx_color("Great", false)), (120, 220, 255));
        assert_eq!(rgb(hit_fx_color("Good", false)), (130, 240, 130));
        assert_eq!(rgb(hit_fx_color("Miss", false)), (200, 90, 90));
        // Break overrides the grade tint with orange (except misses).
        assert_eq!(rgb(hit_fx_color("Perfect", true)), (255, 150, 60));
        assert_eq!(rgb(hit_fx_color("Miss", true)), (200, 90, 90));
    }
}
