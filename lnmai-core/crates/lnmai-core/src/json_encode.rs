//! Public counterparts of Lean's `ToJson` instances.
//!
//! Derived schemas share their field/tag declarations with `json_decode`.
//! Custom schemas use the same encoders as the FFI. Arbitrary precision values
//! remain JSON numbers; a Lean triple is the nested product `[a,[b,c]]`.

use crate::{
    chart_loader::{SlideChartNote, SlideHeadChartNote},
    events::{AudioCommand, RenderCommand},
    input_model::{TimedInputEvent, ZoneQueue},
    integer::Integer,
    lifecycle::{HoldStart, HoldSubState, SlideState, TapFamilyNote},
    natural::Natural,
    rat::Rat,
    runtime_score::{NoteTypeJudgeCounts, RuntimeScoreState},
    simai::syntax::CanonicalSlideShape,
    storage::{ButtonVec, SensorVec},
    symmetry::SlideSymmetry,
    time::{Duration, TimePoint, TimeTick},
    types::{JudgeEvent, JudgePhase, RuntimePos},
};
use num_bigint::{BigInt, BigUint};
use serde_json::{json, Value};

pub trait Encode {
    fn to_json(&self) -> Value;
}

/// `Json.compress (toJson value)`, including Lean's string escaping.
pub fn to_string<T: Encode + ?Sized>(value: &T) -> String {
    crate::lean_json::compress(&value.to_json())
}

impl<T: Encode + ?Sized> Encode for &T {
    fn to_json(&self) -> Value {
        T::to_json(self)
    }
}

impl Encode for bool {
    fn to_json(&self) -> Value {
        Value::Bool(*self)
    }
}
impl Encode for str {
    fn to_json(&self) -> Value {
        Value::String(self.to_owned())
    }
}
impl Encode for String {
    fn to_json(&self) -> Value {
        self.as_str().to_json()
    }
}

macro_rules! number {
    ($($ty:ty),* $(,)?) => {$(
        impl Encode for $ty {
            fn to_json(&self) -> Value {
                Value::Number(self.to_string().parse().expect("integer JSON number"))
            }
        }
    )*};
}
number!(Integer, Natural, BigInt, BigUint, i64, u32, usize);

impl Encode for TimeTick {
    fn to_json(&self) -> Value {
        self.val.to_json()
    }
}
impl Encode for Duration {
    fn to_json(&self) -> Value {
        self.micros.to_json()
    }
}
impl Encode for TimePoint {
    fn to_json(&self) -> Value {
        self.micros.to_json()
    }
}
impl Encode for Rat {
    fn to_json(&self) -> Value {
        json!({
            "num": self.num().to_json(),
            "den": self.den().to_json(),
            "decimal": self.to_decimal_string(),
        })
    }
}

impl<T: Encode> Encode for Option<T> {
    fn to_json(&self) -> Value {
        self.as_ref().map_or(Value::Null, Encode::to_json)
    }
}
impl<T: Encode> Encode for [T] {
    fn to_json(&self) -> Value {
        Value::Array(self.iter().map(Encode::to_json).collect())
    }
}
impl<T: Encode> Encode for Vec<T> {
    fn to_json(&self) -> Value {
        self.as_slice().to_json()
    }
}
impl<A: Encode, B: Encode> Encode for (A, B) {
    fn to_json(&self) -> Value {
        json!([self.0.to_json(), self.1.to_json()])
    }
}
impl<A: Encode, B: Encode, C: Encode> Encode for (A, B, C) {
    fn to_json(&self) -> Value {
        json!([self.0.to_json(), [self.1.to_json(), self.2.to_json()]])
    }
}
impl<T: Encode> Encode for ButtonVec<T> {
    fn to_json(&self) -> Value {
        self.data.to_json()
    }
}
impl<T: Encode> Encode for SensorVec<T> {
    fn to_json(&self) -> Value {
        self.data.to_json()
    }
}
impl<T: Encode> Encode for ZoneQueue<T> {
    fn to_json(&self) -> Value {
        json!({"notes": self.notes.to_json(), "currentIndex": self.current_index.to_json()})
    }
}

impl Encode for JudgePhase {
    fn to_json(&self) -> Value {
        match self {
            Self::Head => json!("head"),
            Self::Tail => json!("tail"),
        }
    }
}
impl Encode for RuntimePos {
    fn to_json(&self) -> Value {
        match self {
            Self::Button(zone) => json!({"button": zone.to_json()}),
            Self::Sensor(area) => json!({"sensor": area.to_json()}),
        }
    }
}
impl Encode for HoldStart {
    fn to_json(&self) -> Value {
        match self {
            Self::Button(zone) => json!({"button": {"zone": zone.to_json()}}),
            Self::Sensor(area) => json!({"sensor": {"area": area.to_json()}}),
        }
    }
}
impl Encode for HoldSubState {
    fn to_json(&self) -> Value {
        match self {
            Self::HeadWaiting => json!("HeadWaiting"),
            Self::HeadJudgeable => json!("HeadJudgeable"),
            Self::HeadJudged(grade) => json!({"HeadJudged": {"grade": grade.to_json()}}),
            Self::BodyHeld => json!("BodyHeld"),
            Self::BodyReleased => json!("BodyReleased"),
            Self::Ended(grade) => json!({"Ended": {"grade": grade.to_json()}}),
        }
    }
}
impl Encode for SlideState {
    fn to_json(&self) -> Value {
        match self {
            Self::Waiting => json!("Waiting"),
            Self::Active(wait_time) => json!({"Active": {"waitTime": wait_time.to_json()}}),
            Self::Judged(grade, wait_time, judge_diff) => json!({"Judged": {
                "grade": grade.to_json(), "waitTime": wait_time.to_json(),
                "judgeDiff": judge_diff.to_json(),
            }}),
            Self::Ended => json!("Ended"),
        }
    }
}
impl Encode for CanonicalSlideShape {
    fn to_json(&self) -> Value {
        match self {
            Self::Line(n) => json!({"line": {"relEnd": n.to_json()}}),
            Self::Circle(n) => json!({"circle": {"relEnd": n.to_json()}}),
            Self::V(n) => json!({"v": {"relEnd": n.to_json()}}),
            Self::Turn(n) => json!({"turn": {"relEnd": n.to_json()}}),
            Self::Pq(n) => json!({"pq": {"relEnd": n.to_json()}}),
            Self::Ppqq(n) => json!({"ppqq": {"relEnd": n.to_json()}}),
            Self::S => json!("s"),
            Self::Wifi => json!("wifi"),
        }
    }
}
impl Encode for SlideSymmetry {
    fn to_json(&self) -> Value {
        json!({
            "rotationSteps": crate::symmetry::rotation_steps(*self),
            "mirrored": crate::symmetry::is_mirrored(*self),
        })
    }
}

impl Encode for SlideHeadChartNote {
    fn to_json(&self) -> Value {
        json!({
            "timing": self.timing.to_json(), "slot": self.slot.to_json(),
            "isBreak": self.is_break, "isEX": self.is_ex,
            "logicalSlideId": self.logical_slide_id.to_json(),
            "noteIndex": self.note_index.to_json(),
        })
    }
}
impl Encode for NoteTypeJudgeCounts {
    fn to_json(&self) -> Value {
        json!({
            "tapCount": self.tap.to_json(), "holdCount": self.hold.to_json(),
            "slideCount": self.slide.to_json(), "touchCount": self.touch.to_json(),
            "breakCount": self.break_.to_json(),
        })
    }
}

macro_rules! ffi_encoder {
    ($($ty:ty => $encoder:ident),* $(,)?) => {$(
        impl Encode for $ty {
            fn to_json(&self) -> Value {
                crate::ffi::$encoder(self)
            }
        }
    )*};
}
ffi_encoder! {
    JudgeEvent => judge_event_json,
    AudioCommand => audio_cmd_json,
    RenderCommand => render_cmd_json,
    TimedInputEvent => timed_event_json,
    TapFamilyNote => tap_family_json,
    SlideChartNote => slide_chart_note_json,
    [Natural; 15] => grade_counts_json,
    RuntimeScoreState => score_json,
}
