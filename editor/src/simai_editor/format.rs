//! Preserve authored Simai, patching only cells touched by graphical edits.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use crate::app::types::{ChartDoc, Note, secs_to_measure};
use super::source::{self, Kind, NoteSpan};

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
    for (left, right) in a.slide.iter_mut().zip(&b.slide) {
        if !close(left.slide_duration, right.slide_duration) || !close(left.slide_start_delay, right.slide_start_delay) { return false; }
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

pub(super) fn equivalent(a: &ChartDoc, b: &ChartDoc) -> bool {
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
    pub fn update(&self, chart: &ChartDoc) -> Result<String, String> {
        if equivalent(&self.chart, chart) { return Ok(self.text.clone()); }
        let text = self.patch(chart)?;
        let parsed = crate::app::maidata::from_maidata_level(
            &format!("&first={}\n&wholebpm={}\n&inote_6={text}\n", self.chart.audio_offset, chart.bpm), 6,
        )?;
        if !equivalent(chart, &parsed) {
            return Err("Cannot safely preserve this Simai notation after the edit; the original text has been kept".into());
        }
        Ok(text)
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
        for note in &chart.notes {
            let index = cells.partition_point(|cell| cell.measure < note.time - 0.00002);
            if let Some(group) = groups.get_mut(index).filter(|_| close(cells[index].measure, note.time)) { group.push(note); }
            else { extra.push(note); }
        }
        let mut patches = Vec::new();
        for (index, cell) in cells.iter().enumerate() {
            let notes = &groups[index];
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
            let (directive, mut value) = advance(events[0].0 - cell.measure)?;
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
                let (directive, rest) = advance(next - time)?;
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

fn advance(gap: f32) -> Result<(String, String), String> {
    let tolerance = (gap.abs() as f64 * f32::EPSILON as f64 * 2.0).max(0.0000005);
    let steps = crate::io::simai::gap_steps(gap as f64, tolerance)?;
    let head = format!("{{{}}}", steps[0]);
    let mut tail = String::from(",");
    for divisor in steps.into_iter().skip(1) { tail.push_str(&format!("{{{divisor}}},")); }
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
        if numerator > 0.0 && close(duration, numerator / denominator as f32) {
            return Some(format!("{denominator}:{}", numerator as u64));
        }
    }
    None
}

/// Generated edits use readable measure fractions whenever the timing permits.
pub(crate) fn note_text(note: &Note, chart: &ChartDoc) -> Result<String, String> {
    let mut text = crate::core_chart::note_text(note, &chart.bpms)?;
    if matches!(note.note_type, crate::app::types::NoteType::Hold | crate::app::types::NoteType::TouchHold) {
        let crosses_bpm = chart.bpms.iter().skip(1).any(|b| b.measure > note.time && b.measure < note.time + note.hold_duration);
        if let Some(value) = fraction(note.hold_duration).filter(|_| !crosses_bpm) {
            if let Some(start) = text.find('[') { text.replace_range(start..text.len(), &format!("[{value}]")); }
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
        let text = source.update(chart).unwrap_or_else(|error| {
            let patched = source.patch(chart);
            panic!("{error}\npatched={patched:?}\nexpected={:?}\nparsed={:?}", chart.notes, patched.as_ref().ok().map(|text| super::super::parse_draft(text, chart.bpm, 6)));
        });
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
    fn inserts_deletes_and_moves_leave_other_cells_alone() {
        let source = document("(120){4}1h[4:2], || keep , braces [4:1]\n,3-5-8[4:1],C,E");
        let mut chart = source.chart.clone();
        chart.notes[0].time = 1.25;
        let moved = changed(&source, &chart);
        assert!(moved.contains("1h[4:2]|| keep , braces [4:1]\n,3-5-8[4:1],C,E"), "{moved}");
        chart.notes.remove(1);
        let deleted = changed(&source, &chart);
        assert!(deleted.contains("1h[4:2]|| keep , braces [4:1]\n,,C,E"), "{deleted}");
        let mut note = chart.notes[0].clone();
        note.id = 999;
        note.time = 1.125;
        chart.notes.push(note);
        let inserted = changed(&source, &chart);
        assert!(inserted.contains("1h[4:2]|| keep , braces [4:1]\n,,C,E"), "{inserted}");
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
        assert!(changed(&source, &chart).contains("{4}2,3,E"));
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
}
