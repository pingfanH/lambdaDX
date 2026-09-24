//! egui panel for live-tuning [`Params`] (note sizes, slide trail, touch).
//!
//! Toggle with `F1`. Edits apply immediately (they are pushed into the global
//! `app::params` every frame, so the next frame renders the new values).
//! **Save** writes pretty JSON to the writable override path
//! (`output/note_params.json`), which is loaded in preference to the bundled
//! `assets/note_params.json` on the next launch.

use egui_macroquad::egui;

use crate::app::params::{self, Params};
use crate::player::state::PadPreviewState;

/// Register a CJK font with egui once, so Chinese labels render instead of
/// tofu. egui's default fonts are ASCII-only.
fn ensure_egui_font(ctx: &egui::Context) {
    use std::cell::Cell;
    thread_local! {
        static SET: Cell<bool> = const { Cell::new(false) };
    }
    SET.with(|set| {
        if set.get() {
            return;
        }
        set.set(true);
        let Some(bytes) = crate::player::font::bytes() else {
            return;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "cjk".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "cjk".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk".to_owned());
        ctx.set_fonts(fonts);
    });
}

/// Slider + `−` / `+` buttons for one parameter. The slider text is the label;
/// the buttons nudge by `step` (clamped to `range`).
fn param(
    ui: &mut egui::Ui,
    v: &mut f32,
    step: f32,
    range: std::ops::RangeInclusive<f32>,
    name: &str,
) {
    let (lo, hi) = (*range.start(), *range.end());
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(v, range).text(name));
        if ui.small_button("−").clicked() {
            *v = (*v - step).clamp(lo, hi);
        }
        if ui.small_button("+").clicked() {
            *v = (*v + step).clamp(lo, hi);
        }
    });
}

/// Draw the panel if enabled.
pub fn draw(ctx: &egui::Context, app: &mut PadPreviewState) {
    if !app.show_params {
        return;
    }

    // Bigger panel text (idempotent — sets, does not multiply).
    ctx.set_pixels_per_point(1.3);
    ensure_egui_font(ctx);

    let mut p = app.params.clone();
    let mut open = app.show_params;
    let mut message: Option<String> = None;

    egui::Window::new("Params  (F1)")
        .open(&mut open)
        .resizable(true)
        .default_width(380.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(520.0)
                .show(ui, |ui| {
                    ui.heading("Note sizes");
                    param(ui, &mut p.tap_size, 2.0, 8.0..=200.0, "tap_size");
                    param(ui, &mut p.hold_width, 2.0, 8.0..=200.0, "hold_width");
                    param(ui, &mut p.star_size, 2.0, 8.0..=200.0, "star_size");
                    param(ui, &mut p.tap_target_offset, 1.0, -40.0..=80.0, "tap_target_offset");
                    param(ui, &mut p.tap_ring_offset, 1.0, -20.0..=80.0, "tap_ring_offset");

                    ui.separator();
                    ui.heading("Slide");
                    param(ui, &mut p.slide_tile_spacing, 1.0, 5.0..=80.0, "tile_spacing");
                    param(ui, &mut p.slide_tile_scale, 0.02, 0.1..=2.0, "tile_scale");
                    param(ui, &mut p.slide_tile_size, 2.0, 8.0..=200.0, "tile_size");
                    param(ui, &mut p.slide_trail_alpha, 5.0, 0.0..=255.0, "trail_alpha");
                    param(ui, &mut p.slide_fade_in, 0.02, 0.0..=1.0, "fade_in");
                    param(ui, &mut p.slide_join_fillet_frac, 0.05, 0.0..=2.0, "join_fillet_frac");
                    param(ui, &mut p.slide_join_fillet_min, 2.0, 0.0..=200.0, "join_fillet_min");
                    param(ui, &mut p.slide_join_fillet_max, 5.0, 0.0..=800.0, "join_fillet_max");
                    // 连接弧的影响程度：对圆(弧) / 对直线。
                    param(ui, &mut p.slide_join_arc_influence, 0.05, 0.0..=2.0, "join_arc_influence 连接弧影响(圆)");
                    param(ui, &mut p.slide_join_line_influence, 0.05, 0.0..=2.0, "join_line_influence 连接弧影响(直线)");
                    param(ui, &mut p.slide_head_gap, 1.0, 0.0..=120.0, "head_gap");
                    param(ui, &mut p.slide_tail_gap, 1.0, 0.0..=120.0, "tail_gap");
                    param(ui, &mut p.star_spawn_scale_gain, 0.05, 0.0..=2.0, "spawn_scale_gain");
                    param(ui, &mut p.star_spawn_alpha_start, 0.05, 0.0..=1.0, "spawn_alpha_start");
                    ui.checkbox(&mut p.note_earlier_on_top, "note: earlier on top");
                    ui.checkbox(&mut p.slide_tile_reverse, "slide: tiles reverse");
                    ui.checkbox(&mut p.slide_sub_reverse, "slide: sub-slides reverse");

                    ui.separator();
                    ui.heading("Touch / touch-hold");
                    param(ui, &mut p.touch_cross_size, 2.0, 8.0..=260.0, "touch_cross_size");
                    param(ui, &mut p.touch_start_dist, 1.0, 0.0..=140.0, "touch_start_dist");
                    param(ui, &mut p.touch_end_dist, 1.0, 0.0..=140.0, "touch_end_dist");
                    param(ui, &mut p.touch_scale, 0.05, 0.1..=3.0, "touch_scale");
                    param(ui, &mut p.touch_grow_frac, 0.01, 0.0..=0.9, "touch_grow_frac");
                    param(ui, &mut p.touch_move_ramp, 0.05, 0.0..=0.9, "touch_move_ramp 渐进");
                    // 基础总时长倍率 + 出生(渐显)占比。
                    param(ui, &mut p.touch_duration_scale, 0.05, 0.2..=3.0, "touch_duration_scale 基础总时长");
                    param(ui, &mut p.touch_spawn_frac, 0.01, 0.0..=0.9, "touch_spawn_frac 出生时长");
                    param(ui, &mut p.touchhold_cross_base, 2.0, 8.0..=320.0, "touchhold_cross");
                    param(ui, &mut p.touchhold_border_base, 4.0, 8.0..=420.0, "touchhold_border");
                    param(ui, &mut p.touchhold_start_dist, 1.0, 0.0..=140.0, "touchhold_start");
                    param(ui, &mut p.touchhold_end_dist, 1.0, 0.0..=140.0, "touchhold_end");
                    param(ui, &mut p.touchhold_scale, 0.05, 0.1..=3.0, "touchhold_scale");
                    param(ui, &mut p.touchhold_rot_offset, 0.05, -3.2..=3.2, "touchhold_rot");

                    ui.separator();
                    ui.heading("Gameplay / playfield");
                    // 整体 pad 缩放：半径和其上的所有元素一起缩放。
                    param(ui, &mut p.pad_zoom, 0.05, 0.3..=2.0, "pad_zoom 整体缩放");
                    // 感应区整体缩放：独立于 pad 半径，只缩放触摸区。
                    param(ui, &mut p.pad_zone_scale, 0.02, 0.5..=1.5, "pad_zone_scale 感应区缩放");
                    // 出生位置 = where notes lock, as a fraction of the judge radius.
                    param(ui, &mut p.note_spawn_frac, 0.01, 0.05..=1.0, "note_spawn_frac 出生位置");
                    // tap 出生(飞行前的缩放)时间，0 = 随流速。
                    param(ui, &mut p.tap_spawn_time, 0.02, 0.0..=1.5, "tap_spawn_time tap出生缩放(s,0=随流速)");
                    // note 逐渐加快：0 = 匀速，1 = 先慢后快。
                    param(ui, &mut p.note_accel, 0.02, 0.0..=1.0, "note_accel note逐渐加快");
                    // tap 辅助线：贴图放 assets/Skins/classic/tap_guide.png。
                    ui.checkbox(&mut p.tap_guide, "tap_guide tap辅助线(贴图 tap_guide.png)");
                    param(ui, &mut p.tap_guide_size, 0.05, 0.1..=4.0, "tap_guide_size 大小");
                    param(ui, &mut p.tap_guide_off_x, 1.0, -200.0..=200.0, "tap_guide_off_x 偏移(侧向)");
                    param(ui, &mut p.tap_guide_off_y, 1.0, -200.0..=200.0, "tap_guide_off_y 偏移(沿飞行,+=外)");
                    param(ui, &mut p.tap_guide_anchor, 0.05, 0.0..=1.0, "tap_guide_anchor 锚点(0顶/0.5中/1底)");
                    param(ui, &mut p.tap_guide_grow, 0.05, 0.0..=3.0, "tap_guide_grow 位移缩放");
                    param(ui, &mut p.tap_guide_alpha, 5.0, 0.0..=255.0, "tap_guide_alpha 透明度");
                    param(ui, &mut p.tap_guide_rot, 0.02, -3.2..=3.2, "tap_guide_rot 角度偏移");
                    // 判定位置 = judge-ring offset from the sensor radius.
                    param(ui, &mut p.tap_target_offset, 1.0, -40.0..=80.0, "tap_target_offset 判定位置");
                    // 判定区透明度 (white on-hit ring), 背景透明度 (pad panel).
                    param(ui, &mut p.judge_ring_alpha, 5.0, 0.0..=255.0, "judge_ring_alpha 判定区透明度");
                    param(ui, &mut p.pad_bg_alpha, 5.0, 0.0..=255.0, "pad_bg_alpha 背景透明度");
                    // pad大小 = outer circle radius multiple; occluding bg opacity.
                    param(ui, &mut p.pad_circle_scale, 0.01, 1.0..=2.0, "pad_circle_scale pad大小");
                    // bg大小 = disc + cover radius multiple (defaults to the mask).
                    param(ui, &mut p.pad_bg_scale, 0.01, 1.0..=2.0, "pad_bg_scale bg大小");
                    param(ui, &mut p.pad_outside_alpha, 5.0, 0.0..=255.0, "pad_outside_alpha 圈外遮盖");
                    // 隐藏 notes / 感应区（配合视频背景只剩视频）。
                    ui.checkbox(&mut p.hide_notes, "hide_notes 隐藏notes");
                    ui.checkbox(&mut p.hide_zones, "hide_zones 隐藏感应区");
                    // 判定点：黑点(最上层) + 相对贴图的偏移(tap/hold/hold尾)。
                    ui.checkbox(&mut p.judge_dot, "judge_dot 判定黑点(最上层)");
                    param(ui, &mut p.judge_dot_size, 0.5, 0.0..=20.0, "judge_dot_size 黑点大小");                    param(ui, &mut p.judge_off_tap, 1.0, -300.0..=300.0, "judge_off_tap tap判定偏移");
                    param(ui, &mut p.judge_off_hold, 1.0, -300.0..=300.0, "judge_off_hold hold判定偏移");
                    param(ui, &mut p.judge_off_hold_end, 1.0, -300.0..=300.0, "judge_off_hold_end hold尾判定偏移");
                    ui.checkbox(&mut p.judge_sfx, "judge_sfx 判定音效(tap/slide/hold/break)");

            ui.separator();
            // 默认流速 / 出生速度：立即生效，Save 后持久保存。
            ui.heading("Speed 流速 (默认值 · 持久保存)");
            param(ui, &mut p.note_speed_default, 0.5, 1.0..=20.0, "note_speed_default 默认流速");
            param(ui, &mut p.touch_speed_default, 0.5, 1.0..=20.0, "touch_speed_default 出生速度");
            ui.label(format!(
                "live: note {:.2} / touch {:.2}  (Save 持久保存)",
                app.note_speed, app.touch_speed
            ));

            ui.separator();
            ui.heading("Autoplay 自动打歌");
            let mut ap = app.autoplay;
            if ui.checkbox(&mut ap, "AUTO (快捷键 O)").changed() {
                crate::player::autoplay::set_on(app, ap);
            }

            ui.separator();
            ui.heading("Playback 播放");
            // 播放速度：改动立即生效，并作为默认值保存。
            let mut speed = app.play_speed;
            if ui
                .add(
                    egui::Slider::new(
                        &mut speed,
                        crate::app::types::SPEED_MIN..=crate::app::types::SPEED_MAX,
                    )
                    .text("play_speed 播放速度"),
                )
                .changed()
            {
                app.set_play_speed(speed);
                p.play_speed_default = app.play_speed;
            }
            // 变速模式：整体速度(含note飞行/star旋转) 或 仅播放速度。
            ui.checkbox(
                &mut p.speed_scales_visuals,
                "整体速度(影响飞行/旋转) · 关=仅播放速度",
            );

            ui.separator();
            ui.heading("Background video (bg.mp4)");
            ui.checkbox(&mut p.bg_video, "bg_video 启用视频背景");
            ui.text_edit_singleline(&mut p.bg_video_path);
            // 开始位置 (秒) = 歌曲 0 秒时对应的视频时间。
            param(ui, &mut p.bg_video_start, 0.1, 0.0..=600.0, "bg_video_start 起始秒(歌曲0处)");
            // xy 位置 (设计像素，随 UI 缩放)
            param(ui, &mut p.bg_video_x, 1.0, -2000.0..=2000.0, "bg_video_x 位置X");
            param(ui, &mut p.bg_video_y, 1.0, -2000.0..=2000.0, "bg_video_y 位置Y");
            // 缩放 / 透明度 / 帧率 / 提取高度
            param(ui, &mut p.bg_video_scale, 0.02, 0.1..=4.0, "bg_video_scale 缩放");
            param(ui, &mut p.bg_video_alpha, 5.0, 0.0..=255.0, "bg_video_alpha 透明度");
            param(ui, &mut p.bg_video_fps, 1.0, 0.0..=120.0, "bg_video_fps 帧率(0=原生)");
            param(ui, &mut p.bg_video_height, 10.0, 120.0..=2160.0, "bg_video_height 高度");
            ui.checkbox(&mut p.bg_video_loop, "bg_video_loop 循环");
                });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    match params::save(&p) {
                        Ok(path) => message = Some(format!("saved → {}", path.display())),
                        Err(e) => message = Some(format!("save failed: {e}")),
                    }
                }
                if ui.button("Reload").clicked() {
                    p = params::load();
                    message = Some("reloaded".to_string());
                }
                if ui.button("Reset").clicked() {
                    p = Params::default();
                    message = Some("reset to defaults".to_string());
                }
            });
            ui.label(format!(
                "override: {}",
                crate::app::platform::output_dir()
                    .map(|d| d.join(params::PARAMS_OVERRIDE).display().to_string())
                    .unwrap_or_default()
            ));
            if let Some(m) = &message {
                ui.label(m);
            }
        });

    app.show_params = open;

    // Apply immediately: mirror into the app and the global (render reads it).
    app.params = p.clone();
    params::set(p);

    // The "Speed" sliders edit the persisted defaults; mirror them onto the
    // live runtime speeds so the change is visible at once.
    app.note_speed = app.params.note_speed_default;
    app.touch_speed = app.params.touch_speed_default;
}
