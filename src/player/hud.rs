//! Pad-preview HUD: a header strip with the playback progress bar (same style
//! as the player UI), the elapsed time and an AUTO toggle.
//!
//! Pointer handling is done with macroquad directly; [`handle_input`] reports
//! whether the mouse was consumed so the pad's own touch routing can ignore it.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player::font;
use crate::player::input::UiPointer;
use crate::player::render::progress;
use crate::player::state::PadPreviewState;

// Colors mirror `player_ui::theme` so the scrubber matches the player exactly.
const PANEL: Color = Color::from_rgba(0x20, 0x20, 0x22, 255);
const RAISED: Color = Color::from_rgba(0x2a, 0x2a, 0x2e, 255);
const BORDER: Color = Color::from_rgba(0x3a, 0x3a, 0x40, 255);
const TEXT: Color = Color::from_rgba(0xd6, 0xd6, 0xda, 255);
const TEXT_DIM: Color = Color::from_rgba(0x9a, 0x9a, 0xa2, 255);
const ACCENT: Color = Color::from_rgba(0x52, 0xd6, 0xe8, 255);

fn contains(r: RectF, p: Vec2) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

/// `(progress_bar, auto_button)` geometry inside the header `r`.
fn geometry(r: RectF, scale: f32) -> (RectF, RectF) {
    let pad = 12.0 * scale;
    let auto_w = 70.0 * scale;
    let time_w = 96.0 * scale;
    let auto_r = RectF {
        x: r.x + r.w - pad - auto_w,
        y: r.y + (r.h - 32.0 * scale) * 0.5,
        w: auto_w,
        h: 32.0 * scale,
    };
    let bar_x = r.x + (r.w * 0.28).max(200.0 * scale);
    let bar_right = auto_r.x - 12.0 * scale - time_w;
    let bar = RectF {
        x: bar_x,
        y: r.y + r.h * 0.5 - 4.0 * scale,
        w: (bar_right - bar_x).max(80.0 * scale),
        h: 8.0 * scale,
    };
    (bar, auto_r)
}

fn bar_frac(app: &PadPreviewState, bar: RectF) -> f32 {
    let dur = app.song_duration();
    // `scrub_to` keeps `song_time` in sync with the drag, so this tracks the
    // pointer for both mouse and touch without reading the mouse.
    (app.song_time() / dur).clamp(0.0, 1.0)
}

/// Handle progress-bar scrubbing and the AUTO button with the unified UI
/// pointer (mouse **and** touch). Returns the pointer id that was consumed, so
/// the pad's touch routing can ignore it this frame.
pub fn handle_input(
    app: &mut PadPreviewState,
    header: RectF,
    scale: f32,
    ui: UiPointer,
) -> Option<u64> {
    let (bar, auto_r) = geometry(header, scale);
    let pos = ui.pos;

    if ui.pressed && contains(auto_r, pos) {
        let on = !app.autoplay;
        crate::player::autoplay::set_on(app, on);
        return Some(ui.id);
    }

    let hit = RectF {
        x: bar.x,
        y: bar.y - 10.0 * scale,
        w: bar.w,
        h: bar.h + 20.0 * scale,
    };
    if ui.pressed && contains(hit, pos) {
        app.scrubbing = true;
        app.scrub_pointer = Some(ui.id);
        app.stop_audio_if_any();
    }
    if app.scrubbing {
        // Only the pointer that grabbed the bar drives it; others pass through.
        if app.scrub_pointer.is_none() || app.scrub_pointer == Some(ui.id) {
            let dur = app.song_duration();
            let t = ((pos.x - bar.x) / bar.w).clamp(0.0, 1.0) * dur;
            if ui.released {
                app.seek_to(t);
                app.scrubbing = false;
                app.scrub_pointer = None;
            } else if ui.down {
                app.scrub_to(t);
            }
            return Some(ui.id);
        }
    }
    None
}

/// Draw the header strip: title, progress bar, elapsed time and AUTO toggle.
pub fn draw(app: &PadPreviewState, header: RectF, scale: f32) {
    draw_rectangle(header.x, header.y, header.w, header.h, PANEL);
    draw_rectangle(header.x, header.y + header.h - 1.0, header.w, 1.0, BORDER);

    font::text(
        "Pad Preview",
        header.x + 12.0 * scale,
        header.y + 24.0 * scale,
        18.0 * scale,
        TEXT,
    );
    font::text(
        "Space 播放  ·  R 重播  ·  ←/→ 快进  ·  O 自动  ·  F1 参数",
        header.x + 12.0 * scale,
        header.y + 48.0 * scale,
        11.0 * scale,
        TEXT_DIM,
    );

    let (bar, auto_r) = geometry(header, scale);
    let frac = bar_frac(app, bar);
    progress::progress_bar(bar, frac, ACCENT, RAISED, BORDER);

    let hovered = contains(bar, vec2(mouse_position().0, mouse_position().1));
    let handle_r = if hovered || app.scrubbing { 7.0 * scale } else { 5.0 * scale };
    draw_circle(
        bar.x + bar.w * frac,
        bar.y + bar.h * 0.5,
        handle_r,
        if hovered || app.scrubbing { ACCENT } else { TEXT_DIM },
    );

    let dur = app.song_duration();
    let shown = if app.scrubbing { frac * dur } else { app.song_time() };
    let label = format_time(shown);
    let tw = font::text_width(&label, 13.0 * scale);
    font::text(
        &label,
        auto_r.x - 12.0 * scale - tw,
        header.y + header.h * 0.5 + 4.0 * scale,
        13.0 * scale,
        TEXT_DIM,
    );

    // AUTO toggle.
    let fill = if app.autoplay { ACCENT } else { RAISED };
    let label_c = if app.autoplay { Color::from_rgba(0x10, 0x10, 0x12, 255) } else { TEXT_DIM };
    draw_rectangle(auto_r.x, auto_r.y, auto_r.w, auto_r.h, fill);
    progress::rect_outline(auto_r, 1.0, BORDER);
    let w = font::text_width("AUTO", 14.0 * scale);
    font::text(
        "AUTO",
        auto_r.x + (auto_r.w - w) * 0.5,
        auto_r.y + auto_r.h * 0.5 + 5.0 * scale,
        14.0 * scale,
        label_c,
    );
}

/// Draw the current Simai fragment(s) and the next group, bottom-right of the
/// pad panel. Same-time fragments (`each`, same-head stars, a conslide's arc
/// tokens) are shown together so nothing is skipped or shown one group ahead.
pub fn draw_simai_debug(app: &PadPreviewState, rect: RectF, scale: f32) {
    let frags = &app.simai_fragments;
    if frags.is_empty() {
        return;
    }
    let t = app.song_time();
    let count = frags.partition_point(|f| f.time <= t);

    // Group fragments by equal time (the timeline is time-ordered).
    let group = |slice: &[crate::app::maidata::SimaiFragment]| -> Vec<usize> {
        if slice.is_empty() {
            return Vec::new();
        }
        let t0 = slice[0].time;
        slice
            .iter()
            .enumerate()
            .take_while(|(_, f)| (f.time - t0).abs() < 1e-4)
            .map(|(i, _)| i)
            .collect()
    };
    let join = |idxs: &[usize]| {
        idxs.iter()
            .filter_map(|i| frags.get(*i))
            .map(|f| f.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };

    let margin = 14.0 * scale;
    let right = rect.x + rect.w - margin;
    let mut y = rect.y + rect.h - margin;

    // Next group (the first group strictly after `count`).
    if count < frags.len() {
        let next = group(&frags[count..]);
        let next: Vec<usize> = next.into_iter().map(|i| i + count).collect();
        let s = format!("→ {}", join(&next));
        let w = font::text_width(&s, 13.0 * scale);
        font::text(&s, right - w, y, 13.0 * scale, TEXT_DIM);
        y -= 18.0 * scale;
    }
    // Current group: the same-time run ending at `count`.
    if count > 0 {
        let t_cur = frags[count - 1].time;
        let start = frags[..count]
            .iter()
            .rposition(|f| (f.time - t_cur).abs() >= 1e-4)
            .map(|i| i + 1)
            .unwrap_or(0);
        let cur: Vec<usize> = (start..count).collect();
        let s = format!("▶ {}", join(&cur));
        let w = font::text_width(&s, 15.0 * scale);
        font::text(&s, right - w, y, 15.0 * scale, ACCENT);
    }
}

/// Draw the lnmai-core score read-outs down the bottom-left of the pad panel.
pub fn draw_score_block(app: &PadPreviewState, rect: RectF, scale: f32) {
    // Without the core (no engine, or the `no_core` option) there is no score;
    // show a placeholder instead.
    if !app.use_core() {
        let margin = 14.0 * scale;
        font::text(
            "None",
            rect.x + margin,
            rect.y + rect.h - margin,
            16.0 * scale,
            ACCENT,
        );
        return;
    }

    let mut rows: Vec<String> = Vec::new();
    let state = app.combo_state_label();
    if state.is_empty() {
        rows.push(format!("COMBO   {}", app.combo()));
    } else {
        rows.push(format!("COMBO   {}  ({state})", app.combo()));
    }
    rows.push(format!("P-COMBO {}", app.p_combo()));
    if let Some(rates) = app.acc_rates() {
        for (label, value) in rates {
            rows.push(format!("{label:<7} {value:.4}%"));
        }
    }
    rows.push(format!("DX      {} / {}", app.dx_score(), app.max_dx_score()));
    rows.push(format!("FAST {}   LATE {}", app.fast_count(), app.late_count()));
    #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
    {
        let (perfect, great, good, miss) = app.grade_totals();
        rows.push(format!(
            "P {perfect}   GR {great}   GD {good}   M {miss}"
        ));
    }

    let line_h = 20.0 * scale;
    let margin = 14.0 * scale;
    let x = rect.x + margin;
    // Bottom-aligned block: the last row sits `margin` above the panel bottom.
    let mut y = rect.y + rect.h - margin;
    for row in rows.iter().rev() {
        font::text(row, x, y, 16.0 * scale, ACCENT);
        y -= line_h;
    }
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
