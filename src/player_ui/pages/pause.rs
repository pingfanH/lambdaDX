//! Pause overlay: dimmed backdrop plus a spring-popped panel.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player_ui::anim;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::PlayerUiApp;
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

pub fn draw(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    // Dim.
    draw::rect(
        RectF {
            x: 0.0,
            y: 0.0,
            w: ctx.w,
            h: ctx.h,
        },
        Color::new(0.0, 0.0, 0.0, 0.72),
    );

    let t = anim::progress(ctx.now, app.page_born, 0.0, theme::D_POP);
    let pop = anim::ease_out_back(t);
    let alpha = anim::ease_out_cubic(t);

    let pw = (ctx.w * 0.86).min(420.0 * ctx.scale);
    let ph = 400.0 * ctx.scale;
    let cx = ctx.w * 0.5;
    let cy = ctx.h * 0.5;
    // Scale about the centre for the pop.
    let w = pw * (0.9 + 0.1 * pop);
    let h = ph * (0.9 + 0.1 * pop);
    let panel = RectF {
        x: cx - w * 0.5,
        y: cy - h * 0.5,
        w,
        h,
    };
    // Offset accent shadow (the sticker depth, flattened).
    draw::shadow_rect(
        panel,
        6.0 * ctx.scale * pop,
        6.0 * ctx.scale * pop,
        draw::with_alpha(Color::new(0.0, 0.0, 0.0, 1.0), 0.5 * alpha),
    );
    draw::cut_rect(
        panel,
        theme::CUT * ctx.scale,
        draw::with_alpha(theme::PANEL, alpha),
        Some(draw::with_alpha(theme::BORDER, alpha)),
        1.0,
    );

    let x = panel.x + 28.0 * ctx.scale;
    let ww = panel.w - 56.0 * ctx.scale;
    let mut y = panel.y + 38.0 * ctx.scale;

    draw::text(
        ctx.font.as_ref(),
        "PLAY SESSION",
        x,
        y,
        11.0 * ctx.scale,
        draw::with_alpha(theme::ACCENT, alpha),
    );
    y += 34.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        "已暂停",
        x,
        y,
        30.0 * ctx.scale,
        draw::with_alpha(theme::TEXT, alpha),
    );
    y += 24.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        &app.pad.chart.title,
        x,
        y,
        14.0 * ctx.scale,
        draw::with_alpha(theme::TEXT_DIM, alpha),
    );
    y += 20.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        &format!("当前时间  {}", format_time(app.pad.mode_song_offset)),
        x,
        y,
        12.0 * ctx.scale,
        draw::with_alpha(theme::TEXT_MUTED, alpha),
    );
    y += 34.0 * ctx.scale;

    let bh = 46.0 * ctx.scale;
    let gap = 10.0 * ctx.scale;
    let items: [(&str, Btn); 4] = [
        ("继续游玩", Btn::Primary),
        ("重新开始", Btn::Secondary),
        ("游玩设置", Btn::Quiet),
        ("退出到选歌", Btn::Danger),
    ];
    for (i, (label, kind)) in items.iter().enumerate() {
        let r = RectF {
            x,
            y,
            w: ww,
            h: bh,
        };
        if pages::button(ctx, input, "pause_btn", i, r, label, *kind) {
            match i {
                0 => app.resume(),
                1 => app.restart(),
                2 => app.open_settings(),
                _ => app.exit_to_select(),
            }
            return;
        }
        y += bh + gap;
    }
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
