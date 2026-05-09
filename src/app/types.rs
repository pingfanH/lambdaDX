use macroquad::prelude::{TouchPhase, Vec2};
use serde::{Deserialize, Serialize};

pub(crate) const LANE_COUNT: usize = 9;
pub(crate) const LANE_LABELS: [&str; LANE_COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "T"];
pub(crate) const SCROLL_SPEED: f32 = 480.0;
pub(crate) const PREVIEW_LEAD_TIME: f32 = 1.6;
pub(crate) const HIT_WINDOW: f32 = 0.06;
pub(crate) const TAP_TRAVEL_TIME: f32 = 0.55;
pub(crate) const TOUCH_TRAVEL_TIME: f32 = 0.5;
pub(crate) const HOLD_TRAVEL_TIME: f32 = 0.55;
pub(crate) const TAP_GROW_FRAC: f32 = 0.35;
pub(crate) const TAP_SPAWN_FRAC: f32 = 0.3;
pub(crate) const TAP_DISAPPEAR_FRAC: f32 = 0.0;
pub(crate) const HOLD_DISAPPEAR_FRAC: f32 = 0.1;
pub(crate) const HOLD_FLY_TIME: f32 = 0.6;
pub(crate) const HOLD_TAIL_FLY_TIME: f32 = 0.40;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const HOLD_LENGTH_FRAC: f32 = 0.4;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const HOLD_LENGTH_FRAC: f32 = 0.6;

pub(crate) const HOLD_SPAWN_FRAC: f32 = 0.5;
pub(crate) const HOLD_TARGET_OFFSET: f32 = 40.0;
pub(crate) const TAP_TARGET_OFFSET: f32 = 15.;
// touch: base values (multiplied by TOUCH_SCALE in code)
pub(crate) const TOUCH_CROSS_SIZE: f32 = 50.0;
pub(crate) const TOUCH_START_DIST: f32 = 30.0;
pub(crate) const TOUCH_END_DIST: f32 = 10.0;
// touchhold: base values (multiplied by TOUCHHOLD_SCALE in code)
pub(crate) const TOUCHHOLD_CROSS_BASE: f32 = 86.0;
pub(crate) const TOUCHHOLD_BORDER_BASE: f32 = 170.0;
pub(crate) const TOUCHHOLD_START_DIST: f32 = 30.0;
pub(crate) const TOUCHHOLD_END_DIST: f32 = 19.0;
pub(crate) const TOUCHHOLD_ROT_OFFSET: f32 = 0.0;
pub(crate) const EACH_WINDOW: f32 = 0.02;
pub(crate) const TOUCH_GROW_FRAC: f32 = 0.25;
pub(crate) const TOUCH_DISAPPEAR_TIME: f32 = -0.1;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const TAP_SIZE: f32 = 40.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const HOLD_WIDTH: f32 = 40.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const TOUCH_SIZE: f32 = 18.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const TOUCH_SCALE: f32 = 1.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const TOUCHHOLD_SCALE: f32 = 0.6;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const TAP_SIZE: f32 = 80.0;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const HOLD_WIDTH: f32 = 80.0;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const TOUCH_SIZE: f32 = 70.0;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const TOUCH_SCALE: f32 = 1.5;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const TOUCHHOLD_SCALE: f32 = 1.0;

pub(crate) const PAD_ROTATION_RAD: f32 = std::f32::consts::FRAC_PI_8;
pub(crate) const TAP_RING_OFFSET: f32 = 14.;
pub(crate) const GRID_DIVISION: u32 = 64;
pub(crate) const SCROLL_SPEED_FACTOR: f32 = 0.01;
pub(crate) const SCROLL_INVERT: bool = true;

pub(crate) const SLIDE_TILE_SPACING: f32 = 20.0;
pub(crate) const SLIDE_TILE_SIZE: f32 = 40.0;
pub(crate) const SLIDE_TILE_SCALE: f32 = 0.4;
pub(crate) const SLIDE_MIN_POINTS: usize = 2;
pub(crate) const STAR_SIZE: f32 = 45.0;
pub(crate) const SLIDE_TRAVEL_TIME: f32 = 0.55;
pub(crate) const SLIDE_STAR_FADE_IN: f32 = 0.12;
pub(crate) const SPEED_MIN: f32 = 0.1;
pub(crate) const SPEED_MAX: f32 = 3.0;
pub(crate) const SPEED_STEP: f32 = 0.1;
pub(crate) const HOLD_RECORD_MIN_DURATION: f32 = 0.2;
pub(crate) const TOUCH_SPEED_MIN: f32 = 0.5;
pub(crate) const TOUCH_SPEED_MAX: f32 = 3.0;
pub(crate) const TOUCH_SPEED_STEP: f32 = 0.1;
pub(crate) const MOUSE_POINTER_ID: u64 = u64::MAX;
pub(crate) const PAD_B_START: u8 = 9;
pub(crate) const PAD_C_ZONE: u8 = 17;
pub(crate) const PAD_ZONE_MAX: u8 = 34;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteType {
    Tap,
    Touch,
    Hold,
    Slide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SlideShape {
    Line,
    Caret,
    Left,
    Right,
    VShape,
    P,
    Q,
    S,
    Z,
    PP,
    QQ,
    BigV,
    Wifi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SlidePoint {
    pub(crate) zone: u8,
    pub(crate) beat_offset: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Note {
    pub(crate) time: f32,
    pub(crate) lane: u8,
    pub(crate) note_type: NoteType,
    #[serde(default)]
    pub(crate) hold_duration: f32,
    #[serde(default)]
    pub(crate) is_each: bool,
    #[serde(default)]
    pub(crate) slide_points: Vec<SlidePoint>,
    #[serde(default)]
    pub(crate) slide_duration: f32,
    #[serde(default = "default_slide_start_delay")]
    pub(crate) slide_start_delay: f32,
    #[serde(default)]
    pub(crate) slide_shape: Option<SlideShape>,
}

fn default_slide_start_delay() -> f32 { SLIDE_STAR_FADE_IN }

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
    // TouchSpeedDown,
    // TouchSpeedUp,
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

#[derive(Debug, Clone)]
pub(crate) struct ActiveRecordHold {
    pub(crate) lane: u8,
    pub(crate) start_time: f32,
    pub(crate) slide_zones: Vec<(u8, f32)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PointerEvent {
    pub(crate) id: u64,
    pub(crate) phase: TouchPhase,
    pub(crate) position: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DragPart { Head, Body, Tail }

#[derive(Debug, Clone, Copy)]
pub(crate) struct PadFeedback {
    pub(crate) zone: u8,
    pub(crate) until: f64,
}

pub(crate) fn hold_tail_time(note: &Note) -> f32 {
    note.time + note.hold_duration.max(0.15)
}

pub(crate) fn sanitize_note_zone(_note_type: NoteType, lane: u8) -> u8 {
    lane.clamp(1, PAD_ZONE_MAX)
}

pub(crate) fn is_touch_zone(zone: u8) -> bool {
    zone >= PAD_B_START
}

pub(crate) fn slide_end_time(note: &Note) -> f32 {
    note.time + note.slide_duration.max(0.3)
}
