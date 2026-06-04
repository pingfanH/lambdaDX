pub mod zone;

use macroquad::prelude::{TouchPhase, Vec2};
use serde::{Deserialize, Serialize};
use crate::app::types::zone::PadZone;

pub const LANE_COUNT: usize = 9;
pub const LANE_LABELS: [&str; LANE_COUNT] = ["1", "2", "3", "4", "5", "6", "7", "8", "T"];
pub const SCROLL_SPEED: f32 = 480.0;
pub const PREVIEW_LEAD_TIME: f32 = 1.6;
pub const HIT_WINDOW: f32 = 0.00;
pub const TAP_TRAVEL_TIME: f32 = 0.55;
pub const TOUCH_TRAVEL_TIME: f32 = 0.5;
pub const HOLD_TRAVEL_TIME: f32 = 0.55;
pub const TAP_GROW_FRAC: f32 = 0.35;
pub const TAP_SPAWN_FRAC: f32 = 0.3;
pub const TAP_DISAPPEAR_FRAC: f32 = 0.0;
pub const HOLD_DISAPPEAR_FRAC: f32 = 0.1;
pub const HOLD_FLY_TIME: f32 = 0.6;
pub const HOLD_TAIL_FLY_TIME: f32 = 0.40;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub const HOLD_LENGTH_FRAC: f32 = 0.4;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub const HOLD_LENGTH_FRAC: f32 = 0.6;

pub const HOLD_SPAWN_FRAC: f32 = 0.5;
pub const HOLD_TARGET_OFFSET: f32 = 40.0;
pub const TAP_TARGET_OFFSET: f32 = 15.;
// touch: base values (multiplied by TOUCH_SCALE in code)
pub const TOUCH_CROSS_SIZE: f32 = 50.0;
pub const TOUCH_START_DIST: f32 = 30.0;
pub const TOUCH_END_DIST: f32 = 10.0;
// touchhold: base values (multiplied by TOUCHHOLD_SCALE in code)
pub const TOUCHHOLD_CROSS_BASE: f32 = 86.0;
pub const TOUCHHOLD_BORDER_BASE: f32 = 170.0;
pub const TOUCHHOLD_START_DIST: f32 = 30.0;
pub const TOUCHHOLD_END_DIST: f32 = 19.0;
pub const TOUCHHOLD_ROT_OFFSET: f32 = 0.0;
pub const EACH_WINDOW: f32 = 0.02;
pub const TOUCH_GROW_FRAC: f32 = 0.25;
pub const TOUCH_DISAPPEAR_TIME: f32 = -0.1;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub const TAP_SIZE: f32 = 40.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub const HOLD_WIDTH: f32 = 40.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub const TOUCH_SIZE: f32 = 18.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub const TOUCH_SCALE: f32 = 1.0;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub const TOUCHHOLD_SCALE: f32 = 0.6;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub const TAP_SIZE: f32 = 80.0;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub const HOLD_WIDTH: f32 = 80.0;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub const TOUCH_SIZE: f32 = 70.0;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub const TOUCH_SCALE: f32 = 1.5;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub const TOUCHHOLD_SCALE: f32 = 1.0;

pub const PAD_ROTATION_RAD: f32 = std::f32::consts::FRAC_PI_8;
pub const TAP_RING_OFFSET: f32 = 14.;
pub const GRID_DIVISION: u32 = 64;
pub const SCROLL_SPEED_FACTOR: f32 = 0.01;
pub const SCROLL_INVERT: bool = true;

pub const SLIDE_TILE_SPACING: f32 = 20.0;
pub const SLIDE_TILE_SIZE: f32 = 40.0;
pub const SLIDE_TILE_SCALE: f32 = 0.4;
pub const SLIDE_MIN_POINTS: usize = 2;
pub const STAR_SIZE: f32 = 45.0;
pub const SLIDE_TRAVEL_TIME: f32 = 0.55;
pub const SLIDE_STAR_FADE_IN: f32 = 0.12;
pub const SPEED_MIN: f32 = 0.1;
pub const SPEED_MAX: f32 = 3.0;
pub const SPEED_STEP: f32 = 0.1;
pub const HOLD_RECORD_MIN_DURATION: f32 = 0.2;
pub const TOUCH_SPEED_MIN: f32 = 0.5;
pub const TOUCH_SPEED_MAX: f32 = 3.0;
pub const TOUCH_SPEED_STEP: f32 = 0.1;
pub const MOUSE_POINTER_ID: u64 = u64::MAX;
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

/// Validate a slide shape with start and end lanes.
/// Returns Ok(()) if valid, Err(message) if invalid.
pub fn validate_slide_shape(shape: SlideShape, start_lane: u8, end_lane: u8) -> Result<(), String> {
    if start_lane < 1 || start_lane > 8 {
        return Err(format!("起始位置 {} 无效，必须是 1-8", start_lane));
    }
    if end_lane < 1 || end_lane > 8 {
        return Err(format!("结束位置 {} 无效，必须是 1-8", end_lane));
    }
    
    // Calculate relative end position (1-indexed, wrapping around 8 positions)
    let rel_end = ((end_lane as i32 - start_lane as i32 + 8) % 8) + 1;
    
    match shape {
        SlideShape::S | SlideShape::Z => {
            // S and Z must end at position 5 (opposite side)
            if rel_end != 5 {
                return Err(format!("{:?} 形状必须结束在对面位置（相对位置5），当前相对位置是 {}", shape, rel_end));
            }
        }
        SlideShape::Wifi => {
            // Wifi must end at position 5 (opposite side)
            if rel_end != 5 {
                return Err(format!("Wifi 形状必须结束在对面位置（相对位置5），当前相对位置是 {}", rel_end));
            }
        }
        SlideShape::VShape => {
            // V shape cannot end at position 5
            if rel_end == 5 {
                return Err("V 形状不能结束在对面位置（相对位置5）".to_string());
            }
        }
        SlideShape::BigV => {
            // BigV (turn) has special rules
            // For simplicity, just check it's not the same position
            if start_lane == end_lane {
                return Err("V 形状不能在同一位置开始和结束".to_string());
            }
        }
        _ => {
            // Other shapes are generally valid
        }
    }
    
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize,Copy)]
pub struct SlidePoint {
    pub zone: PadZone,
    pub beat_offset: f32,
}
impl From<PadZone> for SlidePoint {
    fn from(value: PadZone) -> Self {
        SlidePoint { zone:value,beat_offset:0.0 }
    }
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
    /// 唯一 ID，插入删除后不变
    #[serde(default)]
    pub id: u64,
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
    pub slide: Vec<Slide>,
    /// If this note was expanded from a template instance, tracks its origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_source: Option<NoteTemplateSource>,
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
pub fn sdur_to_mdur(d: f32, start_secs: f32, bpms: &[BpmChange]) -> f32 {
    secs_to_measure(start_secs + d, bpms) - secs_to_measure(start_secs, bpms)
}

/// BPM in effect at the given measure position.
pub fn bpm_at(m: f32, bpms: &[BpmChange]) -> f32 {
    let mut bpm = bpms.first().map(|b| b.bpm).unwrap_or(120.0);
    for b in bpms {
        if b.measure > m + 0.0001 { break; }
        bpm = b.bpm;
    }
    bpm
}

/// Snap a measure value to the nearest 1/384 grid position.
pub fn snap_measure(m: f32) -> f32 {
    const GRID: f32 = 384.0;
    (m * GRID).round() / GRID
}

/// Note head time in seconds.
pub fn note_secs(note: &Note, bpms: &[BpmChange]) -> f32 {
    measure_to_secs(note.time, bpms)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDoc {
    pub version: String,
    pub title: String,
    pub bpm: f32,
    /// Sorted list of BPM changes. First entry starts at measure 1.0.
    #[serde(default)]
    pub bpms: Vec<BpmChange>,
    /// Seconds from audio start to the first beat (Simai `&first`).  When
    /// non-zero the audio playback position is shifted by this amount so that
    /// `song_time == 0` aligns with this point in the audio file.
    #[serde(default)]
    pub audio_offset: f32,
    pub notes: Vec<Note>,
    /// Template definitions (reusable chart fragments).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<TemplateDef>,
    /// Template instances placed in this chart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub template_instances: Vec<TemplateInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitEvent {
    pub time: f32,
    pub lane: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingDoc {
    pub created_at_epoch_ms: u128,
    pub source: String,
    pub chart: ChartDoc,
    pub hits: Vec<HitEvent>,
    pub record_speed: f32,
    pub play_speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Idle,
    Recording,
    Playing,
}

#[derive(Debug, Clone, Copy)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub header: RectF,
    pub timeline: Option<RectF>,
    pub pad: RectF,
}

#[derive(Debug, Clone, Copy)]
pub struct PadGeom {
    pub cx: f32,
    pub cy: f32,
    pub outer_r: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum UiAction {
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
pub struct UiButton {
    pub rect: RectF,
    pub label: &'static str,
    pub action: UiAction,
}

#[derive(Debug, Clone)]
pub struct WavPcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordInputId {
    Key(u8),
    Pointer(u64),
}

#[derive(Debug, Clone)]
pub struct ActiveRecordHold {
    pub lane: u8,
    pub start_time: f32,
    pub slide_zones: Vec<SlidePoint>,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerEvent {
    pub id: u64,
    pub phase: TouchPhase,
    pub position: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragPart { Head, Body, Tail, SlideDelayEnd }

/// Currently selected tool in the timeline-left sidebar (Blender-style N-panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceTool { Tap, Hold, Star }

/// Multi-step placement state machine driven by clicks on the timeline.
/// Tap is single-shot, so it has no in-progress state.
#[derive(Debug, Clone, Copy)]
pub enum PlacementState {
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
pub const TIMELINE_SIDEBAR_W: f32 = 56.0;

/// Compute the screen rects for the three sidebar tool buttons given the
/// timeline panel rect. Returned in the order [Tap, Hold, Star].
pub fn timeline_sidebar_buttons(tl: &RectF) -> [(RectF, PlaceTool, &'static str); 3] {
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
pub struct PadFeedback {
    pub zone: PadZone,
    pub until: f64,
}

/// Hold tail time in seconds (note fields are in measures).
pub fn hold_tail_time(note: &Note, bpms: &[BpmChange]) -> f32 {
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

// ─── Template system ──────────────────────────────────────────────

/// Identifies whether we're editing the main chart or a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SceneRef {
    Main,
    Template {
        template_id: String,
        /// If Some, we're editing via a specific instance at this anchor time.
        /// Notes are offset to appear at their timeline position.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance_anchor: Option<f32>,
    },
}

/// A reusable chart fragment (like an Adobe Animate symbol).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDef {
    pub id: String,
    pub name: String,
    pub version: u32,
    /// Template's internal notes (relative time, 1.0 = start of template).
    pub notes: Vec<Note>,
    /// Total time span in measures.
    pub duration: f32,
}

/// An instance of a template placed in the chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInstance {
    pub instance_id: String,
    pub template_id: String,
    pub template_version: u32,
    /// Measure position in parent scene where this instance is anchored.
    pub anchor_time: f32,
}

/// Metadata attached to expanded notes linking them back to their source instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoteTemplateSource {
    pub instance_id: String,
    pub template_id: String,
    pub template_version: u32,
    pub source_note_id: u64,
}
