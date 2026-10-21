//! Core domain types.
//!
//! Mirrors `LnmaiCore/Types.lean`. Pure data only; no `String`, no
//! collections, so the module stays inside the Aeneas subset.

use super::areas::{ButtonZone, SensorArea};
use super::time::Duration;

/// Judgment grades (15-tier lattice, ordered by quality ascending).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum JudgeGrade {
    #[default]
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
    pub fn is_miss_or_too_fast(&self) -> bool {
        matches!(self, JudgeGrade::Miss | JudgeGrade::TooFast)
    }

    pub fn is_fast(&self) -> bool {
        matches!(
            self,
            JudgeGrade::FastPerfect2nd
                | JudgeGrade::FastPerfect3rd
                | JudgeGrade::FastGreat
                | JudgeGrade::FastGreat2nd
                | JudgeGrade::FastGreat3rd
                | JudgeGrade::FastGood
                | JudgeGrade::TooFast
        )
    }

    pub fn is_late(&self) -> bool {
        matches!(
            self,
            JudgeGrade::LateGood
                | JudgeGrade::LateGreat3rd
                | JudgeGrade::LateGreat2nd
                | JudgeGrade::LateGreat
                | JudgeGrade::LatePerfect3rd
                | JudgeGrade::LatePerfect2nd
                | JudgeGrade::Miss
        )
    }

    /// Distance from Critical Perfect (0 = Perfect, 7 = Miss/TooFast).
    pub fn dist_from_perfect(&self) -> u32 {
        match self {
            JudgeGrade::Miss => 7,
            JudgeGrade::LateGood => 6,
            JudgeGrade::LateGreat3rd => 5,
            JudgeGrade::LateGreat2nd => 4,
            JudgeGrade::LateGreat => 3,
            JudgeGrade::LatePerfect3rd => 2,
            JudgeGrade::LatePerfect2nd => 1,
            JudgeGrade::Perfect => 0,
            JudgeGrade::FastPerfect2nd => 1,
            JudgeGrade::FastPerfect3rd => 2,
            JudgeGrade::FastGreat => 3,
            JudgeGrade::FastGreat2nd => 4,
            JudgeGrade::FastGreat3rd => 5,
            JudgeGrade::FastGood => 6,
            JudgeGrade::TooFast => 7,
        }
    }

    pub fn is_perfect_grade(&self) -> bool {
        matches!(
            self,
            JudgeGrade::Perfect
                | JudgeGrade::LatePerfect2nd
                | JudgeGrade::LatePerfect3rd
                | JudgeGrade::FastPerfect2nd
                | JudgeGrade::FastPerfect3rd
        )
    }

    pub fn is_great_grade(&self) -> bool {
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

    pub fn is_good_grade(&self) -> bool {
        matches!(self, JudgeGrade::LateGood | JudgeGrade::FastGood)
    }
}

/// Display/counting option mirrored from MajdataPlay's `JudgeDisplayOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JudgeDisplayOption {
    #[default]
    All,
    BelowCP,
    BelowP,
    BelowGR,
    MissOnly,
    Disable,
}

/// Note types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoteType {
    #[default]
    Tap,
    Hold,
    Slide,
    Touch,
    Break,
}

impl NoteType {
    pub fn base_score(&self) -> u32 {
        match self {
            NoteType::Tap => 500,
            NoteType::Hold => 1000,
            NoteType::Slide => 1500,
            NoteType::Touch => 500,
            NoteType::Break => 2500,
        }
    }

    /// `NoteType.extraScore`: Break notes earn 100 extra DX score.
    pub fn extra_score(&self) -> u32 {
        match self {
            NoteType::Break => 100,
            _ => 0,
        }
    }
}

/// Slide kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlideKind {
    #[default]
    Single,
    Wifi,
    ConnPart,
}

/// Area policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AreaPolicy {
    #[default]
    Or,
    And,
}

/// Difficulty / judgment style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JudgeStyle {
    #[default]
    Default,
    Maji,
    Gachi,
    Gori,
}

/// Note lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoteStatus {
    #[default]
    Start,
    Inited,
    Scaling,
    Running,
    Arrived,
    End,
}

impl NoteStatus {
    pub fn leq(a: NoteStatus, b: NoteStatus) -> bool {
        match (a, b) {
            (NoteStatus::Start, _) => true,
            (NoteStatus::Inited, NoteStatus::Start) => false,
            (NoteStatus::Inited, _) => true,
            (NoteStatus::Scaling, NoteStatus::Start) => false,
            (NoteStatus::Scaling, NoteStatus::Inited) => false,
            (NoteStatus::Scaling, _) => true,
            (NoteStatus::Running, NoteStatus::Start) => false,
            (NoteStatus::Running, NoteStatus::Inited) => false,
            (NoteStatus::Running, NoteStatus::Scaling) => false,
            (NoteStatus::Running, _) => true,
            (NoteStatus::Arrived, NoteStatus::Start) => false,
            (NoteStatus::Arrived, NoteStatus::Inited) => false,
            (NoteStatus::Arrived, NoteStatus::Scaling) => false,
            (NoteStatus::Arrived, NoteStatus::Running) => false,
            (NoteStatus::Arrived, _) => true,
            (NoteStatus::End, NoteStatus::End) => true,
            (NoteStatus::End, _) => false,
        }
    }
}

/// Combo display state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ComboState {
    #[default]
    None,
    FC,
    FCPlus,
    AP,
    APPlus,
}

/// Typed runtime note/event position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePos {
    Button(ButtonZone),
    Sensor(SensorArea),
}

impl Default for RuntimePos {
    fn default() -> Self {
        Self::Button(ButtonZone::K1)
    }
}

impl core::fmt::Display for JudgeGrade {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

impl RuntimePos {
    pub fn button_zone(&self) -> Option<ButtonZone> {
        match self {
            RuntimePos::Button(zone) => Some(*zone),
            RuntimePos::Sensor(_) => None,
        }
    }

    pub fn sensor_area(&self) -> Option<SensorArea> {
        match self {
            RuntimePos::Button(_) => None,
            RuntimePos::Sensor(area) => Some(*area),
        }
    }
}

/// Judge event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JudgeEventKind {
    #[default]
    Tap,
    Hold,
    Slide,
    Touch,
    Break,
}

/// A single note's judgment result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoteJudgeResult {
    pub grade: JudgeGrade,
    pub diff: Duration,
    pub is_break: bool,
    pub is_ex: bool,
}

impl NoteJudgeResult {
    pub fn is_fast(&self) -> bool {
        self.diff.to_micros() < 0
    }

    pub fn is_miss_or_too_fast(&self) -> bool {
        self.grade.is_miss_or_too_fast()
    }
}

/// Group state for multi-part notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupState {
    pub group_id: usize,
    pub count: usize,
    pub size: usize,
    pub grade: JudgeGrade,
    pub diff: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JudgePhase {
    #[default]
    Head,
    Tail,
}

/// Judge event emitted by Core, consumed by the host for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JudgeEvent {
    pub kind: JudgeEventKind,
    pub phase: JudgePhase,
    pub grade: JudgeGrade,
    pub diff: Duration,
    pub position: RuntimePos,
    pub note_index: usize,
    pub is_break: bool,
    pub is_ex: bool,
    pub multiple: usize,
}

/// Score accumulation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreState {
    pub combo: usize,
    pub p_combo: usize,
    pub c_p_combo: usize,
    pub total_base: usize,
    pub total_extra: usize,
    pub earned_base: usize,
    pub earned_extra: usize,
    pub earned_classic_extra: usize,
    pub lost_base: usize,
    pub lost_extra: usize,
    pub lost_classic_extra: usize,
    pub dx_score: i64,
    pub max_dx_score: usize,
    pub fast_count: usize,
    pub late_count: usize,
}

// Explicit zero fields avoid external Default calls in the extracted model.
#[allow(clippy::derivable_impls)]
impl Default for ScoreState {
    fn default() -> Self {
        ScoreState {
            combo: 0,
            p_combo: 0,
            c_p_combo: 0,
            total_base: 0,
            total_extra: 0,
            earned_base: 0,
            earned_extra: 0,
            earned_classic_extra: 0,
            lost_base: 0,
            lost_extra: 0,
            lost_classic_extra: 0,
            dx_score: 0,
            max_dx_score: 0,
            fast_count: 0,
            late_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dist_from_perfect() {
        assert!(matches!(JudgeGrade::Perfect.dist_from_perfect(), 0));
        assert!(matches!(JudgeGrade::Miss.dist_from_perfect(), 7));
    }

    #[test]
    fn test_base_score() {
        assert!(matches!(NoteType::Break.base_score(), 2500));
        assert!(matches!(NoteType::Break.extra_score(), 100));
    }
}
