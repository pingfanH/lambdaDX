//! Differential tests: Rust output vs the authoritative Lean implementation.
//!
//! These tests are explicitly ignored by the ordinary Rust test suite. Run
//! tools/run_differential.sh to require both Lean oracle binaries and execute them.
//!
//! - `LNMAI_LEAN_PARSER_CLI`: path to the Lean `simai-parser-cli` executable
//!   (`lake build simai-parser-cli` in `lnmai-core`).
//!
//! When set, each chart is parsed by the Lean CLI (newline-delimited JSON over
//! stdin) and by this Rust port, and the two `lowered` payloads must be equal.
//! Explicitly running these tests without the oracle is an error.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::Value;

fn assert_json_equal(rust: &Value, lean: &Value, path: &str) {
    match (rust, lean) {
        (Value::Object(r), Value::Object(l)) => {
            assert_eq!(
                r.keys().collect::<Vec<_>>(),
                l.keys().collect::<Vec<_>>(),
                "{path}: object keys"
            );
            for (key, value) in r {
                assert_json_equal(value, &l[key], &format!("{path}.{key}"));
            }
        }
        (Value::Array(r), Value::Array(l)) => {
            assert_eq!(
                r.len(),
                l.len(),
                "{path}: array length; Rust={r:?}, Lean={l:?}"
            );
            for (i, (a, b)) in r.iter().zip(l).enumerate() {
                assert_json_equal(a, b, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(rust, lean, "{path}"),
    }
}

const CHARTS: &[(&str, u32)] = &[
    ("&first=0\n&inote_1=\n(120)\n1,2,3,\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1h[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1-3[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1v3[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1w5[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1/2/3/4,\n", 1),
    ("&first=0\n&inote_1=\n(120)\n(240)1,2,\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1>2,3<7,\n", 1),
    // touch / touch-hold / grouped touches
    ("&first=0\n&inote_1=\n(120)\nA1,A2,A3,\n", 1),
    ("&first=0\n&inote_1=\n(120)\nA1h[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\nA1/A2/A1,\n", 1),
    ("&first=0\n&inote_1=\n(120)\nA1h[4:1]/A2h[4:1],\n", 1),
    // holds and multi-note segments
    ("&first=0\n&inote_1=\n(120)\n1h[2:1],2h[2:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1/2,3/4,\n", 1),
    // slides incl. chains and headless
    ("&first=0\n&inote_1=\n(120)\n1-3[4:1],5-7[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1-3[4:1]/5-7[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1-3[4:1]!,5-7[4:1],\n", 1),
    ("&first=0\n&inote_1=\n(120)\n1-3[8:1][8:1],\n", 1),
    // multi-measure bodies (regression: body lines used to be reversed) and
    // inline `{n}` subdivision changes across a line break.
    ("&first=0\n&inote_1=\n(120)\n1,\n2,\n3,\n4,\n", 1),
    (
        "&first=0\n&inote_1=\n(210){4},{8},,,,,,,,6,\n5,4,3,4,5,\n",
        1,
    ),
    (
        "&first=0\n&inote_1=\n(120)\n1,2,3,4,\n5,6,7,8,\n1h[4:1],\n2-4[2:1],\n",
        1,
    ),
    // second difficulty block (multi-level selection).
    (
        "&first=0\n&inote_1=\n(120)\n1,\n&inote_2=\n(120)\n2,\n3,\n",
        2,
    ),
];

/// `(name, chart, frame batches)` runtime scenarios.
fn runtime_scenarios() -> Vec<(&'static str, &'static str, Vec<Value>)> {
    let taps = "&first=0\n&inote_1=\n(120)\n1,2,3,\n";
    let touches = "&first=0\n&inote_1=\n(120)\nA1,A2,A3,\n";
    let touch_hold = "&first=0\n&inote_1=\n(120)\nA1h[4:1],\n";
    let holds = "&first=0\n&inote_1=\n(120)\n1h[2:1],\n";
    let slide = "&first=0\n&inote_1=\n(120)\n1-3[4:1],\n";
    let slide_sensor = "&first=0\n&inote_1=\n(120)\n1-3[4:1],\n";

    let c = |tp: i64, zone: &str| serde_json::json!({"buttonClick": {"tp": tp, "zone": zone}});
    let sc = |tp: i64, area: &str| serde_json::json!({"sensorClick": {"tp": tp, "area": area}});
    let sh = |tp: i64, area: &str, down: bool| serde_json::json!({"sensorHold": {"tp": tp, "area": area, "isDown": down}});
    let bh = |tp: i64, zone: &str, down: bool| serde_json::json!({"buttonHold": {"tp": tp, "zone": zone, "isDown": down}});
    let b = |tp: i64, evs: Vec<Value>| serde_json::json!({ "currentTime": tp, "events": evs });

    vec![
        (
            "taps",
            taps,
            vec![
                b(0, vec![c(0, "K1")]),
                b(250000, vec![]),
                b(500000, vec![c(500000, "K2")]),
                b(750000, vec![]),
                b(1000000, vec![c(1000000, "K3")]),
                b(1600000, vec![]),
            ],
        ),
        (
            "touch",
            touches,
            vec![
                b(0, vec![sc(0, "A1")]),
                b(250000, vec![]),
                b(500000, vec![sc(500000, "A2")]),
                b(750000, vec![]),
                b(1000000, vec![sc(1000000, "A3")]),
                b(1600000, vec![]),
            ],
        ),
        (
            "touch-hold",
            touch_hold,
            vec![
                b(0, vec![sc(0, "A1")]),
                b(100000, vec![sh(100000, "A1", true)]),
                b(300000, vec![sh(300000, "A1", true)]),
                b(600000, vec![sh(600000, "A1", true)]),
                b(900000, vec![sh(900000, "A1", false)]),
                b(1300000, vec![]),
            ],
        ),
        (
            "hold-release",
            holds,
            vec![
                b(0, vec![c(0, "K1")]),
                b(100000, vec![bh(100000, "K1", true)]),
                b(300000, vec![bh(300000, "K1", true)]),
                b(700000, vec![bh(700000, "K1", false)]),
                b(1200000, vec![]),
                b(2000000, vec![]),
            ],
        ),
        (
            "slide-noinput",
            slide,
            vec![
                b(0, vec![]),
                b(500000, vec![]),
                b(1000000, vec![]),
                b(1500000, vec![]),
                b(2500000, vec![]),
            ],
        ),
        (
            "slide-sensor",
            slide_sensor,
            vec![
                b(0, vec![sh(0, "A1", true)]),
                b(200000, vec![sh(200000, "A3", true)]),
                b(400000, vec![]),
                b(
                    900000,
                    vec![sh(900000, "A1", false), sh(900000, "A3", false)],
                ),
                b(1400000, vec![]),
                b(2200000, vec![]),
            ],
        ),
    ]
}

fn lean_cli() -> Option<String> {
    std::env::var("LNMAI_LEAN_PARSER_CLI")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Ask the Lean parser CLI for the lowered chart of `(content, level)`.
fn lean_lowered(cli: &str, content: &str, level: u32) -> Option<Value> {
    let mut child = Command::new(cli)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let request = serde_json::json!({
        "mode": "lowered",
        "levelIndex": level,
        "content": content,
    });
    {
        let stdin = child.stdin.as_mut()?;
        writeln!(stdin, "{}", request).ok()?;
        stdin.flush().ok()?;
    }
    // The parser serves requests until EOF. Close input before waiting for it.
    drop(child.stdin.take());
    let stdout = child.stdout.take()?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    assert!(child.wait().expect("wait for Lean parser").success());
    serde_json::from_str(line.trim()).ok()
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn lowered_chart_matches_lean() {
    let cli = lean_cli().expect("set LNMAI_LEAN_PARSER_CLI to run differential tests");

    for (content, level) in CHARTS {
        let rust_raw = lnmai_core::ffi::parse_lowered_chart_json(content, *level);
        let rust: Value = serde_json::from_str(&rust_raw).unwrap();
        let lean = lean_lowered(&cli, content, *level)
            .expect("configured Lean parser must respond with JSON");
        assert_json_equal(&rust, &lean, &format!("chart {content:?}"));
    }
}

// ---------------------------------------------------------------------------
// Runtime stepping differential test
// ---------------------------------------------------------------------------

struct LeanRuntime {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl LeanRuntime {
    fn spawn(cli: &str) -> Option<LeanRuntime> {
        let mut child = Command::new(cli)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        Some(LeanRuntime {
            child,
            stdin,
            reader: BufReader::new(stdout),
        })
    }

    fn request(&mut self, req: &Value) -> Value {
        writeln!(self.stdin, "{}", req).unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    }

    fn load(&mut self, content: &str, level: u32) -> Value {
        self.request(&serde_json::json!({ "op": "load", "content": content, "levelIndex": level }))
    }

    fn step(&mut self, batch: &Value) -> Value {
        self.request(&serde_json::json!({ "op": "step", "batch": batch }))
    }
}

impl Drop for LeanRuntime {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn lean_runtime_cli() -> Option<String> {
    std::env::var("LNMAI_LEAN_RUNTIME_CLI")
        .ok()
        .filter(|s| !s.is_empty())
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn runtime_step_matches_lean() {
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).expect("configured Lean runtime must start");

    for (name, content, batches) in runtime_scenarios() {
        compare_runtime(&mut lean, name, content, &batches);
    }
}

/// Compare a full stepping session (load + N frames) between Rust and Lean.
fn compare_runtime(lean: &mut LeanRuntime, name: &str, content: &str, batches: &[Value]) {
    let loaded = lean.load(content, 1);
    assert_eq!(
        loaded["ok"], true,
        "scenario {:?}: Lean load failed: {}",
        name, loaded
    );

    let created: Value =
        serde_json::from_str(&lnmai_core::ffi::create_empty_session_handle()).unwrap();
    let handle = created["result"]["handle"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let rust_loaded: Value = serde_json::from_str(
        &lnmai_core::ffi::load_chart_into_session_from_text(handle, content, 1),
    )
    .unwrap();
    assert_eq!(
        rust_loaded["ok"], true,
        "scenario {:?}: Rust load failed",
        name
    );

    let lean_state = lean.request(&serde_json::json!({"op": "state"}));
    let rust_state: Value =
        serde_json::from_str(&lnmai_core::ffi::get_game_state_json_by_handle(handle)).unwrap();
    assert_json_equal(&rust_state, &lean_state, &format!("{name} initial state"));

    for (i, batch) in batches.iter().enumerate() {
        let lean_resp = lean.step(batch);
        let rust_resp: Value = serde_json::from_str(
            &lnmai_core::ffi::step_game_state_handle_light(handle, &batch.to_string()),
        )
        .unwrap();
        assert_eq!(lean_resp["ok"], true, "Lean step failed: {lean_resp}");
        assert_eq!(rust_resp["ok"], true, "Rust step failed: {rust_resp}");
        assert_json_equal(
            &rust_resp["result"],
            &lean_resp["result"],
            &format!("{name} frame {i}"),
        );
        let lean_state = lean.request(&serde_json::json!({ "op": "state" }));
        let rust_state: Value =
            serde_json::from_str(&lnmai_core::ffi::get_game_state_json_by_handle(handle)).unwrap();
        assert_eq!(
            lean_state["ok"], true,
            "Lean state query failed: {lean_state}"
        );
        assert_json_equal(
            &rust_state["result"],
            &lean_state["result"],
            &format!("{name} frame {i} state"),
        );
    }

    let _ = lnmai_core::ffi::free_game_state_handle(handle);
}

/// Tiny deterministic PRNG so the input sequence is identical run to run.
struct Lcg(u64);
impl Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn pick<'a>(&mut self, xs: &'a [&'a str]) -> &'a str {
        xs[(self.next_u32() as usize) % xs.len()]
    }
}

fn random_batches(frames: usize, seed: u64) -> Vec<Value> {
    let zones = ["K1", "K2", "K3", "K4", "K5", "K6", "K7", "K8"];
    let sensors = [
        "A1", "A2", "A3", "A4", "A5", "A6", "A7", "A8", "B1", "B2", "A1",
    ];
    let mut rng = Lcg(seed);
    let mut batches = Vec::new();
    let delta: i64 = 50000;
    for frame in 0..frames {
        let tp = frame as i64 * delta;
        let mut events: Vec<Value> = Vec::new();
        let roll = rng.next_u32() % 100;
        if roll < 25 {
            events.push(serde_json::json!({"buttonClick": {"tp": tp, "zone": rng.pick(&zones)}}));
        } else if roll < 45 {
            events.push(serde_json::json!({"sensorClick": {"tp": tp, "area": rng.pick(&sensors)}}));
        } else if roll < 65 {
            let down = rng.next_u32().is_multiple_of(2);
            events.push(serde_json::json!({"sensorHold": {"tp": tp, "area": rng.pick(&sensors), "isDown": down}}));
        } else if roll < 80 {
            let down = rng.next_u32().is_multiple_of(2);
            events.push(serde_json::json!({"buttonHold": {"tp": tp, "zone": rng.pick(&zones), "isDown": down}}));
        }
        batches.push(serde_json::json!({ "currentTime": tp, "events": events }));
    }
    batches
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn runtime_random_matches_lean() {
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).expect("configured Lean runtime must start");

    // A chart mixing every note family, spread over several seconds.
    let content =
        "&first=0\n&inote_1=\n(120)\n1,2h[4:1],A1,A2h[4:1],1-3[4:1],5-7[4:1],3,4,5,1,2,3,\n";

    for seed in [1u64, 7, 42] {
        let batches = random_batches(80, seed);
        compare_runtime(&mut lean, &format!("random-{}", seed), content, &batches);
    }
}

fn expanded_charts() -> Vec<String> {
    let mut charts: Vec<_> = CHARTS
        .iter()
        .filter(|(_, level)| *level == 1)
        .map(|(s, _)| s.to_string())
        .collect();
    charts.extend([
        " \t&title=Indented\n &wholebpm=173\n\r&first=-0.25\n &inote_1=1,\n\r&\r\n2,\n".to_string(),
        "&inote_+1=1,\n&inote_1_0=2,\n&inote_ 1=3,\n&inote_1=4,\n".to_string(),
    ]);
    for start in 1..=8 {
        for shape in ["-", ">", "<", "^", "v", "p", "q", "pp", "qq", "s", "z", "w"] {
            for end in 1..=8 {
                charts.push(format!(
                    "&first=0\n&inote_1=(137.5){{7}}{start}{shape}{end}[7:3],\n"
                ));
            }
        }
    }
    for body in [
        "1-3-5[4:1]",
        "1-3[8:1]-5[8:1]",
        "1-3[4:1]*-5[8:1]",
        "1V35[4:1]",
        "1V75[4:1]",
        "1w5-7[4:1]",
        "1-3[4:1]-5",
        "1b/2x/3bx/4h[4:1]b",
        "A1/A2/B1/C",
        "A1h[4:1]/B1h[4:1]/Ch[4:1]",
        "12,34,56,78",
        "1\u{0060}2\u{0060}3",
        "1\u{0060}\u{0060}2",
        "1-5[120#4:1]",
        "1-5[0.25##0.75]",
        "1-5[0.25##120#4:1]",
        "1-5[##0.75]",
        "1-5[120#4:1][8:1]",
        "1h[4:1][8:1]",
        "1h[4:1][bad]",
        "1-3[4:1]*-5[4:1]*-7[4:1]",
        "1(180)2",
        "1{8}2",
        "1-3[4:1]@",
        "1-3[4:1]!",
        "1-3[4:1]?",
        "1$$",
        "A1f",
        "A1x",
        "K1",
        "9",
        "１",
        "1h[+4:1]",
        "1h[4:--1]",
        "1h[4:1",
        "1h[[4:1]]",
        "E",
        "1,\n2,\r\n3",
        "1||hello\n2",
    ] {
        charts.push(format!(
            "&title=test\n&first=-0.0000005\n&inote_1=(137){{7}}{body},\n"
        ));
    }
    charts
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn frontend_modes_shapes_and_errors_match_lean() {
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    for content in expanded_charts() {
        for is_classic in [false, true] {
            let rust: Value = serde_json::from_str(
                &lnmai_core::ffi::parse_frontend_chart_mode_json(&content, 1, is_classic),
            )
            .unwrap();
            let expected = lean.request(&serde_json::json!({"op":"parse", "mode":"frontend", "content":content, "isClassic":is_classic}));
            assert_json_equal(
                &rust,
                &expected,
                &format!("frontend classic={is_classic} {content:?}"),
            );
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn mutated_simai_syntax_matches_lean() {
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let seeds = [
        "(137.5){7}1bx/2h[4:1]/A1f,3-5[4:1],",
        "(180){8}1-3[8:1]-5[8:1],A1h[4:1]/B2h[4:1],",
        "(120)1-3[4:1]*-5[8:1],1V35[4:1],1w5[2:1],",
        "(120)1-5[0.25##120#4:1],12,34,1`2`3,",
        "(120)1-3[4:1]!,1-3[4:1]@,1-3[4:1]?,E,",
        "(120)A1/A2/B1/C,1||comment\n2,\r\n3,",
    ];
    let alphabet: Vec<_> = "01289ABCEhbxpfvVw-<>^szq*[]{}():,#/!?@` &\n\r\t你好１"
        .chars()
        .collect();
    let mut random = 0x3e8b_d174_56ca_20f9u64;
    for case in 0..16384 {
        let mut body: Vec<_> = seeds[case % seeds.len()].chars().collect();
        for _ in 0..1 + case % 4 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let at = (random >> 32) as usize % (body.len() + 1);
            let c = alphabet[(random >> 16) as usize % alphabet.len()];
            match random % 3 {
                0 => body.insert(at, c),
                1 if at < body.len() => body[at] = c,
                _ if at < body.len() => {
                    body.remove(at);
                }
                _ => {}
            }
        }
        let body: String = body.into_iter().collect();
        let content = format!("&title=syntax mutation\n&first=-0.0000005\n&inote_1={body}\n");
        for classic in [false, true] {
            let rust = decode_response(lnmai_core::ffi::parse_frontend_chart_mode_json(
                &content, 1, classic,
            ));
            let expected = lean.request(&serde_json::json!({
                "op":"parse","mode":"frontend","content":content,
                "isClassic":classic
            }));
            assert_json_equal(
                &rust,
                &expected,
                &format!("Simai mutation {case} classic={classic}: {body:?}"),
            );
        }
    }
}

fn check_direct_step(
    lean: &mut LeanRuntime,
    state: &mut lnmai_core::input_model::GameState,
    batch: Value,
    name: &str,
) {
    check_step(lean, state, batch, name, true);
}

fn check_step(
    lean: &mut LeanRuntime,
    state: &mut lnmai_core::input_model::GameState,
    batch: Value,
    name: &str,
    full_state: bool,
) {
    use lnmai_core::ffi;
    let expected = lean.step(&batch);
    assert_eq!(expected["ok"], true, "{expected}");
    let (next, events, audio, render) = lnmai_core::scheduler::step_frame_timed(
        state,
        &ffi::parse_timed_input_batch(&batch).unwrap(),
    );
    let result = ffi::step_result_json(
        &events,
        &audio,
        &render,
        &next.score,
        next.current_time.clone(),
        lnmai_core::scheduler::is_terminated(&next),
    );
    assert_json_equal(&result, &expected["result"], name);
    if full_state {
        let expected_state = lean.request(&serde_json::json!({"op":"state"}));
        assert_json_equal(
            &ffi::game_state_json(&next),
            &expected_state["result"],
            &format!("{name} state"),
        );
    }
    *state = next;
}

fn read_corpus(names: &[&str]) -> Vec<(String, String, u32)> {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/assets");
    let mut cases = Vec::new();
    for name in names {
        let content = std::fs::read_to_string(assets.join(name).join("maidata.txt")).unwrap();
        let levels: Vec<u32> = content
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("&inote_")?
                    .split_once('=')?
                    .0
                    .parse()
                    .ok()
            })
            .collect();
        assert!(!levels.is_empty(), "{name}: missing difficulty blocks");
        for level in levels {
            cases.push((format!("{name}/{level}"), content.clone(), level));
        }
    }
    cases
}

fn real_corpus() -> Vec<(String, String, u32)> {
    let cases = read_corpus(&[
        "24_Sun Dance",
        "230_CYCLES",
        "462_7thSense",
        "589_Panopticon",
        "834_PANDORA PARADOXXX",
        "11264_幽霊東京",
        "11358_インドア系ならトラックメイカー",
        "11475_SUPER AMBULANCE",
        "100230_[宴]CYCLES",
        "100524_[協]Hand in Hand",
        "小石DISCO",
        "011996 Usagi Flap",
        "011951 サイエンス",
        "蛙石",
    ]);
    assert_eq!(
        cases.len(),
        48,
        "the checked-in corpus must not silently shrink"
    );
    cases
}

fn requested_corpus() -> Vec<(String, String, u32)> {
    let cases = read_corpus(&["011996 Usagi Flap", "011951 サイエンス", "蛙石"]);
    let mut actual: Vec<_> = cases.iter().map(|(name, _, _)| name.as_str()).collect();
    actual.sort_unstable();
    let mut expected = [
        "011996 Usagi Flap/2",
        "011996 Usagi Flap/3",
        "011996 Usagi Flap/4",
        "011996 Usagi Flap/5",
        "011951 サイエンス/2",
        "011951 サイエンス/3",
        "011951 サイエンス/4",
        "011951 サイエンス/5",
        "蛙石/5",
        "蛙石/6",
    ];
    expected.sort_unstable();
    assert_eq!(actual, expected, "requested song difficulties changed");
    cases
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn real_corpus_frontends_match_lean() {
    check_corpus_frontends(real_corpus());
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh --release requested_corpus_"]
fn requested_corpus_frontends_match_lean() {
    check_corpus_frontends(requested_corpus());
}

fn check_corpus_frontends(cases: Vec<(String, String, u32)>) {
    use lnmai_core::ffi;
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    for (name, content, level) in cases {
        for classic in [false, true] {
            let rust = decode_response(ffi::parse_frontend_chart_mode_json(
                &content, level, classic,
            ));
            let expected = lean.request(&serde_json::json!({"op":"parse", "mode":"frontend", "content":content, "levelIndex":level, "isClassic":classic}));
            assert_json_equal(
                &rust,
                &expected,
                &format!("{name} classic={classic} frontend"),
            );
        }
        for mode in ["inspection", "semantic", "normalized"] {
            let text = match mode {
                "inspection" => ffi::parse_frontend_inspection_chart_json(&content, level),
                "semantic" => ffi::parse_frontend_semantic_chart_json(&content, level),
                _ => ffi::parse_normalized_chart_json(&content, level),
            };
            let expected = lean.request(&serde_json::json!({"op":"parse", "mode":mode, "content":content, "levelIndex":level}));
            assert_json_equal(&decode_response(text), &expected, &format!("{name} {mode}"));
        }
        eprintln!("corpus frontend passed: {name}");
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn real_corpus_playthroughs_match_lean() {
    check_corpus_playthroughs(real_corpus());
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh --release requested_corpus_"]
fn requested_corpus_playthroughs_match_lean() {
    check_corpus_playthroughs(requested_corpus());
}

fn check_corpus_playthroughs(cases: Vec<(String, String, u32)>) {
    let cases = match std::env::var("LNMAI_CORPUS_FILTER") {
        Ok(filter) if !filter.is_empty() => cases
            .into_iter()
            .filter(|(name, _, _)| name.starts_with(&filter))
            .collect(),
        _ => cases,
    };
    assert!(!cases.is_empty(), "LNMAI_CORPUS_FILTER selected no cases");
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    // Each shard owns an independent Lean process and game state. Parallelize
    // complete playthroughs without dropping any frame or checkpoint.
    let workers = std::env::var("LNMAI_CORPUS_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(1, usize::from)
                .min(4)
        })
        .min(cases.len());
    let mut shards = vec![Vec::new(); workers];
    for (index, case) in cases.into_iter().enumerate() {
        shards[index % workers].push(case);
    }
    let frame_count: usize = std::thread::scope(|scope| {
        let handles: Vec<_> = shards
            .into_iter()
            .map(|shard| scope.spawn(|| check_real_corpus_shard(&cli, shard)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("corpus playthrough failed"))
            .sum()
    });
    eprintln!("corpus runtime total: {frame_count} compared frames");
}

fn check_real_corpus_shard(cli: &str, cases: Vec<(String, String, u32)>) -> usize {
    use lnmai_core::{chart_loader, default_tactic, ffi, simai::frontend};
    use std::collections::BTreeMap;
    let mut lean = LeanRuntime::spawn(cli).unwrap();
    let mut frame_count = 0;
    for (name, content, level) in cases {
        for classic in [false, true] {
            let spec = frontend::frontend_lowered_chart_with_mode(&content, level.into(), classic)
                .unwrap();
            let loaded = lean.request(
                &serde_json::json!({"op":"loadChart", "chart":ffi::chart_spec_json(&spec)}),
            );
            assert_eq!(loaded["ok"], true, "{name}: {loaded}");
            let initial = chart_loader::build_game_state(&spec);
            let plan = default_tactic::autoplay_plan_from_chart(&spec);
            let expected = lean.request(&serde_json::json!({"op":"autoplay"}));
            assert_json_equal(
                &ffi::autoplay_plan_json(&plan),
                &expected["result"],
                &format!("{name} plan"),
            );
            let tactic = default_tactic::default_tactic_from_chart(&spec);
            let expected = lean.request(&serde_json::json!({"op":"tactic"}));
            assert_json_equal(
                &serde_json::json!({"events":tactic.iter().map(ffi::timed_event_json).collect::<Vec<_>>()}),
                &expected["result"],
                &format!("{name} tactic"),
            );
            let mut batches: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
            for event in tactic {
                batches
                    .entry(event.at().micros.to_i64().unwrap())
                    .or_default()
                    .push(ffi::timed_event_json(&event));
            }
            // Match the Lean replay's 16,667 us filler frames between inputs.
            let mut previous = initial.current_time.micros.to_i64().unwrap();
            for target in batches.keys().copied().collect::<Vec<_>>() {
                while previous + 16_667 < target {
                    previous += 16_667;
                    batches.entry(previous).or_default();
                }
                previous = target;
            }
            let end = previous + 2_000_000;
            while previous < end {
                previous += 16_667;
                batches.entry(previous).or_default();
            }
            for autoplay in [true, false] {
                let loaded = lean.request(
                    &serde_json::json!({"op":"loadChart", "chart":ffi::chart_spec_json(&spec)}),
                );
                assert_eq!(loaded["ok"], true);
                let mut state = initial.clone();
                for (frame, (time, events)) in batches.iter().enumerate() {
                    let events: &[Value] = if autoplay { events } else { &[] };
                    let label = format!(
                        "{name} classic={classic} autoplay={autoplay} frame={frame} time={time}"
                    );
                    // Compare every output (including score, audio and render), plus
                    // full states at checkpoints and the final frame. Small synthetic
                    // scenarios above additionally compare full states on every frame.
                    check_step(
                        &mut lean,
                        &mut state,
                        serde_json::json!({"currentTime":time, "events":events}),
                        &label,
                        frame % 128 == 0 || frame + 1 == batches.len(),
                    );
                    frame_count += 1;
                }
                assert!(
                    lnmai_core::scheduler::is_terminated(&state),
                    "{name}: unsettled chart"
                );
                eprintln!("corpus runtime passed: {name} classic={classic} autoplay={autoplay}, {} frames", batches.len());
            }
        }
    }
    frame_count
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn autoplay_seek_settings_and_resume_match_lean() {
    use lnmai_core::{
        chart_loader, default_tactic, ffi,
        input_model::TimedInputEvent,
        time::{Duration, TimePoint},
        types::{JudgeDisplayOption, JudgeStyle},
    };
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let mut charts: Vec<String> = CHARTS
        .iter()
        .filter(|(_, l)| *l == 1)
        .map(|(s, _)| s.to_string())
        .collect();
    charts.push("&first=0\n&inote_1=(120)1-3-5[2:1]/2w6[2:1]/3-7[2:1]*-5[4:1],A1/A2/A3/B1,1h[1:1]/A1h[1:1]/Ch[1:1],\n".into());
    charts.push(format!(
        "&first=0\n&inote_1=(120){{64}}{},\n",
        (0..16)
            .map(|i| format!("{}-{}[1:1]", i % 8 + 1, (i + 4) % 8 + 1))
            .collect::<Vec<_>>()
            .join(",")
    ));
    for (ci, content) in charts.iter().enumerate() {
        for classic in [false, true] {
            let spec = match lnmai_core::simai::frontend::frontend_lowered_chart_with_mode(
                content,
                (1).into(),
                classic,
            ) {
                Ok(spec) => spec,
                Err(_) => {
                    let expected = lean.request(
                        &serde_json::json!({"op":"parse", "content":content, "isClassic":classic}),
                    );
                    let actual: Value = serde_json::from_str(&ffi::parse_lowered_chart_mode_json(
                        content, 1, classic,
                    ))
                    .unwrap();
                    assert_json_equal(&actual, &expected, "invalid runtime chart");
                    continue;
                }
            };
            let loaded = lean.request(
                &serde_json::json!({"op":"loadChart", "chart":ffi::chart_spec_json(&spec)}),
            );
            assert_eq!(loaded["ok"], true, "{loaded}");
            let initial = chart_loader::build_game_state(&spec);
            let mut state = initial.clone();
            let plan = default_tactic::autoplay_plan_from_chart(&spec);
            let expected_plan = lean.request(&serde_json::json!({"op":"autoplay"}));
            assert_json_equal(
                &ffi::autoplay_plan_json(&plan),
                &expected_plan["result"],
                &format!("plan {ci}"),
            );
            let tactic = default_tactic::default_tactic_from_chart(&spec);
            let expected_tactic = lean.request(&serde_json::json!({"op":"tactic"}));
            assert_json_equal(
                &serde_json::json!({"events": tactic.iter().map(ffi::timed_event_json).collect::<Vec<_>>()}),
                &expected_tactic["result"],
                &format!("tactic {ci}"),
            );
            for style in [
                JudgeStyle::Default,
                JudgeStyle::Maji,
                JudgeStyle::Gachi,
                JudgeStyle::Gori,
            ] {
                state.judge_style = style;
                state.touch_panel_offset = Duration::from_micros(12345);
                state.button_ring_for_touch = true;
                state.display_hold_head_judge_result = true;
                state.note_fast_late_display = JudgeDisplayOption::All;
                state.break_fast_late_display = JudgeDisplayOption::BelowP;
                let settings = lean.request(&serde_json::json!({"op":"settings", "judgeStyle":format!("{style:?}"), "touchPanelOffset":12345,
                    "buttonRingForTouch":true, "displayHoldHeadJudgeResult":true, "noteFastLateDisplay":"All", "breakFastLateDisplay":"BelowP"}));
                assert_json_equal(
                    &ffi::game_state_json(&state),
                    &settings["result"],
                    "settings",
                );
                for (target, keep_held) in [
                    (0, false),
                    (499999, true),
                    (500000, false),
                    (750000, true),
                    (1000000, true),
                    (2000000, false),
                    (100000, false),
                ] {
                    let events = plan.events();
                    let expected = lean.request(
                        &serde_json::json!({"op":"seek", "target":target, "keepHeld":keep_held}),
                    );
                    let (next, render) = lnmai_core::runtime_seek::seek(
                        &spec,
                        &initial,
                        &state,
                        TimePoint::from_micros(target),
                        &events,
                        keep_held,
                    );
                    assert_json_equal(
                        &serde_json::json!({"state":ffi::game_state_json(&next), "renderCommands":render.iter().map(ffi::render_cmd_json).collect::<Vec<_>>()}),
                        &expected["result"],
                        &format!(
                            "seek chart={ci} classic={classic} style={style:?} target={target}"
                        ),
                    );
                    state = next;
                    let at_target: Vec<TimedInputEvent> = events
                        .iter()
                        .filter(|e| e.at().micros == target)
                        .cloned()
                        .collect();
                    check_direct_step(
                        &mut lean,
                        &mut state,
                        serde_json::json!({"currentTime": target, "events":at_target.iter().map(ffi::timed_event_json).collect::<Vec<_>>()}),
                        "resume exact target",
                    );
                    check_direct_step(
                        &mut lean,
                        &mut state,
                        serde_json::json!({"currentTime": target+500000, "events":[]}),
                        "resume timeout",
                    );
                }
            }
        }
    }
}

fn rust_native(
    handle: u64,
    operation: u32,
    time: i64,
    level: u32,
    text: &str,
    events: &[Value],
) -> Value {
    use lnmai_core::{input_model::TimedInputEvent as E, native::*};
    let batch = lnmai_core::ffi::parse_timed_input_batch(
        &serde_json::json!({"currentTime":time,"events":events}),
    )
    .unwrap();
    let events: Vec<_> = batch
        .events
        .iter()
        .map(|e| {
            let (kind, position, down) = match *e {
                E::ButtonClick(_, z) => (0, z.to_index(), false),
                E::ButtonHold(_, z, down) => (1, z.to_index(), down),
                E::SensorClick(_, a) => {
                    (2, SENSOR_AREAS.iter().position(|x| *x == a).unwrap(), false)
                }
                E::SensorHold(_, a, down) => {
                    (3, SENSOR_AREAS.iter().position(|x| *x == a).unwrap(), down)
                }
            };
            LnmaiInputEvent {
                time_us: e.at().micros.to_i64().unwrap(),
                kind,
                position: position as u32,
                is_down: down as u32,
                reserved: 0,
            }
        })
        .collect();
    let r = LnmaiRequest {
        time_us: time,
        level_index: level,
        text: text.as_ptr().cast(),
        text_len: text.len() as u64,
        events: events.as_ptr(),
        event_count: events.len() as u64,
        ..Default::default()
    };
    unsafe {
        let mut out = std::ptr::null_mut();
        let status = lnmai_v3_call(handle, operation, &r, &mut out);
        let mut count = 0;
        let records = if out.is_null() {
            std::ptr::null()
        } else {
            lnmai_v3_records(out, &mut count)
        };
        let records: Vec<_> = if count == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(records, count as usize)
                .iter()
                .map(|r| {
                    let mut row = vec![r.tag];
                    row.extend(r.words);
                    row.into_iter().map(|w| w.to_string()).collect::<Vec<_>>()
                })
                .collect()
        };
        let error = if status == 0 {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(lnmai_v3_error(out))
                .to_string_lossy()
                .into_owned()
        };
        lnmai_v3_free_result(out);
        serde_json::json!({"status":status,"records":records,"error":error})
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn native_records_and_session_errors_match_lean() {
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    for (content, level) in CHARTS {
        let created = lean.request(&serde_json::json!({"op":"native", "operation":0}));
        let lean_handle = created["records"][0][1]
            .as_str()
            .expect("native create record")
            .parse::<u64>()
            .unwrap();
        let created = rust_native(0, 0, 0, 1, "", &[]);
        let rust_handle = created["records"][0][1]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        for (op, time) in [
            (2, 0),
            (4, 0),
            (1, 0),
            (1, 0),
            (4, 0),
            (5, 0),
            (6, 0),
            (2, 0),
            (2, 500000),
            (2, 1000000),
            (2, 0),
            (3, 750000),
            (6, 0),
            (2, 750000),
            (3, 0),
            (3, 10000000),
            (3, -1),
            (7, 0),
        ] {
            let expected = lean.request(
                &serde_json::json!({"op":"native","operation":op,"handle":lean_handle,
                "time":time,"content":content,"levelIndex":level}),
            );
            let result = rust_native(rust_handle, op, time, *level, content, &[]);
            assert_json_equal(
                &result,
                &expected,
                &format!("native op={op} time={time} chart={content:?}"),
            );
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn native_input_events_match_lean() {
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    for (name, content, batches) in runtime_scenarios() {
        let created = lean.request(&serde_json::json!({"op":"native", "operation":0}));
        let lh = created["records"][0][1]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let created = rust_native(0, 0, 0, 1, "", &[]);
        let rh = created["records"][0][1]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let expected = lean.request(
            &serde_json::json!({"op":"native", "operation":1,"handle":lh,"content":content}),
        );
        assert_json_equal(&rust_native(rh, 1, 0, 1, content, &[]), &expected, name);
        for (i, batch) in batches.iter().enumerate() {
            let time = batch["currentTime"].as_i64().unwrap();
            let events = batch["events"].as_array().unwrap();
            let expected = lean.request(&serde_json::json!({"op":"native","operation":2,"handle":lh,"time":time,"events":events}));
            assert_json_equal(
                &rust_native(rh, 2, time, 1, "", events),
                &expected,
                &format!("native {name} frame {i}"),
            );
        }
        let expected = lean.request(&serde_json::json!({"op":"native","operation":7,"handle":lh}));
        assert_json_equal(&rust_native(rh, 7, 0, 1, "", &[]), &expected, name);
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn exact_arithmetic_matches_lean() {
    use lnmai_core::{
        ffi::{integer_json, natural_json},
        integer::Integer,
        natural::Natural,
        rat::Rat,
    };
    use num_bigint::BigInt;

    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let mut boundaries: Vec<BigInt> = [-1, 0, 1].map(BigInt::from).into();
    for bits in [31usize, 32, 63, 64, 127, 128, 1024] {
        for offset in [-1, 0, 1] {
            let n = (BigInt::from(1u8) << bits) + BigInt::from(offset);
            boundaries.push(n.clone());
            boundaries.push(-n);
        }
    }
    let mut inputs = Vec::new();
    for (i, a) in boundaries.iter().enumerate() {
        for (j, b) in boundaries.iter().enumerate() {
            inputs.push([
                a.clone(),
                b.clone(),
                boundaries[(i + 1) % boundaries.len()].clone(),
                boundaries[(j + 1) % boundaries.len()].clone(),
            ]);
        }
    }
    // Seeded multi-limb values exercise carries, normalization and division
    // with unrelated numerators and denominators, up to 2048 bits.
    let mut seed = 0x39e2_c718_a094_65bdu64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut big = || {
        let limbs = (next() % 33) as usize;
        let negative = next() & 1 != 0;
        let mut value = BigInt::from(0);
        for _ in 0..limbs {
            value = (value << 64) + next();
        }
        if negative {
            -value
        } else {
            value
        }
    };
    for _ in 0..512 {
        inputs.push([big(), big(), big(), big()]);
    }
    let pair = |r: Rat| serde_json::json!([integer_json(r.num()), natural_json(r.den())]);
    for (chunk, input) in inputs.chunks(64).enumerate() {
        let mut cases = Vec::new();
        let mut results = Vec::new();
        for [a, b, c, d] in input {
            let ai = Integer::from(a.clone());
            let bi = Integer::from(b.clone());
            cases.push(serde_json::json!({"kind":"integer","a":ai,"b":bi}));
            results.push(serde_json::json!([
                &ai + &bi,
                &ai - &bi,
                &ai * &bi,
                ai.clone() / &bi,
                -ai.clone(),
                ai.abs(),
                ai == bi,
                ai < bi,
                ai <= bi,
                ai.to_word()
            ]));
            let an = Natural::from(a.magnitude().clone());
            let bn = Natural::from(b.magnitude().clone());
            cases.push(serde_json::json!({"kind":"natural","a":an,"b":bn}));
            results.push(serde_json::json!([
                &an + &bn,
                &an - &bn,
                &an * &bn,
                &an / &bn,
                &an % &bn,
                an == bn,
                an < bn,
                an <= bn,
                an.to_word()
            ]));
            let ar = Rat::new(a.clone(), b.clone());
            let br = Rat::new(c.clone(), d.clone());
            cases.push(serde_json::json!({"kind":"rational",
                "a":integer_json(Integer::from(a.clone())),
                "b":integer_json(Integer::from(b.clone())),
                "c":integer_json(Integer::from(c.clone())),
                "d":integer_json(Integer::from(d.clone()))}));
            results.push(serde_json::json!([
                pair(ar.clone()),
                pair(br.clone()),
                pair(ar.add(&br)),
                pair(ar.sub(&br)),
                pair(ar.mul(&br)),
                pair(ar.div(&br)),
                ar == br,
                ar < br,
                ar <= br,
                ar.to_decimal_string()
            ]));
        }
        let expected = lean.request(&serde_json::json!({"op":"numeric","cases":cases}));
        assert_eq!(expected["ok"], true, "{expected}");
        let expected = expected["result"].as_array().unwrap();
        assert_eq!(results.len(), expected.len());
        for (index, (actual, expected)) in results.iter().zip(expected).enumerate() {
            assert_json_equal(
                actual,
                expected,
                &format!("arithmetic chunk={chunk} case={index}"),
            );
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn numeric_boundaries_match_lean() {
    use lnmai_core::{
        ffi::grade_str,
        integer::Integer,
        judge,
        rat::Rat,
        score,
        time::{self, Duration},
    };
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let mut cases = Vec::new();
    let mut results = Vec::new();
    for value in [-1000001i64, -101, -3, -2, -1, 0, 1, 2, 3, 101, 1000001] {
        for divisor in [0u32, 1, 2, 3, 100, 1000000] {
            cases.push(serde_json::json!({"kind":"div","value":value,"divisor":divisor}));
            results.push(serde_json::json!(
                Duration::from_micros(value).div_nat(divisor).micros
            ));
            cases.push(serde_json::json!({"kind":"quantize","num":value,"den":divisor}));
            results.push(serde_json::json!(lnmai_core::ffi::integer_json(
                time::quantize_rat_micros(Rat::new(value, divisor))
            )));
        }
    }
    for text in [
        "",
        "0",
        "-0.25",
        "--1",
        "+1",
        "-+1",
        "1.",
        ".5",
        " 1 ",
        "\u{00a0}1",
        "\u{000b}1",
        "0.0000005",
        "-0.0000001",
        "2.0000009999",
        "1.2.3",
    ] {
        cases.push(serde_json::json!({"kind":"seconds","text":text}));
        results.push(serde_json::json!(
            time::quantize_seconds_string(text).map(|v| lnmai_core::ffi::integer_json(&v))
        ));
    }
    // Lean accepts arbitrary-size integers and decimal denominators. These
    // cases used to overflow i128 or be rejected during decimal parsing.
    for bits in [128usize, 256, 1024] {
        let large = num_bigint::BigInt::from(1u8) << bits;
        for sign in [-1, 1] {
            let numerator: num_bigint::BigInt = &large * sign + 1;
            for denominator in [num_bigint::BigInt::from(3), &large - 1] {
                cases.push(serde_json::json!({"kind":"quantize",
                    "num":lnmai_core::ffi::integer_json(Integer::from(numerator.clone())),
                    "den":lnmai_core::ffi::integer_json(Integer::from(denominator.clone()))}));
                results.push(lnmai_core::ffi::integer_json(time::quantize_rat_micros(
                    Rat::new(numerator.clone(), denominator),
                )));
            }
            let text =
                format!("{numerator}.0000004999999999999999999999999999999999999999999999999999");
            cases.push(serde_json::json!({"kind":"seconds","text":text}));
            results.push(serde_json::json!(
                time::quantize_seconds_string(&text).map(|v| lnmai_core::ffi::integer_json(&v))
            ));
        }
    }
    for boundary in [
        0i64, 16667, 33334, 50001, 66668, 83335, 100002, 150000, 150003, 200004, 250005, 300006,
        316673,
    ] {
        for delta in [-1, 0, 1] {
            for sign in [-1, 1] {
                let diff = (boundary + delta) * sign;
                let d = Duration::from_micros(diff);
                for ex in [false, true] {
                    for (kind, result) in [
                        (
                            "tap",
                            serde_json::json!(grade_str(judge::judge_tap(d.clone(), ex))),
                        ),
                        (
                            "touch",
                            serde_json::json!(judge::judge_touch(d.clone(), ex).map(grade_str)),
                        ),
                        (
                            "slideClassic",
                            serde_json::json!(grade_str(judge::judge_slide_classic(d.clone()))),
                        ),
                        (
                            "slideModern",
                            serde_json::json!(grade_str(judge::judge_slide_modern(
                                d.clone(),
                                Duration::from_micros(500000),
                                ex
                            ))),
                        ),
                    ] {
                        cases.push(
                            serde_json::json!({"kind":kind,"diff":diff,"ex":ex,"stay":500000}),
                        );
                        results.push(result);
                    }
                }
            }
        }
    }
    for tb in [0usize, 1, 500, 100000] {
        for cb in [0usize, 1, 500, 100000] {
            for te in [0usize, 1, 100, 12345] {
                let r = score::compute_acc_rates(tb, cb, 1000, te, 50, 100, 800);
                cases.push(
                    serde_json::json!({"kind":"acc","totalBase":tb,"currentBase":cb,"lostBase":1000,
                    "totalExtra":te,"currentExtra":50,"lostExtra":100,"classicExtra":800}),
                );
                results.push(serde_json::json!([
                    r.classic_acc_plus,
                    r.classic_acc_minus,
                    r.dx_acc_minus101,
                    r.dx_acc_minus100,
                    r.dx_acc_plus
                ]
                .map(|r| [
                    lnmai_core::ffi::integer_json(r.num()),
                    lnmai_core::ffi::natural_json(r.den())
                ])));
            }
        }
    }
    let expected = lean.request(&serde_json::json!({"op":"numeric","cases":cases}));
    assert_eq!(expected["ok"], true, "{expected}");
    for (i, (result, expected)) in results
        .iter()
        .zip(expected["result"].as_array().unwrap())
        .enumerate()
    {
        assert_json_equal(result, expected, &format!("numeric {}", cases[i]));
    }
    assert_eq!(results.len(), expected["result"].as_array().unwrap().len());
}

fn json_oracle(lean: &mut LeanRuntime, action: &str, arg: &str, batch: &str) -> Value {
    lean.request(&serde_json::json!({"op":"json", "action":action,"arg":arg,"batchText":batch}))
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn arbitrary_time_judgments_match_lean() {
    use lnmai_core::{
        ffi::{bigint_json, grade_str},
        judge,
        time::Duration,
        types::JudgeGrade::*,
    };
    use num_bigint::BigInt;
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let grades = [
        Miss,
        TooFast,
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
    ];
    let mut cases = Vec::new();
    let mut results = Vec::new();
    for bits in [63usize, 64, 128, 1024] {
        let huge = BigInt::from(1u8) << bits;
        let diffs = [
            -&huge,
            &huge - 1,
            huge.clone(),
            BigInt::from(-1),
            BigInt::from(0),
            BigInt::from(1),
            BigInt::from(16667),
            BigInt::from(-150000),
            BigInt::from(600013),
        ];
        let stays = [
            -&huge,
            BigInt::from(-4000001),
            BigInt::from(-1000001),
            BigInt::from(-1),
            BigInt::from(0),
            BigInt::from(4000001),
            huge.clone(),
        ];
        for diff in &diffs {
            let d = Duration::from_micros(diff.clone());
            for ex in [false, true] {
                for (kind, result) in [
                    (
                        "tap",
                        serde_json::json!(grade_str(judge::judge_tap(d.clone(), ex))),
                    ),
                    (
                        "touch",
                        serde_json::json!(judge::judge_touch(d.clone(), ex).map(grade_str)),
                    ),
                    (
                        "slideClassic",
                        serde_json::json!(grade_str(judge::judge_slide_classic(d.clone()))),
                    ),
                ] {
                    cases.push(serde_json::json!({"kind":kind,"diff":bigint_json(diff),"ex":ex}));
                    results.push(result);
                }
                for stay in &stays {
                    cases.push(serde_json::json!({"kind":"slideModern","diff":bigint_json(diff),"stay":bigint_json(stay),"ex":ex}));
                    results.push(serde_json::json!(grade_str(judge::judge_slide_modern(
                        d.clone(),
                        Duration::from_micros(stay.clone()),
                        ex
                    ))));
                }
            }
            for offset in &stays {
                cases.push(serde_json::json!({"kind":"tooLate","diff":bigint_json(diff),"offset":bigint_json(offset)}));
                results.push(serde_json::json!(judge::is_too_late_slide(
                    d.clone(),
                    Duration::from_micros(offset.clone())
                )));
            }
        }
        for grade in grades {
            // The restored-state API accepts arbitrary durations, including
            // negative/short lengths and ignore/release times. Exercise both
            // clamping stages and the zero-duration early return.
            for (length, ignore) in [
                (-&huge, -&huge * 2),
                (BigInt::from(-1), -&huge),
                (BigInt::from(0), -&huge),
                (BigInt::from(300000), BigInt::from(0)),
                (BigInt::from(300001), BigInt::from(0)),
                (BigInt::from(300001), BigInt::from(300001)),
                (huge.clone(), &huge + 1),
                (huge.clone(), -&huge),
            ] {
                for release in [-&huge, BigInt::from(0), BigInt::from(1), huge.clone()] {
                    for diff in [-1, 0, 1] {
                        cases.push(serde_json::json!({"kind":"hold","grade":grade_str(grade),
                            "diff":diff,"length":bigint_json(&length),"ignore":bigint_json(&ignore),
                            "release":bigint_json(&release)}));
                        results.push(serde_json::json!(grade_str(judge::judge_hold_end(
                            grade,
                            Duration::from_micros(diff),
                            Duration::from_micros(length.clone()),
                            Duration::from_micros(ignore.clone()),
                            Duration::from_micros(release.clone())
                        ))));
                    }
                }
            }
            // Cancellation leaves a small final-release difference even though
            // both timestamps lie far outside the native signed integer range.
            for diff in [
                -&huge,
                huge.clone(),
                BigInt::from(-1),
                BigInt::from(0),
                BigInt::from(150000),
                BigInt::from(300007),
            ] {
                let timing = -&huge;
                let length = &huge * 3;
                let release = &timing + &length - &diff;
                cases.push(serde_json::json!({"kind":"holdClassic","grade":grade_str(grade),
                    "timing":bigint_json(&timing),"length":bigint_json(&length),"release":bigint_json(&release)}));
                results.push(serde_json::json!(grade_str(judge::judge_hold_classic_end(
                    grade,
                    timing,
                    Duration::from_micros(length),
                    release
                ))));
            }
            // Exercise every percentage boundary on exact huge cross products.
            for percent in [0, 1, 4, 5, 32, 33, 66, 67, 99, 100, 101] {
                for delta in [-1, 0, 1] {
                    let reality = &huge * 100;
                    let length = &reality + 300000;
                    let release = &reality - &huge * percent + delta;
                    let diff = -&huge;
                    cases.push(serde_json::json!({"kind":"hold","grade":grade_str(grade),
                        "diff":bigint_json(&diff),"length":bigint_json(&length),"ignore":300000,"release":bigint_json(&release)}));
                    results.push(serde_json::json!(grade_str(judge::judge_hold_end(
                        grade,
                        Duration::from_micros(diff),
                        Duration::from_micros(length),
                        Duration::from_micros(300000),
                        Duration::from_micros(release)
                    ))));
                }
            }
        }
    }
    // Keep oracle messages small enough that diagnostics identify the case.
    for (batch, actual) in cases.chunks(200).zip(results.chunks(200)) {
        let expected = lean.request(&serde_json::json!({"op":"numeric","cases":batch}));
        assert_eq!(expected["ok"], true, "{expected}");
        let expected = expected["result"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for ((case, result), expected) in batch.iter().zip(actual).zip(expected) {
            assert_json_equal(result, expected, &format!("arbitrary time {case}"));
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn arbitrary_timeline_parser_and_runtime_match_lean() {
    use lnmai_core::ffi;
    use num_bigint::BigInt;
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let parser = lean_cli().expect("Lean parser oracle is required");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    for bits in [64usize, 128, 1024] {
        let huge = BigInt::from(1u8) << bits;
        for sign in [-1, 1] {
            let first = &huge * sign;
            let content = format!("&first={first}\n&inote_1=(120)1b/2h[4:1]/A1/A2h[4:1]/3-5[4:1],");
            let chart = decode_response(ffi::parse_lowered_chart_json(&content, 1));
            let expected = lean_lowered(&parser, &content, 1).unwrap();
            assert_json_equal(&chart, &expected, &format!("parser first={first}"));
            assert_eq!(chart["ok"], true, "{chart}");
            let text = chart["result"].to_string();
            let mut state = json_oracle(&mut lean, "build", &text, "")["result"].clone();
            assert_json_equal(
                &decode_response(ffi::build_game_state_json(&text))["result"],
                &state,
                "large timeline build",
            );
            for offset in [-1000000, 0, 40000, 150001, 500000, 1000001, 10000000] {
                let time = ffi::bigint_json(&(&first * 1000000 + offset));
                let events: Vec<_> = ["K1","K2","K3"].iter().map(|zone|
                    serde_json::json!({"buttonClick":{"tp":time,"zone":zone}}))
                    .chain(["A1","A2"].iter().map(|area|
                        serde_json::json!({"sensorHold":{"tp":time,"area":area,"isDown":true}}))).collect();
                let batch = serde_json::json!({"currentTime":time,"events":events}).to_string();
                let text = state.to_string();
                let expected = json_oracle(&mut lean, "step", &text, &batch);
                assert_eq!(expected["ok"], true, "{expected}");
                let result = decode_response(ffi::step_game_state_json(&text, &batch));
                assert_json_equal(
                    &result,
                    &expected,
                    &format!("timeline 2^{bits} sign={sign} offset={offset}"),
                );
                state = expected["result"]["state"].clone();
            }
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn arbitrary_natural_scores_match_lean() {
    use lnmai_core::{
        ffi::{grade_str, integer_json, natural_json},
        integer::Integer,
        natural::Natural,
        score,
        types::JudgeGrade::*,
    };
    use num_bigint::BigUint;
    let mut lean = LeanRuntime::spawn(&lean_runtime_cli().expect("Lean oracle required")).unwrap();
    let grades = [
        Miss,
        TooFast,
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
    ];
    let mut cases = Vec::new();
    let mut results = Vec::new();
    let values: Vec<Natural> = [0u32, 1, 3, 9, 10, 500, u32::MAX]
        .into_iter()
        .map(Natural::from)
        .chain([64usize, 128, 1024].map(|bits| Natural::from((BigUint::from(1u8) << bits) + 17u8)))
        .collect();
    for n in &values {
        for grade in grades {
            for base in &values {
                cases.push(serde_json::json!({"kind":"nonbreak", "base":base, "grade":grade_str(grade), "multiple":n}));
                results.push(serde_json::json!(score::score_non_break(base, grade, n)));
            }
            cases.push(serde_json::json!({"kind":"break", "grade":grade_str(grade), "multiple":n}));
            let (a, b, c, d, e, f) = score::score_break(grade, n);
            results.push(serde_json::json!([a, [b, [c, [d, [e, f]]]]]));
            for sign in [-1, 1] {
                let dx = Integer::from(n) * sign;
                cases.push(
                    serde_json::json!({"kind":"combo", "combo":n, "pCombo":n+1, "cPCombo":n+2,
                    "dxScoreLost":integer_json(&dx), "grade":grade_str(grade), "multiple":n}),
                );
                let r = score::update_combo(n, n + 1, n + 2, dx, grade, n);
                results.push(serde_json::json!([
                    r.combo,
                    r.p_combo,
                    r.c_p_combo,
                    integer_json(&r.dx_score_lost)
                ]));
            }
        }
        for threshold in [0, 85, 90, 93, 95, 97, 100, 101] {
            for delta in [0, 1, 2] {
                let max = n * 100;
                let achieved = n * threshold + delta - 1;
                cases.push(serde_json::json!({"kind":"rank", "achieved":achieved, "max":max}));
                results.push(serde_json::json!(score::dx_score_rank(achieved, max)));
            }
        }
        for tb in &values {
            let r = score::compute_acc_rates(tb, n, n + 1, n, n + 2, n + 3, n + 4);
            cases.push(
                serde_json::json!({"kind":"acc", "totalBase":tb, "currentBase":n, "lostBase":n+1,
                "totalExtra":n, "currentExtra":n+2, "lostExtra":n+3, "classicExtra":n+4}),
            );
            results.push(serde_json::json!([
                r.classic_acc_plus,
                r.classic_acc_minus,
                r.dx_acc_minus101,
                r.dx_acc_minus100,
                r.dx_acc_plus
            ]
            .map(|r| [integer_json(r.num()), natural_json(r.den())])));
        }
    }
    // Keep each oracle request small enough to avoid testing its recursion limit.
    for (inputs, outputs) in cases.chunks(100).zip(results.chunks(100)) {
        let response = lean.request(&serde_json::json!({"op":"numeric", "cases":inputs}));
        let expected = response["result"].as_array().expect("numeric result array");
        assert_eq!(outputs.len(), expected.len());
        for ((input, actual), expected) in inputs.iter().zip(outputs).zip(expected) {
            assert_json_equal(actual, expected, &format!("natural score {input}"));
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn arbitrary_natural_chart_and_runtime_match_lean() {
    use lnmai_core::{ffi, natural::Natural, simai::frontend};
    use num_bigint::BigUint;
    let mut lean = LeanRuntime::spawn(&lean_runtime_cli().expect("Lean oracle required")).unwrap();
    fn bump_fields(v: &mut Value, fields: &[&str], huge: &BigUint) {
        match v {
            Value::Object(map) => {
                for (key, value) in map {
                    if fields.contains(&key.as_str()) && value.is_number() {
                        let n: BigUint = value.to_string().parse().unwrap();
                        *value = ffi::biguint_json(&(n + huge));
                    } else {
                        bump_fields(value, fields, huge);
                    }
                }
            }
            Value::Array(a) => {
                for value in a {
                    bump_fields(value, fields, huge);
                }
            }
            _ => {}
        }
    }
    let content =
        "&first=0\n&inote_1=(120)1b/2/3h[4:1]/A1/A2/A3h[4:1]/A4h[4:1]/4-6[8:1]-8[8:1],5w1[4:1],";
    let spec = decode_response(ffi::parse_lowered_chart_json(content, 1))["result"].clone();
    for bits in [64usize, 128, 1024] {
        let huge = (BigUint::from(1u8) << bits) + 17u8;
        let mut chart = spec.clone();
        bump_fields(
            &mut chart,
            &[
                "noteIndex",
                "logicalSlideId",
                "parentNoteIndex",
                "sourceGroupId",
                "touchGroupId",
                "touchHoldGroupId",
                "totalJudgeQueueLen",
            ],
            &huge,
        );
        let text = chart.to_string();
        let built = json_oracle(&mut lean, "build", &text, "");
        assert_eq!(built["ok"], true, "{built}");
        assert_json_equal(
            &decode_response(ffi::build_game_state_json(&text)),
            &built,
            &format!("build 2^{bits}"),
        );
        assert_json_equal(
            &decode_response(ffi::default_tactic_from_chart_json(&text)),
            &json_oracle(&mut lean, "tactic", &text, ""),
            &format!("tactic 2^{bits}"),
        );
        for cursors in [false, true] {
            let mut state = built["result"].clone();
            bump_fields(
                &mut state,
                &[
                    "multiple",
                    "initialQueueRemaining",
                    "arrowProgress",
                    "count",
                    "size",
                ],
                &huge,
            );
            if cursors {
                bump_fields(
                    &mut state,
                    &["currentIndex", "buttonQueueIndex", "touchQueueIndex"],
                    &huge,
                );
                for key in ["buttonQueueFrontiers", "touchQueueFrontiers"] {
                    for value in state[key].as_array_mut().unwrap() {
                        *value = ffi::biguint_json(&huge);
                    }
                }
            }
            for time in [-1000000, 0, 100000, 600000, 1500000, 10000000] {
                let events: Vec<_> = ["K1", "K2", "K3", "K4"]
                    .iter()
                    .map(|zone| serde_json::json!({"buttonClick":{"tp":time,"zone":zone}}))
                    .chain(
                        ["A1", "A2", "A3", "A4"]
                            .iter()
                            .map(|area| serde_json::json!({"sensorClick":{"tp":time,"area":area}})),
                    )
                    .collect();
                let batch = serde_json::json!({"currentTime":time,"events":events}).to_string();
                let text = state.to_string();
                let expected = json_oracle(&mut lean, "step", &text, &batch);
                assert_eq!(expected["ok"], true, "{expected}");
                assert_json_equal(
                    &decode_response(ffi::step_game_state_json(&text, &batch)),
                    &expected,
                    &format!("runtime Nat 2^{bits} cursors={cursors} t={time}"),
                );
                state = expected["result"]["state"].clone();
            }
        }
        for body in [
            format!("(120){{{huge}}}1,2h[{huge}:1],3-5[{huge}:7],"),
            format!("(120)1h[4:{huge}],2-4[8:{huge}],"),
        ] {
            let content = format!("&first=0\n&inote_{huge}={body}");
            let expected = lean.request(&serde_json::json!({"op":"parseNat","content":content,"levelIndex":ffi::biguint_json(&huge)}));
            assert_eq!(expected["ok"], true, "{expected}");
            let actual =
                frontend::frontend_lowered_chart(&content, Natural::from(huge.clone())).unwrap();
            assert_json_equal(
                &ffi::chart_spec_json(&actual),
                &expected["result"],
                &format!("parser Nat 2^{bits} {body}"),
            );
        }
    }
}

fn decode_response(text: String) -> Value {
    serde_json::from_str(&text).expect("Rust FFI must return a JSON envelope")
}

/// Change every object member independently, including fields nested inside
/// notes and queues. This exercises decoding order, defaults, and type errors.
fn json_mutations(value: &Value) -> Vec<(String, Value)> {
    fn visit(root: &Value, node: &Value, path: &str, out: &mut Vec<(String, Value)>) {
        match node {
            Value::Object(fields) => {
                for (key, child) in fields {
                    let child_path =
                        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                    let mut absent = root.clone();
                    absent
                        .pointer_mut(path)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                    out.push((format!("{child_path} absent"), absent));
                    for replacement in [
                        Value::Null,
                        serde_json::json!(false),
                        serde_json::json!(-2),
                        serde_json::json!("invalid\tstring"),
                        serde_json::json!({}),
                        serde_json::json!([]),
                    ] {
                        let mut changed = root.clone();
                        *changed.pointer_mut(&child_path).unwrap() = replacement.clone();
                        out.push((format!("{child_path} = {replacement}"), changed));
                    }
                    visit(root, child, &child_path, out);
                }
            }
            Value::Array(items) => {
                for (i, child) in items.iter().enumerate() {
                    // Queue vectors contain mostly identical empty entries.
                    if i > 0 && items[..i].contains(child) {
                        continue;
                    }
                    visit(root, child, &format!("{path}/{i}"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    visit(value, value, "", &mut out);
    out
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn json_chart_decode_and_state_restore_match_lean() {
    use lnmai_core::ffi;
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    assert_json_equal(
        &decode_response(ffi::ffi_version_json()),
        &json_oracle(&mut lean, "version", "", ""),
        "JSON ABI version",
    );
    let content = "&first=0\n&inote_1=(120)1b/2x/3h[4:1]/A1/A2h[4:1]/4-6[8:1]-8[8:1],5w1[4:1],";
    let parsed = decode_response(ffi::parse_lowered_chart_json(content, 1));
    assert_eq!(parsed["ok"], true, "{parsed}");
    let spec = &parsed["result"];
    for (name, value) in
        std::iter::once(("valid chart".to_string(), spec.clone())).chain(json_mutations(spec))
    {
        let text = value.to_string();
        for action in ["build", "tactic"] {
            let rust = decode_response(match action {
                "build" => ffi::build_game_state_json(&text),
                _ => ffi::default_tactic_from_chart_json(&text),
            });
            let expected = json_oracle(&mut lean, action, &text, "");
            assert_json_equal(&rust, &expected, &format!("{action} {name}"));
        }
    }
    let built = json_oracle(&mut lean, "build", &spec.to_string(), "");
    assert_eq!(built["ok"], true, "{built}");
    let initial_state = &built["result"];
    let batch = serde_json::json!({"currentTime":-1000000,"events":[]}).to_string();
    for (name, state) in std::iter::once(("valid state".to_string(), initial_state.clone()))
        .chain(json_mutations(initial_state))
    {
        let text = state.to_string();
        let rust = decode_response(ffi::step_game_state_json(&text, &batch));
        let expected = json_oracle(&mut lean, "step", &text, &batch);
        assert_json_equal(&rust, &expected, &format!("restore {name}"));
    }
    // Restoring states after real transitions covers active hold bodies,
    // touch groups, slide history, judged/ended states, and accumulated scores.
    for (name, content, batches) in runtime_scenarios() {
        lean.load(content, 1);
        for (i, batch) in batches.iter().enumerate() {
            let state = lean.request(&serde_json::json!({"op":"state"}))["result"].to_string();
            let text = batch.to_string();
            let rust = decode_response(ffi::step_game_state_json(&state, &text));
            let expected = json_oracle(&mut lean, "step", &state, &text);
            assert_json_equal(&rust, &expected, &format!("restore {name} frame {i}"));
            lean.step(batch);
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn restored_state_transitions_match_lean() {
    use lnmai_core::ffi;

    // Keep only paths and replacements, not thousands of copies of the state.
    // Restored states may have different queue lengths, cursor positions,
    // histories and lifecycle variants than chart construction produces.
    fn mutations(node: &Value, path: &str, out: &mut Vec<(String, Value)>) {
        let mut replacements = Vec::new();
        match node {
            Value::Bool(b) => replacements.push(serde_json::json!(!b)),
            Value::Number(_) | Value::Null => {
                replacements.extend([
                    serde_json::json!(0),
                    serde_json::json!(1),
                    serde_json::json!(3),
                    serde_json::json!(-1),
                    serde_json::json!(150000),
                    serde_json::json!(-1000000),
                    serde_json::from_str("1208925819614629174706183").unwrap(),
                ]);
            }
            Value::String(s) => {
                for group in [
                    &["Default", "Maji", "Gachi", "Gori"][..],
                    &["Miss", "TooFast", "Perfect", "FastGood", "LateGood"][..],
                    &["All", "BelowCP", "BelowP", "BelowGR", "MissOnly", "Disable"][..],
                    &["K1", "K2", "K8"][..],
                    &["A1", "A2", "B1", "C", "D8", "E8"][..],
                    &["S1", "S3", "S8"][..],
                    &["Or", "And"][..],
                    &["Single", "Wifi", "ConnPart"][..],
                ] {
                    if group.contains(&s.as_str()) {
                        replacements.extend(group.iter().map(|s| serde_json::json!(s)));
                    }
                }
            }
            Value::Array(items) => {
                if !items.is_empty() {
                    replacements.push(serde_json::json!([]));
                    replacements.push(serde_json::json!(&items[1..]));
                    let mut reversed = items.clone();
                    reversed.reverse();
                    replacements.push(Value::Array(reversed));
                    let mut duplicated = items.clone();
                    duplicated.push(items[0].clone());
                    replacements.push(Value::Array(duplicated));
                }
                for (i, child) in items.iter().enumerate() {
                    mutations(child, &format!("{path}/{i}"), out);
                }
            }
            Value::Object(fields) => {
                for (key, child) in fields {
                    // Arbitrary score values have a separate differential test.
                    if key == "score" {
                        continue;
                    }
                    let child_path =
                        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                    if key == "state" {
                        let variants = if fields.contains_key("headGrade") {
                            serde_json::json!([
                                "HeadWaiting", "HeadJudgeable", "BodyHeld", "BodyReleased",
                                {"HeadJudged":{"grade":"Perfect"}},
                                {"HeadJudged":{"grade":"Miss"}},
                                {"Ended":{"grade":"Perfect"}},
                                {"Ended":{"grade":"Miss"}}
                            ])
                        } else if fields.contains_key("judgeQueues") {
                            serde_json::json!([
                                "Waiting", "Ended", {"Active":{"waitTime":0}},
                                {"Active":{"waitTime":-1}},
                                {"Judged":{"grade":"Perfect","waitTime":100000,"judgeDiff":-1}},
                                {"Judged":{"grade":"Miss","waitTime":0,"judgeDiff":1}}
                            ])
                        } else {
                            serde_json::json!([
                                "Waiting", "Judgeable", "Ended",
                                {"Judged":{"grade":"Perfect"}},
                                {"Judged":{"grade":"Miss"}}
                            ])
                        };
                        out.extend(
                            variants
                                .as_array()
                                .unwrap()
                                .iter()
                                .filter(|v| *v != child)
                                .map(|v| (child_path.clone(), v.clone())),
                        );
                    }
                    mutations(child, &child_path, out);
                }
            }
        }
        replacements.dedup();
        out.extend(
            replacements
                .into_iter()
                .filter(|v| v != node)
                .map(|v| (path.to_string(), v)),
        );
    }

    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let content = "&inote_1=(120)1b/2h[2:1]/A1h[2:1]/A2/A3/3-5[8:1]-7[8:1]/4w8[2:1],";
    lean.load(content, 1);
    let mut states = vec![lean.request(&serde_json::json!({"op":"state"}))["result"].clone()];
    let held: Vec<_> = ["K1", "K2", "K3", "K4"]
        .iter()
        .map(|zone| serde_json::json!({"buttonHold":{"tp":0,"zone":zone,"isDown":true}}))
        .chain(
            ["A1", "A2", "A3", "A4"]
                .iter()
                .map(|area| serde_json::json!({"sensorHold":{"tp":0,"area":area,"isDown":true}})),
        )
        .collect();
    for batch in [
        serde_json::json!({"currentTime":0,"events":held}),
        serde_json::json!({"currentTime":600000,"events":[]}),
    ] {
        lean.step(&batch);
        states.push(lean.request(&serde_json::json!({"op":"state"}))["result"].clone());
    }
    let clicks: Vec<_> = ["K1", "K2", "K3", "K4"]
        .iter()
        .map(|zone| serde_json::json!({"buttonClick":{"tp":0,"zone":zone}}))
        .chain(
            ["A1", "A2", "A3", "A4"]
                .iter()
                .map(|area| serde_json::json!({"sensorClick":{"tp":0,"area":area}})),
        )
        .collect();
    let batches = [
        serde_json::json!({"currentTime":0,"events":clicks}).to_string(),
        serde_json::json!({"currentTime":600000,"events":[]}).to_string(),
        serde_json::json!({"currentTime":3000000,"events":[]}).to_string(),
    ];
    let mut accepted = 0;
    let mut rejected = 0;
    for (snapshot, state) in states.iter().enumerate() {
        let mut changes = Vec::new();
        mutations(state, "", &mut changes);
        for (path, replacement) in changes {
            let mut restored = state.clone();
            *restored.pointer_mut(&path).unwrap() = replacement.clone();
            for (frame, batch) in batches.iter().enumerate() {
                let text = restored.to_string();
                let expected = json_oracle(&mut lean, "step", &text, batch);
                assert_json_equal(
                    &decode_response(ffi::step_game_state_json(&text, batch)),
                    &expected,
                    &format!("restore snapshot={snapshot} {path}={replacement} frame={frame}"),
                );
                if expected["ok"] != true {
                    rejected += 1;
                    break;
                }
                accepted += 1;
                restored = expected["result"]["state"].clone();
            }
        }
    }
    assert!(
        accepted > 10000,
        "restored-state coverage unexpectedly shrank"
    );
    eprintln!("restored state transitions: {accepted} accepted, {rejected} decode errors");
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn arbitrary_score_restore_and_judgments_match_lean() {
    use lnmai_core::ffi;
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let content = "&first=0\n&inote_1=(120)1b/2/3h[4:1]/A1/A2h[4:1]/4-6[4:1],";
    let spec = decode_response(ffi::parse_lowered_chart_json(content, 1))["result"].to_string();
    let initial = json_oracle(&mut lean, "build", &spec, "")["result"].clone();
    fn replace_counts(v: &mut Value, value: &Value) {
        match v {
            Value::Object(map) => {
                for n in map.values_mut() {
                    replace_counts(n, value);
                }
            }
            Value::Array(array) => {
                for n in array {
                    replace_counts(n, value);
                }
            }
            Value::Number(_) => *v = value.clone(),
            _ => panic!("unexpected score field {v}"),
        }
    }
    for bits in [64usize, 128, 1024] {
        let huge = (num_bigint::BigInt::from(1u8) << bits) + 17;
        for sign in [-1, 1] {
            for time in [0, 40000, 80000, 120000, 10000000] {
                let mut state = initial.clone();
                replace_counts(&mut state["score"], &ffi::bigint_json(&huge));
                state["score"]["dxScore"] = ffi::bigint_json(&(&huge * sign));
                let events: Vec<_> = ["K1", "K2", "K3", "K4"]
                    .iter()
                    .map(|zone| serde_json::json!({"buttonClick":{"tp":time,"zone":zone}}))
                    .chain(
                        ["A1", "A2"]
                            .iter()
                            .map(|area| serde_json::json!({"sensorClick":{"tp":time,"area":area}})),
                    )
                    .collect();
                let batch = serde_json::json!({"currentTime":time,"events":events}).to_string();
                let text = state.to_string();
                let expected = json_oracle(&mut lean, "step", &text, &batch);
                assert_eq!(expected["ok"], true, "{expected}");
                assert_json_equal(
                    &decode_response(ffi::step_game_state_json(&text, &batch)),
                    &expected,
                    &format!("score 2^{bits} sign={sign} time={time}"),
                );
            }
        }
    }
}

#[test]
#[ignore = "requires Lean oracle binaries; run tools/run_differential.sh"]
fn json_text_and_input_errors_match_lean() {
    use lnmai_core::ffi;
    let cli = lean_runtime_cli().expect("Lean oracle is required; run tools/run_differential.sh");
    let mut lean = LeanRuntime::spawn(&cli).unwrap();
    let spec = ffi::chart_spec_json(&lnmai_core::chart_loader::ChartSpec::default()).to_string();
    let state = json_oracle(&mut lean, "build", &spec, "")["result"].to_string();
    for text in [
        "",
        " ",
        "{",
        "[",
        "[1",
        "{\"x\"}",
        "[1,]",
        "{\"x\":}",
        "null false",
        "n",
        "nuL",
        "falsx",
        "tru",
        "+1",
        "-",
        "01",
        "1.",
        "1e",
        "1e+",
        "1e-",
        "1eX",
        "\"\\q\"",
        "\"\\u12x4\"",
        "\"x\n\"",
        "\"\\ud800\"",
        "\"\\ud800x\"",
        "\"\\ud800\\udc00\"",
        "\"\\udc00\"",
        "true",
        "null",
        "0.0",
        "1e2",
        "[]",
        "{}",
        "{\"taps\":{\"x\":\"你好\",\"y\":\"\t\"}}",
    ] {
        let rust = decode_response(ffi::build_game_state_json(text));
        assert_json_equal(
            &rust,
            &json_oracle(&mut lean, "build", text, ""),
            &format!("chart text {text:?}"),
        );
        let rust = decode_response(ffi::step_game_state_json(&state, text));
        assert_json_equal(
            &rust,
            &json_oracle(&mut lean, "step", &state, text),
            &format!("batch text {text:?}"),
        );
    }
    let batch = serde_json::json!({"currentTime":0,"events":[
        {"buttonClick":{"tp":0,"zone":"K1"}}, {"sensorClick":{"tp":0,"area":"C"}},
        {"buttonHold":{"tp":0,"zone":"K2","isDown":true}}, {"sensorHold":{"tp":0,"area":"B1","isDown":false}}]});
    for (name, value) in json_mutations(&batch) {
        let text = value.to_string();
        let rust = decode_response(ffi::step_game_state_json(&state, &text));
        assert_json_equal(
            &rust,
            &json_oracle(&mut lean, "step", &state, &text),
            &format!("batch {name}"),
        );
    }
    for number in [
        "0.0",
        "1e2",
        "100e-2",
        "0.1e1",
        "-0.0",
        "-1.0",
        "1.25",
        "10e-1",
        "1e-10000",
        "-1.2e-10000",
        "0e-18446744073709551616",
        "1e-18446744073709551616",
        "1e18446744073709551617",
    ] {
        let text = format!("{{\"currentTime\":{number},\"events\":[]}}");
        assert_json_equal(
            &decode_response(ffi::step_game_state_json(&state, &text)),
            &json_oracle(&mut lean, "step", &state, &text),
            &format!("numeric JSON {number}"),
        );
    }
    // Deterministic malformed JSON with multibyte characters covers byte
    // offsets, delimiters and parser backtracking.
    let alphabet: Vec<char> = "[]{}\":, eE+-01.\\ntf\n\t你好🦀".chars().collect();
    let seeds = [
        "{\"currentTime\":0,\"events\":[]}",
        "{\"currentTime\":-1.2e-3,\"events\":[null]}",
        "{\"events\":[{\"sensorClick\":{\"tp\":0,\"area\":\"C\"}}]}",
        "{\"你好\":\"\\ud800\\udc00\"}",
    ];
    let mut random = 0x2a749e31c650d8bfu64;
    for case in 0..512 {
        let mut chars: Vec<char> = seeds[case % seeds.len()].chars().collect();
        for _ in 0..1 + case % 3 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let at = (random >> 32) as usize % (chars.len() + 1);
            let c = alphabet[(random >> 16) as usize % alphabet.len()];
            match random % 3 {
                0 => chars.insert(at, c),
                1 if at < chars.len() => chars[at] = c,
                _ if at < chars.len() => {
                    chars.remove(at);
                }
                _ => {}
            }
        }
        let text: String = chars.into_iter().collect();
        assert_json_equal(
            &decode_response(ffi::step_game_state_json(&state, &text)),
            &json_oracle(&mut lean, "step", &state, &text),
            &format!("mutated JSON {case}: {text:?}"),
        );
    }
}
