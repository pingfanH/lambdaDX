//! Convert parsed Simai `maidata.txt` into the internal [`ChartDoc`].
//!
//! Uses the vendored pure-Rust parser in `crate::simai` (`maisimai`, MIT). The
//! mapping mirrors the chart.json loader:
//!
//! * `SimaiNote::Tap`   → `NoteType::Tap`   (button + 1 = lane 1..=8)
//! * `SimaiNote::Hold`  → `NoteType::Hold`  (duration in measures)
//! * `SimaiNote::Slide` → `NoteType::Slide` (pattern → `SlideShape`, chained
//!   arcs → segments, `*` splits into separate sub-slides)
//! * `TouchTap` / `TouchHold` → sensors mapped onto `PadZone` numbers
//!
//! Slide timing follows the same rule as the chart.json loader / simai spec:
//! the tracing length is the travel, plus a **default one-beat (0.25 measure)
//! wait** unless an explicit `[delay##…]` delay is given. `is_star` (double
//! star) is decided by shared heads, exactly like the chart.json path.

use crate::app::maichart::{mark_double_stars, recompute_each};
use crate::app::types::zone::PadZone;
use crate::app::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
};
use crate::simai::{SimaiChart, SimaiFile, SimaiNote, SlidePattern};

/// Default pre-trace wait for a slide: one beat = 0.25 measure.
const SLIDE_DEFAULT_WAIT: f32 = 0.25;

/// Parse `maidata.txt` text and build a [`ChartDoc`].
///
/// `diff` is 1-based over the chart's difficulties in ascending `&inote_N=`
/// order (so `1` = easiest); `None` picks the hardest.
pub fn from_maidata(text: &str, diff: Option<i32>) -> Result<ChartDoc, String> {
    let file = crate::simai::parse_file(text).map_err(|e| e.to_string())?;
    Ok(convert(file, diff))
}

fn convert(file: SimaiFile, diff: Option<i32>) -> ChartDoc {
    let order = difficulty_order(&file.charts);
    let idx = select_chart(&order, diff);
    let key = file.charts.get(idx).map(|(k, _)| *k).unwrap_or(0);
    let chart: &SimaiChart = file
        .charts
        .get(idx)
        .map(|(_, c)| c)
        .expect("at least one chart");

    let bpms = build_bpms(chart);
    let bpm = bpms.first().map(|b| b.bpm).unwrap_or(120.0);

    let mut notes: Vec<Note> = chart.notes.iter().filter_map(convert_note).collect();
    drop_slide_star_taps(&mut notes);
    notes.sort_by(|a, b| a.time.total_cmp(&b.time));
    mark_double_stars(&mut notes);
    recompute_each(&mut notes);

    ChartDoc {
        version: "simai-1".to_string(),
        title: file.title,
        artist: file.artist,
        simai_level: file
            .levels
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, lv)| lv.parse::<f32>().ok())
            .map(|v| v as u32)
            .unwrap_or(0),
        bpm,
        bpms,
        audio_offset: file.first,
        notes,
        templates: Vec::new(),
        template_instances: Vec::new(),
    }
}

/// Indices of `charts` sorted by ascending `&inote_N=` key.
fn difficulty_order(charts: &[(u32, SimaiChart)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..charts.len()).collect();
    order.sort_by_key(|&i| charts[i].0);
    order
}

/// Pick a chart index. `diff` is 1-based over `order`; `None` = hardest.
fn select_chart(order: &[usize], diff: Option<i32>) -> usize {
    if order.is_empty() {
        return 0;
    }
    match diff {
        Some(d) if d >= 1 => order[(d as usize - 1).min(order.len() - 1)],
        _ => *order.last().unwrap(),
    }
}

/// Drop star taps that are only the auto-generated head of a slide on the same
/// `(measure, lane)`. The parser emits both a `Tap{is_star}` and a `Slide` for a
/// simai `1-5`; the slide renders its own head, so the duplicate tap is removed
/// (matching the original player's import).
fn drop_slide_star_taps(notes: &mut Vec<Note>) {
    use std::collections::HashSet;

    let key = |t: f32, lane: u8| ((t * 100_000.0).round() as i64, lane);
    let slides: HashSet<(i64, u8)> = notes
        .iter()
        .filter(|n| matches!(n.note_type, NoteType::Slide))
        .map(|n| key(n.time, n.lane))
        .collect();
    notes.retain(|n| {
        !(matches!(n.note_type, NoteType::Tap) && n.is_star && slides.contains(&key(n.time, n.lane)))
    });
}

fn build_bpms(chart: &SimaiChart) -> Vec<BpmChange> {
    let mut bpms: Vec<BpmChange> = chart
        .bpms
        .iter()
        .map(|b| BpmChange {
            measure: b.measure,
            bpm: b.bpm,
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

fn convert_note(n: &SimaiNote) -> Option<Note> {
    match n {
        SimaiNote::Tap {
            measure,
            button,
            is_break,
            is_ex,
            is_star,
            hi_speed,
        } => Some(Note {
            time: *measure,
            lane: button + 1,
            note_type: NoteType::Tap,
            is_break: *is_break,
            is_ex: *is_ex,
            is_star: *is_star,
            hi_speed: *hi_speed,
            ..Default::default()
        }),
        SimaiNote::Hold {
            measure,
            button,
            duration,
            is_ex,
            hi_speed,
        } => Some(Note {
            time: *measure,
            lane: button + 1,
            note_type: NoteType::Hold,
            hold_duration: *duration,
            is_ex: *is_ex,
            hi_speed: *hi_speed,
            ..Default::default()
        }),
        SimaiNote::TouchTap {
            measure,
            region,
            position,
            hi_speed,
            ..
        } => Some(Note {
            time: *measure,
            lane: sensor_lane(*region, *position)?,
            note_type: NoteType::Touch,
            hi_speed: *hi_speed,
            ..Default::default()
        }),
        SimaiNote::TouchHold {
            measure,
            region,
            position,
            duration,
            hi_speed,
            ..
        } => Some(Note {
            time: *measure,
            lane: sensor_lane(*region, *position)?,
            note_type: NoteType::Hold,
            hold_duration: *duration,
            hi_speed: *hi_speed,
            ..Default::default()
        }),
        SimaiNote::Slide {
            measure,
            start,
            end,
            pattern,
            reflect,
            duration,
            delay,
            delay_explicit,
            is_break,
            is_ex,
            is_tapless,
            chain,
            hi_speed,
        } => {
            // An explicit `[delay##…]` (even `0##…`) sets the wait directly;
            // otherwise use the simai default one-beat wait.
            let wait = if *delay_explicit {
                *delay
            } else {
                SLIDE_DEFAULT_WAIT
            };
            let slides = build_slides(
                *pattern,
                *end,
                *reflect,
                *duration,
                wait,
                *is_break,
                chain,
            );
            Some(Note {
                time: *measure,
                lane: start + 1,
                note_type: NoteType::Slide,
                is_break: *is_break,
                is_ex: *is_ex,
                is_star: false, // set later by `mark_double_stars`
                is_tapless: *is_tapless,
                hi_speed: *hi_speed,
                slide: slides,
                ..Default::default()
            })
        }
    }
}

/// Build the sub-slides for a simai slide. A `*` chain entry starts a new
/// sub-slide (multiple arrows from one head); other chain entries extend the
/// current one.
#[allow(clippy::too_many_arguments)]
fn build_slides(
    pattern: SlidePattern,
    end: u8,
    reflect: Option<u8>,
    travel: f32,
    wait: f32,
    is_break: bool,
    chain: &[(SlidePattern, u8, Option<u8>, bool)],
) -> Vec<Slide> {
    let seg = |start: u8, end: u8, pattern: SlidePattern, reflect: Option<u8>| SlideSegment {
        points: pattern_points(start, end, pattern, reflect),
        shape: shape_of(pattern),
    };

    let mut slides: Vec<Slide> = Vec::new();
    let mut segments: Vec<SlideSegment> = vec![seg(0, end, pattern, reflect)];
    let mut prev_end = end;

    for (cp, ce, cr, is_new_slide) in chain {
        if *is_new_slide {
            slides.push(Slide {
                segments: std::mem::take(&mut segments),
                slide_duration: wait + travel,
                slide_start_delay: wait,
                slide_is_break: is_break,
            });
        }
        segments.push(seg(prev_end, *ce, *cp, *cr));
        prev_end = *ce;
    }
    if !segments.is_empty() {
        slides.push(Slide {
            segments,
            slide_duration: wait + travel,
            slide_start_delay: wait,
            slide_is_break: is_break,
        });
    }
    slides
}

/// Waypoint zones for one slide arc (excluding the start; the renderer adds it).
/// Matches the original player's `simai_pattern_to_points`.
fn pattern_points(start: u8, end: u8, pattern: SlidePattern, reflect: Option<u8>) -> Vec<SlidePoint> {
    let sp = |z: u8| SlidePoint {
        zone: PadZone::from(z),
        beat_offset: 0.0,
    };
    let _ = start;
    match pattern {
        SlidePattern::BigV => {
            let mut pts = Vec::new();
            if let Some(r) = reflect {
                pts.push(sp(r + 1));
            }
            pts.push(sp(end + 1));
            pts
        }
        SlidePattern::LowerV => vec![sp(17), sp(end + 1)],
        _ => vec![sp(end + 1)],
    }
}

/// Simai slide pattern → renderer shape.
fn shape_of(p: SlidePattern) -> SlideShape {
    match p {
        SlidePattern::Line => SlideShape::Line,
        SlidePattern::Caret => SlideShape::Caret,
        SlidePattern::Left => SlideShape::Left,
        SlidePattern::Right => SlideShape::Right,
        SlidePattern::LowerV => SlideShape::VShape,
        SlidePattern::BigV => SlideShape::BigV,
        SlidePattern::S => SlideShape::S,
        SlidePattern::Z => SlideShape::Z,
        SlidePattern::P => SlideShape::P,
        SlidePattern::Q => SlideShape::Q,
        SlidePattern::PP => SlideShape::PP,
        SlidePattern::QQ => SlideShape::QQ,
        SlidePattern::Wifi => SlideShape::Wifi,
    }
}

/// Touch sensor region + position → 1-based `PadZone` lane.
/// A=1..8, B=9..16, C=17, D=18..25, E=26..33.
fn sensor_lane(region: char, position: u8) -> Option<u8> {
    let base = match region {
        'A' => 1,
        'B' => 9,
        'C' => 17,
        'D' => 18,
        'E' => 26,
        _ => return None,
    };
    if region == 'C' {
        Some(17)
    } else if position <= 7 {
        Some(base + position)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::from_maidata;

    #[test]
    fn simultaneous_notes_are_marked_each() {
        // `1/2` = two taps at the same time (simai "each"); `3` alone is not.
        let c = from_maidata("&title=T\n&inote_1=(120){4}1/2,3\n", None).expect("parse");
        let each: Vec<bool> = c
            .notes
            .iter()
            .filter(|n| matches!(n.note_type, crate::app::types::NoteType::Tap))
            .map(|n| n.is_each)
            .collect();
        assert_eq!(each, vec![true, true, false]);
    }

    #[test]
    fn parses_minimal_maidata() {
        let text = "&title=Demo\n&artist=Me\n&first=0\n&lv_2=3\n&inote_2=(120){4}1,2,1h[4:1],1-5[8:1],E\n";
        let c = from_maidata(text, None).expect("parse");
        assert_eq!(c.title, "Demo");
        assert_eq!(c.artist, "Me");
        assert_eq!(c.notes.len(), 4);
        assert!(c.notes.iter().any(|n| matches!(n.note_type, crate::app::types::NoteType::Hold)));
        let slide = c
            .notes
            .iter()
            .find(|n| matches!(n.note_type, crate::app::types::NoteType::Slide))
            .expect("slide");
        assert!(!slide.slide.is_empty());
        // Default one-beat wait + 1/8 measure travel.
        let sl = &slide.slide[0];
        assert!((sl.slide_start_delay - 0.25).abs() < 1e-4, "delay {}", sl.slide_start_delay);
        assert!((sl.slide_duration - (0.25 + 0.125)).abs() < 1e-4);
    }

    #[test]
    fn explicit_slide_delay_overrides_default() {
        let text = "&title=D\n&inote_2=(120){4}1-5[0.2##0.8],E\n";
        let c = from_maidata(text, None).expect("parse");
        let sl = &c.notes.iter().find(|n| n.slide.len() == 1).unwrap().slide[0];
        // 0.2 s @120bpm = 0.1 measure delay; 0.8 s = 0.4 measure travel.
        assert!((sl.slide_start_delay - 0.1).abs() < 1e-3);
        assert!((sl.slide_duration - 0.5).abs() < 1e-3);
    }

    #[test]
    fn zero_explicit_delay_is_kept() {
        // `4<6[0##0.24]` = left arc 4→6, no wait, 0.24 s travel.
        let text = "&title=D\n&inote_2=(120){4}4<6[0##0.24],E\n";
        let c = from_maidata(text, None).expect("parse");
        let n = c
            .notes
            .iter()
            .find(|n| matches!(n.note_type, crate::app::types::NoteType::Slide))
            .expect("slide");
        assert_eq!(n.lane, 4);
        let sl = &n.slide[0];
        assert!(sl.slide_start_delay.abs() < 1e-6, "delay {}", sl.slide_start_delay);
        // 0.24 s @120bpm = 0.12 measure travel.
        assert!((sl.slide_duration - 0.12).abs() < 1e-3, "dur {}", sl.slide_duration);
    }

    #[test]
    fn diff_selects_by_position() {
        let text = "&title=D\n&inote_2=(120){4}1,\n&inote_5=(120){4}1,2,3,\n";
        let easy = from_maidata(text, Some(1)).unwrap();
        let hard = from_maidata(text, None).unwrap();
        assert_eq!(easy.notes.len(), 1);
        assert_eq!(hard.notes.len(), 3);
    }
}
