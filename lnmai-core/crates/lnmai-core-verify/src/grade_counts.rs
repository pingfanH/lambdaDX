//! Fixed-array representation of Lean's `JudgeGrade → Nat` counters.
//! The element type stays generic so the runtime can retain unbounded counts.

use crate::types::JudgeGrade;

pub const NUM_GRADES: usize = 15;

pub const JUDGE_GRADES: [JudgeGrade; NUM_GRADES] = [
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

pub fn grade_index(grade: JudgeGrade) -> usize {
    match grade {
        JudgeGrade::Miss => 0,
        JudgeGrade::LateGood => 1,
        JudgeGrade::LateGreat3rd => 2,
        JudgeGrade::LateGreat2nd => 3,
        JudgeGrade::LateGreat => 4,
        JudgeGrade::LatePerfect3rd => 5,
        JudgeGrade::LatePerfect2nd => 6,
        JudgeGrade::Perfect => 7,
        JudgeGrade::FastPerfect2nd => 8,
        JudgeGrade::FastPerfect3rd => 9,
        JudgeGrade::FastGreat => 10,
        JudgeGrade::FastGreat2nd => 11,
        JudgeGrade::FastGreat3rd => 12,
        JudgeGrade::FastGood => 13,
        JudgeGrade::TooFast => 14,
    }
}

pub fn get_count<N>(counts: &[N; NUM_GRADES], grade: JudgeGrade) -> &N {
    &counts[grade_index(grade)]
}

pub fn set_count<N>(counts: &mut [N; NUM_GRADES], grade: JudgeGrade, value: N) {
    counts[grade_index(grade)] = value;
}
