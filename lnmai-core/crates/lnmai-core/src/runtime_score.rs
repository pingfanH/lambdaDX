//! Runtime score state with per-note-type judge counts.
//!
//! Extends the verified numeric score fields with `NoteTypeJudgeCounts` (a
//! grade → count table per note type), matching `LnmaiCore/Types.lean`.
//! Counts are stored as fixed arrays indexed by the Lean `JudgeGrade` order.

use crate::integer::Integer;
use crate::natural::Natural;
use crate::score::{base_score, count_fast_late, score_break, score_non_break, update_combo};
use crate::types::{
    ComboState, JudgeDisplayOption, JudgeEvent, JudgeEventKind, JudgeGrade, NoteType,
};
use lnmai_core_verify::grade_counts::{get_count, set_count, NUM_GRADES};
pub use lnmai_core_verify::grade_counts::{grade_index, JUDGE_GRADES};
pub type JudgeCounts = [Natural; NUM_GRADES];

pub fn empty_judge_counts() -> JudgeCounts {
    std::array::from_fn(|_| Natural::default())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteTypeJudgeCounts {
    pub tap: [Natural; NUM_GRADES],
    pub hold: [Natural; NUM_GRADES],
    pub slide: [Natural; NUM_GRADES],
    pub touch: [Natural; NUM_GRADES],
    pub break_: [Natural; NUM_GRADES],
}

impl Default for NoteTypeJudgeCounts {
    fn default() -> Self {
        NoteTypeJudgeCounts {
            tap: std::array::from_fn(|_| Natural::default()),
            hold: std::array::from_fn(|_| Natural::default()),
            slide: std::array::from_fn(|_| Natural::default()),
            touch: std::array::from_fn(|_| Natural::default()),
            break_: std::array::from_fn(|_| Natural::default()),
        }
    }
}

impl NoteTypeJudgeCounts {
    pub fn grade_count(&self, grade: JudgeGrade) -> Natural {
        get_count(&self.tap, grade)
            + get_count(&self.hold, grade)
            + get_count(&self.slide, grade)
            + get_count(&self.touch, grade)
            + get_count(&self.break_, grade)
    }

    pub fn grade_count_where(&self, mut pred: impl FnMut(JudgeGrade) -> bool) -> Natural {
        JUDGE_GRADES
            .into_iter()
            .fold(Natural::default(), |acc, grade| {
                if pred(grade) {
                    acc + self.grade_count(grade)
                } else {
                    acc
                }
            })
    }

    pub fn break_grade_count_where(&self, mut pred: impl FnMut(JudgeGrade) -> bool) -> Natural {
        JUDGE_GRADES
            .into_iter()
            .fold(Natural::default(), |acc, grade| {
                if pred(grade) {
                    acc + get_count(&self.break_, grade)
                } else {
                    acc
                }
            })
    }
}

pub fn empty_note_type_judge_counts() -> NoteTypeJudgeCounts {
    NoteTypeJudgeCounts::default()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeScoreState {
    pub combo: Natural,
    pub p_combo: Natural,
    pub c_p_combo: Natural,
    pub total_base: Natural,
    pub total_extra: Natural,
    pub earned_base: Natural,
    pub earned_extra: Natural,
    pub earned_classic_extra: Natural,
    pub lost_base: Natural,
    pub lost_extra: Natural,
    pub lost_classic_extra: Natural,
    pub dx_score: Integer,
    pub max_dx_score: Natural,
    pub fast_count: Natural,
    pub late_count: Natural,
    pub counts: NoteTypeJudgeCounts,
}

/// `LnmaiCore.comboState`: derive the display from judgment counts, including
/// the separate requirement that AP+ has at least one critically judged Break.
pub fn combo_state(s: &RuntimeScoreState) -> ComboState {
    let noncritical_perfect = |grade| {
        matches!(
            grade,
            JudgeGrade::LatePerfect3rd
                | JudgeGrade::LatePerfect2nd
                | JudgeGrade::FastPerfect2nd
                | JudgeGrade::FastPerfect3rd
        )
    };
    let critical = s.counts.grade_count(JudgeGrade::Perfect);
    let perfect = s.counts.grade_count_where(noncritical_perfect);
    let great = s.counts.grade_count_where(|g| g.is_great_grade());
    let good = s.counts.grade_count_where(|g| g.is_good_grade());
    let miss = s.counts.grade_count_where(|g| g.is_miss_or_too_fast());
    let all_non_miss = critical + perfect + &great + &good;
    let is_full_combo = !all_non_miss.is_zero() && miss.is_zero();
    let is_full_combo_plus = is_full_combo && good.is_zero();
    let is_all_perfect = is_full_combo_plus && great.is_zero();
    let break_critical = get_count(&s.counts.break_, JudgeGrade::Perfect);
    let break_perfect = s.counts.break_grade_count_where(noncritical_perfect);
    let break_great = s.counts.break_grade_count_where(|g| g.is_great_grade());
    let break_good = s.counts.break_grade_count_where(|g| g.is_good_grade());
    let break_miss = s
        .counts
        .break_grade_count_where(|g| g.is_miss_or_too_fast());
    let break_all_non_miss = break_critical + &break_perfect + &break_great + &break_good;
    let break_is_all_perfect = !break_all_non_miss.is_zero()
        && break_miss.is_zero()
        && break_good.is_zero()
        && break_great.is_zero();
    let break_is_theoretical = break_is_all_perfect && break_perfect.is_zero();
    if !is_full_combo {
        ComboState::None
    } else if is_all_perfect {
        if break_is_theoretical {
            ComboState::APPlus
        } else {
            ComboState::AP
        }
    } else if is_full_combo_plus {
        ComboState::FCPlus
    } else {
        ComboState::FC
    }
}

fn event_score_note_type(evt: &JudgeEvent) -> NoteType {
    if evt.is_break || evt.kind == JudgeEventKind::Break {
        NoteType::Break
    } else {
        match evt.kind {
            JudgeEventKind::Tap => NoteType::Tap,
            JudgeEventKind::Hold => NoteType::Hold,
            JudgeEventKind::Slide => NoteType::Slide,
            JudgeEventKind::Touch => NoteType::Touch,
            JudgeEventKind::Break => NoteType::Break,
        }
    }
}

/// Returns `(earnedBase, earnedExtra, earnedClassicExtra, lostBase, lostExtra, lostClassicExtra)`.
fn event_score_deltas(evt: &JudgeEvent, multiple: &Natural) -> [Natural; 6] {
    match event_score_note_type(evt) {
        NoteType::Break => {
            let (a, b, c, d, e, f) = score_break(evt.grade, multiple);
            [a, b, c, d, e, f]
        }
        kind => {
            let (earned, lost) = score_non_break(base_score(kind), evt.grade, multiple);
            [
                earned,
                Natural::default(),
                Natural::default(),
                lost,
                Natural::default(),
                Natural::default(),
            ]
        }
    }
}

/// `foldEventIntoScore`.
pub fn fold_event_into_score(
    note_display: JudgeDisplayOption,
    break_display: JudgeDisplayOption,
    s: &RuntimeScoreState,
    evt: &JudgeEvent,
) -> RuntimeScoreState {
    if evt.kind == JudgeEventKind::Hold && evt.phase == crate::types::JudgePhase::Head {
        return s.clone();
    }
    let multiple = evt.multiple.clone().max(1.into());
    let combo_delta = update_combo(
        s.combo.clone(),
        s.p_combo.clone(),
        s.c_p_combo.clone(),
        s.dx_score.clone(),
        evt.grade,
        &multiple,
    );
    let [eb, ee, ece, lb, le, lce] = event_score_deltas(evt, &multiple);
    let display = if evt.is_break || evt.kind == JudgeEventKind::Break {
        break_display
    } else {
        note_display
    };
    let (is_fast, is_late) = count_fast_late(evt.grade, evt.diff.clone(), display);
    let mut counts = s.counts.clone();
    let bump = |table: &mut JudgeCounts| {
        let value = get_count(table, evt.grade) + &multiple;
        set_count(table, evt.grade, value);
    };
    if evt.is_break || evt.kind == JudgeEventKind::Break {
        bump(&mut counts.break_);
    } else {
        match evt.kind {
            JudgeEventKind::Tap => bump(&mut counts.tap),
            JudgeEventKind::Hold => bump(&mut counts.hold),
            JudgeEventKind::Slide => bump(&mut counts.slide),
            JudgeEventKind::Touch => bump(&mut counts.touch),
            JudgeEventKind::Break => bump(&mut counts.break_),
        }
    }
    RuntimeScoreState {
        combo: combo_delta.combo,
        p_combo: combo_delta.p_combo,
        c_p_combo: combo_delta.c_p_combo,
        earned_base: &s.earned_base + eb,
        earned_extra: &s.earned_extra + ee,
        earned_classic_extra: &s.earned_classic_extra + ece,
        lost_base: &s.lost_base + lb,
        lost_extra: &s.lost_extra + le,
        lost_classic_extra: &s.lost_classic_extra + lce,
        dx_score: combo_delta.dx_score_lost,
        fast_count: &s.fast_count
            + if is_fast {
                multiple.clone()
            } else {
                Natural::default()
            },
        late_count: &s.late_count
            + if is_late {
                multiple
            } else {
                Natural::default()
            },
        counts,
        ..s.clone()
    }
}

/// `foldEventsIntoScore`.
pub fn fold_events_into_score(
    note_display: JudgeDisplayOption,
    break_display: JudgeDisplayOption,
    s: &RuntimeScoreState,
    events: &[JudgeEvent],
) -> RuntimeScoreState {
    let mut acc = s.clone();
    for evt in events {
        acc = fold_event_into_score(note_display, break_display, &acc, evt);
    }
    acc
}
