use crate::app::types::{ChartDoc, hold_tail_time, note_secs, slide_end_time};
use crate::state::{EditorLayout, EditorState, Workspace};
use crate::timeline::geometry::{JUDGE_MARGIN, LANE_GUTTER, MAX_PPS, MIN_PPS, RULER_H};

pub fn selection_span(editor: &EditorState, chart: &ChartDoc) -> Option<(f32, f32)> {
    if editor.workspace == Workspace::Autoplay {
        return editor.autoplay_editor.selection_span();
    }
    chart_selection_span(editor, chart)
}

pub fn chart_selection_span(editor: &EditorState, chart: &ChartDoc) -> Option<(f32, f32)> {
    let candidate = editor
        .selected
        .is_empty()
        .then(|| crate::keyboard::navigation::target(editor, chart))
        .flatten();
    let mut span: Option<(f32, f32)> = None;
    for note in chart
        .notes
        .iter()
        .filter(|n| editor.selected.contains(&n.id) || candidate == Some(n.id))
    {
        let start = note_secs(note, &chart.bpms);
        let end = hold_tail_time(note, &chart.bpms)
            .max(slide_end_time(note, &chart.bpms))
            .max(start);
        span = Some(span.map_or((start, end), |(a, b)| (a.min(start), b.max(end))));
    }
    span
}

pub fn fit_chart(editor: &mut EditorState, span: (f32, f32)) {
    let (start, end) = span;
    let margin = ((end - start) * 0.1).max(0.2);
    let pixels = match editor.layout {
        EditorLayout::Horizontal => editor.keyboard.rect.width() - LANE_GUTTER,
        EditorLayout::Vertical => editor.keyboard.rect.height() - JUDGE_MARGIN - RULER_H,
    }
    .max(100.0);
    editor.pixels_per_second =
        (pixels / (end - start + 2.0 * margin).max(0.1)).clamp(MIN_PPS, MAX_PPS);
    editor.view_start = start - margin;
    editor.follow_playhead = false;
    editor.follow_zoom_detached = false;
    editor.keyboard.detached = true;
}

pub fn current(editor: &mut EditorState, now: f32) {
    if editor.workspace == Workspace::Autoplay {
        editor.autoplay_editor.return_current();
    } else {
        current_chart(editor, now);
    }
}

pub fn current_chart(editor: &mut EditorState, now: f32) {
    editor.follow_view_offset = None;
    editor.follow_zoom_detached = false;
    editor.view_start = crate::timeline::geometry::default_view_reference(editor, now);
    editor.follow_playhead = true;
    editor.keyboard.detached = false;
}

pub fn fit(editor: &mut EditorState, span: (f32, f32)) {
    if editor.workspace == Workspace::Autoplay {
        editor.autoplay_editor.locate(Some(span));
    } else {
        fit_chart(editor, span);
    }
}

pub fn toggle_follow(editor: &mut EditorState, now: f32) {
    if editor.workspace == Workspace::Autoplay {
        editor.autoplay_editor.toggle_follow();
    } else if editor.follow_playhead && !editor.keyboard.detached {
        editor.view_start = crate::keyboard::navigation::reference_time(editor, now);
        editor.follow_playhead = false;
        editor.follow_zoom_detached = false;
        editor.keyboard.detached = true;
    } else {
        current_chart(editor, now);
    }
}

pub fn loop_selection(editor: &mut EditorState, chart: &ChartDoc) {
    if let Some((start, end)) = selection_span(editor, chart) {
        editor.loop_range = Some(((start - 0.25).max(0.0), end + 0.5));
        editor.loop_enabled = true;
    }
}
