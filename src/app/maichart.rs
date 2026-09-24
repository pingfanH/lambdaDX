//! Loader for the "maichart" `chart.json` format used by `.maichart/<song>`
//! folders (a serialized simai AST).
//!
//! The format is JSON (sometimes with a UTF-8 BOM) shaped like:
//!
//! ```json
//! { "songName": "...", "composer": "...", "offset": 0,
//!   "notes": [ { "difficulty": 4, "level": "14.1", "bpmList": { "keyframes": [...] },
//!                "timeSignatureList": {...}, "taps": [...], "holds": [...],
//!                "touches": [...], "toucheHolds": [...], "slides": [...] } ] }
//! ```
//!
//! Positions/durations use a mixed-radix `TimePoint { split, beat }`:
//!
//! * an **absolute position** is `measure = 1 + beat / split` (so `{4,0}` is the
//!   first beat, `{8,43}` is measure `1 + 43/8`);
//! * a **duration** is `beat / split` **measures** (same unit as position, just
//!   without the +1). This is because a simai length `[divider:multiplier]`
//!   means `multiplier / divider` **whole notes**, and one whole note = one
//!   measure. E.g. `[8:17]` → `{split:8, beat:17}` = `17/8` measure = 8.5 beats.
//!
//! Cross-checked against the simai notation reference
//! (<https://w.atwiki.jp/simai/pages/1003.html>): a HOLD `[2:1]` is one half
//! note (= `1/2` measure), and a SLIDE waits **one beat at the current BPM**
//! before tracing — which is exactly the `prepareTime {split:4, beat:1}` this
//! format always stores (`1/4` measure = 1 beat).
//!
//! Slide shapes come from a single-character `fragment.type`:
//! `-` line, `<` left, `>` right, `p`/`q` P/Q, `s`/`z` S/Z, `v` V, `w` wifi.

use serde::Deserialize;

use super::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
};
use crate::app::types::zone::PadZone;

/// Asset path (relative to `assets/`) of the bundled default chart.
pub const DEFAULT_ASSET: &str = "charts/jack_ripper/chart.json";

// ---------------------------------------------------------------------------
// Deserialization structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiChart {
    song_name: String,
    #[serde(default)]
    composer: String,
    #[serde(default)]
    offset: f32,
    notes: Vec<MaiDifficulty>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiDifficulty {
    difficulty: i32,
    #[serde(default)]
    level: String,
    bpm_list: KeyframeList<BpmKeyframe>,
    #[serde(default)]
    taps: Vec<MaiTap>,
    #[serde(default)]
    holds: Vec<MaiHold>,
    #[serde(default)]
    touches: Vec<MaiTouch>,
    #[serde(default, rename = "toucheHolds")]
    touch_holds: Vec<MaiTouchHold>,
    #[serde(default)]
    slides: Vec<MaiSlide>,
}

#[derive(Debug, Deserialize)]
struct KeyframeList<T> {
    keyframes: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct BpmKeyframe {
    time: TimePoint,
    bpm: f32,
}

/// Mixed-radix position or duration: `beat` units of size `1/split`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
struct TimePoint {
    #[serde(default)]
    split: i32,
    #[serde(default)]
    beat: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiTap {
    hit_time: TimePoint,
    button: i32,
    #[serde(default)]
    is_break: bool,
    #[serde(default)]
    is_ex: bool,
    #[serde(default)]
    to_star: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiHold {
    hit_time: TimePoint,
    button: i32,
    hold_time: TimePoint,
    #[serde(default)]
    is_break: bool,
    #[serde(default)]
    is_ex: bool,
    #[serde(default)]
    to_star: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiTouch {
    hit_time: TimePoint,
    button: i32,
    #[serde(default)]
    is_break: bool,
    #[serde(default)]
    is_ex: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiTouchHold {
    hit_time: TimePoint,
    button: i32,
    hold_time: TimePoint,
    #[serde(default)]
    is_break: bool,
    #[serde(default)]
    is_ex: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiSlide {
    hit_time: TimePoint,
    button: i32,
    #[serde(default)]
    parts: Vec<MaiSlidePart>,
    #[serde(default)]
    is_break: bool,
    #[serde(default)]
    is_ex: bool,
    #[serde(default)]
    to_star: bool,
    /// "Hidden head": the slide has no tap star (tapless).
    #[serde(default)]
    hind_head: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiSlidePart {
    prepare_time: TimePoint,
    move_time: TimePoint,
    #[serde(default)]
    is_wifi: bool,
    #[serde(default)]
    fragments: Vec<MaiSlideFragment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaiSlideFragment {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    middle_button: i32,
    #[serde(default)]
    end_button: i32,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load the bundled default chart from `assets/charts/jack_ripper/chart.json`.
pub async fn load_default() -> Result<ChartDoc, String> {
    load_default_with_diff(None).await
}

/// Like [`load_default`] but with an explicit difficulty override.
pub async fn load_default_with_diff(diff: Option<i32>) -> Result<ChartDoc, String> {
    let bytes = super::platform::load_asset_bytes(DEFAULT_ASSET).await?;
    from_bytes_with_diff(&bytes, diff)
}

/// Parse a `chart.json` byte buffer into an internal [`ChartDoc`]. Difficulty is
/// taken from `MAICHART_DIFF` / the hardest chart.
pub fn from_bytes(bytes: &[u8]) -> Result<ChartDoc, String> {
    from_bytes_with_diff(bytes, None)
}

/// Like [`from_bytes`] but with an explicit difficulty override (CLI `--diff`).
pub fn from_bytes_with_diff(bytes: &[u8], diff: Option<i32>) -> Result<ChartDoc, String> {
    // The file may carry a UTF-8 BOM; serde_json rejects it.
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let mai: MaiChart = serde_json::from_str(text).map_err(|e| format!("parse maichart: {e}"))?;
    Ok(convert(mai, diff))
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

fn convert(mai: MaiChart, diff_override: Option<i32>) -> ChartDoc {
    let idx = select_difficulty(&mai.notes, diff_override);
    let diff = &mai.notes[idx];
    let bpms = build_bpms(diff);
    let bpm = bpms.first().map(|b| b.bpm).unwrap_or(120.0);

    let mut notes: Vec<Note> = Vec::new();

    for t in &diff.taps {
        if let Some(lane) = ring_lane(t.button) {
            notes.push(Note {
                time: measure_of(t.hit_time),
                lane,
                note_type: NoteType::Tap,
                is_break: t.is_break,
                is_ex: t.is_ex,
                is_star: t.to_star,
                ..Default::default()
            });
        }
    }

    for h in &diff.holds {
        if let Some(lane) = ring_lane(h.button) {
            notes.push(Note {
                time: measure_of(h.hit_time),
                lane,
                note_type: NoteType::Hold,
                hold_duration: duration_of(h.hold_time),
                is_break: h.is_break,
                is_ex: h.is_ex,
                is_star: h.to_star,
                ..Default::default()
            });
        }
    }

    for s in &diff.slides {
        if let Some(lane) = ring_lane(s.button) {
            notes.push(Note {
                time: measure_of(s.hit_time),
                lane,
                note_type: NoteType::Slide,
                is_break: s.is_break,
                is_ex: s.is_ex,
                // `is_star` means **double star** (multiple slides sharing one
                // head), which the renderer maps to the `star_double*` skins.
                // It is NOT "the slide has a star head". The multi-head pass
                // below sets it; default to false.
                is_star: false,
                is_tapless: s.hind_head,
                slide: build_slides(s, lane, s.is_break),
                ..Default::default()
            });
        }
    }

    // Touch sensors map onto the B ring (zones 9-16). This song has none, but
    // the mapping keeps the loader usable for other charts.
    for t in &diff.touches {
        if let Some(lane) = sensor_lane(t.button) {
            notes.push(Note {
                time: measure_of(t.hit_time),
                lane,
                note_type: NoteType::Touch,
                is_break: t.is_break,
                is_ex: t.is_ex,
                ..Default::default()
            });
        }
    }
    for h in &diff.touch_holds {
        if let Some(lane) = sensor_lane(h.button) {
            notes.push(Note {
                time: measure_of(h.hit_time),
                lane,
                note_type: NoteType::Hold,
                hold_duration: duration_of(h.hold_time),
                is_break: h.is_break,
                is_ex: h.is_ex,
                ..Default::default()
            });
        }
    }

    notes.sort_by(|a, b| a.time.total_cmp(&b.time));
    mark_double_stars(&mut notes);
    recompute_each(&mut notes);

    ChartDoc {
        version: "maichart-1".to_string(),
        title: mai.song_name,
        artist: mai.composer,
        simai_level: parse_level(&diff.level),
        bpm,
        bpms,
        audio_offset: mai.offset,
        notes,
        templates: Vec::new(),
        template_instances: Vec::new(),
    }
}

/// Mark "double star" slide heads.
///
/// A star head is *double* when two or more slides start from the same
/// `(time, lane)` (e.g. simai `1-3/1-5`). The renderer selects the
/// `star_double*` skins via `note.is_star`, so single slides must stay `false`
/// or every head would render as double.
pub(crate) fn mark_double_stars(notes: &mut [Note]) {
    use std::collections::HashMap;

    let key = |n: &Note| ((n.time * 100_000.0).round() as i64, n.lane);
    let mut counts: HashMap<(i64, u8), usize> = HashMap::new();
    for n in notes.iter().filter(|n| matches!(n.note_type, NoteType::Slide)) {
        *counts.entry(key(n)).or_insert(0) += 1;
    }
    for n in notes.iter_mut().filter(|n| matches!(n.note_type, NoteType::Slide)) {
        n.is_star = counts.get(&key(n)).copied().unwrap_or(0) > 1;
    }
}

/// Give every note a unique, non-zero id.
///
/// Simai-converted notes all default to `id == 0`; autoplay hides judged notes
/// by id, so without unique ids a single judgment would hide *every* note.
pub(crate) fn assign_note_ids(notes: &mut [Note]) {
    let mut next = notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
    for note in notes.iter_mut() {
        if note.id == 0 {
            note.id = next;
            next += 1;
        }
    }
}

/// Mark notes that share a hit time with another note (simai "each"). The
/// `is_each` flag selects the `*_each` skins, so a lone note stays `false`.
///
/// Slides' **trails** only pair with other slides (`is_each`), while a slide's
/// **head star** uses `is_each_head`, which follows the tap rule (any note at
/// the same time) — so a star lights up like a tap when anything lands with it.
pub(crate) fn recompute_each(notes: &mut [Note]) {
    let is_slide: Vec<bool> = notes
        .iter()
        .map(|n| matches!(n.note_type, NoteType::Slide))
        .collect();
    let times: Vec<f32> = notes.iter().map(|n| n.time).collect();
    for i in 0..notes.len() {
        let m = times[i];
        notes[i].is_each = times.iter().enumerate().any(|(j, t)| {
            j != i && (t - m).abs() < 0.002 && is_slide[j] == is_slide[i]
        });
        notes[i].is_each_head = times
            .iter()
            .enumerate()
            .any(|(j, t)| j != i && (t - m).abs() < 0.002);
    }
}

/// Choose which difficulty to load. An explicit override (CLI `--diff`) wins,
/// then `MAICHART_DIFF` (1-5), otherwise the hardest one is used.
fn select_difficulty<D: DifficultyLike>(notes: &[D], explicit: Option<i32>) -> usize {
    if notes.is_empty() {
        return 0;
    }
    let want = explicit.or_else(|| {
        std::env::var("MAICHART_DIFF")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
    });
    if let Some(want) = want {
        if let Some(i) = notes.iter().position(|n| n.difficulty() == want) {
            return i;
        }
    }
    // Hardest = largest `difficulty`.
    let mut best = 0;
    for (i, n) in notes.iter().enumerate() {
        if n.difficulty() > notes[best].difficulty() {
            best = i;
        }
    }
    best
}

/// Small trait so `select_difficulty` can stay generic and testable.
trait DifficultyLike {
    fn difficulty(&self) -> i32;
}
impl DifficultyLike for MaiDifficulty {
    fn difficulty(&self) -> i32 {
        self.difficulty
    }
}

/// Absolute measure position from a `{split, beat}` point: `1 + beat/split`.
fn measure_of(tp: TimePoint) -> f32 {
    if tp.split <= 0 {
        return 1.0;
    }
    1.0 + tp.beat as f32 / tp.split as f32
}

/// Duration in measures from a `{split, beat}` duration.
///
/// A simai length `[divider:multiplier]` is `multiplier / divider` **whole
/// notes**, and one whole note is one measure, so the duration is simply
/// `beat / split` measures — the same mapping as an absolute position without
/// the `+1`. (Verified: `[2:1]` = 1/2 measure, `[8:17]` = 17/8 measure, and the
/// default slide wait `{4,1}` = 1/4 measure = 1 beat.)
fn duration_of(tp: TimePoint) -> f32 {
    if tp.split <= 0 {
        return 0.0;
    }
    tp.beat as f32 / tp.split as f32
}

/// Build the BPM change list, sorted by measure.
fn build_bpms(diff: &MaiDifficulty) -> Vec<BpmChange> {
    let mut bpms: Vec<BpmChange> = diff
        .bpm_list
        .keyframes
        .iter()
        .map(|k| BpmChange {
            measure: measure_of(k.time),
            bpm: k.bpm,
        })
        .collect();
    bpms.sort_by(|a, b| a.measure.total_cmp(&b.measure));
    if bpms.is_empty() {
        bpms.push(BpmChange {
            measure: 1.0,
            bpm: 120.0,
        });
    }
    bpms
}

/// Build the [`Slide`]s for a slide note. One part becomes one `Slide`; each
/// fragment inside a part becomes a chained `SlideSegment`.
fn build_slides(
    s: &MaiSlide,
    _head_lane: u8,
    note_is_break: bool,
) -> Vec<Slide> {
    let mut out = Vec::new();
    for part in &s.parts {
        let delay = duration_of(part.prepare_time);
        let travel = duration_of(part.move_time);

        let mut segments = Vec::new();
        for frag in &part.fragments {
            let shape = shape_of(&frag.kind, part.is_wifi);
            let mut points = Vec::new();
            // A middle button (e.g. caret `^`) is an intermediate waypoint.
            if frag.middle_button >= 1 && frag.middle_button <= 8 {
                points.push(SlidePoint::from(PadZone::from(frag.middle_button as u8)));
            }
            if frag.end_button >= 1 && frag.end_button <= 8 {
                points.push(SlidePoint::from(PadZone::from(frag.end_button as u8)));
            }
            if points.is_empty() {
                continue;
            }
            segments.push(SlideSegment { points, shape });
        }
        if segments.is_empty() {
            continue;
        }

        out.push(Slide {
            segments,
            // `slide_duration` is the total span head→tail; `slide_start_delay`
            // is the pre-movement delay (both in measures).
            slide_duration: (delay + travel).max(0.0),
            slide_start_delay: delay.max(0.0),
            slide_is_break: note_is_break,
        });
    }
    out
}

/// Map a fragment type character to a [`SlideShape`].
fn shape_of(kind: &str, is_wifi: bool) -> SlideShape {
    if is_wifi {
        return SlideShape::Wifi;
    }
    match kind {
        "-" | "" => SlideShape::Line,
        "^" => SlideShape::Caret,
        "<" => SlideShape::Left,
        ">" => SlideShape::Right,
        "v" | "V" => SlideShape::VShape,
        "s" => SlideShape::S,
        "z" => SlideShape::Z,
        "p" => SlideShape::P,
        "q" => SlideShape::Q,
        "pp" => SlideShape::PP,
        "qq" => SlideShape::QQ,
        "w" => SlideShape::Wifi,
        _ => SlideShape::Line,
    }
}

/// A-ring button 1-8 → lane 1-8. Anything else is unsupported here.
fn ring_lane(button: i32) -> Option<u8> {
    (1..=8).contains(&button).then_some(button as u8)
}

/// Touch sensor 1-8 → B ring lanes 9-16.
fn sensor_lane(button: i32) -> Option<u8> {
    (1..=8).contains(&button).then_some((button + 8) as u8)
}

/// "14.1" → 14.
fn parse_level(level: &str) -> u32 {
    level
        .parse::<f32>()
        .ok()
        .map(|v| v as u32)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_radix_position_and_duration() {
        // Absolute: {4,0} is the first beat, {8,43} is 1 + 43/8.
        assert_eq!(measure_of(TimePoint { split: 4, beat: 0 }), 1.0);
        assert!((measure_of(TimePoint { split: 8, beat: 43 }) - (1.0 + 43.0 / 8.0)).abs() < 1e-6);
        assert!((measure_of(TimePoint { split: 8, beat: 413 }) - (1.0 + 413.0 / 8.0)).abs() < 1e-6);

        // Duration (measures = whole notes):
        // simai `[4:5]` = 5 quarter notes = 5/4 whole notes = 5/4 measure.
        assert!((duration_of(TimePoint { split: 4, beat: 5 }) - 5.0 / 4.0).abs() < 1e-6);
        // simai `[8:17]` = 17 eighth notes = 17/8 measure.
        assert!((duration_of(TimePoint { split: 8, beat: 17 }) - 17.0 / 8.0).abs() < 1e-6);
        // simai `[2:1]` = one half note = 1/2 measure.
        assert!((duration_of(TimePoint { split: 2, beat: 1 }) - 0.5).abs() < 1e-6);
        // The default slide wait is one beat = 1/4 measure.
        assert!((duration_of(TimePoint { split: 4, beat: 1 }) - 0.25).abs() < 1e-6);
        // A duration is a position without the +1.
        assert!(
            (duration_of(TimePoint { split: 8, beat: 43 }) - (measure_of(TimePoint { split: 8, beat: 43 }) - 1.0)).abs()
                < 1e-6
        );
    }

    #[test]
    fn shape_mapping() {
        assert_eq!(shape_of("-", false), SlideShape::Line);
        assert_eq!(shape_of("<", false), SlideShape::Left);
        assert_eq!(shape_of(">", false), SlideShape::Right);
        assert_eq!(shape_of("q", false), SlideShape::Q);
        assert_eq!(shape_of("p", false), SlideShape::P);
        assert_eq!(shape_of("s", false), SlideShape::S);
        assert_eq!(shape_of("z", false), SlideShape::Z);
        assert_eq!(shape_of("v", false), SlideShape::VShape);
        assert_eq!(shape_of("anything", true), SlideShape::Wifi);
    }

    #[test]
    fn bundled_maichart_converts() {
        let chart = from_bytes(include_bytes!("../../assets/charts/jack_ripper/chart.json"))
            .expect("bundled maichart must parse");
        assert!(chart.title.contains("Jack"), "title = {}", chart.title);
        assert_eq!(chart.bpm, 210.0);
        assert!(chart.notes.len() > 500, "notes = {}", chart.notes.len());
        assert!(chart.bpms.len() >= 3);
        assert!(chart.notes.iter().any(|n| matches!(n.note_type, NoteType::Slide)));
        assert!(chart.notes.iter().any(|n| matches!(n.note_type, NoteType::Hold)));
        // Every slide must carry at least one segment with an endpoint, and its
        // default wait must be one beat (1/4 measure). The tracing span must
        // exceed the wait.
        for n in chart.notes.iter().filter(|n| matches!(n.note_type, NoteType::Slide)) {
            assert!(!n.slide.is_empty());
            for s in &n.slide {
                assert!(s.segments.iter().all(|seg| !seg.points.is_empty()));
                assert!(
                    (s.slide_start_delay - 0.25).abs() < 1e-6,
                    "slide wait should be 1 beat, got {}",
                    s.slide_start_delay
                );
                assert!(s.slide_duration > s.slide_start_delay);
            }
        }
        // Holds must have a positive duration (the `/4` bug made them 4x short,
        // but still positive; assert on a known long-ish value's magnitude).
        assert!(
            chart
                .notes
                .iter()
                .filter(|n| matches!(n.note_type, NoteType::Hold))
                .all(|n| n.hold_duration > 0.0)
        );
        // This song has no multi-head slides, so no head should be "double".
        assert!(
            chart
                .notes
                .iter()
                .filter(|n| matches!(n.note_type, NoteType::Slide))
                .all(|n| !n.is_star),
            "single slides must not use double-star skins"
        );
    }

    #[test]
    fn shared_head_marks_double_star() {
        let mut notes = vec![
            Note {
                id: 1,
                time: 5.0,
                lane: 1,
                note_type: NoteType::Slide,
                ..Default::default()
            },
            Note {
                id: 2,
                time: 5.0,
                lane: 1,
                note_type: NoteType::Slide,
                ..Default::default()
            },
            Note {
                id: 3,
                time: 5.0,
                lane: 2,
                note_type: NoteType::Slide,
                ..Default::default()
            },
        ];
        mark_double_stars(&mut notes);
        assert!(notes[0].is_star && notes[1].is_star);
        assert!(!notes[2].is_star);
    }

    fn note(kind: NoteType, time: f32) -> Note {
        Note {
            time,
            lane: 1,
            note_type: kind,
            ..Default::default()
        }
    }

    #[test]
    fn slide_each_requires_another_slide() {
        // A slide sharing its time with a tap is *not* an each slide; two slides
        // at the same time are.
        let mut notes = vec![
            note(NoteType::Tap, 5.0),
            note(NoteType::Slide, 5.0),
            note(NoteType::Slide, 6.0),
            note(NoteType::Slide, 6.0),
        ];
        recompute_each(&mut notes);
        assert!(!notes[0].is_each, "lone tap");
        assert!(!notes[1].is_each, "slide must not inherit the tap's each");
        assert!(notes[2].is_each && notes[3].is_each, "slide/slide each");
    }

    #[test]
    fn slide_each_ignores_coincident_tap() {
        // A lone slide sharing a time with a lone tap: neither is each, but the
        // slide *head* follows the tap rule and lights up as each.
        let mut notes = vec![note(NoteType::Tap, 5.0), note(NoteType::Slide, 5.0)];
        recompute_each(&mut notes);
        assert!(!notes[0].is_each);
        assert!(!notes[1].is_each);
        assert!(notes[1].is_each_head, "slide head follows the tap each rule");
    }

    #[test]
    fn assign_note_ids_makes_zero_ids_unique() {
        let mut notes = vec![
            note(NoteType::Tap, 1.0),
            note(NoteType::Tap, 2.0),
            note(NoteType::Slide, 3.0),
        ];
        assert!(notes.iter().all(|n| n.id == 0));
        assign_note_ids(&mut notes);
        let ids: Vec<u64> = notes.iter().map(|n| n.id).collect();
        assert!(ids.iter().all(|&id| id != 0));
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
        // Existing non-zero ids are preserved.
        let mut notes = vec![Note { id: 42, ..note(NoteType::Tap, 1.0) }];
        assign_note_ids(&mut notes);
        assert_eq!(notes[0].id, 42);
    }
}
