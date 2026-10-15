use std::time::{Duration, Instant};

use lnmai_core::session::{Empty, Session};

#[test]
#[ignore = "interactive seek timing budget; run without concurrent tests"]
fn dense_chart_seek_does_not_replay_the_entire_history() {
    let source = format!(
        "&inote_6=(120){{4}}{}E\n",
        "1/2h[4:1]/B3/4-8[4:1],".repeat(256)
    );
    let mut session = Session::<Empty>::create_native()
        .unwrap()
        .load_chart_text_native(&source, 6)
        .unwrap();
    let events = session.autoplay_plan().unwrap().events();
    let handle = session.handle();
    let mut worst = Duration::ZERO;
    for seconds in [0, 10, 30, 60, 120, 60, 30, 10, 0] {
        let start = Instant::now();
        let result = session
            .seek_with_events(seconds * 1_000_000, &events, true)
            .unwrap();
        let elapsed = start.elapsed();
        eprintln!(
            "seek {seconds:3}s: {:.3} ms",
            elapsed.as_secs_f64() * 1_000.0
        );
        worst = worst.max(elapsed);
        assert_eq!(session.handle(), handle);
        assert_eq!(result.score.earned_base, 0);
        assert_eq!(result.score.lost_base, 0);
    }
    // Leave headroom for slower CI hosts, but catch the seconds-long history replay.
    assert!(worst < Duration::from_millis(100), "seek took {worst:?}");
}
