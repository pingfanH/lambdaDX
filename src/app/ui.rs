use macroquad::prelude::*;
use macroquad::texture::{load_texture, DrawTextureParams, FilterMode, Texture2D};

use super::chart;
use super::slide_render;
use super::template;
use crate::app::slide::path::*;
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
    PAD_ROTATION_RAD, TAP_RING_OFFSET, GRID_DIVISION, NoteType, SlideShape, TAP_SIZE, HOLD_WIDTH, TOUCH_SIZE,
    FIXED_SLIDE_FADE_IN,
    note_secs, measure_to_secs, secs_to_measure, mdur_to_secs, snap_measure, bpm_at,
};
use super::pad_svg;
use super::types::zone::PadZone;

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
pub async fn load_note_textures(app: &mut AppState) {
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
    for i in 0..11 {
        for path in [format!("Skins/classic/wifi_{i}.png"), format!("wifi_{i}.png")] {
            if let Ok(tex) = load_texture(&path).await {
                tex.set_filter(FilterMode::Linear);
                app.wifi_tex[i as usize] = Some(tex);
                break;
            }
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

pub fn compute_layout(app: &AppState) -> Layout {
    let scale = ui_scale(app);
    let sw = screen_width();
    let sh = screen_height();
    let margin = 20.0 * scale;
    // Prototype toolbar: TOOLBAR_HEIGHT(24) + PADDING(6) = 30 logical → ~60 physical at 2x DPI
    let toolbar_h = 60.0;
    // Prototype sidebar: SIDEBAR_WIDTH(36) + PADDING(6) = 42 logical → ~84 physical at 2x DPI
    let sidebar_w = 84.0;

    let header = RectF { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };

    if app.show_pad_only || app.mobile_ui {
        let pad = RectF {
            x: margin,
            y: toolbar_h,
            w: sw - margin * 2.0,
            h: sh - toolbar_h - margin,
        };
        return Layout { header, timeline: None, pad };
    }

    let content_x = sidebar_w + margin;
    let content_w = sw - content_x - margin;
    let content_y = toolbar_h;
    let content_h = sh - toolbar_h - margin;
    // Prototype: timeline 70%, viewport 30%
    let timeline_w = content_w * 0.7;

    let timeline = RectF {
        x: content_x,
        y: content_y,
        w: timeline_w,
        h: content_h,
    };
    // Pad starts right after timeline (no extra gap), matches egui viewport position
    let pad = RectF {
        x: content_x + timeline_w,
        y: content_y,
        w: content_w - timeline_w,
        h: content_h,
    };
    Layout { header, timeline: Some(timeline), pad }
}

pub fn compute_pad_geom(panel: RectF) -> PadGeom {
    let cx = panel.x + panel.w * 0.5;
    let cy = panel.y + panel.h * 0.5;
    let outer_r = panel.w.min(panel.h) * 0.42;
    PadGeom { cx, cy, outer_r }
}

pub fn build_ui_buttons(layout: Layout, app: &AppState) -> Vec<UiButton> {
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

pub fn draw_layout(app: &AppState, layout: Layout, pad: PadGeom, _buttons: &[UiButton]) {
    if let Some(timeline) = layout.timeline {
        let s = ui_scale(app);
        draw_text(&format!("Wave threshold: {:.2}  [/] keys", app.waveform_threshold),
            timeline.x + 14.0 * s, timeline.y + 24.0 * s, 16.0 * s,
            Color::from_rgba(140, 140, 140, 200));
        super::egui_ui::draw_timeline::draw_timeline_panel(app, timeline);
    }

    // Pad in upper 60% of viewport area (matches egui viewport pad region)
    let pad_area_h = layout.pad.h * 0.6;
    let pad_area = RectF {
        x: layout.pad.x,
        y: layout.pad.y,
        w: layout.pad.w,
        h: pad_area_h,
    };
    let pad_geom = compute_pad_geom(pad_area);
    draw_pad_panel(app, pad_area, pad_geom);
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
            Color::from_rgba(74, 125, 170, 255)
        } else {
            Color::from_rgba(60, 60, 60, 255)
        };
        draw_rectangle(b.rect.x, b.rect.y, b.rect.w, b.rect.h, bg);
        draw_rectangle_lines(
            b.rect.x,
            b.rect.y,
            b.rect.w,
            b.rect.h,
            1.0 * scale,
            Color::from_rgba(70, 70, 70, 255),
        );
        draw_text(
            b.label,
            b.rect.x + 10.0 * scale,
            b.rect.y + 18.0 * scale,
            16.0 * scale,
            Color::from_rgba(224, 224, 224, 255),
        );
    }
}

pub fn rect_contains(r: RectF, p: Vec2) -> bool {
    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
}

pub fn trigger_ui_action(app: &mut AppState, action: UiAction) {
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



pub fn draw_hold_9slice_segment(
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
        if part_len <= 0.0 { return; }
       let center = from + dir * (start_offset + part_len * 0.5);
        draw_texture_ex(
            tex, center.x - width * 0.5, center.y - part_len * 0.5, tint,
            DrawTextureParams {
                source: Some(Rect { x: 0.0, y: src_y, w: tex_w, h: src_h }),
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

pub fn draw_pad_only(app: &AppState, pad: PadGeom, rect: RectF) {
    draw_pad_panel(app, rect, pad);
}

fn draw_pad_panel(app: &AppState, rect: RectF, pad: PadGeom) {
    let scale = ui_scale(app);
    let in_isolation = template::is_in_isolation(app);

    // Pad area is transparent — egui viewport draws background behind.

    // Title: show template name in isolation, otherwise "Pad View".
    // if in_isolation {
    //     let tpl_name = template::current_template_name(app)
    //         .unwrap_or_else(|| "Template".to_string());
    //     let label = format!("Editing: {} (ESC to exit)", tpl_name);
    //     draw_text(
    //         &label,
    //         rect.x + 12.0 * scale,
    //         rect.y + 24.0 * scale,
    //         20.0 * scale,
    //         Color::from_rgba(230, 149, 48, 255), // orange
    //     );
    // } else {
    //     draw_text(
    //         "Pad View",
    //         rect.x + 12.0 * scale,
    //         rect.y + 24.0 * scale,
    //         24.0 * scale,
    //         Color::from_rgba(180, 180, 180, 255),
    //     );
    // }

    let cx = pad.cx;
    let cy = pad.cy;
    let outer_r = pad.outer_r;
    // Tap spawn center: C zone centroid for alignment
    let spawn_cx = app.pad_svg.as_ref()
        .and_then(|svg| svg.pad_visual_center(&pad))
        .unwrap_or(vec2(cx, cy));

    draw_circle(cx, cy, outer_r, Color::from_rgba(35, 35, 35, 255));

    // Tap spawn point indicator
    draw_circle(spawn_cx.x, spawn_cx.y, 3.0 * scale, Color::from_rgba(255, 255, 255, 180));

    let active_zones: Vec<PadZone> = app.active_pointer_zones.values().copied().collect();
    let feedback_zones: Vec<PadZone> = app.pad_feedback.iter().map(|fb| fb.zone).collect();

    if let Some(ref pad_svg) = app.pad_svg {
        for def in &pad_svg.zones {
            let screen_verts = pad_svg.def_screen_verts(def, &pad);
            let centroid = pad_svg.def_screen_centroid(def, &pad);

            let is_active = active_zones.contains(&def.zone);
            let is_feedback = feedback_zones.contains(&def.zone);

            let (fill_color, stroke_color) = if is_active {
                (
                    Color::from_rgba(74, 125, 170, 180),
                    Color::from_rgba(120, 170, 220, 255),
                )
            } else if is_feedback {
                (
                    Color::from_rgba(230, 149, 48, 160),
                    Color::from_rgba(240, 180, 80, 255),
                )
            } else {
                (
                    Color::from_rgba(50, 50, 50, 255),
                    Color::from_rgba(70, 70, 70, 255),
                )
            };

            pad_svg::draw_polygon_fill(&screen_verts, fill_color);
            pad_svg::draw_polygon_lines(&screen_verts, 2.0 * scale, stroke_color);

            let text_color = if is_active || is_feedback {
                WHITE
            } else {
                Color::from_rgba(160, 160, 160, 255)
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
        // 圆弧连接 8 个 tap 圆点
        let arc_steps = 8;
        for i in 0..8 {
            let a0 = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + i as f32 * std::f32::consts::TAU / 8.0;
            let a1 = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + (i + 1) as f32 * std::f32::consts::TAU / 8.0;
            for j in 0..arc_steps {
                let t0 = j as f32 / arc_steps as f32;
                let t1 = (j + 1) as f32 / arc_steps as f32;
                let ang0 = a0 + (a1 - a0) * t0;
                let ang1 = a0 + (a1 - a0) * t1;
                draw_line(
                    spawn_cx.x + ang0.cos() * dot_r,
                    spawn_cx.y + ang0.sin() * dot_r,
                    spawn_cx.x + ang1.cos() * dot_r,
                    spawn_cx.y + ang1.sin() * dot_r,
                    2.0 * scale,
                    Color::from_rgba(255, 255, 255, 120),
                );
            }
        }
        for dot in &a_dots {
            draw_circle(dot.x, dot.y, 5.0 * scale, Color::from_rgba(255, 255, 255, 220));
        }
    }

    let current_t = match app.mode {
        Mode::Playing | Mode::Recording => app.song_time(),
        Mode::Idle => app.timeline_view_time,
    };
    let speed_scale = app.play_speed.max(0.1);

    // ── Trajectory edit overlay ──
    // 编辑时显示 slide 路径的折线和路点圆点
    if let (Some(i), Some(svg)) = (app.editing_slide_path, app.pad_svg.as_ref()) {
        if let Some(note) = app.chart.notes.get(i) {
            if matches!(note.note_type, NoteType::Slide) && !note.slide.is_empty() {
                let spawn_cx = svg.pad_visual_center(&pad).unwrap_or(vec2(pad.cx, pad.cy));
                // 构建路径
                let mut path: Vec<Vec2> = Vec::new();
                if let Some(c) = svg.zone_screen_centroid(PadZone::from(note.lane), &pad) {
                    path.push(c);
                }
                let mut curr_note = note.clone();
                for sl in &note.slide {
                    for seg in &sl.segments {
                        match seg.shape {
                            SlideShape::Q => slide_shape_q(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::QQ => slide_shape_qq(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::P => slide_shape_p(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::PP => slide_shape_pp(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Left => slide_shape_left(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Right => slide_shape_right(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Caret => slide_shape_caret(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Z => slide_shape_z(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::S => slide_shape_s(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                            SlideShape::Wifi => {
                                // Build three separate Wifi lines for editor preview
                                // Calculate start position
                                let start_pos = {
                                    let idx = (note.lane - 1) as f32;
                                    let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                    let target_r = outer_r + TAP_TARGET_OFFSET;
                                    vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                };

                                // Calculate target positions (1-8环形排列)
                                let lane_i = note.lane as i32;
                                let targets = vec![
                                    {
                                        let z = ((lane_i + 3 - 1).rem_euclid(8) + 1) as u8;
                                        let idx = (z - 1) as f32;
                                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                        let target_r = outer_r + TAP_TARGET_OFFSET;
                                        vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                    },
                                    {
                                        let z = ((lane_i + 4 - 1).rem_euclid(8) + 1) as u8;
                                        let idx = (z - 1) as f32;
                                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                        let target_r = outer_r + TAP_TARGET_OFFSET;
                                        vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                    },
                                    {
                                        let z = ((lane_i + 5 - 1).rem_euclid(8) + 1) as u8;
                                        let idx = (z - 1) as f32;
                                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                                        let target_r = outer_r + TAP_TARGET_OFFSET;
                                        vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
                                    },
                                ];

                                // Add all path points for editor preview rendering
                                for target in targets {
                                    path.push(start_pos);
                                    path.push(target);
                                }
                            },

                            _ => slide_shape_line(&mut path, &curr_note, seg, outer_r, spawn_cx, &pad, svg, scale),
                        }
                        if let Some(last_sp) = seg.points.last() {
                            curr_note.lane = last_sp.zone.to_id();
                        }
                    }
                }
                // Polyline
                let line_col = Color::from_rgba(250, 204, 21, 200);
                let line_w = 5. * scale;
                for w in path.windows(2) {
                    draw_line(w[0].x, w[0].y, w[1].x, w[1].y, line_w, line_col);
                }
                // Waypoint dots
                for (k, pt) in path.iter().enumerate() {
                    let is_endpoint = k == 0 || k == path.len() - 1;
                    let r = if is_endpoint { 5.0 * scale } else { 3.5 * scale };
                    draw_circle(pt.x, pt.y, r, Color::from_rgba(255, 220, 50, 200));
                    if is_endpoint {
                        draw_circle_lines(pt.x, pt.y, r, 1.2 * scale, Color::from_rgba(255, 255, 255, 150));
                    }
                }
                let edit_idx = app.editing_slide_idx.unwrap_or(0);
                let banner = format!(
                    "Trajectory edit  #{}:{}  [click=add  Bksp=undo  Esc/E=exit]",
                    i, edit_idx
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

    let bpms = &app.chart.bpms;
    for (_p_idx, note) in app.chart.notes.iter().enumerate() {
        if app.hidden_notes.contains(&note.id) { continue; }
        let zone = sanitize_note_zone(note.note_type, note.lane);
        let ns = note_secs(note, bpms);
        let dt = ns - current_t;
        let dt_scaled = dt / speed_scale;
        let tail_dt = if matches!(note.note_type, NoteType::Hold) {
            hold_tail_time(note, bpms) - current_t
        } else {
            dt
        };
        let tail_dt_scaled = tail_dt / speed_scale;

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
            slide_end_time(note, bpms) - current_t
        } else {
            tail_dt
        };
        let disappear_time = if matches!(note.note_type, NoteType::Touch) { TOUCH_DISAPPEAR_TIME }
            else if matches!(note.note_type, NoteType::Slide) { 0.3 }
            else { 0.18 };
        if slide_tail_dt < -disappear_time || dt > lead_time {
            continue;
        }

        // ── Slide rendering (shared via slide_render module) ──
        if matches!(note.note_type, NoteType::Slide) && !note.slide.is_empty() {
            let spawn_center = app.pad_svg.as_ref()
                .and_then(|svg| svg.pad_visual_center(&pad))
                .unwrap_or(vec2(cx, cy));
            if let Some(ref svg) = app.pad_svg {
                for sl in &note.slide {
                    let slide_dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.3);
                    let fade_in_s = if let Some(t) = FIXED_SLIDE_FADE_IN {
                        t
                    } else {
                        mdur_to_secs(sl.slide_start_delay, note.time, bpms)
                    }
                        .max(0.0).min(slide_dur_s - 0.001).max(0.001);

                    let dbl = note.is_star;
                    let trail_tex = if sl.slide_is_break { app.slide_break_tex.as_ref() }
                        else if note.is_each { app.slide_each_tex.as_ref() }
                        else { app.slide_tex.as_ref() };
                    let star_variant = if note.is_break {
                        if dbl { app.star_double_break_tex.as_ref() } else { app.star_break_tex.as_ref() }
                    } else if note.is_each {
                        if dbl { app.star_double_each_tex.as_ref() } else { app.star_each_tex.as_ref() }
                    } else {
                        if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() }
                    };
                    let star_fb = if dbl { app.star_double_tex.as_ref() } else { app.star_tex.as_ref() };
                    let ex_variant = if note.is_ex {
                        if dbl { app.star_double_ex_tex.as_ref() } else { app.star_ex_tex.as_ref() }
                    } else { None };

                    let tex = slide_render::SlideTextures {
                        trail: trail_tex,
                        star: star_variant.or(star_fb),
                        star_fallback: app.star_tex.as_ref(),
                        star_ex: ex_variant,
                        star_ex_fallback: app.star_ex_tex.as_ref(),
                        wifi: std::array::from_fn(|i| app.wifi_tex[i].as_ref()),
                    };

                    slide_render::draw_slide(
                        note, sl,
                        current_t, ns, slide_dur_s, fade_in_s,
                        &pad, svg, scale, spawn_center, outer_r,
                        &tex, false, speed_scale,
                    );
                }
            }
        }

        // A-zone tap disappears at dt fraction; hold disappears at tail fraction
        if zone <= 8 {
            if matches!(note.note_type, NoteType::Hold) {
                if tail_dt <= (hold_tail_time(note, bpms) - ns) * HOLD_DISAPPEAR_FRAC {
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
            let progress = ((head_travel - dt_scaled) / head_travel).clamp(0.0, 1.0);
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
                let tail_dt = hold_tail_time(note, bpms) - current_t;
                let tail_dt_scaled = tail_dt / speed_scale;
                let tail_fly = if tail_dt_scaled <= HOLD_TAIL_FLY_TIME {
                    (1.0 - tail_dt_scaled / HOLD_TAIL_FLY_TIME).clamp(0.0, 1.0)
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

            // Slide head star is now drawn inside slide_render::draw_slide
            if !matches!(note.note_type, NoteType::Hold | NoteType::Slide) {
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
            let Some(center) = app
                .pad_svg
                .as_ref()
                .and_then(|svg| svg.zone_screen_centroid(PadZone::from(zone), &pad))
            else {
                continue;
            };
            let travel = match note.note_type {
                NoteType::Hold => HOLD_TRAVEL_TIME,
                _ => TOUCH_TRAVEL_TIME,
            };
            let raw = (travel - dt_scaled) / travel;
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
                let hold_progress = (((current_t - ns) / speed_scale) / (hold_tail_time(note, bpms) - ns).max(0.01)).clamp(0.0, 1.0);
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

    // ── Template instance notes on pad (blue-tinted) ──
    if !template::is_in_isolation(app) {
        let tpl_color = Color::from_rgba(180, 200, 255, 220);
        for inst in &app.chart.template_instances {
            if let Some(tpl) = app.chart.templates.iter().find(|t| t.id == inst.template_id) {
                let expanded = template::expand_instance(inst, tpl);
                for note in &expanded {
                    let zone = sanitize_note_zone(note.note_type, note.lane);
                    let ns = note_secs(note, bpms);
                    let dt = ns - current_t;
                    let dt_scaled = dt / speed_scale;
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
                    let disappear_time = if matches!(note.note_type, NoteType::Touch) { TOUCH_DISAPPEAR_TIME }
                        else if matches!(note.note_type, NoteType::Slide) { 0.3 }
                        else { 0.18 };
                    let slide_tail_dt = if matches!(note.note_type, NoteType::Slide) {
                        slide_end_time(note, bpms) - current_t
                    } else {
                        dt
                    };
                    if slide_tail_dt < -disappear_time || dt > lead_time { continue; }

                    // ── Slide rendering ──
                    if matches!(note.note_type, NoteType::Slide) && !note.slide.is_empty() {
                        let spawn_center = app.pad_svg.as_ref()
                            .and_then(|svg| svg.pad_visual_center(&pad))
                            .unwrap_or(vec2(cx, cy));
                        if let Some(ref svg) = app.pad_svg {
                            for sl in &note.slide {
                                let slide_dur_s = mdur_to_secs(sl.slide_duration, note.time, bpms).max(0.3);
                                let fade_in_s = mdur_to_secs(sl.slide_start_delay, note.time, bpms)
                                    .max(0.0).min(slide_dur_s - 0.001).max(0.001);
                                let trail_tex = if sl.slide_is_break { app.slide_break_tex.as_ref() }
                                    else if note.is_each { app.slide_each_tex.as_ref() }
                                    else { app.slide_tex.as_ref() };
                                let star_variant = if note.is_break { app.star_break_tex.as_ref() }
                                    else if note.is_each { app.star_each_tex.as_ref() }
                                    else { app.star_tex.as_ref() };
                                let star_fb = app.star_tex.as_ref();
                                let ex_variant: Option<&Texture2D> = None;
                                let tex = slide_render::SlideTextures {
                                    trail: trail_tex,
                                    star: star_variant.or(star_fb),
                                    star_fallback: app.star_tex.as_ref(),
                                    star_ex: ex_variant,
                                    star_ex_fallback: app.star_ex_tex.as_ref(),
                                    wifi: std::array::from_fn(|i| app.wifi_tex[i].as_ref()),
                                };
                                slide_render::draw_slide(
                                    note, sl,
                                    current_t, ns, slide_dur_s, fade_in_s,
                                    &pad, svg, scale, spawn_center, outer_r,
                                    &tex, false, speed_scale,
                                );
                            }
                        }
                    }

                    if zone <= 8 {
                        let idx = (zone - 1) as f32;
                        let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
                        let dir = vec2(ang.cos(), ang.sin());
                        let head_travel = if matches!(note.note_type, NoteType::Slide) { SLIDE_TRAVEL_TIME } else { TAP_TRAVEL_TIME };
                        let progress = ((head_travel - dt) / head_travel).clamp(0.0, 1.0);
                        let size_scale = if progress < TAP_GROW_FRAC { progress / TAP_GROW_FRAC } else { 1.0 };
                        let fly_progress = if progress < TAP_GROW_FRAC { 0.0 } else { (progress - TAP_GROW_FRAC) / (1.0 - TAP_GROW_FRAC) };
                        let spawn_r = outer_r * TAP_SPAWN_FRAC;
                        let target_r = outer_r + TAP_TARGET_OFFSET;
                        let r = spawn_r + (target_r - spawn_r) * fly_progress;
                        let px = spawn_cx.x + dir.x * r;
                        let py = spawn_cx.y + dir.y * r;

                        // Hold rendering
                        if matches!(note.note_type, NoteType::Hold) {
                            let h_spawn_r = outer_r * HOLD_SPAWN_FRAC;
                            let h_target_r = outer_r + HOLD_TARGET_OFFSET;
                let h_progress = ((HOLD_FLY_TIME - dt_scaled) / HOLD_FLY_TIME).clamp(0.0, 1.0);
                            let h_size_scale = if h_progress < TAP_GROW_FRAC { h_progress / TAP_GROW_FRAC } else { 1.0 };
                            let h_fly_progress = if h_progress < TAP_GROW_FRAC { 0.0 } else { (h_progress - TAP_GROW_FRAC) / (1.0 - TAP_GROW_FRAC) };
                            let full_hold_len = (h_target_r - h_spawn_r) * HOLD_LENGTH_FRAC;
                            let hold_half = (full_hold_len * h_size_scale * 0.5).max(2.0);
                            let head_fly_r = h_spawn_r + (h_target_r - h_spawn_r) * h_fly_progress;
                            let head_r = (head_fly_r + hold_half).min(h_target_r);
                            let tail_dt_h = hold_tail_time(note, bpms) - current_t;
                            let tail_dt_h_scaled = tail_dt_h / speed_scale;
                            let tail_fly = if tail_dt_h_scaled <= HOLD_TAIL_FLY_TIME {
                                (1.0 - tail_dt_h_scaled / HOLD_TAIL_FLY_TIME).clamp(0.0, 1.0)
                            } else { 0.0 };
                            let tail_base = h_spawn_r + (h_target_r - h_spawn_r) * tail_fly;
                            let tail_r = (tail_base - hold_half).max(h_spawn_r * 0.1);
                            let hx = spawn_cx.x + dir.x * head_r;
                            let hy = spawn_cx.y + dir.y * head_r;
                            let tx = spawn_cx.x + dir.x * tail_r;
                            let ty = spawn_cx.y + dir.y * tail_r;
                            let hold_w = HOLD_WIDTH * scale * h_size_scale;
                            if let Some(tex) = app.hold_texture.as_ref() {
                                draw_hold_9slice_segment(tex, vec2(hx, hy), vec2(tx, ty), hold_w.max(1.0), tpl_color);
                            } else {
                                draw_line(hx, hy, tx, ty, HOLD_WIDTH * 0.233 * scale * h_size_scale, Color::from_rgba(100, 140, 200, 200));
                                draw_circle(tx, ty, HOLD_WIDTH * 0.167 * scale * h_size_scale, Color::from_rgba(140, 180, 240, 255));
                            }
                        }

                        // Tap/Touch rendering (not Hold, not Slide)
                        if !matches!(note.note_type, NoteType::Hold | NoteType::Slide) {
                            let ts = TAP_SIZE * scale * size_scale;
                            if let Some(tex) = app.tap_texture.as_ref() {
                                draw_texture_ex(tex, px - ts * 0.5, py - ts * 0.5, tpl_color, DrawTextureParams {
                                    dest_size: Some(vec2(ts, ts)), ..Default::default()
                                });
                            } else {
                                let tr = TAP_SIZE * 0.375 * scale * size_scale;
                                draw_circle(px, py, tr, Color::from_rgba(50, 70, 120, 255));
                                draw_circle_lines(px, py, tr, tr * 0.25, tpl_color);
                            }
                        }
                    } else {
                        // Touch zone rendering
                        let Some(center) = app.pad_svg.as_ref()
                            .and_then(|svg| svg.zone_screen_centroid(PadZone::from(zone), &pad))
                        else { continue; };
                        let travel = match note.note_type {
                            NoteType::Hold => HOLD_TRAVEL_TIME,
                            _ => TOUCH_TRAVEL_TIME,
                        };
                        let raw = (travel - dt_scaled) / travel;
                        let progress = smoothstep(raw.clamp(0.0, 1.0));
                        let alpha_val = if progress < TOUCH_GROW_FRAC {
                            (progress / TOUCH_GROW_FRAC * 220.0) as u8
                        } else { 220 };
                        let move_progress = if progress < TOUCH_GROW_FRAC { 0.0 }
                        else { (progress - TOUCH_GROW_FRAC) / (1.0 - TOUCH_GROW_FRAC) };
                        let dist = (TOUCH_START_DIST + (TOUCH_END_DIST - TOUCH_START_DIST) * move_progress) * TOUCH_SCALE * scale;
                        let ts = TOUCH_CROSS_SIZE * TOUCH_SCALE * scale;

                        if !matches!(note.note_type, NoteType::Hold) {
                            if let Some(tex) = app.touch_tri_tex.as_ref() {
                                let ratio = tex.width() / tex.height();
                                let tw = ts; let th = ts / ratio;
                                let c = Color::from_rgba(180, 200, 255, alpha_val);
                                draw_texture_ex(tex, center.x - tw*0.5, center.y + dist - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), ..Default::default() });
                                draw_texture_ex(tex, center.x - tw*0.5, center.y - dist - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: std::f32::consts::PI, ..Default::default() });
                                draw_texture_ex(tex, center.x - dist - tw*0.5, center.y - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: std::f32::consts::FRAC_PI_2, ..Default::default() });
                                draw_texture_ex(tex, center.x + dist - tw*0.5, center.y - th*0.5, c, DrawTextureParams { dest_size: Some(vec2(tw,th)), rotation: -std::f32::consts::FRAC_PI_2, ..Default::default() });
                            } else {
                                draw_circle(center.x, center.y, 3.0*scale, Color::from_rgba(100,140,200, alpha_val));
                            }
                        }
                        if !matches!(note.note_type, NoteType::Hold) {
                            if let Some(tex) = app.touch_point_tex.as_ref() {
                                let ps = ts * 0.4;
                                draw_texture_ex(tex, center.x - ps*0.5, center.y - ps*0.5,
                                    Color::from_rgba(180, 200, 255, alpha_val),
                                    DrawTextureParams { dest_size: Some(vec2(ps, ps)), ..Default::default() });
                            }
                        }
                    }
                }
            }
        }
    }


    // draw_text(
    //     "Pad zones: A1~A8(Outer) + B1~B8(Inner) + C1(Center) + D1~8(Left) + E1~8(Right)",
    //     rect.x + 12.0 * scale,
    //     rect.y + rect.h - 30.0 * scale,
    //     18.0 * scale,
    //     Color::from_rgba(165, 180, 252, 255),
    // );
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
