use macroquad::prelude::{vec2, TouchPhase, Vec2};
use serde::{Deserialize, Serialize};

pub(crate) const LANE_COUNT: usize = 9;
pub(crate) const LANE_LABELS: [&str; LANE_COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "T"];
pub(crate) const SCROLL_SPEED: f32 = 480.0;
pub(crate) const PREVIEW_LEAD_TIME: f32 = 1.6;
pub(crate) const HIT_WINDOW: f32 = 0.06;
pub(crate) const TAP_TRAVEL_TIME: f32 = 0.30;
pub(crate) const PAD_ROTATION_RAD: f32 = std::f32::consts::FRAC_PI_4;
pub(crate) const SPEED_MIN: f32 = 0.1;
pub(crate) const SPEED_MAX: f32 = 3.0;
pub(crate) const SPEED_STEP: f32 = 0.1;
pub(crate) const HOLD_RECORD_MIN_DURATION: f32 = 0.12;
pub(crate) const MOUSE_POINTER_ID: u64 = u64::MAX;
pub(crate) const PAD_ZONE_COUNT: usize = 33;
pub(crate) const PAD_B_START: u8 = 9;
pub(crate) const PAD_C_ZONE: u8 = 17;
pub(crate) const PAD_D_START: u8 = 18;
pub(crate) const PAD_E_START: u8 = 26;
pub(crate) const PAD_ZONE_MAX: u8 = 33;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteType {
    Tap,
    Touch,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Note {
    pub(crate) time: f32,
    pub(crate) lane: u8,
    pub(crate) note_type: NoteType,
    #[serde(default)]
    pub(crate) hold_duration: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChartDoc {
    pub(crate) version: String,
    pub(crate) title: String,
    pub(crate) bpm: f32,
    pub(crate) notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HitEvent {
    pub(crate) time: f32,
    pub(crate) lane: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecordingDoc {
    pub(crate) created_at_epoch_ms: u128,
    pub(crate) source: String,
    pub(crate) chart: ChartDoc,
    pub(crate) hits: Vec<HitEvent>,
    pub(crate) record_speed: f32,
    pub(crate) play_speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Idle,
    Recording,
    Playing,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RectF {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    pub(crate) header: RectF,
    pub(crate) timeline: Option<RectF>,
    pub(crate) pad: RectF,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PadGeom {
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    pub(crate) outer_r: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum UiAction {
    TogglePlay,
    ToggleRecord,
    Save,
    Load,
    Clear,
    ToggleAudio,
    RecSpeedDown,
    RecSpeedUp,
    PlaySpeedDown,
    PlaySpeedUp,
    TogglePadOnly,
    ToggleMobileUi,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UiButton {
    pub(crate) rect: RectF,
    pub(crate) label: &'static str,
    pub(crate) action: UiAction,
}

#[derive(Debug, Clone)]
pub(crate) struct WavPcm {
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    pub(crate) samples: Vec<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RecordInputId {
    Key(u8),
    Pointer(u64),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveRecordHold {
    pub(crate) lane: u8,
    pub(crate) start_time: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PointerEvent {
    pub(crate) id: u64,
    pub(crate) phase: TouchPhase,
    pub(crate) position: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PadFeedback {
    pub(crate) zone: u8,
    pub(crate) until: f64,
}

pub(crate) fn hold_tail_time(note: &Note) -> f32 {
    note.time + note.hold_duration.max(0.15)
}

pub(crate) fn sanitize_note_zone(note_type: NoteType, lane: u8) -> u8 {
    // Backward compatibility: historical Touch lane 9 means center.
    if matches!(note_type, NoteType::Touch) && lane == 9 {
        return PAD_C_ZONE;
    }
    lane.clamp(1, PAD_ZONE_MAX)
}

pub(crate) fn is_touch_zone(zone: u8) -> bool {
    zone >= PAD_B_START
}

pub(crate) fn pad_zone_center(zone: u8, pad: PadGeom) -> Option<Vec2> {
    let base = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD;
    let sec = std::f32::consts::TAU / 8.0;
    let mut by_ring = |idx: u8, rr: f32| -> Vec2 {
        let i = idx.saturating_sub(1) as f32;
        let ang = base + i * sec;
        vec2(pad.cx + ang.cos() * rr, pad.cy + ang.sin() * rr)
    };

    match zone {
        1..=8 => Some(by_ring(zone, pad.outer_r * 0.96)),
        9..=16 => Some(by_ring(zone - 8, pad.outer_r * 0.56)),
        PAD_C_ZONE => Some(vec2(pad.cx, pad.cy)),
        18..=25 => Some(by_ring(zone - 17, pad.outer_r * 0.40)),
        26..=33 => Some(by_ring(zone - 25, pad.outer_r * 0.26)),
        _ => None,
    }
}

pub(crate) fn pad_zone_from_point(p: Vec2, pad: PadGeom) -> Option<u8> {
    let dx = p.x - pad.cx;
    let dy = p.y - pad.cy;
    let dist = (dx * dx + dy * dy).sqrt();

    let c_r = pad.outer_r * 0.18;
    let e_i = c_r;
    let e_o = pad.outer_r * 0.31;
    let d_o = pad.outer_r * 0.44;
    let b_o = pad.outer_r * 0.62;
    let a_i = b_o;
    let a_o = pad.outer_r * 1.05;

    if dist <= c_r {
        return Some(PAD_C_ZONE);
    }
    if dist < e_i || dist > a_o {
        return None;
    }

    let ang = dy.atan2(dx);
    let base = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD;
    let sector = std::f32::consts::TAU / 8.0;
    let delta = (ang - base).rem_euclid(std::f32::consts::TAU);
    let idx = (delta / sector).floor() as u8 + 1;

    if dist <= e_o {
        Some(PAD_E_START + idx - 1)
    } else if dist <= d_o {
        Some(PAD_D_START + idx - 1)
    } else if dist <= b_o {
        Some(PAD_B_START + idx - 1)
    } else if dist >= a_i {
        Some(idx)
    } else {
        None
    }
}
