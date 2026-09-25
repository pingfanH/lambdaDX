//! Start screen: identity, a vector hero mark and the two entry actions.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player_ui::anim;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::{Page, PlayerUiApp};
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

pub fn draw(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    let wide = ctx.w >= 780.0 * ctx.scale;
    if wide {
        let half = ctx.w * 0.5;
        draw_copy(app, input, ctx, RectF { x: 0.0, y: 0.0, w: half, h: ctx.h });
        draw_hero(ctx, RectF { x: half, y: 0.0, w: half, h: ctx.h });
    } else {
        draw_copy(
            app,
            input,
            ctx,
            RectF { x: 0.0, y: 0.0, w: ctx.w, h: ctx.h * 0.62 },
        );
        draw_hero(
            ctx,
            RectF {
                x: 0.0,
                y: ctx.h * 0.62,
                w: ctx.w,
                h: ctx.h * 0.38,
            },
        );
    }
}

fn draw_copy(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, area: RectF) {
    let pad = 56.0 * ctx.scale;
    let x = area.x + pad;
    let mut y = area.y + area.h * 0.26;

    // Pop-in for the title block.
    let t = anim::progress(ctx.now, app.page_born, 0.0, theme::D_POP);
    let pop = anim::ease_out_back(t);
    let alpha = anim::ease_out_cubic(t).min(1.0);

    let over = draw::with_alpha(theme::ACCENT, alpha);
    draw::text(ctx.font.as_ref(), "ARCADE CHART PLAYER", x, y, 12.0 * ctx.scale, over);
    y += 46.0 * ctx.scale;

    let title_size = if area.w < 560.0 * ctx.scale {
        44.0 * ctx.scale
    } else {
        58.0 * ctx.scale
    };
    // Offset accent copy = flat "ink" shadow behind the wordmark.
    let off = 4.0 * ctx.scale * pop;
    let title = "LambdaDX";
    draw_text_ex(
        title,
        x + off,
        y + off,
        draw::text_params(
            ctx.font.as_ref(),
            title_size,
            draw::with_alpha(theme::ACCENT_DIM, alpha),
        ),
    );
    draw_text_ex(
        title,
        x,
        y,
        draw::text_params(ctx.font.as_ref(), title_size, draw::with_alpha(theme::TEXT, alpha)),
    );
    y += 34.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        "P L A Y E R",
        x + 2.0 * ctx.scale,
        y,
        16.0 * ctx.scale,
        draw::with_alpha(theme::ACCENT, alpha),
    );
    y += 34.0 * ctx.scale;

    draw::text(
        ctx.font.as_ref(),
        "把节拍变成动作。选择谱面，设定难度，进入你的下一局。",
        x,
        y,
        15.0 * ctx.scale,
        draw::with_alpha(theme::TEXT_DIM, alpha),
    );
    y += 44.0 * ctx.scale;

    let bw = (area.w - pad * 2.0).min(360.0 * ctx.scale);
    let bh = 50.0 * ctx.scale;
    let b1 = RectF { x, y, w: bw, h: bh };
    if pages::button(ctx, input, "start_play", 0, b1, "开始 · 选歌", Btn::Primary) {
        if !app.library.songs.is_empty() {
            let idx = app.selected.min(app.library.songs.len() - 1);
            match app.request_load_song(idx) {
                Ok(()) => app.go(Page::SongSelect),
                Err(e) => app.error = Some(e),
            }
        } else {
            app.error = Some("曲库为空".to_string());
        }
    }
    y += bh + 12.0 * ctx.scale;

    let b2 = RectF { x, y, w: bw, h: bh };
    if pages::button(ctx, input, "start_settings", 0, b2, "设置", Btn::Secondary) {
        app.open_settings();
    }
    y += bh + 28.0 * ctx.scale;

    draw::text(
        ctx.font.as_ref(),
        &format!("曲库 · {} 首", app.library.songs.len()),
        x,
        y,
        12.0 * ctx.scale,
        theme::TEXT_MUTED,
    );
    y += 18.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        &app.library.root.display().to_string(),
        x,
        y,
        11.0 * ctx.scale,
        theme::TEXT_MUTED,
    );
}

/// A slowly turning calibration ring — the Blender-adjacent hero mark.
fn draw_hero(ctx: &UiCtx, area: RectF) {
    let c = vec2(area.x + area.w * 0.5, area.y + area.h * 0.5);
    let r = area.w.min(area.h) * 0.30;
    let t = ctx.now as f32;

    // Outer ring + inner ring.
    draw_circle_lines(c.x, c.y, r, 1.0, theme::BORDER);
    draw_circle_lines(c.x, c.y, r * 0.72, 1.0, theme::BORDER_SOFT);
    draw_circle(c.x, c.y, r * 0.60, theme::PANEL);

    // 24 ticks, rotating slowly.
    let rot = t * 0.15;
    for i in 0..24 {
        let a = rot + i as f32 * std::f32::consts::TAU / 24.0;
        let long = i % 6 == 0;
        let r0 = r * if long { 0.92 } else { 0.96 };
        let r1 = r;
        let col = if long { theme::ACCENT } else { theme::BORDER };
        draw_line(
            c.x + a.cos() * r0,
            c.y + a.sin() * r0,
            c.x + a.cos() * r1,
            c.y + a.sin() * r1,
            if long { 2.0 } else { 1.0 },
            col,
        );
    }

    // Counter-rotating sweep arc.
    let sweep = -t * 0.6;
    let mut prev: Option<Vec2> = None;
    for i in 0..=48 {
        let a = sweep + i as f32 * 0.09;
        let p = vec2(c.x + a.cos() * r * 0.84, c.y + a.sin() * r * 0.84);
        if let Some(q) = prev {
            draw_line(q.x, q.y, p.x, p.y, 2.0, theme::ACCENT);
        }
        prev = Some(p);
    }

    pages::badge(ctx, c, r * 0.34, "DX", theme::ACCENT);}
