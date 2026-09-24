//! Pad-preview HUD: a header strip with the playback progress bar (same style
//! as the player UI), the elapsed time and an AUTO toggle.
//!
//! Pointer handling is done with macroquad directly; [`handle_input`] reports
//! whether the mouse was consumed so the pad's own touch routing can ignore it.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player::font;
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
    if app.scrubbing {
        let (mx, _) = mouse_position();
        ((mx - bar.x) / bar.w).clamp(0.0, 1.0)
    } else {
        (app.song_time() / dur).clamp(0.0, 1.0)
    }
}

/// Handle progress-bar scrubbing and the AUTO button. Returns `true` when the
/// mouse should be ignored by the pad's touch routing.
pub fn handle_input(app: &mut PadPreviewState, header: RectF, scale: f32) -> bool {
    let (bar, auto_r) = geometry(header, scale);
    let (mx, my) = mouse_position();
    let pos = vec2(mx, my);
    let pressed = is_mouse_button_pressed(MouseButton::Left);

    if pressed && contains(auto_r, pos) {
        let on = !app.autoplay;
        crate::player::autoplay::set_on(app, on);
        return true;
    }

    let hit = RectF {
        x: bar.x,
        y: bar.y - 10.0 * scale,
        w: bar.w,
        h: bar.h + 20.0 * scale,
    };
    if pressed && contains(hit, pos) {
        app.scrubbing = true;
        app.stop_audio_if_any();
    }
    if app.scrubbing {
        let dur = app.song_duration();
        let t = ((pos.x - bar.x) / bar.w).clamp(0.0, 1.0) * dur;
        if is_mouse_button_released(MouseButton::Left) {
            app.seek_to(t);
            app.scrubbing = false;
        } else if is_mouse_button_down(MouseButton::Left) {
            app.scrub_to(t);
        }
        return true;
    }
    false
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

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
