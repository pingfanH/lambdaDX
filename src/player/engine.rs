//! Judgment engine bridge to `lnmai-core-rs` (FFI build).
//!
//! The chart is loaded into a persistent Lean runtime session; each gameplay
//! frame the player feeds a [`TimedInputBatch`] (button/sensor presses at the
//! current song time in µs) and reads back the resulting judge events. The
//! player no longer computes judge windows itself.

use std::sync::OnceLock;

use lambda_dx::app::types::zone::PadZone;
use lnmai_core_rs::session::{self, Empty, Loaded, Session};
use lnmai_core_rs::types::{
    ButtonZone, JudgeEvent, JudgeEventKind, JudgeGrade, SensorArea, TimedInputBatch, TimedInputEvent,
};

/// Map a pad zone id (1-8 = A buttons, 9-33 = sensors) to lnmai input events.
/// Returns `(click_event, hold_down, hold_up)` for a press/release cycle.
pub fn events_for_zone(
    zone: PadZone,
    tp: i64,
) -> Option<(TimedInputEvent, TimedInputEvent, TimedInputEvent)> {
    let zid = zone.to_id();
    let (click, hold_down, hold_up) = if zid <= 8 {
        let btn = match zid {
            1 => ButtonZone::K1,
            2 => ButtonZone::K2,
            3 => ButtonZone::K3,
            4 => ButtonZone::K4,
            5 => ButtonZone::K5,
            6 => ButtonZone::K6,
            7 => ButtonZone::K7,
            8 => ButtonZone::K8,
            _ => return None,
        };
        (
            TimedInputEvent::ButtonClick { tp, zone: btn },
            TimedInputEvent::ButtonHold {
                tp,
                zone: btn,
                is_down: true,
            },
            TimedInputEvent::ButtonHold {
                tp,
                zone: btn,
                is_down: false,
            },
        )
    } else {
        let area = match zid {
            9..=16 => match zid - 8 {
                1 => SensorArea::B1,
                2 => SensorArea::B2,
                3 => SensorArea::B3,
                4 => SensorArea::B4,
                5 => SensorArea::B5,
                6 => SensorArea::B6,
                7 => SensorArea::B7,
                8 => SensorArea::B8,
                _ => return None,
            },
            17 => SensorArea::C,
            18..=25 => match zid - 17 {
                1 => SensorArea::D1,
                2 => SensorArea::D2,
                3 => SensorArea::D3,
                4 => SensorArea::D4,
                5 => SensorArea::D5,
                6 => SensorArea::D6,
                7 => SensorArea::D7,
                8 => SensorArea::D8,
                _ => return None,
            },
            26..=33 => match zid - 25 {
                1 => SensorArea::E1,
                2 => SensorArea::E2,
                3 => SensorArea::E3,
                4 => SensorArea::E4,
                5 => SensorArea::E5,
                6 => SensorArea::E6,
                7 => SensorArea::E7,
                8 => SensorArea::E8,
                _ => return None,
            },
            _ => return None,
        };
        (
            TimedInputEvent::SensorClick { tp, area },
            TimedInputEvent::SensorHold {
                tp,
                area,
                is_down: true,
            },
            TimedInputEvent::SensorHold {
                tp,
                area,
                is_down: false,
            },
        )
    };
    Some((click, hold_down, hold_up))
}

/// Convert a button zone back to a [`PadZone`] (for engine judge feedback).
pub fn zone_for_button(btn: ButtonZone) -> PadZone {
    let id = match btn {
        ButtonZone::K1 => 1,
        ButtonZone::K2 => 2,
        ButtonZone::K3 => 3,
        ButtonZone::K4 => 4,
        ButtonZone::K5 => 5,
        ButtonZone::K6 => 6,
        ButtonZone::K7 => 7,
        ButtonZone::K8 => 8,
    };
    PadZone::from(id)
}

/// Convert a sensor area to a pad zone id (1-8 A, 9-16 B, 17 C, 18-25 D, 26-33 E).
pub fn zone_for_sensor(area: SensorArea) -> PadZone {
    let id = match area {
        SensorArea::A1 => 1,
        SensorArea::A2 => 2,
        SensorArea::A3 => 3,
        SensorArea::A4 => 4,
        SensorArea::A5 => 5,
        SensorArea::A6 => 6,
        SensorArea::A7 => 7,
        SensorArea::A8 => 8,
        SensorArea::B1 => 9,
        SensorArea::B2 => 10,
        SensorArea::B3 => 11,
        SensorArea::B4 => 12,
        SensorArea::B5 => 13,
        SensorArea::B6 => 14,
        SensorArea::B7 => 15,
        SensorArea::B8 => 16,
        SensorArea::C => 17,
        SensorArea::D1 => 18,
        SensorArea::D2 => 19,
        SensorArea::D3 => 20,
        SensorArea::D4 => 21,
        SensorArea::D5 => 22,
        SensorArea::D6 => 23,
        SensorArea::D7 => 24,
        SensorArea::D8 => 25,
        SensorArea::E1 => 26,
        SensorArea::E2 => 27,
        SensorArea::E3 => 28,
        SensorArea::E4 => 29,
        SensorArea::E5 => 30,
        SensorArea::E6 => 31,
        SensorArea::E7 => 32,
        SensorArea::E8 => 33,
    };
    PadZone::from(id)
}

/// A loaded lnmai-core (FFI) runtime session.
pub struct JudgeEngine {
    session: Session<Loaded>,
}

fn ensure_runtime() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| unsafe {
        session::initialize_runtime()
            .expect("lnmai-core runtime must initialize");
    });
}

impl JudgeEngine {
    /// Create a session and load the chart at `level_index` from Simai text.
    pub fn load(simai_text: &str, level_index: u32) -> Result<Self, String> {
        ensure_runtime();
        let empty = Session::<Empty>::create().map_err(|e| e.json)?;
        let (loaded, _envelope) = empty
            .load_chart_text(simai_text, level_index)
            .map_err(|e| e.json)?;
        Ok(JudgeEngine { session: loaded })
    }

    /// Advance the runtime by one frame with the given input events at
    /// `current_secs` (song time in seconds, converted to µs ticks). Returns
    /// the judge events produced this frame.
    pub fn step(
        &mut self,
        current_secs: f32,
        mut events: Vec<TimedInputEvent>,
    ) -> Result<Vec<JudgeEvent>, String> {
        events.sort_by_key(|e| e.tp());
        let batch = TimedInputBatch {
            current_time: (current_secs.max(0.0) * 1e6) as i64,
            events,
        };
        let json = serde_json::to_string(&batch).map_err(|e| e.to_string())?;
        let envelope = self.session.advance_frame_light(&json).map_err(|e| e.json)?;
        let value: serde_json::Value =
            serde_json::from_str(&envelope.json).map_err(|e| e.to_string())?;
        let events_json = value
            .get("result")
            .and_then(|r| r.get("events"))
            .cloned()
            .unwrap_or_default();
        serde_json::from_value(events_json).map_err(|e| e.to_string())
    }
}

trait InputTp {
    fn tp(&self) -> i64;
}
impl InputTp for TimedInputEvent {
    fn tp(&self) -> i64 {
        match self {
            TimedInputEvent::ButtonClick { tp, .. }
            | TimedInputEvent::ButtonHold { tp, .. }
            | TimedInputEvent::SensorClick { tp, .. }
            | TimedInputEvent::SensorHold { tp, .. } => *tp,
        }
    }
}

use lambda_dx::app::types::note_secs;
use lambda_dx::app::types::sanitize_note_zone;
use lambda_dx::app::types::{NoteType, mdur_to_secs};

fn tp_at(secs: f32) -> i64 {
    (secs.max(0.0) * 1e6) as i64
}

/// Feed autoplay clicks for notes whose judge time has passed, then advance the
/// engine and apply the resulting judge events to the player's feedback.
pub fn step_judge_engine(app: &mut crate::state::PlayerState) {
    if app.judge_engine.is_none() {
        return;
    }

    if app.autoplay {
        let now = app.song_time();
        let bpms = app.chart.bpms.clone();
        for note in &app.chart.notes {
            let ns = note_secs(note, &bpms);
            match note.note_type {
                NoteType::Slide => {
                    // Feed sensor clicks along each slide's waypoint zones so
                    // the engine can judge slides in autoplay.
                    for sl in &note.slide {
                        let start = ns + mdur_to_secs(sl.slide_start_delay, note.time, &bpms);
                        let dur = mdur_to_secs(sl.slide_duration, note.time, &bpms).max(0.05);
                        let mut zones = vec![note.lane];
                        for seg in &sl.segments {
                            for p in &seg.points {
                                let zid = p.zone.to_id();
                                if zones.last() != Some(&zid) {
                                    zones.push(zid);
                                }
                            }
                        }
                        for (k, zid) in zones.iter().enumerate() {
                            if !app.auto_slide_sensors.contains(&(note.id, *zid)) {
                                let frac = if zones.len() <= 1 {
                                    0.0
                                } else {
                                    k as f32 / (zones.len() - 1) as f32
                                };
                                let t = start + dur * frac;
                                if t <= now + 0.02 {
                                    if let Some((click, hold_down, _)) =
                                        events_for_zone(PadZone::from(*zid), tp_at(t))
                                    {
                                        app.engine_events.push(click);
                                        app.engine_events.push(hold_down);
                                    }
                                    app.auto_slide_sensors.insert((note.id, *zid));
                                }
                            }
                        }
                    }
                }
                _ => {
                    if app.auto_judged.contains(&note.id) {
                        continue;
                    }
                    if ns <= now + 0.02 {
                        let zone = PadZone::from(sanitize_note_zone(note.note_type, note.lane));
                        if let Some((click, hold_down, _)) = events_for_zone(zone, tp_at(ns)) {
                            app.engine_events.push(click);
                            app.engine_events.push(hold_down);
                        }
                        app.auto_judged.insert(note.id);
                    }
                }
            }
        }
    }

    let now = app.song_time();
    let events = std::mem::take(&mut app.engine_events);
    let result = app.judge_engine.as_mut().unwrap().step(now, events);
    match result {
        Ok(events) => handle_engine_events(app, events),
        Err(e) => {
            if !app.status.starts_with("判引擎") {
                app.set_status(format!("engine: {e}"));
            }
        }
    }
}

fn handle_engine_events(app: &mut crate::state::PlayerState, events: Vec<JudgeEvent>) {
    let now = app.song_time();
    let bpms = app.chart.bpms.clone();
    for ev in events {
        let is_slide = ev.kind == JudgeEventKind::Slide;
        let pos_zone = || {
            if let Some(b) = ev.position.button {
                Some(zone_for_button(b))
            } else if let Some(s) = ev.position.sensor {
                Some(zone_for_sensor(s))
            } else {
                None
            }
        };
        // The engine reports the slide's head button; show slide feedback at
        // the slide's tail zone with the slide color instead.
        let zone = if is_slide {
            find_slide_tail_zone(app, now, &bpms).or_else(pos_zone)
        } else {
            pos_zone()
        };
        let Some(zone) = zone else {
            continue;
        };
        // Raw lnmai grade string (e.g. "Perfect", "LatePerfect2nd", "Miss"),
        // matching the 8464c6f judge display.
        let label = format!("{:?}", ev.grade);
        let is_miss = ev.grade.is_miss_or_too_fast();
        let duration = if is_miss { 0.24 } else { 0.3 };
        if is_slide {
            eprintln!(
                "[slide] engine slide event at t={now:.3} zone={:?} kind={:?} grade={label}",
                zone, ev.kind
            );
        }
        app.push_judgement_colored(
            zone,
            &label,
            duration,
            macroquad::prelude::Color::from_rgba(255, 255, 255, 255),
        );

        if !is_miss {
            let sfx = match ev.kind {
                JudgeEventKind::Break => app.sfx_break_tap.as_ref(),
                JudgeEventKind::Touch => app.sfx_touch.as_ref(),
                JudgeEventKind::Slide => app.sfx_slide.as_ref(),
                _ => app.sfx_tap.as_ref(),
            };
            if let (Some(s), Some(player)) = (sfx, &mut app.sfx_player) {
                player.play(s, 1.0);
            }
        }
    }
}

/// Find the tail zone of the slide note currently in its run window (used to
/// place slide judge feedback).
fn find_slide_tail_zone(
    app: &crate::state::PlayerState,
    now: f32,
    bpms: &[lambda_dx::app::types::BpmChange],
) -> Option<PadZone> {
    use lambda_dx::app::types::{NoteType, slide_end_time};
    app.chart
        .notes
        .iter()
        .filter(|n| matches!(n.note_type, NoteType::Slide))
        .filter(|n| {
            let start = note_secs(n, bpms);
            let end = slide_end_time(n, bpms);
            // Judge events fire near the slide end; allow a small grace window.
            now >= start - 0.1 && now <= end + 1.0
        })
        .min_by(|a, b| {
            let da = (slide_end_time(a, bpms) - now).abs();
            let db = (slide_end_time(b, bpms) - now).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|n| {
            n.slide
                .last()
                .and_then(|sl| sl.segments.last())
                .and_then(|seg| seg.points.last())
                .map(|p| p.zone)
        })
}
