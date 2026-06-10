use macroquad::material::Material;
use macroquad::prelude::{get_time, Vec2};
use macroquad::texture::Texture2D;
use std::collections::{HashMap, HashSet};
use lambda_dx::app::types::zone::PadZone;
use lnmai_core_rs::lnmai_core_ffi;
use lnmai_core_ffi::session::{self, Session, Empty, Loaded};
use lnmai_core_ffi::types::{TimedInputEvent, JudgeEvent};
use serde_json::json;
use super::audio::BgmPcm;
use super::sfx::{SfxBuffer, SfxPlayer};

use super::types::{
    ActiveRecordHold, ChartDoc, HitEvent, Mode, Note, NoteType, PadFeedback, RecordInputId, SlidePoint,
    WavPcm, DragPart, HIT_WINDOW, HOLD_RECORD_MIN_DURATION, SPEED_MAX, SPEED_MIN,
    TOUCH_DISAPPEAR_TIME, SLIDE_MIN_POINTS, hold_tail_time, is_touch_zone, sanitize_note_zone,
    note_secs, secs_to_measure, mdur_to_secs, sdur_to_mdur, snap_measure,
};

#[derive(Debug, Clone)]
pub struct JudgeText {
    pub zone: PadZone,
    pub grade: String,
    pub until: f64,
}

// #[derive(Debug, serde::Deserialize)]
// #[serde(rename_all = "camelCase")]
// struct JudgeEvent {
//     kind: String,
//     grade: String,
//     note_index: u64,
// }

// #[derive(Debug, serde::Deserialize)]
// struct RuntimeStepLightResult {
//     events: Vec<JudgeEvent>,
// }
//
// #[derive(Debug, serde::Deserialize)]
// struct FfiResult {
//     ok: bool,
//     result: Option<RuntimeStepLightResult>,
// }

/// Runtime mutable state for the editor/simulator.
pub struct PlayerState {
    pub mode: Mode,
    pub mode_wall_anchor: f64,
    pub mode_song_offset: f32,

    pub chart: ChartDoc,
    pub recording_hits: Vec<HitEvent>,
    pub recording_notes: Vec<Note>,
    pub active_record_holds: HashMap<RecordInputId, ActiveRecordHold>,
    pub active_pointer_zones: HashMap<u64, PadZone>,
    pub active_sensor_holds: HashMap<u64, PadZone>,
    pub prev_pointer_pos: HashMap<u64, Vec2>,
    pub pad_feedback: Vec<PadFeedback>,
    pub playback_cursor: usize,
    pub selected_note: Option<usize>,
    pub dragging_note: Option<usize>,
    /// Note index detected under the mouse on press; selection is deferred
    /// until a drag threshold is exceeded.
    pub press_note_candidate: Option<usize>,
    pub drag_part: Option<DragPart>,
    pub drag_start_pos: Option<Vec2>,
    pub drag_start_time: f32,
    pub drag_shift: bool,
    /// Cursor's chart time at the moment of click. Used so dragging tracks the
    /// mouse's absolute position even if the user scrolls the timeline mid-drag.
    pub drag_cursor_anchor_t: f32,
    pub drag_multi_orig: Vec<(usize, f32, u8)>,
    /// For slide tail/delay dragging, which sub-slide in `note.slide` is active.
    pub drag_slide_idx: Option<usize>,
    pub box_start: Option<Vec2>,
    pub box_end: Option<Vec2>,
    /// Chart time anchored for the box-select start, so the start point sticks
    /// to the timeline as the user scrolls during a selection drag.
    pub box_anchor_t: Option<f32>,
    /// Currently selected sidebar tool (Tap / Hold / Star).
    pub place_tool: super::types::PlaceTool,
    /// Multi-step placement state machine (for Hold and Star tools).
    pub placement: super::types::PlacementState,
    /// When `Some(i)`, the user is editing the trajectory of
    /// chart.notes[i] by clicking zones on the Pad. Only meaningful when the
    /// note is a Slide and the app is in Idle mode.
    pub editing_slide_path: Option<usize>,
    /// Which sub-slide in `chart.notes[editing_slide_path].slide` is being edited.
    pub editing_slide_idx: Option<usize>,
    /// Pending slide shape key (e.g. Q, P, S, Z) waiting for a lane number to complete.
    pub pending_slide_shape: Option<super::types::SlideShape>,
    pub waveform_data: Vec<f32>,
    pub waveform_freq_bins: u32,
    pub waveform_time_res: f32,
    pub waveform_max_val: f32,
    pub waveform_threshold: f32,
    pub record_snap_grid: bool,
    pub selected_notes: Vec<usize>,
    pub selected_note_ids: HashSet<u64>,
    pub drag_orig_note: Option<super::types::Note>,
    pub timeline_view_time: f32,
    pub timeline_zoom: f32,
    pub dragging_progress_bar: bool,
    pub undo_stack: Vec<super::types::ChartDoc>,
    pub clipboard: Vec<super::types::Note>,
    pub pasting: bool,
    /// When true, B/X hotkeys modify star head flags instead of slide trail.
    pub editing_star: bool,
    /// Timestamp of last left-click for double-click detection.
    pub last_click_time: f64,
    pub last_click_note: Option<usize>,

    pub record_speed: f32,
    pub play_speed: f32,
    pub touch_speed: f32,

    pub show_pad_only: bool,
    pub mobile_ui: bool,
    pub ui_scale_override: Option<f32>,

    pub audio_source_name: Option<String>,
    pub audio_wav_pcm: Option<WavPcm>,
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
    // Break textures
    pub tap_break_tex: Option<Texture2D>,
    pub hold_break_tex: Option<Texture2D>,
    pub slide_break_tex: Option<Texture2D>,
    pub star_double_break_tex: Option<Texture2D>,
    // Ex overlay textures
    pub tap_ex_tex: Option<Texture2D>,
    pub hold_ex_tex: Option<Texture2D>,
    pub star_ex_tex: Option<Texture2D>,
    pub star_double_ex_tex: Option<Texture2D>,
    pub mask_material: Option<Material>,
    pub pad_rect: Option<egui_macroquad::egui::Rect>,
    pub audio_cache: HashMap<i32, BgmPcm>,
    pub audio_seek_offset: Option<f32>,
    pub pending_audio_start: bool,
    pub audio_enabled: bool,

    pub pad_svg: Option<super::pad_svg::PadSvgDef>,

    // Low-latency SFX via rodio
    pub sfx_player: Option<SfxPlayer>,
    pub sfx_tap: Option<SfxBuffer>,
    pub sfx_touch: Option<SfxBuffer>,
    pub sfx_slide: Option<SfxBuffer>,
    pub sfx_touch_riser: Option<SfxBuffer>,
    pub sfx_break: Option<SfxBuffer>,
    pub sfx_break_tap: Option<SfxBuffer>,
    pub sfx_tap_ex: Option<SfxBuffer>,
    pub sfx_slide_break_start: Option<SfxBuffer>,
    pub sfx_break_slide: Option<SfxBuffer>,
    pub touch_riser_playing: bool,
    pub hit_sounds_played: HashSet<usize>,
    pub next_note_id: u64,
    pub hidden_notes: HashSet<u64>,

    pub status: String,

    /// File path input for chart import.
    pub import_path_input: String,
    /// Pending file dialog import (triggered from main loop).
    pub pending_import: bool,

    /// Imported Simai file for level switching.
    pub imported_simai: Option<maisimai::SimaiFile>,
    /// Available levels: (number, display_text).
    pub import_levels: Vec<(u32, String)>,
    /// Currently selected import level.
    pub import_selected_level: u32,

    pub lnmai_session: Option<Session<Loaded>>,
    pub lnmai_initialized: bool,
    pub lnmai_note_zones: Vec<PadZone>,
    pub judge_texts: Vec<JudgeText>,
    pub lnmai_input_events: Vec<(u64, serde_json::Value)>,

    pub slide_progress: HashMap<u64, SlideProgress>,

    // Autoplay
    pub autoplay: bool,
    pub autoplay_events: Vec<(u64, serde_json::Value)>,
    pub autoplay_index: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SlideProgress {
    pub remaining: u64,
    pub hidden_bars: Vec<u64>,
}

impl PlayerState {
    pub fn new(
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
            active_sensor_holds: HashMap::new(),
            prev_pointer_pos: HashMap::new(),
            pad_feedback: Vec::new(),
            playback_cursor: 0,
            selected_note: None,
            dragging_note: None,
            press_note_candidate: None,
            drag_part: None,
            drag_start_pos: None,
            drag_start_time: 0.0,
            drag_shift: false,
            drag_cursor_anchor_t: 0.0,
            drag_multi_orig: Vec::new(),
            drag_slide_idx: None,
            box_start: None,
            box_end: None,
            box_anchor_t: None,
            place_tool: super::types::PlaceTool::Tap,
            placement: super::types::PlacementState::Idle,
            editing_slide_path: None,
            editing_slide_idx: None,
            pending_slide_shape: None,
            waveform_data: Vec::new(),
            waveform_freq_bins: 0,
            waveform_time_res: 0.0,
            waveform_max_val: 0.0,
            waveform_threshold: 0.3,
            record_snap_grid: true,
            selected_notes: Vec::new(),
            selected_note_ids: HashSet::new(),
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
            wifi_tex: [None, None, None, None, None, None, None, None, None, None, None],
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
            next_note_id: 1,
            hidden_notes: HashSet::new(),
            status: "Ready".to_string(),
            import_path_input: String::new(),
            pending_import: false,
            imported_simai: None,
            import_levels: Vec::new(),
            import_selected_level: 0,
            lnmai_session: None,
            lnmai_initialized: false,
            lnmai_note_zones: Vec::new(),
            judge_texts: Vec::new(),
            lnmai_input_events: Vec::new(),
            slide_progress: HashMap::new(),
            autoplay: false,
            autoplay_events: Vec::new(),
            autoplay_index: 0,
        }
    }

    // ── Setters with logging ──────────────────────────────────────────

    pub fn set_chart(&mut self, mut chart: ChartDoc) {
        for note in &mut chart.notes { if note.id == 0 { note.id = self.next_id(); } }
        let n = chart.notes.len();
        let slides: Vec<_> = chart.notes.iter().enumerate()
            .filter(|(_, n)| matches!(n.note_type, NoteType::Slide))
            .collect();
        println!("[AppState] set_chart: {n} notes, {} slides", slides.len());
        for (i, note) in &slides {
            println!("  slide #{i}: lane={} slides={} tapless={} star={}",
                note.lane, note.slide.len(), note.is_tapless, note.is_star);
            for (si, sl) in note.slide.iter().enumerate() {
                let shapes: Vec<_> = sl.segments.iter().map(|seg| format!("{:?}", seg.shape)).collect();
                let pt_count: usize = sl.segments.iter().map(|seg| seg.points.len()).sum();
                println!("    slide[{si}]: shapes=[{}] pts={pt_count} dur={:.3} delay={:.3} break={}",
                    shapes.join(","), sl.slide_duration, sl.slide_start_delay, sl.slide_is_break);
            }
        }
        self.chart = chart;
    }

    pub fn set_selected_note(&mut self, sel: Option<usize>) {
        if self.selected_note != sel {
            println!("[AppState] selected_note: {:?} -> {:?}", self.selected_note, sel);
        }
        self.selected_note = sel;
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_note_id;
        self.next_note_id += 1;
        id
    }
    pub fn push_note(&mut self, mut note: Note) {
        if note.id == 0 { note.id = self.next_id(); }
        self.chart.notes.push(note);
    }

    pub fn unhide_all_notes(&mut self) {
        self.hidden_notes.clear();
        self.selected_note = None;
        self.selected_notes.clear();
        self.selected_note_ids.clear();
        self.set_status("Unhid all notes".to_string());
    }

    pub fn set_editing_slide_path(&mut self, v: Option<usize>) {
        if self.editing_slide_path != v {
            println!("[AppState] editing_slide_path: {:?} -> {:?}", self.editing_slide_path, v);
        }
        self.editing_slide_path = v;
        if v.is_none() {
            self.editing_slide_idx = None;
        }
    }

    pub fn set_status(&mut self, msg: String) {
        println!("[AppState] status: {}", msg);
        self.status = msg;
    }

    pub fn current_speed(&self) -> f32 {
        match self.mode {
            Mode::Recording => self.record_speed,
            Mode::Playing => self.play_speed,
            Mode::Idle => 0.0,
        }
    }

    pub fn song_time(&self) -> f32 {
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

    pub fn set_record_speed(&mut self, new_speed: f32) {
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

    pub fn set_play_speed(&mut self, new_speed: f32) {
        self.play_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
        if self.mode == Mode::Playing {
            self.rebase_song_clock();
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.request_audio_start();
        }
    }

    // pub fn set_touch_speed(&mut self, new_speed: f32) {
    //     self.touch_speed = new_speed.clamp(TOUCH_SPEED_MIN, TOUCH_SPEED_MAX);
    // }

    pub fn stop_audio_if_any(&mut self) {
        if let Some(player) = &mut self.sfx_player {
            player.stop_bgm();
        }
    }

    pub fn request_audio_start(&mut self) {
        self.pending_audio_start = true;
    }

    pub fn seek_audio_to(&mut self, time: f32) {
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

    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.chart.clone());
        if self.undo_stack.len() > 64 { self.undo_stack.remove(0); }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.set_chart(prev);
            self.recompute_each();
            self.set_status("Undo".to_string());
        }
    }

    pub fn recompute_each(&mut self) {
        let len = self.chart.notes.len();
        for i in 0..len {
            let m = self.chart.notes[i].time;
            let has_sibling = self.chart.notes.iter().enumerate().any(|(j, n)| i != j && (n.time - m).abs() < 0.002);
            self.chart.notes[i].is_each = has_sibling;
        }
    }

    pub fn toggle_play(&mut self) {
        if self.mode == Mode::Playing {
            // Pause
            self.mode_song_offset = self.song_time();
            self.mode = Mode::Idle;
            self.mode_wall_anchor = get_time();
            self.stop_audio_if_any();
            if let Some(player) = &mut self.sfx_player { player.stop_looped(); }
            self.touch_riser_playing = false;
            self.timeline_view_time = self.mode_song_offset;
            self.active_sensor_holds.clear();
            self.free_lnmai_session();
            self.set_status(format!("Paused at {:.2}s", self.mode_song_offset));
        } else {
            // Resume with audio seek
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.mode = Mode::Playing;
            self.mode_wall_anchor = get_time();
            self.hit_sounds_played.clear();
            self.playback_cursor = 0;
            self.judge_texts.clear();
            if self.autoplay {
                self.generate_autoplay_events();
            }
            self.create_lnmai_session();
            self.request_audio_start();
            self.set_status(format!("Resumed @ {:.1}x from {:.2}s", self.play_speed, self.mode_song_offset));
        }
    }
    pub fn toggle_replay(&mut self) {
        self.mode = Mode::Playing;
        self.timeline_view_time =0.;
        self.mode_song_offset = 0.;
        self.audio_seek_offset=Some(0.);
        self.hit_sounds_played.clear();
        self.recording_hits.clear();
        self.recording_notes.clear();
        self.active_record_holds.clear();
        self.active_pointer_zones.clear();
        self.active_sensor_holds.clear();
        self.prev_pointer_pos.clear();
        self.judge_texts.clear();
        if self.autoplay {
            self.generate_autoplay_events();
        }
        self.create_lnmai_session();
        self.request_audio_start();
    }

    pub fn toggle_record(&mut self) {
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
            self.active_sensor_holds.clear();
            self.prev_pointer_pos.clear();
            self.set_mode(Mode::Recording);
            self.set_status(format!("Recording started @ {:.1}x", self.record_speed));
            self.request_audio_start();
        }
    }

    pub fn update_playback(&mut self) {
        // Judgment is now handled by lnmai-core-rs-ffi via advance_lnmai_frame().
        // Keep this as a no-op for backward compat in the main loop.
    }

    pub fn tick_feedback(&mut self) {
        let now = get_time();
        self.pad_feedback.retain(|f| f.until > now);
    }

    /// Playback sounds are now handled by lnmai-core-rs-ffi judgment.
    /// This is kept as a no-op for backward compatibility in the main loop.
    pub fn service_hit_sounds(&mut self) {}

    pub fn init_lnmai(&mut self) {
        if self.lnmai_initialized { return; }
        match unsafe { session::initialize_runtime() } {
            Ok(()) => {
                self.lnmai_initialized = true;
                self.set_status("lnmai runtime initialized".to_string());
            }
            Err(()) => {
                self.set_status("lnmai runtime init failed".to_string());
            }
        }
    }

    pub fn create_lnmai_session(&mut self) {
        if !self.lnmai_initialized { return; }
        self.free_lnmai_session();
        let empty: Session<Empty> = match Session::<Empty>::create() {
            Ok(s) => s,
            Err(e) => { self.set_status(format!("lnmai create session failed: {}", e.json)); return; }
        };
        let simai_file = lambda_dx::simai_io::chart_doc_to_simai_file(&self.chart);
        let simai_text = maisimai::export_file(&simai_file);
        println!("[lnmai simai]\n{simai_text}");

        let level = if self.chart.simai_level > 0 { self.chart.simai_level } else { 6 };
        let (loaded, _info): (Session<Loaded>, _) = match empty.load_chart_text(&simai_text, level) {
            Ok(v) => v,
            Err(e) => { self.set_status(format!("lnmai load chart failed: {}", e.json)); return; }
        };

        self.lnmai_note_zones.clear();
        if let Ok(envelope) = loaded.get_lowered_chart_json() {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&envelope.json) {
                let data = json_val.get("result").unwrap_or(&json_val);
                let mut note_entries: Vec<(u64, PadZone)> = Vec::new();
                for key in &["taps", "holds", "touches", "touchHolds", "slideHeads", "slides"] {
                    if let Some(arr) = data.get(*key).and_then(|n| n.as_array()) {
                        for note in arr {
                            let note_index = note.get("noteIndex").and_then(|v| v.as_u64()).unwrap_or(0);
                            let zone = if let Some(slot) = note.get("slot").and_then(|v| v.as_str()) {
                                if let Some(lane) = slot.strip_prefix('S').and_then(|s| s.parse::<u8>().ok()) {
                                    PadZone::from(lane)
                                } else {
                                    PadZone::from(slot)
                                }
                            } else if let Some(sensor) = note.get("sensorPos").and_then(|v| v.as_str()) {
                                PadZone::from(sensor)
                            } else {
                                PadZone::A1
                            };
                            note_entries.push((note_index, zone));
                        }
                    }
                }
                note_entries.sort_by_key(|(idx, _)| *idx);
                let max_idx = note_entries.last().map(|(i, _)| *i).unwrap_or(0) as usize;
                self.lnmai_note_zones = vec![PadZone::A1; max_idx + 1];
                for (idx, zone) in note_entries {
                    self.lnmai_note_zones[idx as usize] = zone;
                }
                let count = self.lnmai_note_zones.len();
                println!("[lnmai lowered] zone_map len={count}");
            }
        }

        self.lnmai_session = Some(loaded);
        self.set_status(format!("lnmai session loaded ({} lowered notes)", self.lnmai_note_zones.len()));
    }

    pub fn free_lnmai_session(&mut self) {
        let maybe_session: Option<Session<Loaded>> = self.lnmai_session.take();
        if let Some(loaded_session) = maybe_session {
            match loaded_session.free() {
                Err(e) => self.set_status(format!("lnmai free session failed: {}", e.json)),
                _ => {}
            }
        }
    }

    pub fn advance_lnmai_frame(&mut self) {
        if self.mode != Mode::Playing { return; }
        let t_s = self.song_time() as f64;
        let t_us = (t_s * 1_000_000.0) as u64;
        let events: Vec<serde_json::Value> = self.lnmai_input_events.drain(..).map(|(_, v)| v).collect();

        let session: &mut Session<Loaded> = match &mut self.lnmai_session {
            Some(s) => s,
            None => return,
        };

        let batch = json!({
            "currentTime": t_us,
            "events": events
        });
        if !events.is_empty() {
           println!("[lnmai input] currentTime={} events={}", t_us, serde_json::to_string(&batch["events"]).unwrap_or_default());
        }

        match session.advance_frame_light(&batch.to_string()) {
            Ok(envelope) => {
                match serde_json::from_str::<serde_json::Value>(&envelope.json) {
                    Ok(json_val) => {
                        if let Some(result) = json_val.get("result") {
                            let render_commands = result.get("renderCommands")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let audio_commands = result.get("audioCommands")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let judge_events: Vec<JudgeEvent> = result.get("events")
                                .and_then(|v| serde_json::from_value(v.clone()).ok())
                                .unwrap_or_default();
                            
                            self.process_render_commands(&render_commands);
                            self.process_audio_commands(&audio_commands);
                            self.process_judge_events(&judge_events);
                        }
                    }
                    Err(e) => {
                        self.set_status(format!("lnmai parse json failed [{}]", e));
                    }
                }
            }
            Err(e) => {
                self.set_status(format!("lnmai advance frame error: {}", e.json));
            }
        }
    }

    fn process_render_commands(&mut self, commands: &[serde_json::Value]) {
        for cmd in commands {
            if let Some(obj) = cmd.as_object() {
                if let Some(kind) = obj.keys().next() {
                    match kind.as_str() {
                        "HideSlideBars" => {
                            if let Some(data) = obj.get(kind) {
                                let note_index = data.get("noteIndex").and_then(|v| v.as_u64()).unwrap_or(0);
                                let end_index = data.get("endIndex").and_then(|v| v.as_u64()).unwrap_or(0);
                                let entry = self.slide_progress.entry(note_index).or_default();
                                entry.hidden_bars.push(end_index);
                            }
                        }
                        "UpdateSlideProgress" => {
                            if let Some(data) = obj.get(kind) {
                                let note_index = data.get("noteIndex").and_then(|v| v.as_u64()).unwrap_or(0);
                                let remaining = data.get("remaining").and_then(|v| v.as_u64()).unwrap_or(0);
                                println!("UpdateSlideProgress- note_index: {note_index}, remaining: {remaining}");
                                let entry = self.slide_progress.entry(note_index).or_default();
                                entry.remaining = remaining;
                            }
                        }
                        "HideAllSlideBars" => {
                            if let Some(data) = obj.get(kind) {
                                let note_index = data.get("noteIndex").and_then(|v| v.as_u64()).unwrap_or(0);
                                self.slide_progress.remove(&note_index);
                            }
                        }
                        "ShowJudgeResult" => {
                            if let Some(data) = obj.get(kind) {
                                let note_index = data.get("noteIndex").and_then(|v| v.as_u64()).unwrap_or(0);
                                let grade = data.get("grade").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                let zone = self.lnmai_note_zones
                                    .get(note_index as usize)
                                    .copied()
                                    .unwrap_or(PadZone::A1);
                                self.judge_texts.push(JudgeText {
                                    zone,
                                    grade: grade.to_string(),
                                    until: get_time() + 0.6,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn process_audio_commands(&mut self, commands: &[serde_json::Value]) {
        println!("[lnmai audio commands] {}", serde_json::to_string(&commands).unwrap_or_default());
        for cmd in commands {
            if let Some(obj) = cmd.as_object() {
                if let Some(kind) = obj.keys().next() {
                    match kind.as_str() {
                        "PlayJudgeSfx" => {
                            let data = obj.get(kind);
                            let note_kind = data.and_then(|d| d.get("kind")).and_then(|v| v.as_str()).unwrap_or("");
                            let is_break = note_kind == "Break";
                            let is_tap = note_kind == "Tap" || note_kind == "Hold";
                            let is_touch = note_kind == "Touch";
                            if is_break {
                                if let Some(sfx) = &self.sfx_break {
                                    if let Some(player) = &mut self.sfx_player {
                                        player.play(sfx, 1.);
                                    }
                                }
                            } else if is_tap {
                                if let Some(sfx) = &self.sfx_tap {
                                    if let Some(player) = &mut self.sfx_player {
                                        player.play(sfx, 1.);
                                    }
                                }
                            } else if is_touch {
                                if let Some(sfx) = &self.sfx_touch {
                                    if let Some(player) = &mut self.sfx_player {
                                        player.play(sfx, 1.);
                                    }
                                }
                            } else {
                                // Slide judge sfx
                                if let Some(sfx) = &self.sfx_slide {
                                    if let Some(player) = &mut self.sfx_player {
                                        player.play(sfx, 1.);
                                    }
                                }
                            }
                        }
                        "PlaySlideCue" => {
                            if let Some(sfx) = &self.sfx_slide {
                                if let Some(player) = &mut self.sfx_player {
                                    player.play(sfx, 1.);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn process_judge_events(&mut self, events: &[JudgeEvent]) {
        for e in events {
            let zone = self.lnmai_note_zones
                .get(e.note_index as usize)
                .copied()
                .unwrap_or(PadZone::A1);
            println!("[lnmai judge event] note_index={zone}");
            self.judge_texts.push(JudgeText {
                zone,
                grade: format!("{:?}", e.grade),
                until: get_time() + 0.6,
            });
        }
    }

    pub fn tick_judge_texts(&mut self) {
        let now = get_time();
        self.judge_texts.retain(|jt| jt.until > now);
    }

    pub fn push_feedback(&mut self, zone: PadZone, duration: f64) {
        self.pad_feedback.push(PadFeedback {
            zone,
            until: get_time() + duration,
        });
    }

    pub fn start_record_hold_input(&mut self, input_id: RecordInputId, lane: PadZone) {
        let start_time = self.song_time();
        self.active_record_holds
            .entry(input_id)
            .or_insert(ActiveRecordHold { lane: lane.to_id(), start_time, slide_zones: vec![SlidePoint { zone: lane, beat_offset: 0.0 }] });
    }

    pub fn finish_record_hold_input(&mut self, input_id: RecordInputId) {
        let Some(active) = self.active_record_holds.remove(&input_id) else {
            return;
        };
        self.push_recorded_note(active, self.song_time());
    }

    pub fn flush_active_record_holds(&mut self) {
        if self.active_record_holds.is_empty() {
            return;
        }
        let end_time = self.song_time();
        let active: Vec<ActiveRecordHold> = self.active_record_holds.drain().map(|(_, v)| v).collect();
        for item in active {
            self.push_recorded_note(item, end_time);
        }
    }

    pub fn record_slide_zone(&mut self, input_id: RecordInputId, zone: PadZone) {
        let t = self.song_time();
        if let Some(active) = self.active_record_holds.get_mut(&input_id) {
            let last_zone = active.slide_zones.last().map(|sp| sp.zone.to_id()).unwrap_or(active.lane);
            if zone != last_zone {
                active.slide_zones.push(SlidePoint { zone, beat_offset: t - active.start_time });
            }
        }
    }

    fn push_recorded_note(&mut self, active: ActiveRecordHold, end_time: f32) {
        let bpms = &self.chart.bpms;
        let duration_secs = (end_time - active.start_time).max(0.0);
        // Snap start time: convert seconds → measure, snap to 1/384 grid
        let start_measure = if self.record_snap_grid {
            snap_measure(secs_to_measure(active.start_time, bpms))
        } else {
            secs_to_measure(active.start_time, bpms)
        };
        let dur_measure = sdur_to_mdur(duration_secs, active.start_time, bpms);
        // Unique zones visited
        let mut visited: Vec<u8> = Vec::new();
        for sp in &active.slide_zones {
            let zid = sp.zone.to_id();
            if visited.last() != Some(&zid) {
                visited.push(zid);
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

        let slide_points = active.slide_zones.clone();

        let slide_dur = if matches!(note_type, NoteType::Slide) { dur_measure } else { 0.0 };
        let default_delay = sdur_to_mdur(0.12, active.start_time, bpms);

        // Phase 4: classify the recorded trajectory against known shape templates.
        let slide_shape = if matches!(note_type, NoteType::Slide) {
            lambda_dx::slide_match::match_slide_shape(active.lane, &slide_points)
        } else {
            None
        };

        let slide_vec = if matches!(note_type, NoteType::Slide) {
            vec![super::types::Slide {
                segments: vec![super::types::SlideSegment {
                    points: slide_points.clone(),
                    shape: slide_shape.unwrap_or(super::types::SlideShape::Line),
                }],
                slide_duration: slide_dur,
                slide_start_delay: default_delay,
                slide_is_break: false,
            }]
        } else {
            vec![]
        };

        self.chart.notes.push(Note {
            time: start_measure, lane: active.lane, note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) { dur_measure } else { 0.0 },
            slide: slide_vec.clone(),
            ..Default::default()
        });
        self.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.recompute_each();

        self.recording_notes.push(Note {
            time: start_measure, lane: active.lane, note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) { dur_measure } else { 0.0 },
            slide: slide_vec,
            ..Default::default()
        });
        self.recording_hits.push(HitEvent {
            time: active.start_time,
            lane: active.lane,
        });
    }

    pub fn generate_autoplay_events(&mut self) {
        let simai_file = lambda_dx::simai_io::chart_doc_to_simai_file(&self.chart);
        let simai_text = maisimai::export_file(&simai_file);
        let level = if self.chart.simai_level > 0 { self.chart.simai_level } else { 6 };
        match lnmai_core_ffi::api::parse_lowered_chart(&simai_text, level)
            .and_then(|chart_spec| lnmai_core_ffi::api::default_tactic_from_chart(&chart_spec)) {
            Ok(tactic) => {
                self.autoplay_events = tactic.events.iter().map(|e| {
                    let t_us = match e {
                        TimedInputEvent::ButtonClick { tp, .. } => *tp,
                        TimedInputEvent::ButtonHold { tp, .. } => *tp,
                        TimedInputEvent::SensorClick { tp, .. } => *tp,
                        TimedInputEvent::SensorHold { tp, .. } => *tp,
                    };
                    let json_val = match e {
                        TimedInputEvent::ButtonClick { tp, zone } => json!({"buttonClick": {"tp": t_us, "zone": zone}}),
                        TimedInputEvent::ButtonHold { tp, zone, is_down } => json!({"buttonHold": {"tp": t_us, "zone": zone, "isDown": is_down}}),
                        TimedInputEvent::SensorClick { tp, area } => json!({"sensorClick": {"tp": t_us, "area": area}}),
                        TimedInputEvent::SensorHold { tp, area, is_down } => json!({"sensorHold": {"tp": t_us, "area": area, "isDown": is_down}}),
                    };
                    (t_us as u64, json_val)
                }).collect();
                self.autoplay_index = 0;
                self.set_status(format!("Generated {} autoplay events", self.autoplay_events.len()));
            }
            Err(e) => {
                self.set_status(format!("Autoplay tactic failed: {}", e.json));
            }
        }
    }

    pub fn toggle_autoplay(&mut self) {
        self.autoplay = !self.autoplay;
        if self.autoplay {
            self.generate_autoplay_events();
            self.set_status("Autoplay enabled".to_string());
        } else {
            self.autoplay_events.clear();
            self.autoplay_index = 0;
            self.set_status("Autoplay disabled".to_string());
        }
    }
}
