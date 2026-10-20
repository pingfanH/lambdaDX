use super::{snapshot::Snapshot, *};
use crate::app::types::{self, ChartDoc, Note as ChartNote, NoteType};
use std::collections::HashSet;

pub(super) fn tempo_error(chart: &ChartDoc) -> Option<&'static str> {
    if !chart.bpm.is_finite() || chart.bpm <= 0.0 || !chart.audio_offset.is_finite() {
        return Some("Invalid BPM or audio offset");
    }
    if chart
        .bpms
        .iter()
        .any(|b| !b.measure.is_finite() || b.measure < 1.0 || !b.bpm.is_finite() || b.bpm <= 0.0)
        || chart.bpms.first().is_some_and(|b| b.measure != 1.0)
        || chart.bpms.windows(2).any(|p| p[0].measure >= p[1].measure)
    {
        return Some(
            "BPM changes must be finite, positive, strictly ordered and start at measure 1",
        );
    }
    None
}
fn kind(note: &ChartNote) -> NoteKind {
    match note.note_type {
        NoteType::Tap if types::note_is_touch(note) => NoteKind::Touch,
        NoteType::Hold if types::note_is_touch(note) => NoteKind::TouchHold,
        NoteType::Tap => NoteKind::Tap,
        NoteType::Hold => NoteKind::Hold,
        NoteType::Touch => NoteKind::Touch,
        NoteType::TouchHold => NoteKind::TouchHold,
        NoteType::Slide => NoteKind::Slide,
    }
}
fn finite(note: &ChartNote) -> bool {
    [note.time, note.hold_duration, note.hi_speed]
        .into_iter()
        .all(f32::is_finite)
        && note.slide.iter().all(|s| {
            s.slide_duration.is_finite()
                && s.slide_start_delay.is_finite()
                && s.segments
                    .iter()
                    .flat_map(|s| &s.points)
                    .all(|p| p.beat_offset.is_finite())
        })
}
fn dto(note: &ChartNote) -> ToolResult<Note> {
    if !finite(note) {
        return Err(ToolError::new(
            ErrorCode::ValidationFailed,
            format!(
                "Note {} contains a non-finite number; use chart_check",
                note.id
            ),
        ));
    }
    let touch = types::note_is_touch(note);
    Ok(Note {
        id: note.id.to_string(),
        measure: note.time as f64,
        note_type: kind(note),
        ring_lane: (!touch).then_some(note.lane),
        touch_zone: if touch {
            types::zone::id_to_zone(note.lane).map(|z| z.to_string())
        } else {
            None
        },
        hold_duration_measures: note.hold_duration as f64,
        hi_speed: note.hi_speed as f64,
        is_break: note.is_break,
        is_ex: note.is_ex,
        is_star: note.is_star,
        is_fake_rotate: note.is_fake_rotate,
        is_hanabi: note.is_hanabi,
        is_tapless: note.is_tapless,
        is_touch: touch,
        is_each: note.is_each,
        is_each_head: note.is_each_head,
        slides: note
            .slide
            .iter()
            .map(|s| Slide {
                delay_measures: s.slide_start_delay as f64,
                motion_duration_measures: (s.slide_duration as f64) - (s.slide_start_delay as f64),
                is_break: s.slide_is_break,
                segments: s
                    .segments
                    .iter()
                    .map(|segment| SlideSegment {
                        shape: serde_json::to_value(segment.shape)
                            .expect("SlideShape enum serialization")
                            .as_str()
                            .expect("SlideShape is a string")
                            .into(),
                        points: segment
                            .points
                            .iter()
                            .map(|p| SlidePoint {
                                zone: p.zone.to_string(),
                                stored_beat_offset: p.beat_offset as f64,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    })
}
fn page<T>(
    items: Vec<T>,
    offset: usize,
    limit: usize,
) -> ToolResult<(Vec<T>, usize, Option<usize>)> {
    let total = items.len();
    if offset > total {
        return Err(ToolError::invalid(
            "Offset exceeds the filtered result count",
        ));
    }
    let end = offset.saturating_add(limit).min(total);
    Ok((
        items.into_iter().skip(offset).take(limit).collect(),
        total,
        (end < total).then_some(end),
    ))
}
pub(super) fn notes(snapshot: &Snapshot, r: &QueryNotesRequest) -> ToolResult<NotesPage> {
    let ids: HashSet<_> = r
        .note_ids
        .iter()
        .map(|s| s.parse::<u64>().expect("validated ID"))
        .collect();
    let filtered: Vec<_> = snapshot
        .order
        .iter()
        .map(|&i| &snapshot.chart.notes[i])
        .filter(|n| {
            r.range.is_none_or(|range| range.contains(n.time as f64))
                && (ids.is_empty() || ids.contains(&n.id))
                && (r.note_types.is_empty() || r.note_types.contains(&kind(n)))
        })
        .collect();
    let (notes, total, next_offset) = page(filtered, r.offset, r.limit)?;
    let notes = notes.into_iter().map(dto).collect::<ToolResult<Vec<_>>>()?;
    Ok(NotesPage {
        target: r.target.clone(),
        notes,
        total,
        next_offset,
    })
}
pub(super) fn simai(snapshot: &Snapshot, r: &ReadSimaiRequest) -> ToolResult<SimaiSlice> {
    let text = snapshot.text.as_ref().ok_or_else(|| {
        ToolError::new(
            ErrorCode::UnsupportedOperation,
            "No Simai source is loaded; no synthetic text was generated",
        )
    })?;
    let mut pos = SourcePosition { line: 1, column: 1 };
    let mut start = None;
    let mut end = None;
    let mut total_chars = 0;
    for (index, (byte, ch)) in text.char_indices().enumerate() {
        if index == r.range.start_char {
            start = Some((byte, pos.clone()));
        }
        if index == r.range.end_char {
            end = Some((byte, pos.clone()));
        }
        if ch as u32 == 10 {
            pos.line += 1;
            pos.column = 1;
        } else {
            pos.column += 1;
        }
        total_chars = index + 1;
    }
    if r.range.start_char == total_chars {
        start = Some((text.len(), pos.clone()));
    }
    if r.range.end_char == total_chars {
        end = Some((text.len(), pos));
    }
    let (start_byte, start) =
        start.ok_or_else(|| ToolError::invalid("Character range starts beyond the source"))?;
    let (end_byte, end) =
        end.ok_or_else(|| ToolError::invalid("Character range ends beyond the source"))?;
    if end_byte - start_byte > MAX_TEXT_BYTES {
        return Err(ToolError::invalid(
            "Source slice exceeds 16 KiB UTF-8; request fewer characters",
        ));
    }
    Ok(SimaiSlice { target: r.target.clone(), text: text[start_byte..end_byte].into(), range: r.range, start, end, total_chars, unapplied_draft: snapshot.draft, matches_chart: snapshot.matches_chart, mapping_note: "Exact Unicode scalar positions in the captured text; LF advances the one-based line. CR counts as a scalar. No verified source-to-note or measure mapping is exposed.".into() })
}
pub(super) fn check(
    snapshot: &Snapshot,
    r: &CheckRequest,
    core_available: bool,
) -> ToolResult<CheckReport> {
    if matches!(r.level, CheckLevel::Core) {
        return Err(ToolError::new(
            if core_available {
                ErrorCode::UnsupportedOperation
            } else {
                ErrorCode::CoreUnavailable
            },
            "Core checks require the existing core worker integration; no native parsing was started",
        ));
    }
    let mut diagnostics = Vec::new();
    if let Some(message) = tempo_error(&snapshot.chart) {
        diagnostics.push(Diagnostic {
            rule_id: "chart.tempo".into(),
            severity: "error".into(),
            note_id: None,
            measure: None,
            message: message.into(),
        });
    }
    let mut ids = HashSet::new();
    for &index in &snapshot.order {
        let note = &snapshot.chart.notes[index];
        let duplicate = !ids.insert(note.id);
        if r.range
            .is_some_and(|range| note.time.is_finite() && !range.contains(note.time as f64))
        {
            continue;
        }
        let mut push = |rule: &str, message: &str| {
            diagnostics.push(Diagnostic {
                rule_id: rule.into(),
                severity: "error".into(),
                note_id: Some(note.id.to_string()),
                measure: note.time.is_finite().then_some(note.time as f64),
                message: message.into(),
            })
        };
        if duplicate {
            push("note.duplicate_id", "Authored note IDs must be unique");
        }
        if !finite(note) {
            push(
                "note.non_finite",
                "Note contains non-finite timing, duration, speed or waypoint offset",
            );
        }
        if note.time < 1.0 || note.hold_duration < 0.0 || note.hi_speed < 0.0 {
            push(
                "note.timing",
                "Position must start at measure 1; hold duration and speed cannot be negative",
            );
        }
        let touch = types::note_is_touch(note);
        if matches!(kind(note), NoteKind::Hold | NoteKind::TouchHold) && note.hold_duration <= 0.0 {
            push(
                "hold.duration",
                "Hold and touch-hold duration must be positive",
            );
        }
        if (touch && types::zone::id_to_zone(note.lane).is_none())
            || (!touch && !(1..=8).contains(&note.lane))
        {
            push(
                "note.zone",
                "Ring lanes must be 1..8 and touch zones must be A1..E8 or C",
            );
        }
        if note.note_type == NoteType::Slide && (note.slide.is_empty() || touch) {
            push(
                "slide.structure",
                "A slide requires a ring head and at least one trail",
            );
        }
        if note.note_type != NoteType::Slide && !note.slide.is_empty() {
            push("slide.note_type", "Only slide notes can carry slide trails");
        }
        for slide in &note.slide {
            if slide.slide_start_delay < 0.0 || slide.slide_duration <= slide.slide_start_delay {
                push(
                    "slide.duration",
                    "Slide motion duration must be positive and delay nonnegative",
                );
            }
            if slide.segments.is_empty() {
                push(
                    "slide.segments",
                    "Slide trail requires at least one segment",
                );
            }
            let mut from = note.lane;
            for segment in &slide.segments {
                if segment.points.is_empty() {
                    push("slide.points", "Slide segment requires a waypoint");
                }
                for point in &segment.points {
                    let to = point.zone.to_id();
                    if let Err(message) = types::validate_slide_shape(segment.shape, from, to) {
                        push("slide.shape", &message);
                    }
                    from = to;
                }
            }
        }
    }
    let (diagnostics, total, next_offset) = page(diagnostics, r.offset, r.limit)?;
    Ok(CheckReport { target: r.target.clone(), diagnostics, total, next_offset, limitations: vec!["Structural checks cover authored main-chart notes and timing. Template definitions, autoplay events, core compilation, playability and hand movement are not assessed.".into(), "Diagnostics describe the canonical chart, not an unapplied Simai draft.".into()] })
}
