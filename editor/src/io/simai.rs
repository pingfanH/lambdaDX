//! Simai serialization on the original measure clock, with header preservation.

use crate::app::types::{BpmChange, ChartDoc, NoteType};

pub(super) fn document(chart: &ChartDoc, level: u32, original: &str) -> Result<String, String> {
    document_with_source(chart, level, original, None)
}

pub(super) fn document_with_source(
    chart: &ChartDoc, level: u32, original: &str, source: Option<&crate::simai_editor::format::Document>,
) -> Result<String, String> {
    super::validate(chart)?;
    if !chart.templates.is_empty() || !chart.template_instances.is_empty() {
        return Err("Simai cannot retain editor templates; save this document as native JSON".into());
    }
    if [&chart.title, &chart.artist].iter().any(|s| s.contains(['\r', '\n'])) {
        return Err("Title and artist must be single-line fields".into());
    }
    let body = if let Some(source) = source {
        source.update(chart)?
    } else if let Some(text) = body_for_level(original, level) {
        let mut previous = crate::app::maidata::from_maidata_level(original, level)?;
        if chart.notes.iter().all(|n| n.id != 0) { crate::app::maichart::assign_note_ids(&mut previous.notes); }
        crate::simai_editor::format::Document { text, chart: previous }.update(chart)?
    } else { body(chart)? };
    let mut text = original.to_string();
    let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
    set_field(&mut text, "title", &chart.title, newline);
    set_field(&mut text, "artist", &chart.artist, newline);
    set_field(&mut text, "first", &number(chart.audio_offset), newline);
    let bpm = chart.bpms.first().map(|b| b.bpm).unwrap_or(chart.bpm);
    set_field(&mut text, "wholebpm", &number(bpm), newline);
    let rating = format!("lv_{level}");
    // Rating strings such as 14+ are richer than ChartDoc's integer field.
    if !has_field(&text, &rating) {
        set_field(&mut text, &rating, &chart.simai_level.to_string(), newline);
    }
    set_field(&mut text, &format!("inote_{level}"), &body, newline);
    Ok(text)
}

pub(crate) fn body_for_level(text: &str, level: u32) -> Option<String> {
    let fields = fields(text);
    let key = format!("inote_{level}");
    let (index, (_, _, start, _)) = fields.iter().enumerate().rev().find(|(_, (name, ..))| *name == key)?;
    let end = fields.get(index + 1).map(|(_, start, ..)| *start).unwrap_or(text.len());
    Some(text[*start..end].trim_end_matches(['\r', '\n']).to_owned())
}

fn number(value: f32) -> String {
    let text = format!("{value:.9}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn fields(text: &str) -> Vec<(String, usize, usize, usize)> {
    let mut offset = 0;
    let mut result = Vec::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start_matches('\u{feff}').trim_start();
        if let Some((key, _)) = trimmed.strip_prefix('&').and_then(|s| s.split_once('=')) {
            let start = offset + line.len() - trimmed.len();
            let value = start + trimmed.find('=').unwrap() + 1;
            let end = offset + line.trim_end_matches(['\r', '\n']).len();
            result.push((key.trim().to_string(), start, value, end));
        }
        offset += line.len();
    }
    result
}

fn has_field(text: &str, key: &str) -> bool {
    fields(text).iter().any(|(name, ..)| name == key)
}

fn set_field(text: &mut String, key: &str, value: &str, newline: &str) {
    let fields = fields(text);
    let matches: Vec<_> = fields.iter().enumerate().filter(|(_, (name, ..))| name == key).collect();
    if matches.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') { text.push_str(newline); }
        text.push_str(&format!("&{key}={value}{newline}"));
        return;
    }
    for (index, (_, _, start, line_end)) in matches.into_iter().rev() {
        let end = if key.starts_with("inote_") {
            let next = fields.get(index + 1).map(|(_, start, ..)| *start).unwrap_or(text.len());
            let content = &text[*start..next];
            *start + content.trim_end_matches(char::is_whitespace).len()
        } else { *line_end };
        text.replace_range(*start..end, value);
    }
}

pub(crate) fn body(chart: &ChartDoc) -> Result<String, String> {
    serialize_body(chart, false)
}

pub(crate) fn editable_body(chart: &ChartDoc) -> Result<String, String> {
    serialize_body(chart, true)
}

fn serialize_body(chart: &ChartDoc, multiline: bool) -> Result<String, String> {
    let default_bpms = [BpmChange { measure: 1.0, bpm: chart.bpm }];
    let bpms = if chart.bpms.is_empty() { &default_bpms[..] } else { &chart.bpms };
    let mut notes: Vec<_> = chart.notes.iter().collect();
    notes.sort_by(|a, b| a.time.total_cmp(&b.time).then_with(|| a.id.cmp(&b.id)));
    let mut positions = vec![1.0_f64];
    positions.extend(notes.iter().map(|n| n.time as f64));
    positions.extend(bpms.iter().map(|b| b.measure as f64));
    let tail = notes.iter().map(|n| {
        let duration = n.slide.iter().map(|s| s.slide_duration).fold(n.hold_duration, f32::max);
        n.time as f64 + duration as f64
    }).fold(1.0, f64::max);
    positions.push(tail.max(positions.iter().copied().fold(1.0, f64::max)) + 0.25);
    positions.sort_by(f64::total_cmp);
    positions.dedup();
    let mut out = String::new();
    let mut line_start = 0;
    let mut divisor = 0;
    let mut speed = 1.0;
    let mut note_index = 0;
    let mut bpm_index = 0;
    let mut written_position = 1.0;
    for (index, pair) in positions.windows(2).enumerate() {
        let position = pair[0];
        let bpm = bpms[bpms.partition_point(|b| b.measure as f64 <= position).saturating_sub(1)].bpm;
        // The core rounds to microseconds and the editor stores f32 measures.
        // Bound rounding by adjacent events so distinct notes stay distinct.
        let tolerance = (pair[1].abs() * f32::EPSILON as f64)
            .max(bpm as f64 / 240_000_000.0)
            .min((pair[1] - position) * 0.25)
            .min(positions.get(index + 2).map_or(f64::INFINITY, |next| (next - pair[1]) * 0.25));
        let steps = gap_steps(pair[1] - written_position, tolerance)?;
        written_position += steps.iter().map(|d| 1.0 / *d as f64).sum::<f64>();
        // Directives must precede the note in a comma-delimited Simai segment.
        if let Some(&step) = steps.first() { subdivision(&mut out, &mut divisor, step); }
        while bpm_index < bpms.len() && bpms[bpm_index].measure as f64 == position {
            out.push_str(&format!("({})", number(bpms[bpm_index].bpm)));
            bpm_index += 1;
        }
        let start = note_index;
        while note_index < notes.len() && notes[note_index].time as f64 == position { note_index += 1; }
        let group = &notes[start..note_index];
        if let Some(first) = group.first() {
            let effective = |s: f32| if s == 0.0 { 1.0 } else { s };
            let next_speed = effective(first.hi_speed);
            if group.iter().any(|n| effective(n.hi_speed) != next_speed) {
                return Err("Simultaneous notes have different hi-speeds; use native JSON to retain them".into());
            }
            if speed != next_speed {
                out.push_str(&format!("<H{}>", number(next_speed)));
                speed = next_speed;
            }
        }
        for (index, note) in group.iter().enumerate() {
            if note.note_type == NoteType::Slide {
                for slide in &note.slide {
                    if slide.segments.iter().any(|s| s.points.is_empty() || s.points.last().is_some_and(|p| !(1..=8).contains(&p.zone.to_id()))) {
                        return Err(format!("Note {} has an unfinished slide endpoint", note.id));
                    }
                }
                if note.is_tapless && note.slide.is_empty() {
                    return Err(format!("Note {} has no slide trail", note.id));
                }
            }
            let text = crate::simai_editor::format::note_text(note, chart)?;
            if text.is_empty() { return Err(format!("Note {} cannot be exported", note.id)); }
            if index > 0 { out.push('/'); }
            out.push_str(&text);
        }
        for (index, step) in steps.into_iter().enumerate() {
            if index > 0 { subdivision(&mut out, &mut divisor, step); }
            out.push(',');
        }
        if multiline && (pair[1].fract() == 0.0 || out.len() - line_start >= 100) {
            out.push('\n');
            line_start = out.len();
        }
        if out.len() > 16 * 1024 * 1024 { return Err("Chart is too large for Simai export".into()); }
    }
    out.push('E');
    Ok(out)
}

fn subdivision(out: &mut String, current: &mut u64, divisor: u64) {
    if *current != divisor {
        out.push_str(&format!("{{{divisor}}}"));
        *current = divisor;
    }
}

/// Recover simple rhythmic fractions before encoding genuinely off-grid rests.
/// The caller compensates against absolute positions, so rounding cannot drift.
pub(crate) fn gap_steps(mut gap: f64, tolerance: f64) -> Result<Vec<u64>, String> {
    if gap <= 0.0 || !gap.is_finite() || gap > 100_000.0 { return Err("Invalid or excessive measure gap".into()); }
    let mut steps = vec![1; gap.floor() as usize];
    gap -= gap.floor();
    if gap <= tolerance && !steps.is_empty() { return Ok(steps); }
    for divisor in 1..=192 {
        let count = (gap * divisor as f64).round() as usize;
        if count > 0 && (gap - count as f64 / divisor as f64).abs() <= tolerance {
            steps.extend(std::iter::repeat_n(divisor, count));
            return Ok(steps);
        }
    }
    while gap > tolerance || steps.is_empty() {
        let reciprocal = 1.0 / gap;
        let nearest = reciprocal.round().max(1.0);
        let divisor = if (gap - 1.0 / nearest).abs() <= tolerance { nearest } else { reciprocal.ceil() };
        if divisor >= (1_u64 << 40) as f64 { return Err("Measure gap is below Simai timing precision".into()); }
        steps.push(divisor as u64);
        gap -= 1.0 / divisor;
    }
    Ok(steps)
}
