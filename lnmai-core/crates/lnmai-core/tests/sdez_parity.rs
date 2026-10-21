use lnmai_core::chart_loader::ChartSpec;
use lnmai_core::default_tactic::autoplay_plan_from_chart;
use lnmai_core::scheduler;
use lnmai_core::sdez::{AutoplayMode, SdezRuntime};
use lnmai_core::time::TimePoint;

fn time_ms(time: TimePoint) -> f32 {
    time.micros.to_i64().unwrap_or(0) as f32 / 1000.0
}

fn duration_ms(duration: lnmai_core::time::Duration) -> f32 {
    duration.micros.to_i64().unwrap_or(0) as f32 / 1000.0
}

fn chart_end_ms(chart: &ChartSpec) -> f32 {
    let mut end = 0.0_f32;
    for tap in &chart.taps {
        end = end.max(time_ms(tap.timing.clone()));
    }
    for hold in &chart.holds {
        end = end.max(time_ms(hold.timing.clone()) + duration_ms(hold.length.clone()));
    }
    for hold in &chart.touch_holds {
        end = end.max(time_ms(hold.timing.clone()) + duration_ms(hold.length.clone()));
    }
    for touch in &chart.touches {
        end = end.max(time_ms(touch.timing.clone()));
    }
    for slide in &chart.slides {
        end = end.max(time_ms(slide.start_timing.clone()) + duration_ms(slide.length.clone()));
    }
    end + 5000.0
}

fn run_sdez(chart: &ChartSpec) -> (u32, u32, u32) {
    let mut runtime = SdezRuntime::from_chart(chart);
    runtime.autoplay = AutoplayMode::Critical;
    let mut t = 0.0_f32;
    let end = chart_end_ms(chart);
    while t <= end {
        let _ = runtime.step(t);
        t += 16.666666;
    }
    let score = &runtime.score;
    (
        score.theory_score(),
        score.total_score(),
        score.combo,
    )
}

fn run_legacy(chart: &ChartSpec) -> (u32, u32, u32) {
    let mut state = lnmai_core::chart_loader::build_game_state(chart);
    let tactic = autoplay_plan_from_chart(chart).events();
    let mut cursor = 0usize;
    let end = chart_end_ms(chart) as i64 * 1000;
    let mut t = 0i64;
    while t <= end {
        let mut events = Vec::new();
        while cursor < tactic.len() && tactic[cursor].at().micros.to_i64().unwrap_or(0) <= t {
            events.push(tactic[cursor].clone());
            cursor += 1;
        }
        let batch = lnmai_core::input_model::TimedInputBatch {
            current_time: TimePoint::from_micros(t),
            events,
        };
        let (next, _, _, _) = scheduler::step_frame_timed(&state, &batch);
        state = next;
        t += 16_667;
    }
    (
        (state.score.total_base + state.score.total_extra).to_word() as u32,
        (state.score.earned_base + state.score.earned_extra).to_word() as u32,
        state.score.combo.to_word() as u32,
    )
}

fn check(name: &str, text: &str, level: usize) {
    let chart = lnmai_core::simai::frontend::frontend_lowered_chart(text, level.into())
        .expect("chart parses");
    let sdez = run_sdez(&chart);
    let legacy = run_legacy(&chart);
    // The legacy scheduler scores SlideHead objects as taps. SDEZ170 scores
    // the slide as a whole and does not emit a separate star-head score.
    let head_score: u32 = chart
        .slide_heads
        .iter()
        .map(|head| if head.is_break { 2600 } else { 500 })
        .sum();
    let expected = (
        sdez.0 + head_score,
        sdez.1 + head_score,
        sdez.2 + chart.slide_heads.len() as u32,
    );
    assert_eq!(
        legacy, expected,
        "{name}: SDEZ {sdez:?}, slides heads {head_score}, legacy {legacy:?}"
    );
}

#[test]
fn real_charts_match_legacy_autoplay_scores() {
    check(
        "jack_ripper",
        include_str!("fixtures/real_charts/jack_ripper.txt"),
        5,
    );
    check(
        "pandora_paraboxxx",
        include_str!("fixtures/real_charts/pandora_paraboxxx.txt"),
        6,
    );
    check(
        "credit",
        include_str!("fixtures/real_charts/credit.txt"),
        5,
    );
}
