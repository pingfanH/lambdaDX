//! Build the internal [`ChartDoc`] from `maidata.txt`.
//!
//! Parsing is done **entirely by the Lean backend** via the `lnmai-core` FFI
//! (`parse_frontend_chart`). There is no local Simai parser: this module only
//! maps the parser's token stream (times in microseconds, rational BPM/hi-speed,
//! slide bodies) onto the render model, converting the µs clock to the
//! measure-based clock the renderer uses through the parsed BPM table.
//!
//! Mapping mirrors the historical local converter:
//!
//! * `Tap`   → `NoteType::Tap`   (button slot + 1 = lane 1..=8)
//! * `Hold`  → `NoteType::Hold`  (duration in measures)
//! * `Slide` → `NoteType::Slide` (slide body kind → `SlideShape`, a continuous
//!   `>`/`<` chain stays one star with several segments)
//! * `Touch` / `TouchHold` → sensors mapped onto `PadZone` numbers
//!
//! Slide timing follows lnmai-core's resolved model: the pre-trace wait is the
//! parser's `starWait` (zero when absent) and the body length is the token's
//! length, defaulting to **one note increment** when the token has no `[n:m]`.

use crate::app::maichart::{mark_double_stars, recompute_each};
use crate::app::types::{
    BpmChange, ChartDoc, Note, NoteType, Slide, SlidePoint, SlideSegment, SlideShape,
    sdur_to_mdur, secs_to_measure,
};

/// Parse `maidata.txt` text and build a [`ChartDoc`].
///
/// `diff` is 1-based over the chart's difficulties in ascending `&inote_N=`
/// order (so `1` = easiest); `None` picks the hardest.
pub fn from_maidata(text: &str, diff: Option<i32>) -> Result<ChartDoc, String> {
    let key = select_level_key(text, diff)
        .ok_or_else(|| "maidata.txt has no &inote_N level".to_string())?;
    from_level(text, key)
}

/// The `&inote_N` key of the chart selected by `diff`.
///
/// `lnmai-core`'s `levelIndex` is this `N`, not the difficulty rating stored in
/// `ChartDoc::simai_level`.
pub fn inote_key(text: &str, diff: Option<i32>) -> Option<u32> {
    select_level_key(text, diff)
}

/// Build a [`ChartDoc`] for a specific `&inote_N` key (lnmai `levelIndex`).
pub fn from_maidata_level(text: &str, level_index: u32) -> Result<ChartDoc, String> {
    from_level(text, level_index)
}

/// Ascending `&inote_N=` keys present in `text`.
pub fn inote_keys(text: &str) -> Vec<u32> {
    let mut keys: Vec<u32> = Vec::new();
    for line in strip_bom(text).lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("&inote_") else {
            continue;
        };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(key) = digits.parse::<u32>() {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    keys.sort_unstable();
    keys
}

/// Lightweight `maidata.txt` metadata (no chart parsing).
#[derive(Debug, Clone, Default)]
pub struct MaidataMeta {
    pub title: String,
    pub artist: String,
    /// `&lv_N=` entries as `(N, level)`.
    pub levels: Vec<(u32, String)>,
    /// `&first=` audio offset in seconds.
    pub audio_offset: f32,
    /// Number of `&inote_N=` difficulty slots.
    pub chart_count: usize,
}

/// Scan `maidata.txt` header fields (`&title`, `&artist`, `&lv_N`, `&inote_N`).
pub fn metadata(text: &str) -> MaidataMeta {
    let mut meta = MaidataMeta::default();
    for line in strip_bom(text).lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('&') else {
            continue;
        };
        let Some((key, value)) = rest.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "title" {
            meta.title = value.to_string();
        } else if key == "artist" {
            meta.artist = value.to_string();
        } else if key == "first" {
            meta.audio_offset = value.trim().parse().unwrap_or(0.0);
        } else if let Some(digits) = key.strip_prefix("lv_") {
            if let Ok(slot) = digits.parse::<u32>() {
                meta.levels.push((slot, value.to_string()));
            }
        }
    }
    meta.levels.sort_by_key(|(k, _)| *k);
    meta.chart_count = inote_keys(text).len();
    meta
}


/// Pick the `&inote_N` key for `diff` (1-based, ascending; `None` = hardest).
fn select_level_key(text: &str, diff: Option<i32>) -> Option<u32> {
    let keys = inote_keys(text);
    match diff {
        Some(d) if d >= 1 => keys.get(d as usize - 1).copied(),
        _ => keys.last().copied(),
    }
}

/// Drop a leading UTF-8 BOM so the first `&field` line is scannable.
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

#[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
fn from_level(text: &str, level_index: u32) -> Result<ChartDoc, String> {
    crate::core::session::ensure_runtime()
        .map_err(|_| "lnmai-core runtime failed to initialize".to_string())?;
    let parsed = crate::core::api::parse_frontend_chart(text, level_index).map_err(|e| e.json)?;
    Ok(lean::convert(parsed, &metadata(text)))
}

#[cfg(not(any(feature = "backend-lean", feature = "backend-rust")))]
fn from_level(_text: &str, _level_index: u32) -> Result<ChartDoc, String> {
    Err("chart parsing requires the lnmai-core (Lean) backend".to_string())
}

/// One parsed Simai fragment (token) with its playback time in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct SimaiFragment {
    pub time: f32,
    pub text: String,
}

/// The raw Simai token stream for `level_index`, timed in seconds (same clock
/// as `app::types::note_secs`). Used to show which fragment is playing.
#[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
pub fn simai_timeline(text: &str, level_index: u32) -> Result<Vec<SimaiFragment>, String> {
    crate::core::session::ensure_runtime()
        .map_err(|_| "lnmai-core runtime failed to initialize".to_string())?;
    let parsed = crate::core::api::parse_frontend_chart(text, level_index).map_err(|e| e.json)?;
    Ok(lean::timeline(&parsed))
}

#[cfg(not(any(feature = "backend-lean", feature = "backend-rust")))]
pub fn simai_timeline(_text: &str, _level_index: u32) -> Result<Vec<SimaiFragment>, String> {
    Ok(Vec::new())
}

#[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
mod lean {
    use super::*;
    use crate::core::types::{
        FrontendChartResult, OuterSlot, RawNoteKind, Rational, SensorArea, SlideBodyKind,
        SourceEvent,
    };

    /// Map the parser's frontend result onto the render [`ChartDoc`].
    pub fn convert(parsed: FrontendChartResult, meta: &MaidataMeta) -> ChartDoc {
        let fields = &parsed.inspection.metadata.fields;
        // The parser reports most `&` fields but not `&title`, so header metadata
        // is scanned directly (it is plain key/value config, not chart syntax).
        let title = meta.title.clone();
        let artist = meta.artist.clone();

        let events = &parsed.inspection.source.events;
        let bpms = build_bpms(events);
        let bpm = bpms.first().map(|b| b.bpm).unwrap_or(120.0);

        let mut notes = build_notes(&parsed.inspection.tokens, &bpms);
        drop_slide_star_taps(&mut notes);
        notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        mark_double_stars(&mut notes);
        recompute_each(&mut notes);

        let level_index = parsed.inspection.chart.level_index;
        let simai_level = field(fields, &format!("lv_{level_index}"))
            .and_then(|v| v.trim().parse::<f32>().ok())
            .map(|v| v as u32)
            .unwrap_or(0);

        ChartDoc {
            version: "simai-1".to_string(),
            title,
            artist,
            simai_level,
            bpm,
            bpms,
            audio_offset: meta.audio_offset,
            notes,
            templates: Vec::new(),
            template_instances: Vec::new(),
        }
    }

    fn field(fields: &[(String, String)], key: &str) -> Option<String> {
        fields
            .iter()
            .find(|(k, _)| k.trim_start_matches('&') == key)
            .map(|(_, v)| v.clone())
    }

    /// Raw Simai tokens timed in seconds (same clock as the converted notes).
    pub(super) fn timeline(parsed: &FrontendChartResult) -> Vec<SimaiFragment> {
        let bpms = build_bpms(&parsed.inspection.source.events);
        parsed
            .inspection
            .tokens
            .iter()
            .map(|token| {
                let m = measure(token.timing, &bpms);
                SimaiFragment {
                    time: crate::app::types::measure_to_secs(m, &bpms),
                    text: token.raw_text.clone(),
                }
            })
            .collect()
    }

    fn rational_f32(r: &Rational) -> f32 {
        r.decimal.trim().parse().unwrap_or(1.0)
    }

    fn measure(t_us: i64, bpms: &[BpmChange]) -> f32 {
        secs_to_measure(t_us as f32 / 1e6, bpms)
    }

    fn duration(len_us: i64, start_us: i64, bpms: &[BpmChange]) -> f32 {
        sdur_to_mdur(len_us as f32 / 1e6, start_us as f32 / 1e6, bpms)
    }

    /// One note division in microseconds, matching lnmai-core's
    /// `noteTimingIncrement`: `bpmMeasureMicros / divisor` (a measure is
    /// `240_000_000 / bpm` µs). Used as the default slide body length when the
    /// token has no explicit `[n:m]`.
    fn note_increment_us(token: &crate::core::types::RawNoteToken) -> i64 {
        let bpm = rational_f32(&token.bpm);
        let divisor = token.divisor.max(1) as f32;
        if bpm > 0.0 {
            (240_000_000.0 / bpm / divisor) as i64
        } else {
            0
        }
    }

    /// Build the BPM table in measure space from the parser's source events.
    ///
    /// Events carry absolute microsecond timings and the BPM in effect; BPM
    /// changes take effect at the event. Measures are integrated forward from
    /// `t = 0` (measure 1.0) so the µs clock can later be inverted with
    /// `secs_to_measure`.
    fn build_bpms(events: &[SourceEvent]) -> Vec<BpmChange> {
        let mut bpms = vec![BpmChange {
            measure: 1.0,
            bpm: 120.0,
        }];
        let mut measure = 1.0_f32;
        let mut prev_t = 0.0_f32;
        let mut cur_bpm: Option<f32> = None;

        for event in events {
            let t = event.timing as f32 / 1e6;
            let bpm = rational_f32(&event.bpm);
            match cur_bpm {
                None => {
                    cur_bpm = Some(bpm);
                    bpms[0] = BpmChange { measure: 1.0, bpm };
                    measure += (t - prev_t) * bpm / 240.0;
                }
                Some(current) => {
                    measure += (t - prev_t) * current / 240.0;
                    if (bpm - current).abs() > 1e-6 {
                        bpms.push(BpmChange { measure, bpm });
                        cur_bpm = Some(bpm);
                    }
                }
            }
            prev_t = t;
        }

        bpms.sort_by(|a, b| a.measure.total_cmp(&b.measure));
        bpms
    }

    fn build_notes(tokens: &[crate::core::types::RawNoteToken], bpms: &[BpmChange]) -> Vec<Note> {
        let mut out = Vec::new();
        let mut group: Vec<usize> = Vec::new();

        for (index, token) in tokens.iter().enumerate() {
            if matches!(token.kind, RawNoteKind::Slide) {
                // A slide with an explicit `starWait` begins a new star; the
                // following headless tokens continue the same star.
                if token.star_wait.is_some() {
                    flush_slide(&group, tokens, bpms, &mut out);
                    group.clear();
                }
                group.push(index);
            } else {
                flush_slide(&group, tokens, bpms, &mut out);
                group.clear();
                if let Some(note) = simple_note(token, bpms) {
                    out.push(note);
                }
            }
        }
        flush_slide(&group, tokens, bpms, &mut out);
        out
    }

    fn flush_slide(
        group: &[usize],
        tokens: &[crate::core::types::RawNoteToken],
        bpms: &[BpmChange],
        out: &mut Vec<Note>,
    ) {
        if group.is_empty() {
            return;
        }
        if let Some(note) = slide_note(group, tokens, bpms) {
            out.push(note);
        }
    }

    fn simple_note(
        token: &crate::core::types::RawNoteToken,
        bpms: &[BpmChange],
    ) -> Option<Note> {
        let time = measure(token.timing, bpms);
        let hi_speed = rational_f32(&token.h_speed);
        match token.kind {
            RawNoteKind::Tap => Some(Note {
                time,
                lane: slot_lane(token.slot?),
                note_type: NoteType::Tap,
                is_break: token.is_break,
                is_ex: token.is_ex,
                hi_speed,
                ..Default::default()
            }),
            RawNoteKind::Hold => Some(Note {
                time,
                lane: slot_lane(token.slot?),
                note_type: NoteType::Hold,
                hold_duration: duration(token.length.unwrap_or(0), token.timing, bpms),
                is_break: token.is_break,
                is_ex: token.is_ex,
                hi_speed,
                ..Default::default()
            }),
            RawNoteKind::Touch => Some(Note {
                time,
                lane: sensor_lane(token.sensor_pos?)?,
                note_type: NoteType::Touch,
                is_break: token.is_break,
                hi_speed,
                ..Default::default()
            }),
            RawNoteKind::TouchHold => Some(Note {
                time,
                lane: sensor_lane(token.sensor_pos?)?,
                note_type: NoteType::Hold,
                hold_duration: duration(token.length.unwrap_or(0), token.timing, bpms),
                is_break: token.is_break,
                is_ex: token.is_ex,
                hi_speed,
                ..Default::default()
            }),
            RawNoteKind::Slide | RawNoteKind::Rest | RawNoteKind::Unknown => None,
        }
    }

    fn slide_note(
        group: &[usize],
        tokens: &[crate::core::types::RawNoteToken],
        bpms: &[BpmChange],
    ) -> Option<Note> {
        let head = tokens.get(*group.first()?)?;
        let time = measure(head.timing, bpms);
        // lnmai-core resolves a missing pre-trace wait to zero (a slide head is
        // reported with an explicit `starWait`, so this is a fallback only).
        let wait = head
            .star_wait
            .map(|w| duration(w, head.timing, bpms))
            .unwrap_or(0.0);
        let hi_speed = rational_f32(&head.h_speed);

        let mut segments = Vec::new();
        let mut travel = 0.0_f32;
        for &index in group {
            let token = tokens.get(index)?;
            let body = token.slide_body.as_ref()?;
            let end = sensor_lane(body.end_area?)?;
            let reflect = body.turn_area.and_then(sensor_lane);
            segments.push(SlideSegment {
                points: slide_points(body.kind, end, reflect),
                shape: kind_shape(body.kind),
            });
            // A slide without an explicit `[n:m]` has no `length`; lnmai-core
            // defaults it to one note increment (`bpmMeasureMicros / divisor`),
            // e.g. a bare `1w5` still traces for one division.
            let len = token
                .length
                .unwrap_or_else(|| note_increment_us(token));
            travel += duration(len, token.timing, bpms);
        }

        // Break can be signalled on any token of the chain (`is_break`) or as
        // the slide-specific `is_slide_break`; OR them so a break slide is not
        // silently rendered/judged as a normal one.
        let is_break = group.iter().any(|&index| {
            tokens
                .get(index)
                .map(|token| token.is_break || token.is_slide_break)
                .unwrap_or(false)
        });

        // One star per sub-slide: a continuous `>`/`<` chain expands to one
        // runtime slide per arc, recorded so `engine::chart_slide_key` maps the
        // runtime index back onto this note.
        let runtime_parts = segments.len().max(1);
        Some(Note {
            time,
            lane: slot_lane(head.slot?),
            note_type: NoteType::Slide,
            is_break,
            is_ex: head.is_ex,
            is_tapless: head.is_slide_no_head,
            hi_speed,
            slide: vec![Slide {
                segments,
                slide_duration: wait + travel,
                slide_start_delay: wait,
                slide_is_break: is_break,
                runtime_parts,
            }],
            ..Default::default()
        })
    }

    /// Waypoint zones for one slide arc (excluding the start; the renderer adds
    /// it). Matches the historical local parser: a `V` (turn) passes through its
    /// `turnArea`, a `v` through the center, everything else is a direct arc.
    fn slide_points(kind: SlideBodyKind, end: u8, reflect: Option<u8>) -> Vec<SlidePoint> {
        let sp = |z: u8| SlidePoint {
            zone: crate::app::types::zone::PadZone::from(z),
            beat_offset: 0.0,
        };
        match kind {
            SlideBodyKind::Turn => {
                let mut points = Vec::new();
                if let Some(area) = reflect {
                    points.push(sp(area));
                }
                points.push(sp(end));
                points
            }
            SlideBodyKind::V => vec![sp(17), sp(end)],
            _ => vec![sp(end)],
        }
    }

    fn kind_shape(kind: SlideBodyKind) -> SlideShape {
        match kind {
            SlideBodyKind::Line => SlideShape::Line,
            SlideBodyKind::CircleUp => SlideShape::Caret,
            SlideBodyKind::CircleLeft => SlideShape::Left,
            SlideBodyKind::CircleRight => SlideShape::Right,
            SlideBodyKind::V => SlideShape::VShape,
            SlideBodyKind::Turn => SlideShape::BigV,
            SlideBodyKind::P => SlideShape::P,
            SlideBodyKind::Q => SlideShape::Q,
            SlideBodyKind::Pp => SlideShape::PP,
            SlideBodyKind::Qq => SlideShape::QQ,
            SlideBodyKind::S => SlideShape::S,
            SlideBodyKind::Z => SlideShape::Z,
            SlideBodyKind::Wifi => SlideShape::Wifi,
        }
    }

    fn slot_lane(slot: OuterSlot) -> u8 {
        match slot {
            OuterSlot::S1 => 1,
            OuterSlot::S2 => 2,
            OuterSlot::S3 => 3,
            OuterSlot::S4 => 4,
            OuterSlot::S5 => 5,
            OuterSlot::S6 => 6,
            OuterSlot::S7 => 7,
            OuterSlot::S8 => 8,
        }
    }

    /// Touch sensor area → 1-based `PadZone` lane.
    /// A=1..8, B=9..16, C=17, D=18..25, E=26..33.
    fn sensor_lane(area: SensorArea) -> Option<u8> {
        use SensorArea::*;
        Some(match area {
            A1 => 1,
            A2 => 2,
            A3 => 3,
            A4 => 4,
            A5 => 5,
            A6 => 6,
            A7 => 7,
            A8 => 8,
            B1 => 9,
            B2 => 10,
            B3 => 11,
            B4 => 12,
            B5 => 13,
            B6 => 14,
            B7 => 15,
            B8 => 16,
            C => 17,
            D1 => 18,
            D2 => 19,
            D3 => 20,
            D4 => 21,
            D5 => 22,
            D6 => 23,
            D7 => 24,
            D8 => 25,
            E1 => 26,
            E2 => 27,
            E3 => 28,
            E4 => 29,
            E5 => 30,
            E6 => 31,
            E7 => 32,
            E8 => 33,
        })
    }
}

/// Drop star taps that only duplicate a slide head on the same
/// `(measure, lane)` (defensive: the parser no longer emits them).
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

#[cfg(test)]
mod tests {
    use super::{from_maidata, inote_key};

    #[test]
    fn parses_minimal_maidata() {
        let text = "&title=Demo\n&artist=Me\n&first=0\n&lv_2=3\n&inote_2=(120){4}1,2,1h[4:1],1-5[8:1],E\n";
        let c = from_maidata(text, None).expect("parse");
        assert_eq!(c.title, "Demo");
        assert_eq!(c.artist, "Me");
        assert!(c.notes.iter().any(|n| matches!(n.note_type, crate::app::types::NoteType::Hold)));
        assert!(c
            .notes
            .iter()
            .any(|n| matches!(n.note_type, crate::app::types::NoteType::Slide)));
    }

    #[test]
    fn diff_selects_by_position() {
        let text = "&title=D\n&inote_2=(120){4}1,\n&inote_5=(120){4}1,2,3,\n";
        assert_eq!(inote_key(text, Some(1)), Some(2));
        assert_eq!(inote_key(text, None), Some(5));
        let easy = from_maidata(text, Some(1)).unwrap();
        let hard = from_maidata(text, None).unwrap();
        assert!(easy.notes.len() < hard.notes.len());
    }

    #[test]
    fn continuous_chain_stays_one_sub_slide() {
        let text = "&title=D\n&inote_1=(120){4}1v4>3>2[4:1],E\n";
        let c = from_maidata(text, None).expect("parse");
        let n = c
            .notes
            .iter()
            .find(|n| matches!(n.note_type, crate::app::types::NoteType::Slide))
            .expect("slide");
        assert_eq!(n.slide.len(), 1, "one sub-slide for the whole chain");
        assert_eq!(n.slide[0].segments.len(), 3, "three chained segments");
        assert_eq!(n.slide[0].runtime_parts, 3, "lnmai splits it per arc");
    }

    /// The bundled `all_stars` test chart must exercise **every** baked slide
    /// prefab and every segment must map to one (no procedural fallback).
    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    #[test]
    fn bundled_all_stars_chart_covers_every_prefab_type() {
        let text = include_str!("../../assets/charts/all_stars/maidata.txt");
        let c = from_maidata(text, None).expect("parse all_stars");
        let mut keys = std::collections::BTreeSet::new();
        let mut missing = Vec::new();
        for n in c
            .notes
            .iter()
            .filter(|n| matches!(n.note_type, crate::app::types::NoteType::Slide))
        {
            for sl in &n.slide {
                let mut start = n.lane;
                for seg in &sl.segments {
                    let end = seg.points.last().map(|p| p.zone.to_id()).unwrap_or(start);
                    let turn = seg.points.first().map(|p| p.zone.to_id()).unwrap_or(0);
                    match crate::app::slide_svg::prefab_key(seg.shape, start, end, turn) {
                        Some(k) => {
                            keys.insert(k.name);
                        }
                        None => missing.push(format!("{:?} {start}->{end} t{turn}", seg.shape)),
                    }
                    if let Some(last) = seg.points.last() {
                        start = last.zone.to_id();
                    }
                }
            }
        }
        assert!(missing.is_empty(), "unmapped segments: {missing:?}");
        let expected: std::collections::BTreeSet<String> = crate::app::slide_svg::defs()
            .iter_names()
            .map(str::to_string)
            .collect();
        let uncovered: Vec<&String> = expected.difference(&keys).collect();
        assert!(uncovered.is_empty(), "chart misses prefab types: {uncovered:?}");
    }

    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    #[test]
    fn slide_without_timing_defaults_body_to_one_note_increment() {
        // A bare `1w5` has `starWait` but no `length`; lnmai-core traces the
        // body for one division (`bpmMeasureMicros / divisor`), which the local
        // converter must mirror instead of collapsing the body to zero.
        let text = "&title=D\n&inote_1=(120){8}1w5,E\n";
        let c = from_maidata(text, None).expect("parse");
        let n = c
            .notes
            .iter()
            .find(|n| matches!(n.note_type, crate::app::types::NoteType::Slide))
            .expect("slide");
        let sl = &n.slide[0];
        // 120 BPM, {8}: wait 0.5s = 0.25 measure, body 0.25s = 0.125 measure.
        assert!((sl.slide_start_delay - 0.25).abs() < 1e-5, "{sl:?}");
        assert!((sl.slide_duration - 0.375).abs() < 1e-5, "{sl:?}");
        assert!(sl.slide_duration > sl.slide_start_delay);

        // Sanity: the total span matches the core's resolved `start + length`.
        crate::core::session::ensure_runtime().unwrap();
        let parsed = crate::core::api::parse_frontend_chart(text, 1).expect("core");
        let core = parsed.semantic.normalized.slides.first().expect("core slide");
        let core_end_s = (core.start_timing + core.length) as f32 / 1e6;
        let core_head_s = core.head_timing as f32 / 1e6;
        let core_span = crate::app::types::secs_to_measure(core_end_s, &c.bpms)
            - crate::app::types::secs_to_measure(core_head_s, &c.bpms);
        assert!((sl.slide_duration - core_span).abs() < 1e-4, "{sl:?} vs {core_span}");
    }
}
