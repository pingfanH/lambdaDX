//! Human-readable report of the minimal autoplay plan.

use crate::app::types::{ChartDoc, NoteType, note_secs};
use crate::model::{SegmentPlan, SlidePlan};
use crate::planner::{UnitPlan, is_a_ring_segment};

/// Half-window (seconds) used to flag a slide segment whose trigger time may
/// collide with a **non-ex** tap/hold's great/good judgement.
const CONFLICT_WINDOW_S: f64 = 0.08;

pub fn print_report(chart: &ChartDoc, plans: &[SlidePlan], units: &[UnitPlan], block_s: f64) {
    println!("== minimal autoplay plan (docs/AUTOPLAY_GENERATOR.md) ==");
    println!("chart : {} — {}", chart.title, chart.artist);
    println!(
        "slides: {}   units: {}   block: {:.3}s",
        plans.len(),
        units.len(),
        block_s
    );

    // (note_id, time, is_ex) of every tap/hold, for the conflict check.
    let taps: Vec<(u64, f64, bool)> = chart
        .notes
        .iter()
        .filter(|n| matches!(n.note_type, NoteType::Tap | NoteType::Hold))
        .map(|n| (n.id, note_secs(n, &chart.bpms) as f64, n.is_ex))
        .collect();

    let mut flagged = 0usize;
    for (ui, unit) in units.iter().enumerate() {
        println!(
            "\nunit {ui}: [{:.3} .. {:.3}]s   slides={}",
            unit.start_s,
            unit.end_s,
            unit.slide_indices.len()
        );
        for &si in &unit.slide_indices {
            let p = &plans[si];
            println!(
                "  slide #{}.{}  head={:.3}s end={:.3}s dur={:.3}s blocks={} segs={}{}",
                p.note_id,
                p.slide_index,
                p.head_s,
                p.end_s,
                p.duration_s,
                p.block_count(block_s),
                p.segments.len(),
                if p.is_break { "  [break]" } else { "" }
            );
            for seg in &p.segments {
                let zones: Vec<String> = seg.zones.iter().map(|z| z.to_string()).collect();
                let note = conflict_note(seg, &taps);
                if note.is_some() {
                    flagged += 1;
                }
                println!(
                    "    seg{} {:<7} zones=[{}] target={:.3}s{}",
                    seg.index,
                    format!("{:?}", seg.shape),
                    zones.join(","),
                    seg.target_s,
                    note.map(|n| format!("   WARN non-ex tap/hold {n} in great/good window"))
                        .unwrap_or_default()
                );
            }
        }
    }

    println!("\npotential A-zone conflicts: {flagged}");
    if flagged > 0 {
        println!("(enumeration should shift these segment start times off the tap/hold window)");
    }
}

/// Describe a nearby non-ex tap/hold if this A-ring segment could collide.
fn conflict_note(seg: &SegmentPlan, taps: &[(u64, f64, bool)]) -> Option<String> {
    if !is_a_ring_segment(seg) {
        return None;
    }
    taps.iter()
        .filter(|(_, t, ex)| !*ex && (t - seg.target_s).abs() <= CONFLICT_WINDOW_S)
        .min_by(|a, b| (a.1 - seg.target_s).abs().total_cmp(&(b.1 - seg.target_s).abs()))
        .map(|(id, t, _)| format!("note{id}@{t:.3}s"))
}
