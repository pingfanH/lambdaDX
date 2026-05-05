use macroquad::audio::{play_sound, stop_sound, PlaySoundParams, Sound};
use macroquad::material::Material;
use macroquad::prelude::{get_time, Vec2};
use macroquad::texture::Texture2D;
use std::collections::{HashMap, HashSet};

use super::types::{
    ActiveRecordHold, ChartDoc, HitEvent, Mode, Note, NoteType, PadFeedback, RecordInputId, WavPcm,
    HIT_WINDOW, HOLD_RECORD_MIN_DURATION, SPEED_MAX, SPEED_MIN, EACH_WINDOW,
    TOUCH_DISAPPEAR_TIME, hold_tail_time, is_touch_zone, sanitize_note_zone,
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
    pub(crate) mask_material: Option<Material>,
    pub(crate) audio_cache: HashMap<i32, Sound>,
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
            mask_material: None,
            audio_cache: HashMap::new(),
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
            Mode::Idle => 1.0,
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

    pub(crate) fn toggle_play(&mut self) {
        if self.mode == Mode::Playing {
            self.set_mode(Mode::Idle);
            self.stop_audio_if_any();
            self.status = "Stopped playback".to_string();
        } else {
            self.set_mode(Mode::Playing);
            self.hit_sounds_played.clear();
            self.status = format!("Playing chart @ {:.1}x", self.play_speed);
            self.request_audio_start();
        }
    }

    pub(crate) fn toggle_record(&mut self) {
        if self.mode == Mode::Recording {
            self.flush_active_record_holds();
            self.set_mode(Mode::Idle);
            self.stop_audio_if_any();
            self.recording_notes.sort_by(|a, b| a.time.total_cmp(&b.time));
            // Mark simultaneous notes as each
            {
                let notes = &mut self.recording_notes;
                let window = EACH_WINDOW / self.record_speed;
                let mut i = 0;
                while i < notes.len() {
                    let t = notes[i].time;
                    let mut j = i;
                    while j < notes.len() && notes[j].time - t <= window {
                        j += 1;
                    }
                    if j - i >= 2 {
                        for k in i..j {
                            notes[k].is_each = true;
                        }
                    }
                    i = j;
                }
            }
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
            .or_insert(ActiveRecordHold { lane, start_time });
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

    fn push_recorded_note(&mut self, active: ActiveRecordHold, end_time: f32) {
        let duration = (end_time - active.start_time).max(0.0);
        let note_type = if is_touch_zone(active.lane) {
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

        self.recording_notes.push(Note {
            time: active.start_time,
            lane: active.lane,
            note_type,
            hold_duration: if matches!(note_type, NoteType::Hold) {
                duration
            } else {
                0.0
            },
            is_each: false,
        });
        self.recording_hits.push(HitEvent {
            time: active.start_time,
            lane: active.lane,
        });
    }
}
