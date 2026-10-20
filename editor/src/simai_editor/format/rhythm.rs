//! Compact comma timing without interpreting or rewriting authored note tokens.

use super::{Kind, source};

struct Cell {
    text: String,
    duration: f64,
    content: bool,
}

/// Division directives govern the comma after their cell. Empty cells can be
/// joined, but notes, tempo, speed and comments remain anchored in order.
pub(super) fn compact(text: &str) -> Result<String, String> {
    let mut cells = Vec::new();
    let mut division = 4_u64;
    let mut value = String::new();
    let mut content = false;
    for token in source::tokens(text) {
        let raw = &text[token.bytes];
        if token.kind == Kind::Division {
            division = raw.trim_matches(['{', '}']).parse().map_err(|_| "Invalid Simai division")?;
            if division == 0 { return Err("Invalid Simai division".into()); }
        } else if raw == "," {
            cells.push(Cell { text: std::mem::take(&mut value), duration: 1.0 / division as f64, content });
            content = false;
        } else {
            value.push_str(raw);
            content |= token.kind != Kind::Space;
        }
    }
    cells.push(Cell { text: value, duration: 0.0, content });
    let mut out = String::new();
    let mut active = 4;
    let mut index = 0;
    while index < cells.len() {
        let mut gap = cells[index].duration;
        let mut next = index + 1;
        while next < cells.len() && !cells[next].content && cells[next].duration > 0.0 {
            gap += cells[next].duration;
            next += 1;
        }
        if gap > 0.0 {
            emit_gap(&mut out, &cells[index].text, gap, &mut active)?;
            // Keep authored whitespace, especially newlines following comments.
            for cell in &cells[index + 1..next] { out.push_str(&cell.text); }
        } else { out.push_str(&cells[index].text); }
        index = next;
    }
    Ok(out)
}

fn emit_gap(out: &mut String, content: &str, gap: f64, active: &mut u64) -> Result<(), String> {
    let steps = crate::io::simai::gap_steps(gap, 0.0000005)?;
    for (index, step) in steps.into_iter().enumerate() {
        if *active != step {
            out.push_str(&format!("{{{step}}}"));
            *active = step;
        }
        if index == 0 {
            out.push_str(content);
            if source::tokens(content).last().is_some_and(|token| token.kind == Kind::Comment) {
                out.push('\n');
            }
        }
        out.push(',');
    }
    Ok(())
}

pub(super) fn events(events: &[(f32, String)], division: u64, terminate: bool) -> Result<String, String> {
    let mut out = String::new();
    let mut active = division;
    let mut cursor = 0;
    while cursor < events.len() {
        let time = events[cursor].0;
        let end = cursor + events[cursor..].partition_point(|event| (event.0 - time).abs() < 0.00002);
        let content = events[cursor..end].iter().map(|event| event.1.as_str())
            .filter(|text| !text.is_empty()).collect::<Vec<_>>().join("/");
        if let Some(next) = events.get(end) {
            emit_gap(&mut out, &content, (next.0 - time) as f64, &mut active)?;
        } else {
            out.push_str(&content);
            if !content.is_empty() { out.push(','); }
        }
        cursor = end;
    }
    if terminate { out.push('E'); }
    Ok(out)
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;

    #[test]
    fn long_empty_runs_use_coarser_divisions_without_moving_notes_or_e() {
        for text in [
            ",{1}(120)1,{64},,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,2,,,,,,,,,,,,,,,,,,,,,,,,,,,,,E",
            "(120),,,1,{32}2/3,,,2/3,,,{8}2/3,{16}2,{32}3,,,{64}2,,,,,,,,,,,,,,,2,,,,,,,,,,,,,,,,,,,,,,,,,,,,,E",
        ] {
            let compacted = compact(text).unwrap();
            let before = crate::simai_editor::source::timeline(text, 120.0).unwrap();
            let after = crate::simai_editor::source::timeline(&compacted, 120.0).unwrap();
            let before_end = before.last().unwrap().time;
            let after_end = after.last().unwrap().time;
            assert!((before_end - after_end).abs() < 0.00002, "E moved: {compacted}");
            let before = crate::simai_editor::parse_draft(text, 120.0, 6).unwrap();
            let after = crate::simai_editor::parse_draft(&compacted, 120.0, 6).unwrap();
            assert!(super::super::equivalent(&before, &after), "notes moved: {compacted}");
            assert!(!compacted.contains(",,,,,,,,,"), "long empty run: {compacted}");
            assert_eq!(compact(&compacted).unwrap(), compacted, "compaction must be stable");
        }
    }

    #[test]
    fn compaction_keeps_dense_notes_and_clock_directives_anchored() {
        for text in [
            "(120){64}1,2,3,4,5,6,7,8,E",
            "(120){64}1h[4:1],,,,,,,,,,,,,,,(180),,,,,,,,,,,,,,,<H0.75>2-5[4:1],,,,,,,,,,,,,,,E",
            "(120){32}1,,,,,,,,,,,|| keep , and {64} in comment\n,,,,,,,,,,,2,,,,,,,,,,,E",
        ] {
            let compacted = compact(text).unwrap();
            let before = crate::simai_editor::parse_draft(text, 120.0, 6).unwrap();
            let after = crate::simai_editor::parse_draft(&compacted, 120.0, 6).unwrap();
            assert!(super::super::equivalent(&before, &after), "{text} => {compacted}");
            let before = source::timeline(text, 120.0).unwrap();
            let after = source::timeline(&compacted, 120.0).unwrap();
            assert!((before.last().unwrap().time - after.last().unwrap().time).abs() < 0.00002,
                "E moved: {compacted}");
            assert_eq!(compact(&compacted).unwrap(), compacted);
            if text.contains("||") { assert!(compacted.contains("|| keep , and {64} in comment\n")); }
        }
    }
}
