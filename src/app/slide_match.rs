//! Phase 4: Slide trajectory template matching.
//!
//! Given a recorded slide's start lane and visited zone sequence, attempt to
//! classify it into one of the canonical [`super::types::SlideShape`] templates.
//!
//! Current scope (minimal v1): only `Line` (2-point) and `VShape` (3-point)
//! are auto-detected. Longer paths return `None` and are kept as a free-form
//! trajectory; the user can later assign a shape manually.
//!
//! The matcher only considers A-zones (1..=8). Sequences that touch B/C/D/E
//! zones are deferred to later phases.

use super::types::{SlidePoint, SlideShape};

/// Build the deduplicated visited zone sequence (zones with identical neighbors collapsed).
/// `start_lane` is treated as the first zone, then `slide_points` are appended.
fn build_visited(start_lane: u8, slide_points: &[SlidePoint]) -> Vec<u8> {
    let mut visited = vec![start_lane];
    for sp in slide_points {
        let zid = sp.zone.to_id();
        if visited.last() != Some(&zid) {
            visited.push(zid);
        }
    }
    visited
}

/// Whether a zone is in the A-ring (outer pad zones 1..=8).
fn is_a_zone(z: u8) -> bool {
    (1..=8).contains(&z)
}

/// Match a recorded slide to a canonical shape template.
///
/// Returns `Some(shape)` if the visited sequence matches a known template,
/// or `None` if no match is found (caller should keep the free-form trajectory).
pub(crate) fn match_slide_shape(start_lane: u8, slide_points: &[SlidePoint]) -> Option<SlideShape> {
    let visited = build_visited(start_lane, slide_points);

    // All visited zones must be A-zones for v1 (B/C/D/E shapes deferred).
    if !visited.iter().all(|z| is_a_zone(*z)) {
        return None;
    }

    match visited.as_slice() {
        // 2-point straight line between two distinct A-zones.
        [a, b] if a != b => Some(SlideShape::Line),

        // 3-point folded path → V-shape. The middle vertex must differ from
        // both endpoints; the endpoints must differ as well.
        [a, m, b] if a != m && m != b && a != b => Some(SlideShape::VShape),

        // Anything else is left unclassified for v1.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::app::types::zone::PadZone;
    use super::*;

    fn sp(zone: u8) -> SlidePoint {
        SlidePoint { zone:PadZone::from(zone), beat_offset: 0.0 }
    }

    #[test]
    fn line_two_points() {
        assert_eq!(match_slide_shape(1, &[sp(5)]), Some(SlideShape::Line));
        assert_eq!(match_slide_shape(3, &[sp(7)]), Some(SlideShape::Line));
    }

    #[test]
    fn vshape_three_points() {
        assert_eq!(
            match_slide_shape(1, &[sp(3), sp(5)]),
            Some(SlideShape::VShape),
        );
        assert_eq!(
            match_slide_shape(2, &[sp(4), sp(6)]),
            Some(SlideShape::VShape),
        );
    }

    #[test]
    fn dedup_collapses_repeats() {
        // Repeated start zone in slide_points should be deduplicated.
        assert_eq!(match_slide_shape(1, &[sp(1), sp(5)]), Some(SlideShape::Line));
        // Repeated middle should collapse to a 3-point V.
        assert_eq!(
            match_slide_shape(1, &[sp(3), sp(3), sp(5)]),
            Some(SlideShape::VShape),
        );
    }

    #[test]
    fn four_or_more_points_unclassified() {
        assert_eq!(
            match_slide_shape(1, &[sp(3), sp(5), sp(7)]),
            None,
        );
        assert_eq!(
            match_slide_shape(2, &[sp(4), sp(6), sp(8)]),
            None,
        );
    }

    #[test]
    fn touch_zone_unclassified() {
        // Zone 17 is the C ring center; not yet supported.
        assert_eq!(match_slide_shape(1, &[sp(17), sp(5)]), None);
        // Zone 9..=16 are touch zones (B/D/E rings).
        assert_eq!(match_slide_shape(9, &[sp(5)]), None);
    }

    #[test]
    fn degenerate_paths_unclassified() {
        // Single-point or zero-length sequences cannot form a shape.
        assert_eq!(match_slide_shape(1, &[]), None);
        // Same start and end with no via.
        assert_eq!(match_slide_shape(1, &[sp(1)]), None);
        // 3-point with collapsed mid (a == m) is treated as 2-point Line by dedup.
        assert_eq!(match_slide_shape(1, &[sp(1), sp(5)]), Some(SlideShape::Line));
    }
}
