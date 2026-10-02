//! Minimal feasibility probe for the all-Perfect autoplay generator idea
//! (`docs/AUTOPLAY_GENERATOR.md`).
//!
//! It does **not** drive the core yet. It models each slide's lifecycle, cuts
//! it into fixed time blocks, greedily enumerates per-segment trigger times
//! under the document's rules, and reports the schedule plus potential
//! A-zone / non-ex tap-hold conflicts.
//!
//! ```text
//! cargo run --bin autoplay_gen -- <chart folder|file> [--diff N] [--block S]
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

use std::path::PathBuf;

use model::build_slide_plans;
use planner::{DEFAULT_BLOCK_S, plan};
use report::print_report;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chart: Option<PathBuf> = None;
    let mut diff: Option<i32> = None;
    let mut block_s = DEFAULT_BLOCK_S;

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
    let units = plan(&mut plans, block_s);
    print_report(&chart_doc, &plans, &units, block_s);
}

const HELP: &str = "\
autoplay_gen — minimal feasibility probe for the all-Perfect autoplay idea

USAGE:
    autoplay_gen [CHART] [--diff N] [--block SECONDS]

ARGS:
    CHART    Chart folder (maidata.txt) or file. Default: assets/charts/test.

OPTIONS:
    -d, --diff <N>     Difficulty number to load.
    --block <S>        Time-block length in seconds (default 0.1).
    -h, --help         Print this help.
";
