//! Minimal greedy planner for the all-Perfect autoplay idea
//! (`docs/AUTOPLAY_GENERATOR.md`).
//!
//! It groups overlapping slides into **units**, cuts each lifecycle into fixed
//! **time blocks**, and assigns each ordered segment to a block. When only one
//! block is left, every remaining segment shares it ("一起划完").

use crate::model::{SegmentPlan, SlidePlan};

/// Default time-block length, in seconds.
pub const DEFAULT_BLOCK_S: f64 = 0.1;

/// A set of slides whose lifecycles overlap; planned together.
#[derive(Debug, Clone)]
pub struct UnitPlan {
    /// Indices into the `Vec<SlidePlan>` they were built from.
    pub slide_indices: Vec<usize>,
    pub start_s: f64,
    pub end_s: f64,
}

/// Group per-slide plans into overlapping units (greedy, by head time).
pub fn group_units(plans: &[SlidePlan]) -> Vec<UnitPlan> {
    let mut units: Vec<UnitPlan> = Vec::new();
    for (i, p) in plans.iter().enumerate() {
        match units.last_mut() {
            Some(u) if p.head_s < u.end_s => {
                u.slide_indices.push(i);
                u.end_s = u.end_s.max(p.end_s);
            }
            _ => units.push(UnitPlan {
                slide_indices: vec![i],
                start_s: p.head_s,
                end_s: p.end_s,
            }),
        }
    }
    units
}

/// Assign a trigger time to every segment of every slide.
pub fn plan(plans: &mut [SlidePlan], block_s: f64) -> Vec<UnitPlan> {
    let units = group_units(plans);
    for unit in &units {
        for &si in &unit.slide_indices {
            assign_slide(&mut plans[si], block_s);
        }
    }
    units
}

fn assign_slide(slide: &mut SlidePlan, block_s: f64) {
    let n = slide.block_count(block_s);
    let last_t = slide.head_s + (n - 1) as f64 * block_s;
    for (i, seg) in slide.segments.iter_mut().enumerate() {
        let block = i.min(n - 1);
        let blocks_left = n - block;
        // Remaining segments with only one block left all share that block.
        let t = if blocks_left <= 1 {
            last_t
        } else {
            slide.head_s + block as f64 * block_s
        };
        seg.target_s = t;
    }
}

/// Segments whose first zone is on the A ring (id 1..=8) — the ones the doc
/// calls out for the tap/hold conflict check.
pub fn is_a_ring_segment(seg: &SegmentPlan) -> bool {
    seg.zones.first().is_some_and(|z| (1..=8).contains(z))
}
