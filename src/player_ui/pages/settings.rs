//! Player settings.
//!
//! This page is the player-facing counterpart of the F1 parameter panel. The
//! old page only exposed a few session controls, which made note, touch,
//! judgment and effect settings appear to be missing on Android.

use macroquad::prelude::*;

use crate::app::params::Params;
use crate::app::types::RectF;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::PlayerUiApp;
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

const CATS: [(&str, &str); 7] = [
    ("音频", "音乐与击打音效"),
    ("游玩", "速度、判定与辅助"),
    ("Note / Slide", "Note 尺寸与 Slide 轨迹"),
    ("Touch / Pad", "感应区、辅助线与判定位置"),
    ("特效 / 视频", "击打特效与背景视频"),
    ("显示 / 布局", "播放栏位置、方向与 Pad 方向"),
    ("网络预览", "连接 Editor、扫码与记住设备"),
];
fn category_height(scale: f32) -> f32 { (40.0 + CATS.len().div_ceil(3) as f32 * 46.0) * scale }

pub fn draw(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    // Keep the Pad as the visual ground truth. Only a very light scrim is
    // drawn here; opaque panels would hide the playfield while tuning it.
    draw::rect(RectF { x: 0.0, y: 0.0, w: ctx.w, h: ctx.h },
        draw::with_alpha(theme::VOID, 0.025));
    let back_label = if app.settings_return == crate::player_ui::state::Page::Pause {
        "返回暂停"
    } else {
        "返回"
    };
    let (back, trailing) = pages::top_bar_surface(ctx, input, back_label, "设置", "PLAYER SETTINGS",
        draw::with_alpha(theme::PANEL, 0.34));
    let tw = 96.0 * ctx.scale;
    let done = RectF { x: trailing.x + trailing.w - tw, w: tw, ..trailing };
    if pages::button(ctx, input, "set_done", 0, done, "完成", Btn::Primary) {
        save_params(app);
        app.close_settings();
        return;
    }
    if back || is_key_pressed(KeyCode::Escape) {
        save_params(app);
        app.close_settings();
        return;
    }

    let wide = ctx.w >= 820.0 * ctx.scale;
    if wide {
        let cat_w = (240.0 * ctx.scale).min(ctx.w * 0.32);
        draw_categories(app, input, ctx, cat_w);
        draw_panel(app, input, ctx, RectF {
            x: cat_w, y: 64.0 * ctx.scale,
            w: ctx.w - cat_w, h: ctx.h - 64.0 * ctx.scale,
        });
    } else {
        draw_categories(app, input, ctx, ctx.w);
        draw_panel(app, input, ctx, RectF {
            x: 0.0, y: 64.0 * ctx.scale + category_height(ctx.scale),
            w: ctx.w, h: ctx.h - 64.0 * ctx.scale - category_height(ctx.scale),
        });
    }
}

fn draw_categories(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, w: f32) {
    let bar_h = 64.0 * ctx.scale;
    let panel = RectF {
        x: 0.0, y: bar_h, w,
        h: if w < ctx.w { ctx.h - bar_h } else { category_height(ctx.scale) },
    };
    draw::rect(panel, draw::with_alpha(theme::PANEL, 0.14));

    if w >= ctx.w {
        // A narrow/full-width layout uses enough rows so all tabs remain
        // readable and the detail panel starts below them.
        let each = (ctx.w - 32.0 * ctx.scale) / 3.0;
        for (i, (label, _)) in CATS.iter().enumerate() {
            let col = i % 3;
            let row = i / 3;
            draw_cat(app, input, ctx, i, RectF {
                x: 16.0 * ctx.scale + col as f32 * each,
                y: bar_h + 36.0 * ctx.scale + row as f32 * 46.0 * ctx.scale,
                w: each - 8.0 * ctx.scale, h: 40.0 * ctx.scale,
            }, label);
        }
    } else {
        // A wide layout gets a real sidebar. It can show all categories without
        // covering the detail viewport.
        draw::text(ctx.font.as_ref(), "PLAYER SETTINGS", panel.x + 20.0 * ctx.scale,
            panel.y + 30.0 * ctx.scale, 11.0 * ctx.scale, theme::ACCENT);
        let mut y = panel.y + 48.0 * ctx.scale;
        let row_h = ((panel.h - 56.0 * ctx.scale) / CATS.len() as f32).min(68.0 * ctx.scale);
        for (i, (label, _desc)) in CATS.iter().enumerate() {
            let r = RectF {
                x: 16.0 * ctx.scale, y, w: w - 32.0 * ctx.scale, h: row_h - 4.0 * ctx.scale,
            };
            let selected = app.settings_section == i;
            let resp = input.widget(input::id("cat", i), r);
            let fill = if selected { theme::RAISED } else if resp.hovered { theme::PANEL_ALT } else { theme::PANEL };
            draw::cut_rect(r, 8.0 * ctx.scale, draw::with_alpha(fill, 0.34),
                Some(if selected { theme::ACCENT } else { theme::BORDER_SOFT }), 1.0);
            if selected { draw::rect(RectF { x: r.x, y: r.y, w: 3.0 * ctx.scale, h: r.h }, theme::ACCENT); }
            draw::text(ctx.font.as_ref(), label, r.x + 14.0 * ctx.scale,
                r.y + r.h * 0.5 + 5.0 * ctx.scale, 15.0 * ctx.scale, theme::TEXT);
            if row_h >= 64.0 * ctx.scale {
            draw::text(ctx.font.as_ref(), _desc, r.x + 14.0 * ctx.scale,
                r.y + 46.0 * ctx.scale, 11.0 * ctx.scale, theme::TEXT_MUTED);
            }
            if resp.clicked { select_section(app, i); }
            y += row_h;
        }
    }
}

fn draw_cat(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, i: usize, r: RectF, label: &str) {
    let selected = app.settings_section == i;
    let resp = input.widget(input::id("cat", i), r);
    let fill = if selected { theme::ACCENT_DIM } else if resp.hovered { theme::RAISED } else { theme::PANEL_ALT };
    draw::cut_rect(r, 8.0 * ctx.scale, draw::with_alpha(fill, 0.34), Some(theme::BORDER), 1.0);
    draw::text_centered(ctx.font.as_ref(), label, r.x + r.w * 0.5,
        r.y + r.h * 0.5 + 5.0 * ctx.scale, 14.0 * ctx.scale,
        if selected { theme::ACCENT } else { theme::TEXT });
    if resp.clicked { select_section(app, i); }
}

fn select_section(app: &mut PlayerUiApp, section: usize) {
    app.settings_section = section;
    app.settings_scroll = 0.0;
    app.settings_scroll_target = 0.0;
}

fn draw_panel(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, area: RectF) {
    draw::rect(area, draw::with_alpha(theme::VOID,
        if app.settings_section == 6 { 0.88 } else { 0.055 }));
    let x = area.x + 32.0 * ctx.scale;
    let w = (area.w - 64.0 * ctx.scale).min(620.0 * ctx.scale);
    let heading_y = area.y + 30.0 * ctx.scale;
    pages::overline(ctx, "设置项", x, heading_y);
    let heading_y = heading_y + 30.0 * ctx.scale;
    let heading = CATS[app.settings_section.min(CATS.len() - 1)].0;
    draw::text(ctx.font.as_ref(), heading, x, heading_y, 26.0 * ctx.scale, theme::TEXT);

    // Keep the actions reachable while the long settings sections scroll.
    // The footer is outside the content clip so its controls remain visible.
    let footer_h = 92.0 * ctx.scale;
    let content_top_pad = 20.0 * ctx.scale;
    let viewport = RectF {
        x: area.x, y: area.y + 82.0 * ctx.scale, w: area.w,
        h: (area.h - 82.0 * ctx.scale - footer_h).max(0.0),
    };
    let content_height = if app.settings_section == 6 {
        (560.0 + app.link_ui.devices.len() as f32 * 92.0) * ctx.scale
    } else { settings_content_height(app.settings_section, ctx.scale) };
    let max_scroll = (content_height
        + content_top_pad - viewport.h).max(0.0);
    app.settings_scroll_target = (app.settings_scroll_target
        - input.touch_scroll(viewport)
        - input.scroll(viewport) * 40.0 * ctx.scale).clamp(0.0, max_scroll);
    app.settings_scroll = crate::player_ui::anim::approach(
        &mut app.settings_scroll, app.settings_scroll_target, 18.0, ctx.dt);
    let mut y = viewport.y + content_top_pad - app.settings_scroll;

    let mut params = app.pad.params.clone();
    {
        let _clip = draw::clip(viewport);
        match app.settings_section {
            0 => draw_audio(app, input, ctx, x, w, &mut y, &mut params),
            1 => draw_gameplay(app, input, ctx, x, w, &mut y, &mut params),
            2 => draw_note_slide(input, ctx, x, w, &mut y, &mut params),
            3 => draw_touch_pad(input, ctx, x, w, &mut y, &mut params),
            4 => draw_effects_video(input, ctx, x, w, &mut y, &mut params),
            6 => draw_network(app, input, ctx, x, w, &mut y),
            _ => draw_display(app, input, ctx, x, w, &mut y, &mut params),
        }
    }
    if params != app.pad.params {
        app.pad.params = params;
        crate::app::params::set(app.pad.params.clone());
        app.settings_params_dirty = true;
    }

    let footer = RectF {
        x: area.x,
        y: area.y + area.h - footer_h,
        w: area.w,
        h: footer_h.max(0.0),
    };
    draw::rect(footer, draw::with_alpha(theme::PANEL, 0.34));
    draw::rect(RectF { x: footer.x, y: footer.y, w: footer.w, h: 1.0 }, theme::BORDER_SOFT);
    let action_y = footer.y + 10.0 * ctx.scale;
    if pages::button(ctx, input, "set_save", 0,
        RectF { x, y: action_y, w: 180.0 * ctx.scale, h: 42.0 * ctx.scale }, "保存设置", Btn::Primary) {
        save_params(app);
    }
    if pages::button(ctx, input, "set_reset", 0,
        RectF { x: x + 192.0 * ctx.scale, y: action_y, w: 180.0 * ctx.scale, h: 42.0 * ctx.scale }, "恢复默认", Btn::Quiet) {
        reset_params(app);
    }
    let dirty = if app.settings_params_dirty { " · 有未保存修改" } else { "" };
    draw::text(ctx.font.as_ref(), &format!("{}{}", app.status, dirty), x, action_y + 64.0 * ctx.scale,
        12.0 * ctx.scale, theme::TEXT_MUTED);
}

fn settings_content_height(section: usize, scale: f32) -> f32 {
    let height = match section {
        0 => 520.0,
        1 => 1_250.0,
        2 => 2_450.0,
        3 => 3_100.0,
        4 => 1_850.0,
        _ => 2_050.0,
    };
    height * scale
}

fn draw_network(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32) {
    use crate::player_ui::remote::Action;
    let state = &mut app.link_ui;
    label(ctx, x, y, if state.connected { "已连接 Editor" } else { "独立设备 / 局域网预览" });
    label(ctx, x, y, &state.peer);
    label(ctx, x, y, &state.status);
    if state.listening {
        label(ctx, x, y, &format!("本机地址: {}", state.address));
        label(ctx, x, y, &format!("首次配对码: {}", state.pin));
    }
    for (i, (text, action)) in [
        ("允许 Editor 搜索连接", Action::Listen),
        ("扫描 Editor 连接二维码", Action::Scan),
        ("手动输入连接码", Action::Manual),
        ("断开 / 停止监听", Action::Disconnect),
    ].into_iter().enumerate() {
        let r = RectF { x, y: *y, w, h: 42.0 * ctx.scale };
        if pages::button(ctx, input, "link_action", i, r, text, Btn::Quiet) { state.action = Some(action); }
        *y += 54.0 * ctx.scale;
    }
    label(ctx, x, y, "记住的设备（首次连接成功后自动保存）");
    for (i, saved) in state.devices.clone().into_iter().enumerate() {
        label(ctx, x, y, &saved.device.name);
        let r = RectF { x, y: *y, w: w * 0.65, h: 42.0 * ctx.scale };
        if let Some(invitation) = saved.invitation {
            if pages::button(ctx, input, "link_reconnect", i, r, "重连", Btn::Quiet) {
                state.action = Some(Action::Connect(invitation));
            }
        } else {
            draw::text(ctx.font.as_ref(), "由 Editor 搜索重连", x, *y + 25.0 * ctx.scale, 13.0 * ctx.scale, theme::TEXT_MUTED);
        }
        if pages::button(ctx, input, "link_forget", i, RectF { x: x + w * 0.7, w: w * 0.3, ..r }, "移除", Btn::Quiet) {
            state.action = Some(Action::Forget(saved.device.id));
        }
        *y += 64.0 * ctx.scale;
    }
}

fn slider(ctx: &UiCtx, input: &mut Input, y: &mut f32, x: f32, w: f32,
    name: &str, value: &mut f32, min: f32, max: f32) {
    let r = RectF { x, y: *y, w, h: 56.0 * ctx.scale };
    pages::slider_row(ctx, input, name, 0, r, name, value, min, max);
    *y += 72.0 * ctx.scale;
}

fn toggle(ctx: &UiCtx, input: &mut Input, y: &mut f32, x: f32, w: f32,
    name: &str, label: &str, value: &mut bool) {
    let r = RectF { x, y: *y, w, h: 52.0 * ctx.scale };
    pages::toggle_row(ctx, input, name, 0, r, label, "", value);
    *y += 68.0 * ctx.scale;
}

fn volume_slider(ctx: &UiCtx, input: &mut Input, y: &mut f32, x: f32, w: f32,
    name: &str, volume: &mut f32) {
    let initial = *volume * 100.0;
    let mut percent = initial;
    slider(ctx, input, y, x, w, name, &mut percent, 0.0, 100.0);
    if percent != initial {
        *volume = percent / 100.0;
    }
}

fn label(ctx: &UiCtx, x: f32, y: &mut f32, text: &str) {
    draw::text(ctx.font.as_ref(), text, x, *y, 14.0 * ctx.scale, theme::TEXT);
    *y += 28.0 * ctx.scale;
}

fn draw_audio(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, x: f32, w: f32,
    y: &mut f32, p: &mut Params) {
    let r = RectF { x, y: *y, w, h: 52.0 * ctx.scale };
    pages::toggle_row(ctx, input, "set_audio", 0, r, "音乐与判定音效",
        "关闭后游玩仍会继续，但不会播放声音。", &mut app.pad.audio_enabled);
    *y += 76.0 * ctx.scale;
    volume_slider(ctx, input, y, x, w, "音乐音量 (%)", &mut p.music_volume);
    toggle(ctx, input, y, x, w, "set_hit_sfx", "击打音效", &mut p.hit_sfx);
    volume_slider(ctx, input, y, x, w, "击打音量 (%)", &mut p.hit_sfx_volume);
    toggle(ctx, input, y, x, w, "set_answer_sfx", "正解音 answer.wav", &mut p.answer_sfx);
    volume_slider(ctx, input, y, x, w, "正解音量 (%)", &mut p.answer_sfx_volume);
    draw::text(ctx.font.as_ref(), &format!("当前音源  {}", app.pad.audio_source_name.as_deref().unwrap_or("(无)")),
        x, *y, 12.0 * ctx.scale, theme::TEXT_MUTED);
    *y += 30.0 * ctx.scale;
}

fn draw_gameplay(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, x: f32, w: f32,
    y: &mut f32, p: &mut Params) {
    let mut note_speed = app.pad.note_speed;
    slider(ctx, input, y, x, w, "流速 (Note Speed)", &mut note_speed, 5.0, 10.0);
    app.pad.note_speed = note_speed;
    let mut touch_speed = app.pad.touch_speed;
    slider(ctx, input, y, x, w, "Touch 流速", &mut touch_speed, 5.0, 10.0);
    app.pad.touch_speed = touch_speed;
    let mut fade = app.pad.slide_fade_in;
    slider(ctx, input, y, x, w, "Slide 提前量 (秒)", &mut fade, 0.2, 1.2);
    app.pad.slide_fade_in = fade;
    let mut speed = app.pad.play_speed;
    slider(ctx, input, y, x, w, "播放速度", &mut speed, 0.5, 2.0);
    if speed != app.pad.play_speed {
        app.pad.set_play_speed(speed);
    }
    slider(ctx, input, y, x, w, "默认流速", &mut p.note_speed_default, 1.0, 20.0);
    slider(ctx, input, y, x, w, "默认 Touch 流速", &mut p.touch_speed_default, 1.0, 20.0);
    slider(ctx, input, y, x, w, "Note 出现率", &mut p.note_appear_rate, 0.05, 0.6);
    slider(ctx, input, y, x, w, "Note 出生位置", &mut p.note_spawn_frac, 0.05, 1.0);
    slider(ctx, input, y, x, w, "Tap 出生时间", &mut p.tap_spawn_time, 0.0, 1.5);
    slider(ctx, input, y, x, w, "Note 加速度", &mut p.note_accel, 0.0, 1.0);
    toggle(ctx, input, y, x, w, "set_speed_visuals", "整体速度（影响飞行与旋转）", &mut p.speed_scales_visuals);
    toggle(ctx, input, y, x, w, "set_autoplay_screen", "Autoplay 内屏模式", &mut p.autoplay_inner_screen);
    toggle(ctx, input, y, x, w, "set_no_core", "旧版 autoplay（不使用 lnmai-core）", &mut p.no_core);
    let mut autoplay = app.pad.autoplay;
    toggle(ctx, input, y, x, w, "set_autoplay", "AUTO", &mut autoplay);
    if autoplay != app.pad.autoplay { app.set_autoplay(autoplay); }
    label(ctx, x, y, "快捷键");
    draw::text(ctx.font.as_ref(), "1–8 / T 触发触摸区域 · Space 播放/暂停 · R 重播 · ESC 暂停",
        x, *y, 12.0 * ctx.scale, theme::TEXT_DIM);
    *y += 30.0 * ctx.scale;
}

fn draw_note_slide(input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32, p: &mut Params) {
    label(ctx, x, y, "Note 皮肤");
    for (i, skin) in crate::app::note_skin::NoteSkin::ALL.into_iter().enumerate() {
        let r = RectF { x: x + i as f32 * (w + 8.0 * ctx.scale) / 2.0, y: *y,
            w: (w - 8.0 * ctx.scale) / 2.0, h: 40.0 * ctx.scale };
        let selected = p.note_skin == skin;
        if pages::button(ctx, input, "set_note_skin", i, r, skin.label(),
            if selected { Btn::Primary } else { Btn::Quiet }) { p.note_skin = skin; }
    }
    *y += 62.0 * ctx.scale;
    slider(ctx, input, y, x, w, "Tap 大小", &mut p.tap_size, 8.0, 200.0);
    slider(ctx, input, y, x, w, "Hold 宽度", &mut p.hold_width, 8.0, 200.0);
    slider(ctx, input, y, x, w, "Star 大小", &mut p.star_size, 8.0, 200.0);
    slider(ctx, input, y, x, w, "Tap 判定位置", &mut p.tap_target_offset, -40.0, 80.0);
    slider(ctx, input, y, x, w, "Tap 环位置", &mut p.tap_ring_offset, -20.0, 80.0);
    slider(ctx, input, y, x, w, "编辑器 Note 大小", &mut p.editor_note_scale, 0.2, 3.0);
    toggle(ctx, input, y, x, w, "set_editor_slide_opposite", "编辑器新建 Slide 默认到对侧", &mut p.editor_slide_default_opposite);
    label(ctx, x, y, "Slide 轨迹与星星");
    slider(ctx, input, y, x, w, "Tile 间距", &mut p.slide_tile_spacing, 5.0, 80.0);
    slider(ctx, input, y, x, w, "Tile 缩放", &mut p.slide_tile_scale, 0.1, 2.0);
    slider(ctx, input, y, x, w, "Tile 大小", &mut p.slide_tile_size, 8.0, 200.0);
    slider(ctx, input, y, x, w, "轨迹透明度", &mut p.slide_trail_alpha, 0.0, 255.0);
    slider(ctx, input, y, x, w, "Slide 渐入", &mut p.slide_fade_in, 0.0, 1.0);
    slider(ctx, input, y, x, w, "连接圆弧比例", &mut p.slide_join_fillet_frac, 0.0, 2.0);
    slider(ctx, input, y, x, w, "连接圆弧最小值", &mut p.slide_join_fillet_min, 0.0, 200.0);
    slider(ctx, input, y, x, w, "连接圆弧最大值", &mut p.slide_join_fillet_max, 0.0, 800.0);
    slider(ctx, input, y, x, w, "圆弧连接影响", &mut p.slide_join_arc_influence, 0.0, 2.0);
    slider(ctx, input, y, x, w, "直线连接影响", &mut p.slide_join_line_influence, 0.0, 2.0);
    slider(ctx, input, y, x, w, "Slide 头部空隙", &mut p.slide_head_gap, 0.0, 120.0);
    slider(ctx, input, y, x, w, "Slide 尾部空隙", &mut p.slide_tail_gap, 0.0, 120.0);
    slider(ctx, input, y, x, w, "Star 出生放大", &mut p.star_spawn_scale_gain, 0.0, 2.0);
    slider(ctx, input, y, x, w, "Star 出生透明度", &mut p.star_spawn_alpha_start, 0.0, 1.0);
    slider(ctx, input, y, x, w, "Break 闪烁速度", &mut p.break_shine_speed, 0.0, 8.0);
    slider(ctx, input, y, x, w, "Slide 判定贴图大小", &mut p.slide_just_scale, 0.05, 3.0);
    toggle(ctx, input, y, x, w, "set_slide_tile_reverse", "Slide Tile 反向", &mut p.slide_tile_reverse);
    toggle(ctx, input, y, x, w, "set_slide_sub_reverse", "Slide 子轨迹反向", &mut p.slide_sub_reverse);
    toggle(ctx, input, y, x, w, "set_note_earlier", "较早的 Note 在上层", &mut p.note_earlier_on_top);
    toggle(ctx, input, y, x, w, "set_hide_notes", "隐藏 Notes", &mut p.hide_notes);
    toggle(ctx, input, y, x, w, "set_hide_slide_just", "隐藏 Slide 判定贴图", &mut p.hide_slide_just);
}

fn draw_touch_pad(input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32, p: &mut Params) {
    label(ctx, x, y, "Touch / TouchHold");
    slider(ctx, input, y, x, w, "Touch 十字大小", &mut p.touch_cross_size, 8.0, 260.0);
    slider(ctx, input, y, x, w, "Touch 出生距离", &mut p.touch_start_dist, 0.0, 140.0);
    slider(ctx, input, y, x, w, "Touch 结束距离", &mut p.touch_end_dist, 0.0, 140.0);
    slider(ctx, input, y, x, w, "Touch 缩放", &mut p.touch_scale, 0.1, 3.0);
    slider(ctx, input, y, x, w, "Touch 长大比例", &mut p.touch_grow_frac, 0.0, 0.9);
    slider(ctx, input, y, x, w, "Touch 移动渐进", &mut p.touch_move_ramp, 0.0, 0.9);
    slider(ctx, input, y, x, w, "Touch 总时长", &mut p.touch_duration_scale, 0.2, 3.0);
    slider(ctx, input, y, x, w, "Touch 出生时长", &mut p.touch_spawn_frac, 0.0, 0.9);
    slider(ctx, input, y, x, w, "TouchHold 十字", &mut p.touchhold_cross_base, 8.0, 320.0);
    slider(ctx, input, y, x, w, "TouchHold 边框", &mut p.touchhold_border_base, 8.0, 420.0);
    slider(ctx, input, y, x, w, "TouchHold 出生距离", &mut p.touchhold_start_dist, 0.0, 140.0);
    slider(ctx, input, y, x, w, "TouchHold 结束距离", &mut p.touchhold_end_dist, 0.0, 140.0);
    slider(ctx, input, y, x, w, "TouchHold 缩放", &mut p.touchhold_scale, 0.1, 3.0);
    slider(ctx, input, y, x, w, "TouchHold 旋转", &mut p.touchhold_rot_offset, -3.2, 3.2);
    slider(ctx, input, y, x, w, "Hold 出生时间", &mut p.hold_spawn_time, 0.0, 1.5);
    label(ctx, x, y, "感应区与 Pad");
    toggle(ctx, input, y, x, w, "set_sensor_range", "感应区范围触发", &mut p.sensor_range_enable);
    slider(ctx, input, y, x, w, "范围触发半径", &mut p.sensor_range_px, 0.0, 120.0);
    slider(ctx, input, y, x, w, "Pad 整体缩放", &mut p.pad_zoom, 0.3, 2.0);
    slider(ctx, input, y, x, w, "感应区缩放", &mut p.pad_zone_scale, 0.5, 1.5);
    toggle(ctx, input, y, x, w, "set_hide_zones", "隐藏感应区", &mut p.hide_zones);
    toggle(ctx, input, y, x, w, "set_tap_guide", "Tap 辅助线", &mut p.tap_guide);
    slider(ctx, input, y, x, w, "辅助线基准", &mut p.guide_base_px, 8.0, 240.0);
    slider(ctx, input, y, x, w, "Tap 辅助线大小", &mut p.tap_guide_size, 0.1, 4.0);
    slider(ctx, input, y, x, w, "Hold 辅助线大小", &mut p.hold_guide_size, 0.1, 4.0);
    slider(ctx, input, y, x, w, "辅助线侧向偏移", &mut p.tap_guide_off_x, -200.0, 200.0);
    slider(ctx, input, y, x, w, "辅助线沿飞行偏移", &mut p.tap_guide_off_y, -200.0, 200.0);
    slider(ctx, input, y, x, w, "辅助线锚点", &mut p.tap_guide_anchor, 0.0, 1.0);
    slider(ctx, input, y, x, w, "辅助线成长", &mut p.tap_guide_grow, 0.0, 3.0);
    slider(ctx, input, y, x, w, "辅助线透明度", &mut p.tap_guide_alpha, 0.0, 255.0);
    slider(ctx, input, y, x, w, "辅助线旋转", &mut p.tap_guide_rot, -3.2, 3.2);
    label(ctx, x, y, "判定位置与 Pad 外观");
    slider(ctx, input, y, x, w, "判定区透明度", &mut p.judge_ring_alpha, 0.0, 255.0);
    slider(ctx, input, y, x, w, "Pad 背景透明度", &mut p.pad_bg_alpha, 0.0, 255.0);
    slider(ctx, input, y, x, w, "Pad 圆大小", &mut p.pad_circle_scale, 1.0, 2.0);
    slider(ctx, input, y, x, w, "Pad 背景大小", &mut p.pad_bg_scale, 1.0, 2.0);
    slider(ctx, input, y, x, w, "圈外遮盖透明度", &mut p.pad_outside_alpha, 0.0, 255.0);
    toggle(ctx, input, y, x, w, "set_judge_dot", "显示判定黑点", &mut p.judge_dot);
    toggle(ctx, input, y, x, w, "set_judge_windows", "显示判定区间", &mut p.show_judge_windows);
    slider(ctx, input, y, x, w, "判定黑点大小", &mut p.judge_dot_size, 0.0, 20.0);
    slider(ctx, input, y, x, w, "Tap 判定偏移", &mut p.judge_off_tap, -300.0, 300.0);
    slider(ctx, input, y, x, w, "Hold 头判定偏移", &mut p.judge_off_hold, -300.0, 300.0);
    slider(ctx, input, y, x, w, "Hold 尾判定偏移", &mut p.judge_off_hold_end, -300.0, 300.0);
    slider(ctx, input, y, x, w, "Hold 头辅助线偏移", &mut p.hold_guide_off, -300.0, 300.0);
    slider(ctx, input, y, x, w, "Hold 尾辅助线偏移", &mut p.hold_end_guide_off, -300.0, 300.0);
}

fn draw_effects_video(input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32, p: &mut Params) {
    label(ctx, x, y, "击打特效");
    toggle(ctx, input, y, x, w, "set_fx_timeline", "特效跟随时间轴（暂停即冻结）", &mut p.fx_timeline);
    toggle(ctx, input, y, x, w, "set_hit_fx", "击打特效", &mut p.hit_fx);
    toggle(ctx, input, y, x, w, "set_hit_fx_anim", "使用 Flash 击打影片", &mut p.hit_fx_anim);
    toggle(ctx, input, y, x, w, "set_hit_fx_anim_tint", "按判定等级染色", &mut p.hit_fx_anim_tint);
    slider(ctx, input, y, x, w, "击打特效大小", &mut p.hit_fx_size, 8.0, 200.0);
    slider(ctx, input, y, x, w, "烟花特效大小", &mut p.hanabi_fx_scale, 0.05, 3.0);
    slider(ctx, input, y, x, w, "特效时长", &mut p.hit_fx_duration, 0.05, 1.5);
    slider(ctx, input, y, x, w, "特效透明度", &mut p.hit_fx_alpha, 0.0, 255.0);
    slider(ctx, input, y, x, w, "特效线宽", &mut p.hit_fx_ring, 0.5, 20.0);
    slider(ctx, input, y, x, w, "火花数量", &mut p.hit_fx_sparks, 0.0, 32.0);
    slider(ctx, input, y, x, w, "火花长度", &mut p.hit_fx_spark_len, 0.0, 5.0);
    slider(ctx, input, y, x, w, "特效扩散", &mut p.hit_fx_grow, 0.0, 4.0);
    slider(ctx, input, y, x, w, "中心闪光", &mut p.hit_fx_flash, 0.0, 3.0);
    draw::text(ctx.font.as_ref(), &format!("影片名称: {}", p.hit_fx_anim_clip), x, *y, 12.0 * ctx.scale, theme::TEXT_MUTED);
    *y += 30.0 * ctx.scale;
    label(ctx, x, y, "背景视频");
    toggle(ctx, input, y, x, w, "set_bg_video", "启用视频背景", &mut p.bg_video);
    draw::text(ctx.font.as_ref(), &format!("视频路径: {}", if p.bg_video_path.is_empty() { "<assets>/bg.mp4" } else { &p.bg_video_path }), x, *y, 12.0 * ctx.scale, theme::TEXT_MUTED);
    *y += 30.0 * ctx.scale;
    slider(ctx, input, y, x, w, "视频起始秒", &mut p.bg_video_start, 0.0, 600.0);
    slider(ctx, input, y, x, w, "视频位置 X", &mut p.bg_video_x, -2000.0, 2000.0);
    slider(ctx, input, y, x, w, "视频位置 Y", &mut p.bg_video_y, -2000.0, 2000.0);
    slider(ctx, input, y, x, w, "视频缩放", &mut p.bg_video_scale, 0.1, 4.0);
    slider(ctx, input, y, x, w, "视频透明度", &mut p.bg_video_alpha, 0.0, 255.0);
    slider(ctx, input, y, x, w, "视频帧率", &mut p.bg_video_fps, 0.0, 120.0);
    slider(ctx, input, y, x, w, "视频输出高度", &mut p.bg_video_height, 120.0, 2160.0);
    toggle(ctx, input, y, x, w, "set_bg_video_loop", "视频循环", &mut p.bg_video_loop);
}

fn draw_display(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx, x: f32, w: f32, y: &mut f32, p: &mut Params) {
    use crate::app::params::{CenterDisplay, CenterCombo, CenterAchievement};
    label(ctx, x, y, "Pad 中心统计");
    choice(ctx, input, x, w, y, "center_display", &mut p.center_display, &CenterDisplay::ALL);
    if matches!(p.center_display, CenterDisplay::Achievement | CenterDisplay::Both) {
        label(ctx, x, y, "达成率类型");
        choice(ctx, input, x, w, y, "center_achievement", &mut p.center_achievement, &CenterAchievement::ALL);
        slider(ctx, input, y, x, w, "达成率大小", &mut p.center_achievement_scale, 0.3, 3.0);
    }
    if matches!(p.center_display, CenterDisplay::Combo | CenterDisplay::Both) {
        label(ctx, x, y, "Combo 类型");
        choice(ctx, input, x, w, y, "center_combo", &mut p.center_combo, &CenterCombo::ALL);
        slider(ctx, input, y, x, w, "Combo 大小", &mut p.center_combo_scale, 0.3, 3.0);
    }
    label(ctx, x, y, "Pad 播放区域");
    direction(ctx, input, x, w, y, "pad_direction", "Pad 方向", &mut p.pad_rotation_deg);
    label(ctx, x, y, "播放进度栏");
    toggle(ctx, input, y, x, w, "set_hud_follow_pad", "播放栏方向跟随 Pad", &mut p.hud_follow_pad_rotation);
    if !p.hud_follow_pad_rotation {
        direction(ctx, input, x, w, y, "hud_direction", "播放栏方向", &mut p.hud_rotation_deg);
    }
    slider(ctx, input, y, x, w, "播放栏位置 X", &mut p.hud_offset_x, -1280.0, 1280.0);
    slider(ctx, input, y, x, w, "播放栏位置 Y", &mut p.hud_offset_y, -760.0, 760.0);
    if pages::button(ctx, input, "set_layout_reset", 0,
        RectF { x, y: *y, w: w.min(180.0 * ctx.scale), h: 36.0 * ctx.scale },
        "重置播放布局", Btn::Quiet) {
        p.pad_rotation_deg = 0.0;
        p.hud_rotation_deg = 0.0;
        p.hud_follow_pad_rotation = false;
        p.hud_offset_x = 0.0;
        p.hud_offset_y = 0.0;
    }
    *y += 56.0 * ctx.scale;
    label(ctx, x, y, "界面");
    let old_scale = app.pad.ui_scale_override;
    let mut scale = old_scale.unwrap_or(1.0);
    slider(ctx, input, y, x, w, "界面缩放", &mut scale,
        crate::player::render::scale::UI_SCALE_MIN, crate::player::render::scale::UI_SCALE_MAX);
    if (scale - old_scale.unwrap_or(1.0)).abs() > f32::EPSILON {
        app.pad.ui_scale_override = Some(scale);
        p.player_ui_scale = Some(scale);
    }
    toggle(ctx, input, y, x, w, "set_mobile", "移动端布局", &mut app.pad.mobile_ui);
    draw::text(ctx.font.as_ref(), "移动端布局会增大控件和触摸热区。", x, *y,
        12.0 * ctx.scale, theme::TEXT_MUTED);
    *y += 30.0 * ctx.scale;
    label(ctx, x, y, "当前状态");
    draw::text(ctx.font.as_ref(), &app.status, x, *y,
        12.0 * ctx.scale, theme::TEXT_DIM);
    *y += 24.0 * ctx.scale;
}

fn choice<T: Copy + PartialEq>(ctx: &UiCtx, input: &mut Input, x: f32, w: f32, y: &mut f32,
    id: &str, value: &mut T, options: &[(T, &str)]) {
    let cols = options.len().min(3);
    let gap = 8.0 * ctx.scale;
    let width = (w - gap * (cols - 1) as f32) / cols as f32;
    for (i, (option, name)) in options.iter().enumerate() {
        let r = RectF { x: x + (i % cols) as f32 * (width + gap),
            y: *y + (i / cols) as f32 * 46.0 * ctx.scale, w: width, h: 38.0 * ctx.scale };
        if pages::button(ctx, input, id, i, r, name,
            if *value == *option { Btn::Primary } else { Btn::Quiet }) { *value = *option; }
    }
    *y += options.len().div_ceil(cols) as f32 * 46.0 * ctx.scale + 12.0 * ctx.scale;
}

fn direction(ctx: &UiCtx, input: &mut Input, x: f32, w: f32, y: &mut f32,
    id: &str, name: &str, angle: &mut f32) {
    label(ctx, x, y, name);
    let gap = 8.0 * ctx.scale;
    let button_w = (w - gap * 3.0) / 4.0;
    for (i, value) in [0.0_f32, 90.0, 180.0, 270.0].into_iter().enumerate() {
        let selected = (angle.rem_euclid(360.0) - value).abs() < 0.01;
        if pages::button(ctx, input, id, i,
            RectF { x: x + i as f32 * (button_w + gap), y: *y, w: button_w, h: 36.0 * ctx.scale },
            &format!("{value:.0}°"), if selected { Btn::Primary } else { Btn::Quiet }) {
            *angle = value;
        }
    }
    *y += 52.0 * ctx.scale;
    slider(ctx, input, y, x, w, &format!("{name}角度"), angle, 0.0, 360.0);
}

fn save_params(app: &mut PlayerUiApp) {
    crate::app::params::set(app.pad.params.clone());
    match crate::app::params::save(&app.pad.params) {
        Ok(path) => {
            app.settings_params_dirty = false;
            app.status = format!("设置已保存: {}", path.display());
        }
        Err(error) => app.status = format!("保存失败: {error}"),
    }
}

fn reset_params(app: &mut PlayerUiApp) {
    app.pad.params = Params::default();
    crate::app::params::set(app.pad.params.clone());
    app.pad.note_speed = app.pad.params.note_speed_default;
    app.pad.touch_speed = app.pad.params.touch_speed_default;
    app.pad.slide_fade_in = app.pad.params.slide_fade_in;
    app.pad.set_play_speed(app.pad.params.play_speed_default);
    app.pad.ui_scale_override = None;
    app.pad.mobile_ui = false;
    crate::player::render::textures::sync_note_skin(&mut app.pad);
    app.settings_params_dirty = true;
    app.status = "已恢复默认设置（点击保存设置后写入）".to_string();
}
