//! Minimal feasibility probe for the all-Perfect autoplay generator idea
//! (`docs/AUTOPLAY_GENERATOR.md`).
//!
//! Flow: model each slide's lifecycle → cut it into fixed time blocks → for
//! every slide, enumerate candidate block timings until its runtime arcs judge
//! non-Miss (backtracking; a slide with no working branch is recorded) → drive
//! lnmai-core to re-check the committed tactic. With `--preview`, the tactic is
//! handed to the pad preview, but **only** when the plan verified all-Perfect.
//!
//! ```text
//! cargo run --bin autoplay_gen -- <chart> [--diff N] [--block S]
//!     [--max-tries N] [--preview] [--default-tactic]
//! ```

// `app`/`core`/`player` are near-verbatim shared modules with unused items.
#![allow(dead_code, unused_variables, unused_imports)]

#[path = "../app/mod.rs"]
mod app;
#[path = "../core/mod.rs"]
mod core;
#[path = "../player/mod.rs"]
mod player;

mod model;
mod planner;
mod report;
mod search;
mod verify;

use std::path::{Path, PathBuf};

use crate::core::types::TimedInputEvent;
use model::build_slide_plans;
use planner::{DEFAULT_BLOCK_S, plan};
use report::print_report;
use verify::VerifyResult;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chart: Option<PathBuf> = None;
    let mut diff: Option<i32> = None;
    let mut block_s = DEFAULT_BLOCK_S;
    let mut preview = false;
    let mut default_tactic = false;
    let mut max_tries = 5000usize;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return;
            }
            "-d" | "--diff" => {
                i += 1;
                diff = args.get(i).and_then(|v| v.parse().ok());
            }
            "--block" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|v| v.parse::<f64>().ok()) {
                    if v > 0.0 {
                        block_s = v;
                    }
                }
            }
            "--max-tries" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    max_tries = v.max(1);
                }
            }
            "--preview" => preview = true,
            "--default-tactic" => default_tactic = true,
            other if !other.starts_with('-') => chart = Some(PathBuf::from(other)),
            other => {
                eprintln!("unknown option: {other}\n{HELP}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let path = chart.unwrap_or_else(|| app::platform::asset_dir().join("charts/test"));
    let mut chart_doc = match app::chart::load_chart_from_path(&path, diff) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load {}: {e}", path.display());
            std::process::exit(2);
        }
    };
    // Simai-converted notes share id 0; make them unique for the report.
    app::maichart::assign_note_ids(&mut chart_doc.notes);

    let mut plans = build_slide_plans(&chart_doc);

    // Core work needs the raw Simai text and a level.
    let text = app::chart::read_simai_source(&path);
    let level = text
        .as_deref()
        .and_then(|t| app::maidata::inote_key(t, diff))
        .or_else(|| (chart_doc.simai_level > 0).then_some(chart_doc.simai_level));
    let (Some(text), Some(level)) = (text, level) else {
        eprintln!("\n(no Simai source/level — planning report only)");
        let units = plan(&mut plans, block_s);
        print_report(&chart_doc, &plans, &units, block_s);
        return;
    };

    // ── Sanity: verify the core's own default tactic ───────────────────
    if default_tactic {
        let events = player::engine::JudgeEngine::load(&text, level)
            .ok()
            .and_then(|e| e.default_tactic().ok())
            .unwrap_or_default();
        let units = plan(&mut plans, block_s);
        print_report(&chart_doc, &plans, &units, block_s);
        println!("\nevents: {} (core default tactic)", events.len());
        let song_end = plans.iter().map(|p| p.end_s).fold(0.0_f64, f64::max) as f32;
        let result = verify::verify(&text, level, &events, song_end);
        print_verify(&result);
        maybe_preview(preview, &path, result.all_perfect(), &result.events);
        return;
    }

    // ── Backtracking enumeration ───────────────────────────────────────
    let outcome = search::search(&text, level, &mut plans, block_s, max_tries);
    let units = planner::group_units(&plans);
    print_report(&chart_doc, &plans, &units, block_s);

    println!("\n== enumeration ==");
    println!(
        "tries: {}   failed slides: {}   (block={:.3}s, max_tries={})",
        outcome.tries,
        outcome.failed.len(),
        block_s,
        max_tries
    );
    if !outcome.failed.is_empty() {
        println!("  failed slides: {:?}", outcome.failed);
    }
    print_verify(&outcome.final_verify);
    maybe_preview(
        preview,
        &path,
        outcome.perfect(),
        &outcome.final_verify.events,
    );
}

fn print_verify(result: &VerifyResult) {
    println!("\n== core verification ==");
    println!(
        "arcs: {}   judged: {}   misses: {}   unjudged: {}",
        result.arcs,
        result.judged,
        result.misses.len(),
        result.unjudged.len()
    );
    if !result.misses.is_empty() {
        println!("  missed arcs: {:?}", result.misses);
    }
    println!(
        "{}",
        if result.all_perfect() {
            "all judged arcs Perfect"
        } else {
            "NOT all-Perfect"
        }
    );
}

/// Preview only when the tactic verified all-Perfect.
fn maybe_preview(preview: bool, chart: &Path, perfect: bool, events: &[TimedInputEvent]) {
    if !preview {
        return;
    }
    if !perfect {
        println!("preview skipped: plan is not all-Perfect");
        return;
    }
    match launch_preview(chart, events) {
        Ok(pid) => println!("launched preview (pid {pid})"),
        Err(e) => eprintln!("preview failed: {e}"),
    }
}

/// Write the generated tactic and spawn `lambda_dx_pad_preview` with autoplay.
fn launch_preview(chart: &Path, events: &[TimedInputEvent]) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("current exe has no parent dir")?;
    let preview = dir.join(format!("lambda_dx_pad_preview{}", std::env::consts::EXE_SUFFIX));
    if !preview.is_file() {
        return Err(format!(
            "{} not found — build it first: cargo build --bin lambda_dx_pad_preview",
            preview.display()
        ));
    }
    let tactic = std::env::temp_dir().join("autoplay_gen_tactic.json");
    let json = serde_json::to_string(events).map_err(|e| e.to_string())?;
    std::fs::write(&tactic, json).map_err(|e| format!("write {}: {e}", tactic.display()))?;
    // Detach: the preview is its own window; don't inherit our stdio.
    let child = std::process::Command::new(&preview)
        .arg(chart)
        .arg("--autoplay")
        .env("MAI2_AUTOPLAY_TACTIC", &tactic)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(child.id())
}

const HELP: &str = "\
autoplay_gen — minimal feasibility probe for the all-Perfect autoplay idea

USAGE:
    autoplay_gen [CHART] [--diff N] [--block SECONDS] [--max-tries N]
                 [--preview] [--default-tactic]

ARGS:
    CHART    Chart folder (maidata.txt) or file. Default: assets/charts/test.

OPTIONS:
    -d, --diff <N>     Difficulty number to load.
    --block <S>        Time-block length in seconds (default 0.1).
    --max-tries <N>    Cap on core evaluations during enumeration (default 5000).
    --default-tactic   Verify the core's own default tactic instead of enumerating.
    --preview          Launch the pad preview with the generated tactic, but
                       only if every judged arc verifies Perfect.
    -h, --help         Print this help.
";
