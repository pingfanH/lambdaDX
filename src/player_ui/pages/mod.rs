//! Page dispatch and the shared widget kit (buttons, top bar, sliders, toggles,
//! badges, backdrop and the page sweep).

pub mod gameplay;
pub mod pause;
pub mod settings;
pub mod song_select;
pub mod start;

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player_ui::anim;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::state::{Page, PlayerUiApp};
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

pub fn draw(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    backdrop(ctx);

    if app.shows_gameplay_background() {
        gameplay::draw_view(app, ctx, input);
    }

    match app.page {
        Page::Start => start::draw(app, input, ctx),
        Page::SongSelect => song_select::draw(app, input, ctx),
        Page::Settings => settings::draw(app, input, ctx),
        Page::Gameplay => gameplay::draw_hud(app, input, ctx),
        Page::Pause => {
            gameplay::draw_hud(app, input, ctx);
            pause::draw(app, input, ctx);
        }
    }

    if let Some(err) = app.error.clone() {
        toast(ctx, &err, theme::DANGER);
    }
}

/// The void background with a very faint dot grid and a slow diagonal hatch.
pub fn backdrop(ctx: &UiCtx) {
    draw::rect(
        RectF {
            x: 0.0,
            y: 0.0,
            w: ctx.w,
            h: ctx.h,
        },
        theme::VOID,
    );
    draw::dots(
        RectF {
            x: 0.0,
            y: 0.0,
            w: ctx.w,
            h: ctx.h,
        },
        28.0 * ctx.scale,
        draw::with_alpha(theme::GRID, 0.35),
    );
}

/// Masked page transition: a slanted edge sweeps left→right; everything it has
/// passed is the **next scene**, everything ahead of it is the **previous
/// scene** — neither scene translates. The outgoing scene is captured once into
/// `app.transition_tex` on the switch frame and re-drawn in place through a
/// slanted mask; a dark gradient + accent rim mark the moving seam.
pub fn draw_transition(app: &mut PlayerUiApp, ctx: &UiCtx) {
    let mut t = anim::progress(ctx.now, app.transition_at, 0.0, theme::D_SWEEP);
    // Dev harness: freeze the wipe at a fixed progress for deterministic shots.
    if let Ok(v) = std::env::var("MAI2_UI_TRANS_HOLD") {
        if let Ok(v) = v.parse::<f32>() {
            t = v;
        }
    }
    if t >= 1.0 {
        app.transition_tex = None;
        app.transition_snap = None;
        return;
    }
    let Some(tex) = &app.transition_tex else {
        return;
    };

    let e = anim::ease_in_out_cubic(t);
    // Slant matches the old sweep band (~9°).
    let skew = ctx.h * 0.16;
    // Leading (top) edge runs to `w + skew` so the trailing (bottom) edge also
    // leaves the screen at the end.
    let edge_top = e * (ctx.w + skew);
    let edge_bot = edge_top - skew;
    let xt = edge_top.clamp(0.0, ctx.w);
    let xb = edge_bot.clamp(0.0, ctx.w);
    if xt >= ctx.w && xb >= ctx.w {
        return; // fully revealed
    }

    // Previous scene, in place, visible only where `screen_x >= edge(y)`.
    // V is flipped: `get_screen_data` reads the framebuffer bottom-up.
    let mesh = Mesh {
        vertices: vec![
            Vertex::new(xt, 0.0, 0.0, xt / ctx.w, 1.0, WHITE),
            Vertex::new(ctx.w, 0.0, 0.0, 1.0, 1.0, WHITE),
            Vertex::new(ctx.w, ctx.h, 0.0, 1.0, 0.0, WHITE),
            Vertex::new(xb, ctx.h, 0.0, xb / ctx.w, 0.0, WHITE),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(tex.clone()),
    };
    draw_mesh(&mesh);

    // A slanted parallelogram with top-left at `a` and the given width.
    let slab = |a: f32, width: f32| -> [Vec2; 4] {
        [
            vec2(a, 0.0),
            vec2(a + width, 0.0),
            vec2(a + width - skew, ctx.h),
            vec2(a - skew, ctx.h),
        ]
    };

    // Dark gradient cast onto the incoming (left) scene, easing toward the seam.
    let grad_w = 34.0 * ctx.scale;
    let steps = 12;
    for i in 0..steps {
        let f = (i as f32 + 1.0) / steps as f32;
        let alpha = 0.42 * f * f;
        draw::fill_poly(
            &slab(edge_top - grad_w + (grad_w / steps as f32) * i as f32, grad_w / steps as f32),
            Color::new(0.0, 0.0, 0.0, alpha),
        );
    }
    // Accent rim on the seam.
    draw::fill_poly(&slab(edge_top, 2.5 * ctx.scale), theme::ACCENT);
}

/// Capture the outgoing scene into `app.transition_tex`. Must be called after
/// the page is drawn, on the frame the switch happens (the framebuffer still
/// holds the previous page then).
pub fn capture_transition(app: &mut PlayerUiApp, ctx: &UiCtx) {
    let t = anim::progress(ctx.now, app.transition_at, 0.0, theme::D_SWEEP);
    if t >= 1.0 || app.transition_snap == Some(app.transition_at) {
        return;
    }
    let img = crate::player_ui::perf::time("transition.readback", get_screen_data);
    app.transition_tex = Some(crate::player_ui::perf::time("transition.upload", || {
        Texture2D::from_image(&img)
    }));
    app.transition_snap = Some(app.transition_at);
}

/// A clipping-free bottom toast.
pub fn toast(ctx: &UiCtx, msg: &str, color: Color) {
    let size = 13.0 * ctx.scale;
    let w = draw::text_width(ctx.font.as_ref(), msg, size) + 28.0 * ctx.scale;
    let r = RectF {
        x: ctx.w * 0.5 - w * 0.5,
        y: ctx.h - 52.0 * ctx.scale,
        w,
        h: 32.0 * ctx.scale,
    };
    draw::cut_rect(r, 8.0, theme::PANEL_ALT, Some(color), 1.0);
    draw::text_centered(
        ctx.font.as_ref(),
        msg,
        r.x + r.w * 0.5,
        r.y + r.h * 0.5 + size * 0.35,
        size,
        color,
    );
}

// ── Widget kit ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Primary,
    Secondary,
    Quiet,
    Danger,
}

fn btn_colors(kind: Btn) -> (Color, Color, Color) {
    match kind {
        Btn::Primary => (theme::ACCENT, theme::VOID, theme::ACCENT),
        Btn::Secondary => (theme::RAISED, theme::TEXT, theme::BORDER),
        Btn::Quiet => (Color::new(0.0, 0.0, 0.0, 0.0), theme::TEXT_DIM, theme::BORDER),
        Btn::Danger => (theme::DANGER, theme::VOID, theme::DANGER),
    }
}

/// A cut-corner button. Returns true on click.
pub fn button(
    ctx: &UiCtx,
    input: &mut Input,
    name: &str,
    idx: usize,
    r: RectF,
    label: &str,
    kind: Btn,
) -> bool {
    let resp = input.widget(input::id(name, idx), r);
    let (fill, text_c, border) = btn_colors(kind);
    let fill = if resp.held {
        draw::shade(fill, 0.82)
    } else if resp.hovered {
        draw::mix(fill, Color::new(1.0, 1.0, 1.0, 1.0), 0.10)
    } else {
        fill
    };
    // Pressed widgets sink 1px (the sticker-sheet press-in, flattened).
    let y_off = if resp.held { 1.0 } else { 0.0 };
    let rr = RectF { y: r.y + y_off, ..r };
    let cut = (theme::CUT * 0.55) * ctx.scale;
    if kind != Btn::Quiet {
        draw::shadow_rect(rr, 2.0 * ctx.scale, 2.0 * ctx.scale, draw::with_alpha(Color::new(0.0, 0.0, 0.0, 1.0), 0.35));
    }
    draw::cut_rect(rr, cut, fill, Some(border), 1.0);
    // Accent left tick on hover for secondary/quiet.
    if resp.hovered && matches!(kind, Btn::Secondary | Btn::Quiet) {
        draw::rect(
            RectF {
                x: rr.x,
                y: rr.y,
                w: 2.0 * ctx.scale,
                h: rr.h,
            },
            theme::ACCENT,
        );
    }
    let size = 14.0 * ctx.scale;
    draw::text_centered(
        ctx.font.as_ref(),
        label,
        rr.x + rr.w * 0.5,
        rr.y + rr.h * 0.5 + size * 0.35,
        size,
        text_c,
    );
    resp.clicked
}

/// Overline label in accent, small and spaced.
pub fn overline(ctx: &UiCtx, s: &str, x: f32, y: f32) {
    draw::text(ctx.font.as_ref(), s, x, y, 11.0 * ctx.scale, theme::ACCENT);
}

/// Top bar chrome. Returns `(back_clicked, trailing_area)`.
pub fn top_bar(
    ctx: &UiCtx,
    input: &mut Input,
    back_label: &str,
    title: &str,
    subtitle: &str,
) -> (bool, RectF) {
    let bar_h = 64.0 * ctx.scale;
    let bar = RectF {
        x: 0.0,
        y: 0.0,
        w: ctx.w,
        h: bar_h,
    };
    draw::rect(bar, theme::PANEL);
    draw::rect(
        RectF {
            x: 0.0,
            y: bar_h - 1.0,
            w: ctx.w,
            h: 1.0,
        },
        theme::BORDER,
    );

    let pad = 20.0 * ctx.scale;
    let back = RectF {
        x: pad,
        y: 14.0 * ctx.scale,
        w: 84.0 * ctx.scale,
        h: 36.0 * ctx.scale,
    };
    let back_clicked = button(ctx, input, "top_back", 0, back, back_label, Btn::Quiet);

    let tx = back.x + back.w + 18.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        title,
        tx,
        bar_h * 0.5 + 4.0 * ctx.scale,
        22.0 * ctx.scale,
        theme::TEXT,
    );
    if !subtitle.is_empty() {
        let tw = draw::text_width(ctx.font.as_ref(), title, 22.0 * ctx.scale);
        draw::text(
            ctx.font.as_ref(),
            subtitle,
            tx + tw + 12.0 * ctx.scale,
            bar_h * 0.5 + 4.0 * ctx.scale,
            12.0 * ctx.scale,
            theme::TEXT_MUTED,
        );
    }

    let trailing = RectF {
        x: ctx.w - pad - 120.0 * ctx.scale,
        y: 14.0 * ctx.scale,
        w: 120.0 * ctx.scale,
        h: 36.0 * ctx.scale,
    };
    (back_clicked, trailing)
}

/// A labelled toggle row. Returns true if the value changed.
pub fn toggle_row(
    ctx: &UiCtx,
    input: &mut Input,
    name: &str,
    idx: usize,
    r: RectF,
    title: &str,
    desc: &str,
    value: &mut bool,
) -> bool {
    draw::text(ctx.font.as_ref(), title, r.x, r.y + 18.0 * ctx.scale, 15.0 * ctx.scale, theme::TEXT);
    if !desc.is_empty() {
        draw::text(ctx.font.as_ref(), desc, r.x, r.y + 38.0 * ctx.scale, 12.0 * ctx.scale, theme::TEXT_MUTED);
    }
    let box_r = RectF {
        x: r.x + r.w - 44.0 * ctx.scale,
        y: r.y + 8.0 * ctx.scale,
        w: 40.0 * ctx.scale,
        h: 22.0 * ctx.scale,
    };
    let resp = input.widget(input::id(name, idx), box_r);
    let changed = resp.clicked;
    if changed {
        *value = !*value;
    }
    let track = if *value { theme::ACCENT_DIM } else { theme::RAISED };
    draw::rect(box_r, track);
    draw::rect_outline(box_r, 1.0, if resp.hovered { theme::ACCENT } else { theme::BORDER });
    let knob_x = if *value {
        box_r.x + box_r.w - 11.0 * ctx.scale
    } else {
        box_r.x + 11.0 * ctx.scale
    };
    draw_circle(
        knob_x,
        box_r.y + box_r.h * 0.5,
        8.0 * ctx.scale,
        if *value { theme::ACCENT } else { theme::TEXT_MUTED },
    );
    changed
}

/// A labelled slider row. Returns true if the value changed.
pub fn slider_row(
    ctx: &UiCtx,
    input: &mut Input,
    name: &str,
    idx: usize,
    r: RectF,
    label: &str,
    value: &mut f32,
    min: f32,
    max: f32,
) -> bool {
    let size = 15.0 * ctx.scale;
    draw::text(ctx.font.as_ref(), label, r.x, r.y + 16.0 * ctx.scale, size, theme::TEXT);
    let value_txt = format!("{value:.2}");
    draw::text_right(
        ctx.font.as_ref(),
        &value_txt,
        r.x + r.w,
        r.y + 16.0 * ctx.scale,
        size,
        theme::ACCENT,
    );

    let track = RectF {
        x: r.x,
        y: r.y + 34.0 * ctx.scale,
        w: r.w,
        h: 6.0 * ctx.scale,
    };
    // Widen the hit area.
    let hit = RectF {
        y: r.y + 24.0 * ctx.scale,
        h: 26.0 * ctx.scale,
        ..track
    };
    let resp = input.widget(input::id(name, idx), hit);
    let mut changed = false;
    if (resp.held || resp.clicked) && hit.w > 0.0 {
        let frac = ((input.pos.x - track.x) / track.w).clamp(0.0, 1.0);
        let nv = min + (max - min) * frac;
        if (nv - *value).abs() > f32::EPSILON {
            *value = nv;
            changed = true;
        }
    }
    let frac = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    draw::rect(track, theme::RAISED);
    draw::rect(
        RectF {
            w: track.w * frac,
            ..track
        },
        theme::ACCENT,
    );
    let knob = RectF {
        x: track.x + track.w * frac - 6.0 * ctx.scale,
        y: track.y - 5.0 * ctx.scale,
        w: 12.0 * ctx.scale,
        h: 16.0 * ctx.scale,
    };
    draw::rect(knob, if resp.hovered || resp.held { theme::ACCENT } else { theme::TEXT });
    changed
}

/// Hexagon badge with a glyph — the level/mode stamp.
pub fn badge(ctx: &UiCtx, center: Vec2, radius: f32, glyph: &str, color: Color) {
    let mut pts = Vec::new();
    for i in 0..6 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 6.0;
        pts.push(vec2(center.x + a.cos() * radius, center.y + a.sin() * radius));
    }
    draw::fill_poly(&pts, theme::PANEL_ALT);
    draw::polyline(&pts, 2.0 * ctx.scale, color);
    let size = radius * 0.9;
    draw::text_centered(
        ctx.font.as_ref(),
        glyph,
        center.x,
        center.y + size * 0.35,
        size,
        color,
    );
}

/// Progress for a list item revealed at index `i` under the current page.
pub fn item_reveal(app: &PlayerUiApp, ctx: &UiCtx, i: usize) -> (f32, f32) {
    let t = anim::progress(
        ctx.now,
        app.page_born,
        i as f64 * theme::STAGGER,
        theme::D_CARD,
    );
    let e = anim::ease_out_cubic(t);
    // (opacity, y-offset)
    (e, (1.0 - e) * 10.0 * ctx.scale)
}
