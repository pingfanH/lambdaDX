//! Player UI state: the page machine, the chart selection and the settings,
//! wrapping the reused pad view/playback ([`PadPreviewState`]).

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use macroquad::prelude::{Texture2D, get_time};

use crate::app::audio;
use crate::app::types::zone::PadZone;
use crate::app::types::{BpmChange, ChartDoc, Note, NoteType, WavPcm, measure_to_secs, note_secs};
use crate::player::cues::CueTrack;
use crate::player::state::PadPreviewState;
use crate::player_ui::library::{self, Library};

/// Build the sorted autoplay event schedule from a chart: a synthetic touch
/// press at every note head, a release later, and a "hide" marker for when the
/// note should vanish (head for taps/touches, tail/end for holds/slides).
fn autoplay_events_for(notes: &[Note], bpms: &[BpmChange]) -> Vec<AutoplayEvent> {
    const TAP_RELEASE: f32 = 0.06;
    let mut events: Vec<AutoplayEvent> = Vec::with_capacity(notes.len() * 2);
    for note in notes {
        let zone = PadZone::from(note.lane);
        let head = note_secs(note, bpms);
        let (release, hide_at) = match note.note_type {
            NoteType::Hold => {
                let tail = measure_to_secs(note.time + note.hold_duration, bpms);
                (tail, tail)
            }
            NoteType::Slide => {
                let dur = note
                    .slide
                    .iter()
                    .map(|s| s.slide_duration)
                    .fold(0.0_f32, f32::max);
                let end = measure_to_secs(note.time + dur, bpms);
                (end, end)
            }
            _ => (head + TAP_RELEASE, head),
        };
        events.push(AutoplayEvent {
            t: head,
            zone,
            down: true,
            hide: (hide_at <= head + 1e-4).then_some(note.id),
        });
        events.push(AutoplayEvent {
            t: release,
            zone,
            down: false,
            hide: (hide_at > head + 1e-4).then_some(note.id),
        });
    }
    events.sort_by(|a, b| a.t.total_cmp(&b.t));
    events
}

/// Advance the autoplay cursor to `t`, returning the new cursor and the events
/// to apply this frame. Events older than 200 ms (jumped past by a seek) are
/// consumed without applying; a backward seek rewinds the cursor.
fn autoplay_due(
    events: &[AutoplayEvent],
    cursor: usize,
    t: f32,
) -> (usize, Vec<AutoplayEvent>) {
    let mut cursor = cursor;
    if cursor > 0 && events.get(cursor - 1).is_some_and(|e| e.t > t + 0.02) {
        cursor = events.partition_point(|e| e.t < t);
    }
    let mut due = Vec::new();
    while let Some(&ev) = events.get(cursor) {
        if ev.t > t {
            break;
        }
        if t - ev.t <= 0.20 {
            due.push(ev);
        }
        cursor += 1;
    }
    (cursor, due)
}

/// Synthetic pointer id for a zone (kept clear of real touches and the keyboard
/// lane ids by living just below `u64::MAX`).
fn autoplay_pointer_id(zone: PadZone) -> u64 {
    const AUTOPLAY_ID_BASE: u64 = u64::MAX - 3000;
    AUTOPLAY_ID_BASE - zone.to_id() as u64
}

fn is_autoplay_pointer_id(id: u64) -> bool {
    id > u64::MAX - 3100 && id < u64::MAX - 3000
}

#[derive(Debug, Clone, Copy)]
struct AutoplayEvent {
    t: f32,
    zone: PadZone,
    down: bool,
    hide: Option<u64>,
}

/// Result of a background audio decode.
type AudioDecoded = (Option<String>, Option<WavPcm>);

/// A chart parsed on a worker thread, ready to install on the main thread.
struct LoadedChart {
    index: usize,
    text: String,
    levels: Vec<(u32, String)>,
    key: Option<u32>,
    chart: ChartDoc,
    audio_path: Option<std::path::PathBuf>,
}

type ChartResult = Result<LoadedChart, String>;

/// Read + parse + convert a song's chart entirely off the main thread.
fn load_chart_job(song: &library::LibrarySong, index: usize) -> ChartResult {
    let _s = crate::player_ui::perf::Scope::new(format!("chart_job[{index}]"));
    let text = std::fs::read_to_string(&song.chart_path)
        .map_err(|e| format!("读取 {}: {e}", song.chart_path.display()))?;
    let levels = song.levels.clone();
    let key = library::default_level(&levels);
    let chart = library::chart_for_level(&text, key).map_err(|e| e.to_string())?;
    let audio_path = crate::app::chart::find_audio_in_dir(&song.folder);
    Ok(LoadedChart {
        index,
        text,
        levels,
        key,
        chart,
        audio_path,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Start,
    SongSelect,
    Settings,
    Gameplay,
    Pause,
}

pub struct PlayerUiApp {
    /// The reused view + playback (pad rendering, audio clock, cues).
    pub pad: PadPreviewState,

    // ── Page machine ──────────────────────────────────────────────────
    pub page: Page,
    pub prev_page: Page,
    pub settings_return: Page,
    pub page_born: f64,
    pub transition_at: f64,
    /// Snapshot of the outgoing scene, captured once per transition so the
    /// masked wipe can show it sliding right while the new scene sits left.
    pub transition_tex: Option<Texture2D>,
    /// Which `transition_at` the snapshot belongs to.
    pub transition_snap: Option<f64>,

    // ── Library / selection ───────────────────────────────────────────
    pub library: Library,
    pub selected: usize,
    pub loaded: Option<usize>,
    /// Raw `maidata.txt` backing the current song, for difficulty switching.
    pub chart_text: Option<String>,
    pub levels: Vec<(u32, String)>,
    pub selected_level: Option<u32>,
    pub using_custom: bool,
    /// Audio track for the loaded song; decoded on a worker thread.
    pub audio_path: Option<std::path::PathBuf>,
    /// In-flight decode and the song index it belongs to.
    audio_rx: Option<Receiver<AudioDecoded>>,
    audio_for: Option<usize>,
    /// Song index whose audio is already installed in `pad`.
    audio_ready_for: Option<usize>,
    /// Waiting for the audio decode before starting playback.
    pub preparing: bool,
    /// True while the player is dragging the gameplay progress bar.
    pub scrubbing: bool,

    // ── Background chart load ─────────────────────────────────────────
    chart_rx: Option<Receiver<ChartResult>>,
    /// True while a chart is being parsed on a worker thread.
    pub loading_song: bool,
    pub loading_song_index: Option<usize>,

    // ── Autoplay (chart-driven, simulates real touches) ───────────────
    /// When on, the pad presses itself at each note's hit time.
    pub autoplay: bool,
    autoplay_events: Vec<AutoplayEvent>,
    autoplay_cursor: usize,
    /// Note ids hidden by autoplay, so a seek can restore them.
    autoplay_hidden: Vec<u64>,

    // ── Settings tab ──────────────────────────────────────────────────
    pub settings_section: usize,

    // ── List interaction ──────────────────────────────────────────────
    pub list_scroll: f32,
    pub list_scroll_target: f32,

    // ── Messaging ─────────────────────────────────────────────────────
    pub status: String,
    pub error: Option<String>,

    /// Set once the gameplay frame has been presented (audio gate).
    pub gameplay_presented: bool,
}

impl PlayerUiApp {
    pub fn new(
        chart: ChartDoc,
        audio_source_name: Option<String>,
        audio_wav_pcm: Option<WavPcm>,
    ) -> Self {
        let pad = PadPreviewState::new(chart, audio_source_name, audio_wav_pcm);
        let now = get_time();
        Self {
            pad,
            page: Page::Start,
            prev_page: Page::Start,
            settings_return: Page::Start,
            page_born: now,
            transition_at: now - 10.0,
            transition_tex: None,
            transition_snap: None,
            library: Library::empty(),
            selected: 0,
            loaded: None,
            chart_text: None,
            levels: Vec::new(),
            selected_level: None,
            using_custom: false,
            audio_path: None,
            audio_rx: None,
            audio_for: None,
            audio_ready_for: None,
            preparing: false,
            scrubbing: false,
            chart_rx: None,
            loading_song: false,
            loading_song_index: None,
            autoplay: false,
            autoplay_events: Vec::new(),
            autoplay_cursor: 0,
            autoplay_hidden: Vec::new(),
            settings_section: 0,
            list_scroll: 0.0,
            list_scroll_target: 0.0,
            status: "Ready".to_string(),
            error: None,
            gameplay_presented: false,
        }
    }

    pub fn go(&mut self, page: Page) {
        if self.page == page {
            return;
        }
        if page != Page::Gameplay {
            self.preparing = false;
        }
        self.prev_page = self.page;
        self.page = page;
        let now = get_time();
        self.page_born = now;
        self.transition_at = now;
        self.list_scroll_target = self.list_scroll;
    }

    /// Switch page **without** the masked transition (and cancel any in-flight
    /// one). Used for pause/resume, where the overlay is the animation.
    pub fn go_instant(&mut self, page: Page) {
        if self.page == page {
            return;
        }
        if page != Page::Gameplay {
            self.preparing = false;
        }
        self.prev_page = self.page;
        self.page = page;
        self.page_born = get_time();
        self.transition_tex = None;
        self.transition_snap = None;
        self.list_scroll_target = self.list_scroll;
    }

    pub fn open_settings(&mut self) {
        if self.page == Page::Settings {
            return;
        }
        self.settings_return = self.page;
        self.go(Page::Settings);
    }

    pub fn close_settings(&mut self) {
        self.go(self.settings_return);
    }

    /// True when the pad should be drawn under the current page. Settings is a
    /// full opaque page (it never reveals the pad behind it).
    pub fn shows_gameplay_background(&self) -> bool {
        matches!(self.page, Page::Gameplay | Page::Pause)
    }

    // ── Song loading ──────────────────────────────────────────────────

    /// Request a song load on a worker thread; the result is installed by
    /// [`Self::poll_chart`]. Selecting a song therefore never blocks a frame.
    pub fn request_load_song(&mut self, index: usize) -> Result<(), String> {
        let song = self
            .library
            .songs
            .get(index)
            .cloned()
            .ok_or("曲库中没有这首歌")?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(load_chart_job(&song, index));
        });
        self.chart_rx = Some(rx);
        self.loading_song = true;
        self.loading_song_index = Some(index);
        self.selected = index;
        Ok(())
    }

    /// Install a finished background chart load. Call once per frame.
    pub fn poll_chart(&mut self) {
        let Some(rx) = &self.chart_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(loaded)) => {
                self.chart_rx = None;
                self.loading_song = false;
                self.loading_song_index = None;
                self.install_loaded(loaded);
            }
            Ok(Err(e)) => {
                self.chart_rx = None;
                self.loading_song = false;
                self.loading_song_index = None;
                self.error = Some(e);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.chart_rx = None;
                self.loading_song = false;
                self.loading_song_index = None;
            }
        }
    }

    /// Synchronous load (used by the dev harness and as a fallback). UI paths use
    /// [`Self::request_load_song`].
    pub fn load_song(&mut self, index: usize) -> Result<(), String> {
        let _s = crate::player_ui::perf::Scope::new(format!("load_song[{index}]"));
        let song = self
            .library
            .songs
            .get(index)
            .cloned()
            .ok_or("曲库中没有这首歌")?;
        let loaded = load_chart_job(&song, index)?;
        self.install_loaded(loaded);
        Ok(())
    }

    fn install_loaded(&mut self, loaded: LoadedChart) {
        self.chart_text = Some(loaded.text);
        self.levels = loaded.levels;
        self.selected_level = loaded.key;
        self.apply_chart(loaded.chart);

        self.audio_path = loaded.audio_path;
        self.pad.audio_wav_pcm = None;
        self.pad.audio_cache.clear();
        self.pad.audio_source_name = self
            .audio_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        self.audio_ready_for = None;
        self.start_audio_decode(loaded.index);

        self.loaded = Some(loaded.index);
        self.selected = loaded.index;
        self.using_custom = false;
        self.error = None;
        self.status = format!(
            "已载入 {} · {} notes",
            self.pad.chart.title,
            self.pad.chart.notes.len()
        );
    }

    /// Switch to a difficulty slot without reloading audio.
    pub fn select_level(&mut self, key: u32) -> Result<(), String> {
        let _s = crate::player_ui::perf::Scope::new(format!("select_level[{key}]"));
        let text = self.chart_text.clone().ok_or("当前歌曲没有可切换的难度")?;
        let chart = crate::player_ui::perf::time("  parse+convert chart", || {
            library::chart_for_level(&text, Some(key)).map_err(|e| e.to_string())
        })?;
        self.selected_level = Some(key);
        self.apply_chart(chart);
        self.error = None;
        self.status = format!("难度已切换至 Lv.{key}");
        Ok(())
    }

    /// Swap the chart into the reused view and rebuild derived state.
    pub fn apply_chart(&mut self, chart: ChartDoc) {
        self.pad.stop_audio_if_any();
        self.pad.mode = crate::app::types::Mode::Idle;
        self.pad.playback_pending = false;
        self.pad.mode_song_offset = 0.0;
        self.pad.timeline_view_time = 0.0;
        self.pad.mode_wall_anchor = get_time();
        self.pad.chart = chart;
        // Simai-converted notes all default to id 0; give them unique ids so
        // per-note hiding (autoplay judgment) and slide-progress keys work.
        let mut next_id = self.pad.chart.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        for note in &mut self.pad.chart.notes {
            if note.id == 0 {
                note.id = next_id;
                next_id += 1;
            }
        }
        self.pad.hidden_notes.clear();
        self.pad.slide_progress.clear();
        self.pad.active_pointer_zones.clear();
        self.pad.prev_pointer_pos.clear();
        self.pad.cue_track = Some(CueTrack::from_chart(&self.pad.chart));
        self.rebuild_autoplay_events();
    }

    // ── Autoplay ──────────────────────────────────────────────────────

    /// Rebuild the sorted autoplay schedule for the current chart.
    fn rebuild_autoplay_events(&mut self) {
        self.autoplay_events = autoplay_events_for(&self.pad.chart.notes, &self.pad.chart.bpms);
        self.autoplay_cursor = 0;
        self.autoplay_hidden.clear();
    }

    pub fn set_autoplay(&mut self, on: bool) {
        if self.autoplay == on {
            return;
        }
        self.autoplay = on;
        if !on {
            self.release_autoplay_touches();
            self.clear_autoplay_hidden();
        }
        let t = self.pad.song_time();
        self.autoplay_cursor = self.autoplay_events.partition_point(|e| e.t < t);
        self.status = if on {
            "Autoplay: ON".to_string()
        } else {
            "Autoplay: OFF".to_string()
        };
    }

    /// Drive autoplay for this frame: synthesize touches and hide judged notes.
    pub fn tick_autoplay(&mut self) {
        if !self.autoplay {
            return;
        }
        if self.pad.mode != crate::app::types::Mode::Playing || self.pad.playback_pending {
            self.release_autoplay_touches();
            return;
        }
        let t = self.pad.song_time();
        // Backward seek / restart: rewind instead of dumping a burst.
        if self.autoplay_cursor > 0
            && self
                .autoplay_events
                .get(self.autoplay_cursor - 1)
                .is_some_and(|e| e.t > t + 0.02)
        {
            self.autoplay_cursor = self.autoplay_events.partition_point(|e| e.t < t);
            self.release_autoplay_touches();
            self.clear_autoplay_hidden();
        }
        let (cursor, due) = autoplay_due(&self.autoplay_events, self.autoplay_cursor, t);
        self.autoplay_cursor = cursor;
        for ev in due {
            self.apply_autoplay_event(ev);
        }
    }

    fn apply_autoplay_event(&mut self, ev: AutoplayEvent) {
        let id = autoplay_pointer_id(ev.zone);
        if ev.down {
            // Synthetic touch: same visual path as a real finger press.
            self.pad.active_pointer_zones.insert(id, ev.zone);
            self.pad.push_feedback(ev.zone, 0.12);
            if let Some(label) = crate::player::input::hit::judge_label_for_zone(&self.pad, ev.zone)
            {
                self.pad.push_judgement(ev.zone, label, 0.6);
            }
        } else {
            self.pad.active_pointer_zones.remove(&id);
        }
        if let Some(note_id) = ev.hide {
            // Note disappears the moment it is judged.
            self.pad.hidden_notes.insert(note_id);
            self.autoplay_hidden.push(note_id);
        }
    }

    fn release_autoplay_touches(&mut self) {
        let ids: Vec<u64> = self
            .pad
            .active_pointer_zones
            .keys()
            .copied()
            .filter(|id| is_autoplay_pointer_id(*id))
            .collect();
        for id in ids {
            self.pad.active_pointer_zones.remove(&id);
        }
    }

    fn clear_autoplay_hidden(&mut self) {
        for id in self.autoplay_hidden.drain(..) {
            self.pad.hidden_notes.remove(&id);
        }
    }

    pub fn begin_gameplay(&mut self) -> Result<(), String> {
        if self.loading_song {
            return Err("谱面载入中…".to_string());
        }
        if self.loaded.is_none() && !self.library.songs.is_empty() {
            self.request_load_song(self.selected)?;
            return Err("谱面载入中…".to_string());
        }
        if self.pad.chart.notes.is_empty() {
            return Err("该谱面没有音符".to_string());
        }
        // Audio already installed (or there is no track): start straight away.
        if self.pad.audio_wav_pcm.is_some() || self.audio_path.is_none() {
            self.pad.start_playback_at(0.0);
            self.go(Page::Gameplay);
            return Ok(());
        }
        // Otherwise the decode is running on a worker thread (started when the
        // song was selected). Enter gameplay immediately with a "preparing"
        // state so the transition and UI keep animating, then begin as soon as
        // the audio lands.
        if let Some(index) = self.loaded {
            self.start_audio_decode(index);
        }
        self.preparing = true;
        self.go(Page::Gameplay);
        Ok(())
    }

    /// Kick off (or re-target) a background decode of `index`'s audio. The
    /// result is installed by [`Self::poll_audio`] on the main thread.
    fn start_audio_decode(&mut self, index: usize) {
        if self.audio_for == Some(index) && self.audio_rx.is_some() {
            return;
        }
        if self.audio_ready_for == Some(index) && self.pad.audio_wav_pcm.is_some() {
            return;
        }
        let Some(path) = self.audio_path.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = crate::player_ui::perf::time("  decode audio (bg thread)", || {
                audio::load_audio_from_path(&path)
            });
            let _ = tx.send(result);
        });
        self.audio_rx = Some(rx);
        self.audio_for = Some(index);
    }

    /// Install a finished background decode; if playback was waiting on it,
    /// start now. Call once per frame.
    pub fn poll_audio(&mut self) {
        let Some(rx) = &self.audio_rx else {
            return;
        };
        let ready = match rx.try_recv() {
            Ok((name, pcm)) => {
                let index = self.audio_for;
                if index == self.loaded && pcm.is_some() {
                    self.pad.audio_source_name = name;
                    self.pad.audio_wav_pcm = pcm;
                    self.pad.audio_cache.clear();
                    self.audio_ready_for = index;
                }
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => true,
        };
        if !ready {
            return;
        }
        self.audio_rx = None;
        if self.preparing && self.page == Page::Gameplay {
            self.preparing = false;
            self.pad.start_playback_at(0.0);
        }
    }

    // ── Seeking ───────────────────────────────────────────────────────

    /// Update the visible position while dragging (no audio restart).
    pub fn scrub_to(&mut self, t: f32) {
        let t = t.clamp(0.0, self.song_duration());
        self.pad.mode_song_offset = t;
        self.pad.timeline_view_time = t;
        self.pad.mode_wall_anchor = get_time();
        if let Some(track) = self.pad.cue_track.as_mut() {
            track.reset(t);
        }
    }

    /// Commit a seek: reposition the clock and restart audio if playing.
    pub fn seek_to(&mut self, t: f32) {
        let t = t.clamp(0.0, self.song_duration());
        self.pad.seek_audio_to(t);
        self.pad.mode_song_offset = t;
        self.pad.timeline_view_time = t;
        self.pad.mode_wall_anchor = get_time();
    }

    pub fn seek_relative(&mut self, delta: f32) {
        let t = self.pad.song_time() + delta;
        self.seek_to(t);
    }

    // ── Playback helpers ──────────────────────────────────────────────

    pub fn pause(&mut self) {
        if self.pad.mode == crate::app::types::Mode::Playing {
            self.pad.toggle_play();
        }
        self.go_instant(Page::Pause);
    }

    pub fn resume(&mut self) {
        self.pad.toggle_play();
        self.go_instant(Page::Gameplay);
    }

    pub fn restart(&mut self) {
        // If the audio has not finished decoding yet, go through "preparing".
        if self.pad.audio_wav_pcm.is_none() && self.audio_path.is_some() {
            if let Some(index) = self.loaded {
                self.start_audio_decode(index);
            }
            self.preparing = true;
            self.go(Page::Gameplay);
            return;
        }
        self.pad.start_playback_at(0.0);
        self.go(Page::Gameplay);
    }

    pub fn exit_to_select(&mut self) {
        self.pad.stop_audio_if_any();
        self.pad.mode = crate::app::types::Mode::Idle;
        self.pad.playback_pending = false;
        self.pad.mode_song_offset = 0.0;
        self.pad.mode_wall_anchor = get_time();
        self.go(Page::SongSelect);
    }

    pub fn song_duration(&self) -> f32 {
        self.pad
            .audio_wav_pcm
            .as_ref()
            .map(|pcm| {
                pcm.samples.len() as f32 / f32::from(pcm.channels.max(1)) / pcm.sample_rate as f32
            })
            .unwrap_or(1.0)
            .max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(simai: &str) -> ChartDoc {
        crate::app::maidata::from_maidata(simai, None).expect("chart")
    }

    #[test]
    fn autoplay_events_press_and_hide_on_judgment() {
        let c = chart("&title=T\n&inote_1=(120){4}1,2,3,4\n");
        let events = autoplay_events_for(&c.notes, &c.bpms);
        // One press + one release per note.
        assert_eq!(events.len(), c.notes.len() * 2);
        let presses = events.iter().filter(|e| e.down).count();
        assert_eq!(presses, c.notes.len());
        // Taps hide at the head.
        assert!(events.iter().filter(|e| e.down).all(|e| e.hide.is_some()));
        assert!(events.windows(2).all(|w| w[0].t <= w[1].t), "sorted");
    }

    #[test]
    fn autoplay_due_fires_due_events() {
        let c = chart("&title=T\n&inote_1=(120){4}1,2,3,4\n");
        let events = autoplay_events_for(&c.notes, &c.bpms);
        let (cursor, due) = autoplay_due(&events, 0, 0.05);
        assert!(cursor >= 1);
        assert!(due.iter().any(|e| e.down), "the head press is due at t≈0");
    }

    #[test]
    fn autoplay_due_skips_stale_events() {
        let events = vec![
            AutoplayEvent { t: 0.0, zone: PadZone::from(1), down: true, hide: Some(1) },
            AutoplayEvent { t: 5.0, zone: PadZone::from(2), down: true, hide: Some(2) },
        ];
        let (cursor, due) = autoplay_due(&events, 0, 5.0);
        assert_eq!(cursor, 2);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].zone, PadZone::from(2));
    }

    #[test]
    fn autoplay_due_resyncs_after_backward_seek() {
        let events = vec![
            AutoplayEvent { t: 0.0, zone: PadZone::from(1), down: true, hide: Some(1) },
            AutoplayEvent { t: 1.0, zone: PadZone::from(2), down: true, hide: Some(2) },
        ];
        let (cursor, due) = autoplay_due(&events, 2, 0.0);
        assert_eq!(cursor, 1, "rewinds to the event at t=0");
        assert_eq!(due[0].zone, PadZone::from(1));
    }
}
