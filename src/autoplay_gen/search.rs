//! Backtracking enumeration over each slide's block timing.
//!
//! For every slide (in order) it tries each candidate start offset; a branch is
//! accepted when the slide's runtime arcs are judged non-Miss, then it is
//! committed and the search moves to the next slide. When a slide's candidates
//! are all exhausted it is recorded as failed and the search continues.
//!
//! This is deliberately brute-force and bounded by `max_tries`; the point is
//! feasibility, not speed.

use crate::core::types::TimedInputEvent;
use crate::model::SlidePlan;
use crate::player::engine::timed_input_tp;
use crate::planner::{assign_slide, candidate_offsets};
use crate::verify::{self, VerifyResult};

pub struct SearchOutcome {
    pub tries: usize,
    /// Plan indices for which no candidate offset avoided a Miss.
    pub failed: Vec<usize>,
    /// Verification of the committed tactic over the whole chart.
    pub final_verify: VerifyResult,
}

impl SearchOutcome {
    pub fn perfect(&self) -> bool {
        self.failed.is_empty() && self.final_verify.all_perfect()
    }
}

pub fn search(
    text: &str,
    level: u32,
    plans: &mut [SlidePlan],
    block_s: f64,
    max_tries: usize,
) -> SearchOutcome {
    let ranges: Vec<(usize, usize)> = plans
        .iter()
        .map(|p| (p.runtime_start, p.runtime_parts))
        .collect();
    let mut committed: Vec<TimedInputEvent> = Vec::new();
    let mut failed = Vec::new();
    let mut tries = 0usize;

    for (pi, slide) in plans.iter_mut().enumerate() {
        let (r0, rn) = ranges[pi];
        let range: Vec<usize> = (r0..r0 + rn).collect();
        let end_s = slide.end_s as f32 + 1.0;
        let mut chosen = None;

        for off in candidate_offsets(slide, block_s) {
            if tries >= max_tries {
                break;
            }
            assign_slide(slide, block_s, off);
            let mut all = committed.clone();
            all.extend(verify::build_events(std::slice::from_ref(slide), 0.05));
            all.sort_by_key(timed_input_tp);
            tries += 1;

            let result = verify::verify(text, level, &all, end_s);
            // A branch fails only if one of THIS slide's arcs missed; arcs of
            // not-yet-placed slides are expected to miss.
            if !result.misses.iter().any(|rt| range.contains(rt)) {
                committed = all;
                chosen = Some(off);
                break;
            }
        }
        if chosen.is_none() {
            failed.push(pi);
        }
    }

    let song_end = plans.iter().map(|p| p.end_s).fold(0.0_f64, f64::max) as f32;
    let final_verify = verify::verify(text, level, &committed, song_end);
    SearchOutcome {
        tries,
        failed,
        final_verify,
    }
}
