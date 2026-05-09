use macroquad::audio::{play_sound, stop_sound, PlaySoundParams, Sound};
use macroquad::material::Material;
use macroquad::prelude::{get_time, Vec2};
use macroquad::texture::Texture2D;
use std::collections::{HashMap, HashSet};

use super::types::{
    ActiveRecordHold, ChartDoc, HitEvent, Mode, Note, NoteType, PadFeedback, RecordInputId, WavPcm, DragPart,
    HIT_WINDOW, HOLD_RECORD_MIN_DURATION, SPEED_MAX, SPEED_MIN, EACH_WINDOW,
    TOUCH_DISAPPEAR_TIME, SLIDE_MIN_POINTS, hold_tail_time, is_touch_zone, sanitize_note_zone, slide_end_time,
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
    pub(crate) drag_part: Option<DragPart>,
    pub(crate) drag_start_pos: Option<Vec2>,
    pub(crate) drag_start_time: f32,
    pub(crate) drag_multi_orig: Vec<(usize, f32, u8)>,
    pub(crate) box_start: Option<Vec2>,
    pub(crate) box_end: Option<Vec2>,
    pub(crate) waveform_data: Vec<f32>,
    pub(crate) waveform_freq_bins: u32,
    pub(crate) waveform_time_res: f32,
    pub(crate) waveform_threshold: f32,
    pub(crate) record_snap_grid: bool,
    pub(crate) selected_notes: Vec<usize>,
    pub(crate) drag_orig_note: Option<super::types::Note>,
    pub(crate) timeline_view_time: f32,
    pub(crate) undo_stack: Vec<super::types::ChartDoc>,
    pub(crate) clipboard: Vec<super::types::Note>,
    pub(crate) pasting: bool,

    pub(crate) record_speed: f32,
    pub(crate) play_speed: f32,
    pub(crate) touch_speed: f32,

    pub(crate) show_pad_only: bool,
    pub(crate) mobile_ui: bool,
    pub(crate) ui_scale_override: Option<f32>,

    pub(crate) audio_source_name: Option<String>,
    pub(crate) audio_wav_pcm: Option<WavPcm>,
    pub(crate) audio: Option<Sound>,
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
    pub(crate) mask_material: Option<Material>,
    pub(crate) pad_rect: Option<egui_macroquad::egui::Rect>,
    pub(crate) audio_cache: HashMap<i32, Sound>,
    pub(crate) audio_seek_offset: Option<f32>,
    pub(crate) pending_audio_start: bool,
    pub(crate) audio_enabled: bool,

    pub(crate) pad_svg: Option<super::pad_svg::PadSvgDef>,

    pub(crate) hit_sound: Option<Sound>,
    pub(crate) touch_sound: Option<Sound>,
    pub(crate) touch_riser_sound: Option<Sound>,
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
            drag_part: None,
            drag_start_pos: None,
            drag_start_time: 0.0,
            drag_multi_orig: Vec::new(),
            box_start: None,
            box_end: None,
            waveform_data: Vec::new(),
            waveform_freq_bins: 0,
            waveform_time_res: 0.0,
            waveform_threshold: 0.3,
            record_snap_grid: true,
            selected_notes: Vec::new(),
            drag_orig_note: None,
            timeline_view_time: 0.0,
            undo_stack: Vec::new(),
            clipboard: Vec::new(),
            pasting: false,
            record_speed: 1.0,
            play_speed: 1.0,
            touch_speed: 0.3,
            show_pad_only: false,
            mobile_ui,
            ui_scale_override,
            audio_source_name,
            audio_wav_pcm,
            audio: None,
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
            mask_material: None,
            pad_rect: None,
            audio_cache: HashMap::new(),
            audio_seek_offset: None,
            pending_audio_start: false,
            audio_enabled: true,
            pad_svg: None,
            hit_sound: None,
            touch_sound: None,
            touch_riser_sound: None,
            touch_riser_playing: false,
            hit_sounds_played: HashSet::new(),
            status: "Ready".to_string(),
        }
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
        if self.mode == Mode::Recording {
            self.rebase_song_clock();
            self.request_audio_start();
        }
        self.record_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
    }

    pub(crate) fn set_play_speed(&mut self, new_speed: f32) {
        if self.mode == Mode::Playing {
            self.rebase_song_clock();
            self.request_audio_start();
        }
        self.play_speed = new_speed.clamp(SPEED_MIN, SPEED_MAX);
    }

    // pub(crate) fn set_touch_speed(&mut self, new_speed: f32) {
    //     self.touch_speed = new_speed.clamp(TOUCH_SPEED_MIN, TOUCH_SPEED_MAX);
    // }

    pub(crate) fn stop_audio_if_any(&self) {
        if let Some(sound) = &self.audio {
            stop_sound(sound);
        }
    }

    pub(crate) fn request_audio_start(&mut self) {
        self.pending_audio_start = true;
    }

    pub(crate) fn seek_audio_to(&mut self, time: f32) {
        self.audio_seek_offset = Some(time);
        self.pending_audio_start = true;
    }

    pub(crate) fn push_undo(&mut self) {
        self.undo_stack.push(self.chart.clone());
        if self.undo_stack.len() > 64 { self.undo_stack.remove(0); }
    }

    pub(crate) fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.chart = prev;
            self.recompute_each();
            self.status = "Undo".to_string();
        }
    }

    pub(crate) fn recompute_each(&mut self) {
        let len = self.chart.notes.len();
        for i in 0..len {
            let t = self.chart.notes[i].time;
            let has_sibling = self.chart.notes.iter().enumerate().any(|(j, n)| i != j && (n.time - t).abs() < 0.001);
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
            if let Some(s) = &self.touch_riser_sound { stop_sound(s); }
            self.touch_riser_playing = false;
            self.timeline_view_time = self.mode_song_offset;
            self.status = format!("Paused at {:.2}s", self.mode_song_offset);
        } else {
            // Resume with audio seek
            self.audio_seek_offset = Some(self.mode_song_offset);
            self.mode = Mode::Playing;
            self.mode_wall_anchor = get_time();
            self.hit_sounds_played.clear();
            self.playback_cursor = 0;
            self.request_audio_start();
            self.status = format!("Resumed @ {:.1}x from {:.2}s", self.play_speed, self.mode_song_offset);
        }
    }

    pub(crate) fn toggle_record(&mut self) {
        if self.mode == Mode::Recording {
            self.flush_active_record_holds();
            self.set_mode(Mode::Idle);
            self.stop_audio_if_any();
            self.recording_notes.sort_by(|a, b| a.time.total_cmp(&b.time));
            self.chart.notes = self.recording_notes.clone();
            self.status = format!(
                "Record stopped: {} notes @ {:.1}x",
                self.chart.notes.len(),
                self.record_speed
            );
        } else {
            self.recording_hits.clear();
            self.recording_notes.clear();
            self.active_record_holds.clear();
            self.active_pointer_zones.clear();
            self.prev_pointer_pos.clear();
            self.set_mode(Mode::Recording);
            self.status = format!("Recording started @ {:.1}x", self.record_speed);
            self.request_audio_start();
        }
    }

    pub(crate) fn update_playback(&mut self) {
        if self.mode != Mode::Playing {
            return;
        }
        let t = self.song_time();

        while self.playback_cursor < self.chart.notes.len() {
            if self.chart.notes[self.playback_cursor].time + HIT_WINDOW < t {
                self.playback_cursor += 1;
            } else {
                break;
            }
        }

        if let Some(last) = self.chart.notes.last() {
            if t > last.time + 1.2 {
                self.set_mode(Mode::Idle);
                self.stop_audio_if_any();
                self.status = "Playback finished".to_string();
            }
        }
    }

    pub(crate) fn tick_feedback(&mut self) {
        let now = get_time();
        self.pad_feedback.retain(|f| f.until > now);
    }

    /// Play hit sound for notes that just reached their hit point or tail end.
    pub(crate) fn service_hit_sounds(&mut self) {
        if self.mode != Mode::Playing {
            return;
        }
        let t = self.song_time();

        for (i, note) in self.chart.notes.iter().enumerate() {
            // Head hit — touch uses TOUCH_DISAPPEAR_TIME to align with visual
            let hit_time = if matches!(note.note_type, NoteType::Touch) {
                note.time + TOUCH_DISAPPEAR_TIME
            } else {
                note.time
            };
            if !self.hit_sounds_played.contains(&i) && hit_time <= t {
                let s = if matches!(note.note_type, NoteType::Touch) {
                    self.touch_sound.as_ref()
                } else {
                    self.hit_sound.as_ref()
                };
                if let Some(sound) = s {
                    play_sound(sound, PlaySoundParams { looped: false, volume: 1.0 });
                }
                self.hit_sounds_played.insert(i);
            }
            // Hold tail
            let tail_key = i + self.chart.notes.len();
            if matches!(note.note_type, NoteType::Hold)
                && !self.hit_sounds_played.contains(&tail_key)
                && hold_tail_time(note) <= t
            {
                if let Some(sound) = self.hit_sound.as_ref() {
                    play_sound(sound, PlaySoundParams { looped: false, volume: 1.0 });
                }
                self.hit_sounds_played.insert(tail_key);
            }
        }

        // Touch hold riser: play while any touch hold is active
        let active_th = self.chart.notes.iter()
            .filter(|n| matches!(n.note_type, NoteType::Hold)
                && is_touch_zone(sanitize_note_zone(n.note_type, n.lane))
                && n.time <= t && hold_tail_time(n) > t)
            .count();
        if active_th > 0 && !self.touch_riser_playing {
            if let Some(s) = &self.touch_riser_sound {
                play_sound(s, PlaySoundParams { looped: true, volume: 0.5 });
                self.touch_riser_playing = true;
            }
        } else if active_th == 0 && self.touch_riser_playing {
            if let Some(s) = &self.touch_riser_sound {
                stop_sound(s);
                self.touch_riser_playing = false;
            }
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
        let duration = (end_time - active.start_time).max(0.0);
        let start_time = if self.record_snap_grid {
            let beat = 60.0 / self.chart.bpm;
            let grid = beat / (super::types::GRID_DIVISION as f32 / 4.0);
            (active.start_time / grid).round() * grid
        } else { active.start_time };
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
            if duration >= HOLD_RECORD_MIN_DURATION {
                NoteType::Hold
            } else {
                NoteType::Touch
            }
        } else if duration >= HOLD_RECORD_MIN_DURATION {
            NoteType::Hold
        } else {
            NoteType::Tap
        };

        let slide_points: Vec<super::types::SlidePoint> = active.slide_zones.iter()
            .map(|(z, off)| super::types::SlidePoint { zone: *z, beat_offset: *off })
            .collect();

        let slide_dur = if matches!(note_type, NoteType::Slide) { duration } else { 0.0 };

        self.chart.notes.push(Note {
            time: start_time, lane: active.lane, note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) { duration } else { 0.0 },
            is_each: false, slide_points: slide_points.clone(),
            slide_duration: slide_dur, slide_shape: None,
        });
        self.chart.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.recompute_each();

        self.recording_notes.push(Note {
            time: start_time, lane: active.lane, note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) { duration } else { 0.0 },
            is_each: false, slide_points,
            slide_duration: slide_dur, slide_shape: None,
        });
        self.recording_hits.push(HitEvent {
            time: active.start_time,
            lane: active.lane,
        });
    }
}
