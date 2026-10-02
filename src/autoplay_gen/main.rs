//! Minimal feasibility probe for the all-Perfect autoplay generator idea
//! (`docs/AUTOPLAY_GENERATOR.md`).
//!
//! Flow: model each slide's lifecycle → cut it into fixed time blocks →
//! greedily enumerate per-segment trigger times → build a sensor event stream →
//! drive lnmai-core and check every slide arc is non-Miss. With `--preview`,
//! the generated tactic is handed to the pad preview, but **only** when the
//! plan verified as all-Perfect.
//!
//! ```text
//! cargo run --bin autoplay_gen -- <chart> [--diff N] [--block S] [--preview]
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
mod verify;

use std::path::{Path, PathBuf};

use crate::core::types::TimedInputEvent;
use model::build_slide_plans;
use planner::{DEFAULT_BLOCK_S, plan};
use report::print_report;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chart: Option<PathBuf> = None;
    let mut diff: Option<i32> = None;
    let mut block_s = DEFAULT_BLOCK_S;
    let mut preview = false;
    let mut default_tactic = false;

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

    // ── Plan ───────────────────────────────────────────────────────────
    let mut plans = build_slide_plans(&chart_doc);
    let units = plan(&mut plans, block_s);
    print_report(&chart_doc, &plans, &units, block_s);

    // ── Drive the core ─────────────────────────────────────────────────
    let Some(text) = app::chart::read_simai_source(&path) else {
        eprintln!("\n(core verification skipped: no Simai source at {})", path.display());
        return;
    };
    let level = app::maidata::inote_key(&text, diff).or_else(|| {
        (chart_doc.simai_level > 0).then_some(chart_doc.simai_level)
    });
    let Some(level) = level else {
        eprintln!("\n(core verification skipped: no &inote_N level)");
        return;
    };

    let events = if default_tactic {
        // Sanity path: verify the core's own default tactic through the same
        // pipeline (useful to prove the verify + preview hand-off).
        player::engine::JudgeEngine::load(&text, level)
            .ok()
            .and_then(|e| e.default_tactic().ok())
            .unwrap_or_default()
    } else {
        verify::build_events(&plans, 0.05)
    };
    println!(
        "\nevents: {} ({})",
        events.len(),
        if default_tactic { "core default tactic" } else { "plan-generated" }
    );
    let song_end_s = plans.iter().map(|p| p.end_s).fold(0.0_f64, f64::max) as f32;
    let result = verify::verify(&text, level, &events, song_end_s);

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
    if result.all_perfect() {
        println!("all judged arcs Perfect");
    } else {
        println!("NOT all-Perfect");
    }

    // ── Preview (only when every block is Perfect) ─────────────────────
    if !preview {
        return;
    }
    if !result.all_perfect() {
        println!("preview skipped: plan is not all-Perfect");
        return;
    }
    match launch_preview(&path, &result.events) {
        Ok(child) => println!("launched preview (pid {child})"),
        Err(e) => eprintln!("preview failed: {e}"),
    }
}

/// Write the generated tactic and spawn `lambda_dx_pad_preview` with autoplay.
fn launch_preview(chart: &Path, events: &[TimedInputEvent]) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("current exe has no parent dir")?;
    let preview = dir.join(format!(
        "lambda_dx_pad_preview{}",
        std::env::consts::EXE_SUFFIX
    ));
    if !preview.is_file() {
        return Err(format!(
            "{} not found — build it first: cargo build --bin lambda_dx_pad_preview",
            preview.display()
        ));
    }
    let tactic = std::env::temp_dir().join("autoplay_gen_tactic.json");
    let json = serde_json::to_string(events).map_err(|e| e.to_string())?;
    std::fs::write(&tactic, json).map_err(|e| format!("write {}: {e}", tactic.display()))?;
    // Detach: the preview is its own window; don't inherit our stdio (otherwise
    // a caller piping this process would block until the window closes).
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
    autoplay_gen [CHART] [--diff N] [--block SECONDS] [--preview]

ARGS:
    CHART    Chart folder (maidata.txt) or file. Default: assets/charts/test.

OPTIONS:
    -d, --diff <N>     Difficulty number to load.
    --block <S>        Time-block length in seconds (default 0.1).
    --default-tactic   Verify the core's own default tactic instead of the
                       plan-generated events.
    --preview          Launch the pad preview with the generated tactic, but
                       only if every slide arc verifies Perfect.
    -h, --help         Print this help.
";
