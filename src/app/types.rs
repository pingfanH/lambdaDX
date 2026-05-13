use macroquad::prelude::{TouchPhase, Vec2};
use serde::{Deserialize, Serialize};

pub(crate) const LANE_COUNT: usize = 9;
pub(crate) const LANE_LABELS: [&str; LANE_COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "T"];
pub(crate) const SCROLL_SPEED: f32 = 480.0;
pub(crate) const PREVIEW_LEAD_TIME: f32 = 1.6;
pub(crate) const HIT_WINDOW: f32 = 0.00;
pub(crate) const TAP_TRAVEL_TIME: f32 = 0.55;
pub(crate) const TOUCH_TRAVEL_TIME: f32 = 0.5;
pub(crate) const HOLD_TRAVEL_TIME: f32 = 0.55;
pub const TAP_GROW_FRAC: f32 = 0.35;
pub const TAP_SPAWN_FRAC: f32 = 0.3;
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
pub const TAP_TARGET_OFFSET: f32 = 15.;
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

pub const PAD_ROTATION_RAD: f32 = std::f32::consts::FRAC_PI_8;
pub const TAP_RING_OFFSET: f32 = 14.;
pub const GRID_DIVISION: u32 = 64;
pub(crate) const SCROLL_SPEED_FACTOR: f32 = 0.01;
pub(crate) const SCROLL_INVERT: bool = true;

pub const SLIDE_TILE_SPACING: f32 = 20.0;
pub const SLIDE_TILE_SIZE: f32 = 40.0;
pub const SLIDE_TILE_SCALE: f32 = 0.4;
pub const SLIDE_MIN_POINTS: usize = 2;
pub const STAR_SIZE: f32 = 45.0;
pub const SLIDE_TRAVEL_TIME: f32 = 0.55;
pub const SLIDE_STAR_FADE_IN: f32 = 0.12;
pub(crate) const SPEED_MIN: f32 = 0.1;
pub(crate) const SPEED_MAX: f32 = 3.0;
pub(crate) const SPEED_STEP: f32 = 0.1;
pub(crate) const HOLD_RECORD_MIN_DURATION: f32 = 0.2;
pub(crate) const TOUCH_SPEED_MIN: f32 = 0.5;
pub(crate) const TOUCH_SPEED_MAX: f32 = 3.0;
pub(crate) const TOUCH_SPEED_STEP: f32 = 0.1;
pub(crate) const MOUSE_POINTER_ID: u64 = u64::MAX;
pub const PAD_B_START: u8 = 9;
pub const PAD_C_ZONE: u8 = 17;
pub const PAD_ZONE_MAX: u8 = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    Tap,
    Touch,
    Hold,
    Slide,
}
impl Default for NoteType {
    fn default() -> Self {
        NoteType::Tap
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideShape {
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
pub struct SlidePoint {
    pub zone: u8,
    pub beat_offset: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub segments: Vec<SlideSegment>,
    /// Total slide span in measures (head → tail) for this individual slide.
    pub slide_duration: f32,
    /// Delay before slide motion starts, in measures.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub slide_start_delay: f32,
    /// Whether this slide trail is a break slide.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub slide_is_break: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideSegment {
    pub points: Vec<SlidePoint>,
    pub shape: SlideShape,
}


fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

/// Note times and durations are stored in **measures** (where measure 1.0 =
/// the first beat of the song).  Use `measure_to_secs` / `mdur_to_secs` to
/// convert to wall-clock seconds for playback and rendering.
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct Note {
    /// Measure position (1.0 = first beat).
    pub time: f32,
    pub lane: u8,
    pub note_type: NoteType,
    /// Duration in measures.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub hold_duration: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_each: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_break: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_ex: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_star: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_tapless: bool,
    pub slide: Vec<Slide>
}

// ─── BPM change list ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmChange {
    pub measure: f32,
    pub bpm: f32,
}

// ─── Measure ↔ Seconds conversion (multi-BPM) ────────────────────

/// Measure position → seconds.  `m = 1.0` ⇒ `t = 0`.
/// Accounts for BPM changes in the list.
pub fn measure_to_secs(m: f32, bpms: &[BpmChange]) -> f32 {
    if bpms.is_empty() {
        return (m - 1.0) * 2.0; // fallback 120 BPM
    }
    let mut t = 0.0_f32;
    let mut prev_m = bpms[0].measure;
    let mut cur_bpm = bpms[0].bpm;
    for b in bpms.iter().skip(1) {
        if b.measure >= m {
            break;
        }
        t += (b.measure - prev_m) * 240.0 / cur_bpm;
        prev_m = b.measure;
        cur_bpm = b.bpm;
    }
    t += (m - prev_m) * 240.0 / cur_bpm;
    t
}

/// Seconds → measure position (inverse of `measure_to_secs`).
pub fn secs_to_measure(t: f32, bpms: &[BpmChange]) -> f32 {
    if bpms.is_empty() {
        return t * 0.5 + 1.0; // fallback 120 BPM
    }
    let mut remaining = t;
    let mut prev_m = bpms[0].measure;
    let mut cur_bpm = bpms[0].bpm;
    for b in bpms.iter().skip(1) {
        let section_dur = (b.measure - prev_m) * 240.0 / cur_bpm;
        if remaining <= section_dur {
            return prev_m + remaining * cur_bpm / 240.0;
        }
        remaining -= section_dur;
        prev_m = b.measure;
        cur_bpm = b.bpm;
    }
    prev_m + remaining * cur_bpm / 240.0
}

/// Duration in measures → duration in seconds, starting at measure `start_m`.
pub fn mdur_to_secs(d: f32, start_m: f32, bpms: &[BpmChange]) -> f32 {
    measure_to_secs(start_m + d, bpms) - measure_to_secs(start_m, bpms)
}

/// Duration in seconds → duration in measures, starting at `start_secs`.
pub(crate) fn sdur_to_mdur(d: f32, start_secs: f32, bpms: &[BpmChange]) -> f32 {
    secs_to_measure(start_secs + d, bpms) - secs_to_measure(start_secs, bpms)
}

/// BPM in effect at the given measure position.
pub(crate) fn bpm_at(m: f32, bpms: &[BpmChange]) -> f32 {
    let mut bpm = bpms.first().map(|b| b.bpm).unwrap_or(120.0);
    for b in bpms {
        if b.measure > m + 0.0001 { break; }
        bpm = b.bpm;
    }
    bpm
}

/// Snap a measure value to the nearest 1/384 grid position.
pub(crate) fn snap_measure(m: f32) -> f32 {
    const GRID: f32 = 384.0;
    (m * GRID).round() / GRID
}

/// Note head time in seconds.
pub fn note_secs(note: &Note, bpms: &[BpmChange]) -> f32 {
    measure_to_secs(note.time, bpms)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChartDoc {
    pub(crate) version: String,
    pub(crate) title: String,
    pub(crate) bpm: f32,
    /// Sorted list of BPM changes. First entry starts at measure 1.0.
    #[serde(default)]
    pub(crate) bpms: Vec<BpmChange>,
    /// Seconds from audio start to the first beat (Simai `&first`).  When
    /// non-zero the audio playback position is shifted by this amount so that
    /// `song_time == 0` aligns with this point in the audio file.
    #[serde(default)]
    pub(crate) audio_offset: f32,
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
pub struct PadGeom {
    pub cx: f32,
    pub cy: f32,
    pub outer_r: f32,
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
pub(crate) enum DragPart { Head, Body, Tail, SlideDelayEnd }

/// Currently selected tool in the timeline-left sidebar (Blender-style N-panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaceTool { Tap, Hold, Star }

/// Multi-step placement state machine driven by clicks on the timeline.
/// Tap is single-shot, so it has no in-progress state.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PlacementState {
    Idle,
    /// First click of a Hold has been made; cursor preview shows the tail.
    HoldPending { anchor_t: f32, lane: u8 },
    /// First click of a Star (head) has been made; preview a dashed line.
    StarHead { head_t: f32, lane: u8 },
    /// Second click confirmed slide_start_delay; preview the slide bar.
    StarDelay { head_t: f32, lane: u8, delay_end_t: f32 },
}

/// Width of the tool sidebar inside the timeline panel (in screen pixels,
/// before applying ui_scale). Kept as a single source of truth so the
/// renderer (`ui.rs`) and input handler (`input.rs`) stay aligned.
pub(crate) const TIMELINE_SIDEBAR_W: f32 = 56.0;

/// Compute the screen rects for the three sidebar tool buttons given the
/// timeline panel rect. Returned in the order [Tap, Hold, Star].
pub(crate) fn timeline_sidebar_buttons(tl: &RectF) -> [(RectF, PlaceTool, &'static str); 3] {
    let pad = 6.0_f32;
    let btn_h = 50.0_f32;
    let x = tl.x + pad;
    let w = TIMELINE_SIDEBAR_W - pad * 2.0;
    let y0 = tl.y + 66.0;
    [
        (RectF { x, y: y0,                       w, h: btn_h }, PlaceTool::Tap,  "Tap"),
        (RectF { x, y: y0 + (btn_h + 6.0),       w, h: btn_h }, PlaceTool::Hold, "Hold"),
        (RectF { x, y: y0 + (btn_h + 6.0) * 2.0, w, h: btn_h }, PlaceTool::Star, "Star"),
    ]
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PadFeedback {
    pub(crate) zone: u8,
    pub(crate) until: f64,
}

/// Hold tail time in seconds (note fields are in measures).
pub(crate) fn hold_tail_time(note: &Note, bpms: &[BpmChange]) -> f32 {
    let dur_s = mdur_to_secs(note.hold_duration, note.time, bpms).max(0.15);
    note_secs(note, bpms) + dur_s
}

pub fn sanitize_note_zone(_note_type: NoteType, lane: u8) -> u8 {
    lane.clamp(1, PAD_ZONE_MAX)
}

pub fn is_touch_zone(zone: u8) -> bool {
    zone >= PAD_B_START
}

/// Slide end time in seconds — takes the longest slide's duration.
pub fn slide_end_time(note: &Note, bpms: &[BpmChange]) -> f32 {
    let max_dur = note.slide.iter()
        .map(|s| s.slide_duration)
        .fold(0.0_f32, f32::max);
    let dur_s = mdur_to_secs(max_dur, note.time, bpms).max(0.3);
    note_secs(note, bpms) + dur_s
}
