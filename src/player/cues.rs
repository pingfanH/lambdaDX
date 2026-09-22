//! Time-based audio cues.
//!
//! The preview has no judgment engine, so cue sounds are driven purely by song
//! time: [`CueTrack`] holds a sorted list of cue instants (tap head, hold head,
//! hold tail, slide head) and a forward cursor. Each frame the caller advances
//! it to the current song time and plays a sound for every cue crossed.
//! Regression, seeking and restarting re-sync without firing, so cues never
//! double-play.

use crate::app::types::{ChartDoc, NoteType, hold_tail_time, note_secs};

/// What a cue corresponds to. All cues currently play the same sound, but the
/// kind is kept so different effects can be wired later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// A tap note reaching its judgment point.
    Tap,
    /// A hold note's head reaching its judgment point.
    HoldHead,
    /// A hold note's tail reaching its judgment point.
    HoldTail,
    /// A slide note's star head reaching its judgment point.
    SlideHead,
}

#[derive(Debug, Clone, Copy)]
pub struct CueEvent {
    pub time: f32,
    pub cue: Cue,
}

/// Sorted cue instants plus a monotonic cursor.
pub struct CueTrack {
    events: Vec<CueEvent>,
    cursor: usize,
    last_t: f32,
}

impl CueTrack {
    /// Build the cue list from a chart (tap head, hold head, hold tail, slide
    /// head).
    pub fn from_chart(chart: &ChartDoc) -> Self {
        let bpms = &chart.bpms;
        let mut events = Vec::new();
        for n in &chart.notes {
            match n.note_type {
                NoteType::Tap => events.push(CueEvent {
                    time: note_secs(n, bpms),
                    cue: Cue::Tap,
                }),
                NoteType::Hold => {
                    events.push(CueEvent {
                        time: note_secs(n, bpms),
                        cue: Cue::HoldHead,
                    });
                    events.push(CueEvent {
                        time: hold_tail_time(n, bpms),
                        cue: Cue::HoldTail,
                    });
                }
                NoteType::Slide => events.push(CueEvent {
                    time: note_secs(n, bpms),
                    cue: Cue::SlideHead,
                }),
                NoteType::Touch => {}
            }
        }
        events.sort_by(|a, b| a.time.total_cmp(&b.time));
        Self {
            events,
            cursor: 0,
            last_t: 0.0,
        }
    }

    /// Reposition to `t` without firing (on play / seek / restart).
    pub fn reset(&mut self, t: f32) {
        // First event strictly after `t`; everything at/before it is consumed.
        self.cursor = self.events.partition_point(|e| e.time <= t);
        self.last_t = t;
    }

    /// Advance to `t` and return how many cues were crossed since the last call.
    /// A backward jump just re-syncs (returns 0).
    pub fn take_due(&mut self, t: f32) -> usize {
        if t < self.last_t {
            self.reset(t);
            return 0;
        }
        let mut fired = 0;
        while self.cursor < self.events.len() && self.events[self.cursor].time <= t {
            if self.events[self.cursor].time > self.last_t {
                fired += 1;
            }
            self.cursor += 1;
        }
        self.last_t = t;
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::CueTrack;
    use crate::app::types::{BpmChange, ChartDoc, Note, NoteType};

    /// BPM 120 → one measure = 2 seconds, so measure `m` is at `(m - 1) * 2` s.
    fn chart_with_tap_and_hold() -> ChartDoc {
        ChartDoc {
            version: "test".into(),
            title: "t".into(),
            artist: String::new(),
            simai_level: 0,
            bpm: 120.0,
            bpms: vec![BpmChange {
                measure: 1.0,
                bpm: 120.0,
            }],
            audio_offset: 0.0,
            notes: vec![
                Note {
                    time: 2.0, // 2.0 s
                    lane: 1,
                    note_type: NoteType::Tap,
                    ..Default::default()
                },
                Note {
                    time: 3.0, // head 4.0 s
                    lane: 2,
                    note_type: NoteType::Hold,
                    hold_duration: 1.0, // tail at measure 4.0 = 6.0 s
                    ..Default::default()
                },
                Note {
                    time: 5.0, // star head 8.0 s
                    lane: 4,
                    note_type: NoteType::Slide,
                    ..Default::default()
                },
            ],
            templates: Vec::new(),
            template_instances: Vec::new(),
        }
    }

    #[test]
    fn cues_fire_once_when_crossed() {
        let mut track = CueTrack::from_chart(&chart_with_tap_and_hold());
        track.reset(0.0);

        assert_eq!(track.take_due(1.0), 0);
        assert_eq!(track.take_due(2.5), 1, "tap head");
        assert_eq!(track.take_due(2.5), 0, "no double fire");
        assert_eq!(track.take_due(4.5), 1, "hold head");
        assert_eq!(track.take_due(6.1), 1, "hold tail");
        assert_eq!(track.take_due(6.1), 0);
        assert_eq!(track.take_due(8.1), 1, "slide star head");
        assert_eq!(track.take_due(8.1), 0);
    }

    #[test]
    fn reset_skips_past_cues_and_backward_seek_resyncs() {
        let mut track = CueTrack::from_chart(&chart_with_tap_and_hold());

        // Starting at 3.0 s consumes the tap and lets the hold + slide cues fire.
        track.reset(3.0);
        assert_eq!(track.take_due(8.5), 3);

        // Seeking backward re-syncs instead of firing everything.
        assert_eq!(track.take_due(1.0), 0);
        assert_eq!(track.take_due(2.5), 1, "tap fires again after rewind");
    }
}
