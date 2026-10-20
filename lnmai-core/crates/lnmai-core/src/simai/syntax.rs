//! Source/IR data types for the Simai pipeline.
//!
//! Mirrors `LnmaiCore/Simai/Syntax.lean`. The corresponding JSON instances
//! are implemented in `json_encode` and `json_decode`.

use crate::areas::{OuterSlot, SensorArea};
use crate::natural::Natural;
use crate::rat::Rat;
use crate::time::{Duration, TimePoint};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePos {
    pub line: Natural,
    pub column: Natural,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub start: SourcePos,
    pub stop: SourcePos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParseErrorKind {
    #[default]
    InvalidSyntax,
    InvalidShape,
    InvalidEndPosition,
    InvalidTurnPosition,
    InvalidChainTiming,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub raw_text: String,
    pub message: String,
    pub span: Option<SourceSpan>,
}

pub use lnmai_core_verify::shape::{SlideBodyKind, SlideKind};

/// Lean allows arbitrary natural endpoints even in unchecked shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalSlideShape {
    Line(Natural),
    Circle(Natural),
    V(Natural),
    Turn(Natural),
    Pq(Natural),
    Ppqq(Natural),
    S,
    Wifi,
}

impl Default for CanonicalSlideShape {
    fn default() -> Self {
        Self::Line(0.into())
    }
}

impl From<lnmai_core_verify::shape::CanonicalSlideShape> for CanonicalSlideShape {
    fn from(shape: lnmai_core_verify::shape::CanonicalSlideShape) -> Self {
        use lnmai_core_verify::shape::CanonicalSlideShape as Kernel;
        match shape {
            Kernel::Line(n) => Self::Line(n.into()),
            Kernel::Circle(n) => Self::Circle(n.into()),
            Kernel::V(n) => Self::V(n.into()),
            Kernel::Turn(n) => Self::Turn(n.into()),
            Kernel::Pq(n) => Self::Pq(n.into()),
            Kernel::Ppqq(n) => Self::Ppqq(n.into()),
            Kernel::S => Self::S,
            Kernel::Wifi => Self::Wifi,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideShape {
    pub canonical: CanonicalSlideShape,
    pub symmetry: crate::symmetry::SlideSymmetry,
}

impl Default for SlideShape {
    fn default() -> Self {
        Self {
            canonical: CanonicalSlideShape::default(),
            symmetry: crate::symmetry::direct(),
        }
    }
}

impl From<lnmai_core_verify::shape::SlideShape> for SlideShape {
    fn from(shape: lnmai_core_verify::shape::SlideShape) -> Self {
        Self {
            canonical: shape.canonical.into(),
            symmetry: shape.symmetry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSlideBody {
    pub raw_text: String,
    pub start_lane: OuterSlot,
    pub kind: SlideBodyKind,
    pub end_area: Option<SensorArea>,
    pub turn_area: Option<SensorArea>,
}

impl Default for ParsedSlideBody {
    fn default() -> Self {
        Self {
            raw_text: String::new(),
            start_lane: OuterSlot::S1,
            kind: SlideBodyKind::Line,
            end_area: None,
            turn_area: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideNoteSemantics {
    pub raw_text: String,
    pub start_slot: OuterSlot,
    pub end_area: SensorArea,
    pub shape: SlideShape,
    pub is_just_right: bool,
}

impl Default for SlideNoteSemantics {
    fn default() -> Self {
        Self {
            raw_text: String::new(),
            start_slot: OuterSlot::S1,
            end_area: SensorArea::A1,
            shape: SlideShape::default(),
            is_just_right: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingPointSemantics {
    pub timing: TimePoint,
    pub bpm: Rat,
    pub h_speed: Rat,
    pub notes: Vec<SlideNoteSemantics>,
}

impl Default for TimingPointSemantics {
    fn default() -> Self {
        Self {
            timing: TimePoint::zero(),
            bpm: Rat::zero(),
            h_speed: Rat::zero(),
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SimaiChartSemantics {
    pub timing_points: Vec<TimingPointSemantics>,
    pub comma_timings: Vec<TimePoint>,
    pub title: String,
    pub designer: String,
    pub level: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RawNoteKind {
    #[default]
    Tap,
    Hold,
    Slide,
    Touch,
    TouchHold,
    Rest,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaidataMetadata {
    pub fields: Vec<(String, String)>,
}

pub type MaidataChartBlock =
    lnmai_core_verify::maidata_line_adapter::MaidataChartBlock<String, Natural>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaidataFile {
    pub metadata: MaidataMetadata,
    pub charts: Vec<MaidataChartBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNoteToken {
    pub raw_text: String,
    pub kind: RawNoteKind,
    pub timing: TimePoint,
    pub bpm: Rat,
    pub h_speed: Rat,
    pub divisor: Natural,
    pub slot: Option<OuterSlot>,
    pub sensor_pos: Option<SensorArea>,
    pub slide_body: Option<ParsedSlideBody>,
    pub length: Option<Duration>,
    pub star_wait: Option<Duration>,
    pub is_break: bool,
    pub is_ex: bool,
    pub is_hanabi: bool,
    pub is_slide_no_head: bool,
    pub is_force_star: bool,
    pub is_fake_rotate: bool,
    pub is_slide_break: bool,
    pub source_group_id: Option<Natural>,
    pub source_group_index: Option<Natural>,
    pub source_group_size: Option<Natural>,
    pub source_pos: Option<SourceSpan>,
}

impl Default for RawNoteToken {
    fn default() -> Self {
        Self {
            raw_text: String::new(),
            kind: RawNoteKind::Tap,
            timing: TimePoint::zero(),
            bpm: Rat::zero(),
            h_speed: Rat::zero(),
            divisor: 0.into(),
            slot: None,
            sensor_pos: None,
            slide_body: None,
            length: None,
            star_wait: None,
            is_break: false,
            is_ex: false,
            is_hanabi: false,
            is_slide_no_head: false,
            is_force_star: false,
            is_fake_rotate: false,
            is_slide_break: false,
            source_group_id: None,
            source_group_index: None,
            source_group_size: None,
            source_pos: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceNote {
    pub token: RawNoteToken,
    pub source_pos: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEvent {
    pub timing: TimePoint,
    pub bpm: Rat,
    pub h_speed: Rat,
    pub divisor: Natural,
    pub notes: Vec<SourceNote>,
    pub source_pos: Option<SourceSpan>,
}

impl Default for SourceEvent {
    fn default() -> Self {
        SourceEvent {
            timing: TimePoint::zero(),
            bpm: Rat::zero(),
            h_speed: Rat::zero(),
            divisor: 0.into(),
            notes: Vec::new(),
            source_pos: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceChart {
    pub events: Vec<SourceEvent>,
}
