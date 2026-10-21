use lnmai_core::sdez::{
    get_judge_score, get_judge_timing, get_slide_judge_timing, judge_hold_total, EJudgeType,
    ETiming, NoteScoreType,
};
use serde_json::Value;

fn timing(name: &str) -> ETiming {
    match name {
        "TooFast" => ETiming::TooFast,
        "FastGood" => ETiming::FastGood,
        "FastGreat3rd" => ETiming::FastGreat3rd,
        "FastGreat2nd" => ETiming::FastGreat2nd,
        "FastGreat" => ETiming::FastGreat,
        "FastPerfect2nd" => ETiming::FastPerfect2nd,
        "FastPerfect" => ETiming::FastPerfect,
        "Critical" => ETiming::Critical,
        "LatePerfect" => ETiming::LatePerfect,
        "LatePerfect2nd" => ETiming::LatePerfect2nd,
        "LateGreat" => ETiming::LateGreat,
        "LateGreat2nd" => ETiming::LateGreat2nd,
        "LateGreat3rd" => ETiming::LateGreat3rd,
        "LateGood" => ETiming::LateGood,
        "TooLate" => ETiming::TooLate,
        other => panic!("unknown timing {other}"),
    }
}

fn name(timing: ETiming) -> &'static str {
    match timing {
        ETiming::TooFast => "TooFast",
        ETiming::FastGood => "FastGood",
        ETiming::FastGreat3rd => "FastGreat3rd",
        ETiming::FastGreat2nd => "FastGreat2nd",
        ETiming::FastGreat => "FastGreat",
        ETiming::FastPerfect2nd => "FastPerfect2nd",
        ETiming::FastPerfect => "FastPerfect",
        ETiming::Critical => "Critical",
        ETiming::LatePerfect => "LatePerfect",
        ETiming::LatePerfect2nd => "LatePerfect2nd",
        ETiming::LateGreat => "LateGreat",
        ETiming::LateGreat2nd => "LateGreat2nd",
        ETiming::LateGreat3rd => "LateGreat3rd",
        ETiming::LateGood => "LateGood",
        ETiming::TooLate => "TooLate",
        ETiming::End => "End",
    }
}

#[test]
fn sdez_oracle_vectors_match() {
    let data: Value =
        serde_json::from_str(include_str!("golden/sdez170_vectors.json")).expect("vectors");

    for case in data["judge"].as_array().unwrap() {
        let kind = match case["type"].as_u64().unwrap() {
            0 => EJudgeType::Tap,
            1 => EJudgeType::HoldOut,
            2 => EJudgeType::SlideOut,
            3 => EJudgeType::Touch,
            4 => EJudgeType::ExTap,
            _ => EJudgeType::Break,
        };
        let mut diff = case["diff"].as_f64().unwrap() as f32;
        let option = case["option"].as_f64().unwrap() as f32;
        let result = get_judge_timing(&mut diff, option, kind);
        assert_eq!(name(result), case["timing"].as_str().unwrap(), "{case}");
        let expected_after = case["after"].as_f64().unwrap() as f32;
        assert!(
            (diff - expected_after).abs() < 0.002,
            "diff mismatch for {case}: {diff} vs {expected_after}"
        );
    }

    for case in data["slide"].as_array().unwrap() {
        let mut diff = case["diff"].as_f64().unwrap() as f32;
        let wait = case["wait"].as_f64().unwrap() as f32;
        let result = get_slide_judge_timing(&mut diff, 0.0, EJudgeType::SlideOut, wait);
        assert_eq!(name(result), case["timing"].as_str().unwrap(), "{case}");
        let expected_after = case["after"].as_f64().unwrap() as f32;
        assert!(
            (diff - expected_after).abs() < 0.002,
            "slide diff mismatch for {case}: {diff} vs {expected_after}"
        );
    }

    for case in data["hold"].as_array().unwrap() {
        let result = judge_hold_total(
            case["holdTime"].as_f64().unwrap() as f32,
            case["release"].as_f64().unwrap() as f32,
            timing(case["timing"].as_str().unwrap()),
            case["bodyOn"].as_bool().unwrap(),
            case["touch"].as_bool().unwrap(),
        );
        assert_eq!(name(result), case["result"].as_str().unwrap(), "{case}");
    }

    let tables = data["scores"].as_array().unwrap();
    for (type_index, row) in tables.iter().enumerate() {
        let score_type = match type_index {
            0 => NoteScoreType::Tap,
            1 => NoteScoreType::Hold,
            2 => NoteScoreType::Slide,
            3 => NoteScoreType::Break,
            4 => NoteScoreType::Touch,
            _ => NoteScoreType::BreakBonus,
        };
        for (timing_index, expected) in row.as_array().unwrap().iter().enumerate() {
            let timing = match timing_index {
                0 => ETiming::TooFast,
                1 => ETiming::FastGood,
                2 => ETiming::FastGreat3rd,
                3 => ETiming::FastGreat2nd,
                4 => ETiming::FastGreat,
                5 => ETiming::FastPerfect2nd,
                6 => ETiming::FastPerfect,
                7 => ETiming::Critical,
                8 => ETiming::LatePerfect,
                9 => ETiming::LatePerfect2nd,
                10 => ETiming::LateGreat,
                11 => ETiming::LateGreat2nd,
                12 => ETiming::LateGreat3rd,
                13 => ETiming::LateGood,
                _ => ETiming::TooLate,
            };
            assert_eq!(
                get_judge_score(timing, score_type),
                expected.as_u64().unwrap() as u32
            );
        }
    }
}
