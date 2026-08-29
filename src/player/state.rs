use super::audio::BgmPcm;
use super::sfx::{SfxBuffer, SfxPlayer};
use lambda_dx::app::types::zone::PadZone;
use lambda_dx::app::types::{FIXED_SLIDE_FADE_IN, PadGeom, SLIDE_TILE_SPACING};
use lambda_dx::pad_svg::PadSvgDef;
use lambda_dx::slide::segmentation;
use lambda_dx::slide_render;
use macroquad::material::Material;
use macroquad::prelude::{Vec2, get_time};
use macroquad::texture::Texture2D;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::types::{
    ActiveRecordHold, ChartDoc, DragPart, HIT_WINDOW, HOLD_RECORD_MIN_DURATION, HitEvent,
    JudgeFeedback, Mode, Note, NoteType, PadFeedback, RecordInputId, SLIDE_MIN_POINTS, SPEED_MAX,
    SPEED_MIN, SlidePoint, TOUCH_DISAPPEAR_TIME, WavPcm, hold_tail_time, is_touch_zone,
    mdur_to_secs, note_secs, sanitize_note_zone, sdur_to_mdur, secs_to_measure, snap_measure,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SlideProgress {
    /// Number of visual/judgment areas already completed.
    pub completed_areas: usize,
    /// Whether the current non-final area has seen an initial press.
    area_on: bool,
}

#[derive(Debug, Clone)]
pub struct LibrarySong {
    pub title: String,
    pub artist: String,
    pub chart_path: PathBuf,
    pub cover_path: Option<PathBuf>,
    pub descriptor: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerPage {
    Start,
    SongSelect,
    Settings,
    Gameplay,
    Pause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSettingsSection {
    Audio,
    Gameplay,
    Display,
}

#[derive(Debug, Clone)]
pub struct PlayerUiState {
    pub page: PlayerPage,
    pub settings_return: PlayerPage,
    pub selected_song: usize,
    pub loaded_song: Option<usize>,
    pub using_custom_song: bool,
    pub song_error: Option<String>,
    pub settings_section: PlayerSettingsSection,
}

impl Default for PlayerUiState {
    fn default() -> Self {
        Self {
            page: PlayerPage::Start,
            settings_return: PlayerPage::Start,
            selected_song: 0,
            loaded_song: None,
            using_custom_song: false,
            song_error: None,
            settings_section: PlayerSettingsSection::Audio,
        }
    }
}

impl PlayerUiState {
    pub fn open_settings(&mut self) {
        if self.page != PlayerPage::Settings {
            self.settings_return = self.page;
            self.page = PlayerPage::Settings;
        }
    }

    pub fn close_settings(&mut self) {
        self.page = self.settings_return;
    }

    pub const fn shows_gameplay_background(&self) -> bool {
        match self.page {
            PlayerPage::Gameplay | PlayerPage::Pause => true,
            PlayerPage::Settings => matches!(
                self.settings_return,
                PlayerPage::Gameplay | PlayerPage::Pause
            ),
            PlayerPage::Start | PlayerPage::SongSelect => false,
        }
    }
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
    pub player_ui: PlayerUiState,
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
    pub judge_feedback: Vec<JudgeFeedback>,
    pub playback_cursor: usize,
    pub selected_note: Option<u64>,
    pub dragging_note: Option<u64>,
    /// Note index detected under the mouse on press; selection is deferred
    /// until a drag threshold is exceeded.
    pub press_note_candidate: Option<u64>,
    pub drag_part: Option<DragPart>,
    pub drag_start_pos: Option<Vec2>,
    pub drag_start_time: f32,
    pub drag_shift: bool,
    /// Cursor's chart time at the moment of click. Used so dragging tracks the
    /// mouse's absolute position even if the user scrolls the timeline mid-drag.
    pub drag_cursor_anchor_t: f32,
    pub drag_multi_orig: Vec<(u64, f32, u8)>,
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
    pub selected_notes: Vec<u64>,
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
    pub last_click_note: Option<u64>,

    pub record_speed: f32,
    pub play_speed: f32,
    pub touch_speed: f32,
    /// Base note flight speed (流速), matching MajdataView's `noteSpeed`.
    pub note_speed: f32,
    /// Seconds before the hit the slide trail starts fading in (slide 显示时机).
    /// MajdataView uses `fadeInTime = -3.926913 / noteSpeed`.
    pub slide_fade_in: f32,

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
    pub ui_cover_textures: Vec<Option<egui_macroquad::egui::TextureHandle>>,
    pub ui_logo_texture: Option<egui_macroquad::egui::TextureHandle>,
    pub ui_assets_loaded: bool,
    pub song_library: Vec<LibrarySong>,
    pub song_library_scanned: bool,
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
    /// Notes already auto-judged by the lnmai engine (autoplay).
    pub auto_judged: HashSet<u64>,
    /// (note_id, sensor zone) already auto-triggered for slides (autoplay).
    pub auto_slide_sensors: HashSet<(u64, u8)>,
    pub next_note_id: u64,
    pub hidden_notes: HashSet<u64>,
    pub autoplay: bool,
    /// Per-note, per-sub-slide progress used to hide completed trail areas.
    pub slide_progress: HashMap<(u64, usize), SlideProgress>,

    /// Loaded lnmai-core judgment session (None until a chart is loaded).
    pub judge_engine: Option<super::engine::JudgeEngine>,
    /// Input events collected for the current frame, fed to the engine.
    pub engine_events: Vec<lnmai_core_rs::types::TimedInputEvent>,

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
}

impl PlayerState {
    pub fn new(
        chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
    ) -> Self {
        let mobile_ui = cfg!(any(target_os = "android", target_os = "ios"))
            || std::env::var("MAI2_MOBILE_UI")
                .map(|v| v == "1")
                .unwrap_or(false);

        let ui_scale_override = std::env::var("MAI2_UI_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v.clamp(0.7, 2.4));

        Self {
            player_ui: PlayerUiState::default(),
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
            judge_feedback: Vec::new(),
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
            note_speed: 7.5,
            slide_fade_in: 3.926_913 / 7.5,
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
            wifi_tex: [
                None, None, None, None, None, None, None, None, None, None, None,
            ],
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
            ui_cover_textures: Vec::new(),
            ui_logo_texture: None,
            ui_assets_loaded: false,
            song_library: Vec::new(),
            song_library_scanned: false,
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
            auto_judged: HashSet::new(),
            auto_slide_sensors: HashSet::new(),
            next_note_id: 1,
            hidden_notes: HashSet::new(),
            autoplay: false,
            slide_progress: HashMap::new(),
            judge_engine: None,
            engine_events: Vec::new(),
            status: "Ready".to_string(),
            import_path_input: String::new(),
            pending_import: false,
            imported_simai: None,
            import_levels: Vec::new(),
            import_selected_level: 0,
        }
    }

    // ── Setters with logging ──────────────────────────────────────────

    pub fn set_chart(&mut self, mut chart: ChartDoc) {
        for note in &mut chart.notes {
            if note.id == 0 {
                note.id = self.next_id();
            }
        }
        let n = chart.notes.len();
        let slides: Vec<_> = chart
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(n.note_type, NoteType::Slide))
            .collect();
        println!("[AppState] set_chart: {n} notes, {} slides", slides.len());
        for (i, note) in &slides {
            println!(
                "  slide #{i}: lane={} slides={} tapless={} star={}",
                note.lane,
                note.slide.len(),
                note.is_tapless,
                note.is_star
            );
            for (si, sl) in note.slide.iter().enumerate() {
                let shapes: Vec<_> = sl
                    .segments
                    .iter()
                    .map(|seg| format!("{:?}", seg.shape))
                    .collect();
                let pt_count: usize = sl.segments.iter().map(|seg| seg.points.len()).sum();
                println!(
                    "    slide[{si}]: shapes=[{}] pts={pt_count} dur={:.3} delay={:.3} break={}",
                    shapes.join(","),
                    sl.slide_duration,
                    sl.slide_start_delay,
                    sl.slide_is_break
                );
            }
        }
        self.chart = chart;
        self.slide_progress.clear();
    }

    pub fn set_selected_note(&mut self, sel: Option<u64>) {
        if self.selected_note != sel {
            println!(
                "[AppState] selected_note: {:?} -> {:?}",
                self.selected_note, sel
            );
        }
        self.selected_note = sel;
    }

    pub fn find_note_index(&self, id: u64) -> Option<usize> {
        self.chart.notes.iter().position(|n| n.id == id)
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_note_id;
        self.next_note_id += 1;
        id
    }
    pub fn push_note(&mut self, mut note: Note) {
        if note.id == 0 {
            note.id = self.next_id();
        }
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
            println!(
                "[AppState] editing_slide_path: {:?} -> {:?}",
                self.editing_slide_path, v
            );
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
        if self.undo_stack.len() > 64 {
            self.undo_stack.remove(0);
        }
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
            let has_sibling = self
                .chart
                .notes
                .iter()
                .enumerate()
                .any(|(j, n)| i != j && (n.time - m).abs() < 0.002);
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
            if let Some(player) = &mut self.sfx_player {
                player.stop_looped();
            }
            self.touch_riser_playing = false;
            self.timeline_view_time = self.mode_song_offset;
            self.active_sensor_holds.clear();
            for progress in self.slide_progress.values_mut() {
                progress.area_on = false;
            }
            self.set_status(format!("Paused at {:.2}s", self.mode_song_offset));
        } else {
            // Resume with audio seek
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.mode = Mode::Playing;
            self.mode_wall_anchor = get_time();
            self.hit_sounds_played.clear();
            self.playback_cursor = 0;
            self.request_audio_start();
            self.set_status(format!(
                "Resumed @ {:.1}x from {:.2}s",
                self.play_speed, self.mode_song_offset
            ));
        }
    }
    pub fn toggle_replay(&mut self) {
        self.mode = Mode::Playing;
        self.timeline_view_time = 0.;
        self.mode_song_offset = 0.;
        self.audio_seek_offset = Some(0.);
        self.hit_sounds_played.clear();
        self.recording_hits.clear();
        self.recording_notes.clear();
        self.active_record_holds.clear();
        self.active_pointer_zones.clear();
        self.active_sensor_holds.clear();
        self.prev_pointer_pos.clear();
        self.slide_progress.clear();
        self.request_audio_start();
    }

    /// Advance Slide judge areas from the currently held pad sensors.
    ///
    /// Each consecutive zone run is one area. Intermediate areas need an
    /// On->Off transition; the final area completes on On, matching
    /// MajdataView's `Area.IsLast` behavior.
    pub fn update_slide_judgment(
        &mut self,
        pad: PadGeom,
        svg: &PadSvgDef,
        scale: f32,
        spawn_center: macroquad::prelude::Vec2,
    ) {
        if self.mode != Mode::Playing {
            return;
        }

        let now = self.song_time();
        let autoplay = self.autoplay;
        let active: HashSet<PadZone> = self.active_sensor_holds.values().copied().collect();
        let bpms = self.chart.bpms.clone();

        // The lnmai-core engine owns autoplay judging when loaded; this manual
        // autoplay feedback only runs as a fallback. The manual slide-area
        // progression (bar hiding on touch) below always runs.
        if autoplay && self.judge_engine.is_none() {
            let auto_judgements: Vec<(usize, PadZone)> = self
                .chart
                .notes
                .iter()
                .enumerate()
                .filter_map(|(index, note)| {
                    if self.hit_sounds_played.contains(&index) {
                        return None;
                    }
                    let note_time = note_secs(note, &bpms);
                    if now >= note_time {
                        Some((
                            index,
                            PadZone::from(sanitize_note_zone(note.note_type, note.lane)),
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            for (index, zone) in auto_judgements {
                self.hit_sounds_played.insert(index);
                self.push_judgement(zone, "PERFECT", 0.32);
            }
        }

        let slide_notes: Vec<(u64, usize, f32, f32, Vec<(PadZone, usize)>, usize)> = self
            .chart
            .notes
            .iter()
            .filter(|note| matches!(note.note_type, NoteType::Slide))
            .flat_map(|note| {
                let head_time = note_secs(note, &bpms);
                let slide_bpms = bpms.clone();
                note.slide
                    .iter()
                    .enumerate()
                    .map(move |(slide_idx, slide)| {
                        let path = slide_render::build_slide_path(
                            note,
                            slide,
                            &pad,
                            svg,
                            scale,
                            spawn_center,
                            pad.outer_r,
                        );
                        let visual =
                            segmentation::build(&path, SLIDE_TILE_SPACING * scale, svg, &pad);
                        let areas = visual
                            .judge_segments
                            .iter()
                            .map(|segment| (segment.zone, segment.end_bar))
                            .collect();
                        (
                            note.id,
                            slide_idx,
                            head_time
                                + mdur_to_secs(slide.slide_start_delay, note.time, &slide_bpms),
                            // `slide_duration` is the total span from the head.
                            head_time + mdur_to_secs(slide.slide_duration, note.time, &slide_bpms),
                            areas,
                            visual.bars.len(),
                        )
                    })
            })
            .collect();

        for (note_id, slide_idx, start_time, end_time, areas, bar_count) in slide_notes {
            if now < start_time || now > end_time + 0.6 || areas.is_empty() || bar_count == 0 {
                continue;
            }
            let progress = self.slide_progress.entry((note_id, slide_idx)).or_default();
            if autoplay && self.judge_engine.is_none() {
                let process =
                    ((now - start_time) / (end_time - start_time).max(0.001)).clamp(0.0, 1.0);
                let mut completed_zones = Vec::new();
                while progress.completed_areas < areas.len() {
                    let area_index = progress.completed_areas;
                    let segment_end = areas[area_index].1 as f32 / bar_count as f32;
                    if segment_end > process {
                        break;
                    }
                    progress.completed_areas += 1;
                    progress.area_on = false;
                    completed_zones.push(areas[area_index].0);
                }
                for zone in completed_zones {
                    self.push_judgement(zone, "SLIDE", 0.26);
                }
                continue;
            }
            while progress.completed_areas < areas.len() {
                let is_last = progress.completed_areas + 1 == areas.len();
                let zone = areas[progress.completed_areas].0;
                if is_last {
                    if active.contains(&zone) {
                        progress.completed_areas += 1;
                    }
                    break;
                }
                if progress.area_on {
                    if !active.contains(&zone) {
                        progress.completed_areas += 1;
                        progress.area_on = false;
                        continue;
                    }
                } else if active.contains(&zone) {
                    progress.area_on = true;
                }
                break;
            }
        }
    }

    pub fn toggle_record(&mut self) {
        if self.mode == Mode::Recording {
            self.flush_active_record_holds();
            self.set_mode(Mode::Idle);
            self.stop_audio_if_any();
            self.recording_notes
                .sort_by(|a, b| a.time.total_cmp(&b.time));
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

    pub fn tick_feedback(&mut self) {
        let now = get_time();
        self.pad_feedback.retain(|f| f.until > now);
    }

    pub fn push_feedback(&mut self, zone: PadZone, duration: f64) {
        self.pad_feedback.push(PadFeedback {
            zone,
            until: get_time() + duration,
        });
    }

    pub fn judge_input(&mut self, zone: PadZone) {
        if self.mode != Mode::Playing {
            return;
        }
        let now = self.song_time();
        let best = self
            .chart
            .notes
            .iter()
            .filter(|note| sanitize_note_zone(note.note_type, note.lane) == zone.to_id())
            .map(|note| (note_secs(note, &self.chart.bpms) - now).abs())
            .min_by(f32::total_cmp);
        let Some(diff) = best else {
            self.push_judgement(zone, "MISS", 0.24);
            return;
        };
        let (label, duration) = if diff <= 0.06 {
            ("PERFECT", 0.32)
        } else if diff <= 0.14 {
            ("GREAT", 0.28)
        } else if diff <= 0.24 {
            ("GOOD", 0.24)
        } else {
            ("MISS", 0.24)
        };
        self.push_judgement(zone, label, duration);
    }

    /// Record a zone press/release into the lnmai-core judgment engine's input
    /// buffer for the current frame. Falls back to the manual judge windows
    /// when no engine is loaded.
    pub fn record_engine_input(&mut self, zone: PadZone, is_down: bool) {
        if self.judge_engine.is_none() {
            if is_down {
                self.judge_input(zone);
            }
            return;
        }
        let tp = (self.song_time().max(0.0) * 1e6) as i64;
        if let Some((click, hold_down, hold_up)) = super::engine::events_for_zone(zone, tp) {
            if is_down {
                self.engine_events.push(click);
                self.engine_events.push(hold_down);
            } else {
                self.engine_events.push(hold_up);
            }
        }
    }

    /// (Re)load the lnmai-core judgment engine for the currently imported chart.
    pub fn reload_judge_engine(&mut self) {
        self.judge_engine = None;
        let Some(file) = self.imported_simai.clone() else {
            return;
        };
        let text = maisimai::export_file(&file);
        match super::engine::JudgeEngine::load(&text, self.import_selected_level) {
            Ok(engine) => {
                self.judge_engine = Some(engine);
                self.set_status(format!("判引擎已载入 (Lv.{})", self.import_selected_level));
            }
            Err(e) => {
                self.set_status(format!("判引擎载入失败: {e}"));
            }
        }
    }

    pub fn push_judgement(&mut self, zone: PadZone, label: &str, duration: f64) {
        let color = match label {
            "PERFECT" => macroquad::prelude::Color::from_rgba(250, 204, 21, 255),
            "GREAT" => macroquad::prelude::Color::from_rgba(52, 211, 153, 255),
            "GOOD" => macroquad::prelude::Color::from_rgba(96, 165, 250, 255),
            "SLIDE" => macroquad::prelude::Color::from_rgba(232, 121, 249, 255),
            _ => macroquad::prelude::Color::from_rgba(248, 113, 113, 255),
        };
        let started = get_time();
        self.judge_feedback.push(JudgeFeedback {
            zone,
            label: label.to_string(),
            color,
            started,
            until: started + duration,
        });
    }

    pub fn start_record_hold_input(&mut self, input_id: RecordInputId, lane: PadZone) {
        let start_time = self.song_time();
        self.active_record_holds
            .entry(input_id)
            .or_insert(ActiveRecordHold {
                lane: lane.to_id(),
                start_time,
                slide_zones: vec![SlidePoint {
                    zone: lane,
                    beat_offset: 0.0,
                }],
            });
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
        let active: Vec<ActiveRecordHold> =
            self.active_record_holds.drain().map(|(_, v)| v).collect();
        for item in active {
            self.push_recorded_note(item, end_time);
        }
    }

    pub fn record_slide_zone(&mut self, input_id: RecordInputId, zone: PadZone) {
        let t = self.song_time();
        if let Some(active) = self.active_record_holds.get_mut(&input_id) {
            let last_zone = active
                .slide_zones
                .last()
                .map(|sp| sp.zone.to_id())
                .unwrap_or(active.lane);
            if zone != last_zone {
                active.slide_zones.push(SlidePoint {
                    zone,
                    beat_offset: t - active.start_time,
                });
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

        let slide_dur = if matches!(note_type, NoteType::Slide) {
            dur_measure
        } else {
            0.0
        };
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
        let note_id = self.next_id();

        self.chart.notes.push(Note {
            id: note_id,
            time: start_measure,
            lane: active.lane,
            note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) {
                dur_measure
            } else {
                0.0
            },
            slide: slide_vec.clone(),
            ..Default::default()
        });
        self.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.recompute_each();

        self.recording_notes.push(Note {
            id: note_id,
            time: start_measure,
            lane: active.lane,
            note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) {
                dur_measure
            } else {
                0.0
            },
            slide: slide_vec,
            ..Default::default()
        });
        self.recording_hits.push(HitEvent {
            time: active.start_time,
            lane: active.lane,
        });
    }
}

#[cfg(test)]
mod player_ui_tests {
    use super::{PlayerPage, PlayerUiState};

    #[test]
    fn settings_returns_to_the_page_that_opened_it() {
        // Given
        let mut ui = PlayerUiState {
            page: PlayerPage::Pause,
            ..PlayerUiState::default()
        };

        // When
        ui.open_settings();
        ui.close_settings();

        // Then
        assert_eq!(ui.page, PlayerPage::Pause);
    }

    #[test]
    fn gameplay_remains_visible_behind_pause_settings() {
        // Given
        let mut ui = PlayerUiState {
            page: PlayerPage::Pause,
            ..PlayerUiState::default()
        };

        // When
        ui.open_settings();

        // Then
        assert!(ui.shows_gameplay_background());
    }
}
