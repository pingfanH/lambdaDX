use std::collections::{HashMap, HashSet};

use macroquad::prelude::{Color, Texture2D, Vec2, get_time};

use crate::app::audio::{BgmPcm, BgmPlayer, SfxBuffer};
use crate::app::pad_svg::PadSvgDef;
use crate::app::params;
use crate::app::types::zone::PadZone;
use crate::app::types::{
    ChartDoc, JudgeFeedback, Mode, NOTE_SPEED, PadFeedback, SPEED_MAX, SPEED_MIN, WavPcm,
};
use crate::player::cues::CueTrack;
use crate::player::video::VideoBg;

/// Per-sub-slide visual progress. In the standalone preview the trail is never
/// hidden by judgment, so `hidden_until_bar` stays 0; kept as a typed map so the
/// renderer's lookup matches the original player.
#[derive(Debug, Clone, Copy, Default)]
pub struct SlideProgress {
    pub hidden_until_bar: usize,
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

    // ── Note appearance ──────────────────────────────────────────────
    pub note_speed: f32,
    pub touch_speed: f32,
    pub slide_fade_in: f32,

    // ── Chart ────────────────────────────────────────────────────────
    pub chart: ChartDoc,
    pub hidden_notes: HashSet<u64>,
    pub slide_progress: HashMap<(u64, usize), SlideProgress>,

    // ── Pad interaction ──────────────────────────────────────────────
    pub pad_svg: Option<PadSvgDef>,
    pub active_pointer_zones: HashMap<u64, PadZone>,
    pub prev_pointer_pos: HashMap<u64, Vec2>,
    pub pad_feedback: Vec<PadFeedback>,
    pub judge_feedback: Vec<JudgeFeedback>,

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
    /// Judgment cue sounds per kind (tap / slide / hold / break), if present.
    pub sfx_tap: Option<SfxBuffer>,
    pub sfx_slide: Option<SfxBuffer>,
    pub sfx_hold: Option<SfxBuffer>,
    pub sfx_break: Option<SfxBuffer>,
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
    /// When on, the pad feeds lnmai-core's default tactic.
    pub autoplay: bool,
    pub autoplay_cursor: usize,

    // ── Progress / seeking ───────────────────────────────────────────
    /// True while the progress bar is being dragged.
    pub scrubbing: bool,

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
    pub engine_events: Vec<lnmai_core::types::TimedInputEvent>,
    /// Autoplay: lnmai-core's default replay tactic, consumed by timestamp.
    pub autoplay_tactic: Vec<lnmai_core::types::TimedInputEvent>,
    pub autoplay_tactic_cursor: usize,
    pub autoplay_click_held: Vec<lnmai_core::types::SensorArea>,
    /// Latest lnmai-core score snapshot (combo, DX score, judge counts).
    pub core_score: Option<lnmai_core::types::ScoreState>,
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
            note_speed: NOTE_SPEED,
            touch_speed: NOTE_SPEED*0.7,
            slide_fade_in: 3.926_913 / NOTE_SPEED,
            chart,
            hidden_notes: HashSet::new(),
            slide_progress: HashMap::new(),
            pad_svg: None,
            active_pointer_zones: HashMap::new(),
            prev_pointer_pos: HashMap::new(),
            pad_feedback: Vec::new(),
            judge_feedback: Vec::new(),
            audio_source_name,
            audio_wav_pcm,
            audio_cache: HashMap::new(),
            audio_seek_offset: None,
            pending_audio_start: false,
            audio_enabled: true,
            bgm_player: BgmPlayer::new().ok(),
            answer_sfx: None,
            sfx_tap: None,
            sfx_slide: None,
            sfx_hold: None,
            sfx_break: None,
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
            autoplay_cursor: 0,
            scrubbing: false,
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
            autoplay_click_held: Vec::new(),
            core_score: None,
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

    /// The judgment cue sound for a note kind/variant (falls back to
    /// `answer.wav`).
    fn cue_sfx(&self, cue: crate::player::cues::Cue, is_break: bool) -> Option<&SfxBuffer> {
        use crate::player::cues::Cue;
        let picked = if is_break {
            self.sfx_break.as_ref()
        } else {
            match cue {
                Cue::Tap => self.sfx_tap.as_ref(),
                Cue::SlideHead => self.sfx_tap.as_ref(),
                Cue::HoldHead | Cue::HoldTail => self.sfx_hold.as_ref(),
            }
        };
        picked.or(self.answer_sfx.as_ref())
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
        for ev in due {
            let buf = if params::judge_sfx() {
                self.cue_sfx(ev.cue, ev.is_break)
            } else {
                self.answer_sfx.as_ref()
            };
            self.play_sfx(buf);
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
        let now = get_time();
        self.pad_feedback.retain(|f| f.until > now);
        self.judge_feedback.retain(|f| f.until > now);
    }

    pub fn push_feedback(&mut self, zone: PadZone, duration: f64) {
        self.pad_feedback.push(PadFeedback {
            zone,
            until: get_time() + duration,
        });
    }

    pub fn push_judgement(&mut self, zone: PadZone, label: &str, duration: f64) {
        let now = get_time();
        self.judge_feedback.push(JudgeFeedback {
            zone,
            label: label.to_string(),
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            started: now,
            until: now + duration,
        });
    }

    // ── lnmai-core judgment engine ───────────────────────────────────

    /// Load `lnmai-core`'s judgment engine for a chart given as Simai text.
    ///
    /// `level_index` is the `&inote_N` block to select. Also builds the default
    /// replay tactic used by autoplay.
    pub fn load_engine(&mut self, simai_text: &str, level_index: u32) -> Result<(), String> {
        let engine = crate::player::engine::JudgeEngine::load(simai_text, level_index)?;
        self.autoplay_tactic = engine.default_tactic().unwrap_or_default();
        self.autoplay_tactic_cursor = 0;
        self.autoplay_click_held.clear();
        self.judge_engine = Some(engine);
        self.engine_events.clear();
        self.core_score = None;
        self.slide_progress.clear();
        self.simai_source = Some(simai_text.to_string());
        self.simai_level = level_index;
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

    pub fn has_engine(&self) -> bool {
        self.judge_engine.is_some()
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
        use lnmai_core::types::ComboState::*;
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
        if self.judge_engine.is_none() {
            return;
        }
        use crate::app::types::{NoteType, mdur_to_secs, note_secs};
        let bpms = self.chart.bpms.clone();
        let mut past: Vec<(u64, usize)> = Vec::new();
        let mut future: Vec<(u64, usize)> = Vec::new();
        for note in &self.chart.notes {
            if !matches!(note.note_type, NoteType::Slide) {
                continue;
            }
            let ns = note_secs(note, &bpms);
            for (si, sl) in note.slide.iter().enumerate() {
                let end = ns + mdur_to_secs(sl.slide_duration, note.time, &bpms);
                if end < t {
                    past.push((note.id, si));
                } else {
                    future.push((note.id, si));
                }
            }
        }
        for key in future {
            self.slide_progress.remove(&key);
        }
        for key in past {
            self.slide_progress.insert(
                key,
                SlideProgress {
                    hidden_until_bar: usize::MAX,
                },
            );
        }
    }

    /// Apply lnmai-core's per-slide trail-consumption state to the chart's
    /// `(note_id, slide_idx)` keys used by the renderer.
    pub fn apply_core_slide_progress_updates(
        &mut self,
        updates: &[crate::player::engine::SlideProgressUpdate],
    ) {
        for update in updates {
            let Some((note_id, slide_idx)) =
                crate::player::engine::chart_slide_key(&self.chart, update.runtime_slide_index)
            else {
                continue;
            };
            self.slide_progress
                .entry((note_id, slide_idx))
                .and_modify(|progress| progress.hidden_until_bar = update.hidden_until_bar)
                .or_insert(SlideProgress {
                    hidden_until_bar: update.hidden_until_bar,
                });
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
}
