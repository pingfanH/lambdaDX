use macroquad::material::Material;
use macroquad::prelude::{get_time, Vec2};
use macroquad::texture::Texture2D;
use std::collections::{HashMap, HashSet};

use super::audio::BgmPcm;
use super::sfx::{SfxBuffer, SfxPlayer};

use super::types::{
    ActiveRecordHold, ChartDoc, HitEvent, Mode, Note, NoteType, PadFeedback, RecordInputId, WavPcm, DragPart,
    HIT_WINDOW, HOLD_RECORD_MIN_DURATION, SPEED_MAX, SPEED_MIN,
    TOUCH_DISAPPEAR_TIME, SLIDE_MIN_POINTS, hold_tail_time, is_touch_zone, sanitize_note_zone,
    note_secs, secs_to_measure, mdur_to_secs, sdur_to_mdur, snap_measure,
};

/// Runtime mutable state for the editor/simulator.
pub(crate) struct AppState {
    pub(crate) mode: Mode,
    pub(crate) mode_wall_anchor: f64,
    pub(crate) mode_song_offset: f32,

    pub(crate) chart: ChartDoc,
    pub(crate) recording_hits: Vec<HitEvent>,
    pub(crate) recording_notes: Vec<Note>,
    pub(crate) active_record_holds: HashMap<RecordInputId, ActiveRecordHold>,
    pub(crate) active_pointer_zones: HashMap<u64, u8>,
    pub(crate) prev_pointer_pos: HashMap<u64, Vec2>,
    pub(crate) pad_feedback: Vec<PadFeedback>,
    pub(crate) playback_cursor: usize,
    pub(crate) selected_note: Option<usize>,
    pub(crate) dragging_note: Option<usize>,
    /// Note index detected under the mouse on press; selection is deferred
    /// until a drag threshold is exceeded.
    pub(crate) press_note_candidate: Option<usize>,
    pub(crate) drag_part: Option<DragPart>,
    pub(crate) drag_start_pos: Option<Vec2>,
    pub(crate) drag_start_time: f32,
    /// Cursor's chart time at the moment of click. Used so dragging tracks the
    /// mouse's absolute position even if the user scrolls the timeline mid-drag.
    pub(crate) drag_cursor_anchor_t: f32,
    pub(crate) drag_multi_orig: Vec<(usize, f32, u8)>,
    pub(crate) box_start: Option<Vec2>,
    pub(crate) box_end: Option<Vec2>,
    /// Chart time anchored for the box-select start, so the start point sticks
    /// to the timeline as the user scrolls during a selection drag.
    pub(crate) box_anchor_t: Option<f32>,
    /// Currently selected sidebar tool (Tap / Hold / Star).
    pub(crate) place_tool: super::types::PlaceTool,
    /// Multi-step placement state machine (for Hold and Star tools).
    pub(crate) placement: super::types::PlacementState,
    /// When `Some(i)`, the user is editing the trajectory (slide_points) of
    /// chart.notes[i] by clicking zones on the Pad. Only meaningful when the
    /// note is a Slide and the app is in Idle mode.
    pub(crate) editing_slide_path: Option<usize>,
    /// Pending slide shape key (e.g. Q, P, S, Z) waiting for a lane number to complete.
    pub(crate) pending_slide_shape: Option<super::types::SlideShape>,
    pub(crate) waveform_data: Vec<f32>,
    pub(crate) waveform_freq_bins: u32,
    pub(crate) waveform_time_res: f32,
    pub(crate) waveform_threshold: f32,
    pub(crate) record_snap_grid: bool,
    pub(crate) selected_notes: Vec<usize>,
    pub(crate) drag_orig_note: Option<super::types::Note>,
    pub(crate) timeline_view_time: f32,
    pub(crate) timeline_zoom: f32,
    pub(crate) dragging_progress_bar: bool,
    pub(crate) undo_stack: Vec<super::types::ChartDoc>,
    pub(crate) clipboard: Vec<super::types::Note>,
    pub(crate) pasting: bool,
    /// When true, B/X hotkeys modify star head flags instead of slide trail.
    pub(crate) editing_star: bool,
    /// Timestamp of last left-click for double-click detection.
    pub(crate) last_click_time: f64,
    pub(crate) last_click_note: Option<usize>,

    pub(crate) record_speed: f32,
    pub(crate) play_speed: f32,
    pub(crate) touch_speed: f32,

    pub(crate) show_pad_only: bool,
    pub(crate) mobile_ui: bool,
    pub(crate) ui_scale_override: Option<f32>,

    pub(crate) audio_source_name: Option<String>,
    pub(crate) audio_wav_pcm: Option<WavPcm>,
    pub(crate) tap_texture: Option<Texture2D>,
    pub(crate) hold_texture: Option<Texture2D>,
    pub(crate) touch_tri_tex: Option<Texture2D>,
    pub(crate) touch_point_tex: Option<Texture2D>,
    pub(crate) tap_each_tex: Option<Texture2D>,
    pub(crate) hold_each_tex: Option<Texture2D>,
    pub(crate) touch_tri_each_tex: Option<Texture2D>,
    pub(crate) touch_point_each_tex: Option<Texture2D>,
    pub(crate) touchhold_tex: [Option<Texture2D>; 4],
    pub(crate) touchhold_border_tex: Option<Texture2D>,
    pub(crate) slide_tex: Option<Texture2D>,
    pub(crate) slide_each_tex: Option<Texture2D>,
    pub(crate) star_tex: Option<Texture2D>,
    pub(crate) star_each_tex: Option<Texture2D>,
    pub(crate) star_break_tex: Option<Texture2D>,
    pub(crate) star_double_tex: Option<Texture2D>,
    pub(crate) star_double_each_tex: Option<Texture2D>,
    // Break textures
    pub(crate) tap_break_tex: Option<Texture2D>,
    pub(crate) hold_break_tex: Option<Texture2D>,
    pub(crate) slide_break_tex: Option<Texture2D>,
    pub(crate) star_double_break_tex: Option<Texture2D>,
    // Ex overlay textures
    pub(crate) tap_ex_tex: Option<Texture2D>,
    pub(crate) hold_ex_tex: Option<Texture2D>,
    pub(crate) star_ex_tex: Option<Texture2D>,
    pub(crate) star_double_ex_tex: Option<Texture2D>,
    pub(crate) mask_material: Option<Material>,
    pub(crate) pad_rect: Option<egui_macroquad::egui::Rect>,
    pub(crate) audio_cache: HashMap<i32, BgmPcm>,
    pub(crate) audio_seek_offset: Option<f32>,
    pub(crate) pending_audio_start: bool,
    pub(crate) audio_enabled: bool,

    pub(crate) pad_svg: Option<super::pad_svg::PadSvgDef>,

    // Low-latency SFX via rodio
    pub(crate) sfx_player: Option<SfxPlayer>,
    pub(crate) sfx_tap: Option<SfxBuffer>,
    pub(crate) sfx_touch: Option<SfxBuffer>,
    pub(crate) sfx_slide: Option<SfxBuffer>,
    pub(crate) sfx_touch_riser: Option<SfxBuffer>,
    pub(crate) sfx_break: Option<SfxBuffer>,
    pub(crate) sfx_break_tap: Option<SfxBuffer>,
    pub(crate) sfx_tap_ex: Option<SfxBuffer>,
    pub(crate) sfx_slide_break_start: Option<SfxBuffer>,
    pub(crate) sfx_break_slide: Option<SfxBuffer>,
    pub(crate) touch_riser_playing: bool,
    pub(crate) hit_sounds_played: HashSet<usize>,

    pub(crate) status: String,
}

impl AppState {
    pub(crate) fn new(
        chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
    ) -> Self {
        let mobile_ui = cfg!(any(target_os = "android", target_os = "ios"))
            || std::env::var("MAI2_MOBILE_UI").map(|v| v == "1").unwrap_or(false);

        let ui_scale_override = std::env::var("MAI2_UI_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v.clamp(0.7, 2.4));

        Self {
            mode: Mode::Idle,
            mode_wall_anchor: get_time(),
            mode_song_offset: 0.0,
            chart,
            recording_hits: Vec::new(),
            recording_notes: Vec::new(),
            active_record_holds: HashMap::new(),
            active_pointer_zones: HashMap::new(),
            prev_pointer_pos: HashMap::new(),
            pad_feedback: Vec::new(),
            playback_cursor: 0,
            selected_note: None,
            dragging_note: None,
            press_note_candidate: None,
            drag_part: None,
            drag_start_pos: None,
            drag_start_time: 0.0,
            drag_cursor_anchor_t: 0.0,
            drag_multi_orig: Vec::new(),
            box_start: None,
            box_end: None,
            box_anchor_t: None,
            place_tool: super::types::PlaceTool::Tap,
            placement: super::types::PlacementState::Idle,
            editing_slide_path: None,
            pending_slide_shape: None,
            waveform_data: Vec::new(),
            waveform_freq_bins: 0,
            waveform_time_res: 0.0,
            waveform_threshold: 0.3,
            record_snap_grid: true,
            selected_notes: Vec::new(),
            drag_orig_note: None,
            timeline_view_time: 0.0,
            timeline_zoom: 1.0,
            dragging_progress_bar: false,
            undo_stack: Vec::new(),
            clipboard: Vec::new(),
            pasting: false,
            editing_star: false,
            last_click_time: 0.0,
            last_click_note: None,
            record_speed: 1.0,
            play_speed: 1.0,
            touch_speed: 0.3,
            show_pad_only: false,
            mobile_ui,
            ui_scale_override,
            audio_source_name,
            audio_wav_pcm,
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
            pad_rect: None,
            audio_cache: HashMap::new(),
            audio_seek_offset: None,
            pending_audio_start: false,
            audio_enabled: true,
            pad_svg: None,
            sfx_player: None,
            sfx_tap: None,
            sfx_touch: None,
            sfx_slide: None,
            sfx_touch_riser: None,
            sfx_break: None,
            sfx_break_tap: None,
            sfx_tap_ex: None,
            sfx_slide_break_start: None,
            sfx_break_slide: None,
            touch_riser_playing: false,
            hit_sounds_played: HashSet::new(),
            status: "Ready".to_string(),
        }
    }

    // ── Setters with logging ──────────────────────────────────────────

    pub(crate) fn set_chart(&mut self, chart: ChartDoc) {
        let n = chart.notes.len();
        let slides: Vec<_> = chart.notes.iter().enumerate()
            .filter(|(_, n)| matches!(n.note_type, NoteType::Slide))
            .collect();
        println!("[AppState] set_chart: {n} notes, {} slides", slides.len());
        for (i, note) in &slides {
            println!("  slide #{i}: lane={} shape={:?} pts={:?} dur={:.3} delay={:.3} break={} ex={} tapless={} star={}",
                note.lane, note.slide_shape, note.slide_points, note.slide_duration, note.slide_start_delay,
                note.is_break, note.is_ex, note.is_tapless, note.is_star);
        }
        self.chart = chart;
    }

    pub(crate) fn set_selected_note(&mut self, sel: Option<usize>) {
        if self.selected_note != sel {
            println!("[AppState] selected_note: {:?} -> {:?}", self.selected_note, sel);
        }
        self.selected_note = sel;
    }

    pub(crate) fn set_editing_slide_path(&mut self, v: Option<usize>) {
        if self.editing_slide_path != v {
            println!("[AppState] editing_slide_path: {:?} -> {:?}", self.editing_slide_path, v);
        }
        self.editing_slide_path = v;
    }

    pub(crate) fn set_status(&mut self, msg: String) {
        println!("[AppState] status: {}", msg);
        self.status = msg;
    }

    pub(crate) fn current_speed(&self) -> f32 {
        match self.mode {
            Mode::Recording => self.record_speed,
            Mode::Playing => self.play_speed,
            Mode::Idle => 0.0,
        }
    }

    pub(crate) fn song_time(&self) -> f32 {
        let elapsed_wall = (get_time() - self.mode_wall_anchor) as f32;
        self.mode_song_offset + elapsed_wall * self.current_speed()
    }

    fn rebase_song_clock(&mut self) {
        self.mode_song_offset = self.song_time();
        self.mode_wall_anchor = get_time();
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.mode_wall_anchor = get_time();
        self.mode_song_offset = 0.0;
        if mode == Mode::Playing {
            self.playback_cursor = 0;
        }
    }

    pub(crate) fn set_record_speed(&mut self, new_speed: f32) {
        self.record_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
        if self.mode == Mode::Recording {
            // Anchor the song clock at the current time, then rebuild audio
            // from that exact offset at the new speed. Without setting
            // `audio_seek_offset` the rebuild would start from t=0 and
            // immediately desync from the chart cursor.
            self.rebase_song_clock();
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.request_audio_start();
        }
    }

    pub(crate) fn set_play_speed(&mut self, new_speed: f32) {
        self.play_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
        if self.mode == Mode::Playing {
            self.rebase_song_clock();
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.request_audio_start();
        }
    }

    // pub(crate) fn set_touch_speed(&mut self, new_speed: f32) {
    //     self.touch_speed = new_speed.clamp(TOUCH_SPEED_MIN, TOUCH_SPEED_MAX);
    // }

    pub(crate) fn stop_audio_if_any(&mut self) {
        if let Some(player) = &mut self.sfx_player {
            player.stop_bgm();
        }
    }

    pub(crate) fn request_audio_start(&mut self) {
        self.pending_audio_start = true;
    }

    pub(crate) fn seek_audio_to(&mut self, time: f32) {
        self.audio_seek_offset = Some(time);
        // Only kick off a fresh audio build/play when the user is actually
        // playing or recording. While Idle (paused), we just remember the
        // intended seek; resuming via `toggle_play` will use it. Triggering
        // audio in Idle would call `current_speed() == 0.0`, which clamps to
        // `SPEED_MIN = 0.1` inside the speed-shift builder, producing
        // 1/10x-speed audio — that's the "very slow music after pause" bug.
        if matches!(self.mode, Mode::Playing | Mode::Recording) {
            self.pending_audio_start = true;
        } else {
            // Stop any leftover audio so scrubbing while paused stays silent.
            self.stop_audio_if_any();
        }
    }

    pub(crate) fn push_undo(&mut self) {
        self.undo_stack.push(self.chart.clone());
        if self.undo_stack.len() > 64 { self.undo_stack.remove(0); }
    }

    pub(crate) fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.set_chart(prev);
            self.recompute_each();
            self.set_status("Undo".to_string());
        }
    }

    pub(crate) fn recompute_each(&mut self) {
        let len = self.chart.notes.len();
        for i in 0..len {
            let m = self.chart.notes[i].time;
            let has_sibling = self.chart.notes.iter().enumerate().any(|(j, n)| i != j && (n.time - m).abs() < 0.002);
            self.chart.notes[i].is_each = has_sibling;
        }
    }

    pub(crate) fn toggle_play(&mut self) {
        if self.mode == Mode::Playing {
            // Pause
            self.mode_song_offset = self.song_time();
            self.mode = Mode::Idle;
            self.mode_wall_anchor = get_time();
            self.stop_audio_if_any();
            if let Some(player) = &mut self.sfx_player { player.stop_looped(); }
            self.touch_riser_playing = false;
            self.timeline_view_time = self.mode_song_offset;
            self.set_status(format!("Paused at {:.2}s", self.mode_song_offset));
        } else {
            // Resume with audio seek
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.mode = Mode::Playing;
            self.mode_wall_anchor = get_time();
            self.hit_sounds_played.clear();
            self.playback_cursor = 0;
            self.request_audio_start();
            self.set_status(format!("Resumed @ {:.1}x from {:.2}s", self.play_speed, self.mode_song_offset));
        }
    }

    pub(crate) fn toggle_record(&mut self) {
        if self.mode == Mode::Recording {
            self.flush_active_record_holds();
            self.set_mode(Mode::Idle);
            self.stop_audio_if_any();
            self.recording_notes.sort_by(|a, b| a.time.total_cmp(&b.time));
            self.chart.notes = self.recording_notes.clone();
            self.set_status(format!(
                "Record stopped: {} notes @ {:.1}x",
                self.chart.notes.len(),
                self.record_speed
            ));
        } else {
            self.recording_hits.clear();
            self.recording_notes.clear();
            self.active_record_holds.clear();
            self.active_pointer_zones.clear();
            self.prev_pointer_pos.clear();
            self.set_mode(Mode::Recording);
            self.set_status(format!("Recording started @ {:.1}x", self.record_speed));
            self.request_audio_start();
        }
    }

    pub(crate) fn update_playback(&mut self) {
        if self.mode != Mode::Playing {
            return;
        }
        let t = self.song_time();
        let bpm = self.chart.bpm;

        while self.playback_cursor < self.chart.notes.len() {
            if note_secs(&self.chart.notes[self.playback_cursor], bpm) + HIT_WINDOW < t {
                self.playback_cursor += 1;
            } else {
                break;
            }
        }

        if let Some(last) = self.chart.notes.last() {
            if t > note_secs(last, bpm) + 1.2 {
                self.set_mode(Mode::Idle);
                self.stop_audio_if_any();
                self.set_status("Playback finished".to_string());
            }
        }
    }

    pub(crate) fn tick_feedback(&mut self) {
        let now = get_time();
        self.pad_feedback.retain(|f| f.until > now);
    }

    /// Play hit sound for notes that just reached their hit point or tail end.
    ///
    /// Instead of issuing one `play_sound` per note (which overwhelms the
    /// audio mixer on dense clusters), we count how many tap-type and
    /// touch-type events fire this frame and play each sound type **once**
    /// with volume scaled by the cluster size (capped at 1.0).
    pub(crate) fn service_hit_sounds(&mut self) {
        if self.mode != Mode::Playing {
            return;
        }
        let t = self.song_time();
        let bpm = self.chart.bpm;

        // --- count pending hit-sound events per type ---
        let mut tap_count: u32 = 0;
        let mut touch_count: u32 = 0;
        let mut slide_count: u32 = 0;
        let mut break_tap_count: u32 = 0;
        let mut ex_tap_count: u32 = 0;
        let mut slide_break_start_count: u32 = 0;
        let mut slide_break_end_count: u32 = 0;

        for (i, note) in self.chart.notes.iter().enumerate() {
            let ns = note_secs(note, bpm);
            // Head hit — touch uses TOUCH_DISAPPEAR_TIME to align with visual
            let hit_time = if matches!(note.note_type, NoteType::Touch) {
                ns + TOUCH_DISAPPEAR_TIME
            } else {
                ns
            };
            if !self.hit_sounds_played.contains(&i) && hit_time <= t {
                if matches!(note.note_type, NoteType::Touch) {
                    touch_count += 1;
                } else if note.is_break {
                    // Break tap/star/hold head: play break_tap + break simultaneously
                    break_tap_count += 1;
                } else if note.is_ex {
                    ex_tap_count += 1;
                } else if !matches!(note.note_type, NoteType::Slide) {
                    // Normal tap/hold head (not slide — slide star head gets tap sound below)
                    tap_count += 1;
                }
                // Slide (non-break, non-ex) star head also gets a tap sound
                if matches!(note.note_type, NoteType::Slide) && !note.is_break && !note.is_ex {
                    tap_count += 1;
                }
                self.hit_sounds_played.insert(i);
            }
            // Slide start (when the star begins moving)
            if matches!(note.note_type, NoteType::Slide) {
                let slide_key = i + self.chart.notes.len() * 3;
                let slide_move_time = ns + mdur_to_secs(note.slide_start_delay, bpm);
                if !self.hit_sounds_played.contains(&slide_key) && slide_move_time <= t {
                    if note.is_break {
                        slide_break_start_count += 1;
                    } else {
                        slide_count += 1;
                    }
                    self.hit_sounds_played.insert(slide_key);
                }
                // Slide end sound (break slides only)
                let slide_end_key = i + self.chart.notes.len() * 4;
                let slide_end_t = ns + mdur_to_secs(note.slide_duration, bpm);
                if note.is_break
                    && !self.hit_sounds_played.contains(&slide_end_key)
                    && slide_end_t <= t
                {
                    slide_break_end_count += 1;
                    self.hit_sounds_played.insert(slide_end_key);
                }
            }
            // Hold tail
            let tail_key = i + self.chart.notes.len();
            if matches!(note.note_type, NoteType::Hold)
                && !self.hit_sounds_played.contains(&tail_key)
                && hold_tail_time(note, bpm) <= t
            {
                tap_count += 1;
                self.hit_sounds_played.insert(tail_key);
            }
        }

        // --- play one sound per type via rodio, volume scales with cluster size ---
        let player = match &mut self.sfx_player {
            Some(p) => p,
            None => return,
        };

        if tap_count > 0 {
            if let Some(buf) = &self.sfx_tap {
                let vol = (0.6 + 0.1 * tap_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }
        if break_tap_count > 0 {
            if let Some(buf) = &self.sfx_break_tap {
                let vol = (0.6 + 0.1 * break_tap_count as f32).min(1.0);
                player.play(buf, vol);
            }
            if let Some(buf) = &self.sfx_break {
                let vol = (0.6 + 0.1 * break_tap_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }
        if ex_tap_count > 0 {
            if let Some(buf) = &self.sfx_tap_ex {
                let vol = (0.6 + 0.1 * ex_tap_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }
        if touch_count > 0 {
            if let Some(buf) = &self.sfx_touch {
                let vol = (0.6 + 0.1 * touch_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }
        if slide_count > 0 {
            if let Some(buf) = &self.sfx_slide {
                let vol = (0.6 + 0.1 * slide_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }
        if slide_break_start_count > 0 {
            if let Some(buf) = &self.sfx_slide_break_start {
                let vol = (0.6 + 0.1 * slide_break_start_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }
        if slide_break_end_count > 0 {
            if let Some(buf) = &self.sfx_break_slide {
                let vol = (0.6 + 0.1 * slide_break_end_count as f32).min(1.0);
                player.play(buf, vol);
            }
        }

        // Touch hold riser: play while any touch hold is active
        let active_th = self.chart.notes.iter()
            .filter(|n| matches!(n.note_type, NoteType::Hold)
                && is_touch_zone(sanitize_note_zone(n.note_type, n.lane))
                && note_secs(n, bpm) <= t && hold_tail_time(n, bpm) > t)
            .count();
        if active_th > 0 && !self.touch_riser_playing {
            if let Some(buf) = &self.sfx_touch_riser {
                player.play_looped(buf, 0.5);
                self.touch_riser_playing = true;
            }
        } else if active_th == 0 && self.touch_riser_playing {
            player.stop_looped();
            self.touch_riser_playing = false;
        }
    }

    pub(crate) fn push_feedback(&mut self, zone: u8, duration: f64) {
        self.pad_feedback.push(PadFeedback {
            zone,
            until: get_time() + duration,
        });
    }

    pub(crate) fn start_record_hold_input(&mut self, input_id: RecordInputId, lane: u8) {
        let start_time = self.song_time();
        self.active_record_holds
            .entry(input_id)
            .or_insert(ActiveRecordHold { lane, start_time, slide_zones: vec![(lane, 0.0)] });
    }

    pub(crate) fn finish_record_hold_input(&mut self, input_id: RecordInputId) {
        let Some(active) = self.active_record_holds.remove(&input_id) else {
            return;
        };
        self.push_recorded_note(active, self.song_time());
    }

    pub(crate) fn flush_active_record_holds(&mut self) {
        if self.active_record_holds.is_empty() {
            return;
        }
        let end_time = self.song_time();
        let active: Vec<ActiveRecordHold> = self.active_record_holds.drain().map(|(_, v)| v).collect();
        for item in active {
            self.push_recorded_note(item, end_time);
        }
    }

    pub(crate) fn record_slide_zone(&mut self, input_id: RecordInputId, zone: u8) {
        let t = self.song_time();
        if let Some(active) = self.active_record_holds.get_mut(&input_id) {
            let last_zone = active.slide_zones.last().map(|z| z.0).unwrap_or(active.lane);
            if zone != last_zone {
                active.slide_zones.push((zone, t - active.start_time));
            }
        }
    }

    fn push_recorded_note(&mut self, active: ActiveRecordHold, end_time: f32) {
        let bpm = self.chart.bpm;
        let duration_secs = (end_time - active.start_time).max(0.0);
        // Snap start time: convert seconds → measure, snap to 1/384 grid
        let start_measure = if self.record_snap_grid {
            snap_measure(secs_to_measure(active.start_time, bpm))
        } else {
            secs_to_measure(active.start_time, bpm)
        };
        let dur_measure = sdur_to_mdur(duration_secs, bpm);
        // Unique zones visited
        let mut visited: Vec<u8> = Vec::new();
        for (z, _) in &active.slide_zones {
            if visited.last() != Some(z) {
                visited.push(*z);
            }
        }
        let note_type = if visited.len() >= SLIDE_MIN_POINTS && is_touch_zone(active.lane) {
            NoteType::Slide
        } else if is_touch_zone(active.lane) {
            if duration_secs >= HOLD_RECORD_MIN_DURATION {
                NoteType::Hold
            } else {
                NoteType::Touch
            }
        } else if duration_secs >= HOLD_RECORD_MIN_DURATION {
            NoteType::Hold
        } else {
            NoteType::Tap
        };

        let slide_points: Vec<super::types::SlidePoint> = active.slide_zones.iter()
            .map(|(z, off)| super::types::SlidePoint { zone: *z, beat_offset: *off })
            .collect();

        let slide_dur = if matches!(note_type, NoteType::Slide) { dur_measure } else { 0.0 };
        let default_delay = sdur_to_mdur(0.12, bpm);

        // Phase 4: classify the recorded trajectory against known shape templates.
        let slide_shape = if matches!(note_type, NoteType::Slide) {
            super::slide_match::match_slide_shape(active.lane, &slide_points)
        } else {
            None
        };

        self.chart.notes.push(Note {
            time: start_measure, lane: active.lane, note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) { dur_measure } else { 0.0 },
            is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false,
            star_is_break: false, star_is_ex: false,
            slide_points: slide_points.clone(),
            slide_duration: slide_dur, slide_start_delay: default_delay, slide_shape,
        });
        self.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.recompute_each();

        self.recording_notes.push(Note {
            time: start_measure, lane: active.lane, note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) { dur_measure } else { 0.0 },
            is_each: false, is_break: false, is_ex: false, is_star: false, is_tapless: false,
            star_is_break: false, star_is_ex: false,
            slide_points,
            slide_duration: slide_dur, slide_start_delay: default_delay, slide_shape,
        });
        self.recording_hits.push(HitEvent {
            time: active.start_time,
            lane: active.lane,
        });
    }
}
