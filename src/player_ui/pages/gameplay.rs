//! Gameplay: the reused pad view plus a flat HUD bar.

use macroquad::prelude::*;

use crate::app::types::{Mode, RectF};
use crate::player::render::{feedback, notes, pad as padmod};
use crate::player::{input as pinput, layout as playout};
use crate::player_ui::draw;
use crate::player_ui::input::{self, Input};
use crate::player_ui::pages::{self, Btn};
use crate::player_ui::state::{Page, PlayerUiApp};
use crate::player_ui::theme;
use crate::player_ui::UiCtx;

/// Draw the pad view. Input is only forwarded on the live gameplay page. This
/// recomposes the reused render passes over a themed surface (instead of the
/// preview's default panel) so the "Pad View" placeholder label is dropped.
pub fn draw_view(app: &mut PlayerUiApp, ctx: &UiCtx, input: &mut Input) {
    let scale = ctx.scale;
    let hud_h = 66.0 * scale;
    let margin = 16.0 * scale;
    let pad_rect = RectF {
        x: margin,
        y: hud_h + margin * 0.5,
        w: ctx.w - margin * 2.0,
        h: ctx.h - hud_h - margin * 1.5,
    };
    let pad_geom = playout::compute_pad_geom(pad_rect);

    if app.page == Page::Gameplay {
        pinput::handle_lane_input(&mut app.pad);
        let pointer_events = pinput::collect_pointer_events();
        pinput::handle_touch_controls(&mut app.pad, pad_geom, &pointer_events);
    }

    // Themed pad surface.
    draw::rect(pad_rect, theme::PANEL);
    draw::dots(
        pad_rect,
        26.0 * scale,
        draw::with_alpha(theme::GRID, 0.35),
    );
    draw::rect_outline(pad_rect, 1.0, theme::BORDER_SOFT);

    padmod::draw_pad_disc(pad_geom.cx, pad_geom.cy, pad_geom.outer_r);

    let spawn_cx = app
        .pad
        .pad_svg
        .as_ref()
        .and_then(|svg| svg.pad_visual_center(&pad_geom))
        .unwrap_or(vec2(pad_geom.cx, pad_geom.cy));
    padmod::draw_spawn_dot(spawn_cx, scale);
    padmod::draw_zones(&app.pad, &pad_geom, scale);
    padmod::draw_ring_indicators(spawn_cx, pad_geom.outer_r, scale);

    let current_t = match app.pad.mode {
        Mode::Playing | Mode::Recording => app.pad.song_time(),
        Mode::Idle => app.pad.timeline_view_time,
    };
    let speed_scale = app.pad.play_speed.max(0.1);
    notes::draw_notes(&app.pad, &pad_geom, scale, spawn_cx, current_t, speed_scale);
    feedback::draw(&app.pad, &pad_geom, pad_geom.outer_r, spawn_cx, scale);
}

pub fn draw_hud(app: &mut PlayerUiApp, input: &mut Input, ctx: &UiCtx) {
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
            app.pad.start_playback_at(0.0);
        } else if is_key_pressed(KeyCode::A) {
            let on = !app.autoplay;
            app.set_autoplay(on);
        }
    } else if is_key_pressed(KeyCode::Escape) {
        app.resume();
    }

    let scale = ctx.scale;
    let hud_h = 66.0 * scale;
    draw::rect(
        RectF {
            x: 0.0,
            y: 0.0,
            w: ctx.w,
            h: hud_h,
        },
        theme::PANEL,
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
        w: 84.0 * scale,
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
    draw::text(
        ctx.font.as_ref(),
        &app.pad.chart.title,
        tx,
        hud_h * 0.5 - 3.0 * scale,
        17.0 * scale,
        theme::TEXT,
    );
    let lvl = app
        .selected_level
        .map(|k| format!("Lv.{k}"))
        .unwrap_or_else(|| "—".to_string());
    draw::text(
        ctx.font.as_ref(),
        &format!("{}  ·  {:.1}x", lvl, app.pad.play_speed),
        tx,
        hud_h * 0.5 + 17.0 * scale,
        11.0 * scale,
        theme::TEXT_MUTED,
    );

    // Progress + time. Right side layout: [time] [AUTO] [PLAY] (audio dot).
    let play_w = 62.0 * scale;
    let auto_w = 70.0 * scale;
    let time_w = 100.0 * scale;
    let right = 16.0 * scale;
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
    let bar_x = tx + (ctx.w * 0.30).max(200.0 * scale);
    let bar_right = autoplay_r.x - 12.0 * scale - time_w;
    let bar = RectF {
        x: bar_x,
        y: hud_h * 0.5 - 4.0 * scale,
        w: (bar_right - bar_x).max(60.0 * scale),
        h: 8.0 * scale,
    };
    let dur = app.song_duration();
    let frac = (app.pad.song_time() / dur).clamp(0.0, 1.0);

    // Draggable scrub region (taller than the visual bar for an easy grab).
    let hit = RectF {
        x: bar.x,
        y: bar.y - 10.0 * scale,
        w: bar.w,
        h: bar.h + 20.0 * scale,
    };
    let resp = input.widget(input::id("hud_progress", 0), hit);
    if resp.held && !app.preparing {
        if !app.scrubbing {
            app.scrubbing = true;
            app.pad.stop_audio_if_any();
        }
        let t = ((input.pos.x - bar.x) / bar.w).clamp(0.0, 1.0) * dur;
        app.scrub_to(t);
    }
    // Commit on release (also when the release happens outside the bar).
    if app.scrubbing && input.released {
        let t = ((input.pos.x - bar.x) / bar.w).clamp(0.0, 1.0) * dur;
        app.seek_to(t);
        app.scrubbing = false;
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
        shown_frac * dur
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
    let ap_kind = if app.autoplay {
        Btn::Primary
    } else {
        Btn::Quiet
    };
    if pages::button(ctx, input, "hud_autoplay", 0, autoplay_r, "AUTO", ap_kind) {
        let on = !app.autoplay;
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
            "载入音频…",
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
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
