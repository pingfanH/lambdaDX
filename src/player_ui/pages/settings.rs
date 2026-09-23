//! Settings: audio / gameplay / display, split into a category column and a
//! detail column.

use macroquad::prelude::*;

use crate::app::types::RectF;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::PlayerUiApp;
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

const CATS: [(&str, &str); 3] = [
    ("音频", "音乐与判定音效"),
    ("游玩", "速度与辅助"),
    ("显示", "布局与视觉"),
];

pub fn draw(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    // Opaque background: settings never shows the pad/game behind it.
    draw::rect(
        RectF {
            x: 0.0,
            y: 0.0,
            w: ctx.w,
            h: ctx.h,
        },
        theme::VOID,
    );

    let back_label = if app.settings_return == crate::player_ui::state::Page::Pause {
        "返回暂停"
    } else {
        "返回"
    };
    let (back, trailing) = pages::top_bar(ctx, input, back_label, "设置", "PLAYER SETTINGS");
    let tw = 96.0 * ctx.scale;
    let done = RectF {
        x: trailing.x + trailing.w - tw,
        w: tw,
        ..trailing
    };
    if pages::button(ctx, input, "set_done", 0, done, "完成", Btn::Primary) {
        app.close_settings();
        return;
    }
    if back {
        app.close_settings();
        return;
    }
    if is_key_pressed(KeyCode::Escape) {
        app.close_settings();
        return;
    }

    let wide = ctx.w >= 820.0 * ctx.scale;
    if wide {
        let cat_w = (240.0 * ctx.scale).min(ctx.w * 0.32);
        draw_categories(app, input, ctx, cat_w);
        let area = RectF {
            x: cat_w,
            y: 64.0 * ctx.scale,
            w: ctx.w - cat_w,
            h: ctx.h - 64.0 * ctx.scale,
        };
        draw_panel(app, input, ctx, area);
    } else {
        draw_categories(app, input, ctx, ctx.w);
        let area = RectF {
            x: 0.0,
            y: 64.0 * ctx.scale + 88.0 * ctx.scale,
            w: ctx.w,
            h: ctx.h - 64.0 * ctx.scale - 88.0 * ctx.scale,
        };
        draw_panel(app, input, ctx, area);
    }
}

fn draw_categories(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, w: f32) {
    let bar_h = 64.0 * ctx.scale;
    let panel = RectF {
        x: 0.0,
        y: bar_h,
        w,
        h: if w < ctx.w { ctx.h - bar_h } else { 88.0 * ctx.scale },
    };
    draw::rect(panel, theme::PANEL);

    if w >= ctx.w {
        // Horizontal row of categories.
        let each = (ctx.w - 24.0 * ctx.scale) / 3.0;
        for (i, (label, _)) in CATS.iter().enumerate() {
            let r = RectF {
                x: 12.0 * ctx.scale + i as f32 * each,
                y: bar_h + 14.0 * ctx.scale,
                w: each - 8.0 * ctx.scale,
                h: 60.0 * ctx.scale,
            };
            draw_cat(app, input, ctx, i, r, label);
        }
    } else {
        draw::text(
            ctx.font.as_ref(),
            "PLAYER SETTINGS",
            panel.x + 20.0 * ctx.scale,
            panel.y + 30.0 * ctx.scale,
            11.0 * ctx.scale,
            theme::ACCENT,
        );
        let mut y = panel.y + 48.0 * ctx.scale;
        for (i, (label, desc)) in CATS.iter().enumerate() {
            let r = RectF {
                x: 16.0 * ctx.scale,
                y,
                w: w - 32.0 * ctx.scale,
                h: 60.0 * ctx.scale,
            };
            let selected = app.settings_section == i;
            let resp = input.widget(input::id("cat", i), r);
            let fill = if selected {
                theme::RAISED
            } else if resp.hovered {
                theme::PANEL_ALT
            } else {
                theme::PANEL
            };
            draw::cut_rect(
                r,
                8.0 * ctx.scale,
                fill,
                Some(if selected { theme::ACCENT } else { theme::BORDER_SOFT }),
                1.0,
            );
            if selected {
                draw::rect(
                    RectF {
                        x: r.x,
                        y: r.y,
                        w: 3.0 * ctx.scale,
                        h: r.h,
                    },
                    theme::ACCENT,
                );
            }
            draw::text(
                ctx.font.as_ref(),
                label,
                r.x + 14.0 * ctx.scale,
                r.y + 26.0 * ctx.scale,
                15.0 * ctx.scale,
                theme::TEXT,
            );
            draw::text(
                ctx.font.as_ref(),
                desc,
                r.x + 14.0 * ctx.scale,
                r.y + 46.0 * ctx.scale,
                11.0 * ctx.scale,
                theme::TEXT_MUTED,
            );
            if resp.clicked {
                app.settings_section = i;
            }
            y += 68.0 * ctx.scale;
        }
    }
}

fn draw_cat(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, i: usize, r: RectF, label: &str) {
    let selected = app.settings_section == i;
    let resp = input.widget(input::id("cat", i), r);
    let fill = if selected {
        theme::ACCENT_DIM
    } else if resp.hovered {
        theme::RAISED
    } else {
        theme::PANEL_ALT
    };
    draw::cut_rect(r, 8.0 * ctx.scale, fill, Some(theme::BORDER), 1.0);
    draw::text_centered(
        ctx.font.as_ref(),
        label,
        r.x + r.w * 0.5,
        r.y + r.h * 0.5 + 5.0 * ctx.scale,
        15.0 * ctx.scale,
        if selected { theme::ACCENT } else { theme::TEXT },
    );
    if resp.clicked {
        app.settings_section = i;
    }
}

fn draw_panel(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, area: RectF) {
    let x = area.x + 32.0 * ctx.scale;
    let w = (area.w - 64.0 * ctx.scale).min(560.0 * ctx.scale);
    let mut y = area.y + 30.0 * ctx.scale;

    pages::overline(ctx, "设置项", x, y);
    y += 30.0 * ctx.scale;
    let heading = match app.settings_section {
        0 => "音频",
        1 => "游玩体验",
        _ => "显示与布局",
    };
    draw::text(ctx.font.as_ref(), heading, x, y, 26.0 * ctx.scale, theme::TEXT);
    y += 34.0 * ctx.scale;

    match app.settings_section {
        0 => draw_audio(app, input, ctx, x, w, &mut y),
        1 => draw_gameplay(app, input, ctx, x, w, &mut y),
        _ => draw_display(app, input, ctx, x, w, &mut y),
    }

    y += 20.0 * ctx.scale;
    draw::rect(
        RectF {
            x,
            y,
            w,
            h: 1.0,
        },
        theme::BORDER_SOFT,
    );
    y += 18.0 * ctx.scale;
    let rb = RectF {
        x,
        y,
        w: 180.0 * ctx.scale,
        h: 42.0 * ctx.scale,
    };
    if pages::button(ctx, input, "set_reset", 0, rb, "恢复默认", Btn::Quiet) {
        app.pad.audio_enabled = true;
        app.pad.note_speed = 7.5;
        app.pad.touch_speed = 7.5;
        app.pad.slide_fade_in = 3.926_913 / 7.5;
        app.pad.set_play_speed(1.0);
        app.pad.ui_scale_override = None;
        app.pad.mobile_ui = false;
        app.status = "已恢复默认设置".to_string();
    }
    y += 56.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        &app.status,
        x,
        y,
        12.0 * ctx.scale,
        theme::TEXT_MUTED,
    );
}

fn draw_audio(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32) {
    let r = RectF {
        x,
        y: *y,
        w,
        h: 52.0 * ctx.scale,
    };
    pages::toggle_row(
        ctx,
        input,
        "set_audio",
        0,
        r,
        "音乐与判定音效",
        "关闭后游玩仍会继续，但不会播放声音。",
        &mut app.pad.audio_enabled,
    );
    *y += 72.0 * ctx.scale;

    draw::text(
        ctx.font.as_ref(),
        &format!(
            "当前音源  {}",
            app.pad.audio_source_name.as_deref().unwrap_or("(无)")
        ),
        x,
        *y,
        12.0 * ctx.scale,
        theme::TEXT_MUTED,
    );
    *y += 24.0 * ctx.scale;
}

fn draw_gameplay(
    app: &mut PlayerUiApp,
    input: &mut Input,
    ctx: &UiCtx,
    x: f32,
    w: f32,
    y: &mut f32,
) {
    let mut note_speed = app.pad.note_speed;
    let r = RectF { x, y: *y, w, h: 56.0 * ctx.scale };
    if pages::slider_row(ctx, input, "set_note_speed", 0, r, "流速 (Note Speed)", &mut note_speed, 5.0, 10.0)
    {
        app.pad.note_speed = note_speed;
    }
    *y += 72.0 * ctx.scale;

    let mut touch_speed = app.pad.touch_speed;
    let r = RectF { x, y: *y, w, h: 56.0 * ctx.scale };
    if pages::slider_row(ctx, input, "set_touch_speed", 0, r, "Touch 流速", &mut touch_speed, 5.0, 10.0) {
        app.pad.touch_speed = touch_speed;
    }
    *y += 72.0 * ctx.scale;

    let mut fade = app.pad.slide_fade_in;
    let r = RectF { x, y: *y, w, h: 56.0 * ctx.scale };
    if pages::slider_row(ctx, input, "set_fade", 0, r, "Slide 提前量 (秒)", &mut fade, 0.2, 1.2) {
        app.pad.slide_fade_in = fade;
    }
    *y += 72.0 * ctx.scale;

    let mut speed = app.pad.play_speed;
    let r = RectF { x, y: *y, w, h: 56.0 * ctx.scale };
    if pages::slider_row(ctx, input, "set_play_speed", 0, r, "播放速度", &mut speed, 0.5, 2.0) {
        app.pad.set_play_speed(speed);
    }
    *y += 84.0 * ctx.scale;

    draw::text(ctx.font.as_ref(), "快捷键", x, *y, 14.0 * ctx.scale, theme::TEXT);
    *y += 22.0 * ctx.scale;
    draw::text(
        ctx.font.as_ref(),
        "1–8 / T 触发触摸区域 · Space 播放/暂停 · R 重播 · ESC 暂停",
        x,
        *y,
        12.0 * ctx.scale,
        theme::TEXT_DIM,
    );
    *y += 24.0 * ctx.scale;
}

fn draw_display(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32) {
    let mut scale = app.pad.ui_scale_override.unwrap_or(1.0);
    let r = RectF { x, y: *y, w, h: 56.0 * ctx.scale };
    if pages::slider_row(ctx, input, "set_scale", 0, r, "界面缩放", &mut scale, 0.8, 1.6) {
        app.pad.ui_scale_override = Some(scale);
    }
    *y += 72.0 * ctx.scale;

    let r = RectF { x, y: *y, w, h: 52.0 * ctx.scale };
    pages::toggle_row(
        ctx,
        input,
        "set_mobile",
        0,
        r,
        "移动端布局",
        "使用更大的控件和更紧凑的单列布局。",
        &mut app.pad.mobile_ui,
    );
    *y += 72.0 * ctx.scale;

    draw::text(ctx.font.as_ref(), "当前状态", x, *y, 14.0 * ctx.scale, theme::TEXT);
    *y += 22.0 * ctx.scale;
    draw::text(ctx.font.as_ref(), &app.status, x, *y, 12.0 * ctx.scale, theme::TEXT_DIM);
    *y += 24.0 * ctx.scale;
}
