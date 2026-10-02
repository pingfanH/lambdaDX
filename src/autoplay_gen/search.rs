//! Baseline-first, localized re-enumeration.
//!
//! 1. Verify the default (chart-time) tactic. If every arc is Perfect **and**
//!    has no A-zone conflict, stop — nothing to do.
//! 2. Otherwise collect the failing / conflicting chart slides and every slide
//!    whose lifecycle **overlaps** one of them; that is the working set.
//! 3. Re-time only the working set's runtime arcs, greedily accepting the
//!    `{offset × fast}` change that lexicographically reduces
//!    `(non-Perfect arcs, A-zone conflicts)`. New failures / conflicts pull more
//!    slides into the working set.
//!
//! A-zone conflicts are slide A-ring presses that fall inside a **non-ex**
//! tap/hold window (`docs/AUTOPLAY_GENERATOR.md`, "补充约束"). Bounded by
//! `max_tries` core evaluations.

use std::collections::BTreeSet;

use crate::core::types::ChartSpec;
use crate::model::SlidePlan;
use crate::verify::{self, ArcTiming, ConflictWindow, VerifyResult};

/// Offset granularity and half-range (in steps) explored per arc. 10 ms steps
/// are fine enough to land inside a Perfect window; ±12 steps is ±120 ms.
const OFFSET_STEP_US: i64 = 10_000;
const OFFSET_STEPS: i64 = 12;

pub struct SearchOutcome {
    /// The chart-time tactic was already all-Perfect with no A-zone conflict.
    pub baseline_perfect: bool,
    pub tries: usize,
    /// Chart-slide (plan) indices that were re-enumerated.
    pub work_slides: Vec<usize>,
    /// Final per-runtime-arc timing knobs.
    pub timings: Vec<ArcTiming>,
    /// Remaining A-zone conflicts (slide A-ring press inside a non-ex window).
    pub conflicts: usize,
    pub final_verify: VerifyResult,
}

pub fn search(
    text: &str,
    level: u32,
    plans: &[SlidePlan],
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
    if result.all_perfect() && conflicts == 0 {
        return SearchOutcome {
            baseline_perfect: true,
            tries,
            work_slides: Vec::new(),
            timings,
            conflicts,
            final_verify: result,
        };
    }

    // runtime arc -> chart slide (plan index).
    let arc_to_plan: Vec<Option<usize>> = {
        let mut v = vec![None; n_arcs];
        for (pi, p) in plans.iter().enumerate() {
            for rt in p.runtime_start..p.runtime_start + p.runtime_parts {
                if rt < v.len() {
                    v[rt] = Some(pi);
                }
            }
        }
        v
    };

    let mut work: BTreeSet<usize> = BTreeSet::new();
    loop {
        // Slides that are currently bad (Miss / non-Perfect) or in conflict.
        let mut failed_plans: BTreeSet<usize> = BTreeSet::new();
        for rt in result.misses.iter().chain(result.imperfect.iter()) {
            if let Some(Some(pi)) = arc_to_plan.get(*rt) {
                failed_plans.insert(*pi);
            }
        }
        for rt in 0..n_arcs {
            if conflict_arcs.get(rt).copied().unwrap_or(0) > 0 {
                if let Some(Some(pi)) = arc_to_plan.get(rt) {
                    failed_plans.insert(*pi);
                }
            }
        }
        if failed_plans.is_empty() {
            break;
        }
        for &pi in &failed_plans {
            work.insert(pi);
            let (h0, e0) = (plans[pi].head_s, plans[pi].end_s);
            for (qi, q) in plans.iter().enumerate() {
                if q.head_s < e0 && q.end_s > h0 {
                    work.insert(qi);
                }
            }
        }

        let work_arcs: Vec<usize> = work
            .iter()
            .flat_map(|&pi| {
                plans[pi].runtime_start..plans[pi].runtime_start + plans[pi].runtime_parts
            })
            .filter(|rt| *rt < n_arcs)
            .collect();

        // Greedy: find the single timing change that lexicographically reduces
        // `(bad arcs, A-zone conflicts)`. Both a plain offset and a fast-mode
        // variant are tried per arc.
        let baseline_score = (result.bad(), conflicts);
        let mut best: Option<(usize, ArcTiming, VerifyResult, Vec<usize>, (usize, usize))> = None;
        'search: for &rt in &work_arcs {
            for fast in [false, true] {
                for k in -OFFSET_STEPS..=OFFSET_STEPS {
                    let arc = ArcTiming {
                        offset_us: k * OFFSET_STEP_US,
                        fast,
                    };
                    if arc.offset_us == 0 && !arc.fast {
                        continue;
                    }
                    if tries >= max_tries {
                        break 'search;
                    }
                    let mut trial = timings.clone();
                    trial[rt] = arc;
                    let r = run(text, level, spec, &trial, song_end_s);
                    tries += 1;
                    let c = verify::zone_conflicts_by_arc(spec, &trial, windows);
                    let ct: usize = c.iter().sum();
                    let score = (r.bad(), ct);
                    if score < baseline_score && best.as_ref().is_none_or(|b| score < b.4) {
                        best = Some((rt, arc, r, c, score));
                    }
                }
            }
        }

        match best {
            Some((rt, arc, r, c, _score)) => {
                timings[rt] = arc;
                result = r;
                conflict_arcs = c;
                conflicts = conflict_arcs.iter().sum();
            }
            None => break, // no single change improves; give up
        }
    }

    SearchOutcome {
        baseline_perfect: false,
        tries,
        work_slides: work.into_iter().collect(),
        timings,
        conflicts,
        final_verify: result,
    }
}

fn run(text: &str, level: u32, spec: &ChartSpec, timings: &[ArcTiming], end_s: f32) -> VerifyResult {
    let events = verify::build_events_with_offsets(spec, timings);
    verify::verify(text, level, &events, end_s)
}
