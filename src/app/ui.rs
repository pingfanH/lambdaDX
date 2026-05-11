use macroquad::prelude::*;
use macroquad::texture::{load_texture, DrawTextureParams, FilterMode, Texture2D};

use super::chart;
use super::state::AppState;
use super::types::{
    hold_tail_time, is_touch_zone, sanitize_note_zone, slide_end_time, Layout, Mode, PadGeom, RectF,
    UiAction, UiButton, LANE_COUNT, LANE_LABELS, PAD_C_ZONE,
    PREVIEW_LEAD_TIME, SCROLL_SPEED, SPEED_MAX, SPEED_MIN, SPEED_STEP, TAP_TRAVEL_TIME,
    TOUCH_TRAVEL_TIME, HOLD_TRAVEL_TIME, TAP_GROW_FRAC, TAP_SPAWN_FRAC,
    TAP_DISAPPEAR_FRAC, HOLD_DISAPPEAR_FRAC, HOLD_TAIL_FLY_TIME, HOLD_LENGTH_FRAC,
    HOLD_SPAWN_FRAC, HOLD_TARGET_OFFSET, TAP_TARGET_OFFSET, HOLD_FLY_TIME,
    TOUCH_START_DIST, TOUCH_END_DIST, TOUCHHOLD_START_DIST,
    TOUCHHOLD_END_DIST, TOUCHHOLD_ROT_OFFSET, TOUCH_CROSS_SIZE, TOUCH_SCALE, TOUCHHOLD_SCALE,
    TOUCHHOLD_CROSS_BASE, TOUCHHOLD_BORDER_BASE,
    TOUCH_GROW_FRAC,
    TOUCH_DISAPPEAR_TIME, SLIDE_TILE_SPACING, SLIDE_TILE_SIZE, SLIDE_TILE_SCALE, SLIDE_TRAVEL_TIME, STAR_SIZE,
    HIT_WINDOW,
    PAD_ROTATION_RAD, TAP_RING_OFFSET, GRID_DIVISION, NoteType, TAP_SIZE, HOLD_WIDTH, TOUCH_SIZE,
    note_secs, measure_to_secs, secs_to_measure, mdur_to_secs, snap_measure,
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
        match load_texture(path).await {
          Ok(tex)=>{
              tex.set_filter(FilterMode::Linear);
            app.tap_texture = Some(tex);
            break;
          },
          Err(e)=>{
            println!("e:{}",e);
          }
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

    let touch_tri_candidates = ["Skins/classic/touch.png", "touch.png"];
    for path in touch_tri_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_tri_tex = Some(tex);
            break;
        }
    }
    let touch_pt_candidates = ["Skins/classic/touch_point.png", "touch_point.png"];
    for path in touch_pt_candidates {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_point_tex = Some(tex);
            break;
        }
    }
    // Each variant textures
    for path in ["Skins/classic/tap_each.png", "tap_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/hold_each.png", "hold_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/touch_each.png", "touch_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_tri_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/touch_point_each.png", "touch_point_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touch_point_each_tex = Some(tex);
            break;
        }
    }
    // Touch hold textures (4-frame cross, rotated 45°)
    for (i, name) in ["touchhold_0", "touchhold_1", "touchhold_2", "touchhold_3"].iter().enumerate() {
        for path in [format!("Skins/classic/{name}.png"), format!("{name}.png")] {
            if let Ok(tex) = load_texture(&path).await {
                tex.set_filter(FilterMode::Linear);
                app.touchhold_tex[i] = Some(tex);
                break;
            }
        }
    }
    for path in ["Skins/classic/touchhold_border.png", "touchhold_border.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.touchhold_border_tex = Some(tex);
            break;
        }
    }
    // Slide textures
    for path in ["Skins/classic/slide.png", "slide.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.slide_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/slide_each.png", "slide_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.slide_each_tex = Some(tex);
            break;
        }
    }

    // Star textures
    for path in ["Skins/classic/star.png", "star.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_each.png", "star_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_each_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_break.png", "star_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double.png", "star_double.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double_each.png", "star_double_each.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_each_tex = Some(tex);
            break;
        }
    }

    // Break textures
    for path in ["Skins/classic/tap_break.png", "tap_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/hold_break.png", "hold_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/slide_break.png", "slide_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.slide_break_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double_break.png", "star_double_break.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_break_tex = Some(tex);
            break;
        }
    }
    // Ex overlay textures
    for path in ["Skins/classic/tap_ex.png", "tap_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.tap_ex_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/hold_ex.png", "hold_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.hold_ex_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_ex.png", "star_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_ex_tex = Some(tex);
            break;
        }
    }
    for path in ["Skins/classic/star_double_ex.png", "star_double_ex.png"] {
        if let Ok(tex) = load_texture(path).await {
            tex.set_filter(FilterMode::Linear);
            app.star_double_ex_tex = Some(tex);
            break;
        }
    }

    if app.tap_texture.is_none() {
        app.set_status("tap texture not found (tried tap.png / Skins/classic/tap.png)".to_string());
    } else if app.hold_texture.is_none() {
        app.set_status("hold texture not found (tried hold.png / Skins/classic/hold.png)".to_string());
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
    let header_h = if app.mobile_ui { 60.0 } else { 40.0 } * scale;

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
    let bw = if app.mobile_ui { 130.0 } else { 92.0 } * scale;
    let bh = if app.mobile_ui { 46.0 } else { 26.0 } * scale;
    let gap = if app.mobile_ui { 12.0 } else { 8.0 } * scale;

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

pub(crate) fn draw_layout(app: &AppState, layout: Layout, pad: PadGeom, _buttons: &[UiButton]) {
    // Header bg (egui toolbar overlays on top)
    // draw_rectangle(layout.header.x, layout.header.y, layout.header.w, layout.header.h,
    //     Color::from_rgba(17, 24, 39, 255));

    // Waveform threshold control
    let s = ui_scale(app);
    draw_text(&format!("Wave threshold: {:.2}  [/] keys", app.waveform_threshold), layout.header.x + 14.0 * s, layout.header.y + 80.0 * s, 16.0 * s, Color::from_rgba(125, 211, 252, 200));

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
            Ok(path) => app.set_status(format!("Saved recording: {}", path.display())),
            Err(err) => app.set_status(format!("Save failed: {err}")),
        },
        UiAction::Load => match chart::load_latest_saved_chart() {
            Ok(chart) => {
                app.set_chart(chart);
                app.set_status("Loaded latest saved chart".to_string());
            }
            Err(err) => app.set_status(format!("Load latest failed: {err}")),
        },
        UiAction::Clear => {
            app.recording_hits.clear();
            app.recording_notes.clear();
            app.active_record_holds.clear();
            app.active_pointer_zones.clear();
            app.set_status("Cleared recording hits".to_string());
        }
        UiAction::ToggleAudio => {
            app.audio_enabled = !app.audio_enabled;
            app.set_status(format!("Audio enabled: {}", app.audio_enabled));
            if !app.audio_enabled {
                app.stop_audio_if_any();
            } else if matches!(app.mode, Mode::Playing | Mode::Recording) {
                app.request_audio_start();
            }
        }
        UiAction::RecSpeedDown => {
            app.set_record_speed((app.record_speed - SPEED_STEP).max(SPEED_MIN));
            app.set_status(format!("Record speed: {:.1}x", app.record_speed));
        }
        UiAction::RecSpeedUp => {
            app.set_record_speed((app.record_speed + SPEED_STEP).min(SPEED_MAX));
            app.set_status(format!("Record speed: {:.1}x", app.record_speed));
        }
        UiAction::PlaySpeedDown => {
            app.set_play_speed((app.play_speed - SPEED_STEP).max(SPEED_MIN));
            app.set_status(format!("Playback speed: {:.1}x", app.play_speed));
        }
        UiAction::PlaySpeedUp => {
            app.set_play_speed((app.play_speed + SPEED_STEP).min(SPEED_MAX));
            app.set_status(format!("Playback speed: {:.1}x", app.play_speed));
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
            app.set_status(format!("Pad only: {}", app.show_pad_only));
        }
        UiAction::ToggleMobileUi => {
            app.mobile_ui = !app.mobile_ui;
            app.set_status(format!("Mobile UI mode: {}", app.mobile_ui));
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
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);

    // Keep cap aspect by converting source-pixel cap height into screen height using width ratio.
    let cap_dest_h = (cap_h * (width / tex_w)).max(1.0);

    // Extend draw range so y0/y1 sit at the CENTER of caps (not the edge).
    let top = y0.min(y1) - cap_dest_h * 0.5;
    let bottom = y0.max(y1) + cap_dest_h * 0.5;
    let total_h = (bottom - top).max(1.0);

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
    let tex_w = tex.width().max(1.0);
    let tex_h = tex.height().max(3.0);
    let cap_h = (tex_h * 0.28).max(1.0).min(tex_h * 0.45);
    let body_src_h = (tex_h - cap_h * 2.0).max(1.0);
    let cap_len = (cap_h * (width / tex_w)).max(1.0);

    // Extend so from/to sit at cap centers (not edges).
    let raw_delta = to - from;
    let raw_len = raw_delta.length().max(0.001);
    let dir = raw_delta / raw_len;
    let ext_from = from - dir * cap_len * 0.5;
    let ext_to = to + dir * cap_len * 0.5;
    let total_len = (ext_to - ext_from).length().max(1.0);
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;

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
        let center = ext_from + dir * (start_offset + part_len * 0.5);
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
    // draw_rectangle(rect.x, rect.y, rect.w, rect.h, Color::from_rgba(17, 24, 39, 255));
    draw_text(
        "Timeline (Vertical) : 1~8 Tap/Hold + T Touch",
        rect.x + 12.0 * scale,
        rect.y + 24.0 * scale,
        24.0 * scale,
        WHITE,
    );

    let sidebar_w = super::types::TIMELINE_SIDEBAR_W;
    let track_x = rect.x + 14.0 * scale + sidebar_w;
    let track_y = rect.y + 40.0 * scale;
    let track_w = rect.w - 28.0 * scale - sidebar_w;
    let track_h = rect.h - 54.0 * scale;

    // ── Tool sidebar (Tap / Hold / Star) ──
    for (btn, tool, label) in super::types::timeline_sidebar_buttons(&rect) {
        let active = app.place_tool == tool;
        let (fill, border) = if active {
            (Color::from_rgba(56, 189, 248, 200), Color::from_rgba(255, 255, 255, 230))
        } else {
            (Color::from_rgba(30, 41, 59, 220), Color::from_rgba(71, 85, 105, 255))
        };
        draw_rectangle(btn.x, btn.y, btn.w, btn.h, fill);
        draw_rectangle_lines(btn.x, btn.y, btn.w, btn.h, 1.5, border);
        // Icon glyph based on tool.
        match tool {
            super::types::PlaceTool::Tap => {
                if let Some(tex) = &app.tap_texture {
                    let s = btn.w.min(btn.h) * 0.55;
                    draw_texture_ex(tex, btn.x + btn.w * 0.5 - s * 0.5, btn.y + btn.h * 0.35 - s * 0.5,
                        WHITE, DrawTextureParams { dest_size: Some(vec2(s, s)), ..Default::default() });
                } else {
                    draw_circle(btn.x + btn.w * 0.5, btn.y + btn.h * 0.35, btn.w * 0.18,
                        Color::from_rgba(244, 114, 182, 230));
                }
            }
            super::types::PlaceTool::Hold => {
                let hw = btn.w * 0.18;
                let cx = btn.x + btn.w * 0.5;
                let top = btn.y + btn.h * 0.18;
                let h = btn.h * 0.42;
                draw_rectangle(cx - hw * 0.5, top, hw, h, Color::from_rgba(244, 114, 182, 200));
                draw_rectangle_lines(cx - hw * 0.5, top, hw, h, 1.0, Color::from_rgba(251, 113, 133, 255));
            }
            super::types::PlaceTool::Star => {
                let cx = btn.x + btn.w * 0.5;
                let cy = btn.y + btn.h * 0.35;
                if let Some(tex) = &app.star_tex {
                    let s = btn.w.min(btn.h) * 0.55;
                    draw_texture_ex(tex, cx - s * 0.5, cy - s * 0.5,
                        WHITE, DrawTextureParams { dest_size: Some(vec2(s, s)), ..Default::default() });
                } else {
                    draw_poly(cx, cy, 5, btn.w * 0.22, 0.0, Color::from_rgba(250, 204, 21, 230));
                }
            }
        }
        let lbl_color = if active { Color::from_rgba(15, 23, 42, 255) } else { WHITE };
        let tw = measure_text(label, None, 16, 1.0).width;
        draw_text(label, btn.x + (btn.w - tw) * 0.5, btn.y + btn.h - 8.0, 16.0, lbl_color);
    }
    let ruler_w = 64.0 * scale;
    let lanes_w = track_w - ruler_w;
    let lane_w = lanes_w / LANE_COUNT as f32;

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

    let judge_y = track_y + track_h - 38.0 * scale;
    let now = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    let bpm = app.chart.bpm;

    // BPM-based grid lines (cover full track height)
    let beat_s = 60.0 / app.chart.bpm;
    let grid_s = beat_s / (GRID_DIVISION as f32 / 4.0);
    let margin_s = track_h / SCROLL_SPEED;
    let view_start = now - margin_s;
    let view_end = now + margin_s;

    let mut t = (view_start / grid_s).floor() * grid_s;
    while t <= view_end {
        let yy = judge_y - (t - now) * SCROLL_SPEED;
        let bar_s = beat_s * 4.0;
        let dist_to_bar = ((t % bar_s) + bar_s) % bar_s;
        let is_bar = dist_to_bar < grid_s * 0.5 || (bar_s - dist_to_bar) < grid_s * 0.5;
        let dist_to_beat = ((t % beat_s) + beat_s) % beat_s;
        let is_beat = dist_to_beat < grid_s * 0.5 || (beat_s - dist_to_beat) < grid_s * 0.5;
        let color = if is_bar {
            Color::from_rgba(185, 28, 28, 255)
        } else if is_beat {
            Color::from_rgba(100, 116, 139, 255)
        } else {
            Color::from_rgba(30, 41, 55, 255)
        };
        let thickness = if is_bar { 2.0 } else if is_beat { 1.5 } else { 0.5 } * scale;
        draw_line(track_x + ruler_w, yy, track_x + track_w, yy, thickness, color);
        if is_bar {
            let bar_num = (t / (beat_s * 4.0)) as i32;
            draw_text(&format!("{bar_num}"), track_x + 10.0 * scale, yy + 4.0 * scale, 16.0 * scale,
                Color::from_rgba(148, 163, 184, 255));
        }
        t += grid_s;
    }

    // Beat-focused vertical spectrum (low freqs = wider, brighter)
    if app.waveform_freq_bins > 0 && !app.waveform_data.is_empty() {
        let fb = app.waveform_freq_bins as usize;
        let num_tb = app.waveform_data.len() / fb;
        let dt = app.waveform_time_res;
        let max_val = app.waveform_data.iter().cloned().fold(0.0_f32, f32::max).max(0.1);
        // Only use low frequencies (0-500Hz for kick/snare/beat detection)
        let sr = app.audio_wav_pcm.as_ref().map(|p| p.sample_rate as f32).unwrap_or(44100.0);
        let max_hz = 600.0;
        let beat_bins = ((fb as f32 * max_hz / (sr * 0.5)) as usize).min(fb).max(8);
        let bin_step = (beat_bins as f32 / 40.0).max(1.0) as usize;
        let disp_bins = beat_bins / bin_step;
        let half_w = lanes_w * 0.45;
        for ti in 0..num_tb {
            let t = ti as f32 * dt;
            let cy = judge_y - (t - now) * SCROLL_SPEED;
            if cy < track_y || cy > track_y + track_h { continue; }
            // Compute total low-freq energy for this time bin
            let mut total = 0.0;
            let mut peak = 0.0;
            for fi in 0..beat_bins {
                let v = app.waveform_data[ti * fb + fi];
                total += v;
                if v > peak { peak = v; }
            }
            let avg_norm = (total / beat_bins as f32 / max_val).min(1.0);
            let peak_norm = (peak / max_val).min(1.0);
            if avg_norm < 0.005 { continue; }
            // Draw a prominent beat bar
            let bar_w = half_w * peak_norm * 1.5;
            let color = if peak_norm > app.waveform_threshold {
                Color::from_rgba(255, 180, 50, (peak_norm * 255.0) as u8)
            } else {
                Color::from_rgba(60, 120, (avg_norm*200.0) as u8, (avg_norm * 180.0) as u8)
            };
            draw_rectangle(track_x + ruler_w + half_w - bar_w, cy - 2.0, bar_w * 2.0, 4.0, color);
            // Per-frequency thin bars overlaid
            for di in 0..disp_bins {
                let fi = di * bin_step;
                let mag = app.waveform_data[ti * fb + fi];
                let norm = (mag / max_val).min(1.0);
                if norm < 0.03 { continue; }
                let fw = half_w * norm * 0.6 / disp_bins as f32;
                let fcolor = if norm > app.waveform_threshold {
                    Color::from_rgba(255, 200, 80, (norm * 200.0) as u8)
                } else {
                    Color::from_rgba(60, 140, 255, (norm * 150.0) as u8)
                };
                let lx = track_x + ruler_w + half_w - fw * di as f32 * 0.3;
                let rx = track_x + ruler_w + half_w + fw * di as f32 * 0.3 - fw;
                draw_rectangle(lx, cy - 1.0, fw, 2.0, fcolor);
                draw_rectangle(rx, cy - 1.0, fw, 2.0, fcolor);
            }
        }
    }

    // Scrubber triangle on ruler at now position
    let scrub_y = judge_y;
    if scrub_y >= track_y && scrub_y <= track_y + track_h {
        draw_triangle(
            vec2(track_x + ruler_w - 4.0 * scale, scrub_y),
            vec2(track_x + ruler_w - 14.0 * scale, scrub_y - 6.0 * scale),
            vec2(track_x + ruler_w - 14.0 * scale, scrub_y + 6.0 * scale),
            Color::from_rgba(239, 68, 68, 255),
        );
    }

    for (idx, note) in app.chart.notes.iter().enumerate() {
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let ns = note_secs(note, bpm);
        let dt = ns - now;
        let tail_dt = match note.note_type {
            NoteType::Hold => hold_tail_time(note, bpm) - now,
            NoteType::Slide => ns + mdur_to_secs(note.slide_duration, bpm) - now,
            _ => dt,
        };
        // Keep the note visible while either its head OR its tail is on-screen.
        // (For Tap/Touch tail_dt == dt, so the check collapses to the original.)
        if tail_dt < -0.4 || dt.min(tail_dt) > PREVIEW_LEAD_TIME {
            continue;
        }
        let lane_index = if is_touch_zone(zone) {
            LANE_COUNT - 1
        } else {
            (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1)
        };
        let cx = track_x + ruler_w + lane_w * lane_index as f32 + lane_w * 0.5;
        let scroll =  SCROLL_SPEED;
        let ny = judge_y - dt * scroll;

        // Selection highlight
        if app.selected_note == Some(idx) {
            draw_circle(cx, ny, 16.0, Color::from_rgba(56, 189, 248, 100));
        }

        match note.note_type {
            NoteType::Tap => {
                let tap_tex = if note.is_break {
                    app.tap_break_tex.as_ref()
                } else if note.is_each {
                    app.tap_each_tex.as_ref()
                } else {
                    app.tap_texture.as_ref()
                }.or(app.tap_texture.as_ref());
                let ts = TAP_SIZE * scale;
                if let Some(tex) = tap_tex {
                    draw_tap_sprite(tex, cx, ny, ts);
                    if note.is_ex {
                        if let Some(ex) = app.tap_ex_tex.as_ref() {
                            draw_tap_sprite(ex, cx, ny, ts);
                        }
                    }
                } else {
                    let tr = TAP_SIZE * 0.3125 * scale;
                    draw_circle(cx, ny, tr, Color::from_rgba(17, 24, 39, 255));
                    draw_circle_lines(cx, ny, tr, tr * 0.3, Color::from_rgba(244, 114, 182, 255));
                    draw_circle(cx, ny, tr * 0.3, Color::from_rgba(249, 168, 212, 255));
                }
                // Judgment center dot
                draw_circle(cx, ny, 2.5 * scale, Color::from_rgba(255, 255, 255, 200));
            }
            NoteType::Touch => {
                let tri_tex = if note.is_each { app.touch_tri_each_tex.as_ref() } else { app.touch_tri_tex.as_ref() }
                    .or(app.touch_tri_tex.as_ref());
                let pt_tex = if note.is_each { app.touch_point_each_tex.as_ref() } else { app.touch_point_tex.as_ref() }
                    .or(app.touch_point_tex.as_ref());
                if let Some(tex) = tri_tex {
                    let ratio = tex.width() / tex.height();
                    let ts = 30.0 * scale;
                    let tw = ts; let th = ts / ratio;
                    let color = Color::from_rgba(255, 255, 255, 200);
                    draw_texture_ex(tex, cx - tw*0.5, ny + ts*0.3 - th*0.5, color, DrawTextureParams { dest_size: Some(vec2(tw,th)), ..Default::default() });
                    draw_texture_ex(tex, cx - tw*0.5, ny - ts*0.3 - th*0.5, color, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: std::f32::consts::PI, ..Default::default() });
                    draw_texture_ex(tex, cx - ts*0.3 - tw*0.5, ny - th*0.5, color, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: std::f32::consts::FRAC_PI_2, ..Default::default() });
                    draw_texture_ex(tex, cx + ts*0.3 - tw*0.5, ny - th*0.5, color, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: -std::f32::consts::FRAC_PI_2, ..Default::default() });
                }
                if let Some(pt) = pt_tex {
                    draw_texture_ex(pt, cx - 6.0*scale, ny - 6.0*scale, Color::from_rgba(255,255,255,200), DrawTextureParams { dest_size: Some(vec2(12.0*scale,12.0*scale)), ..Default::default() });
                } else {
                    draw_circle(cx, ny, 3.0*scale, Color::from_rgba(103,232,249,200));
                }
            }
            NoteType::Slide => {
                // Slide is split into two editable regions on the timeline:
                //   [note.time, note.time + slide_start_delay]  → dashed (start delay)
                //   [note.time + slide_start_delay, note.time + slide_duration] → slide.png tiles
                let dur_s = mdur_to_secs(note.slide_duration, bpm).max(0.0);
                let delay_s = mdur_to_secs(note.slide_start_delay, bpm).max(0.0).min(dur_s);
                let delay_y = judge_y - (dt + delay_s) * scroll;
                let tail_y = judge_y - (dt + dur_s) * scroll;

                // Dashed line for the start-delay region (between head and delay_y).
                let delay_h = (ny - delay_y).abs();
                if delay_s > 0.0 && delay_h > 0.5 {
                    let dash_len = 6.0 * scale;
                    let gap = 4.0 * scale;
                    let period = dash_len + gap;
                    let top = ny.min(delay_y);
                    let n_dashes = (delay_h / period).ceil() as i32;
                    let col = Color::from_rgba(253, 224, 71, 220);
                    for k in 0..n_dashes {
                        let y0 = top + (k as f32) * period;
                        let y1 = (y0 + dash_len).min(top + delay_h);
                        draw_line(cx, y0, cx, y1, 2.0 * scale, col);
                    }
                }

                // Travel region: folded path across A-zone lanes based on slide_points.
                let slide_tex = if note.is_break { app.slide_break_tex.as_ref() } else if note.is_each { app.slide_each_tex.as_ref() } else { app.slide_tex.as_ref() };
                let travel_h = (delay_y - tail_y).abs();
                if travel_h > 0.5 {
                    // Build waypoint list: head lane, then each A-zone slide_point.
                    let zone_to_cx_a = |z: u8| -> f32 {
                        let li = (z.saturating_sub(1) as usize).min(LANE_COUNT - 2);
                        track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5
                    };
                    // Filter to A-zone points only (zone 1-8).
                    let a_points: Vec<&super::types::SlidePoint> = note.slide_points.iter()
                        .filter(|sp| sp.zone >= 1 && sp.zone <= 8)
                        .collect();
                    let mut waypoints: Vec<(f32, f32)> = Vec::new(); // (x, y)
                    waypoints.push((cx, delay_y)); // start at delay-end position
                    let n_pts = a_points.len();
                    if n_pts > 0 {
                        for (pi, sp) in a_points.iter().enumerate() {
                            let frac = (pi + 1) as f32 / (n_pts) as f32;
                            let wy = delay_y + (tail_y - delay_y) * frac;
                            let wx = zone_to_cx_a(sp.zone);
                            waypoints.push((wx, wy));
                        }
                    } else {
                        waypoints.push((cx, tail_y));
                    }

                    // Draw segments between waypoints.
                    let line_w = 3.0 * scale;
                    let col = Color::from_rgba(250, 204, 21, 200);
                    let tile_col = Color::from_rgba(255, 255, 255, 230);
                    for seg in 0..waypoints.len() - 1 {
                        let (x0, y0) = waypoints[seg];
                        let (x1, y1) = waypoints[seg + 1];
                        if let Some(tex) = slide_tex {
                            // Tile textures along the segment, rotated to face travel direction.
                            let dx = x1 - x0;
                            let dy = y1 - y0;
                            let seg_len = (dx * dx + dy * dy).sqrt();
                            let bar_w = 14.0 * scale;
                            let tw_nat = tex.width() * scale * SLIDE_TILE_SCALE;
                            let th_nat = tex.height() * scale * SLIDE_TILE_SCALE;
                            let tile_h = th_nat * (bar_w / tw_nat).max(0.001);
                            let spacing = SLIDE_TILE_SPACING * scale;
                            let angle = dy.atan2(dx) + std::f32::consts::PI;
                            let steps = (seg_len / spacing).ceil().max(1.0) as i32;
                            for k in 0..=steps {
                                let t = k as f32 / steps as f32;
                                let px = x0 + dx * t;
                                let py = y0 + dy * t;
                                draw_texture_ex(tex, px - bar_w * 0.5, py - tile_h * 0.5,
                                    tile_col,
                                    DrawTextureParams {
                                        dest_size: Some(vec2(bar_w, tile_h)),
                                        rotation: angle,
                                        ..Default::default()
                                    });
                            }
                        } else {
                            draw_line(x0, y0, x1, y1, line_w, col);
                        }
                    }
                    // Draw dots at each waypoint (except first which is delay-end handle).
                    for &(wx, wy) in &waypoints[1..] {
                        draw_circle(wx, wy, 3.0 * scale, Color::from_rgba(253, 224, 71, 200));
                    }
                }

                // Star head at note.time (same size as Tap)
                // Skip for tapless slides (they share the parent star's head)
                if !note.is_tapless {
                    // Use double-star textures when is_star is set (multiple slides)
                    let is_double = note.is_star;
                    let star_tex = if note.star_is_break {
                        if is_double { app.star_double_break_tex.as_ref() } else { app.star_break_tex.as_ref() }
                    } else if note.is_each {
                        if is_double { app.star_double_each_tex.as_ref() } else { app.star_each_tex.as_ref() }
                    } else {
                        if is_double { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() }
                    };
                    let fallback = if is_double { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() };
                    let ss = TAP_SIZE * scale;
                    if let Some(tex) = star_tex.or(fallback).or(app.star_tex.as_ref()) {
                        draw_texture_ex(tex, cx - ss * 0.5, ny - ss * 0.5,
                            Color::from_rgba(255, 255, 255, 230),
                            DrawTextureParams { dest_size: Some(vec2(ss, ss)), ..Default::default() });
                        if note.star_is_ex {
                            let ex_tex = if is_double { app.star_double_ex_tex.as_ref() } else { app.star_ex_tex.as_ref() };
                            if let Some(ex) = ex_tex.or(app.star_ex_tex.as_ref()) {
                                draw_texture_ex(ex, cx - ss * 0.5, ny - ss * 0.5,
                                    Color::from_rgba(255, 255, 255, 230),
                                    DrawTextureParams { dest_size: Some(vec2(ss, ss)), ..Default::default() });
                            }
                        }
                    } else {
                        draw_poly(cx, ny, 5, ss * 0.4, 0.0, Color::from_rgba(250, 204, 21, 230));
                    }
                }

                // Delay-end handle (smaller, white-bordered, drag adjusts slide_start_delay).
                if delay_s > 0.0 || dur_s > 0.0 {
                    draw_circle(cx, delay_y, 4.5 * scale, Color::from_rgba(56, 189, 248, 230));
                    draw_circle_lines(cx, delay_y, 4.5 * scale, 1.5 * scale, Color::from_rgba(255, 255, 255, 220));
                }
                // Tail handle (drag adjusts slide_duration) — at the last A-zone waypoint's lane.
                if dur_s > 0.0 {
                    let tail_cx = if let Some(last) = note.slide_points.iter().rev().find(|sp| sp.zone >= 1 && sp.zone <= 8) {
                        let li = (last.zone.saturating_sub(1) as usize).min(LANE_COUNT - 2);
                        track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5
                    } else { cx };
                    draw_circle(tail_cx, tail_y, 5.5 * scale, Color::from_rgba(250, 204, 21, 230));
                    draw_circle_lines(tail_cx, tail_y, 5.5 * scale, 1.5 * scale, Color::from_rgba(255, 255, 255, 220));
                }
            }
            NoteType::Hold => {
                let tail_time = hold_tail_time(note, bpm);
                let tail_dt = tail_time - now;
                let scroll = if is_touch_zone(zone) {
                    SCROLL_SPEED * app.touch_speed
                } else {
                    SCROLL_SPEED
                };
                let tail_y = judge_y - tail_dt * scroll;
                let hold_tex = if note.is_break {
                    app.hold_break_tex.as_ref()
                } else if note.is_each {
                    app.hold_each_tex.as_ref()
                } else {
                    app.hold_texture.as_ref()
                }.or(app.hold_texture.as_ref());
                let hw = HOLD_WIDTH * scale;
                if let Some(tex) = hold_tex {
                    draw_hold_9slice_vertical(tex, cx, ny, tail_y, hw);
                    if note.is_ex {
                        if let Some(ex) = app.hold_ex_tex.as_ref() {
                            draw_hold_9slice_vertical(ex, cx, ny, tail_y, hw);
                        }
                    }
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
                // Judgment center dots (head & tail)
                draw_circle(cx, ny, 2.5 * scale, Color::from_rgba(255, 255, 255, 200));
                draw_circle(cx, tail_y, 2.5 * scale, Color::from_rgba(255, 255, 255, 180));
            }
        }
    }
            // Ghost note at mouse position (hover indicator)
        let (mx, my) = mouse_position();
        //println!("mouse_position {} {}",mx,my);
        if mx >= track_x + ruler_w && mx <= track_x + track_w && my >= track_y && my <= track_y + track_h {
            let dt = (judge_y - my) / SCROLL_SPEED;
            let gt = (now + dt).max(0.0);
            let beat_s = 60.0 / app.chart.bpm;
            let grid_s = beat_s / (GRID_DIVISION as f32 / 4.0);
            let gt = (gt / grid_s).round() * grid_s;
            let gy = judge_y - (gt - now) * SCROLL_SPEED;
            let glx = mx - (track_x + ruler_w);
            if glx >= 0.0 {
                let glane_i = ((glx / lane_w) as i32).clamp(0, LANE_COUNT as i32 - 1);
                let glane = if glane_i == LANE_COUNT as i32 - 1 { 9 } else { (glane_i + 1) as u8 };
                let gcx = track_x + ruler_w + lane_w * glane_i as f32 + lane_w * 0.5;
                let gzone = sanitize_note_zone(super::types::NoteType::Tap, glane);
                if is_touch_zone(gzone) {
                    if let Some(tex) = &app.touch_tri_tex {
                        let ratio = tex.width() / tex.height();
                        let ts = 30.0 * scale;
                        let tw = ts; let th = ts / ratio;
                        draw_texture_ex(tex, gcx - tw * 0.5, gy - th * 0.5 + ts * 0.3, Color::from_rgba(255,255,255,120),
                            DrawTextureParams { dest_size: Some(vec2(tw, th)), ..Default::default() });
                        draw_texture_ex(tex, gcx - tw * 0.5, gy - th * 0.5 - ts * 0.3, Color::from_rgba(255,255,255,120),
                            DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: std::f32::consts::PI, ..Default::default() });
                        draw_texture_ex(tex, gcx - ts * 0.3 - tw * 0.5, gy - th * 0.5, Color::from_rgba(255,255,255,120),
                            DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: std::f32::consts::FRAC_PI_2, ..Default::default() });
                        draw_texture_ex(tex, gcx + ts * 0.3 - tw * 0.5, gy - th * 0.5, Color::from_rgba(255,255,255,120),
                            DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: -std::f32::consts::FRAC_PI_2, ..Default::default() });
                    }
                    if let Some(pt) = &app.touch_point_tex { draw_texture_ex(pt, gcx - 6.0*scale, gy - 6.0*scale, Color::from_rgba(255,255,255,120), DrawTextureParams { dest_size: Some(vec2(12.0*scale, 12.0*scale)), ..Default::default() }); }
                } else {
                    let ghost_alpha = Color::from_rgba(255, 255, 255, 120);
                    match app.place_tool {
                        super::types::PlaceTool::Hold => {
                            // Show hold texture ghost
                            let hold_tex = app.hold_texture.as_ref();
                            if let Some(tex) = hold_tex {
                                let hw = HOLD_WIDTH * scale;
                                let tail_y = gy - 60.0 * scale; // short preview bar
                                draw_hold_9slice_vertical(tex, gcx, gy, tail_y, hw);
                            } else {
                                let hw = 6.0 * scale;
                                draw_rectangle(gcx - hw * 0.5, gy - 60.0 * scale, hw, 60.0 * scale, Color::from_rgba(244, 114, 182, 80));
                            }
                        }
                        super::types::PlaceTool::Star => {
                            // Show star texture ghost (same size as tap)
                            if let Some(tex) = app.star_tex.as_ref() {
                                let ss = TAP_SIZE * scale;
                                draw_texture_ex(tex, gcx - ss * 0.5, gy - ss * 0.5, ghost_alpha,
                                    DrawTextureParams { dest_size: Some(vec2(ss, ss)), ..Default::default() });
                            } else {
                                draw_poly(gcx, gy, 5, 11.0 * scale, 0.0, Color::from_rgba(250, 204, 21, 100));
                            }
                        }
                        super::types::PlaceTool::Tap => {
                            if let Some(tex) = &app.tap_texture {
                                draw_texture_ex(tex, gcx - TAP_SIZE*0.5*scale, gy - TAP_SIZE*0.5*scale, ghost_alpha,
                                    DrawTextureParams { dest_size: Some(vec2(TAP_SIZE*scale, TAP_SIZE*scale)), ..Default::default() });
                            } else {
                                draw_circle(gcx, gy, 11.0*scale, Color::from_rgba(244,114,182,100));
                                draw_circle_lines(gcx, gy, 11.0*scale, 2.5*scale, Color::from_rgba(244,114,182,180));
                            }
                        }
                    }
                }
            }
        }


        // Placement preview for Hold / Star multi-step tools.
        {
            use super::types::{PlacementState, PlaceTool};
            let (mx, my) = mouse_position();
            let inside = mx >= track_x + ruler_w && mx <= track_x + track_w
                && my >= track_y && my <= track_y + track_h;
            // Cursor's snapped chart time and lane (best-effort; only used when inside).
            let cursor_t = if inside {
                let raw = (now + (judge_y - my) / SCROLL_SPEED).max(0.0);
                Some(snap_measure(secs_to_measure(raw, bpm)))
            } else { None };
            // Helper to compute lane center x.
            let lane_cx = |lane: u8| -> f32 {
                let li = if is_touch_zone(sanitize_note_zone(super::types::NoteType::Tap, lane))
                    { LANE_COUNT - 1 }
                    else { (lane.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
                track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5
            };
            let t_to_y = |t: f32| -> f32 { judge_y - (measure_to_secs(t, bpm) - now) * SCROLL_SPEED };
            // Dashed-line helper.
            let dashed = |cx: f32, y0: f32, y1: f32, col: Color| {
                let h = (y1 - y0).abs();
                if h < 0.5 { return; }
                let dash = 6.0 * scale; let gap = 4.0 * scale;
                let period = dash + gap;
                let top = y0.min(y1);
                let n = (h / period).ceil() as i32;
                for k in 0..n {
                    let a = top + (k as f32) * period;
                    let b = (a + dash).min(top + h);
                    draw_line(cx, a, cx, b, 2.0 * scale, col);
                }
            };

            match app.placement {
                PlacementState::HoldPending { anchor_t, lane } => {
                    let cx = lane_cx(lane);
                    let ay = t_to_y(anchor_t);
                    // Anchor dot
                    draw_circle(cx, ay, 6.0 * scale, Color::from_rgba(244, 114, 182, 230));
                    draw_circle_lines(cx, ay, 6.0 * scale, 1.5 * scale, Color::from_rgba(255, 255, 255, 220));
                    if let Some(t2) = cursor_t {
                        let by = t_to_y(t2);
                        let bar_w = HOLD_WIDTH * 0.4 * scale;
                        let top = ay.min(by); let h = (ay - by).abs();
                        if h > 0.5 {
                            draw_rectangle(cx - bar_w * 0.5, top, bar_w, h,
                                Color::from_rgba(244, 114, 182, 90));
                            draw_rectangle_lines(cx - bar_w * 0.5, top, bar_w, h, 1.0 * scale,
                                Color::from_rgba(244, 114, 182, 180));
                        }
                        draw_circle(cx, by, 4.0 * scale, Color::from_rgba(244, 114, 182, 180));
                    }
                }
                PlacementState::StarHead { head_t, lane } => {
                    let cx = lane_cx(lane);
                    let hy = t_to_y(head_t);
                    // Star head marker
                    let ss = 18.0 * scale;
                    if let Some(tex) = &app.star_tex {
                        draw_texture_ex(tex, cx - ss * 0.5, hy - ss * 0.5,
                            Color::from_rgba(255, 255, 255, 230),
                            DrawTextureParams { dest_size: Some(vec2(ss, ss)), ..Default::default() });
                    } else {
                        draw_poly(cx, hy, 5, ss * 0.4, 0.0, Color::from_rgba(250, 204, 21, 230));
                    }
                    if let Some(t2) = cursor_t {
                        if t2 > head_t {
                            let cy = t_to_y(t2);
                            dashed(cx, hy, cy, Color::from_rgba(253, 224, 71, 220));
                            draw_circle(cx, cy, 4.5 * scale, Color::from_rgba(56, 189, 248, 180));
                        }
                    }
                }
                PlacementState::StarDelay { head_t, lane, delay_end_t } => {
                    let cx = lane_cx(lane);
                    let hy = t_to_y(head_t);
                    let dy = t_to_y(delay_end_t);
                    // Star head
                    let ss = 18.0 * scale;
                    if let Some(tex) = &app.star_tex {
                        draw_texture_ex(tex, cx - ss * 0.5, hy - ss * 0.5,
                            Color::from_rgba(255, 255, 255, 230),
                            DrawTextureParams { dest_size: Some(vec2(ss, ss)), ..Default::default() });
                    } else {
                        draw_poly(cx, hy, 5, ss * 0.4, 0.0, Color::from_rgba(250, 204, 21, 230));
                    }
                    // Dashed delay segment
                    dashed(cx, hy, dy, Color::from_rgba(253, 224, 71, 220));
                    // Delay-end handle
                    draw_circle(cx, dy, 4.5 * scale, Color::from_rgba(56, 189, 248, 230));
                    draw_circle_lines(cx, dy, 4.5 * scale, 1.5 * scale, Color::from_rgba(255, 255, 255, 220));
                    // Travel preview to cursor
                    if let Some(t2) = cursor_t {
                        if t2 > delay_end_t {
                            let cy = t_to_y(t2);
                            let bar_w = 6.0 * scale;
                            let top = dy.min(cy); let h = (dy - cy).abs();
                            if h > 0.5 {
                                draw_rectangle(cx - bar_w * 0.5, top, bar_w, h,
                                    Color::from_rgba(250, 204, 21, 110));
                                draw_rectangle_lines(cx - bar_w * 0.5, top, bar_w, h, 1.0 * scale,
                                    Color::from_rgba(253, 224, 71, 200));
                            }
                            draw_circle(cx, cy, 5.0 * scale, Color::from_rgba(250, 204, 21, 180));
                        }
                    }
                    let _ = PlaceTool::Star; // silence unused-warning if tool path changes later
                }
                PlacementState::Idle => {}
            }
        }

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
        // Box selection preview
        if let (Some(start), Some(end)) = (app.box_start, app.box_end) {
            if start != end {
                let x1 = start.x.min(end.x); let x2 = start.x.max(end.x);
                let y1 = start.y.min(end.y); let y2 = start.y.max(end.y);
                draw_rectangle(x1, y1, x2 - x1, y2 - y1, Color::from_rgba(56, 189, 248, 30));
                draw_rectangle_lines(x1, y1, x2 - x1, y2 - y1, 1.5 * scale, Color::from_rgba(56, 189, 248, 180));
            }
        }
        // Multi-select highlight
        for &i in &app.selected_notes {
            if let Some(note) = app.chart.notes.get(i) {
                let zone = sanitize_note_zone(note.note_type, note.lane);
                let dt = note_secs(note, bpm) - now;
                if dt < -0.3 || dt > PREVIEW_LEAD_TIME { continue; }
                let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
                let cx = track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
                let ny = judge_y - dt * SCROLL_SPEED;
                draw_circle(cx, ny, 15.0, Color::from_rgba(250, 204, 21, 80));
            }
        }

        // Paste ghost
        if app.pasting && !app.clipboard.is_empty() {
            let (mx, my) = mouse_position();
            let dt = (judge_y - my) / SCROLL_SPEED;
            let raw_secs = (now + dt).max(0.0);
            let grid_step = 1.0 / super::types::GRID_DIVISION as f32;
            let raw_m = secs_to_measure(raw_secs, bpm);
            let target_m = (raw_m / grid_step).round() * grid_step;
            let min_t = app.clipboard.iter().map(|n| n.time).fold(f32::MAX, f32::min);
            let t_off = target_m - min_t;
            let lx = mx - (track_x + ruler_w);
            let tgt_lane = if lx >= 0.0 {
                let l = (lx / lane_w) as i32; let l = l.clamp(0, LANE_COUNT as i32 - 1) as u8;
                if l == LANE_COUNT as u8 - 1 { 9 } else { l + 1 }
            } else { 1 };
            let a_lane = app.clipboard.first().map(|n| n.lane).unwrap_or(1);
            let l_off = tgt_lane as i32 - a_lane as i32;
            for n in &app.clipboard {
                let t_m = n.time + t_off;
                let lane = (n.lane as i32 + l_off).clamp(1, super::types::PAD_ZONE_MAX as i32) as u8;
                let zone = sanitize_note_zone(n.note_type, lane);
                let dt2 = measure_to_secs(t_m, bpm) - now;
                if dt2 < -0.3 || dt2 > PREVIEW_LEAD_TIME { continue; }
                let li = if is_touch_zone(zone) { LANE_COUNT - 1 } else { (zone.saturating_sub(1) as usize).min(LANE_COUNT - 1) };
                let cx = track_x + ruler_w + lane_w * li as f32 + lane_w * 0.5;
                let ny = judge_y - dt2 * SCROLL_SPEED;
                if is_touch_zone(zone) || matches!(n.note_type, super::types::NoteType::Touch) {
                    if let Some(tex) = &app.touch_tri_tex {
                        let ratio = tex.width() / tex.height();
                        let ts = 20.0 * scale; let tw = ts; let th = ts / ratio;
                        let c = Color::from_rgba(255,255,255,120);
                        draw_texture_ex(tex, cx - tw*0.5, ny + ts*0.15 - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), ..Default::default() });
                        draw_texture_ex(tex, cx - tw*0.5, ny - ts*0.15 - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: std::f32::consts::PI, ..Default::default() });
                        draw_texture_ex(tex, cx - ts*0.15 - tw*0.5, ny - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: std::f32::consts::FRAC_PI_2, ..Default::default() });
                        draw_texture_ex(tex, cx + ts*0.15 - tw*0.5, ny - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: -std::f32::consts::FRAC_PI_2, ..Default::default() });
                    }
                    if let Some(pt) = &app.touch_point_tex {
                        draw_texture_ex(pt, cx - 5.0*scale, ny - 5.0*scale, Color::from_rgba(255,255,255,120), DrawTextureParams { dest_size: Some(vec2(10.0*scale,10.0*scale)), ..Default::default() });
                    }
                } else {
                    if let Some(tex) = &app.tap_texture {
                        draw_texture_ex(tex, cx - TAP_SIZE*0.4*scale, ny - TAP_SIZE*0.4*scale, Color::from_rgba(255,255,255,120), DrawTextureParams { dest_size: Some(vec2(TAP_SIZE*0.8*scale, TAP_SIZE*0.8*scale)), ..Default::default() });
                    } else {
                        draw_circle(cx, ny, 11.0*scale, Color::from_rgba(244,114,182,80));
                        draw_circle_lines(cx, ny, 11.0*scale, 2.0*scale, Color::from_rgba(244,114,182,160));
                    }
                }
            }
        }

        // Judge line on top
        draw_line(track_x + ruler_w, judge_y, track_x + track_w, judge_y, 2.0 * scale, Color::from_rgba(239, 68, 68, 255));
}

pub(crate) fn draw_pad_only(app: &AppState, pad: PadGeom, rect: RectF) {
    draw_pad_panel(app, rect, pad);
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
    // Tap spawn center: C zone centroid for alignment
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
            let text_size = 17.0 * scale;
            let text_dims = measure_text(&def.label, None, text_size as _, 1.0);
            draw_text(
                &def.label,
                centroid.x - text_dims.width * 0.5,
                centroid.y + text_dims.height * 0.35,
                text_size,
                text_color,
            );
        }

        // Draw A-zone tap indicators with connecting octagon
        // Draw A-zone tap indicators as a perfect circle centered on spawn_cx
        let dot_r = outer_r + TAP_RING_OFFSET * scale;
        let mut a_dots: Vec<Vec2> = Vec::new();
        for i in 0..8 {
            let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + i as f32 * std::f32::consts::TAU / 8.0;
            a_dots.push(vec2(spawn_cx.x + ang.cos() * dot_r, spawn_cx.y + ang.sin() * dot_r));
        }
        for i in 0..8 {
            let j = (i + 1) % 8;
            draw_line(a_dots[i].x, a_dots[i].y, a_dots[j].x, a_dots[j].y, 2.0 * scale, Color::from_rgba(255, 255, 255, 120));
        }
        for dot in &a_dots {
            draw_circle(dot.x, dot.y, 5.0 * scale, Color::from_rgba(255, 255, 255, 220));
        }
    }

    let current_t = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };

    // ── Trajectory edit overlay ──
    // When the user is editing a slide note's path, highlight its current
    // start + slide_points with numbered markers and connecting polyline so
    // they can see what will be appended next.
    if let (Some(i), Some(svg)) = (app.editing_slide_path, app.pad_svg.as_ref()) {
        if let Some(note) = app.chart.notes.get(i) {
            if matches!(note.note_type, NoteType::Slide) {
                let mut pts: Vec<(Vec2, String)> = Vec::new();
                if let Some(c) = svg.zone_screen_centroid(note.lane, &pad) {
                    pts.push((c, format!("S{}", note.lane)));
                }
                for (k, sp) in note.slide_points.iter().enumerate() {
                    if let Some(c) = svg.zone_screen_centroid(sp.zone, &pad) {
                        pts.push((c, format!("{}", k + 1)));
                    }
                }
                let line_col = Color::from_rgba(250, 204, 21, 220);
                for w in pts.windows(2) {
                    draw_line(w[0].0.x, w[0].0.y, w[1].0.x, w[1].0.y, 4.0 * scale, line_col);
                }
                for (p, lbl) in &pts {
                    draw_circle(p.x, p.y, 12.0 * scale, Color::from_rgba(250, 204, 21, 220));
                    draw_circle_lines(p.x, p.y, 12.0 * scale, 2.0 * scale, BLACK);
                    let sz = 16.0 * scale;
                    let dims = measure_text(lbl, None, sz as _, 1.0);
                    draw_text(lbl, p.x - dims.width * 0.5, p.y + dims.height * 0.35, sz, BLACK);
                }
                let banner = format!(
                    "Trajectory edit  #{}  pts={}  shape={:?}  [click=add  Bksp=undo  Esc/E=exit]",
                    i, note.slide_points.len(), note.slide_shape
                );
                draw_rectangle(
                    rect.x + 8.0 * scale, rect.y + 36.0 * scale,
                    (rect.w - 16.0 * scale).max(10.0), 24.0 * scale,
                    Color::from_rgba(250, 204, 21, 60),
                );
                draw_text(&banner, rect.x + 14.0 * scale, rect.y + 53.0 * scale,
                    16.0 * scale, Color::from_rgba(250, 204, 21, 255));
            }
        }
    }

    let bpm = app.chart.bpm;
    for note in &app.chart.notes {
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let ns = note_secs(note, bpm);
        let dt = ns - current_t;
        let tail_dt = if matches!(note.note_type, NoteType::Hold) {
            hold_tail_time(note, bpm) - current_t
        } else {
            dt
        };

        let lead_time = if zone <= 8 {
            if matches!(note.note_type, NoteType::Hold) { HOLD_FLY_TIME }
            else if matches!(note.note_type, NoteType::Slide) { SLIDE_TRAVEL_TIME }
            else { TAP_TRAVEL_TIME }
        } else {
            match note.note_type {
                NoteType::Hold => HOLD_TRAVEL_TIME,
                NoteType::Slide => SLIDE_TRAVEL_TIME,
                _ => TOUCH_TRAVEL_TIME,
            }
        };
        let slide_tail_dt = if matches!(note.note_type, NoteType::Slide) {
            slide_end_time(note, bpm) - current_t
        } else {
            tail_dt
        };
        let disappear_time = if matches!(note.note_type, NoteType::Touch) { TOUCH_DISAPPEAR_TIME }
            else if matches!(note.note_type, NoteType::Slide) { 0.3 }
            else { 0.18 };
        if slide_tail_dt < -disappear_time || dt > lead_time {
            continue;
        }

        // ── Inline Slide rendering (path tiles + flying star + touch-zone fade-in head) ──
        // Drawn here so layering follows chart order: later notes cover earlier ones.
        if matches!(note.note_type, NoteType::Slide) && !note.slide_points.is_empty() {
            let slide_dur_s = mdur_to_secs(note.slide_duration, bpm).max(0.3);
            let slide_end_s = ns + slide_dur_s;
            if dt <= SLIDE_TRAVEL_TIME && current_t <= slide_end_s + 0.2 {
                let slide_tex = if note.is_break { app.slide_break_tex.as_ref() } else if note.is_each { app.slide_each_tex.as_ref() } else { app.slide_tex.as_ref() };
                let (tw, th) = if let Some(t) = slide_tex {
                    (t.width() * scale * SLIDE_TILE_SCALE, t.height() * scale * SLIDE_TILE_SCALE)
                } else {
                    (SLIDE_TILE_SIZE * scale, SLIDE_TILE_SIZE * scale)
                };

                // Build path of centroids
                let mut path: Vec<Vec2> = Vec::new();
                if let Some(ref svg) = app.pad_svg {
                    let start_pt = if note.lane <= 8 {
                        let idx = (note.lane - 1) as f32;
                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                        let target_r = outer_r + TAP_TARGET_OFFSET;
                        Some(vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r))
                    } else {
                        svg.zone_screen_centroid(note.lane, &pad)
                    };
                    if let Some(c) = start_pt { path.push(c); }
                    for sp in &note.slide_points {
                        if sp.zone == note.lane && path.len() == 1 { continue; }
                        let c = if sp.zone >= 1 && sp.zone <= 8 {
                            let idx = (sp.zone - 1) as f32;
                            let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                            let target_r = outer_r + TAP_TARGET_OFFSET;
                            Some(vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r))
                        } else {
                            svg.zone_screen_centroid(sp.zone, &pad)
                        };
                        if let Some(c) = c {
                            if path.last().map(|p| (*p - c).length() > 1.0).unwrap_or(true) {
                                path.push(c);
                            }
                        }
                    }
                }

                if path.len() >= 2 {
                    let seg_lens: Vec<f32> = path.windows(2).map(|w| (w[1] - w[0]).length().max(0.001)).collect();
                    let total_len: f32 = seg_lens.iter().sum();

                    // Honor the full configured delay; clamp only to the total
                    // slide duration so travel_dur stays positive.
                    let fade_in_s = mdur_to_secs(note.slide_start_delay, bpm)
                        .max(0.0)
                        .min(slide_dur_s - 0.001)
                        .max(0.001);
                    let path_alpha: u8 = if dt > 0.0 {
                        0
                    } else {
                        let f = ((current_t - ns) / fade_in_s).clamp(0.0, 1.0);
                        (f * 220.0) as u8
                    };
                    let travel_dur_s = (slide_dur_s - fade_in_s).max(0.001);
                    let star_t = if current_t < ns + fade_in_s {
                        0.0
                    } else {
                        ((current_t - ns - fade_in_s) / travel_dur_s).clamp(0.0, 1.0)
                    };
                    let star_dist_along = star_t * total_len;

                    let point_at = |d: f32| -> (Vec2, f32) {
                        let mut acc = 0.0;
                        for (i, w) in path.windows(2).enumerate() {
                            let len = seg_lens[i];
                            if d <= acc + len {
                                let local = (d - acc) / len;
                                let p = w[0] + (w[1] - w[0]) * local;
                                let dir = (w[1] - w[0]).normalize_or_zero();
                                return (p, dir.y.atan2(dir.x));
                            }
                            acc += len;
                        }
                        let last = path.windows(2).last().unwrap();
                        let dir = (last[1] - last[0]).normalize_or_zero();
                        (*path.last().unwrap(), dir.y.atan2(dir.x))
                    };

                    // Path tiles
                    for (si, w) in path.windows(2).enumerate() {
                        let a = w[0]; let b = w[1];
                        let seg_len = seg_lens[si];
                        let dir = (b - a) / seg_len;
                        let angle = dir.y.atan2(dir.x) + std::f32::consts::PI;
                        let spacing = SLIDE_TILE_SPACING * scale;
                        let seg_start_d: f32 = seg_lens.iter().take(si).sum();
                        let mut pos = 0.0;
                        while pos < seg_len {
                            let abs_d = seg_start_d + pos;
                            if abs_d < star_dist_along { pos += spacing; continue; }
                            let pt = a + dir * pos;
                            if let Some(tex) = slide_tex {
                                draw_texture_ex(tex, pt.x - tw * 0.5, pt.y - th * 0.5,
                                    Color::from_rgba(255, 255, 255, path_alpha),
                                    DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: angle, ..Default::default() });
                            }
                            pos += spacing;
                        }
                    }

                    // Touch-zone slide pre-judge head star (A-zone heads are drawn below in the main branch)
                    // Skip for tapless slides (they share the parent star's head).
                    if note.lane > 8 && dt > 0.0 && dt < SLIDE_TRAVEL_TIME && !note.is_tapless {
                        let head_progress = ((SLIDE_TRAVEL_TIME - dt) / SLIDE_TRAVEL_TIME).clamp(0.0, 1.0);
                        let size_scale = if head_progress < TAP_GROW_FRAC { head_progress / TAP_GROW_FRAC } else { 1.0 };
                        let ss = STAR_SIZE * scale * size_scale;
                        let dbl = note.is_star;
                        let star_tex = if note.star_is_break {
                            if dbl { app.star_double_break_tex.as_ref() } else { app.star_break_tex.as_ref() }
                        } else if note.is_each {
                            if dbl { app.star_double_each_tex.as_ref() } else { app.star_each_tex.as_ref() }
                        } else {
                            if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() }
                        };
                        let fb = if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() };
                        let head_rot = head_progress * std::f32::consts::TAU;
                        if let Some(tex) = star_tex.or(fb).or(app.star_tex.as_ref()) {
                            draw_texture_ex(tex, path[0].x - ss * 0.5, path[0].y - ss * 0.5, WHITE,
                                DrawTextureParams { dest_size: Some(vec2(ss, ss)), rotation: head_rot, ..Default::default() });
                            if note.star_is_ex {
                                let ex = if dbl { app.star_double_ex_tex.as_ref() } else { app.star_ex_tex.as_ref() };
                                if let Some(ex_tex) = ex.or(app.star_ex_tex.as_ref()) {
                                    draw_texture_ex(ex_tex, path[0].x - ss * 0.5, path[0].y - ss * 0.5, WHITE,
                                        DrawTextureParams { dest_size: Some(vec2(ss, ss)), rotation: head_rot, ..Default::default() });
                                }
                            }
                        }
                    }

                    // Flying star (post-judge)
                    if current_t >= ns && current_t <= slide_end_s {
                        let (star_pos, angle) = point_at(star_dist_along);
                        let ss = STAR_SIZE * scale;
                        let dbl = note.is_star;
                        let star_tex = if note.star_is_break {
                            if dbl { app.star_double_break_tex.as_ref() } else { app.star_break_tex.as_ref() }
                        } else if note.is_each {
                            if dbl { app.star_double_each_tex.as_ref() } else { app.star_each_tex.as_ref() }
                        } else {
                            if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() }
                        };
                        let fb = if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() };
                        if let Some(tex) = star_tex.or(fb).or(app.star_tex.as_ref()) {
                            draw_texture_ex(tex, star_pos.x - ss * 0.5, star_pos.y - ss * 0.5, WHITE,
                                DrawTextureParams { dest_size: Some(vec2(ss, ss)), rotation: angle, ..Default::default() });
                            if note.star_is_ex {
                                let ex = if dbl { app.star_double_ex_tex.as_ref() } else { app.star_ex_tex.as_ref() };
                                if let Some(ex_tex) = ex.or(app.star_ex_tex.as_ref()) {
                                    draw_texture_ex(ex_tex, star_pos.x - ss * 0.5, star_pos.y - ss * 0.5, WHITE,
                                        DrawTextureParams { dest_size: Some(vec2(ss, ss)), rotation: angle, ..Default::default() });
                                }
                            }
                        } else {
                            draw_poly(star_pos.x, star_pos.y, 5, ss * 0.4, angle.to_degrees(),
                                Color::from_rgba(250, 204, 21, 255));
                        }
                    }
                }
            }
        }

        // A-zone tap disappears at dt fraction; hold disappears at tail fraction
        if zone <= 8 {
            if matches!(note.note_type, NoteType::Hold) {
                if tail_dt <= (hold_tail_time(note, bpm) - ns) * HOLD_DISAPPEAR_FRAC {
                    continue;
                }
            } else if !matches!(note.note_type, NoteType::Slide) && dt <= TAP_TRAVEL_TIME * TAP_DISAPPEAR_FRAC {
                continue;
            }
        }

        if zone <= 8 {
            let idx = (zone - 1) as f32;
            let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
            let dir = vec2(ang.cos(), ang.sin());
            let head_travel = if matches!(note.note_type, NoteType::Slide) { SLIDE_TRAVEL_TIME } else { TAP_TRAVEL_TIME };
            let progress = ((head_travel - dt) / head_travel).clamp(0.0, 1.0);
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
            let target_r = outer_r + TAP_TARGET_OFFSET;
            // r = midpoint (grow: fixed at spawn, fly: moves to target)
            let r = spawn_r + (target_r - spawn_r) * fly_progress;
            let px = spawn_cx.x + dir.x * r;
            let py = spawn_cx.y + dir.y * r;

            if matches!(note.note_type, NoteType::Hold) {
                let h_spawn_r = outer_r * HOLD_SPAWN_FRAC;
                let h_target_r = outer_r + HOLD_TARGET_OFFSET;
                let h_progress = ((HOLD_FLY_TIME - dt) / HOLD_FLY_TIME).clamp(0.0, 1.0);
                let h_size_scale = if h_progress < TAP_GROW_FRAC { h_progress / TAP_GROW_FRAC } else { 1.0 };
                let h_fly_progress = if h_progress < TAP_GROW_FRAC { 0.0 } else { (h_progress - TAP_GROW_FRAC) / (1.0 - TAP_GROW_FRAC) };
                let full_hold_len = (h_target_r - h_spawn_r) * HOLD_LENGTH_FRAC;
                // Uniform scale: length and width both scale 0→1 during grow
                let hold_half = (full_hold_len * h_size_scale * 0.5).max(2.0);
                // Head flies during fly phase using hold's own fly time
                let head_fly_r = h_spawn_r + (h_target_r - h_spawn_r) * h_fly_progress;
                let head_r = (head_fly_r + hold_half).min(h_target_r);
                // Tail lags at spawn, flies to target in last HOLD_TAIL_FLY_TIME seconds
                let tail_dt = hold_tail_time(note, bpm) - current_t;
                let tail_fly = if tail_dt <= HOLD_TAIL_FLY_TIME {
                    (1.0 - tail_dt / HOLD_TAIL_FLY_TIME).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let tail_base = h_spawn_r + (h_target_r - h_spawn_r) * tail_fly;
                let tail_r = (tail_base - hold_half).max(h_spawn_r * 0.1);
                let hx = spawn_cx.x + dir.x * head_r;
                let hy = spawn_cx.y + dir.y * head_r;
                let tx = spawn_cx.x + dir.x * tail_r;
                let ty = spawn_cx.y + dir.y * tail_r;
                // Width scales 0→1 during grow, body length stays full
                let hold_w = HOLD_WIDTH * scale * h_size_scale;
                let hold_tex = if note.is_break {
                    app.hold_break_tex.as_ref()
                } else if note.is_each {
                    app.hold_each_tex.as_ref()
                } else {
                    app.hold_texture.as_ref()
                };
                if let Some(tex) = hold_tex.or(app.hold_texture.as_ref()) {
                    draw_hold_9slice_segment(tex, vec2(hx, hy), vec2(tx, ty), hold_w.max(1.0), Color::from_rgba(255, 255, 255, 255));
                    if note.is_ex {
                        if let Some(ex_tex) = app.hold_ex_tex.as_ref() {
                            draw_hold_9slice_segment(ex_tex, vec2(hx, hy), vec2(tx, ty), hold_w.max(1.0), Color::from_rgba(255, 255, 255, 255));
                        }
                    }
                } else {
                    draw_line(hx, hy, tx, ty, HOLD_WIDTH * 0.233 * scale * h_size_scale, Color::from_rgba(251, 113, 133, 200));
                    draw_circle(tx, ty, HOLD_WIDTH * 0.167 * scale * h_size_scale, Color::from_rgba(253, 164, 175, 255));
                }
            }

            if matches!(note.note_type, NoteType::Slide) {
                // Star head flies in like a tap before judge time. Hide once the
                // dedicated slide section takes over (at/after note.time).
                // Skip for tapless slides (they share the parent star's head).
                if dt > 0.0 && !note.is_tapless {
                    let ss = STAR_SIZE * scale * size_scale;
                    let dbl = note.is_star;
                    let star_tex = if note.star_is_break {
                        if dbl { app.star_double_break_tex.as_ref() } else { app.star_break_tex.as_ref() }
                    } else if note.is_each {
                        if dbl { app.star_double_each_tex.as_ref() } else { app.star_each_tex.as_ref() }
                    } else {
                        if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() }
                    };
                    let fb = if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() };
                    let star_rot = fly_progress * std::f32::consts::TAU;
                    if let Some(tex) = star_tex.or(fb).or(app.star_tex.as_ref()) {
                        draw_texture_ex(tex, px - ss * 0.5, py - ss * 0.5, WHITE, DrawTextureParams {
                            dest_size: Some(vec2(ss, ss)),
                            rotation: star_rot,
                            ..Default::default()
                        });
                        if note.star_is_ex {
                            let ex = if dbl { app.star_double_ex_tex.as_ref() } else { app.star_ex_tex.as_ref() };
                            if let Some(ex_tex) = ex.or(app.star_ex_tex.as_ref()) {
                                draw_texture_ex(ex_tex, px - ss * 0.5, py - ss * 0.5, WHITE, DrawTextureParams {
                                    dest_size: Some(vec2(ss, ss)),
                                    rotation: star_rot,
                                    ..Default::default()
                                });
                            }
                        }
                    } else {
                        let tr = STAR_SIZE * 0.4 * scale * size_scale;
                        draw_poly(px, py, 4, tr, star_rot.to_degrees(), Color::from_rgba(250, 204, 21, 255));
                        draw_poly_lines(px, py, 4, tr, star_rot.to_degrees(), 2.0 * scale, Color::from_rgba(253, 224, 71, 255));
                    }
                }
            } else if !matches!(note.note_type, NoteType::Hold) {
                let ts = TAP_SIZE * scale * size_scale;
                let tap_tex = if note.is_break {
                    app.tap_break_tex.as_ref()
                } else if note.is_each {
                    app.tap_each_tex.as_ref()
                } else {
                    app.tap_texture.as_ref()
                };
                if let Some(tex) = tap_tex.or(app.tap_texture.as_ref()) {
                    draw_texture_ex(tex, px - ts * 0.5, py - ts * 0.5, WHITE, DrawTextureParams {
                        dest_size: Some(vec2(ts, ts)),
                        ..Default::default()
                    });
                    // Ex overlay on top, same size
                    if note.is_ex {
                        if let Some(ex_tex) = app.tap_ex_tex.as_ref() {
                            draw_texture_ex(ex_tex, px - ts * 0.5, py - ts * 0.5, WHITE, DrawTextureParams {
                                dest_size: Some(vec2(ts, ts)),
                                ..Default::default()
                            });
                        }
                    }
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
            // Slide head in touch zone is drawn in the dedicated slide section below
            if matches!(note.note_type, NoteType::Slide) {
                continue;
            }
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
            // Phase 1: fade in 0→255. Phase 2: animate movement.
            let alpha = if progress < TOUCH_GROW_FRAC {
                (progress / TOUCH_GROW_FRAC * 255.0) as u8
            } else {
                255
            };
            let move_progress = if progress < TOUCH_GROW_FRAC {
                0.0
            } else {
                (progress - TOUCH_GROW_FRAC) / (1.0 - TOUCH_GROW_FRAC)
            };
            let dist = (TOUCH_START_DIST + (TOUCH_END_DIST - TOUCH_START_DIST) * move_progress) * TOUCH_SCALE * scale;
            let ts = TOUCH_CROSS_SIZE * TOUCH_SCALE * scale;

            // Regular touch cross (skip for hold)
            if !matches!(note.note_type, NoteType::Hold) {
                let tri_tex = if note.is_each { app.touch_tri_each_tex.as_ref() } else { app.touch_tri_tex.as_ref() };
                if let Some(tex) = tri_tex {
                    let ratio = tex.width() / tex.height();
                    let tw = ts;
                    let th = ts / ratio;
                    let draw_tri = |cx: f32, cy: f32, rot: f32| {
                        draw_texture_ex(tex, cx - tw * 0.5, cy - th * 0.5,
                            Color::from_rgba(255, 255, 255, alpha),
                            DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: rot, ..Default::default() });
                    };
                    draw_tri(center.x, center.y + dist, 0.0);
                    draw_tri(center.x, center.y - dist, std::f32::consts::PI);
                    draw_tri(center.x - dist, center.y, std::f32::consts::FRAC_PI_2);
                    draw_tri(center.x + dist, center.y, -std::f32::consts::FRAC_PI_2);
                }
            }
            // Center dot (for non-hold; hold draws it later on top)
            if !matches!(note.note_type, NoteType::Hold) {
                let pt_tex = if note.is_each { app.touch_point_each_tex.as_ref() } else { app.touch_point_tex.as_ref() };
                if let Some(tex) = pt_tex {
                    let ps = ts * 0.4;
                    draw_texture_ex(tex, center.x - ps * 0.5, center.y - ps * 0.5,
                        Color::from_rgba(255, 255, 255, alpha),
                        DrawTextureParams { dest_size: Some(vec2(ps, ps)), ..Default::default() });
                }
            }

            if matches!(note.note_type, NoteType::Hold) {
                // Touch hold: 4-texture cross rotated 45°, with progress border
                let hold_progress = ((current_t - ns) / (hold_tail_time(note, bpm) - ns).max(0.01)).clamp(0.0, 1.0);
                let hold_dist = (TOUCHHOLD_START_DIST + (TOUCHHOLD_END_DIST - TOUCHHOLD_START_DIST) * move_progress) * TOUCHHOLD_SCALE * scale;
                let d = hold_dist * 0.707; // √2/2 for diagonal
                // Cross rotated 45° CW from regular touch, starting top-right
                let hts = TOUCHHOLD_CROSS_BASE * TOUCHHOLD_SCALE * scale;
                let ro = TOUCHHOLD_ROT_OFFSET;
                let positions = [
                    (center.x + d, center.y - d, -3.0 * std::f32::consts::FRAC_PI_4 + ro), // top-right (0)
                    (center.x + d, center.y + d, -std::f32::consts::FRAC_PI_4 + ro),        // bottom-right (1)
                    (center.x - d, center.y + d, std::f32::consts::FRAC_PI_4 + ro),         // bottom-left (2)
                    (center.x - d, center.y - d, 3.0 * std::f32::consts::FRAC_PI_4 + ro),   // top-left (3)
                ];
                for (i, (px, py, rot)) in positions.iter().enumerate() {
                    if let Some(tex) = &app.touchhold_tex[i] {
                        let ratio = tex.width() / tex.height();
                        let tw = hts;
                        let th = hts / ratio;
                        draw_texture_ex(tex, px - tw * 0.5, py - th * 0.5,
                            Color::from_rgba(255, 255, 255, alpha),
                            DrawTextureParams { dest_size: Some(vec2(tw, th)), rotation: *rot, ..Default::default() });
                    }
                }
                // Progress border: shader-based clockwise sweep
                if let Some(border) = &app.touchhold_border_tex {
                    let bs = TOUCHHOLD_BORDER_BASE * TOUCHHOLD_SCALE * scale;
                    // Ghost ring
                    draw_texture_ex(border, center.x - bs * 0.5, center.y - bs * 0.5,
                        Color::from_rgba(255, 255, 255, 0),
                        DrawTextureParams { dest_size: Some(vec2(bs, bs)), ..Default::default() });
                    // Shader sweep
                    if let Some(ref mat) = app.mask_material {
                        macroquad::material::gl_use_material(mat);
                        mat.set_uniform("progress", hold_progress);
                    }
                    draw_texture_ex(border, center.x - bs * 0.5, center.y - bs * 0.5,
                        WHITE,
                        DrawTextureParams { dest_size: Some(vec2(bs, bs)), ..Default::default() });
                    if app.mask_material.is_some() {
                        macroquad::material::gl_use_default_material();
                    }
                }
                // Center dot on top for hold
                let pt_tex = if note.is_each { app.touch_point_each_tex.as_ref() } else { app.touch_point_tex.as_ref() };
                if let Some(tex) = pt_tex {
                    let ps = hts * 0.4;
                    draw_texture_ex(tex, center.x - ps * 0.5, center.y - ps * 0.5,
                        Color::from_rgba(255, 255, 255, alpha),
                        DrawTextureParams { dest_size: Some(vec2(ps, ps)), ..Default::default() });
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

fn freq_to_color(freq_frac: f32, norm: f32) -> Color {
    let alpha = (norm * 200.0) as u8;
    if freq_frac < 0.33 {
        Color::from_rgba(0, (norm*255.0) as u8, (norm*200.0) as u8, alpha)
    } else if freq_frac < 0.66 {
        Color::from_rgba((norm*200.0) as u8, (norm*255.0) as u8, 0, alpha)
    } else {
        Color::from_rgba((norm*255.0) as u8, (norm*100.0) as u8, (norm*100.0) as u8, alpha)
    }
}
