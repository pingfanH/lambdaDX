//! Preserve authored Simai, patching only cells touched by graphical edits.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use crate::app::types::{ChartDoc, Note, Slide, SlideSegment, SlideShape, secs_to_measure};
use super::source::{self, Kind, NoteSpan};

mod rhythm;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Document {
    pub text: String,
    pub chart: ChartDoc,
}

struct Cell {
    range: Range<usize>,
    comma: Option<usize>,
    measure: f32,
    division: String,
    notes: Vec<NoteSpan>,
}

struct Patch { range: Range<usize>, text: String }

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.00002_f32.max(a.abs().max(b.abs()) * f32::EPSILON * 2.0)
}

fn normalize_slide_segments(slide: &mut Slide) {
    // The waypoint editor stores a ring polyline in one segment; Simai stores
    // one segment per leg. Preserve every point, including its timing offset.
    if slide.segments.iter().any(|s| s.shape == SlideShape::Line && s.points.len() > 1
        && s.points.iter().all(|p| (1..=8).contains(&p.zone.to_id()))) {
        slide.segments = std::mem::take(&mut slide.segments).into_iter().flat_map(|segment| {
            if segment.shape == SlideShape::Line && segment.points.len() > 1
                && segment.points.iter().all(|p| (1..=8).contains(&p.zone.to_id())) {
                segment.points.into_iter().map(|point| SlideSegment {
                    shape: SlideShape::Line, points: vec![point],
                }).collect()
            } else { vec![segment] }
        }).collect();
    }
    // This is derived runtime-index mapping, not authored path data.
    slide.runtime_parts = slide.segments.len().max(1);
}

fn same_note(a: &Note, b: &Note, timing: bool) -> bool {
    if timing && !close(a.time, b.time) { return false; }
    if !close(a.hold_duration, b.hold_duration) || a.slide.len() != b.slide.len() { return false; }
    let mut a = a.clone();
    a.id = b.id;
    a.time = b.time;
    a.is_each = b.is_each;
    a.is_each_head = b.is_each_head;
    a.template_source = b.template_source.clone();
    a.hold_duration = b.hold_duration;
    if a.hi_speed == 0.0 { a.hi_speed = 1.0; }
    let mut b = b.clone();
    if b.hi_speed == 0.0 { b.hi_speed = 1.0; }
    for (left, right) in a.slide.iter_mut().zip(&mut b.slide) {
        if !close(left.slide_duration, right.slide_duration) || !close(left.slide_start_delay, right.slide_start_delay) { return false; }
        normalize_slide_segments(left);
        normalize_slide_segments(right);
        left.slide_duration = right.slide_duration;
        left.slide_start_delay = right.slide_start_delay;
        for (l, r) in left.segments.iter_mut().zip(&right.segments) {
            for (l, r) in l.points.iter_mut().zip(&r.points) {
                if close(l.beat_offset, r.beat_offset) { l.beat_offset = r.beat_offset; }
            }
        }
    }
    a == b
}

fn same_except_lane(a: &Note, b: &Note) -> bool {
    let mut a = a.clone();
    a.lane = b.lane;
    same_note(&a, b, false)
}

pub(crate) fn equivalent(a: &ChartDoc, b: &ChartDoc) -> bool {
    if a.notes.len() != b.notes.len() || a.bpms.len() != b.bpms.len() { return false; }
    if a.bpms.iter().zip(&b.bpms).any(|(a, b)| !close(a.measure, b.measure) || !close(a.bpm, b.bpm)) { return false; }
    let mut ordered: Vec<_> = b.notes.iter().collect();
    ordered.sort_by(|a, b| a.time.total_cmp(&b.time));
    let mut used = HashSet::new();
    a.notes.iter().all(|note| {
        let tolerance = 0.00002_f32.max(note.time.abs() * f32::EPSILON * 2.0);
        let start = ordered.partition_point(|n| n.time < note.time - tolerance);
        ordered.iter().enumerate().skip(start).take_while(|(_, n)| n.time <= note.time + tolerance)
            .find(|(i, n)| !used.contains(i) && same_note(note, n, true))
            .is_some_and(|(i, _)| used.insert(i))
    })
}

impl Document {
    /// External edits may patch existing cells, but never normalize the whole body.
    #[cfg(feature = "editor-automation")]
    pub fn update_strict(&self, chart: &ChartDoc) -> Result<String, String> {
        if equivalent(&self.chart, chart) { return Ok(self.text.clone()); }
        let text = self.patch(chart)?;
        if self.represents(&text, chart) { Ok(text) }
        else { Err("Cannot preserve the authored Simai for this edit".into()) }
    }
    pub fn update(&self, chart: &ChartDoc) -> Result<String, String> {
        if equivalent(&self.chart, chart) { return Ok(self.text.clone()); }
        let rhythm_changed = self.chart.notes.len() != chart.notes.len()
            || self.chart.notes.iter().zip(&chart.notes).any(|(a, b)| a.id != b.id || !close(a.time, b.time));
        if let Ok(text) = self.patch(chart) {
            if rhythm_changed {
                if let Ok(compacted) = rhythm::compact(&text) {
                    if self.represents(&compacted, chart) { return Ok(compacted); }
                }
            }
            if self.represents(&text, chart) { return Ok(text); }
        }
        // A new head can precede the first authored cell or follow E. If local
        // mapping cannot express it, rebuild the clock instead of freezing it.
        let text = crate::io::simai::editable_body(chart)?;
        let mut text = rhythm::compact(&text).ok()
            .filter(|compacted| self.represents(compacted, chart)).unwrap_or(text);
        for token in source::tokens(&self.text).into_iter().filter(|t| t.kind == Kind::Comment) {
            text.push_str(if self.text.contains("\r\n") { "\r\n" } else { "\n" });
            text.push_str(self.text[token.bytes].trim_end_matches('\r'));
        }
        if self.represents(&text, chart) { return Ok(text); }
        Err("This edit contains note data that Simai cannot represent; save as native JSON".into())
    }

    fn represents(&self, text: &str, chart: &ChartDoc) -> bool {
        crate::app::maidata::from_maidata_level(
            &format!("&first={}\n&wholebpm={}\n&inote_6={text}\n", chart.audio_offset, chart.bpm), 6,
        ).is_ok_and(|parsed| equivalent(chart, &parsed))
    }

    fn patch(&self, chart: &ChartDoc) -> Result<String, String> {
        let mapped = source::mapped_timeline(&self.text, &self.chart)?;
        let tokens = source::tokens(&self.text);
        let chars: Vec<_> = self.text.char_indices().map(|(i, _)| i).chain(std::iter::once(self.text.len())).collect();
        let bytes = |range: &Range<usize>| chars[range.start]..chars[range.end];
        let mut ranges = Vec::new();
        let mut start = 0;
        let mut division = "{4}".to_owned();
        for token in &tokens {
            let value = &self.text[token.bytes.clone()];
            if token.kind == Kind::Division { division = value.to_owned(); }
            if value == "," {
                ranges.push((start..token.bytes.start, Some(token.bytes.start), division.clone()));
                start = token.bytes.end;
            }
        }
        if ranges.len() < mapped.len() { ranges.push((start..self.text.len(), None, division)); }
        if ranges.len() != mapped.len() { return Err("Cannot map the original Simai cells".into()); }
        let cells: Vec<_> = ranges.into_iter().zip(mapped).map(|((range, comma, division), cell)| Cell {
            range, comma, division, measure: secs_to_measure(cell.time, &self.chart.bpms), notes: cell.notes,
        }).collect();
        if cells.is_empty() { return Err("The original Simai has no editable cells".into()); }
        let previous: HashMap<_, _> = self.chart.notes.iter().map(|n| (n.id, n)).collect();
        let current: HashMap<_, _> = chart.notes.iter().map(|n| (n.id, n)).collect();
        let clock_changed = self.chart.bpms != chart.bpms;
        let needs_clock_rewrite = |note: &Note| clock_changed
            && matches!(note.note_type, crate::app::types::NoteType::Hold | crate::app::types::NoteType::TouchHold | crate::app::types::NoteType::Slide);
        let mut raw = HashMap::new();
        for cell in &cells {
            for span in &cell.notes {
                if span.ids.is_empty() { return Err(format!("Cannot map original Simai note {}", span.text)); }
                if span.ids.len() == 1 && span.ids.iter().all(|id| current.get(id)
                    .is_some_and(|n| !needs_clock_rewrite(n) && previous.get(id).is_some_and(|old| same_note(old, n, false)))) {
                    raw.insert(span.ids[0], span.text.clone());
                } else if span.ids.len() == 1 {
                    let id = span.ids[0];
                    if let (Some(old), Some(note)) = (previous.get(&id), current.get(&id)) {
                        if !needs_clock_rewrite(note) && same_except_lane(old, note) && !crate::app::types::note_is_touch(old) {
                            let mut text = span.text.clone();
                            text.replace_range(..1, &note.lane.to_string());
                            raw.insert(id, text);
                        }
                    }
                }
            }
        }
        let mut groups: Vec<Vec<&Note>> = vec![Vec::new(); cells.len()];
        let mut extra = Vec::new();
        let owners: HashMap<_, _> = cells.iter().enumerate().flat_map(|(index, cell)|
            cell.notes.iter().flat_map(move |span| span.ids.iter().map(move |id| (*id, index)))).collect();
        for note in &chart.notes {
            // Nearby micro-rests may round to the same f32 measure. An
            // unchanged head still belongs to its bound source span.
            let owner = owners.get(&note.id).copied()
                .filter(|_| previous.get(&note.id).is_some_and(|old| old.time == note.time));
            let index = owner.or_else(|| cells.iter().enumerate()
                .filter(|(_, cell)| close(cell.measure, note.time))
                .min_by(|(_, a), (_, b)| (a.measure - note.time).abs().total_cmp(&(b.measure - note.time).abs()))
                .map(|(index, _)| index));
            if let Some(index) = index { groups[index].push(note); }
            else { extra.push(note); }
        }
        let mut trailing = Vec::new();
        if let Some(last) = cells.last().filter(|cell| cell.comma.is_none()) {
            let mut remaining = Vec::new();
            for note in extra {
                if note.time > last.measure { trailing.push((note.time, render(&[note], &raw, chart)?)); }
                else { remaining.push(note); }
            }
            extra = remaining;
        }
        let mut patches = Vec::new();
        for (index, cell) in cells.iter().enumerate() {
            let notes = &groups[index];
            if cell.comma.is_none() && !trailing.is_empty() {
                let terminator = tokens.iter().find(|token| cell.range.contains(&token.bytes.start)
                    && &self.text[token.bytes.clone()] == "E")
                    .ok_or("Extend an unterminated Simai body using the rhythm serializer")?;
                let mut events = trailing.clone();
                events.push((cell.measure, render(notes, &raw, chart)?));
                events.sort_by(|a, b| a.0.total_cmp(&b.0));
                let value = rhythm::events(&events, division_value(&cell.division), true)?;
                patches.push(Patch { range: terminator.bytes.clone(), text: value });
                continue;
            }
            let old_ids: HashSet<_> = cell.notes.iter().flat_map(|s| s.ids.iter().copied()).collect();
            let new_ids: HashSet<_> = notes.iter().map(|n| n.id).collect();
            let stable = old_ids.len() == new_ids.len() && old_ids.iter().all(|id| new_ids.contains(id));
            if stable {
                for span in &cell.notes {
                    let group: Vec<_> = span.ids.iter().filter_map(|id| notes.iter().find(|n| n.id == *id).copied()).collect();
                    if group.iter().all(|n| !needs_clock_rewrite(n) && previous.get(&n.id).is_some_and(|old| same_note(old, n, true))) { continue; }
                    let replacement = render(&group, &raw, chart)?;
                    let compact = span.chars.len() == 1 && (
                        cell.notes.iter().any(|other| other.chars.last().is_some_and(|r| r.end == span.chars[0].start))
                        || cell.notes.iter().any(|other| other.chars.first().is_some_and(|r| r.start == span.chars[0].end)));
                    let replacement = if compact && !replacement.bytes().all(|c| (b'1'..=b'8').contains(&c)) { format!("/{replacement}/") } else { replacement };
                    replace_span(&mut patches, span, &bytes, &replacement);
                }
            } else {
                for span in &cell.notes { replace_span(&mut patches, span, &bytes, ""); }
                for token in tokens.iter().filter(|t| cell.range.contains(&t.bytes.start)) {
                    if matches!(&self.text[token.bytes.clone()], "/" | "`") { patches.push(Patch { range: token.bytes.clone(), text: String::new() }); }
                }
                if !notes.is_empty() {
                    let at = cell.notes.first().and_then(|s| s.chars.first()).map(|r| chars[r.start])
                        .unwrap_or_else(|| insertion(&self.text, cell, &tokens));
                    // Join replacement and deletion at the same byte boundary.
                    let mut value = render(notes, &raw, chart)?;
                    if tokens.iter().any(|t| cell.range.contains(&t.bytes.start) && &self.text[t.bytes.clone()] == "E") {
                        value.push(',');
                    }
                    if let Some(patch) = patches.iter_mut().find(|p| p.range.start == at && !p.range.is_empty()) { patch.text = value; }
                    else { patches.push(Patch { range: at..at, text: value }); }
                }
            }
        }
        // Clock directives retain their cells. New clock events share the same
        // insertion path as notes, including off-grid additions.
        let mut seen_bpms = HashSet::new();
        for token in tokens.iter().filter(|t| t.kind == Kind::Bpm) {
            let index = cells.partition_point(|c| c.range.end < token.bytes.start).min(cells.len() - 1);
            let value = chart.bpms.iter().enumerate().find(|(_, b)| close(b.measure, cells[index].measure));
            let replacement = value.map(|(i, b)| {
                seen_bpms.insert(i);
                let original = &self.text[token.bytes.clone()];
                if original.trim_start_matches('(').trim_end_matches(')').parse::<f32>().is_ok_and(|old| close(old, b.bpm)) { original.to_owned() }
                else { format!("({})", number(b.bpm)) }
            }).unwrap_or_default();
            if replacement != self.text[token.bytes.clone()] { patches.push(Patch { range: token.bytes.clone(), text: replacement }); }
        }
        let mut events: Vec<(f32, String)> = extra.iter().map(|n| Ok((n.time, render(&[*n], &raw, chart)?))).collect::<Result<_, String>>()?;
        for (i, bpm) in chart.bpms.iter().enumerate() {
            if seen_bpms.contains(&i) || (i == 0 && close(bpm.measure, 1.0)) { continue; }
            let index = cells.partition_point(|cell| cell.measure < bpm.measure - 0.00002);
            let value = format!("({})", number(bpm.bpm));
            if let Some(cell) = cells.get(index).filter(|cell| close(cell.measure, bpm.measure)) {
                patches.push(Patch { range: cell.range.start..cell.range.start, text: value });
            } else { events.push((bpm.measure, value)); }
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut by_interval: Vec<Vec<(f32, String)>> = vec![Vec::new(); cells.len()];
        for (time, value) in events {
            let index = cells.partition_point(|cell| cell.measure < time).checked_sub(1).ok_or("Notes cannot precede the Simai start")?;
            by_interval[index].push((time, value));
        }
        for (index, events) in by_interval.into_iter().enumerate().filter(|(_, e)| !e.is_empty()) {
            let cell = &cells[index];
            let (directive, mut value) = advance(events[0].0 - cell.measure)
                .map_err(|error| format!("{error}: cell {} -> {}", cell.measure, events[0].0))?;
            let at = tokens.iter().filter(|t| cell.range.contains(&t.bytes.start)
                && matches!(t.kind, Kind::Bpm | Kind::Division | Kind::Speed))
                .map(|t| t.bytes.end).max().unwrap_or(cell.range.start);
            patches.push(Patch { range: at..at, text: directive });
            let mut cursor = 0;
            while cursor < events.len() {
                let time = events[cursor].0;
                let end = cursor + events[cursor..].partition_point(|e| close(e.0, time));
                let next = events.get(end).map(|e| e.0)
                    .or_else(|| cells.get(index + 1).map(|c| c.measure)).unwrap_or(time + 0.25);
                let (directive, rest) = advance(next - time)
                    .map_err(|error| format!("{error}: event {time} -> {next}"))?;
                value.push_str(&directive);
                let (directives, notes): (Vec<_>, Vec<_>) = events[cursor..end].iter().partition(|(_, s)| s.starts_with('('));
                for (_, text) in directives { value.push_str(text); }
                value.push_str(&notes.into_iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("/"));
                value.push_str(&rest);
                cursor = end;
            }
            if cells.get(index + 1).is_some() { value.push_str(&cell.division); }
            else { value.push('E'); }
            if let Some(comma) = cell.comma { patches.push(Patch { range: comma..comma + 1, text: value }); }
            else {
                if let Some(token) = tokens.iter().find(|t| cell.range.contains(&t.bytes.start) && &self.text[t.bytes.clone()] == "E") {
                    patches.push(Patch { range: token.bytes.clone(), text: String::new() });
                }
                let at = insertion(&self.text, cell, &tokens);
                patches.push(Patch { range: at..at, text: value });
            }
        }
        apply_patches(&self.text, patches)
    }
}

fn insertion(text: &str, cell: &Cell, tokens: &[source::Token]) -> usize {
    tokens.iter().filter(|t| cell.range.contains(&t.bytes.start))
        .find(|t| t.kind == Kind::Comment || &text[t.bytes.clone()] == "E")
        .map(|t| t.bytes.start).unwrap_or(cell.range.end)
}

fn replace_span(patches: &mut Vec<Patch>, span: &NoteSpan, bytes: &impl Fn(&Range<usize>) -> Range<usize>, value: &str) {
    for (i, range) in span.chars.iter().enumerate() {
        patches.push(Patch { range: bytes(range), text: if i == 0 { value.to_owned() } else { String::new() } });
    }
}

fn render(notes: &[&Note], raw: &HashMap<u64, String>, chart: &ChartDoc) -> Result<String, String> {
    notes.iter().map(|note| raw.get(&note.id).cloned().map(Ok).unwrap_or_else(|| note_text(note, chart)))
        .collect::<Result<Vec<_>, _>>().map(|notes| notes.join("/"))
}

fn number(value: f32) -> String {
    format!("{value:.9}").trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn division_value(value: &str) -> u64 {
    value.trim_matches(['{', '}']).parse().unwrap_or(4)
}

fn advance(gap: f32) -> Result<(String, String), String> {
    let tolerance = (gap.abs() as f64 * f32::EPSILON as f64 * 2.0).max(0.0000005);
    let steps = crate::io::simai::gap_steps(gap as f64, tolerance)?;
    let head = format!("{{{}}}", steps[0]);
    let mut tail = String::from(",");
    let mut active = steps[0];
    for divisor in steps.into_iter().skip(1) {
        if divisor != active { tail.push_str(&format!("{{{divisor}}}")); active = divisor; }
        tail.push(',');
    }
    Ok((head, tail))
}

fn apply_patches(text: &str, mut patches: Vec<Patch>) -> Result<String, String> {
    patches.sort_by(|a, b| a.range.start.cmp(&b.range.start).then(a.range.end.cmp(&b.range.end)));
    let mut result = String::new();
    let mut end = 0;
    for patch in patches {
        if patch.range.start < end { return Err("Overlapping Simai source edits; the original text has been kept".into()); }
        result.push_str(&text[end..patch.range.start]);
        result.push_str(&patch.text);
        end = patch.range.end;
    }
    result.push_str(&text[end..]);
    Ok(result)
}

fn fraction(duration: f32) -> Option<String> {
    if duration <= 0.0 { return None; }
    for denominator in 1..=192 {
        let numerator = (duration * denominator as f32).round();
        // Fraction recovery must be stricter than whole-note equivalence:
        // independently snapping wait and motion can exceed that budget.
        if numerator > 0.0 && (duration - numerator / denominator as f32).abs() <= 0.000001 {
            return Some(format!("{denominator}:{}", numerator as u64));
        }
    }
    None
}

/// Generated edits use readable measure fractions whenever the timing permits.
pub(crate) fn note_text(note: &Note, chart: &ChartDoc) -> Result<String, String> {
    let mut text = crate::core_chart::note_text(note, &chart.bpms)?;
    // Rendering's bpm_at deliberately anticipates nearby boundaries. Simai
    // durations must instead use the tempo at the actual note head.
    let bpm = chart.bpms.iter().rev().find(|b| b.measure <= note.time)
        .or_else(|| chart.bpms.first()).map_or(chart.bpm, |b| b.bpm);
    if matches!(note.note_type, crate::app::types::NoteType::Hold | crate::app::types::NoteType::TouchHold) {
        let seconds = crate::app::types::mdur_to_secs(note.hold_duration, note.time, &chart.bpms);
        if let Some(value) = fraction(seconds * bpm / 240.0) {
            if let (Some(start), Some(end)) = (text.find('['), text.find(']')) {
                text.replace_range(start..=end, &format!("[{value}]"));
            }
        }
    } else if note.note_type == crate::app::types::NoteType::Slide {
        let mut cursor = 0;
        for slide in &note.slide {
            if slide.segments.iter().all(|segment| segment.points.is_empty()
                || segment.points.last().is_some_and(|point| point.zone.to_id() == 17)) { continue; }
            let Some(start) = text[cursor..].find('[').map(|i| cursor + i) else { break; };
            let Some(end) = text[start..].find(']').map(|i| start + i) else { break; };
            let wait = crate::app::types::mdur_to_secs(slide.slide_start_delay, note.time, &chart.bpms);
            let total = crate::app::types::mdur_to_secs(slide.slide_duration, note.time, &chart.bpms);
            let motion = total - wait;
            let value = if close(wait, 60.0 / bpm) {
                fraction(motion * bpm / 240.0)
            } else if wait > 0.0 {
                let wait_bpm = 60.0 / wait;
                fraction(motion * wait_bpm / 240.0).map(|duration| format!("{}#{duration}", number(wait_bpm)))
            } else { None };
            if let Some(value) = value {
                let replacement = format!("[{value}]");
                text.replace_range(start..=end, &replacement);
                cursor = start + replacement.len();
            } else { cursor = end + 1; }
        }
    }
    Ok(text)
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;

    fn document(text: &str) -> Document {
        let mut chart = super::super::parse_draft(text, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        Document { text: text.to_owned(), chart }
    }

    fn changed(source: &Document, chart: &ChartDoc) -> String {
        let text = source.update(chart).unwrap();
        let parsed = super::super::parse_draft(&text, chart.bpm, 6).unwrap();
        assert!(equivalent(chart, &parsed), "{text}");
        text
    }

    #[test]
    fn unchanged_and_lane_edits_preserve_spelling() {
        let source = document("(120.00){4}1h[4:2], || preserved comment\n  3-5-8[4:1],C,E");
        assert_eq!(source.update(&source.chart).unwrap(), source.text);
        let mut chart = source.chart.clone();
        chart.notes[0].lane = 2;
        assert_eq!(changed(&source, &chart), source.text.replacen("1h", "2h", 1));
    }

    #[test]
    fn native_polyline_equivalence_preserves_every_leg_and_timing() {
        let source = document("(120){4}3-8-5[160#24:5],E");
        let mut native = source.chart.clone();
        let slide = &mut native.notes[0].slide[0];
        let last = slide.segments.pop().unwrap();
        slide.segments[0].points.extend(last.points);
        slide.runtime_parts = 1;
        assert!(equivalent(&native, &source.chart));
        assert!(equivalent(&source.chart, &native));
        for edit in 0..9 {
            let mut altered = native.clone();
            let slide = &mut altered.notes[0].slide[0];
            match edit {
                0 => { slide.segments[0].points.remove(0); }
                1 => slide.segments[0].points.reverse(),
                2 => slide.segments[0].points[0].zone = crate::app::types::zone::PadZone::A7,
                3 => slide.segments[0].shape = SlideShape::VShape,
                4 => slide.segments[0].points[0].beat_offset = 0.125,
                5 => slide.slide_start_delay += 0.125,
                6 => slide.slide_duration += 0.125,
                7 => slide.slide_is_break = !slide.slide_is_break,
                _ => slide.segments[0].points[0].zone = crate::app::types::zone::PadZone::C,
            }
            assert!(!equivalent(&altered, &source.chart), "edit {edit}");
            assert!(!equivalent(&source.chart, &altered), "edit {edit}, reversed");
        }
        assert_eq!(native.notes[0].slide[0].segments.len(), 1);
        assert_eq!(native.notes[0].slide[0].runtime_parts, 1);
    }

    #[test]
    fn bundled_jack_ripper_visual_edits_remain_representable() {
        let original = include_str!("../../../assets/charts/jack_ripper/maidata.txt");
        let level = *crate::app::maidata::inote_keys(original).last().unwrap();
        let mut chart = crate::app::maidata::from_maidata_level(original, level).unwrap();
        crate::app::maichart::assign_note_ids(&mut chart.notes);
        let source = Document {
            text: crate::io::simai::body_for_level(original, level).unwrap(), chart: chart.clone(),
        };
        let cells = source::mapped_timeline(&source.text, &source.chart).unwrap();
        assert!(cells.iter().flat_map(|cell| &cell.notes).all(|span| !span.ids.is_empty()),
            "all original note spans must map even beside very short rests");
        let hold = chart.notes.iter().position(|n| n.note_type == crate::app::types::NoteType::Hold).unwrap();
        let slide = chart.notes.iter().position(|n| n.note_type == crate::app::types::NoteType::Slide).unwrap();
        for edit in 0..9 {
            let mut edited = chart.clone();
            match edit {
                0 => edited.notes[0].lane = edited.notes[0].lane % 8 + 1,
                1 => edited.notes[0].time += 0.125,
                2 => edited.notes[hold].time += 0.125,
                3 => edited.notes[slide].time += 0.125,
                4 => edited.notes[hold].hold_duration += 0.125,
                5 => edited.notes[slide].slide[0].slide_duration += 0.125,
                6 | 7 => {
                    let time = if edit == 6 { 1.0 } else { edited.notes.last().unwrap().time + 2.0 };
                    edited.notes.push(Note { id: 10000, time, lane: 7, hi_speed: 1.0, ..Default::default() });
                }
                _ => { edited.notes.remove(hold); }
            }
            let text = source.update(&edited).unwrap_or_else(|error| panic!("edit {edit}: {error}"));
            assert!(source.represents(&text, &edited), "edit {edit}");
            // Generated source must remain editable after another operation.
            let next = Document { text, chart: edited.clone() };
            edited.notes[0].lane = edited.notes[0].lane % 8 + 1;
            let text = next.update(&edited).unwrap_or_else(|error| panic!("second edit after {edit}: {error}"));
            assert!(next.represents(&text, &edited), "second edit after {edit}");
        }
    }

    #[test]
    fn bundled_jack_ripper_canonical_durations_round_trip() {
        let original = include_str!("../../../assets/charts/jack_ripper/maidata.txt");
        let chart = crate::app::maidata::from_maidata_level(original, 5).unwrap();
        let source = Document { text: String::new(), chart: chart.clone() };
        let text = crate::io::simai::editable_body(&chart).unwrap();
        assert!(source.represents(&text, &chart));
    }

    #[test]
    fn holds_just_before_a_tempo_change_keep_the_start_tempo() {
        for note in ["8h[0.263157]", "Ch[0.263157]", "8-4[0.285714##0.263157]"] {
            let source = document(&format!("(210){{10001}}{note},(228),E"));
            let text = crate::io::simai::editable_body(&source.chart).unwrap();
            assert!(source.represents(&text, &source.chart), "{text}");
        }
    }

    #[test]
    fn inserts_deletes_and_moves_leave_other_cells_alone() {
        let source = document("(120){4}1h[4:2], || keep , braces [4:1]\n,3-5-8[4:1],C,E");
        let mut chart = source.chart.clone();
        chart.notes[0].time = 1.25;
        let moved = changed(&source, &chart);
        assert!(moved.contains("1h[4:2]|| keep , braces [4:1]\n,3-5-8[4:1],C,E"), "{moved}");
        chart.notes.remove(1);
        let deleted = changed(&source, &chart);
        assert!(deleted.contains("1h[4:2]|| keep , braces [4:1]\n"), "{deleted}");
        assert!(!deleted.contains("3-5-8"), "{deleted}");
        let mut note = chart.notes[0].clone();
        note.id = 999;
        note.time = 1.125;
        chart.notes.push(note);
        let inserted = changed(&source, &chart);
        assert!(inserted.contains("1h[4:2]|| keep , braces [4:1]\n"), "{inserted}");
    }

    #[test]
    fn off_grid_and_past_end_insertions_keep_old_notes() {
        let source = document("(120){4}1h[4:2],3,E");
        for time in [1.125, 1.3333334, 1.5, 3.0] {
            let mut chart = source.chart.clone();
            chart.notes.push(Note { id: 999, time, lane: 7, hi_speed: 1.0, ..Default::default() });
            let text = changed(&source, &chart);
            assert!(text.contains("1h[4:2]"), "{text}");
        }
    }

    #[test]
    fn compact_taps_can_be_changed_or_removed() {
        let source = document("(120){4}12,3,E");
        let mut chart = source.chart.clone();
        chart.notes[0].is_break = true;
        changed(&source, &chart);
        chart.notes.remove(0);
        assert!(changed(&source, &chart).contains("2,3,E"));
    }

    #[test]
    fn bpm_changes_and_shared_slide_spans_stay_valid() {
        let source = document("(120){4}1-5[4:1]*-7[4:1],2h[4:2],(180)3,E");
        let mut chart = source.chart.clone();
        chart.notes.last_mut().unwrap().lane = 4;
        let text = changed(&source, &chart);
        assert!(text.starts_with("(120){4}1-5[4:1]*-7[4:1],2h[4:2],(180)4"), "{text}");
        chart.bpms[0].bpm = 150.0;
        chart.bpm = 150.0;
        changed(&source, &chart);
    }
    #[test]
    fn moving_a_note_does_not_expand_the_source_into_empty_micro_cells() {
        let source = document("{4}(120){16}{32}{32}1,");
        let mut chart = source.chart.clone();
        chart.notes[0].time = 1.1875;
        let text = changed(&source, &chart);
        assert!(!text.contains("{64}"), "{text}");
        assert!(text.matches("{32}").count() <= 2, "{text}");
        assert!(text.len() < 50, "{text}");
    }

    #[test]
    fn moved_hold_and_slide_keep_readable_measure_durations() {
        let source = document("(120){4}1h[1:2],2-4[1:1],E");
        let mut chart = source.chart.clone();
        chart.notes[0].time = 1.5;
        chart.notes[1].time = 1.75;
        let text = changed(&source, &chart);
        assert!(text.contains("h[1:2]"), "{text}");
        assert!(text.contains("[1:1]"), "{text}");
        assert!(!text.contains("0.500000"), "{text}");
    }

    #[test]
    fn repeated_moves_stay_compact() {
        let mut source = document("{4}(120){16}{32}{32}1,");
        for step in 1..40 {
            let mut chart = source.chart.clone();
            chart.notes[0].time = 1.0 + (step % 17) as f32 / 32.0;
            let text = changed(&source, &chart);
            assert!(text.len() < 70, "step {step}: {text}");
            assert!(!text.contains("{32}{32}"), "{text}");
            source = Document { text, chart };
        }
    }

    #[test]
    fn additions_before_after_e_and_without_terminator_are_representable() {
        for text in ["(120){4},,1,E", "(120){4},,1,", "(120){4},,1", "E"] {
            let source = document(text);
            let mut chart = source.chart.clone();
            for (index, time) in [1.0, 1.125, 1.5, 1.625, 2.0, 4.125].into_iter().enumerate() {
                chart.notes.push(Note { id: 100 + index as u64, time, lane: 7, hi_speed: 1.0, ..Default::default() });
            }
            changed(&source, &chart);
        }
    }

    #[test]
    fn coarse_rests_do_not_change_implicit_slide_durations() {
        for body in [
            "(120){64}1w5,,,,,,,,,,,,,,,2,,,,,,,,,,,,,,,E",
            "(120){32}1-5,,,,,,,,,,,,,,,2,,,,,,,,,,,,,,,E",
        ] {
            let source = document(body);
            let mut chart = source.chart.clone();
            chart.notes.last_mut().unwrap().time += 1.0 / 64.0;
            let text = changed(&source, &chart);
            assert!(!text.contains(",,,,,,,,,"), "{text}");
        }
    }

    #[test]
    fn generated_and_resized_holds_and_slides_use_beat_fractions() {
        let source = document("(120){4}1h[4:1],2-4[0.500000##0.500000],E");
        let mut chart = source.chart.clone();
        chart.notes[0].hold_duration = 0.5;
        chart.notes[1].slide[0].slide_duration = 0.75;
        let text = changed(&source, &chart);
        assert!(text.contains("1h[2:1]"), "{text}");
        assert!(text.contains("2-4[2:1]"), "{text}");
        assert!(!text.contains("##"), "{text}");
        let mut slide = chart.notes[1].clone();
        slide.id = 100;
        slide.time = 3.0;
        chart.notes.push(slide);
        let text = changed(&source, &chart);
        assert!(text.contains("2-4[2:1]"), "{text}");
    }

    #[test]
    fn moves_across_bpm_and_speed_changes_keep_sound_timings() {
        let source = document("(120){4}1h[2:1],<H0.75>2-5[4:1],(180),<H1.25>3,E");
        let mut chart = source.chart.clone();
        chart.notes[0].time = 1.625;
        chart.notes[1].time = 2.25;
        chart.notes[2].time = 1.125;
        changed(&source, &chart);
    }

    #[test]
    fn multiple_simultaneous_notes_past_e_keep_their_owner() {
        let source = document("(120){4}1,E");
        let mut chart = source.chart.clone();
        for (id, time, lane) in [(10, 1.25, 2), (11, 1.5, 3), (12, 1.5, 4), (13, 2.0, 5)] {
            chart.notes.push(Note { id, time, lane, hi_speed: 1.0, ..Default::default() });
        }
        changed(&source, &chart);
    }

    #[test]
    fn off_grid_insertions_and_comments_have_a_valid_fallback() {
        let source = document("(188){4}1, || 注释 , [2:9]\r\n2h[2:9],3-5-8[8:5],E");
        let mut chart = source.chart.clone();
        for (index, time) in [1.12345, 1.3333334, 3.142857, 5.77777].into_iter().enumerate() {
            chart.notes.push(Note { id: 100 + index as u64, time, lane: 8, hi_speed: 1.0, ..Default::default() });
        }
        let text = changed(&source, &chart);
        assert!(text.contains("|| 注释 , [2:9]\r\n"), "{text}");
    }

    #[test]
    fn extending_the_body_keeps_trailing_comments() {
        for body in ["(120){4}1,E || keep tail", "(120){4}1 || keep tail", "(120){4}1, || keep tail"] {
            let source = document(body);
            let mut chart = source.chart.clone();
            chart.notes.push(Note { id: 100, time: 2.0, lane: 8, hi_speed: 1.0, ..Default::default() });
            let text = changed(&source, &chart);
            assert!(text.contains("|| keep tail"), "{text}");
        }
    }

    #[test]
    fn custom_slide_waits_and_zero_waits_keep_their_timings() {
        let source = document("(120){4}1-5[4:1],E");
        for (wait, motion) in [(0.125, 0.5), (0.5, 0.25), (0.0, 0.25), (0.25, 0.0)] {
            let mut chart = source.chart.clone();
            chart.notes[0].slide[0].slide_start_delay = wait;
            chart.notes[0].slide[0].slide_duration = wait + motion;
            changed(&source, &chart);
        }
    }

    #[test]
    fn editing_mixed_notation_repeatedly_keeps_the_parsed_document() {
        for body in [
            "(188){4}3h[1:2], || keep\n4,6h[2:9],4<5-8[8:5],E",
            "(120){16}12,3-5[4:1]*-7[4:1],Ch[2:1],B3,E",
            "(120){4}1,2-5[4:1],(180),,Ch[4:1],E",
        ] {
            let mut source = document(body);
            for index in 0..12 {
                let mut chart = source.chart.clone();
                let target = index % chart.notes.len();
                chart.notes[target].time = 1.0 + (index * 7 % 37) as f32 / 16.0;
                crate::app::maichart::recompute_each(&mut chart.notes);
                crate::app::maichart::mark_double_stars(&mut chart.notes);
                let text = changed(&source, &chart);
                assert!(text.len() < 2000, "{text}");
                source = Document { text, chart };
            }
        }
    }

}
