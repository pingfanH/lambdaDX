use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type TimePoint = i64;
pub type Duration = i64;

fn default_one_u64() -> u64 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FfiErrorInfo {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct FfiEnvelope<T = serde_json::Value> {
    pub ok: bool,
    #[serde(default)]
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<FfiErrorInfo>,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

pub type FfiResult = FfiEnvelope<serde_json::Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfiVersion {
    pub abi_version: u64,
    pub schema: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JudgeGrade {
    Miss,
    LateGood,
    LateGreat3rd,
    LateGreat2nd,
    LateGreat,
    LatePerfect3rd,
    LatePerfect2nd,
    Perfect,
    FastPerfect2nd,
    FastPerfect3rd,
    FastGreat,
    FastGreat2nd,
    FastGreat3rd,
    FastGood,
    TooFast,
}

impl JudgeGrade {
    pub fn is_miss_or_too_fast(self) -> bool {
        matches!(self, JudgeGrade::Miss | JudgeGrade::TooFast)
    }

    pub fn is_great_grade(self) -> bool {
        matches!(
            self,
            JudgeGrade::LateGreat
                | JudgeGrade::LateGreat2nd
                | JudgeGrade::LateGreat3rd
                | JudgeGrade::FastGreat
                | JudgeGrade::FastGreat2nd
                | JudgeGrade::FastGreat3rd
        )
    }

    pub fn is_good_grade(self) -> bool {
        matches!(self, JudgeGrade::LateGood | JudgeGrade::FastGood)
    }
}

pub type JudgeCounts = BTreeMap<JudgeGrade, u64>;

/// Result-combo display category derived from accumulated judge counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ComboState {
    None,
    FC,
    FCPlus,
    AP,
    APPlus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JudgeEventKind {
    Tap,
    Hold,
    Slide,
    Touch,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum RuntimeSlideKind {
    Single,
    Wifi,
    ConnPart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AreaPolicy {
    Or,
    And,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JudgeStyle {
    Default,
    Maji,
    Gachi,
    Gori,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JudgeDisplayOption {
    All,
    BelowCP,
    BelowP,
    BelowGR,
    MissOnly,
    Disable,
}

impl Default for JudgeDisplayOption {
    fn default() -> Self {
        JudgeDisplayOption::All
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SensorArea {
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    A8,
    B1,
    B2,
    B3,
    B4,
    B5,
    B6,
    B7,
    B8,
    C,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ButtonZone {
    K1,
    K2,
    K3,
    K4,
    K5,
    K6,
    K7,
    K8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OuterSlot {
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rational {
    pub num: i64,
    pub den: u64,
    pub decimal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePos {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<ButtonZone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor: Option<SensorArea>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePos {
    pub line: u64,
    pub column: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    pub start: SourcePos,
    pub stop: SourcePos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParseErrorKind {
    InvalidSyntax,
    InvalidShape,
    InvalidEndPosition,
    InvalidTurnPosition,
    InvalidChainTiming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub raw_text: String,
    pub message: String,
    #[serde(default)]
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyntaxSlideKind {
    Line,
    Circle,
    V,
    Turn,
    Pq,
    Ppqq,
    S,
    Wifi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanonicalSlideShape {
    #[serde(rename = "line")]
    Line {
        #[serde(rename = "relEnd")]
        rel_end: u64,
    },
    #[serde(rename = "circle")]
    Circle {
        #[serde(rename = "relEnd")]
        rel_end: u64,
    },
    #[serde(rename = "v")]
    V {
        #[serde(rename = "relEnd")]
        rel_end: u64,
    },
    #[serde(rename = "turn")]
    Turn {
        #[serde(rename = "relEnd")]
        rel_end: u64,
    },
    #[serde(rename = "pq")]
    Pq {
        #[serde(rename = "relEnd")]
        rel_end: u64,
    },
    #[serde(rename = "ppqq")]
    Ppqq {
        #[serde(rename = "relEnd")]
        rel_end: u64,
    },
    #[serde(rename = "s")]
    S,
    #[serde(rename = "wifi")]
    Wifi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSymmetry {
    pub rotation_steps: u64,
    pub mirrored: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideShape {
    pub canonical: CanonicalSlideShape,
    pub symmetry: SlideSymmetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlideBodyKind {
    Line,
    CircleRight,
    CircleLeft,
    CircleUp,
    V,
    Pp,
    Qq,
    P,
    Q,
    S,
    Z,
    Turn,
    Wifi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSlideBody {
    pub raw_text: String,
    pub start_lane: OuterSlot,
    pub kind: SlideBodyKind,
    #[serde(default)]
    pub end_area: Option<SensorArea>,
    #[serde(default)]
    pub turn_area: Option<SensorArea>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideNoteSemantics {
    pub raw_text: String,
    pub start_slot: OuterSlot,
    pub end_area: SensorArea,
    pub shape: SlideShape,
    pub is_just_right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RawNoteKind {
    Tap,
    Hold,
    Slide,
    Touch,
    TouchHold,
    Rest,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaidataMetadata {
    pub fields: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaidataChartBlock {
    pub level_index: u64,
    pub raw_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawNoteToken {
    pub raw_text: String,
    pub kind: RawNoteKind,
    pub timing: TimePoint,
    pub bpm: Rational,
    pub h_speed: Rational,
    pub divisor: u64,
    #[serde(default)]
    pub slot: Option<OuterSlot>,
    #[serde(default)]
    pub sensor_pos: Option<SensorArea>,
    #[serde(default)]
    pub slide_body: Option<ParsedSlideBody>,
    #[serde(default)]
    pub length: Option<Duration>,
    #[serde(default)]
    pub star_wait: Option<Duration>,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub is_hanabi: bool,
    #[serde(default)]
    pub is_slide_no_head: bool,
    #[serde(default)]
    pub is_force_star: bool,
    #[serde(default)]
    pub is_fake_rotate: bool,
    #[serde(default)]
    pub is_slide_break: bool,
    #[serde(default)]
    pub source_group_id: Option<u64>,
    #[serde(default)]
    pub source_group_index: Option<u64>,
    #[serde(default)]
    pub source_group_size: Option<u64>,
    #[serde(default)]
    pub source_pos: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceNote {
    pub token: RawNoteToken,
    #[serde(default)]
    pub source_pos: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEvent {
    pub timing: TimePoint,
    pub bpm: Rational,
    pub h_speed: Rational,
    pub divisor: u64,
    pub notes: Vec<SourceNote>,
    #[serde(default)]
    pub source_pos: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceChart {
    pub events: Vec<SourceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideAreaSpec {
    pub target_areas: Vec<SensorArea>,
    pub policy: AreaPolicy,
    #[serde(default)]
    pub is_last: bool,
    #[serde(default)]
    pub is_skippable: bool,
    pub arrow_progress_when_on: u64,
    pub arrow_progress_when_finished: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSlideDebug {
    pub note_index: u64,
    pub raw_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedTap {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub is_hanabi: bool,
    #[serde(default)]
    pub is_force_star: bool,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedHold {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    pub length: Duration,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub is_hanabi: bool,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedTouch {
    pub timing: TimePoint,
    pub sensor_pos: SensorArea,
    #[serde(default)]
    pub is_break: bool,
    #[serde(default)]
    pub is_hanabi: bool,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedTouchHold {
    pub timing: TimePoint,
    pub sensor_pos: SensorArea,
    pub length: Duration,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub is_hanabi: bool,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSlide {
    /// Body-side preserved slide-head timing anchor.
    ///
    /// The current schema exports `headTiming`.
    #[serde(rename = "headTiming")]
    pub head_timing: TimePoint,
    pub slot: OuterSlot,
    pub length: Duration,
    pub start_timing: TimePoint,
    pub h_speed: Rational,
    pub slide_kind: RuntimeSlideKind,
    #[serde(default)]
    pub is_classic: bool,
    pub track_count: u64,
    #[serde(default)]
    pub judge_at: Option<TimePoint>,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub is_hanabi: bool,
    /// Preferred normalized semantic flag for whether a judged head exists.
    pub has_head_note: bool,
    /// Preferred normalized semantic flag for whether a slide body exists.
    pub has_body: bool,
    /// Compatibility metadata from parser-originated chart syntax.
    ///
    /// Do not treat this as the long-term semantic authority for head/body
    /// existence when `has_head_note` / `has_body` are available.
    #[serde(default)]
    pub is_slide_no_head: bool,
    #[serde(default)]
    pub is_force_star: bool,
    #[serde(default)]
    pub is_fake_rotate: bool,
    #[serde(default)]
    pub is_slide_break: bool,
    #[serde(default)]
    pub is_conn_slide: bool,
    #[serde(default)]
    pub parent_note_index: Option<u64>,
    #[serde(default)]
    pub is_group_head: bool,
    #[serde(default)]
    pub is_group_end: bool,
    pub total_judge_queue_len: u64,
    pub judge_queues: Vec<Vec<SlideAreaSpec>>,
    #[serde(default)]
    pub source_group_id: Option<u64>,
    #[serde(default)]
    pub source_group_index: Option<u64>,
    #[serde(default)]
    pub source_group_size: Option<u64>,
    #[serde(default = "default_one_u64")]
    pub multiple: u64,
    pub note_index: u64,
    pub simai_shape: SlideShape,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedChart {
    pub taps: Vec<NormalizedTap>,
    pub holds: Vec<NormalizedHold>,
    pub touches: Vec<NormalizedTouch>,
    pub touch_holds: Vec<NormalizedTouchHold>,
    pub slides: Vec<NormalizedSlide>,
    pub slide_debug: Vec<NormalizedSlideDebug>,
    pub slide_skipping: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendChartInspection {
    pub metadata: MaidataMetadata,
    pub chart: MaidataChartBlock,
    pub source: SourceChart,
    pub tokens: Vec<RawNoteToken>,
    pub slide_notes: Vec<SlideNoteSemantics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSemanticChart {
    pub normalized: NormalizedChart,
    pub lowered: ChartSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendChartResult {
    pub semantic: FrontendSemanticChart,
    pub inspection: FrontendChartInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapChartNote {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    pub button_queue_index: u64,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldChartNote {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    pub length: Duration,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub is_touch: bool,
    #[serde(default)]
    pub is_classic: Option<bool>,
    pub button_queue_index: u64,
    #[serde(default)]
    pub touch_hold_group_id: Option<u64>,
    #[serde(default)]
    pub touch_hold_group_size: Option<u64>,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchChartNote {
    pub timing: TimePoint,
    pub sensor_pos: SensorArea,
    #[serde(default)]
    pub is_break: bool,
    #[serde(default)]
    pub source_group_id: Option<u64>,
    #[serde(default)]
    pub source_group_index: Option<u64>,
    #[serde(default)]
    pub source_group_size: Option<u64>,
    pub touch_queue_index: u64,
    #[serde(default)]
    pub touch_group_id: Option<u64>,
    #[serde(default)]
    pub touch_group_size: Option<u64>,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchHoldChartNote {
    pub timing: TimePoint,
    pub sensor_pos: SensorArea,
    pub length: Duration,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default)]
    pub source_group_id: Option<u64>,
    #[serde(default)]
    pub source_group_index: Option<u64>,
    #[serde(default)]
    pub source_group_size: Option<u64>,
    pub touch_queue_index: u64,
    #[serde(default)]
    pub touch_group_id: Option<u64>,
    #[serde(default)]
    pub touch_group_size: Option<u64>,
    #[serde(default)]
    pub touch_hold_group_id: Option<u64>,
    #[serde(default)]
    pub touch_hold_group_size: Option<u64>,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideHeadChartNote {
    pub timing: TimePoint,
    pub slot: OuterSlot,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    /// Shared logical slide identity linking lowered head/body objects.
    #[serde(default)]
    pub logical_slide_id: u64,
    /// Runtime object note id.
    ///
    /// Lowered slide heads and slide bodies use distinct `noteIndex` values
    /// while sharing `logicalSlideId`.
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideChartNote {
    /// Body-side preserved slide-head timing anchor.
    ///
    /// The current schema exports `headTiming`.
    #[serde(rename = "headTiming")]
    pub head_timing: TimePoint,
    pub slot: OuterSlot,
    pub length: Duration,
    pub start_timing: TimePoint,
    pub slide_kind: RuntimeSlideKind,
    #[serde(default)]
    pub is_classic: bool,
    /// Compatibility metadata retained on the lowered body object.
    #[serde(default)]
    pub is_slide_no_head: bool,
    #[serde(default)]
    pub is_conn_slide: bool,
    #[serde(default)]
    pub parent_note_index: Option<u64>,
    #[serde(default)]
    pub is_group_head: bool,
    #[serde(default)]
    pub is_group_end: bool,
    #[serde(default)]
    pub parent_finished: bool,
    #[serde(default)]
    pub parent_pending_finish: bool,
    pub total_judge_queue_len: u64,
    pub track_count: u64,
    #[serde(default)]
    pub judge_at: Option<TimePoint>,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    #[serde(default = "default_one_u64")]
    pub multiple: u64,
    /// Shared logical slide identity linking lowered head/body objects.
    #[serde(default)]
    pub logical_slide_id: u64,
    /// Runtime object note id.
    ///
    /// Lowered slide heads and slide bodies use distinct `noteIndex` values
    /// while sharing `logicalSlideId`.
    pub note_index: u64,
    pub judge_queues: Vec<Vec<SlideAreaSpec>>,
    #[serde(default)]
    pub debug_simai: Option<(String, (String, bool))>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpec {
    pub taps: Vec<TapChartNote>,
    pub holds: Vec<HoldChartNote>,
    pub touches: Vec<TouchChartNote>,
    pub touch_holds: Vec<TouchHoldChartNote>,
    #[serde(default)]
    pub slide_heads: Vec<SlideHeadChartNote>,
    pub slides: Vec<SlideChartNote>,
    #[serde(default)]
    pub slide_skipping: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimedInputEvent {
    #[serde(rename = "buttonClick")]
    ButtonClick { tp: TimePoint, zone: ButtonZone },
    #[serde(rename = "buttonHold")]
    ButtonHold {
        tp: TimePoint,
        zone: ButtonZone,
        #[serde(rename = "isDown")]
        is_down: bool,
    },
    #[serde(rename = "sensorClick")]
    SensorClick { tp: TimePoint, area: SensorArea },
    #[serde(rename = "sensorHold")]
    SensorHold {
        tp: TimePoint,
        area: SensorArea,
        #[serde(rename = "isDown")]
        is_down: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedInputBatch {
    pub current_time: TimePoint,
    pub events: Vec<TimedInputEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualTacticSequence {
    pub events: Vec<TimedInputEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInput {
    pub button_clicked: Vec<bool>,
    pub button_held: Vec<bool>,
    pub sensor_clicked: Vec<bool>,
    pub sensor_held: Vec<bool>,
    pub button_click_count: Vec<u64>,
    pub sensor_click_count: Vec<u64>,
    pub delta: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneQueue<T> {
    pub notes: Vec<T>,
    pub current_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonNoteParams {
    pub judge_timing: TimePoint,
    pub judge_offset: Duration,
    #[serde(default)]
    pub is_break: bool,
    #[serde(rename = "isEX", default)]
    pub is_ex: bool,
    pub note_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoldStart {
    #[serde(rename = "button")]
    Button { zone: ButtonZone },
    #[serde(rename = "sensor")]
    Sensor { area: SensorArea },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapState {
    #[serde(rename = "Waiting")]
    Waiting,
    #[serde(rename = "Judgeable")]
    Judgeable,
    #[serde(rename = "Judged")]
    Judged { grade: JudgeGrade },
    #[serde(rename = "Ended")]
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapNote {
    pub params: CommonNoteParams,
    pub lane: OuterSlot,
    pub state: TapState,
    pub button_queue_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideHeadNote {
    pub params: CommonNoteParams,
    pub lane: OuterSlot,
    pub state: TapState,
    pub logical_slide_id: u64,
    pub button_queue_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TapFamilyNote {
    #[serde(rename = "tap")]
    Tap {
        params: CommonNoteParams,
        lane: OuterSlot,
        state: TapState,
        #[serde(rename = "buttonQueueIndex")]
        button_queue_index: u64,
    },
    #[serde(rename = "slideHead")]
    SlideHead {
        params: CommonNoteParams,
        lane: OuterSlot,
        state: TapState,
        #[serde(rename = "logicalSlideId")]
        logical_slide_id: u64,
        #[serde(rename = "buttonQueueIndex")]
        button_queue_index: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoldSubState {
    #[serde(rename = "HeadWaiting")]
    HeadWaiting,
    #[serde(rename = "HeadJudgeable")]
    HeadJudgeable,
    #[serde(rename = "HeadJudged")]
    HeadJudged { grade: JudgeGrade },
    #[serde(rename = "BodyHeld")]
    BodyHeld,
    #[serde(rename = "BodyReleased")]
    BodyReleased,
    #[serde(rename = "Ended")]
    Ended { grade: JudgeGrade },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldNote {
    pub params: CommonNoteParams,
    pub start: HoldStart,
    pub state: HoldSubState,
    pub length: Duration,
    pub button_queue_index: u64,
    pub head_diff: Duration,
    pub head_grade: JudgeGrade,
    pub player_release_time: Duration,
    #[serde(default)]
    pub release_ignore_time: Duration,
    #[serde(default)]
    pub is_classic: bool,
    #[serde(default)]
    pub is_touch_hold: bool,
    pub touch_queue_index: u64,
    #[serde(default)]
    pub touch_group_id: Option<u64>,
    pub touch_group_size: u64,
    #[serde(default)]
    pub touch_hold_group_id: Option<u64>,
    pub touch_hold_group_size: u64,
    #[serde(default)]
    pub touch_hold_group_triggered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchState {
    #[serde(rename = "Waiting")]
    Waiting,
    #[serde(rename = "Judgeable")]
    Judgeable,
    #[serde(rename = "Judged")]
    Judged { grade: JudgeGrade },
    #[serde(rename = "Ended")]
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchNote {
    pub params: CommonNoteParams,
    pub state: TouchState,
    pub sensor_pos: SensorArea,
    pub touch_queue_index: u64,
    #[serde(default)]
    pub touch_group_id: Option<u64>,
    pub touch_group_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlideState {
    #[serde(rename = "Waiting")]
    Waiting,
    #[serde(rename = "Active")]
    Active {
        #[serde(rename = "waitTime")]
        wait_time: Duration,
    },
    #[serde(rename = "Judged")]
    Judged {
        grade: JudgeGrade,
        #[serde(rename = "waitTime")]
        wait_time: Duration,
        #[serde(rename = "judgeDiff")]
        judge_diff: Duration,
    },
    #[serde(rename = "Ended")]
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideArea {
    pub target_areas: Vec<SensorArea>,
    pub policy: AreaPolicy,
    #[serde(default)]
    pub is_last: bool,
    #[serde(default)]
    pub is_skippable: bool,
    pub arrow_progress_when_on: u64,
    pub arrow_progress_when_finished: u64,
    #[serde(default)]
    pub was_on: bool,
    #[serde(default)]
    pub was_off: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideNote {
    pub params: CommonNoteParams,
    pub lane: OuterSlot,
    pub state: SlideState,
    pub length: Duration,
    /// Runtime body-side preserved slide-head timing anchor.
    ///
    /// Runtime state serializes `headTiming`.
    #[serde(rename = "headTiming")]
    pub head_timing: TimePoint,
    pub start_timing: TimePoint,
    pub slide_kind: RuntimeSlideKind,
    #[serde(default)]
    pub is_classic: bool,
    #[serde(default)]
    pub is_conn_slide: bool,
    #[serde(default)]
    pub parent_note_index: Option<u64>,
    #[serde(default)]
    pub is_group_part_head: bool,
    #[serde(default)]
    pub is_group_part_end: bool,
    #[serde(default)]
    pub parent_finished: bool,
    #[serde(default)]
    pub parent_pending_finish: bool,
    pub initial_queue_remaining: u64,
    pub total_judge_queue_len: u64,
    pub track_count: u64,
    #[serde(default)]
    pub is_checkable: bool,
    #[serde(default = "default_one_u64")]
    pub multiple: u64,
    pub judge_queues: Vec<Vec<SlideArea>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupState {
    pub group_id: u64,
    pub count: u64,
    pub size: u64,
    pub grade: JudgeGrade,
    pub diff: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchHoldBodyGroupState {
    pub group_id: u64,
    pub member_note_indices: Vec<u64>,
    pub triggered_note_indices: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteTypeJudgeCounts {
    pub tap_count: JudgeCounts,
    pub hold_count: JudgeCounts,
    pub slide_count: JudgeCounts,
    pub touch_count: JudgeCounts,
    pub break_count: JudgeCounts,
}

impl NoteTypeJudgeCounts {
    /// Counts one judge grade across all note families.
    pub fn grade_count(&self, grade: JudgeGrade) -> u64 {
        self.tap_count.get(&grade).copied().unwrap_or(0)
            + self.hold_count.get(&grade).copied().unwrap_or(0)
            + self.slide_count.get(&grade).copied().unwrap_or(0)
            + self.touch_count.get(&grade).copied().unwrap_or(0)
            + self.break_count.get(&grade).copied().unwrap_or(0)
    }

    /// Counts all judge grades matching a predicate across all note families.
    pub fn grade_count_where(&self, mut pred: impl FnMut(JudgeGrade) -> bool) -> u64 {
        ALL_JUDGE_GRADES
            .iter()
            .copied()
            .filter(|grade| pred(*grade))
            .map(|grade| self.grade_count(grade))
            .sum()
    }
}

/// Judge grades in the same order used by the Lean JSON count serializer.
pub const ALL_JUDGE_GRADES: [JudgeGrade; 15] = [
    JudgeGrade::Miss,
    JudgeGrade::LateGood,
    JudgeGrade::LateGreat3rd,
    JudgeGrade::LateGreat2nd,
    JudgeGrade::LateGreat,
    JudgeGrade::LatePerfect3rd,
    JudgeGrade::LatePerfect2nd,
    JudgeGrade::Perfect,
    JudgeGrade::FastPerfect2nd,
    JudgeGrade::FastPerfect3rd,
    JudgeGrade::FastGreat,
    JudgeGrade::FastGreat2nd,
    JudgeGrade::FastGreat3rd,
    JudgeGrade::FastGood,
    JudgeGrade::TooFast,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreState {
    pub combo: u64,
    pub p_combo: u64,
    pub c_p_combo: u64,
    pub total_base: u64,
    pub total_extra: u64,
    pub earned_base: u64,
    pub earned_extra: u64,
    pub lost_base: u64,
    pub lost_extra: u64,
    /// DX-score loss delta: `0` means no loss, negative values represent lost
    /// DX score. Use [`ScoreState::dx_score_remaining`] for the current
    /// achieved DX score.
    pub dx_score: i64,
    /// Total available DX score for the loaded chart.
    pub max_dx_score: u64,
    pub fast_count: u64,
    pub late_count: u64,
    pub counts: NoteTypeJudgeCounts,
}

impl ScoreState {
    /// Achieved DX score, computed from the core loss-delta field.
    pub fn dx_score_remaining(&self) -> i64 {
        self.max_dx_score as i64 + self.dx_score
    }

    /// Display combo category matching `LnmaiCore.comboState`.
    pub fn combo_state(&self) -> ComboState {
        let critical = self.counts.grade_count(JudgeGrade::Perfect);
        let perfect = self.counts.grade_count_where(|grade| {
            matches!(
                grade,
                JudgeGrade::LatePerfect3rd
                    | JudgeGrade::LatePerfect2nd
                    | JudgeGrade::FastPerfect2nd
                    | JudgeGrade::FastPerfect3rd
            )
        });
        let great = self
            .counts
            .grade_count_where(|grade| grade.is_great_grade());
        let good = self.counts.grade_count_where(|grade| grade.is_good_grade());
        let miss = self
            .counts
            .grade_count_where(|grade| grade.is_miss_or_too_fast());
        let all_non_miss = critical + perfect + great + good;

        if all_non_miss == 0 || miss != 0 {
            ComboState::None
        } else if perfect == 0 && great == 0 && good == 0 {
            ComboState::APPlus
        } else if great == 0 && good == 0 {
            ComboState::AP
        } else if good == 0 {
            ComboState::FCPlus
        } else {
            ComboState::FC
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub current_time: TimePoint,
    pub prev_button: Vec<bool>,
    pub prev_sensor: Vec<bool>,
    pub button_queue_frontiers: Vec<u64>,
    pub touch_queue_frontiers: Vec<u64>,
    pub tap_queues: Vec<ZoneQueue<TapFamilyNote>>,
    pub hold_queues: Vec<ZoneQueue<HoldNote>>,
    pub touch_hold_queues: Vec<ZoneQueue<HoldNote>>,
    pub touch_queues: Vec<ZoneQueue<TouchNote>>,
    pub slides: Vec<SlideNote>,
    pub active_holds: Vec<(ButtonZone, HoldNote)>,
    pub active_touch_holds: Vec<(SensorArea, HoldNote)>,
    pub touch_group_states: Vec<GroupState>,
    pub touch_hold_group_states: Vec<TouchHoldBodyGroupState>,
    pub current_batch: TimedInputBatch,
    pub score: ScoreState,
    pub judge_style: JudgeStyle,
    pub touch_panel_offset: Duration,
    pub subdivide_slide_judge_grade: bool,
    #[serde(default)]
    pub note_fast_late_display: JudgeDisplayOption,
    #[serde(default = "default_break_fast_late_display")]
    pub break_fast_late_display: JudgeDisplayOption,
}

fn default_break_fast_late_display() -> JudgeDisplayOption {
    JudgeDisplayOption::Disable
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeEvent {
    pub kind: JudgeEventKind,
    pub grade: JudgeGrade,
    pub diff: Duration,
    pub position: RuntimePos,
    pub note_index: u64,
    #[serde(default)]
    pub is_break: bool,
    #[serde(default = "default_one_u64")]
    pub multiple: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCommand {
    #[serde(rename = "PlayJudgeSfx")]
    PlayJudgeSfx {
        kind: JudgeEventKind,
        grade: JudgeGrade,
        #[serde(rename = "isBreak", default)]
        is_break: bool,
        #[serde(rename = "atTime")]
        at_time: TimePoint,
        #[serde(rename = "noteIndex")]
        note_index: u64,
    },
    #[serde(rename = "PlaySlideCue")]
    PlaySlideCue {
        #[serde(rename = "noteIndex")]
        note_index: u64,
        #[serde(rename = "trackIndex")]
        track_index: u64,
        #[serde(rename = "isBreak", default)]
        is_break: bool,
        #[serde(rename = "atTime")]
        at_time: TimePoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderCommand {
    #[serde(rename = "ShowJudgeResult")]
    ShowJudgeResult {
        kind: JudgeEventKind,
        grade: JudgeGrade,
        #[serde(rename = "isBreak", default)]
        is_break: bool,
        diff: Duration,
        #[serde(rename = "noteIndex")]
        note_index: u64,
    },
    #[serde(rename = "UpdateSlideProgress")]
    UpdateSlideProgress {
        #[serde(rename = "noteIndex")]
        note_index: u64,
        remaining: u64,
    },
    #[serde(rename = "UpdateSlideTrackProgress")]
    UpdateSlideTrackProgress {
        #[serde(rename = "noteIndex")]
        note_index: u64,
        #[serde(rename = "trackIndex")]
        track_index: u64,
        remaining: u64,
    },
    #[serde(rename = "HideAllSlideBars")]
    HideAllSlideBars {
        #[serde(rename = "noteIndex")]
        note_index: u64,
    },
    #[serde(rename = "HideSlideBars")]
    HideSlideBars {
        #[serde(rename = "noteIndex")]
        note_index: u64,
        #[serde(rename = "endIndex")]
        end_index: u64,
    },
    #[serde(rename = "HideSlideTrackBars")]
    HideSlideTrackBars {
        #[serde(rename = "noteIndex")]
        note_index: u64,
        #[serde(rename = "trackIndex")]
        track_index: u64,
        #[serde(rename = "endIndex")]
        end_index: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStepResult {
    pub state: GameState,
    pub events: Vec<JudgeEvent>,
    pub audio_commands: Vec<AudioCommand>,
    pub render_commands: Vec<RenderCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStepLightResult {
    pub events: Vec<JudgeEvent>,
    pub audio_commands: Vec<AudioCommand>,
    pub render_commands: Vec<RenderCommand>,
    pub score: ScoreState,
    pub current_time: TimePoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedChartSummary {
    pub tap_count: u64,
    pub hold_count: u64,
    pub touch_count: u64,
    pub touch_hold_count: u64,
    pub slide_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub handle: u64,
    pub state: String,
    #[serde(default)]
    pub summary: Option<LoadedChartSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandleOnly {
    pub handle: u64,
}
