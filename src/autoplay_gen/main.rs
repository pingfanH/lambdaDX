//! Minimal feasibility probe for the all-Perfect autoplay generator idea
//! (`docs/AUTOPLAY_GENERATOR.md`).
//!
//! Flow: model each slide's lifecycle → cut it into fixed time blocks → build a
//! sensor event stream from the core's lowered chart → run the chart-time
//! tactic first and skip if it is already all-Perfect, else re-enumerate only
//! the failing / overlapping slides → verify against lnmai-core. A verified
//! result is cached as JSON and handed to the pad preview via
//! `--autoplay-tactic`.
//!
//! ```text
//! cargo run --bin autoplay_gen -- <chart> [--diff N] [--block S]
//!     [--max-tries N] [--preview] [--default-tactic] [--spec]
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
    let mut spec_events = false;
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
            "--spec" => spec_events = true,
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

    let engine = player::engine::JudgeEngine::load(&text, level).expect("engine");
    let spec = engine.chart_spec().expect("lowered chart");
    let song_end = spec
        .slides
        .iter()
        .map(|s| (s.start_timing + s.length) as f64 / 1e6)
        .fold(0.0_f64, f64::max) as f32;

    // ── Sanity: verify the core's own default tactic ───────────────────
    if default_tactic {
        let events = engine.default_tactic().unwrap_or_default();
        let units = plan(&mut plans, block_s);
        print_report(&chart_doc, &plans, &units, block_s);
        println!("\nevents: {} (core default tactic)", events.len());
        let result = verify::verify(&text, level, &events, song_end);
        print_verify(&result);
        finish(preview, &path, &chart_doc.title, level, &result);
        return;
    }

    // ── Spec-derived events (full zone sequence per runtime arc) ───────
    if spec_events {
        let events = verify::build_events_from_spec(&spec);
        let units = plan(&mut plans, block_s);
        print_report(&chart_doc, &plans, &units, block_s);
        println!(
            "\nevents: {} (spec-derived, {} slide arcs)",
            events.len(),
            spec.slides.len()
        );
        let result = verify::verify(&text, level, &events, song_end);
        print_verify(&result);
        finish(preview, &path, &chart_doc.title, level, &result);
        return;
    }

    // ── Baseline + localized enumeration over spec runtime arcs ────────
    let units = plan(&mut plans, block_s);
    print_report(&chart_doc, &plans, &units, block_s);

    let outcome = search::search(&text, level, &plans, &spec, song_end, max_tries);
    if outcome.baseline_perfect {
        println!(
            "\n== baseline ==\nchart-time tactic is already all-Perfect — enumeration skipped (tries {})",
            outcome.tries
        );
    } else {
        let moved = outcome
            .timings
            .iter()
            .filter(|t| t.offset_us != 0 || t.fast)
            .count();
        println!("\n== enumeration ==");
        println!(
            "tries: {}   work slides: {:?}   arcs re-timed: {}",
            outcome.tries, outcome.work_slides, moved
        );
    }
    print_verify(&outcome.final_verify);
    finish(preview, &path, &chart_doc.title, level, &outcome.final_verify);
}

fn print_verify(result: &VerifyResult) {
    println!("\n== core verification ==");
    println!(
        "arcs: {}   judged: {}   misses: {}   non-perfect: {}   unjudged: {}",
        result.arcs,
        result.judged,
        result.misses.len(),
        result.imperfect.len(),
        result.unjudged.len()
    );
    if !result.misses.is_empty() {
        println!("  missed arcs: {:?}", result.misses);
    }
    if !result.imperfect.is_empty() {
        println!("  non-perfect arcs: {:?}", result.imperfect);
    }
    if !result.grades.is_empty() {
        let list: Vec<String> = result
            .grades
            .iter()
            .map(|(rt, g)| format!("{rt}:{g:?}"))
            .collect();
        println!("  grades: {}", list.join("  "));
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

/// Cache a verified tactic and print the path plus a ready-to-run preview line.
fn finish(preview: bool, chart: &Path, title: &str, level: u32, result: &VerifyResult) {
    if !result.all_perfect() {
        println!("not all-Perfect — nothing cached, preview skipped");
        return;
    }
    let cached = match cache_tactic(chart, title, level, &result.events) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cache failed: {e}");
            return;
        }
    };
    println!("\n== cached ==");
    println!("tactic file : {}", cached.display());
    println!("run preview :");
    println!(
        "  cargo run --bin lambda_dx_pad_preview -- {} --autoplay-tactic {}",
        chart.display(),
        cached.display()
    );
    println!(
        "  (or export MAI2_AUTOPLAY_TACTIC={})",
        cached.display()
    );
    if preview {
        match launch_preview(chart, &cached) {
            Ok(pid) => println!("launched preview (pid {pid})"),
            Err(e) => eprintln!("preview failed: {e}"),
        }
    }
}

/// Write the event list to `out/autoplay_gen/<title>_lv<level>_<hash>.json`.
///
/// The hash is of the chart path: two charts can share title+level (e.g.
/// `サイエンス1/` and `サイエンス2/`), and their tactics must not overwrite
/// each other.
fn cache_tactic(
    chart: &Path,
    title: &str,
    level: u32,
    events: &[TimedInputEvent],
) -> Result<PathBuf, String> {
    use std::hash::{Hash, Hasher};

    let dir = PathBuf::from("out/autoplay_gen");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let slug: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let slug = slug.trim_matches('_');
    let slug = if slug.is_empty() { "chart" } else { slug };
    let key = chart.canonicalize().unwrap_or_else(|_| chart.to_path_buf());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.to_string_lossy().hash(&mut hasher);
    let tag = hasher.finish() & 0xffff_ffff;
    let path = dir.join(format!("{slug}_lv{level}_{tag:08x}.json"));
    let json = serde_json::to_string(events).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Spawn `lambda_dx_pad_preview` with the cached tactic.
fn launch_preview(chart: &Path, tactic: &Path) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("current exe has no parent dir")?;
    let preview = dir.join(format!("lambda_dx_pad_preview{}", std::env::consts::EXE_SUFFIX));
    if !preview.is_file() {
        return Err(format!(
            "{} not found — build it first: cargo build --bin lambda_dx_pad_preview",
            preview.display()
        ));
    }
    // Detach: the preview is its own window; don't inherit our stdio.
    let child = std::process::Command::new(&preview)
        .arg(chart)
        .arg("--autoplay-tactic")
        .arg(tactic)
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
                 [--preview] [--default-tactic] [--spec]

ARGS:
    CHART    Chart folder (maidata.txt) or file. Default: assets/charts/test.

OPTIONS:
    -d, --diff <N>     Difficulty number to load.
    --block <S>        Time-block length in seconds (default 0.1).
    --max-tries <N>    Cap on core evaluations during enumeration (default 5000).
    --default-tactic   Verify the core's own default tactic instead of enumerating.
    --spec             Build events from the core's lowered chart (full zone
                       sequence per runtime arc) and verify them.
    --preview          Launch the pad preview with the cached tactic, but only
                       if every judged arc verifies Perfect.
    -h, --help         Print this help.
";
