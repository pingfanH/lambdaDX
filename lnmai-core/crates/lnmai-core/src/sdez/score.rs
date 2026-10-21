//! SDEZ170 score accumulation.

use super::judge::{convert_judge, get_judge_score, ETiming, JudgeBox, NoteScoreType};
use super::runtime::{SdezEvent, SdezNoteKind, SdezPhase};

pub const NOTE_TYPE_COUNT: usize = 5;
pub const TIMING_COUNT: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SdezScore {
    pub combo: u32,
    pub max_combo: u32,
    pub counts: [[u32; TIMING_COUNT]; NOTE_TYPE_COUNT],
    pub total_base: u32,
    pub earned_base: u32,
    pub total_break_bonus: u32,
    pub earned_break_bonus: u32,
    pub dx_score: u32,
}

impl SdezScore {
    pub fn fold(&mut self, event: &SdezEvent) {
        if event.kind == SdezNoteKind::Hold && event.phase == SdezPhase::Head {
            return;
        }
        let timing = event.judged.timing;
        let note_type = note_type_index(event.kind, event.is_break);
        self.counts[note_type][timing as usize] += 1;

        let score_type = score_type(event.kind, event.is_break);
        self.total_base += get_judge_score(ETiming::Critical, score_type);
        self.earned_base += get_judge_score(timing, score_type);
        if event.is_break {
            self.total_break_bonus +=
                get_judge_score(ETiming::Critical, NoteScoreType::BreakBonus);
            self.earned_break_bonus += get_judge_score(timing, NoteScoreType::BreakBonus);
        }

        if matches!(timing, ETiming::TooFast | ETiming::TooLate) {
            self.combo = 0;
        } else {
            self.combo += 1;
            self.max_combo = self.max_combo.max(self.combo);
        }

        self.dx_score += match convert_judge(timing) {
            JudgeBox::Critical => 3,
            JudgeBox::Perfect => 2,
            JudgeBox::Great => 1,
            _ => 0,
        };
    }

    pub fn total_score(&self) -> u32 {
        self.earned_base + self.earned_break_bonus
    }

    pub fn theory_score(&self) -> u32 {
        self.total_base + self.total_break_bonus
    }
}

fn note_type_index(kind: SdezNoteKind, is_break: bool) -> usize {
    if is_break {
        return 4;
    }
    match kind {
        SdezNoteKind::Tap => 0,
        SdezNoteKind::Hold => 1,
        SdezNoteKind::Slide => 2,
        SdezNoteKind::Touch => 3,
    }
}

fn score_type(kind: SdezNoteKind, is_break: bool) -> NoteScoreType {
    if is_break {
        return NoteScoreType::Break;
    }
    match kind {
        SdezNoteKind::Tap => NoteScoreType::Tap,
        SdezNoteKind::Hold => NoteScoreType::Hold,
        SdezNoteKind::Slide => NoteScoreType::Slide,
        SdezNoteKind::Touch => NoteScoreType::Touch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::note::JudgedNote;

    fn event(kind: SdezNoteKind, phase: SdezPhase, timing: ETiming, is_break: bool) -> SdezEvent {
        SdezEvent {
            note_index: 1,
            kind,
            phase,
            is_break,
            is_ex: false,
            button: None,
            sensor: None,
            is_wifi: false,
            judged: JudgedNote {
                timing,
                diff_msec: 0.0,
            },
        }
    }

    #[test]
    fn hold_head_is_not_scored() {
        let mut score = SdezScore::default();
        score.fold(&event(
            SdezNoteKind::Hold,
            SdezPhase::Head,
            ETiming::Critical,
            false,
        ));
        assert_eq!(score.combo, 0);
        assert_eq!(score.total_base, 0);
        score.fold(&event(
            SdezNoteKind::Hold,
            SdezPhase::Tail,
            ETiming::Critical,
            false,
        ));
        assert_eq!(score.combo, 1);
        assert_eq!(score.earned_base, 1000);
    }

    #[test]
    fn break_splits_base_and_bonus() {
        let mut score = SdezScore::default();
        score.fold(&event(
            SdezNoteKind::Tap,
            SdezPhase::Head,
            ETiming::Critical,
            true,
        ));
        assert_eq!(score.earned_base, 2500);
        assert_eq!(score.earned_break_bonus, 100);
        assert_eq!(score.theory_score(), 2600);
        assert_eq!(score.dx_score, 3);
    }

    #[test]
    fn miss_resets_combo() {
        let mut score = SdezScore::default();
        score.fold(&event(
            SdezNoteKind::Tap,
            SdezPhase::Head,
            ETiming::Critical,
            false,
        ));
        score.fold(&event(
            SdezNoteKind::Tap,
            SdezPhase::Head,
            ETiming::TooLate,
            false,
        ));
        assert_eq!(score.combo, 0);
        assert_eq!(score.max_combo, 1);
    }
}
