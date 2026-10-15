use crate::theme;
use egui_macroquad::egui::Color32;

pub const LENGTH: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Tap,
    Hold,
    Slide,
    Touch,
}

impl Kind {
    pub const ALL: [Self; 4] = [Self::Tap, Self::Hold, Self::Slide, Self::Touch];
    pub fn label(self) -> &'static str {
        match self {
            Self::Tap => "Tap",
            Self::Hold => "Hold",
            Self::Slide => "Slide",
            Self::Touch => "Touch",
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            Self::Tap => theme::PINK,
            Self::Hold => theme::GOLD,
            Self::Slide => theme::BLUE,
            Self::Touch => theme::GREEN,
        }
    }
}

#[derive(Clone)]
pub struct Note {
    pub id: u64,
    pub kind: Kind,
    pub beat: f32,
    pub lane: u8,
    pub end_lane: u8,
    pub duration: f32,
    pub each: bool,
    pub events: Vec<Event>,
}

#[derive(Clone)]
pub struct Event {
    pub sensor: String,
    pub start: f32,
    pub end: f32,
}

impl Note {
    pub fn new(id: u64, kind: Kind, beat: f32, lane: u8, duration: f32) -> Self {
        let end_lane = (lane + 4) % 8;
        let events = match kind {
            Kind::Slide => vec![
                Event {
                    sensor: format!("A{}", lane + 1),
                    start: 0.0,
                    end: 0.16,
                },
                Event {
                    sensor: format!("B{}", lane + 1),
                    start: 0.45,
                    end: 0.45 + duration * 0.33,
                },
                Event {
                    sensor: "C".into(),
                    start: duration * 0.4,
                    end: duration * 0.7,
                },
                Event {
                    sensor: format!("B{}", end_lane + 1),
                    start: duration * 0.65,
                    end: duration * 0.9,
                },
                Event {
                    sensor: format!("A{}", end_lane + 1),
                    start: duration * 0.85,
                    end: duration,
                },
            ],
            _ => vec![Event {
                sensor: format!(
                    "{}{}",
                    if kind == Kind::Touch { "B" } else { "A" },
                    lane + 1
                ),
                start: 0.0,
                end: duration.max(0.18),
            }],
        };
        Self {
            id,
            kind,
            beat,
            lane,
            end_lane,
            duration,
            each: false,
            events,
        }
    }
    pub fn end(&self) -> f32 {
        self.beat + self.duration.max(0.2)
    }
}

#[derive(Clone)]
struct Snapshot {
    notes: Vec<Note>,
    selected: Option<u64>,
    label: String,
}

pub struct Demo {
    pub notes: Vec<Note>,
    pub selected: Option<u64>,
    pub selected_event: Option<usize>,
    pub beat: f32,
    pub cursor: f32,
    pub playing: bool,
    pub speed: f32,
    pub bpm: f32,
    pub looping: bool,
    pub tool: Option<Kind>,
    pub snap: bool,
    pub division: u32,
    pub show_grid: bool,
    pub show_sensors: bool,
    pub note_scale: f32,
    pub translucent: bool,
    pub history: Vec<String>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    next_id: u64,
}

impl Default for Demo {
    fn default() -> Self {
        let mut notes = Vec::new();
        for section in 0..4 {
            let b = section as f32 * 16.0;
            for (kind, offset, lane, length) in [
                (Kind::Tap, 0.0, 0, 0.0),
                (Kind::Tap, 1.0, 2, 0.0),
                (Kind::Hold, 2.0, 4, 2.0),
                (Kind::Touch, 3.0, 6, 0.0),
                (Kind::Tap, 4.0, 1, 0.0),
                (Kind::Tap, 4.0, 5, 0.0),
                (Kind::Touch, 5.0, 3, 0.0),
                (Kind::Tap, 6.0, 7, 0.0),
                (Kind::Slide, 8.0, 1, 4.0),
                (Kind::Hold, 8.0, 5, 3.0),
                (Kind::Tap, 9.5, 3, 0.0),
                (Kind::Touch, 10.5, 6, 0.0),
                (Kind::Tap, 12.0, 0, 0.0),
                (Kind::Tap, 12.0, 4, 0.0),
                (Kind::Slide, 13.0, 6, 2.0),
                (Kind::Tap, 15.0, 2, 0.0),
            ] {
                let mut note = Note::new(notes.len() as u64 + 1, kind, b + offset, lane, length);
                note.each = offset == 4.0 || offset == 8.0 || offset == 12.0;
                notes.push(note);
            }
        }
        Self {
            notes,
            selected: Some(25),
            selected_event: None,
            beat: 24.0,
            cursor: 24.0,
            playing: false,
            speed: 1.0,
            bpm: 150.0,
            looping: true,
            tool: None,
            snap: true,
            division: 4,
            show_grid: true,
            show_sensors: false,
            note_scale: 1.0,
            translucent: true,
            history: vec!["Demo session".into()],
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 65,
        }
    }
}

impl Demo {
    pub fn tick(&mut self, dt: f32) {
        if self.playing {
            self.beat += dt * self.bpm / 60.0 * self.speed;
            if self.beat > LENGTH {
                if self.looping {
                    self.beat %= LENGTH;
                } else {
                    self.beat = LENGTH;
                    self.playing = false;
                }
            }
        }
    }
    pub fn snapped(&self, beat: f32) -> f32 {
        let beat = beat.clamp(0.0, LENGTH);
        if self.snap {
            (beat * self.division as f32).round() / self.division as f32
        } else {
            beat
        }
    }
    pub fn selected(&self) -> Option<&Note> {
        self.notes.iter().find(|n| Some(n.id) == self.selected)
    }
    pub fn select(&mut self, id: Option<u64>) {
        self.selected = id;
        self.selected_event = None;
    }
    pub fn checkpoint(&mut self, label: &str) {
        self.undo.push(Snapshot {
            notes: self.notes.clone(),
            selected: self.selected,
            label: label.into(),
        });
        self.redo.clear();
        self.history.push(label.into());
    }
    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snapshot {
                notes: self.notes.clone(),
                selected: self.selected,
                label: s.label.clone(),
            });
            self.notes = s.notes;
            self.selected = s.selected;
            self.selected_event = None;
            self.history.push(format!("Undo: {}", s.label));
        }
    }
    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snapshot {
                notes: self.notes.clone(),
                selected: self.selected,
                label: s.label.clone(),
            });
            self.notes = s.notes;
            self.selected = s.selected;
            self.selected_event = None;
            self.history.push(format!("Redo: {}", s.label));
        }
    }
    pub fn delete(&mut self) {
        if let Some(id) = self.selected {
            self.checkpoint("Delete note");
            self.notes.retain(|n| n.id != id);
            self.select(None);
        }
    }
    pub fn duplicate(&mut self) {
        if let Some(mut note) = self.selected().cloned() {
            self.checkpoint("Duplicate note");
            note.id = self.next_id;
            self.next_id += 1;
            note.beat = (note.end() + 0.5).min(LENGTH - note.duration);
            self.select(Some(note.id));
            self.notes.push(note);
        }
    }
    pub fn place(&mut self, kind: Kind, beat: f32, lane: u8) {
        self.checkpoint("Place note");
        let duration = match kind {
            Kind::Hold => 2.0,
            Kind::Slide => 4.0,
            _ => 0.0,
        };
        let note = Note::new(self.next_id, kind, self.snapped(beat), lane, duration);
        self.select(Some(note.id));
        self.next_id += 1;
        self.notes.push(note);
    }
    pub fn next(&mut self, reverse: bool) {
        let mut ids: Vec<_> = self.notes.iter().map(|n| (n.beat, n.id)).collect();
        ids.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if ids.is_empty() {
            return;
        }
        let at = ids
            .iter()
            .position(|n| Some(n.1) == self.selected)
            .unwrap_or(ids.len() - 1);
        let next = if reverse {
            (at + ids.len() - 1) % ids.len()
        } else {
            (at + 1) % ids.len()
        };
        self.select(Some(ids[next].1));
        self.cursor = ids[next].0;
        self.beat = self.cursor;
    }
}
