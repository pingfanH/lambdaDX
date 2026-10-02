//! Data model for the minimal autoplay plan.
//!
//! Mirrors the vocabulary in `docs/AUTOPLAY_GENERATOR.md`:
//! a slide has a **lifecycle** (head → tail) split into ordered **judge
//! segments** (partitions), and its duration is cut into fixed **time blocks**.

use crate::app::types::{ChartDoc, NoteType, SlideShape, mdur_to_secs, note_secs, slide_end_time};

/// One judge segment (partition) of a slide, with its assigned trigger time.
#[derive(Debug, Clone)]
pub struct SegmentPlan {
    pub index: usize,
    pub shape: SlideShape,
    /// Sensor zone ids the star crosses on this segment (path order).
    pub zones: Vec<u8>,
    /// Approximate share of the lifecycle spent on this segment.
    pub span_s: f64,
    /// Assigned trigger time (seconds); filled in by [`crate::planner`].
    pub target_s: f64,
}

/// One slide's lifecycle: head → tail, split into ordered segments.
#[derive(Debug, Clone)]
pub struct SlidePlan {
    pub note_id: u64,
    pub slide_index: usize,
    pub head_s: f64,
    pub end_s: f64,
    pub duration_s: f64,
    pub is_break: bool,
    pub segments: Vec<SegmentPlan>,
    /// First runtime (lnmai-core) slide index this sub-slide expands to.
    pub runtime_start: usize,
    /// How many runtime slides this sub-slide expands to (`runtime_parts`).
    pub runtime_parts: usize,
}

impl SlidePlan {
    /// Number of fixed time blocks the lifecycle spans (at least one).
    pub fn block_count(&self, block_s: f64) -> usize {
        if block_s <= 0.0 {
            return 1;
        }
        ((self.duration_s / block_s).floor() as usize).max(1)
    }
}

/// Build one plan per chart sub-slide, sorted by head time.
pub fn build_slide_plans(chart: &ChartDoc) -> Vec<SlidePlan> {
    let bpms = &chart.bpms;
    let mut out = Vec::new();
    let mut runtime_cursor = 0usize;
    for note in &chart.notes {
        if !matches!(note.note_type, NoteType::Slide) {
            continue;
        }
        let head_note_s = note_secs(note, bpms) as f64;
        let head_s = head_note_s;
        let end_s = (slide_end_time(note, bpms) as f64).max(head_s);
        for (si, slide) in note.slide.iter().enumerate() {
            let total_s = mdur_to_secs(slide.slide_duration, note.time, bpms).max(0.0) as f64;
            let n = slide.segments.len().max(1) as f64;
            let seg_span = total_s / n;
            let segments = slide
                .segments
                .iter()
                .enumerate()
                .map(|(gi, seg)| SegmentPlan {
                    index: gi,
                    shape: seg.shape,
                    zones: seg.points.iter().map(|p| p.zone.to_id()).collect(),
                    span_s: seg_span,
                    target_s: head_s,
                })
                .collect();
            let parts = slide.runtime_parts.max(1);
            out.push(SlidePlan {
                note_id: note.id,
                slide_index: si,
                head_s,
                end_s,
                duration_s: (end_s - head_s).max(0.0),
                is_break: note.is_break || slide.slide_is_break,
                segments,
                runtime_start: runtime_cursor,
                runtime_parts: parts,
            });
            runtime_cursor += parts;
        }
    }
    out.sort_by(|a, b| a.head_s.total_cmp(&b.head_s));
    out
}
