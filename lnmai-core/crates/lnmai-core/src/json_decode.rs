//! Lean-compatible decoders and shared public JSON schema declarations.
//!
//! Derived structures decode absent fields as null (so Option fields may be
//! absent). Custom Lean decoders distinguish missing-only defaults from defaults
//! on any decoding error. Keep that distinction here, including error order.
//! The derived schema macros also implement `json_encode::Encode`, so both
//! directions use the same field names and constructor tags.

use crate::{
    areas::*,
    chart_loader::*,
    default_tactic::{AutoplayGroup, AutoplayPart, AutoplayPlan},
    events::{AudioCommand, RenderCommand},
    ffi::{LoadedChartSummary, RuntimeStepLightResult, RuntimeStepResult},
    input_model::*,
    lifecycle::*,
    natural::Natural,
    rat::Rat,
    runtime_score::{NoteTypeJudgeCounts, RuntimeScoreState},
    simai::{
        ir::*,
        slide_tables::SlideAreaSpec,
        syntax::{
            CanonicalSlideShape, MaidataChartBlock, MaidataFile, MaidataMetadata, ParseError,
            ParseErrorKind, ParsedSlideBody, RawNoteKind, RawNoteToken, SimaiChartSemantics,
            SlideBodyKind, SlideKind as SimaiSlideKind, SlideNoteSemantics, SlideShape,
            SourceChart, SourceEvent, SourceNote, SourcePos, SourceSpan, TimingPointSemantics,
        },
    },
    storage::{ButtonVec, SensorVec},
    symmetry::SlideSymmetry,
    time::{Duration, TimePoint, TimeTick},
    types::*,
};
use num_bigint::{BigInt, BigUint};
use serde_json::Value;

pub trait Decode: Sized {
    fn decode(value: &Value) -> Result<Self, String>;
}

pub fn from_str<T: Decode>(text: &str) -> Result<T, String> {
    T::decode(&crate::lean_json::parse(text)?)
}

fn field<T: Decode>(v: &Value, key: &str) -> Result<T, String> {
    T::decode(v.get(key).unwrap_or(&Value::Null))
}

fn property<'a>(v: &'a Value, key: &str) -> Result<&'a Value, String> {
    v.as_object()
        .ok_or_else(|| "object expected".to_string())?
        .get(key)
        .ok_or_else(|| format!("property not found: {key}"))
}

fn optional<T: Decode>(v: &Value, key: &str, fallback: T) -> Result<T, String> {
    match v.get(key) {
        Some(value) => T::decode(value),
        None => Ok(fallback),
    }
}

fn tag(v: &Value) -> Result<&str, String> {
    if let Some(s) = v.as_str() {
        return Ok(s);
    }
    if let Some(m) = v.as_object() {
        if m.len() == 1 {
            return Ok(m.keys().next().unwrap());
        }
    }
    Err("no inductive tag found".into())
}

fn ctor<'a>(v: &'a Value, name: &str, fields: &[&str]) -> Result<Vec<&'a Value>, String> {
    let payload = property(v, name)?;
    // Lean retrieves all fields before decoding any of them.
    fields.iter().map(|key| property(payload, key)).collect()
}

fn pretty(v: &Value) -> String {
    crate::lean_json::pretty(v)
}

impl Decode for bool {
    fn decode(v: &Value) -> Result<Self, String> {
        v.as_bool().ok_or_else(|| "Bool expected".into())
    }
}
impl Decode for String {
    fn decode(v: &Value) -> Result<Self, String> {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| "String expected".into())
    }
}
impl Decode for i64 {
    fn decode(v: &Value) -> Result<Self, String> {
        crate::lean_json::integer(v)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| "Integer expected".into())
    }
}
impl Decode for BigInt {
    fn decode(v: &Value) -> Result<Self, String> {
        crate::lean_json::integer(v)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| "Integer expected".into())
    }
}
impl Decode for crate::integer::Integer {
    fn decode(v: &Value) -> Result<Self, String> {
        BigInt::decode(v).map(Into::into)
    }
}
impl Decode for BigUint {
    fn decode(v: &Value) -> Result<Self, String> {
        crate::lean_json::integer(v)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| "Natural number expected".into())
    }
}
impl Decode for crate::natural::Natural {
    fn decode(v: &Value) -> Result<Self, String> {
        BigUint::decode(v).map(Into::into)
    }
}
impl Decode for usize {
    fn decode(v: &Value) -> Result<Self, String> {
        crate::lean_json::integer(v)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| "Natural number expected".into())
    }
}
impl Decode for TimePoint {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self::from_micros(BigInt::decode(v)?))
    }
}
impl Decode for Duration {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self::from_micros(BigInt::decode(v)?))
    }
}
impl Decode for TimeTick {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self::of_int(BigInt::decode(v)?))
    }
}
impl Decode for Rat {
    fn decode(v: &Value) -> Result<Self, String> {
        let num = property(v, "num")?;
        let den = property(v, "den")?;
        let num = BigInt::decode(num)?;
        let den = BigUint::decode(den)?;
        if den == BigUint::from(0u8) {
            Err("rational denominator must be nonzero".into())
        } else {
            Ok(Self::new(num, den))
        }
    }
}
impl<T: Decode> Decode for Option<T> {
    fn decode(v: &Value) -> Result<Self, String> {
        if v.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::decode(v)?))
        }
    }
}
impl<T: Decode> Decode for Vec<T> {
    fn decode(v: &Value) -> Result<Self, String> {
        v.as_array()
            .ok_or_else(|| format!("expected JSON array, got '{}'", pretty(v)))?
            .iter()
            .map(T::decode)
            .collect()
    }
}
impl<A: Decode, B: Decode> Decode for (A, B) {
    fn decode(v: &Value) -> Result<Self, String> {
        match v.as_array().map(Vec::as_slice) {
            Some([a, b]) => Ok((A::decode(a)?, B::decode(b)?)),
            _ => Err(format!("expected pair, got '{}'", pretty(v))),
        }
    }
}
impl<A: Decode, B: Decode, C: Decode> Decode for (A, B, C) {
    fn decode(v: &Value) -> Result<Self, String> {
        let (a, (b, c)) = <(A, (B, C))>::decode(v)?;
        Ok((a, b, c))
    }
}
impl<T: Decode> Decode for ButtonVec<T> {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self {
            data: Vec::decode(v)?,
        })
    }
}
impl<T: Decode> Decode for SensorVec<T> {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self {
            data: Vec::decode(v)?,
        })
    }
}
impl<T: Decode> Decode for ZoneQueue<T> {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self {
            notes: field(v, "notes")?,
            current_index: field(v, "currentIndex")?,
        })
    }
}

macro_rules! derived_struct {
    ($ty:ident, $name:literal, {$($field:ident: $key:literal),* $(,)?}) => {
        impl Decode for $ty {
            fn decode(v: &Value) -> Result<Self, String> {
                Ok(Self { $($field: field(v,$key).map_err(|e| format!("{}.{}: {}",$name,$key,e))?),* })
            }
        }
        impl crate::json_encode::Encode for $ty {
            fn to_json(&self) -> Value {
                serde_json::json!({$($key: crate::json_encode::Encode::to_json(&self.$field)),*})
            }
        }
    };
}
macro_rules! derived_enum {
    ($ty:ident, {$($variant:ident: $tag:literal),* $(,)?}) => {
        impl Decode for $ty {
            fn decode(v: &Value) -> Result<Self, String> {
                match tag(v)? { $($tag => Ok(Self::$variant),)* _ => Err("no inductive constructor matched".into()) }
            }
        }
        impl crate::json_encode::Encode for $ty {
            fn to_json(&self) -> Value {
                Value::String(match self {$(Self::$variant => $tag),*}.into())
            }
        }
    };
    ($ty:ident, [$($variant:ident),* $(,)?]) => {
        impl Decode for $ty {
            fn decode(v: &Value) -> Result<Self, String> {
                match tag(v)? { $(stringify!($variant) => Ok(Self::$variant),)* _ => Err("no inductive constructor matched".into()) }
            }
        }
        impl crate::json_encode::Encode for $ty {
            fn to_json(&self) -> Value {
                Value::String(match self {$(Self::$variant => stringify!($variant)),*}.into())
            }
        }
    };
}
macro_rules! area_enum {
    ($ty:ident, [$($variant:ident),* $(,)?]) => {
        impl Decode for $ty {
            fn decode(v: &Value) -> Result<Self, String> {
                match v.as_str() { $(Some(stringify!($variant)) => Ok(Self::$variant),)* _ => Err(concat!("invalid ", stringify!($ty)).into()) }
            }
        }
        impl crate::json_encode::Encode for $ty {
            fn to_json(&self) -> Value {
                Value::String(match self {$(Self::$variant => stringify!($variant)),*}.into())
            }
        }
    };
}
area_enum!(
    SensorArea,
    [
        A1, A2, A3, A4, A5, A6, A7, A8, B1, B2, B3, B4, B5, B6, B7, B8, C, D1, D2, D3, D4, D5, D6,
        D7, D8, E1, E2, E3, E4, E5, E6, E7, E8
    ]
);
area_enum!(ButtonZone, [K1, K2, K3, K4, K5, K6, K7, K8]);
area_enum!(OuterSlot, [S1, S2, S3, S4, S5, S6, S7, S8]);
derived_enum!(
    JudgeGrade,
    [
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
        TooFast
    ]
);
derived_enum!(JudgeStyle, [Default, Maji, Gachi, Gori]);
derived_enum!(
    JudgeDisplayOption,
    [All, BelowCP, BelowP, BelowGR, MissOnly, Disable]
);
derived_enum!(AreaPolicy, [Or, And]);
derived_enum!(SlideKind, [Single, Wifi, ConnPart]);
derived_enum!(NoteType, [Tap, Hold, Slide, Touch, Break]);
derived_enum!(JudgeEventKind, [Tap, Hold, Slide, Touch, Break]);

impl Decode for JudgePhase {
    fn decode(v: &Value) -> Result<Self, String> {
        match tag(v)? {
            "head" => Ok(Self::Head),
            "tail" => Ok(Self::Tail),
            _ => Err("no inductive constructor matched".into()),
        }
    }
}

impl Decode for RuntimePos {
    fn decode(v: &Value) -> Result<Self, String> {
        if !v.is_object() {
            return Err("invalid RuntimePos".into());
        }
        if let Some(button) = v.get("button") {
            return ButtonZone::decode(button).map(Self::Button);
        }
        if let Some(sensor) = v.get("sensor") {
            return SensorArea::decode(sensor).map(Self::Sensor);
        }
        Err("invalid RuntimePos".into())
    }
}

impl Decode for JudgeEvent {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self {
            kind: field(v, "kind")?,
            phase: field(v, "phase")?,
            grade: field(v, "grade")?,
            diff: field(v, "diff")?,
            position: field(v, "position")?,
            note_index: field(v, "noteIndex")?,
            is_break: field(v, "isBreak").unwrap_or(false),
            is_ex: field(v, "isEX").unwrap_or(false),
            multiple: field(v, "multiple").unwrap_or_else(|_| 1.into()),
        })
    }
}

impl Decode for AudioCommand {
    fn decode(v: &Value) -> Result<Self, String> {
        match tag(v)? {
            "PlayJudgeSfx" => {
                let f = ctor(
                    v,
                    "PlayJudgeSfx",
                    &["kind", "grade", "isBreak", "atTime", "noteIndex"],
                )?;
                Ok(Self::PlayJudgeSfx {
                    kind: Decode::decode(f[0])?,
                    grade: Decode::decode(f[1])?,
                    is_break: Decode::decode(f[2])?,
                    at_time: Decode::decode(f[3])?,
                    note_index: Decode::decode(f[4])?,
                })
            }
            "PlaySlideCue" => {
                let f = ctor(
                    v,
                    "PlaySlideCue",
                    &["noteIndex", "trackIndex", "isBreak", "atTime"],
                )?;
                Ok(Self::PlaySlideCue {
                    note_index: Decode::decode(f[0])?,
                    track_index: Decode::decode(f[1])?,
                    is_break: Decode::decode(f[2])?,
                    at_time: Decode::decode(f[3])?,
                })
            }
            "PlayTouchHoldBody" => {
                let f = ctor(v, "PlayTouchHoldBody", &["noteIndex", "atTime"])?;
                Ok(Self::PlayTouchHoldBody {
                    note_index: Decode::decode(f[0])?,
                    at_time: Decode::decode(f[1])?,
                })
            }
            _ => Err("no inductive constructor matched".into()),
        }
    }
}

impl Decode for RenderCommand {
    fn decode(v: &Value) -> Result<Self, String> {
        match tag(v)? {
            "ShowJudgeResult" => {
                let f = ctor(
                    v,
                    "ShowJudgeResult",
                    &["kind", "grade", "isBreak", "diff", "noteIndex"],
                )?;
                Ok(Self::ShowJudgeResult {
                    kind: Decode::decode(f[0])?,
                    grade: Decode::decode(f[1])?,
                    is_break: Decode::decode(f[2])?,
                    diff: Decode::decode(f[3])?,
                    note_index: Decode::decode(f[4])?,
                })
            }
            "UpdateSlideProgress" => {
                let f = ctor(v, "UpdateSlideProgress", &["noteIndex", "remaining"])?;
                Ok(Self::UpdateSlideProgress {
                    note_index: Decode::decode(f[0])?,
                    remaining: Decode::decode(f[1])?,
                })
            }
            "UpdateSlideTrackProgress" => {
                let f = ctor(
                    v,
                    "UpdateSlideTrackProgress",
                    &["noteIndex", "trackIndex", "remaining"],
                )?;
                Ok(Self::UpdateSlideTrackProgress {
                    note_index: Decode::decode(f[0])?,
                    track_index: Decode::decode(f[1])?,
                    remaining: Decode::decode(f[2])?,
                })
            }
            "HideAllSlideBars" => {
                let f = ctor(v, "HideAllSlideBars", &["noteIndex"])?;
                Ok(Self::HideAllSlideBars {
                    note_index: Decode::decode(f[0])?,
                })
            }
            "HideSlideBars" => {
                let f = ctor(v, "HideSlideBars", &["noteIndex", "endIndex"])?;
                Ok(Self::HideSlideBars {
                    note_index: Decode::decode(f[0])?,
                    end_index: Decode::decode(f[1])?,
                })
            }
            "HideSlideTrackBars" => {
                let f = ctor(
                    v,
                    "HideSlideTrackBars",
                    &["noteIndex", "trackIndex", "endIndex"],
                )?;
                Ok(Self::HideSlideTrackBars {
                    note_index: Decode::decode(f[0])?,
                    track_index: Decode::decode(f[1])?,
                    end_index: Decode::decode(f[2])?,
                })
            }
            _ => Err("no inductive constructor matched".into()),
        }
    }
}

derived_struct!(NoteJudgeResult, "LnmaiCore.NoteJudgeResult", {
    grade: "grade", diff: "diff", is_break: "isBreak", is_ex: "isEX",
});
derived_struct!(RuntimeStepResult, "LnmaiCore.FFI.RuntimeStepResult", {
    state: "state", events: "events", audio_commands: "audioCommands",
    render_commands: "renderCommands", terminated: "terminated",
});
derived_struct!(RuntimeStepLightResult, "LnmaiCore.FFI.RuntimeStepLightResult", {
    events: "events", audio_commands: "audioCommands", render_commands: "renderCommands",
    score: "score", current_time: "currentTime", terminated: "terminated",
});
derived_struct!(LoadedChartSummary, "LnmaiCore.FFI.LoadedChartSummary", {
    tap_count: "tapCount", hold_count: "holdCount", touch_count: "touchCount",
    touch_hold_count: "touchHoldCount", slide_head_count: "slideHeadCount", slide_count: "slideCount",
});

derived_enum!(ParseErrorKind, {
    InvalidSyntax: "invalidSyntax", InvalidShape: "invalidShape",
    InvalidEndPosition: "invalidEndPosition", InvalidTurnPosition: "invalidTurnPosition",
    InvalidChainTiming: "invalidChainTiming",
});
derived_enum!(SimaiSlideKind, {
    Line: "line", Circle: "circle", V: "v", Turn: "turn",
    Pq: "pq", Ppqq: "ppqq", S: "s", Wifi: "wifi",
});
derived_enum!(SlideBodyKind, {
    Line: "line", CircleRight: "circleRight", CircleLeft: "circleLeft",
    CircleUp: "circleUp", V: "v", Pp: "pp", Qq: "qq", P: "p", Q: "q",
    S: "s", Z: "z", Turn: "turn", Wifi: "wifi",
});
derived_enum!(RawNoteKind, {
    Tap: "tap", Hold: "hold", Slide: "slide", Touch: "touch",
    TouchHold: "touchHold", Rest: "rest", Unknown: "unknown",
});

impl Decode for CanonicalSlideShape {
    fn decode(v: &Value) -> Result<Self, String> {
        let name = tag(v)?;
        match name {
            "s" => Ok(Self::S),
            "wifi" => Ok(Self::Wifi),
            "line" | "circle" | "v" | "turn" | "pq" | "ppqq" => {
                let f = ctor(v, name, &["relEnd"])?;
                let n = Natural::decode(f[0])?;
                Ok(match name {
                    "line" => Self::Line(n),
                    "circle" => Self::Circle(n),
                    "v" => Self::V(n),
                    "turn" => Self::Turn(n),
                    "pq" => Self::Pq(n),
                    _ => Self::Ppqq(n),
                })
            }
            _ => Err("no inductive constructor matched".into()),
        }
    }
}

impl Decode for SlideSymmetry {
    fn decode(v: &Value) -> Result<Self, String> {
        let rotation_steps: Natural = field(v, "rotationSteps")?;
        let mirrored: bool = field(v, "mirrored")?;
        let k = (rotation_steps % Natural::from(8u8)).to_word() as u32;
        Ok(if mirrored { Self::Sr(k) } else { Self::R(k) })
    }
}

derived_struct!(SourcePos, "LnmaiCore.Simai.SourcePos", {
    line: "line", column: "column",
});
derived_struct!(SourceSpan, "LnmaiCore.Simai.SourceSpan", {
    start: "start", stop: "stop",
});
derived_struct!(ParseError, "LnmaiCore.Simai.ParseError", {
    kind: "kind", raw_text: "rawText", message: "message", span: "span",
});
derived_struct!(SlideShape, "LnmaiCore.Simai.SlideShape", {
    canonical: "canonical", symmetry: "symmetry",
});
derived_struct!(ParsedSlideBody, "LnmaiCore.Simai.ParsedSlideBody", {
    raw_text: "rawText", start_lane: "startLane", kind: "kind",
    end_area: "endArea", turn_area: "turnArea",
});
derived_struct!(SlideNoteSemantics, "LnmaiCore.Simai.SlideNoteSemantics", {
    raw_text: "rawText", start_slot: "startSlot", end_area: "endArea",
    shape: "shape", is_just_right: "isJustRight",
});
derived_struct!(TimingPointSemantics, "LnmaiCore.Simai.TimingPointSemantics", {
    timing: "timing", bpm: "bpm", h_speed: "hSpeed", notes: "notes",
});
derived_struct!(SimaiChartSemantics, "LnmaiCore.Simai.SimaiChartSemantics", {
    timing_points: "timingPoints", comma_timings: "commaTimings",
    title: "title", designer: "designer", level: "level",
});
derived_struct!(MaidataMetadata, "LnmaiCore.Simai.MaidataMetadata", {
    fields: "fields",
});
derived_struct!(MaidataChartBlock, "LnmaiCore.Simai.MaidataChartBlock", {
    level_index: "levelIndex", raw_body: "rawBody",
});
derived_struct!(MaidataFile, "LnmaiCore.Simai.MaidataFile", {
    metadata: "metadata", charts: "charts",
});
derived_struct!(RawNoteToken, "LnmaiCore.Simai.RawNoteToken", {
    raw_text: "rawText", kind: "kind", timing: "timing", bpm: "bpm",
    h_speed: "hSpeed", divisor: "divisor", slot: "slot", sensor_pos: "sensorPos",
    slide_body: "slideBody", length: "length", star_wait: "starWait",
    is_break: "isBreak", is_ex: "isEX", is_hanabi: "isHanabi",
    is_slide_no_head: "isSlideNoHead", is_force_star: "isForceStar",
    is_fake_rotate: "isFakeRotate", is_slide_break: "isSlideBreak",
    source_group_id: "sourceGroupId", source_group_index: "sourceGroupIndex",
    source_group_size: "sourceGroupSize", source_pos: "sourcePos",
});
derived_struct!(SourceNote, "LnmaiCore.Simai.SourceNote", {
    token: "token", source_pos: "sourcePos",
});
derived_struct!(SourceEvent, "LnmaiCore.Simai.SourceEvent", {
    timing: "timing", bpm: "bpm", h_speed: "hSpeed", divisor: "divisor",
    notes: "notes", source_pos: "sourcePos",
});
derived_struct!(SourceChart, "LnmaiCore.Simai.SourceChart", {
    events: "events",
});
derived_struct!(NormalizedSlideDebug, "LnmaiCore.Simai.NormalizedSlideDebug", {
    note_index: "noteIndex", raw_text: "rawText",
});
derived_struct!(NormalizedTap, "LnmaiCore.Simai.NormalizedTap", {
    timing: "timing", slot: "slot", is_break: "isBreak", is_ex: "isEX",
    is_hanabi: "isHanabi", is_force_star: "isForceStar", note_index: "noteIndex",
});
derived_struct!(NormalizedHold, "LnmaiCore.Simai.NormalizedHold", {
    timing: "timing", slot: "slot", length: "length", is_break: "isBreak",
    is_ex: "isEX", is_hanabi: "isHanabi", note_index: "noteIndex",
});
derived_struct!(NormalizedTouchHold, "LnmaiCore.Simai.NormalizedTouchHold", {
    timing: "timing", sensor_pos: "sensorPos", length: "length",
    is_break: "isBreak", is_ex: "isEX", is_hanabi: "isHanabi",
    source_group_id: "sourceGroupId", source_group_index: "sourceGroupIndex",
    source_group_size: "sourceGroupSize", note_index: "noteIndex",
});
derived_struct!(NormalizedTouch, "LnmaiCore.Simai.NormalizedTouch", {
    timing: "timing", sensor_pos: "sensorPos", is_break: "isBreak", is_hanabi: "isHanabi",
    source_group_id: "sourceGroupId", source_group_index: "sourceGroupIndex",
    source_group_size: "sourceGroupSize", note_index: "noteIndex",
});
derived_struct!(NormalizedSlide, "LnmaiCore.Simai.NormalizedSlide", {
    head_timing: "headTiming", slot: "slot", length: "length", start_timing: "startTiming",
    group_start_timing: "groupStartTiming", h_speed: "hSpeed", slide_kind: "slideKind",
    is_classic: "isClassic", track_count: "trackCount", judge_at: "judgeAt",
    is_break: "isBreak", is_ex: "isEX", is_hanabi: "isHanabi",
    has_head_note: "hasHeadNote", has_body: "hasBody", is_slide_no_head: "isSlideNoHead",
    is_force_star: "isForceStar", is_fake_rotate: "isFakeRotate", is_slide_break: "isSlideBreak",
    is_conn_slide: "isConnSlide", parent_note_index: "parentNoteIndex",
    is_group_head: "isGroupHead", is_group_end: "isGroupEnd",
    total_judge_queue_len: "totalJudgeQueueLen", judge_queues: "judgeQueues",
    source_group_id: "sourceGroupId", source_group_index: "sourceGroupIndex",
    source_group_size: "sourceGroupSize", multiple: "multiple", note_index: "noteIndex",
    simai_shape: "simaiShape",
});
derived_struct!(NormalizedChart, "LnmaiCore.Simai.NormalizedChart", {
    taps: "taps", holds: "holds", touches: "touches", touch_holds: "touchHolds",
    slides: "slides", slide_debug: "slideDebug", slide_skipping: "slideSkipping",
});
derived_struct!(FrontendChartInspection, "LnmaiCore.Simai.FrontendChartInspection", {
    metadata: "metadata", chart: "chart", source: "source",
    tokens: "tokens", slide_notes: "slideNotes",
});
derived_struct!(FrontendSemanticChart, "LnmaiCore.Simai.FrontendSemanticChart", {
    normalized: "normalized", lowered: "lowered",
});
derived_struct!(FrontendChartResult, "LnmaiCore.Simai.FrontendChartResult", {
    semantic: "semantic", inspection: "inspection",
});
derived_struct!(AutoplayPart, "LnmaiCore.AutoplayPart", {
    runtime_note_index: "runtimeNoteIndex", role: "role", track_index: "trackIndex",
    events: "events",
});
derived_struct!(AutoplayGroup, "LnmaiCore.AutoplayGroup", {
    id: "id", kind: "kind", runtime_note_indices: "runtimeNoteIndices",
    parts: "parts", depends_on_group_ids: "dependsOnGroupIds",
});
derived_struct!(AutoplayPlan, "LnmaiCore.AutoplayPlan", {
    version: "version", groups: "groups",
});

impl Decode for TimedInputEvent {
    fn decode(v: &Value) -> Result<Self, String> {
        let name = tag(v)?;
        let keys: &[&str] = match name {
            "buttonClick" => &["tp", "zone"],
            "buttonHold" => &["tp", "zone", "isDown"],
            "sensorClick" => &["tp", "area"],
            "sensorHold" => &["tp", "area", "isDown"],
            _ => return Err("no inductive constructor matched".into()),
        };
        let f = ctor(v, name, keys)?;
        Ok(match name {
            "buttonClick" => Self::ButtonClick(TimePoint::decode(f[0])?, ButtonZone::decode(f[1])?),
            "buttonHold" => Self::ButtonHold(
                TimePoint::decode(f[0])?,
                ButtonZone::decode(f[1])?,
                bool::decode(f[2])?,
            ),
            "sensorClick" => Self::SensorClick(TimePoint::decode(f[0])?, SensorArea::decode(f[1])?),
            _ => Self::SensorHold(
                TimePoint::decode(f[0])?,
                SensorArea::decode(f[1])?,
                bool::decode(f[2])?,
            ),
        })
    }
}

impl Decode for HoldStart {
    fn decode(v: &Value) -> Result<Self, String> {
        match tag(v)? {
            "button" => Ok(Self::Button(ButtonZone::decode(
                ctor(v, "button", &["zone"])?[0],
            )?)),
            "sensor" => Ok(Self::Sensor(SensorArea::decode(
                ctor(v, "sensor", &["area"])?[0],
            )?)),
            _ => Err("no inductive constructor matched".into()),
        }
    }
}
macro_rules! tap_state {
    ($ty:ident) => {
        impl Decode for $ty {
            fn decode(v: &Value) -> Result<Self, String> {
                match tag(v)? {
                    "Waiting" => Ok(Self::Waiting),
                    "Judgeable" => Ok(Self::Judgeable),
                    "Ended" => Ok(Self::Ended),
                    "Judged" => Ok(Self::Judged(JudgeGrade::decode(
                        ctor(v, "Judged", &["grade"])?[0],
                    )?)),
                    _ => Err("no inductive constructor matched".into()),
                }
            }
        }
        impl crate::json_encode::Encode for $ty {
            fn to_json(&self) -> Value {
                match self {
                    Self::Waiting => serde_json::json!("Waiting"),
                    Self::Judgeable => serde_json::json!("Judgeable"),
                    Self::Judged(grade) => serde_json::json!({"Judged": {"grade": crate::json_encode::Encode::to_json(grade)}}),
                    Self::Ended => serde_json::json!("Ended"),
                }
            }
        }
    };
}
tap_state!(TapState);
tap_state!(TouchState);
impl Decode for HoldSubState {
    fn decode(v: &Value) -> Result<Self, String> {
        match tag(v)? {
            "HeadWaiting" => Ok(Self::HeadWaiting),
            "HeadJudgeable" => Ok(Self::HeadJudgeable),
            "BodyHeld" => Ok(Self::BodyHeld),
            "BodyReleased" => Ok(Self::BodyReleased),
            "HeadJudged" => Ok(Self::HeadJudged(JudgeGrade::decode(
                ctor(v, "HeadJudged", &["grade"])?[0],
            )?)),
            "Ended" => Ok(Self::Ended(JudgeGrade::decode(
                ctor(v, "Ended", &["grade"])?[0],
            )?)),
            _ => Err("no inductive constructor matched".into()),
        }
    }
}
impl Decode for SlideState {
    fn decode(v: &Value) -> Result<Self, String> {
        match tag(v)? {
            "Waiting" => Ok(Self::Waiting),
            "Ended" => Ok(Self::Ended),
            "Active" => Ok(Self::Active(Duration::decode(
                ctor(v, "Active", &["waitTime"])?[0],
            )?)),
            "Judged" => {
                let f = ctor(v, "Judged", &["grade", "waitTime", "judgeDiff"])?;
                Ok(Self::Judged(
                    JudgeGrade::decode(f[0])?,
                    Duration::decode(f[1])?,
                    Duration::decode(f[2])?,
                ))
            }
            _ => Err("no inductive constructor matched".into()),
        }
    }
}
impl Decode for TapFamilyNote {
    fn decode(v: &Value) -> Result<Self, String> {
        let kind: String = field(v, "kind")?;
        let params: CommonNoteParams = field(v, "params")?;
        let lane = field(v, "lane")?;
        let state = field(v, "state")?;
        let button_queue_index: crate::natural::Natural =
            optional(v, "buttonQueueIndex", 0.into())?;
        match kind.as_str() {
            "tap" => Ok(Self::Tap(TapNote {
                params,
                lane,
                state,
                button_queue_index,
            })),
            "slideHead" => Ok(Self::SlideHead(SlideHeadNote {
                logical_slide_id: optional(v, "logicalSlideId", params.note_index.clone())?,
                params,
                lane,
                state,
                button_queue_index,
            })),
            other => Err(format!("unknown TapFamilyNote kind: {other}")),
        }
    }
}
impl Decode for SlideHeadChartNote {
    fn decode(v: &Value) -> Result<Self, String> {
        let timing = field(v, "timing")?;
        let slot = field(v, "slot")?;
        let is_break = field(v, "isBreak").unwrap_or(false);
        let is_ex = field(v, "isEX").unwrap_or(false);
        let note_index: crate::natural::Natural = field(v, "noteIndex").unwrap_or_default();
        let logical_slide_id = optional(v, "logicalSlideId", note_index.clone())?;
        Ok(Self {
            timing,
            slot,
            is_break,
            is_ex,
            note_index,
            logical_slide_id,
        })
    }
}
impl Decode for SlideChartNote {
    fn decode(v: &Value) -> Result<Self, String> {
        let head_timing = field(v, "headTiming")?;
        let slot = field(v, "slot")?;
        let length = field(v, "length")?;
        let note_index: crate::natural::Natural = field(v, "noteIndex").unwrap_or_default();
        let logical_slide_id = optional(v, "logicalSlideId", note_index.clone())?;
        Ok(Self {
            head_timing,
            slot,
            length,
            note_index,
            logical_slide_id,
            start_timing: field(v, "startTiming").unwrap_or(TimePoint::zero()),
            group_start_timing: field(v, "groupStartTiming").unwrap_or(None),
            slide_kind: field(v, "slideKind").unwrap_or(SlideKind::Single),
            is_classic: field(v, "isClassic").unwrap_or(false),
            is_slide_no_head: field(v, "isSlideNoHead").unwrap_or(false),
            is_conn_slide: field(v, "isConnSlide").unwrap_or(false),
            parent_note_index: field(v, "parentNoteIndex").unwrap_or(None),
            is_group_head: field(v, "isGroupHead").unwrap_or(false),
            is_group_end: field(v, "isGroupEnd").unwrap_or(false),
            parent_finished: field(v, "parentFinished").unwrap_or(false),
            parent_pending_finish: field(v, "parentPendingFinish").unwrap_or(false),
            total_judge_queue_len: field(v, "totalJudgeQueueLen").unwrap_or((0).into()),
            track_count: field(v, "trackCount").unwrap_or((1).into()),
            judge_at: field(v, "judgeAt").unwrap_or(None),
            is_break: field(v, "isBreak").unwrap_or(false),
            is_ex: field(v, "isEX").unwrap_or(false),
            multiple: field(v, "multiple").unwrap_or((1).into()),
            judge_queues: field(v, "judgeQueues").unwrap_or_default(),
            debug_simai: field(v, "debugSimai").unwrap_or(None),
        })
    }
}
impl Decode for [crate::natural::Natural; 15] {
    fn decode(v: &Value) -> Result<Self, String> {
        let mut counts = std::array::from_fn(|_| crate::natural::Natural::default());
        for (i, name) in [
            "Miss",
            "LateGood",
            "LateGreat3rd",
            "LateGreat2nd",
            "LateGreat",
            "LatePerfect3rd",
            "LatePerfect2nd",
            "Perfect",
            "FastPerfect2nd",
            "FastPerfect3rd",
            "FastGreat",
            "FastGreat2nd",
            "FastGreat3rd",
            "FastGood",
            "TooFast",
        ]
        .iter()
        .enumerate()
        {
            counts[i] = field(v, name)?;
        }
        Ok(counts)
    }
}
impl Decode for NoteTypeJudgeCounts {
    fn decode(v: &Value) -> Result<Self, String> {
        Ok(Self {
            tap: Decode::decode(property(v, "tapCount")?)?,
            hold: Decode::decode(property(v, "holdCount")?)?,
            slide: Decode::decode(property(v, "slideCount")?)?,
            touch: Decode::decode(property(v, "touchCount")?)?,
            break_: Decode::decode(property(v, "breakCount")?)?,
        })
    }
}
impl Decode for RuntimeScoreState {
    fn decode(v: &Value) -> Result<Self, String> {
        if !v.is_object() {
            return Err("invalid ScoreState".into());
        }
        Ok(Self {
            combo: optional(v, "combo", Default::default())?,
            p_combo: optional(v, "pCombo", Default::default())?,
            c_p_combo: optional(v, "cPCombo", Default::default())?,
            total_base: optional(v, "totalBase", Default::default())?,
            total_extra: optional(v, "totalExtra", Default::default())?,
            earned_base: optional(v, "earnedBase", Default::default())?,
            earned_extra: optional(v, "earnedExtra", Default::default())?,
            earned_classic_extra: optional(v, "earnedClassicExtra", Default::default())?,
            lost_base: optional(v, "lostBase", Default::default())?,
            lost_extra: optional(v, "lostExtra", Default::default())?,
            lost_classic_extra: optional(v, "lostClassicExtra", Default::default())?,
            dx_score: optional(v, "dxScore", Default::default())?,
            max_dx_score: optional(v, "maxDxScore", Default::default())?,
            fast_count: optional(v, "fastCount", Default::default())?,
            late_count: optional(v, "lateCount", Default::default())?,
            counts: optional(v, "counts", NoteTypeJudgeCounts::default())?,
        })
    }
}

derived_struct!(TapChartNote, "LnmaiCore.ChartLoader.TapChartNote", {
    timing: "timing",
    slot: "slot",
    is_break: "isBreak",
    is_ex: "isEX",
    button_queue_index: "buttonQueueIndex",
    note_index: "noteIndex",
});

derived_struct!(HoldChartNote, "LnmaiCore.ChartLoader.HoldChartNote", {
    timing: "timing",
    slot: "slot",
    length: "length",
    is_break: "isBreak",
    is_ex: "isEX",
    is_touch: "isTouch",
    is_classic: "isClassic",
    button_queue_index: "buttonQueueIndex",
    touch_hold_group_id: "touchHoldGroupId",
    touch_hold_group_size: "touchHoldGroupSize",
    note_index: "noteIndex",
});

derived_struct!(TouchHoldChartNote, "LnmaiCore.ChartLoader.TouchHoldChartNote", {
    timing: "timing",
    sensor_pos: "sensorPos",
    length: "length",
    is_break: "isBreak",
    is_ex: "isEX",
    source_group_id: "sourceGroupId",
    source_group_index: "sourceGroupIndex",
    source_group_size: "sourceGroupSize",
    touch_queue_index: "touchQueueIndex",
    touch_group_id: "touchGroupId",
    touch_group_size: "touchGroupSize",
    touch_hold_group_id: "touchHoldGroupId",
    touch_hold_group_size: "touchHoldGroupSize",
    note_index: "noteIndex",
});

derived_struct!(TouchChartNote, "LnmaiCore.ChartLoader.TouchChartNote", {
    timing: "timing",
    sensor_pos: "sensorPos",
    is_break: "isBreak",
    source_group_id: "sourceGroupId",
    source_group_index: "sourceGroupIndex",
    source_group_size: "sourceGroupSize",
    touch_queue_index: "touchQueueIndex",
    touch_group_id: "touchGroupId",
    touch_group_size: "touchGroupSize",
    note_index: "noteIndex",
});

derived_struct!(ChartSpec, "LnmaiCore.ChartLoader.ChartSpec", {
    taps: "taps",
    holds: "holds",
    touches: "touches",
    touch_holds: "touchHolds",
    slide_heads: "slideHeads",
    slides: "slides",
    slide_skipping: "slideSkipping",
    button_ring_for_touch: "buttonRingForTouch",
});

derived_struct!(SlideAreaSpec, "LnmaiCore.Simai.SlideAreaSpec", {
    target_areas: "targetAreas",
    policy: "policy",
    is_last: "isLast",
    is_skippable: "isSkippable",
    arrow_progress_when_on: "arrowProgressWhenOn",
    arrow_progress_when_finished: "arrowProgressWhenFinished",
});

derived_struct!(FrameInput, "LnmaiCore.InputModel.FrameInput", {
    button_clicked: "buttonClicked",
    button_held: "buttonHeld",
    sensor_clicked: "sensorClicked",
    sensor_held: "sensorHeld",
    button_click_count: "buttonClickCount",
    sensor_click_count: "sensorClickCount",
    delta: "delta",
});

derived_struct!(TimedInputBatch, "LnmaiCore.InputModel.TimedInputBatch", {
    current_time: "currentTime",
    events: "events",
});

derived_struct!(GameState, "LnmaiCore.InputModel.GameState", {
    current_time: "currentTime",
    prev_button: "prevButton",
    prev_sensor: "prevSensor",
    button_queue_frontiers: "buttonQueueFrontiers",
    touch_queue_frontiers: "touchQueueFrontiers",
    tap_queues: "tapQueues",
    hold_queues: "holdQueues",
    touch_hold_queues: "touchHoldQueues",
    touch_queues: "touchQueues",
    slides: "slides",
    active_holds: "activeHolds",
    active_touch_holds: "activeTouchHolds",
    touch_group_states: "touchGroupStates",
    touch_hold_group_states: "touchHoldGroupStates",
    current_batch: "currentBatch",
    score: "score",
    judge_style: "judgeStyle",
    touch_panel_offset: "touchPanelOffset",
    subdivide_slide_judge_grade: "subdivideSlideJudgeGrade",
    note_fast_late_display: "noteFastLateDisplay",
    break_fast_late_display: "breakFastLateDisplay",
    display_hold_head_judge_result: "displayHoldHeadJudgeResult",
    button_ring_for_touch: "buttonRingForTouch",
});

derived_struct!(CommonNoteParams, "LnmaiCore.Lifecycle.CommonNoteParams", {
    judge_timing: "judgeTiming",
    judge_offset: "judgeOffset",
    is_break: "isBreak",
    is_ex: "isEX",
    note_index: "noteIndex",
});

derived_struct!(TapNote, "LnmaiCore.Lifecycle.TapNote", {
    params: "params", lane: "lane", state: "state",
    button_queue_index: "buttonQueueIndex",
});
derived_struct!(SlideHeadNote, "LnmaiCore.Lifecycle.SlideHeadNote", {
    params: "params", lane: "lane", state: "state",
    logical_slide_id: "logicalSlideId", button_queue_index: "buttonQueueIndex",
});

derived_struct!(HoldNote, "LnmaiCore.Lifecycle.HoldNote", {
    params: "params",
    start: "start",
    state: "state",
    length: "length",
    button_queue_index: "buttonQueueIndex",
    head_diff: "headDiff",
    head_grade: "headGrade",
    player_release_time: "playerReleaseTime",
    release_ignore_time: "releaseIgnoreTime",
    is_classic: "isClassic",
    is_touch_hold: "isTouchHold",
    touch_queue_index: "touchQueueIndex",
    touch_group_id: "touchGroupId",
    touch_group_size: "touchGroupSize",
    touch_hold_group_id: "touchHoldGroupId",
    touch_hold_group_size: "touchHoldGroupSize",
    touch_hold_group_triggered: "touchHoldGroupTriggered",
});

derived_struct!(TouchNote, "LnmaiCore.Lifecycle.TouchNote", {
    params: "params",
    state: "state",
    sensor_pos: "sensorPos",
    touch_queue_index: "touchQueueIndex",
    touch_group_id: "touchGroupId",
    touch_group_size: "touchGroupSize",
});

derived_struct!(SlideArea, "LnmaiCore.Lifecycle.SlideArea", {
    target_areas: "targetAreas",
    policy: "policy",
    is_last: "isLast",
    is_skippable: "isSkippable",
    arrow_progress_when_on: "arrowProgressWhenOn",
    arrow_progress_when_finished: "arrowProgressWhenFinished",
    was_on: "wasOn",
    was_off: "wasOff",
    target_was_on: "targetWasOn",
    target_was_off: "targetWasOff",
});

derived_struct!(SlideNote, "LnmaiCore.Lifecycle.SlideNote", {
    params: "params",
    lane: "lane",
    state: "state",
    length: "length",
    head_timing: "headTiming",
    start_timing: "startTiming",
    group_start_timing: "groupStartTiming",
    slide_kind: "slideKind",
    is_classic: "isClassic",
    is_conn_slide: "isConnSlide",
    parent_note_index: "parentNoteIndex",
    is_group_part_head: "isGroupPartHead",
    is_group_part_end: "isGroupPartEnd",
    parent_finished: "parentFinished",
    parent_pending_finish: "parentPendingFinish",
    initial_queue_remaining: "initialQueueRemaining",
    total_judge_queue_len: "totalJudgeQueueLen",
    track_count: "trackCount",
    is_checkable: "isCheckable",
    slide_sound_played: "slideSoundPlayed",
    multiple: "multiple",
    judge_queues: "judgeQueues",
});

derived_struct!(TouchHoldBodyGroupState, "LnmaiCore.TouchHoldBodyGroupState", {
    group_id: "groupId",
    member_note_indices: "memberNoteIndices",
    triggered_note_indices: "triggeredNoteIndices",
});

derived_struct!(GroupState, "LnmaiCore.GroupState", {
    group_id: "groupId",
    count: "count",
    size: "size",
    grade: "grade",
    diff: "diff",
});
