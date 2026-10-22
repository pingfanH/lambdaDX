//! Gameplay: the reused pad view plus a flat HUD bar.

use macroquad::prelude::*;

use crate::app::params;
use crate::app::types::{Mode, RectF};
use crate::player::render::{self, PadSurface};
use crate::player::{input as pinput, layout as playout};
use crate::player_ui::UiCtx;
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::{Page, PlayerUiApp};
use crate::player_ui::theme;
use crate::player_ui::transform;

/// Draw the pad view (visual only). Pad input is routed separately by
/// [`handle_pad_input`] *after* the UI, so overlapping UI widgets win.
/// Both binaries share [`render::draw_pad_panel`]; this passes the player's
/// themed surface (flat panel + dot grid + border).
pub fn draw_view(app: &mut PlayerUiApp, ctx: &UiCtx) {
    // Feed the pad renderer the current song's cover (updates as it decodes).
    if !app.remote_preview && let Some(i) = app.loaded.or(Some(app.selected)) {
        app.pad.cover_texture = app.library.ensure_cover(i).cloned();
    }
    // Full-window pad panel so the circle is centred on screen (the HUD floats
    // on top).
    let pad_rect = RectF {
        x: 0.0,
        y: 0.0,
        w: ctx.w,
        h: ctx.h,
    };
    let pad_geom = playout::compute_pad_geom(pad_rect);

    let bg_a = params::pad_bg_alpha().clamp(0.0, 255.0) / 255.0;
    render::draw_pad_panel_oriented(
        &app.pad, pad_rect, pad_geom, PadSurface::themed(bg_a), app.pad.params.pad_rotation_deg,
    );
}

/// Route pad input on gameplay/pause (so a paused slide can still be tracked).
/// Call this after the UI has drawn: a widget that captured the pointer sets
/// `input.consumed`, and the pad then ignores the contact — otherwise a button
/// sitting on top of a sensor zone would be swallowed by the pad.
pub fn handle_pad_input(app: &mut PlayerUiApp, ctx: &UiCtx, input: &mut Input) {
    if app.preparing || !matches!(app.page, Page::Gameplay | Page::Pause) || input.is_consumed() {
        return;
    }
    let pad_rect = RectF {
        x: 0.0,
        y: 0.0,
        w: ctx.w,
        h: ctx.h,
    };
    let pad_geom = playout::compute_pad_geom(pad_rect);
    pinput::handle_lane_input(&mut app.pad);
    let xf = transform::pad_transform(ctx.w, ctx.h, &app.pad.params);
    let mut pointer_events = pinput::collect_pointer_events();
    for event in &mut pointer_events {
        event.position = xf.to_local(event.position);
    }
    let pad_hit = pinput::handle_touch_controls(&mut app.pad, pad_geom, &pointer_events);
    if pad_hit {
        input.consume();
    }
}

pub fn draw_hud(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    if app.remote_preview && app.preparing { draw_overlays(app, ctx); return; }
    // Playback keys. Seeking works on both the live gameplay and the pause page.
    if app.page == Page::Gameplay || app.page == Page::Pause {
        if is_key_pressed(KeyCode::Left) {
            app.seek_relative(-5.0);
        }
        if is_key_pressed(KeyCode::Right) {
            app.seek_relative(5.0);
        }
    }
    if app.page == Page::Gameplay {
        if is_key_pressed(KeyCode::Escape) {
            app.pause();
        } else if is_key_pressed(KeyCode::Space) {
            app.pad.toggle_play();
        } else if is_key_pressed(KeyCode::R) {
            app.restart();
        } else if is_key_pressed(KeyCode::F5) {
            if let Err(e) = app.reload_current_song() {
                app.error = Some(e);
            }
        } else if is_key_pressed(KeyCode::A) {
            let on = !app.pad.autoplay;
            app.set_autoplay(on);
        }
    } else if is_key_pressed(KeyCode::Escape) {
        app.resume();
    }

    draw_oriented_bar(app, input, ctx);
    draw_overlays(app, ctx);
}

/// Show the actual playback-bar layout while adjusting settings, without input.
pub fn draw_hud_preview(app: &mut PlayerUiApp, ctx: &UiCtx) {
    let mut input = Input::default();
    input.consume();
    draw_oriented_bar(app, &mut input, ctx);
}

fn draw_oriented_bar(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    let scale = transform::hud_scale(ctx.w, ctx.h, ctx.scale);
    let (width, xf) = transform::hud_layout(ctx.w, ctx.h, scale, &app.pad.params);
    let local_ctx = UiCtx { w: width, h: 66.0 * scale, scale, font: ctx.font.clone(), ..*ctx };
    let _model = transform::model(xf);
    input.with_transform(xf, |input| draw_playback_bar(app, input, &local_ctx));
}

fn draw_playback_bar(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
    let scale = ctx.scale;
    let compact = ctx.w < 700.0 * scale;
    let hud_h = 66.0 * scale;
    draw::rect(
        RectF {
            x: 0.0,
            y: 0.0,
            w: ctx.w,
            h: hud_h,
        },
        draw::with_alpha(theme::PANEL, 0.78),
    );
    draw::rect(
        RectF {
            x: 0.0,
            y: hud_h - 1.0,
            w: ctx.w,
            h: 1.0,
        },
        theme::BORDER,
    );

    // Pause button.
    let pause_r = RectF {
        x: 16.0 * scale,
        y: 13.0 * scale,
        w: if compact { 56.0 } else { 84.0 } * scale,
        h: 40.0 * scale,
    };
    let label = if app.page == Page::Gameplay {
        "暂停"
    } else {
        "继续"
    };
    if pages::button(ctx, input, "hud_pause", 0, pause_r, label, Btn::Secondary) {
        if app.page == Page::Gameplay {
            app.pause();
        } else {
            app.resume();
        }
    }

    // Title block.
    let tx = pause_r.x + pause_r.w + 18.0 * scale;
    let lvl = app
        .selected_level
        .map(|k| format!("Lv.{k}"))
        .unwrap_or_else(|| "—".to_string());

    // Progress + time. Right side layout: [time] [AUTO] [PLAY] (audio dot).
    let play_w = if compact { 50.0 } else { 62.0 } * scale;
    let auto_w = if compact { 54.0 } else { 70.0 } * scale;
    let time_w = if compact { 52.0 } else { 100.0 } * scale;
    let right = 28.0 * scale;
    let play_r = RectF {
        x: ctx.w - right - play_w,
        y: 17.0 * scale,
        w: play_w,
        h: 32.0 * scale,
    };
    let autoplay_r = RectF {
        x: play_r.x - 8.0 * scale - auto_w,
        y: 17.0 * scale,
        w: auto_w,
        h: 32.0 * scale,
    };
    let bar_right = autoplay_r.x - 12.0 * scale - time_w;
    let available = (bar_right - tx).max(0.0);
    let title_w = if available >= 230.0 * scale {
        (available * 0.45).min(300.0 * scale)
    } else { 0.0 };
    if title_w > 0.0 {
        draw::text_ellipsized(ctx.font.as_ref(), &app.pad.chart.title, tx,
            hud_h * 0.5 - 3.0 * scale, 17.0 * scale, theme::TEXT, title_w - 16.0 * scale);
        draw::text_ellipsized(ctx.font.as_ref(), &format!("{}  ·  {:.1}x", lvl, app.pad.play_speed),
            tx, hud_h * 0.5 + 17.0 * scale, 11.0 * scale, theme::TEXT_MUTED, title_w - 16.0 * scale);
    }
    let bar_x = tx + title_w;
    let bar = RectF {
        x: bar_x,
        y: hud_h * 0.5 - 4.0 * scale,
        w: (bar_right - bar_x).max(1.0),
        h: 8.0 * scale,
    };
    let frac = app.pad.playback_progress();

    // Draggable scrub region (taller than the visual bar for an easy grab).
    let hit = RectF {
        x: bar.x,
        y: bar.y - 10.0 * scale,
        w: bar.w,
        h: bar.h + 20.0 * scale,
    };
    let resp = input.drag(input::id("hud_progress", 0), hit);
    if resp.held && !app.preparing {
        if !app.scrubbing {
            app.scrubbing = true;
            app.pad.scrubbing = true;
            app.pad.stop_audio_if_any();
        }
        let t = app.pad.time_at_progress((input.pos.x - bar.x) / bar.w);
        app.scrub_to(t);
    }
    // Commit on release (also when the release happens outside the bar).
    if app.scrubbing && input.released {
        let t = app.pad.time_at_progress((input.pos.x - bar.x) / bar.w);
        app.seek_to(t);
        app.scrubbing = false;
        app.pad.scrubbing = false;
    }

    let shown_frac = if app.scrubbing {
        ((input.pos.x - bar.x) / bar.w).clamp(0.0, 1.0)
    } else {
        frac
    };
    draw::progress_bar(bar, shown_frac, theme::ACCENT, theme::RAISED, theme::BORDER);
    // Handle: grows on hover / while dragging.
    let handle_r = if resp.hovered || app.scrubbing {
        7.0 * scale
    } else {
        5.0 * scale
    };
    draw_circle(
        bar.x + bar.w * shown_frac,
        bar.y + bar.h * 0.5,
        handle_r,
        if resp.hovered || app.scrubbing {
            theme::ACCENT
        } else {
            theme::TEXT_DIM
        },
    );

    let time_size = 13.0 * scale;
    let shown_time = if app.scrubbing {
        app.pad.time_at_progress(shown_frac)
    } else {
        app.pad.song_time()
    };
    draw::text_right(
        ctx.font.as_ref(),
        &format_time(shown_time),
        autoplay_r.x - 12.0 * scale,
        hud_h * 0.5 + 4.0 * scale,
        time_size,
        theme::TEXT_DIM,
    );

    // AUTOPLAY toggle.
    let ap_kind = if app.pad.autoplay {
        Btn::Primary
    } else {
        Btn::Quiet
    };
    if pages::button(ctx, input, "hud_autoplay", 0, autoplay_r, "AUTO", ap_kind) {
        let on = !app.pad.autoplay;
        app.set_autoplay(on);
    }

    // PLAY / pause.
    let kind = if app.pad.mode == Mode::Playing {
        Btn::Secondary
    } else {
        Btn::Quiet
    };
    if pages::button(ctx, input, "hud_play", 0, play_r, "PLAY", kind) {
        app.pad.toggle_play();
    }

    // Audio indicator.
    let dot_c = vec2(play_r.x + play_r.w + 12.0 * scale, hud_h * 0.5);
    draw_circle(
        dot_c.x,
        dot_c.y,
        4.0 * scale,
        if app.pad.audio_enabled {
            theme::SUCCESS
        } else {
            theme::TEXT_MUTED
        },
    );

}

fn draw_overlays(app: &PlayerUiApp, ctx: &UiCtx) {
    let scale = ctx.scale;
    // Hint.
    if app.page == Page::Gameplay {
        draw::text_right(
            ctx.font.as_ref(),
            "SPACE 暂停 · R 重播 · A 自动 · ←/→ 快退/快进 · 拖动进度条 · ESC 暂停",
            ctx.w - 16.0 * scale,
            ctx.h - 12.0 * scale,
            11.0 * scale,
            theme::TEXT_MUTED,
        );
    }

    // Preparing overlay while the song audio finishes decoding off-thread.
    if app.preparing {
        let overlay = RectF {
            x: ctx.w * 0.5 - 150.0 * scale,
            y: ctx.h * 0.5 - 44.0 * scale,
            w: 300.0 * scale,
            h: 88.0 * scale,
        };
        draw::rect(overlay, Color::new(0.0, 0.0, 0.0, 0.55));
        draw::rect_outline(overlay, 1.0, theme::BORDER);
        draw::text_centered(
            ctx.font.as_ref(),
            if app.remote_preview { "接收谱面与判定…" } else { "载入音频…" },
            overlay.x + overlay.w * 0.5,
            overlay.y + 38.0 * scale,
            16.0 * scale,
            theme::TEXT,
        );
        // Indeterminate sweep.
        let track = RectF {
            x: overlay.x + 40.0 * scale,
            y: overlay.y + 54.0 * scale,
            w: overlay.w - 80.0 * scale,
            h: 4.0 * scale,
        };
        draw::rect(track, theme::RAISED);
        let phase = (ctx.now as f32 * 0.9) % 1.0;
        let knob = track.w * 0.35;
        let kx = track.x + (track.w + knob) * phase - knob;
        draw::rect(
            RectF {
                x: kx.max(track.x),
                y: track.y,
                w: (kx + knob).min(track.x + track.w) - kx.max(track.x),
                h: track.h,
            },
            theme::ACCENT,
        );
    }
}

fn format_time(seconds: f32) -> String {
    let total = if seconds < 0.0 { seconds.abs().ceil() } else { seconds } as u32;
    let sign = if seconds < 0.0 { "-" } else { "" };
    format!("{sign}{:02}:{:02}", total / 60, total % 60)
}
