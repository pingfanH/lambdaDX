use std::sync::{Mutex, OnceLock};

use lnmai_core::session::{self, Empty, FfiEnvelope, Session};
use lnmai_core::types::{ChartSpec, RuntimeStepLightResult, TimedInputBatch};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn ensure_runtime() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| unsafe { session::initialize_runtime().unwrap() });
}

fn decode_result<T: serde::de::DeserializeOwned>(env: &FfiEnvelope) -> T {
    env.decode_result().unwrap()
}

#[test]
fn lean_session_loads_lowers_and_steps_chart() {
    let _guard = test_guard();
    ensure_runtime();
    let chart_text = "&title=Smoke\n&artist=Test\n&first=0\n&lv_6=1\n&inote_6=(120){4}1,2,E\n";

    let empty = Session::<Empty>::create().unwrap();
    let (mut loaded, _load_info) = empty.load_chart_text(chart_text, 6).unwrap();

    let lowered_env = loaded.get_lowered_chart_json().unwrap();
    let lowered_chart: ChartSpec = decode_result(&lowered_env);
    assert_eq!(lowered_chart.taps.len(), 2);

    let state_env = loaded.get_state_json().unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_env.json).unwrap();
    assert_eq!(state["result"]["currentTime"], 0);

    let batch = TimedInputBatch {
        current_time: 0,
        events: vec![],
    };
    let step = loaded
        .advance_frame_light(&serde_json::to_string(&batch).unwrap())
        .unwrap();
    let result: RuntimeStepLightResult = decode_result(&step);
    assert_eq!(result.current_time, 0);

    let (_empty, _unload_info) = loaded.unload_chart().unwrap();
}
