//! Runtime types with exact integer time; finite enums reuse the verified core.
pub use crate::input_model::TouchHoldBodyGroupState;
use crate::natural::Natural;
pub use crate::runtime_score::{
    combo_state, empty_judge_counts, empty_note_type_judge_counts, JudgeCounts,
    NoteTypeJudgeCounts, RuntimeScoreState as ScoreState,
};
use crate::time::Duration;
pub use lnmai_core_verify::types::{
    AreaPolicy, ComboState, JudgeDisplayOption, JudgeEventKind, JudgeGrade, JudgePhase, JudgeStyle,
    NoteStatus, NoteType, RuntimePos, SlideKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteJudgeResult {
    pub grade: JudgeGrade,
    pub diff: Duration,
    pub is_break: bool,
    pub is_ex: bool,
}

impl Default for NoteJudgeResult {
    fn default() -> Self {
        Self {
            grade: JudgeGrade::Miss,
            diff: Duration::zero(),
            is_break: false,
            is_ex: false,
        }
    }
}

impl NoteJudgeResult {
    pub fn is_fast(&self) -> bool {
        self.diff.to_micros() < 0
    }

    pub fn is_miss_or_too_fast(&self) -> bool {
        self.grade.is_miss_or_too_fast()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupState {
    pub group_id: Natural,
    pub count: Natural,
    pub size: Natural,
    pub grade: JudgeGrade,
    pub diff: Duration,
}

impl Default for GroupState {
    fn default() -> Self {
        Self {
            group_id: 0.into(),
            count: 0.into(),
            size: 0.into(),
            grade: JudgeGrade::Miss,
            diff: Duration::zero(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeEvent {
    pub kind: JudgeEventKind,
    pub phase: JudgePhase,
    pub grade: JudgeGrade,
    pub diff: Duration,
    pub position: RuntimePos,
    pub note_index: Natural,
    pub is_break: bool,
    pub is_ex: bool,
    pub multiple: Natural,
}

impl Default for JudgeEvent {
    fn default() -> Self {
        Self {
            kind: JudgeEventKind::Tap,
            phase: JudgePhase::Head,
            grade: JudgeGrade::Miss,
            diff: Duration::zero(),
            position: RuntimePos::Button(crate::areas::ButtonZone::K1),
            note_index: 0.into(),
            is_break: false,
            is_ex: false,
            multiple: 0.into(),
        }
    }
}

pub fn judge_grade_code(grade: JudgeGrade) -> &'static str {
    match grade {
        JudgeGrade::Miss => "Miss",
        JudgeGrade::LateGood => "LateGood",
        JudgeGrade::LateGreat3rd => "LateGreat3rd",
        JudgeGrade::LateGreat2nd => "LateGreat2nd",
        JudgeGrade::LateGreat => "LateGreat",
        JudgeGrade::LatePerfect3rd => "LatePerfect3rd",
        JudgeGrade::LatePerfect2nd => "LatePerfect2nd",
        JudgeGrade::Perfect => "Perfect",
        JudgeGrade::FastPerfect2nd => "FastPerfect2nd",
        JudgeGrade::FastPerfect3rd => "FastPerfect3rd",
        JudgeGrade::FastGreat => "FastGreat",
        JudgeGrade::FastGreat2nd => "FastGreat2nd",
        JudgeGrade::FastGreat3rd => "FastGreat3rd",
        JudgeGrade::FastGood => "FastGood",
        JudgeGrade::TooFast => "TooFast",
    }
}
