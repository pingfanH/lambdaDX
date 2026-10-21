//! Aeneas-friendly convert module
//!
//! Grade conversion functions

use super::types::{JudgeGrade, JudgeStyle};

/// Convert grade using Maji mode
pub fn convert_maji(grade: JudgeGrade) -> JudgeGrade {
    match grade {
        JudgeGrade::LateGreat => JudgeGrade::LateGood,
        JudgeGrade::LateGreat2nd => JudgeGrade::LateGood,
        JudgeGrade::LateGreat3rd => JudgeGrade::LateGood,
        JudgeGrade::FastGreat => JudgeGrade::FastGood,
        JudgeGrade::FastGreat2nd => JudgeGrade::FastGood,
        JudgeGrade::FastGreat3rd => JudgeGrade::FastGood,
        JudgeGrade::LatePerfect3rd => JudgeGrade::LateGreat,
        JudgeGrade::FastPerfect3rd => JudgeGrade::FastGreat,
        JudgeGrade::LatePerfect2nd => JudgeGrade::LatePerfect2nd,
        JudgeGrade::FastPerfect2nd => JudgeGrade::FastPerfect2nd,
        JudgeGrade::LateGood => JudgeGrade::Miss,
        JudgeGrade::FastGood => JudgeGrade::TooFast,
        JudgeGrade::Perfect => JudgeGrade::Perfect,
        JudgeGrade::Miss => JudgeGrade::Miss,
        JudgeGrade::TooFast => JudgeGrade::TooFast,
    }
}

/// Convert grade using Gachi mode
pub fn convert_gachi(grade: JudgeGrade) -> JudgeGrade {
    match grade {
        JudgeGrade::LatePerfect3rd => JudgeGrade::LateGood,
        JudgeGrade::FastPerfect3rd => JudgeGrade::FastGood,
        JudgeGrade::LatePerfect2nd => JudgeGrade::LateGreat,
        JudgeGrade::FastPerfect2nd => JudgeGrade::FastGreat,
        JudgeGrade::LateGreat => JudgeGrade::Miss,
        JudgeGrade::LateGreat2nd => JudgeGrade::Miss,
        JudgeGrade::LateGreat3rd => JudgeGrade::Miss,
        JudgeGrade::LateGood => JudgeGrade::Miss,
        JudgeGrade::FastGreat => JudgeGrade::TooFast,
        JudgeGrade::FastGreat2nd => JudgeGrade::TooFast,
        JudgeGrade::FastGreat3rd => JudgeGrade::TooFast,
        JudgeGrade::FastGood => JudgeGrade::TooFast,
        JudgeGrade::Perfect => JudgeGrade::Perfect,
        JudgeGrade::Miss => JudgeGrade::Miss,
        JudgeGrade::TooFast => JudgeGrade::TooFast,
    }
}

/// Convert grade using Gori mode
pub fn convert_gori(grade: JudgeGrade) -> JudgeGrade {
    match grade {
        JudgeGrade::Perfect => JudgeGrade::Perfect,
        JudgeGrade::Miss => JudgeGrade::Miss,
        JudgeGrade::LateGood => JudgeGrade::Miss,
        JudgeGrade::LateGreat3rd => JudgeGrade::Miss,
        JudgeGrade::LateGreat2nd => JudgeGrade::Miss,
        JudgeGrade::LateGreat => JudgeGrade::Miss,
        JudgeGrade::LatePerfect3rd => JudgeGrade::Miss,
        JudgeGrade::LatePerfect2nd => JudgeGrade::Miss,
        JudgeGrade::FastPerfect2nd => JudgeGrade::TooFast,
        JudgeGrade::FastPerfect3rd => JudgeGrade::TooFast,
        JudgeGrade::FastGreat => JudgeGrade::TooFast,
        JudgeGrade::FastGreat2nd => JudgeGrade::TooFast,
        JudgeGrade::FastGreat3rd => JudgeGrade::TooFast,
        JudgeGrade::FastGood => JudgeGrade::TooFast,
        JudgeGrade::TooFast => JudgeGrade::TooFast,
    }
}

/// Convert grade based on judge style
pub fn convert_grade(style: JudgeStyle, grade: JudgeGrade) -> JudgeGrade {
    match style {
        JudgeStyle::Default => grade,
        JudgeStyle::Maji => convert_maji(grade),
        JudgeStyle::Gachi => convert_gachi(grade),
        JudgeStyle::Gori => convert_gori(grade),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_fixed() {
        for style in &[
            JudgeStyle::Default,
            JudgeStyle::Maji,
            JudgeStyle::Gachi,
            JudgeStyle::Gori,
        ] {
            assert_eq!(
                convert_grade(*style, JudgeGrade::Perfect),
                JudgeGrade::Perfect
            );
        }
    }

    #[test]
    fn test_miss_fixed() {
        for style in &[
            JudgeStyle::Default,
            JudgeStyle::Maji,
            JudgeStyle::Gachi,
            JudgeStyle::Gori,
        ] {
            assert_eq!(convert_grade(*style, JudgeGrade::Miss), JudgeGrade::Miss);
        }
    }

    #[test]
    fn test_default_identity() {
        let grades = [
            JudgeGrade::Miss,
            JudgeGrade::LateGood,
            JudgeGrade::LateGreat,
            JudgeGrade::Perfect,
            JudgeGrade::TooFast,
        ];
        for grade in grades {
            assert_eq!(convert_grade(JudgeStyle::Default, grade), grade);
        }
    }
}
