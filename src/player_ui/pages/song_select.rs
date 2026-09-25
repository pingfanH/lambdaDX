//! Song select: a staggered, sliding song list beside a cover/detail panel.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player_ui::anim;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::{Page, PlayerUiApp};
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

const ROW_H: f32 = 66.0;

pub fn draw(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    if app.loaded.is_none() && !app.loading_song && !app.library.songs.is_empty() {
        let idx = app.selected.min(app.library.songs.len() - 1);
        let _ = app.request_load_song(idx);
    }

    let (back, trailing) = pages::top_bar(
        ctx,
        input,
        "返回",
        "选择歌曲",
        &format!("{} 首曲目", app.library.songs.len()),
    );
    let tw = 90.0 * ctx.scale;
    let settings_btn = RectF {
        x: trailing.x + trailing.w - tw,
        w: tw,
        ..trailing
    };
    if pages::button(ctx, input, "ss_settings", 0, settings_btn, "设置", Btn::Quiet) {
        app.open_settings();
    }
    if back {
        app.go(Page::Start);
        return;
    }
    if is_key_pressed(KeyCode::Escape) {
        app.go(Page::Start);
        return;
    }

    draw::rect(
        RectF {
            x: 0.0,
            y: 64.0 * ctx.scale,
            w: ctx.w,
            h: 1.0,
        },
        theme::BORDER,
    );

    let wide = ctx.w >= 900.0 * ctx.scale;
    if wide {
        let list_w = (420.0 * ctx.scale).min(ctx.w * 0.44);
        draw_list(app, input, ctx, list_w, ctx.h);
        draw_detail(app, input, ctx, list_w);
    } else {
        let list_h = (ctx.h - 64.0 * ctx.scale) * 0.52;
        draw_list(app, input, ctx, ctx.w, list_h);
        let area = RectF {
            x: 0.0,
            y: list_h,
            w: ctx.w,
            h: ctx.h - list_h,
        };
        draw_detail_in(app, input, ctx, area);
    }
}

fn draw_list(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, list_w: f32, list_h: f32) {
    let bar_h = 64.0 * ctx.scale;
    let panel = RectF {
        x: 0.0,
        y: bar_h,
        w: list_w,
        h: list_h - bar_h,
    };
    draw::rect(panel, theme::PANEL);
    draw::hatch(panel, 22.0 * ctx.scale, -0.13, draw::with_alpha(theme::GRID, 0.4));
    let edge = RectF {
        x: list_w - 1.0,
        y: bar_h,
        w: 1.0,
        h: list_h - bar_h,
    };
    if list_w < ctx.w {
        draw::rect(edge, theme::BORDER);
    }

    let header_h = 92.0 * ctx.scale;
    let header = RectF {
        x: 0.0,
        y: bar_h,
        w: list_w,
        h: header_h,
    };

    // Watermark behind the list.
    draw::text(
        ctx.font.as_ref(),
        "SELECT",
        panel.x + 16.0 * ctx.scale,
        panel.y + panel.h - 18.0 * ctx.scale,
        64.0 * ctx.scale,
        draw::with_alpha(theme::BORDER_SOFT, 0.5),
    );

    let view = RectF {
        x: 0.0,
        y: bar_h + header_h,
        w: list_w,
        h: list_h - bar_h - header_h,
    };

    let row_total = ROW_H * ctx.scale + 10.0 * ctx.scale;
    let content_h = app.library.songs.len() as f32 * row_total + 16.0 * ctx.scale;
    let max_scroll = (content_h - view.h).max(0.0);
    app.list_scroll_target =
        (app.list_scroll_target - input.scroll(view) * 40.0 * ctx.scale).clamp(0.0, max_scroll);
    app.list_scroll = anim::approach(&mut app.list_scroll, app.list_scroll_target, 18.0, ctx.dt);

    let count = app.library.songs.len();
    for i in 0..count {
        let y = view.y + 8.0 * ctx.scale + i as f32 * row_total - app.list_scroll;
        if y + row_total < view.y - 4.0 || y > view.y + view.h + 4.0 {
            continue;
        }
        draw_row(app, input, ctx, i, y, list_w);
    }

    // Header mask (covers rows scrolled behind it).
    draw::rect(header, theme::PANEL);
    draw::text(
        ctx.font.as_ref(),
        "曲库",
        header.x + 20.0 * ctx.scale,
        header.y + 34.0 * ctx.scale,
        12.0 * ctx.scale,
        theme::ACCENT,
    );
    draw::text(
        ctx.font.as_ref(),
        "选择一首歌开始",
        header.x + 20.0 * ctx.scale,
        header.y + 62.0 * ctx.scale,
        20.0 * ctx.scale,
        theme::TEXT,
    );
}

fn draw_row(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, i: usize, y: f32, list_w: f32) {
    let (alpha, dy) = pages::item_reveal(app, ctx, i);
    if alpha <= 0.01 {
        return;
    }
    let selected = app.loaded == Some(i);
    let row = RectF {
        x: 16.0 * ctx.scale,
        y: y + dy,
        w: list_w - 32.0 * ctx.scale,
        h: ROW_H * ctx.scale,
    };
    let resp = input.widget(input::id("songrow", i), row);

    let slide = if selected {
        theme::SELECT_SLIDE * ctx.scale
    } else if resp.hovered {
        theme::HOVER_SLIDE * ctx.scale
    } else {
        0.0
    };
    let rr = RectF {
        x: row.x + slide,
        ..row
    };

    let fill = if selected {
        theme::PANEL_ALT
    } else if resp.hovered {
        theme::RAISED
    } else {
        theme::PANEL
    };
    draw::cut_rect(
        rr,
        10.0 * ctx.scale,
        draw::with_alpha(fill, alpha),
        Some(if selected {
            theme::ACCENT
        } else if resp.hovered {
            theme::BORDER
        } else {
            theme::BORDER_SOFT
        }),
        1.0,
    );
    if selected || resp.hovered {
        draw::rect(
            RectF {
                x: rr.x,
                y: rr.y,
                w: 3.0 * ctx.scale,
                h: rr.h,
            },
            theme::ACCENT,
        );
    }

    // Cover thumb.
    let thumb = RectF {
        x: rr.x + 10.0 * ctx.scale,
        y: rr.y + 10.0 * ctx.scale,
        w: 46.0 * ctx.scale,
        h: 46.0 * ctx.scale,
    };
    draw_cover(app, ctx, i, thumb, alpha);

    let song = &app.library.songs[i];
    let tx = thumb.x + thumb.w + 12.0 * ctx.scale;
    let title_size = 15.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        &song.title,
        tx,
        rr.y + 30.0 * ctx.scale,
        title_size,
        draw::with_alpha(theme::TEXT, alpha),
    );
    draw::text(
        ctx.font.as_ref(),
        &song.artist,
        tx,
        rr.y + 49.0 * ctx.scale,
        12.0 * ctx.scale,
        draw::with_alpha(theme::TEXT_MUTED, alpha),
    );

    if selected {
        let badge_c = vec2(rr.x + rr.w - 22.0 * ctx.scale, rr.y + rr.h * 0.5);
        pages::badge(ctx, badge_c, 12.0 * ctx.scale, "▶", theme::ACCENT);
    }

    if resp.clicked && !selected {
        if let Err(e) = app.request_load_song(i) {
            app.error = Some(e);
        }
        app.list_scroll_target = app.list_scroll;
    }
}

fn draw_detail(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, list_w: f32) {
    let area = RectF {
        x: list_w,
        y: 64.0 * ctx.scale,
        w: ctx.w - list_w,
        h: ctx.h - 64.0 * ctx.scale,
    };
    draw_detail_in(app, input, ctx, area);
}

fn draw_detail_in(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, area: RectF) {
    let cx = area.x + area.w * 0.5;
    let mut y = area.y + 34.0 * ctx.scale;

    let cover_size = (area.w.min(area.h) * 0.42).min(300.0 * ctx.scale);
    let cover = RectF {
        x: cx - cover_size * 0.5,
        y,
        w: cover_size,
        h: cover_size,
    };
    // The selected song's art always decodes eagerly; the list fills lazily.
    app.library.ensure_cover(app.selected);
    draw_cover(app, ctx, app.selected, cover, 1.0);
    y += cover_size + 22.0 * ctx.scale;

    let song = app.library.songs.get(app.selected);
    let (title, artist, designer, descriptor) = match song {
        Some(s) => (
            s.title.clone(),
            s.artist.clone(),
            s.designer.clone(),
            s.descriptor.clone(),
        ),
        None => ("未命名歌曲".into(), String::new(), String::new(), String::new()),
    };

    draw::text_centered(ctx.font.as_ref(), "NOW SELECTING", cx, y, 11.0 * ctx.scale, theme::ACCENT);
    y += 26.0 * ctx.scale;
    draw::text_centered(ctx.font.as_ref(), &title, cx, y, 26.0 * ctx.scale, theme::TEXT);
    y += 24.0 * ctx.scale;
    if !artist.is_empty() {
        draw::text_centered(ctx.font.as_ref(), &artist, cx, y, 14.0 * ctx.scale, theme::TEXT_DIM);
    }
    y += 22.0 * ctx.scale;
    let meta = if designer.is_empty() {
        descriptor
    } else {
        format!("谱师 {designer} · {descriptor}")
    };
    draw::text_centered(ctx.font.as_ref(), &meta, cx, y, 12.0 * ctx.scale, theme::TEXT_MUTED);
    y += 30.0 * ctx.scale;

    // Difficulty pills.
    let levels = app.levels.clone();
    if levels.is_empty() {
        draw::text_centered(
            ctx.font.as_ref(),
            "使用当前谱面",
            cx,
            y,
            13.0 * ctx.scale,
            theme::TEXT_MUTED,
        );
        y += 30.0 * ctx.scale;
    } else {
        let pill_w = 78.0 * ctx.scale;
        let pill_h = 38.0 * ctx.scale;
        let gap = 8.0 * ctx.scale;
        let total = levels.len() as f32 * pill_w + (levels.len() as f32 - 1.0) * gap;
        let mut px = cx - total * 0.5;
        for (ki, (key, display)) in levels.iter().enumerate() {
            let pr = RectF {
                x: px,
                y,
                w: pill_w,
                h: pill_h,
            };
            let selected = app.selected_level == Some(*key);
            let resp = input.widget(input::id("level", ki), pr);
            let fill = if selected {
                theme::ACCENT
            } else if resp.hovered {
                theme::RAISED_HOVER
            } else {
                theme::RAISED
            };
            draw::cut_rect(
                pr,
                8.0 * ctx.scale,
                fill,
                Some(if selected { theme::ACCENT } else { theme::BORDER }),
                1.0,
            );
            let label = if display.is_empty() {
                format!("{key}")
            } else {
                format!("Lv.{display}")
            };
            draw::text_centered(
                ctx.font.as_ref(),
                &label,
                pr.x + pr.w * 0.5,
                pr.y + pr.h * 0.5 + 5.0 * ctx.scale,
                15.0 * ctx.scale,
                if selected { theme::VOID } else { theme::TEXT },
            );
            if resp.clicked && !selected {
                if let Err(e) = app.select_level(*key) {
                    app.error = Some(e);
                }
            }
            px += pill_w + gap;
        }
        y += pill_h + 22.0 * ctx.scale;
    }

    let bw = (area.w * 0.7).min(320.0 * ctx.scale);
    let by = y + 34.0 * ctx.scale;
    let br = RectF {
        x: cx - bw * 0.5,
        y: by,
        w: bw,
        h: 50.0 * ctx.scale,
    };
    if app.loading_song {
        draw::text_centered(
            ctx.font.as_ref(),
            "载入谱面…",
            cx,
            y,
            13.0 * ctx.scale,
            theme::ACCENT,
        );
        draw::cut_rect(br, 8.0 * ctx.scale, theme::RAISED, Some(theme::BORDER), 1.0);
        draw::text_centered(
            ctx.font.as_ref(),
            "载入中…",
            br.x + br.w * 0.5,
            br.y + br.h * 0.5 + 5.0 * ctx.scale,
            14.0 * ctx.scale,
            theme::TEXT_MUTED,
        );
    } else {
        draw::text_centered(
            ctx.font.as_ref(),
            &format!("{} notes · {:.0} BPM", app.pad.chart.notes.len(), app.pad.chart.bpm),
            cx,
            y,
            13.0 * ctx.scale,
            theme::TEXT_DIM,
        );
        if pages::button(ctx, input, "ss_play", 0, br, "开始游玩", Btn::Primary) {
            if let Err(e) = app.begin_gameplay() {
                app.error = Some(e);
            }
        }
    }
}

/// Draw a cover, letterboxed inside `box`.
fn draw_cover(app: &PlayerUiApp, ctx: &UiCtx, index: usize, box_r: RectF, alpha: f32) {
    let tex = app.library.covers.get(index).and_then(|t| t.as_ref());
    match tex {
        Some(tex) => {
            let (tw, th) = (tex.width(), tex.height());
            let aspect = if th > 0.0 { tw / th } else { 1.0 };
            let (w, h) = if aspect >= 1.0 {
                (box_r.w, box_r.w / aspect)
            } else {
                (box_r.h * aspect, box_r.h)
            };
            let dst = RectF {
                x: box_r.x + (box_r.w - w) * 0.5,
                y: box_r.y + (box_r.h - h) * 0.5,
                w,
                h,
            };
            draw_texture_ex(
                tex,
                dst.x,
                dst.y,
                draw::with_alpha(Color::new(1.0, 1.0, 1.0, 1.0), alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(dst.w, dst.h)),
                    ..Default::default()
                },
            );
            draw::rect_outline(dst, 1.0, theme::BORDER);
        }
        None => {
            draw::cut_rect(
                box_r,
                8.0 * ctx.scale,
                theme::PANEL_ALT,
                Some(theme::BORDER_SOFT),
                1.0,
            );
            pages::badge(
                ctx,
                vec2(box_r.x + box_r.w * 0.5, box_r.y + box_r.h * 0.5),
                box_r.w.min(box_r.h) * 0.24,
                "♪",
                theme::TEXT_MUTED,
            );
        }
    }
}
