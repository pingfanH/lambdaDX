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

    // ── Progress / seeking ───────────────────────────────────────────
    /// True while the progress bar is being dragged.
    pub scrubbing: bool,
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
    /// Pending input events (pad presses/releases) for the next engine step.
    pub engine_events: Vec<crate::core::types::TimedInputEvent>,
    /// Autoplay: lnmai-core's default replay tactic, consumed by timestamp.
    pub autoplay_tactic: Vec<crate::core::types::TimedInputEvent>,
    pub autoplay_tactic_cursor: usize,
    /// Latest lnmai-core score snapshot (combo, DX score, judge counts).
    pub core_score: Option<crate::core::types::ScoreState>,
    /// Raw Simai fragments of the loaded chart, timed in seconds — used by the
    /// HUD to show which fragment is currently playing.
    pub simai_fragments: Vec<crate::app::maidata::SimaiFragment>,
    /// Simai source + `&inote_N` used to (re)build the engine on restart.
    simai_source: Option<String>,
    simai_level: u32,
}

impl PadPreviewState {
    pub fn new(
        mut chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
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

        let cue_track = Some(CueTrack::from_chart(&chart));

        Self {
            mode: Mode::Idle,
            mode_wall_anchor: get_time(),
            mode_song_offset: 0.0,
            playback_pending: false,
            play_speed: 1.0,
            timeline_view_time: 0.0,
            forced_time: None,
            note_speed: NOTE_SPEED,
            touch_speed: NOTE_SPEED,
            slide_fade_in: 3.926_913 / crate::app::types::note_speed_from_setting(NOTE_SPEED),
            chart,
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
            bgm_player: BgmPlayer::new().ok(),
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
            cover_texture: None,
            tap_guide_tex: None,
            tap_guide_each_tex: None,
            tap_guide_break_tex: None,
            slide_guide_tex: None,
            hold_end_guide_tex: None,
            hold_end_each_guide_tex: None,
            hold_end_break_guide_tex: None,
            autoplay: false,
            autoplay_click_held: Vec::new(),
            autoplay_explicit_held: Vec::new(),
            autoplay_tactic_path: None,
            external_autoplay: false,
            scrubbing: false,
            scrub_pointer: None,
            video_bg: VideoBg::new(),
            mobile_ui,
            ui_scale_override,
            status: "Ready".to_string(),
            params: crate::app::params::Params::default(),
            show_params: false,
            judge_engine: None,
            engine_events: Vec::new(),
            autoplay_tactic: Vec::new(),
            autoplay_tactic_cursor: 0,
            core_score: None,
            simai_fragments: Vec::new(),
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
        if let Some(t) = self.forced_time {
            return t;
        }
        if self.playback_pending {
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
            self.mode_wall_anchor = get_time();
            self.playback_pending = false;
            self.stop_audio_if_any();
            self.timeline_view_time = self.mode_song_offset;
            self.active_pointer_zones.clear();
            self.prev_pointer_pos.clear();
            self.set_status(format!("Paused at {:.2}s", self.mode_song_offset));
        } else {
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.mode = Mode::Playing;
            self.mode_wall_anchor = get_time();
            self.playback_pending = false;
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
        self.mode = Mode::Playing;
        self.timeline_view_time = time.max(0.0);
        self.mode_song_offset = time.max(0.0);
        self.mode_wall_anchor = get_time();
        self.playback_pending = true;
        self.audio_seek_offset = Some(time.max(0.0));
        self.active_pointer_zones.clear();
        self.prev_pointer_pos.clear();
        self.slide_progress.clear();
        self.slide_judge.clear();
        self.hidden_notes.clear();
        self.hold_head_hits.clear();
        // Restarting from the top rebuilds the core session so combo/DX reset.
        if time <= 1e-4 {
            self.reset_engine();
        }
        self.reconcile_slide_progress_for(self.mode_song_offset);
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

    /// Song length in seconds (from the decoded PCM; falls back to 1s).
    pub fn song_duration(&self) -> f32 {
        self.audio_wav_pcm
            .as_ref()
            .map(|pcm| {
                pcm.samples.len() as f32 / f32::from(pcm.channels.max(1)) / pcm.sample_rate as f32
            })
            .unwrap_or(1.0)
            .max(1.0)
    }

    /// Update the visible position while dragging the progress bar (no audio
    /// restart).
    pub fn scrub_to(&mut self, t: f32) {
        let t = t.clamp(0.0, self.song_duration());
        self.mode_song_offset = t;
        self.timeline_view_time = t;
        self.mode_wall_anchor = get_time();
        self.reset_cues(t);
        self.reconcile_slide_progress_for(t);
    }

    /// Commit a seek: reposition the clock and restart audio if playing.
    pub fn seek_to(&mut self, t: f32) {
        let t = t.clamp(0.0, self.song_duration());
        self.seek_audio_to(t);
        self.mode_song_offset = t;
        self.timeline_view_time = t;
        self.mode_wall_anchor = get_time();
        self.reconcile_slide_progress_for(t);
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
        use crate::player::render::skin::SkinVariant;
        use crate::player::sfx::{self, SfxKind};
        use crate::player::cues::Cue;
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
        if self.mode != Mode::Playing || self.playback_pending {
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
        self.play_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
        if self.mode == Mode::Playing {
            self.mode_song_offset = self.song_time();
            self.mode_wall_anchor = get_time();
            self.audio_seek_offset = Some(self.mode_song_offset);
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
        let loaded = external.and_then(|p| read_tactic_file(&p));
        self.external_autoplay = loaded.is_some();
        self.autoplay_tactic =
            loaded.unwrap_or_else(|| engine.default_tactic().unwrap_or_default());
        self.autoplay_tactic_cursor = 0;
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

    /// Rebuild the engine (resetting core score/slide state) from the stored
    /// Simai source, if any. Used on restart.
    pub fn reset_engine(&mut self) {
        if let Some(text) = self.simai_source.clone() {
            let level = self.simai_level;
            let _ = self.load_engine(&text, level);
        }
    }

    /// Drop the loaded lnmai-core session (e.g. when switching to a chart with
    /// no Simai source). After this [`Self::use_core`] is false again.
    pub fn unload_engine(&mut self) {
        self.judge_engine = None;
        self.autoplay_tactic.clear();
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
        self.core_score.as_ref().map(|s| s.max_dx_score).unwrap_or(0)
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

    /// Mark slides whose local tail is already past `t` as hidden and un-hide
    /// the rest. Used after a seek/scrub so the core-driven slides do not pile
    /// up: lnmai-core does not backfill slides skipped by a timeline jump.
    pub fn reconcile_slide_progress_for(&mut self, t: f32) {
        if !self.use_core() {
            return;
        }
        use crate::app::types::{NoteType, mdur_to_secs, note_secs};
        let bpms = self.chart.bpms.clone();
        let mut past: Vec<((u64, usize), usize)> = Vec::new();
        let mut future: Vec<(u64, usize)> = Vec::new();
        for note in &self.chart.notes {
            if !matches!(note.note_type, NoteType::Slide) {
                continue;
            }
            let ns = note_secs(note, &bpms);
            for (si, sl) in note.slide.iter().enumerate() {
                let end = ns + mdur_to_secs(sl.slide_duration, note.time, &bpms);
                if end < t {
                    past.push(((note.id, si), sl.runtime_parts));
                } else {
                    future.push((note.id, si));
                }
            }
        }
        for key in future {
            self.slide_progress.remove(&key);
        }
        for (key, parts) in past {
            self.slide_progress.insert(
                key,
                SlideProgress {
                    seg_frac: vec![1.0; parts.max(1)],
                    track_frac: HashMap::new(),
                    track_hidden_until: HashMap::new(),
                },
            );
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
            let Some((note_id, slide_idx, seg_idx, seg_count)) =
                crate::player::engine::chart_slide_position(&self.chart, update.runtime_slide_index)
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

/// Read a JSON `TimedInputEvent` list produced by the `autoplay_gen` bin.
fn read_tactic_file(
    path: &std::path::Path,
) -> Option<Vec<crate::core::types::TimedInputEvent>> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: Color) -> (u8, u8, u8) {
        (
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        )
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
