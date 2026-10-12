//! Right-hand inspector: note properties and chart info.

use egui_macroquad::egui;

use crate::app::types::{BpmChange, ChartDoc, Note, NoteType};
use crate::state::EditorState;
use crate::theme::{FONT_SMALL, TEXT_DIM};

use super::widgets::{drag_row, hsep, section_header, text_row, toggle_chip, value_row};

/// Draw both inspector sections. `open` toggles each section's collapsed state.
pub fn draw(ui: &mut egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc, open: &mut [bool; 2]) {
    note_properties(ui, editor, chart, &mut open[0]);
    ui.add_space(4.0);
    hsep(ui);
    ui.add_space(4.0);
    chart_info(ui, editor, chart, &mut open[1]);
}

fn note_properties(ui: &mut egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc, open: &mut bool) {
    if section_header(ui, "Note Properties", *open) {
        *open = !*open;
    }
    if !*open {
        return;
    }

    // Multi-select: read-only aggregate (a value when every note agrees, else
    // "mixed"). Editing is per-note, so it stays a single-selection action.
    if editor.selected.len() > 1 {
        draw_multi(ui, chart, &editor.selected);
        return;
    }

    // The rows are always laid out (with "-" placeholders when nothing is
    // selected) so the panel keeps the design's shape.
    let selected = editor
        .selected
        .iter()
        .next()
        .copied()
        .and_then(|id| chart.notes.iter().position(|n| n.id == id));
    let Some(idx) = selected else {
        let hint = if editor.selected.is_empty() {
            "no selection"
        } else {
            "selection not found"
        };
        ui.label(egui::RichText::new(hint).size(FONT_SMALL).color(TEXT_DIM));
        value_row(ui, "Type", "—");
        value_row(ui, "Time", "—");
        value_row(ui, "Lane", "—");
        value_row(ui, "Duration", "—");
        return;
    };

    let id = chart.notes[idx].id;
    ui.label(
        egui::RichText::new(format!("{} selected · id {id}", editor.selected.len()))
            .size(FONT_SMALL)
            .color(TEXT_DIM),
    );

    // Read the note out first so a single edit batches into one undo snapshot.
    let note = &chart.notes[idx];
    let note_type = note.note_type;
    let mut time = note.time;
    let mut lane = note.lane as i32;
    let mut dur = note.hold_duration;
    let mut break_on = note.is_break;
    let mut ex_on = note.is_ex;
    let mut each_on = note.is_each;
    let mut star_on = note.is_star;
    let mut tapless_on = note.is_tapless;
    let is_slide = matches!(note_type, NoteType::Slide);
    let (mut slide_delay, mut slide_dur) = slide_timing(note).unwrap_or((0.0, 0.0));

    let mut changed = false;
    value_row(ui, "Type", &format!("{note_type:?}"));
    changed |= drag_row(ui, "Time", &mut time, 0.01, 0.0..=1.0e6);
    let mut lane_f = lane as f32;
    if drag_row(ui, "Lane", &mut lane_f, 1.0, 1.0..=8.0) {
        lane = lane_f.round() as i32;
        changed = true;
    }
    if matches!(note_type, NoteType::Hold | NoteType::TouchHold) {
        changed |= drag_row(ui, "Duration", &mut dur, 0.01, 0.0..=1.0e6);
    }
    if is_slide {
        changed |= drag_row(ui, "Delay", &mut slide_delay, 0.01, 0.0..=1.0e6);
        changed |= drag_row(ui, "Slide Dur", &mut slide_dur, 0.01, 0.0..=1.0e6);
    }

    ui.horizontal_wrapped(|ui| {
        changed |= toggle_chip(ui, "Break", &mut break_on);
        changed |= toggle_chip(ui, "Ex", &mut ex_on);
        changed |= toggle_chip(ui, "Each", &mut each_on);
        if is_slide {
            changed |= toggle_chip(ui, "Star", &mut star_on);
            changed |= toggle_chip(ui, "Tapless", &mut tapless_on);
        }
    });

    if changed {
        editor.push_history(chart);
        let n = &mut chart.notes[idx];
        n.time = time.max(0.0);
        n.lane = lane.clamp(1, 8) as u8;
        n.hold_duration = dur.max(0.0);
        n.is_break = break_on;
        n.is_ex = ex_on;
        n.is_each = each_on;
        n.is_star = star_on;
        n.is_tapless = tapless_on;
        if is_slide {
            set_slide_timing(n, slide_delay, slide_dur);
        }
    }
}

/// Read-only aggregate for a multi-note selection.
fn draw_multi(ui: &mut egui::Ui, chart: &ChartDoc, ids: &std::collections::HashSet<u64>) {
    let notes: Vec<&Note> = chart.notes.iter().filter(|n| ids.contains(&n.id)).collect();
    let count = notes.len();
    ui.label(
        egui::RichText::new(format!("{count} selected · aggregate"))
            .size(FONT_SMALL)
            .color(TEXT_DIM),
    );
    value_row(ui, "Type", &same_or_mixed(notes.iter().map(|n| format!("{:?}", n.note_type))));
    value_row(ui, "Time", &same_or_mixed(notes.iter().map(|n| format!("{:.3}", n.time))));
    value_row(ui, "Lane", &same_or_mixed(notes.iter().map(|n| n.lane.to_string())));
    value_row(ui, "Duration", &same_or_mixed(notes.iter().map(|n| format!("{:.3}", n.hold_duration))));
    value_row(ui, "Break", &tri_state(notes.iter().map(|n| n.is_break)));
    value_row(ui, "Ex", &tri_state(notes.iter().map(|n| n.is_ex)));
    value_row(ui, "Each", &tri_state(notes.iter().map(|n| n.is_each)));
}

/// The shared value across `vals`, or `"mixed"` when they differ (empty → "—").
fn same_or_mixed(vals: impl Iterator<Item = String>) -> String {
    let mut it = vals;
    let Some(first) = it.next() else {
        return "—".to_string();
    };
    if it.all(|v| v == first) {
        first
    } else {
        "mixed".to_string()
    }
}

/// `"On"` / `"Off"` when every flag agrees, else `"mixed"`.
fn tri_state(vals: impl Iterator<Item = bool>) -> String {
    let all: Vec<bool> = vals.collect();
    if all.is_empty() {
        "—".to_string()
    } else if all.iter().all(|b| *b) {
        "On".to_string()
    } else if all.iter().all(|b| !*b) {
        "Off".to_string()
    } else {
        "mixed".to_string()
    }
}

/// The first sub-slide's `(start delay, duration)` in measures, if the note has
/// any sub-slides (slides only).
fn slide_timing(note: &Note) -> Option<(f32, f32)> {
    note.slide
        .first()
        .map(|s| (s.slide_start_delay, s.slide_duration))
}

/// Write `(start delay, duration)` in measures to every sub-slide of `note`
/// (negative values clamp to zero).
fn set_slide_timing(note: &mut Note, delay: f32, dur: f32) {
    for s in &mut note.slide {
        s.slide_start_delay = delay.max(0.0);
        s.slide_duration = dur.max(0.0);
    }
}

/// Set the chart's primary BPM, keeping `bpms` (the source of truth for
/// measure↔seconds conversion) in sync: updates the first change, or seeds one
/// at measure 1.0 when the list is empty.
fn set_chart_bpm(chart: &mut ChartDoc, bpm: f32) {
    let bpm = bpm.max(1.0);
    chart.bpm = bpm;
    match chart.bpms.first_mut() {
        Some(first) => first.bpm = bpm,
        None => chart.bpms.push(BpmChange { measure: 1.0, bpm }),
    }
}

fn chart_info(ui: &mut egui::Ui, editor: &mut EditorState, chart: &mut ChartDoc, open: &mut bool) {
    if section_header(ui, "Chart Info", *open) {
        *open = !*open;
    }
    if !*open {
        return;
    }
    let mut title = chart.title.clone();
    let mut artist = chart.artist.clone();
    let mut bpm = chart.bpm;
    let mut offset = chart.audio_offset;

    let mut changed = false;
    changed |= text_row(ui, "Title", &mut title);
    changed |= text_row(ui, "Artist", &mut artist);
    changed |= drag_row(ui, "BPM", &mut bpm, 0.1, 1.0..=1000.0);
    changed |= drag_row(ui, "Offset", &mut offset, 0.001, -60.0..=60.0);

    if changed {
        editor.push_history(chart);
        chart.title = title;
        chart.artist = artist;
        set_chart_bpm(chart, bpm);
        chart.audio_offset = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::Slide;

    fn sub(delay: f32, dur: f32) -> Slide {
        Slide {
            segments: Vec::new(),
            slide_duration: dur,
            slide_start_delay: delay,
            slide_is_break: false,
            runtime_parts: 1,
        }
    }

    fn slide_note(delays_durs: &[(f32, f32)]) -> Note {
        Note {
            note_type: NoteType::Slide,
            slide: delays_durs.iter().map(|(d, t)| sub(*d, *t)).collect(),
            ..Default::default()
        }
    }

    fn chart() -> ChartDoc {
        ChartDoc {
            version: String::new(),
            title: String::new(),
            artist: String::new(),
            simai_level: 0,
            bpm: 120.0,
            bpms: Vec::new(),
            audio_offset: 0.0,
            notes: Vec::new(),
            templates: Vec::new(),
            template_instances: Vec::new(),
        }
    }

    #[test]
    fn slide_timing_reads_the_first_sub_slide() {
        let n = slide_note(&[(0.5, 2.0), (1.0, 3.0)]);
        assert_eq!(slide_timing(&n), Some((0.5, 2.0)));
        assert_eq!(slide_timing(&Note::default()), None, "a tap has no sub-slides");
    }

    #[test]
    fn set_slide_timing_writes_every_sub_slide_and_clamps() {
        let mut n = slide_note(&[(0.0, 0.0), (0.0, 0.0)]);
        set_slide_timing(&mut n, -1.0, 4.0);
        assert!(n.slide.iter().all(|s| s.slide_start_delay == 0.0));
        assert!(n.slide.iter().all(|s| s.slide_duration == 4.0));
    }

    #[test]
    fn set_chart_bpm_seeds_then_updates_bpms() {
        let mut c = chart();
        set_chart_bpm(&mut c, 150.0);
        assert_eq!(c.bpm, 150.0);
        assert_eq!(c.bpms.len(), 1);
        assert_eq!(c.bpms[0].measure, 1.0);
        assert_eq!(c.bpms[0].bpm, 150.0);

        set_chart_bpm(&mut c, 180.0);
        assert_eq!(c.bpms.len(), 1, "an existing list stays a single entry");
        assert_eq!(c.bpms[0].bpm, 180.0);
        assert_eq!(c.bpm, 180.0);

        set_chart_bpm(&mut c, 0.0);
        assert_eq!(c.bpm, 1.0, "bpm clamps to a positive value");
    }
}
