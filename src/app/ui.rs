use macroquad::prelude::*;
use macroquad::texture::{load_texture, DrawTextureParams, FilterMode, Texture2D};

use super::chart;
use super::state::AppState;
use super::types::{
    hold_tail_time, is_touch_zone, sanitize_note_zone, Layout, Mode, PadGeom, RectF,
    UiAction, UiButton, LANE_COUNT, LANE_LABELS, PAD_C_ZONE,
    PREVIEW_LEAD_TIME, SCROLL_SPEED, SPEED_MAX, SPEED_MIN, SPEED_STEP, TAP_TRAVEL_TIME,
    TOUCH_TRAVEL_TIME, HOLD_TRAVEL_TIME, TAP_GROW_FRAC, TAP_SPAWN_FRAC,
    TAP_DISAPPEAR_FRAC, HOLD_DISAPPEAR_FRAC, HIT_WINDOW,
    PAD_ROTATION_RAD, NoteType, TAP_SIZE, HOLD_WIDTH, TOUCH_SIZE,
};
use super::pad_svg;

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Mai2Chart Macroquad Local Demo".to_string(),
        window_width: 1280,
        window_height: 760,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

/// Load note skins from assets.
/// - `tap.png`: used for tap notes.
/// - `hold.png`: used as vertical 9-slice for hold notes.
pub(crate) async fn load_note_textures(app: &mut AppState) {
    let tap_candidates = [
        "tap.png",
        "Skins/classic/tap.png",
        "skins/classic/tap.png",
    ];
    for path in tap_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_texture = Some(tex);
            break;
        }
    }

    let hold_candidates = [
        "hold.png",
        "Skins/classic/hold.png",
        "skins/classic/hold.png",
    ];
    for path in hold_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_texture = Some(tex);
            break;
        }
    }

    if app.tap_texture.is_none() {
        app.status = "tap texture not found (tried tap.png / Skins/classic/tap.png)".to_string();
    } else if app.hold_texture.is_none() {
        app.status = "hold texture not found (tried hold.png / Skins/classic/hold.png)".to_string();
    }
}

fn ui_scale(app: &AppState) -> f32 {
    if let Some(v) = app.ui_scale_override {
        return v;
    }
    if app.mobile_ui {
        // Scale based on shorter screen dimension vs desktop reference (760px),
        // to compensate for high-DPI physical pixels on mobile.
        let base = screen_width().min(screen_height()) / 760.0;
        base.max(1.35)
    } else {
        1.0
    }
}

pub(crate) fn compute_layout(app: &AppState) -> Layout {
    let scale = ui_scale(app);
    let sw = screen_width();
    let sh = screen_height();
    let margin = 20.0 * scale;
    let header_h = 110.0 * scale;

    let header = RectF {
        x: margin,
        y: margin,
        w: sw - margin * 2.0,
        h: header_h - 20.0 * scale,
    };

    if app.show_pad_only || app.mobile_ui {
        let pad = RectF {
            x: margin,
            y: header_h,
            w: sw - margin * 2.0,
            h: sh - header_h - margin,
        };
        return Layout {
            header,
            timeline: None,
            pad,
        };
    }

    let timeline_w = sw * 0.62;
    let timeline = RectF {
        x: margin,
        y: header_h,
        w: timeline_w,
        h: sh - header_h - margin,
    };
    let pad = RectF {
        x: margin + timeline_w + margin,
        y: header_h,
        w: sw - timeline_w - margin * 3.0,
        h: sh - header_h - margin,
    };
    Layout {
        header,
        timeline: Some(timeline),
        pad,
    }
}

pub(crate) fn compute_pad_geom(panel: RectF) -> PadGeom {
    let cx = panel.x + panel.w * 0.5;
    let cy = panel.y + panel.h * 0.5;
    let outer_r = panel.w.min(panel.h) * 0.42;
    PadGeom { cx, cy, outer_r }
}

pub(crate) fn build_ui_buttons(layout: Layout, app: &AppState) -> Vec<UiButton> {
    let scale = ui_scale(app);
    let mut out = Vec::new();
    let bw = if app.mobile_ui { 72.0 } else { 92.0 } * scale;
    let bh = if app.mobile_ui { 28.0 } else { 26.0 } * scale;
    let gap = 8.0 * scale;

    let row1 = [
        ("Play", UiAction::TogglePlay),
        ("Record", UiAction::ToggleRecord),
        ("Save", UiAction::Save),
        ("Load", UiAction::Load),
        ("Clear", UiAction::Clear),
        ("Audio", UiAction::ToggleAudio),
    ];
    let row2 = [
        ("Rec-", UiAction::RecSpeedDown),
        ("Rec+", UiAction::RecSpeedUp),
        ("Play-", UiAction::PlaySpeedDown),
        ("Play+", UiAction::PlaySpeedUp),
        ("PadOnly", UiAction::TogglePadOnly),
        ("MobileUI", UiAction::ToggleMobileUi),
    ];
    // let row3 = [
    //     ("TSpd-", UiAction::TouchSpeedDown),
    //     ("TSpd+", UiAction::TouchSpeedUp),
    // ];

    let row_total = row1.len() as f32 * bw + (row1.len() as f32 - 1.0) * gap;
    let start_x = (layout.header.x + layout.header.w - row_total - 12.0 * scale)
        .max(layout.header.x + 14.0 * scale);
    let row1_y = layout.header.y + 12.0 * scale;
    let row2_y = row1_y + bh + 8.0 * scale;

    for (i, (label, action)) in row1.iter().enumerate() {
        out.push(UiButton {
            rect: RectF {
                x: start_x + i as f32 * (bw + gap),
                y: row1_y,
                w: bw,
                h: bh,
            },
            label,
            action: *action,
        });
    }
    for (i, (label, action)) in row2.iter().enumerate() {
        out.push(UiButton {
            rect: RectF {
                x: start_x + i as f32 * (bw + gap),
                y: row2_y,
                w: bw,
                h: bh,
            },
            label,
            action: *action,
        });
    }
    // for (i, (label, action)) in row3.iter().enumerate() {
    //     out.push(UiButton {
    //         rect: RectF {
    //             x: start_x + i as f32 * (bw + gap),
    //             y: row3_y,
    //             w: bw,
    //             h: bh,
    //         },
    //         label,
    //         action: *action,
    //     });
    // }

    out
}

pub(crate) fn draw_layout(app: &AppState, layout: Layout, pad: PadGeom, buttons: &[UiButton]) {
    let scale = ui_scale(app);
    draw_rectangle(
        layout.header.x,
        layout.header.y,
        layout.header.w,
        layout.header.h,
        Color::from_rgba(17, 24, 39, 255),
    );

    draw_text(
        "Mai2Chart Local Demo (macroquad)",
        layout.header.x + 14.0 * scale,
        layout.header.y + 30.0 * scale,
        28.0 * scale,
        WHITE,
    );

    let mode_label = match app.mode {
        Mode::Idle => "IDLE",
        Mode::Recording => "RECORDING",
        Mode::Playing => "PLAYBACK",
    };

    draw_text(
        &format!(
            "Touch controls enabled  |  Mode: {mode_label}  |  Hold: press and release"
        ),
        layout.header.x + 14.0 * scale,
        layout.header.y + 56.0 * scale,
        20.0 * scale,
        Color::from_rgba(148, 163, 184, 255),
    );

    draw_text(
        &format!(
            "RecSpeed [{:.1}x]   PlaySpeed [{:.1}x]  Status: {}",
            app.record_speed, app.play_speed, app.status
        ),
        layout.header.x + 14.0 * scale,
        layout.header.y + 84.0 * scale,
        20.0 * scale,
        Color::from_rgba(125, 211, 252, 255),
    );

    draw_ui_buttons(app, buttons);

    if let Some(timeline) = layout.timeline {
        draw_timeline_panel(app, timeline);
    }

    draw_pad_panel(app, layout.pad, pad);
}

fn draw_ui_buttons(app: &AppState, buttons: &[UiButton]) {
    let scale = ui_scale(app);
    for b in buttons {
        let active = match b.action {
            UiAction::TogglePlay => app.mode == Mode::Playing,
            UiAction::ToggleRecord => app.mode == Mode::Recording,
            UiAction::ToggleAudio => app.audio_enabled,
            UiAction::TogglePadOnly => app.show_pad_only,
            UiAction::ToggleMobileUi => app.mobile_ui,
            _ => false,
        };
        let bg = if active {
            Color::from_rgba(30, 58, 138, 255)
        } else {
            Color::from_rgba(31, 41, 55, 255)
        };
        draw_rectangle(b.rect.x, b.rect.y, b.rect.w, b.rect.h, bg);
        draw_rectangle_lines(
            b.rect.x,
            b.rect.y,
            b.rect.w,
            b.rect.h,
            1.0 * scale,
            Color::from_rgba(71, 85, 105, 255),
        );
        draw_text(
            b.label,
            b.rect.x + 10.0 * scale,
            b.rect.y + 18.0 * scale,
            16.0 * scale,
            Color::from_rgba(226, 232, 240, 255),
        );
    }
}

pub(crate) fn rect_contains(r: RectF, p: Vec2) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

pub(crate) fn trigger_ui_action(app: &mut AppState, action: UiAction) {
    match action {
        UiAction::TogglePlay => app.toggle_play(),
        UiAction::ToggleRecord => app.toggle_record(),
        UiAction::Save => match chart::save_recording_doc(app) {
            Ok(path) => app.status = format!("Saved recording: {}", path.display()),
            Err(err) => app.status = format!("Save failed: {err}"),
        },
        UiAction::Load => match chart::load_latest_saved_chart() {
            Ok(chart) => {
                app.chart = chart;
                app.status = "Loaded latest saved chart".to_string();
            }
            Err(err) => app.status = format!("Load latest failed: {err}"),
        },
        UiAction::Clear => {
            app.recording_hits.clear();
            app.recording_notes.clear();
            app.active_record_holds.clear();
            app.active_pointer_zones.clear();
            app.status = "Cleared recording hits".to_string();
        }
        UiAction::ToggleAudio => {
            app.audio_enabled = !app.audio_enabled;
            app.status = format!("Audio enabled: {}", app.audio_enabled);
            if !app.audio_enabled {
                app.stop_audio_if_any();
            } else if matches!(app.mode, Mode::Playing | Mode::Recording) {
                app.request_audio_start();
            }
        }
        UiAction::RecSpeedDown => {
            app.set_record_speed((app.record_speed - SPEED_STEP).max(SPEED_MIN));
            app.status = format!("Record speed: {:.1}x", app.record_speed);
        }
        UiAction::RecSpeedUp => {
            app.set_record_speed((app.record_speed + SPEED_STEP).min(SPEED_MAX));
            app.status = format!("Record speed: {:.1}x", app.record_speed);
        }
        UiAction::PlaySpeedDown => {
            app.set_play_speed((app.play_speed - SPEED_STEP).max(SPEED_MIN));
            app.status = format!("Playback speed: {:.1}x", app.play_speed);
        }
        UiAction::PlaySpeedUp => {
            app.set_play_speed((app.play_speed + SPEED_STEP).min(SPEED_MAX));
            app.status = format!("Playback speed: {:.1}x", app.play_speed);
        }
        // UiAction::TouchSpeedDown => {
        //     app.set_touch_speed((app.touch_speed - TOUCH_SPEED_STEP).max(TOUCH_SPEED_MIN));
        //     app.status = format!("Touch speed: {:.1}x", app.touch_speed);
        // }
        // UiAction::TouchSpeedUp => {
        //     app.set_touch_speed((app.touch_speed + TOUCH_SPEED_STEP).min(TOUCH_SPEED_MAX));
        //     app.status = format!("Touch speed: {:.1}x", app.touch_speed);
        // }
        UiAction::TogglePadOnly => {
            app.show_pad_only = !app.show_pad_only;
            app.status = format!("Pad only: {}", app.show_pad_only);
        }
        UiAction::ToggleMobileUi => {
            app.mobile_ui = !app.mobile_ui;
            app.status = format!("Mobile UI mode: {}", app.mobile_ui);
        }
    }
}

fn draw_tap_sprite(tex: &Texture2D, cx: f32, cy: f32, size: f32) {
    draw_texture_ex(
        tex,
        cx - size * 0.5,
        cy - size * 0.5,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(size, size)),
            ..Default::default()
        },
    );
}

/// Draw hold texture with vertical 9-slice behavior:
/// top cap fixed + middle stretched + bottom cap fixed.
fn draw_hold_9slice_vertical(tex: &Texture2D, cx: f32, y0: f32, y1: f32, width: f32) {
    let top = y0.min(y1);
    let bottom = y0.max(y1);
    let total_h = (bottom - top).max(1.0);

    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);

    // Keep cap aspect by converting source-pixel cap height into screen height using width ratio.
    let cap_dest_h = (cap_h * (width / tex_w)).max(1.0);
    let mut top_h = cap_dest_h.min(total_h * 0.5);
    let mut bottom_h = cap_dest_h.min(total_h * 0.5);
    if top_h + bottom_h > total_h {
        let k = total_h / (top_h + bottom_h);
        top_h *= k;
        bottom_h *= k;
    }
    let body_h = (total_h - top_h - bottom_h).max(0.0);
    let x = cx - width * 0.5;

    if top_h > 0.0 {
        draw_texture_ex(
            tex,
            x,
            top,
            WHITE,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: tex_w,
                    h: cap_h,
                }),
                dest_size: Some(vec2(width, top_h)),
                ..Default::default()
            },
        );
    }
    if body_h > 0.0 {
        draw_texture_ex(
            tex,
            x,
            top + top_h,
            WHITE,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: cap_h,
                    w: tex_w,
                    h: body_src_h,
                }),
                dest_size: Some(vec2(width, body_h)),
                ..Default::default()
            },
        );
    }
    if bottom_h > 0.0 {
        draw_texture_ex(
            tex,
            x,
            bottom - bottom_h,
            WHITE,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: tex_h - cap_h,
                    w: tex_w,
                    h: cap_h,
                }),
                dest_size: Some(vec2(width, bottom_h)),
                ..Default::default()
            },
        );
    }
}

fn draw_hold_9slice_segment(
    tex: &Texture2D,
    from: Vec2,
    to: Vec2,
    width: f32,
    tint: Color,
) {
    let delta = to - from;
    let total_len = delta.length().max(1.0);
    let dir = delta / total_len;
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_len = (cap_h * (width / tex_w)).max(1.0);

    // Minimum visible cap size in screen pixels
    let min_cap = 4.0;
    let mut head_len = cap_len.max(min_cap).min(total_len * 0.5);
    let mut tail_len = cap_len.max(min_cap).min(total_len * 0.5);
    if head_len + tail_len > total_len {
        let k = total_len / (head_len + tail_len);
        head_len *= k;
        tail_len *= k;
    }
    let body_len = (total_len - head_len - tail_len).max(0.0);

    let draw_part = |start_offset: f32, part_len: f32, src_y: f32, src_h: f32| {
        if part_len <= 0.0 {
            return;
        }
        let center = from + dir * (start_offset + part_len * 0.5);
        draw_texture_ex(
            tex,
            center.x - width * 0.5,
            center.y - part_len * 0.5,
            tint,
            DrawTextureParams {
                source: Some(Rect {
                    x: 0.0,
                    y: src_y,
                    w: tex_w,
                    h: src_h,
                }),
                dest_size: Some(vec2(width, part_len)),
                rotation: angle,
                ..Default::default()
            },
        );
    };

    draw_part(0.0, head_len, 0.0, cap_h);
    draw_part(head_len, body_len, cap_h, body_src_h);
    draw_part(head_len + body_len, tail_len, tex_h - cap_h, cap_h);
}

fn draw_timeline_panel(app: &AppState, rect: RectF) {
    let scale = ui_scale(app);
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, Color::from_rgba(17, 24, 39, 255));
    draw_text(
        "Timeline (Vertical) : 1~8 Tap/Hold + T Touch",
        rect.x + 12.0 * scale,
        rect.y + 24.0 * scale,
        24.0 * scale,
        WHITE,
    );

    let track_x = rect.x + 14.0 * scale;
    let track_y = rect.y + 40.0 * scale;
    let track_w = rect.w - 28.0 * scale;
    let track_h = rect.h - 54.0 * scale;
    let ruler_w = 64.0 * scale;
    let lanes_w = track_w - ruler_w;
    let lane_w = lanes_w / LANE_COUNT as f32;

    draw_rectangle(track_x, track_y, track_w, track_h, Color::from_rgba(11, 18, 32, 255));
    draw_rectangle(track_x, track_y, ruler_w, track_h, Color::from_rgba(8, 12, 23, 255));

    for (i, label) in LANE_LABELS.iter().enumerate() {
        let lx = track_x + ruler_w + lane_w * i as f32 + lane_w * 0.45;
        let color = if *label == "T" {
            Color::from_rgba(253, 224, 71, 255)
        } else {
            Color::from_rgba(226, 232, 240, 255)
        };
        draw_text(label, lx, track_y + 18.0 * scale, 20.0 * scale, color);
    }

    for i in 0..=LANE_COUNT {
        let lx = track_x + ruler_w + lane_w * i as f32;
        let c = if i == 4 {
            Color::from_rgba(100, 116, 139, 255)
        } else {
            Color::from_rgba(51, 65, 85, 255)
        };
        draw_line(
            lx,
            track_y,
            lx,
            track_y + track_h,
            if i == 4 { 2.0 * scale } else { 1.0 * scale },
            c,
        );
    }

    let rows = 18;
    for i in 0..=rows {
        let yy = track_y + (track_h / rows as f32) * i as f32;
        let is_major = i % 3 == 0;
        let c = if is_major {
            Color::from_rgba(185, 28, 28, 255)
        } else {
            Color::from_rgba(22, 163, 74, 255)
        };
        draw_line(track_x + ruler_w, yy, track_x + track_w, yy, 1.0 * scale, c);
        if is_major {
            let bar_num = 108_i32 - (i / 3) as i32;
            draw_text(
                &format!("{bar_num}"),
                track_x + 10.0 * scale,
                yy + 4.0 * scale,
                18.0 * scale,
                Color::from_rgba(148, 163, 184, 255),
            );
        }
    }

    let now = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => 0.0,
    };

    let judge_y = track_y + track_h - 38.0 * scale;
    draw_line(
        track_x + ruler_w,
        judge_y,
        track_x + track_w,
        judge_y,
        2.0 * scale,
        Color::from_rgba(239, 68, 68, 255),
    );

    for note in &app.chart.notes {
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let dt = note.time - now;
        let tail_dt = if matches!(note.note_type, NoteType::Hold) {
            hold_tail_time(note) - now
        } else {
            dt
        };
        if tail_dt < -0.4 || dt > PREVIEW_LEAD_TIME {
            continue;
        }
        let lane_index = if is_touch_zone(zone) {
            LANE_COUNT - 1
        } else {
            (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
        };
        let cx = track_x + ruler_w + lane_w * lane_index as f32 + lane_w * 0.5;
        let scroll =  SCROLL_SPEED;
        //     if is_touch_zone(zone) {
        //     SCROLL_SPEED * app.touch_speed
        // } else {
        //     SCROLL_SPEED
        // };
        let ny = judge_y - dt * scroll;

        match note.note_type {
            NoteType::Tap => {
                if let Some(tex) = &app.tap_texture {
                    draw_tap_sprite(tex, cx, ny, TAP_SIZE * scale);
                } else {
                    let tr = TAP_SIZE * 0.3125 * scale;
                    draw_circle(cx, ny, tr, Color::from_rgba(17, 24, 39, 255));
                    draw_circle_lines(cx, ny, tr, tr * 0.3, Color::from_rgba(244, 114, 182, 255));
                    draw_circle(cx, ny, tr * 0.3, Color::from_rgba(249, 168, 212, 255));
                }
            }
            NoteType::Touch => {
                let ts = TOUCH_SIZE * scale;
                let half = ts * 0.5;
                draw_rectangle(cx - half, ny - half, ts, ts, Color::from_rgba(15, 23, 42, 255));
                draw_rectangle_lines(cx - half, ny - half, ts, ts, 2.0 * scale, Color::from_rgba(103, 232, 249, 255));
                draw_circle(cx, ny, ts * 0.14, Color::from_rgba(103, 232, 249, 255));
            }
            NoteType::Hold => {
                let tail_time = hold_tail_time(note);
                let tail_dt = tail_time - now;
                let scroll = if is_touch_zone(zone) {
                    SCROLL_SPEED * app.touch_speed
                } else {
                    SCROLL_SPEED
                };
                let tail_y = judge_y - tail_dt * scroll;
                if let Some(tex) = &app.hold_texture {
                    draw_hold_9slice_vertical(tex, cx, ny, tail_y, HOLD_WIDTH * scale);
                } else {
                    let hw = HOLD_WIDTH * scale;
                    let top = ny.min(tail_y);
                    let h = (ny - tail_y).abs().max(hw * 0.133);
                    draw_rectangle(cx - hw * 0.2, top, hw * 0.4, h, Color::from_rgba(190, 24, 93, 130));
                    draw_rectangle_lines(cx - hw * 0.2, top, hw * 0.4, h, 1.0 * scale, Color::from_rgba(244, 114, 182, 200));
                    let hr = hw * 0.367;
                    draw_circle(cx, ny, hr, Color::from_rgba(17, 24, 39, 255));
                    draw_circle_lines(cx, ny, hr, hw * 0.1, Color::from_rgba(251, 113, 133, 255));
                    draw_circle(cx, ny, hw * 0.107, Color::from_rgba(253, 164, 175, 255));
                    draw_circle(cx, tail_y, hw * 0.133, Color::from_rgba(251, 113, 133, 220));
                }
            }
        }
    }

    if app.mode == Mode::Recording {
        for hit in &app.recording_hits {
            let zone = if hit.lane == 9 { PAD_C_ZONE } else { hit.lane };
            let dt = hit.time - now;
            if dt < -0.3 || dt > 1.5 {
                continue;
            }
            let lane_index = if is_touch_zone(zone) {
                LANE_COUNT - 1
            } else {
                (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
            };
            let cx = track_x + ruler_w + lane_w * lane_index as f32 + lane_w * 0.5;
            let ny = judge_y - dt * SCROLL_SPEED;
            draw_circle(cx, ny, 4.0 * scale, Color::from_rgba(56, 189, 248, 220));
        }
    }
}

fn draw_pad_panel(app: &AppState, rect: RectF, pad: PadGeom) {
    let scale = ui_scale(app);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::from_rgba(17, 24, 39, 255),
    );
    draw_text(
        "Pad View",
        rect.x + 12.0 * scale,
        rect.y + 24.0 * scale,
        24.0 * scale,
        WHITE,
    );

    let cx = pad.cx;
    let cy = pad.cy;
    let outer_r = pad.outer_r;
    // Tap spawn center: midpoint of C1 and C2 centroids for alignment
    let spawn_cx = app.pad_svg.as_ref()
        .and_then(|svg| svg.pad_visual_center(&pad))
        .unwrap_or(vec2(cx, cy));

    draw_circle(cx, cy, outer_r, Color::from_rgba(16, 24, 38, 255));

    // Tap spawn point indicator
    draw_circle(spawn_cx.x, spawn_cx.y, 3.0 * scale, Color::from_rgba(255, 255, 255, 180));

    let active_zones: Vec<u8> = app.active_pointer_zones.values().copied().collect();
    let feedback_zones: Vec<u8> = app.pad_feedback.iter().map(|fb| fb.zone).collect();

    if let Some(ref pad_svg) = app.pad_svg {
        for def in &pad_svg.zones {
            let screen_verts = pad_svg.def_screen_verts(def, &pad);
            let centroid = pad_svg.def_screen_centroid(def, &pad);

            let is_active = active_zones.contains(&def.zone);
            let is_feedback = feedback_zones.contains(&def.zone);

            let (fill_color, stroke_color) = if is_active {
                (
                    Color::from_rgba(56, 189, 248, 180),
                    Color::from_rgba(125, 211, 252, 255),
                )
            } else if is_feedback {
                (
                    Color::from_rgba(250, 204, 21, 160),
                    Color::from_rgba(252, 211, 77, 255),
                )
            } else {
                (
                    Color::from_rgba(30, 41, 59, 255),
                    Color::from_rgba(71, 85, 105, 255),
                )
            };

            pad_svg::draw_polygon_fill(&screen_verts, fill_color);
            pad_svg::draw_polygon_lines(&screen_verts, 2.0 * scale, stroke_color);

            let text_color = if is_active || is_feedback {
                WHITE
            } else {
                Color::from_rgba(148, 163, 184, 255)
            };
            let text_size = 14.0 * scale;
            let text_dims = measure_text(&def.label, None, text_size as _, 1.0);
            draw_text(
                &def.label,
                centroid.x - text_dims.width * 0.5,
                centroid.y + text_dims.height * 0.35,
                text_size,
                text_color,
            );
        }

        // Draw A-zone tap indicators at SVG centroid-projected positions
        for zone in 1..=8 {
            if let Some(centroid) = pad_svg.zone_screen_centroid(zone, &pad) {
                let dir = (centroid - vec2(cx, cy)).normalize_or_zero();
                let dot_r = outer_r - 4.0 * scale;
                draw_circle(
                    cx + dir.x * dot_r,
                    cy + dir.y * dot_r,
                    4.0 * scale,
                    Color::from_rgba(255, 255, 255, 200),
                );
            }
        }
    }

    let current_t = if app.mode == Mode::Playing {
        app.song_time()
    } else {
        0.0
    };

    for note in &app.chart.notes {
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let dt = note.time - current_t;
        let tail_dt = if matches!(note.note_type, NoteType::Hold) {
            hold_tail_time(note) - current_t
        } else {
            dt
        };

        let lead_time = if zone <= 8 {
            TAP_TRAVEL_TIME
        } else {
            match note.note_type {
                NoteType::Hold => HOLD_TRAVEL_TIME,
                _ => TOUCH_TRAVEL_TIME,
            }
        };
        if tail_dt < -0.18 || dt > lead_time {
            continue;
        }
        // A-zone tap disappears at dt fraction; hold disappears at tail fraction
        if zone <= 8 {
            if matches!(note.note_type, NoteType::Hold) {
                if tail_dt <= (hold_tail_time(note) - note.time) * HOLD_DISAPPEAR_FRAC {
                    continue;
                }
            } else if dt <= TAP_TRAVEL_TIME * TAP_DISAPPEAR_FRAC {
                continue;
            }
        }

        if zone <= 8 {
            let dir = app.pad_svg.as_ref()
                .and_then(|svg| svg.zone_screen_centroid(zone, &pad))
                .map(|c| (c - spawn_cx).normalize_or_zero())
                .unwrap_or_else(|| {
                    let idx = (zone - 1) as f32;
                    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                    vec2(ang.cos(), ang.sin())
                });
            let progress = ((TAP_TRAVEL_TIME - dt) / TAP_TRAVEL_TIME).clamp(0.0, 1.0);
            // Phase 1: grow from 0 to 1 at spawn point. Phase 2: fly at full size.
            let size_scale = if progress < TAP_GROW_FRAC {
                progress / TAP_GROW_FRAC
            } else {
                1.0
            };
            let fly_progress = if progress < TAP_GROW_FRAC {
                0.0
            } else {
                (progress - TAP_GROW_FRAC) / (1.0 - TAP_GROW_FRAC)
            };
            let spawn_r = outer_r * TAP_SPAWN_FRAC;
            let target_r = outer_r - 4.0 * scale;
            // r = midpoint (grow: fixed at spawn, fly: moves to target)
            let r = spawn_r + (target_r - spawn_r) * fly_progress;
            let px = spawn_cx.x + dir.x * r;
            let py = spawn_cx.y + dir.y * r;

            if matches!(note.note_type, NoteType::Hold) {
                let hold_dur = hold_tail_time(note) - note.time;
                let full_hold_len = (target_r - spawn_r) * (hold_dur / TAP_TRAVEL_TIME).min(1.0);
                // Grow from midpoint: head outward, tail inward, symmetric
                let hold_half = (full_hold_len * size_scale * 0.5).max(2.0);
                let head_r = (r + hold_half).min(target_r);
                let tail_r = (r - hold_half).max(spawn_r * 0.1);
                let hx = spawn_cx.x + dir.x * head_r;
                let hy = spawn_cx.y + dir.y * head_r;
                let tx = spawn_cx.x + dir.x * tail_r;
                let ty = spawn_cx.y + dir.y * tail_r;
                if let Some(hold_tex) = &app.hold_texture {
                    draw_hold_9slice_segment(hold_tex, vec2(hx, hy), vec2(tx, ty), HOLD_WIDTH * scale, Color::from_rgba(255, 255, 255, 255));
                } else {
                    draw_line(hx, hy, tx, ty, HOLD_WIDTH * 0.233 * scale, Color::from_rgba(251, 113, 133, 200));
                    draw_circle(tx, ty, HOLD_WIDTH * 0.167 * scale, Color::from_rgba(253, 164, 175, 255));
                }
            }

            if !matches!(note.note_type, NoteType::Hold) {
                let ts = TAP_SIZE * scale * size_scale;
                if let Some(tex) = &app.tap_texture {
                    draw_texture_ex(tex, px - ts * 0.5, py - ts * 0.5, WHITE, DrawTextureParams {
                        dest_size: Some(vec2(ts, ts)),
                        ..Default::default()
                    });
                } else {
                    let tr = TAP_SIZE * 0.375 * scale * size_scale;
                    draw_circle(px, py, tr, Color::from_rgba(17, 24, 39, 255));
                    draw_circle_lines(px, py, tr, tr * 0.25, Color::from_rgba(244, 114, 182, 255));
                    draw_circle(px, py, tr * 0.317, Color::from_rgba(249, 168, 212, 255));
                }
            }

            if dt.abs() <= HIT_WINDOW {
                draw_circle_lines(px, py, TAP_SIZE * 0.53 * scale, 2.0 * scale, Color::from_rgba(255, 255, 255, 220));
            }
        } else {
            let Some(center) = app
                .pad_svg
                .as_ref()
                .and_then(|svg| svg.zone_screen_centroid(zone, &pad))
            else {
                continue;
            };
            let travel = match note.note_type {
                NoteType::Hold => HOLD_TRAVEL_TIME,
                _ => TOUCH_TRAVEL_TIME,
            };
            let raw = (travel - dt) / travel;
            let progress = smoothstep(raw.clamp(0.0, 1.0));
            let size = (14.0 + 10.0 * progress) * scale;
            let half = size * 0.5;
            let alpha = (90.0 + 165.0 * progress) as u8;
            draw_rectangle(
                center.x - half,
                center.y - half,
                size,
                size,
                Color::from_rgba(15, 23, 42, alpha),
            );
            draw_rectangle_lines(
                center.x - half,
                center.y - half,
                size,
                size,
                2.0 * scale,
                Color::from_rgba(103, 232, 249, alpha),
            );
            if dt.abs() <= HIT_WINDOW {
                draw_circle_lines(center.x, center.y, TOUCH_SIZE * scale, 2.0 * scale, Color::from_rgba(255, 255, 255, 220));
            }
            if matches!(note.note_type, NoteType::Hold) {
                let head_center = center;
                let len = (hold_tail_time(note) - note.time).max(0.0) * (outer_r * 0.38);
                let dir = (center - vec2(cx, cy)).normalize_or_zero();
                let seg_to = head_center + dir * len.min(outer_r * 0.26);
                if let Some(hold_tex) = &app.hold_texture {
                    draw_hold_9slice_segment(hold_tex, head_center, seg_to, HOLD_WIDTH * 0.8 * scale, Color::from_rgba(255, 255, 255, alpha));
                } else {
                    draw_line(head_center.x, head_center.y, seg_to.x, seg_to.y, HOLD_WIDTH * 0.133 * scale,
                        Color::from_rgba(251, 113, 133, alpha),
                    );
                }
            }
        }
    }

    draw_text(
        "Pad zones: A1~A8(Outer) + B1~B8(Inner) + C1(Center) + D1~8(Left) + E1~8(Right)",
        rect.x + 12.0 * scale,
        rect.y + rect.h - 30.0 * scale,
        18.0 * scale,
        Color::from_rgba(165, 180, 252, 255),
    );
}

fn smoothstep(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
