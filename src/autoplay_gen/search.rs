//! Baseline-first, **per-unit** localized re-enumeration.
//!
//! Slides are grouped into overlapping **units** (`planner::group_units`). For
//! each unit (in chart order) the search re-times that unit's runtime arcs (and
//! the arcs of any lifecycle-overlapping neighbour) with the `{offset × fast}`
//! candidates, greedily reducing `(non-Perfect arcs, A-zone conflicts)` until it
//! reaches AP or runs out of improving candidates. Progress and per-unit AP
//! status are printed; the best tactic found is returned even when non-AP so the
//! caller can still cache the closest attempt.
//!
//! A-zone conflicts are slide A-ring presses inside a **non-ex** tap/hold window
//! (`docs/AUTOPLAY_GENERATOR.md`, "补充约束"). Bounded by `max_tries`.

use std::collections::BTreeSet;

use crate::core::types::ChartSpec;
use crate::model::SlidePlan;
use crate::planner::UnitPlan;
use crate::verify::{self, ArcTiming, ConflictWindow, VerifyResult};

/// Offset granularity and half-range (in steps) explored per arc. 10 ms steps
/// are fine enough to land inside a Perfect window; ±12 steps is ±120 ms.
const OFFSET_STEP_US: i64 = 10_000;
const OFFSET_STEPS: i64 = 12;

/// Print a progress line every this many core evaluations.
const PROGRESS_EVERY: usize = 200;

pub struct SearchOutcome {
    /// The chart-time tactic was already all-Perfect with no A-zone conflict
    /// (enumeration did nothing).
    pub baseline_perfect: bool,
    /// Final tactic is all judged arcs Perfect (A-zone conflicts ignored).
    pub ap: bool,
    pub tries: usize,
    /// Units that reached AP.
    pub units_ap: usize,
    pub units_total: usize,
    /// Chart-slide (plan) indices that were touched by the enumeration.
    pub work_slides: Vec<usize>,
    /// Final per-runtime-arc timing knobs.
    pub timings: Vec<ArcTiming>,
    /// Remaining A-zone conflicts (slide A-ring press inside a non-ex window).
    pub conflicts: usize,
    pub final_verify: VerifyResult,
}

/// Flush stdout so progress prints are visible immediately even when piped.
fn flush() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn arcs_of<'a>(plans: &[SlidePlan], indices: impl Iterator<Item = &'a usize>, n_arcs: usize) -> Vec<usize> {
    indices
        .flat_map(|&pi| {
            plans[pi].runtime_start..plans[pi].runtime_start + plans[pi].runtime_parts
        })
        .filter(|rt| *rt < n_arcs)
        .collect()
}

pub fn search(
    text: &str,
    level: u32,
    plans: &[SlidePlan],
    units: &[UnitPlan],
    spec: &ChartSpec,
    windows: &[ConflictWindow],
    song_end_s: f32,
    max_tries: usize,
) -> SearchOutcome {
    let n_arcs = spec.slides.len();
    let mut timings = vec![ArcTiming::default(); n_arcs];
    let mut tries = 0usize;

    let mut result = run(text, level, spec, &timings, song_end_s);
    tries += 1;
    let mut conflict_arcs = verify::zone_conflicts_by_arc(spec, &timings, windows);
    let mut conflicts: usize = conflict_arcs.iter().sum();
    let baseline_perfect = result.all_perfect() && conflicts == 0;

    println!(
        "[enum] baseline: bad {}  conflicts {}  units {}  (tries {})",
        result.bad(),
        conflicts,
        units.len(),
        tries
    );
    flush();

    let mut work_slides: BTreeSet<usize> = BTreeSet::new();
    let mut units_ap = 0usize;

    for (ui, unit) in units.iter().enumerate() {
        // Slides of this unit that are bad or in conflict.
        let mut failed: BTreeSet<usize> = BTreeSet::new();
        for &pi in &unit.slide_indices {
            let arcs: Vec<usize> = (plans[pi].runtime_start
                ..plans[pi].runtime_start + plans[pi].runtime_parts)
                .filter(|rt| *rt < n_arcs)
                .collect();
            let is_bad = arcs
                .iter()
                .any(|rt| result.misses.contains(rt) || result.imperfect.contains(rt));
            let is_conf = arcs
                .iter()
                .any(|rt| conflict_arcs.get(*rt).copied().unwrap_or(0) > 0);
            if is_bad || is_conf {
                failed.insert(pi);
            }
        }
        if failed.is_empty() {
            println!("[enum] unit {ui}: AP  (already clean, tries {tries})");
            flush();
            units_ap += 1;
            continue;
        }

        // Only the failing slides plus their lifecycle-overlapping neighbours
        // need re-timing.
        let mut work: BTreeSet<usize> = failed.clone();
        for &pi in &failed {
            let (h0, e0) = (plans[pi].head_s, plans[pi].end_s);
            for (qi, q) in plans.iter().enumerate() {
                if q.head_s < e0 && q.end_s > h0 {
                    work.insert(qi);
                }
            }
        }
        work_slides.extend(work.iter().copied());
        let work_arcs = arcs_of(plans, work.iter(), n_arcs);
        let unit_arcs = arcs_of(plans, unit.slide_indices.iter(), n_arcs);

        let start_tries = tries;
        loop {
            if tries >= max_tries {
                break;
            }
            let baseline_score = (result.bad(), conflicts);
            // (arc, branch, timing, result, arc conflicts, score)
            let mut best: Option<(usize, Option<usize>, ArcTiming, VerifyResult, usize, (usize, usize))> =
                None;
            'scan: for &rt in &work_arcs {
                let tracks = spec
                    .slides
                    .get(rt)
                    .map(|s| s.judge_queues.len())
                    .unwrap_or(1);
                // Whole-arc variants: offset × fast.
                let mut variants: Vec<(Option<usize>, ArcTiming)> = Vec::new();
                for fast in [false, true] {
                    for k in -OFFSET_STEPS..=OFFSET_STEPS {
                        let arc = ArcTiming {
                            offset_us: k * OFFSET_STEP_US,
                            fast,
                            ..Default::default()
                        };
                        if !arc.is_default() {
                            variants.push((None, arc));
                        }
                    }
                }
                // Multi-track (wifi): each branch is a separate star.
                if tracks > 1 {
                    for ti in 0..tracks.min(verify::MAX_TRACKS) {
                        for k in -OFFSET_STEPS..=OFFSET_STEPS {
                            if k == 0 {
                                continue;
                            }
                            let mut arc = timings[rt];
                            arc.track_offset_us[ti] = k * OFFSET_STEP_US;
                            if arc != timings[rt] {
                                variants.push((Some(ti), arc));
                            }
                        }
                    }
                }
                for (ti, arc) in variants {
                    if tries >= max_tries {
                        break 'scan;
                    }
                    let mut trial = timings.clone();
                    trial[rt] = arc;
                    let r = run(text, level, spec, &trial, song_end_s);
                    tries += 1;
                    // Only this arc's conflicts can change.
                    let c_rt = verify::zone_conflict_for_arc(spec, arc, rt, windows);
                    let ct = conflicts - conflict_arcs.get(rt).copied().unwrap_or(0) + c_rt;
                    let score = (r.bad(), ct);
                    if score < baseline_score && best.as_ref().is_none_or(|b| score < b.5) {
                        best = Some((rt, ti, arc, r, c_rt, score));
                    }
                    if tries % PROGRESS_EVERY == 0 {
                        println!(
                                "  [enum] unit {ui}: ...tries {tries}/{max_tries}  (best so far bad {}, conflicts {})",
                                baseline_score.0, baseline_score.1
                            );
                            flush();
                        }
                }
            }

            match best {
                Some((rt, ti, arc, r, c_rt, score)) => {
                    let old = conflict_arcs.get(rt).copied().unwrap_or(0);
                    timings[rt] = arc;
                    result = r;
                    conflict_arcs[rt] = c_rt;
                    conflicts = conflicts - old + c_rt;
                    match ti {
                        Some(t) => println!(
                            "  [enum] unit {ui}: arc {rt} branch {t} -> off={:+}us  bad {}->{}  conflicts {}->{}",
                            arc.track_offset_us[t], baseline_score.0, score.0, baseline_score.1,
                            conflicts
                        ),
                        None => println!(
                            "  [enum] unit {ui}: arc {rt} -> off={:+}us fast={}  bad {}->{}  conflicts {}->{}",
                            arc.offset_us, arc.fast, baseline_score.0, score.0, baseline_score.1,
                            conflicts
                        ),
                    }
                    flush();
                }
                None => break, // no single change improves; unit enumeration exhausted
            }
        }

        let unit_bad: Vec<usize> = result
            .misses
            .iter()
            .chain(result.imperfect.iter())
            .copied()
            .filter(|rt| unit_arcs.contains(rt))
            .collect();
        let unit_conf: usize = unit_arcs
            .iter()
            .map(|rt| conflict_arcs.get(*rt).copied().unwrap_or(0))
            .sum();
        if unit_bad.is_empty() && unit_conf == 0 {
            println!(
                "[enum] unit {ui}: AP  (tries used {}, total {tries})",
                tries - start_tries
            );
            units_ap += 1;
        } else {
            println!(
                "[enum] unit {ui}: not AP — bad arcs {unit_bad:?}, conflicts {unit_conf}  (tries used {}, total {tries})",
                tries - start_tries
            );
        }
        flush();
    }

    let ap = result.all_perfect();
    println!(
        "[enum] done: {} {}  bad {}  conflicts {}  units AP {}/{}  tries {}/{}",
        if ap { "AP" } else { "NOT AP" },
        if baseline_perfect { "(baseline)" } else { "" },
        result.bad(),
        conflicts,
        units_ap,
        units.len(),
        tries,
        max_tries
    );
    flush();

    SearchOutcome {
        baseline_perfect,
        ap,
        tries,
        units_ap,
        units_total: units.len(),
        work_slides: work_slides.into_iter().collect(),
        timings,
        conflicts,
        final_verify: result,
    }
}

fn run(text: &str, level: u32, spec: &ChartSpec, timings: &[ArcTiming], end_s: f32) -> VerifyResult {
    let events = verify::build_events_with_offsets(spec, timings);
    verify::verify(text, level, &events, end_s)
}
