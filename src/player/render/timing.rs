//! Per-note timing derivation.
//!
//! Chart note fields are stored in measures; this module converts them to the
//! screen-timing values the renderer needs, in one place, so every note kind
//! (tap/hold/touch/slide) agrees on visibility and flight progress.
//!
//! See the module docs in `render/mod.rs` for the motion model these values feed.

use crate::app::types::{
    BpmChange, NOTE_LOCK_DISTANCE, NOTE_OUTER_DISTANCE, Note, NoteType, hold_tail_time,
    note_flight_speed, note_lead_time, note_secs, sanitize_note_zone, slide_end_time,
    touch_whole_duration,
};
use crate::player::state::PadPreviewState;

/// Timing snapshot for a single note on the current frame.
pub struct NoteTiming {
    /// Sanitized pad zone (1..=33).
    pub zone: u8,
    /// Note head time in seconds.
    pub ns: f32,
    /// Seconds until the head hits (`ns - current_t`).
    pub dt: f32,
    /// `dt` divided by play speed, so flight is speed-independent.
    pub dt_scaled: f32,
    /// Seconds until the hold/slide tail ends.
    pub tail_dt: f32,
    pub tail_dt_scaled: f32,
    /// Slide tail only; equals `tail_dt` for non-slides.
    pub slide_tail_dt: f32,
    /// Ring flight speed (base note speed × hi-speed), used by ring notes.
    pub speed: f32,
    /// Touch flight speed (`app.touch_speed` based), used by touch notes.
    pub touch_flight: f32,
    /// How long before the head the note first becomes visible.
    pub lead_time: f32,
    /// How long after the tail the note stays on screen before culling.
    pub disappear_time: f32,
    /// Play speed, kept here so helpers do not need to re-derive it.
    pub speed_scale: f32,
}

/// Derive [`NoteTiming`] for `note` at `current_t` seconds.
pub fn compute(
    note: &Note,
    app: &PadPreviewState,
    bpms: &[BpmChange],
    current_t: f32,
    speed_scale: f32,
) -> NoteTiming {
    let zone = sanitize_note_zone(note.note_type, note.lane);
    let ns = note_secs(note, bpms);
    let dt = ns - current_t;
    let dt_scaled = dt / speed_scale;

    // Hold and slide tails differ from the head; other note types reuse `dt`.
    let tail_dt = if matches!(note.note_type, NoteType::Hold) {
        hold_tail_time(note, bpms) - current_t
    } else {
        dt
    };
    let tail_dt_scaled = tail_dt / speed_scale;

    let speed = note_flight_speed(note, app.note_speed);
    let touch_flight = note_flight_speed(note, app.touch_speed);

    // Ring notes fly in over `note_lead_time`; touch/hold use the touch
    // whole-duration model; slides use the head's radial lead. Taps may appear
    // earlier when `tap_spawn_time` gives them a longer birth animation.
    let lead_time = if zone <= 8 {
        match note.note_type {
            NoteType::Tap => tap_lead_time(speed),
            _ => note_lead_time(speed),
        }
    } else {
        match note.note_type {
            NoteType::Touch | NoteType::Hold => touch_whole_duration(touch_flight),
            NoteType::Slide | NoteType::Tap => note_lead_time(speed),
        }
    };

    let slide_tail_dt = if matches!(note.note_type, NoteType::Slide) {
        slide_end_time(note, bpms) - current_t
    } else {
        tail_dt
    };

    // How long the note lingers after its tail time.
    // * Touch: uses its own (negative) constant.
    // * Hold: zero — the hold vanishes the instant its tail reaches the
    //   judgment ring.
    // * Slide: zero — the trail is consumed by the star and the note vanishes
    //   when the star reaches the tail (see `slide`).
    // * Tap: keeps flying past the ring for a moment (see `ring`).
    let disappear_time = match note.note_type {
        NoteType::Touch => crate::app::types::TOUCH_DISAPPEAR_TIME,
        NoteType::Hold | NoteType::Slide => 0.0,
        NoteType::Tap => 0.18,
    };

    NoteTiming {
        zone,
        ns,
        dt,
        dt_scaled,
        tail_dt,
        tail_dt_scaled,
        slide_tail_dt,
        speed,
        touch_flight,
        lead_time,
        disappear_time,
        speed_scale,
    }
}

/// Lead time for a tap. When `tap_spawn_time` is set, the tap appears that much
/// earlier so it can run its scale-up ("birth") animation before flying out.
fn tap_lead_time(speed: f32) -> f32 {
    let spawn_t = crate::app::params::tap_spawn_time();
    if spawn_t > 0.0 {
        (NOTE_OUTER_DISTANCE - NOTE_LOCK_DISTANCE) / speed.max(0.1) + spawn_t
    } else {
        note_lead_time(speed)
    }
}

impl NoteTiming {
    /// True while the note is inside its visible window.
    pub fn visible(&self) -> bool {
        self.slide_tail_dt >= -self.disappear_time && self.dt_scaled <= self.lead_time
    }

    /// The radius at which a ring note "locks" before travelling outward.
    pub fn lock_radius(&self, outer_r: f32, target_offset: f32) -> f32 {
        crate::app::types::note_lock_radius(outer_r, target_offset)
    }

    /// Outer travel distance constant, re-exported for hold drain math.
    pub const OUTER_DISTANCE: f32 = NOTE_OUTER_DISTANCE;
}
