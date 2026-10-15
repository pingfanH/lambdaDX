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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_f1_preserves_host_dpi_scale() {
        for dpi in [1.0, 2.0] {
            let ctx = egui::Context::default();
            let mut app = crate::player::render::scale::tests::app();
            app.show_params = true;
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 760.0),
            ));
            raw.viewports
                .get_mut(&raw.viewport_id)
                .unwrap()
                .native_pixels_per_point = Some(dpi);
            let _ = ctx.run(raw.clone(), |_| {});
            let _ = ctx.run(raw.clone(), |ctx| draw(ctx, &mut app));
            let output = ctx.run(raw, |ctx| draw(ctx, &mut app));
            assert_eq!(
                output.pixels_per_point, dpi,
                "F1 changed the host scale on DPI {dpi}"
            );
        }
    }
}

/// Register a CJK font with egui once, so Chinese labels render instead of
/// tofu. egui's default fonts are ASCII-only.
pub fn ensure_egui_font(ctx: &egui::Context) {
    let id = egui::Id::new("cjk-font-installed");
    if ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
        return;
    }
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
    ctx.data_mut(|d| d.insert_temp(id, true));
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
    ui.label(egui::RichText::new(name).small());
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(v, range));
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
    draw_impl(ctx, app, false, |_| {});
}

/// Draw the panel inside the editor, preserving the host's logical UI size.
pub fn draw_embedded(ctx: &egui::Context, app: &mut PadPreviewState) {
    draw_embedded_with(ctx, app, |_| {});
}

pub fn draw_embedded_with(
    ctx: &egui::Context,
    app: &mut PadPreviewState,
    host: impl FnMut(&mut egui::Ui),
) {
    draw_impl(ctx, app, true, host);
}

pub fn judgment_option(ui: &mut egui::Ui, p: &mut Params) {
    ui.checkbox(&mut p.show_judge_windows, "显示判定区间")
        .on_hover_text("Timeline / Pad 共用核心判定时间。Slide 仅显示终点附近的 Great / Perfect 窗口，且须先完成前面的感应区。");
    if p.show_judge_windows {
        ui.horizontal_wrapped(|ui| {
            for (i, label) in ["Good", "Great", "Perfect", "Critical Perfect"].iter().enumerate() {
                let [r, g, b] = crate::player::judge_windows::COLORS[i];
                ui.colored_label(egui::Color32::from_rgb(r, g, b), *label);
            }
            let [r, g, b] = crate::player::judge_windows::HOLD_COLOR;
            ui.colored_label(egui::Color32::from_rgb(r, g, b), "Hold Perfect")
                .on_hover_text("绿线表示准时按下且持续按住时，保留 Perfect 所需的有效距离；中途松手或头部判定会改变结果。经典 Hold 尾部窗口只保留头部等级，不提升等级。");
        });
    }
}

fn draw_impl(
    ctx: &egui::Context,
    app: &mut PadPreviewState,
    embedded: bool,
    mut host: impl FnMut(&mut egui::Ui),
) {
    if !app.show_params {
        return;
    }

    ensure_egui_font(ctx);

    let mut p = app.params.clone();
    let mut open = app.show_params;
    let message_id = egui::Id::new("params-message");
    let mut message = ctx.data(|d| d.get_temp::<String>(message_id));
    let original_speed = p.play_speed_default;

    egui::Window::new("设置")
        .id(egui::Id::new("params-window"))
        .order(egui::Order::Foreground)
        .open(&mut open)
        .resizable(true)
        .default_width(440.0)
        .max_width((ctx.screen_rect().width() - 32.0).max(240.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height((ctx.screen_rect().height() - 180.0).clamp(120.0, 600.0))
                .show(ui, |ui| {
                    egui::CollapsingHeader::new("视图与判定")
                        .default_open(true)
                        .show(ui, |ui| {
                            judgment_option(ui, &mut p);
                            ui.checkbox(&mut p.hide_notes, "隐藏 Notes");
                            ui.checkbox(&mut p.hide_zones, "隐藏感应区");
                            ui.checkbox(&mut p.hide_slide_just, "隐藏 Slide 判定贴图");
                            ui.checkbox(&mut p.note_earlier_on_top, "较早的 Note 在上层");
                        });
                    if embedded {
                        egui::CollapsingHeader::new("编辑器")
                            .default_open(true)
                            .show(ui, |ui| host(ui));
                    }
                    ui.collapsing("Note 尺寸", |ui| {
                        param(ui, &mut p.tap_size, 2.0, 8.0..=200.0, "tap_size");
                        param(ui, &mut p.hold_width, 2.0, 8.0..=200.0, "hold_width");
                        param(ui, &mut p.star_size, 2.0, 8.0..=200.0, "star_size");
                        param(
                            ui,
                            &mut p.tap_target_offset,
                            1.0,
                            -40.0..=80.0,
                            "tap_target_offset",
                        );
                        param(
                            ui,
                            &mut p.tap_ring_offset,
                            1.0,
                            -20.0..=80.0,
                            "tap_ring_offset",
                        );
                        // 编辑器时间轴 note 大小倍率：统一放大/缩小时间轴上的所有 note。
                        param(
                            ui,
                            &mut p.editor_note_scale,
                            0.02,
                            0.2..=3.0,
                            "editor_note_scale 时间轴note大小",
                        );
                    });
                    ui.collapsing("Slide 轨迹与星星", |ui| {
                        param(
                            ui,
                            &mut p.slide_tile_spacing,
                            1.0,
                            5.0..=80.0,
                            "tile_spacing",
                        );
                        param(ui, &mut p.slide_tile_scale, 0.02, 0.1..=2.0, "tile_scale");
                        param(ui, &mut p.slide_tile_size, 2.0, 8.0..=200.0, "tile_size");
                        param(
                            ui,
                            &mut p.slide_trail_alpha,
                            5.0,
                            0.0..=255.0,
                            "trail_alpha",
                        );
                        param(ui, &mut p.slide_fade_in, 0.02, 0.0..=1.0, "fade_in");
                        param(
                            ui,
                            &mut p.slide_join_fillet_frac,
                            0.05,
                            0.0..=2.0,
                            "join_fillet_frac",
                        );
                        param(
                            ui,
                            &mut p.slide_join_fillet_min,
                            2.0,
                            0.0..=200.0,
                            "join_fillet_min",
                        );
                        param(
                            ui,
                            &mut p.slide_join_fillet_max,
                            5.0,
                            0.0..=800.0,
                            "join_fillet_max",
                        );
                        // 连接弧的影响程度：对圆(弧) / 对直线。
                        param(
                            ui,
                            &mut p.slide_join_arc_influence,
                            0.05,
                            0.0..=2.0,
                            "join_arc_influence 连接弧影响(圆)",
                        );
                        param(
                            ui,
                            &mut p.slide_join_line_influence,
                            0.05,
                            0.0..=2.0,
                            "join_line_influence 连接弧影响(直线)",
                        );
                        param(ui, &mut p.slide_head_gap, 1.0, 0.0..=120.0, "head_gap");
                        param(ui, &mut p.slide_tail_gap, 1.0, 0.0..=120.0, "tail_gap");
                        param(
                            ui,
                            &mut p.star_spawn_scale_gain,
                            0.05,
                            0.0..=2.0,
                            "spawn_scale_gain",
                        );
                        param(
                            ui,
                            &mut p.star_spawn_alpha_start,
                            0.05,
                            0.0..=1.0,
                            "spawn_alpha_start",
                        );
                        // Break 闪烁速度倍率（1 = MajdataPlay 原始 ~0.17 rad/帧）。
                        param(
                            ui,
                            &mut p.break_shine_speed,
                            0.1,
                            0.0..=8.0,
                            "break_shine_speed",
                        );
                        ui.checkbox(&mut p.slide_tile_reverse, "slide: tiles reverse");
                        ui.checkbox(&mut p.slide_sub_reverse, "slide: sub-slides reverse");
                    });
                    ui.collapsing("Slide 判定贴图 (slideok)", |ui| {
                        for (label, adj) in [
                            ("str_l  -", &mut p.slide_just_str_l),
                            ("str_r  -", &mut p.slide_just_str_r),
                            ("curv_l  <", &mut p.slide_just_curv_l),
                            ("curv_r  >", &mut p.slide_just_curv_r),
                            ("wifi_u", &mut p.slide_just_wifi_u),
                            ("wifi_d", &mut p.slide_just_wifi_d),
                        ] {
                            ui.label(label);
                            param(ui, &mut adj.scale, 0.02, 0.05..=3.0, "scale");
                            param(
                                ui,
                                &mut adj.rot,
                                0.02,
                                -std::f32::consts::PI..=std::f32::consts::PI,
                                "rot(rad)",
                            );
                            param(ui, &mut adj.off_x, 1.0, -400.0..=400.0, "off_x");
                            param(ui, &mut adj.off_y, 1.0, -400.0..=400.0, "off_y");
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut adj.flip_x, "flip_x");
                                ui.checkbox(&mut adj.flip_y, "flip_y");
                            });
                        }
                    });
                    ui.collapsing("Touch / TouchHold", |ui| {
                        param(
                            ui,
                            &mut p.touch_cross_size,
                            2.0,
                            8.0..=260.0,
                            "touch_cross_size",
                        );
                        param(
                            ui,
                            &mut p.touch_start_dist,
                            1.0,
                            0.0..=140.0,
                            "touch_start_dist",
                        );
                        param(
                            ui,
                            &mut p.touch_end_dist,
                            1.0,
                            0.0..=140.0,
                            "touch_end_dist",
                        );
                        param(ui, &mut p.touch_scale, 0.05, 0.1..=3.0, "touch_scale");
                        param(
                            ui,
                            &mut p.touch_grow_frac,
                            0.01,
                            0.0..=0.9,
                            "touch_grow_frac",
                        );
                        param(
                            ui,
                            &mut p.touch_move_ramp,
                            0.05,
                            0.0..=0.9,
                            "touch_move_ramp 渐进",
                        );
                        // 基础总时长倍率 + 出生(渐显)占比。
                        param(
                            ui,
                            &mut p.touch_duration_scale,
                            0.05,
                            0.2..=3.0,
                            "touch_duration_scale 基础总时长",
                        );
                        param(
                            ui,
                            &mut p.touch_spawn_frac,
                            0.01,
                            0.0..=0.9,
                            "touch_spawn_frac 出生时长",
                        );
                        param(
                            ui,
                            &mut p.touchhold_cross_base,
                            2.0,
                            8.0..=320.0,
                            "touchhold_cross",
                        );
                        param(
                            ui,
                            &mut p.touchhold_border_base,
                            4.0,
                            8.0..=420.0,
                            "touchhold_border",
                        );
                        param(
                            ui,
                            &mut p.touchhold_start_dist,
                            1.0,
                            0.0..=140.0,
                            "touchhold_start",
                        );
                        param(
                            ui,
                            &mut p.touchhold_end_dist,
                            1.0,
                            0.0..=140.0,
                            "touchhold_end",
                        );
                        param(
                            ui,
                            &mut p.touchhold_scale,
                            0.05,
                            0.1..=3.0,
                            "touchhold_scale",
                        );
                        param(
                            ui,
                            &mut p.touchhold_rot_offset,
                            0.05,
                            -3.2..=3.2,
                            "touchhold_rot",
                        );
                        // hold 贴图偏移 + 出生时间。
                        param(
                            ui,
                            &mut p.hold_spawn_time,
                            0.02,
                            0.0..=1.5,
                            "hold_spawn_time hold出生时间(s,0=随流速)",
                        );
                    });
                    ui.collapsing("Pad 与判定位置", |ui| {
                        // 整体 pad 缩放：半径和其上的所有元素一起缩放。
                        param(ui, &mut p.pad_zoom, 0.05, 0.3..=2.0, "pad_zoom 整体缩放");
                        // 感应区整体缩放：独立于 pad 半径，只缩放触摸区。
                        param(
                            ui,
                            &mut p.pad_zone_scale,
                            0.02,
                            0.5..=1.5,
                            "pad_zone_scale 感应区缩放",
                        );
                        // 感应区范围触发：点不必精准落在感应区内，10px 圆碰到即触发。
                        ui.checkbox(
                            &mut p.sensor_range_enable,
                            "sensor_range 感应区范围触发(圆碰到就触发)",
                        );
                        param(
                            ui,
                            &mut p.sensor_range_px,
                            1.0,
                            0.0..=120.0,
                            "sensor_range_px 范围半径(px)",
                        );
                        // 出生位置 = where notes lock, as a fraction of the judge radius.
                        param(
                            ui,
                            &mut p.note_spawn_frac,
                            0.01,
                            0.05..=1.0,
                            "note_spawn_frac 出生位置",
                        );
                        // tap 出生(飞行前的缩放)时间，0 = 随流速。
                        param(
                            ui,
                            &mut p.tap_spawn_time,
                            0.02,
                            0.0..=1.5,
                            "tap_spawn_time tap出生缩放(s,0=随流速)",
                        );
                        // note 逐渐加快：0 = 匀速，1 = 先慢后快。
                        param(
                            ui,
                            &mut p.note_accel,
                            0.02,
                            0.0..=1.0,
                            "note_accel note逐渐加快",
                        );
                        // tap 辅助线：贴图放 assets/Skins/classic/tap_guide.png。
                        ui.checkbox(&mut p.tap_guide, "tap_guide tap辅助线(贴图 tap_guide.png)");
                        param(
                            ui,
                            &mut p.guide_base_px,
                            1.0,
                            8.0..=240.0,
                            "guide_base_px 辅助线基准(固定)",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_size,
                            0.05,
                            0.1..=4.0,
                            "tap_guide_size 辅助线大小",
                        );
                        param(
                            ui,
                            &mut p.hold_guide_size,
                            0.02,
                            0.1..=4.0,
                            "hold_guide_size hold辅助线大小",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_off_x,
                            1.0,
                            -200.0..=200.0,
                            "tap_guide_off_x 偏移(侧向)",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_off_y,
                            1.0,
                            -200.0..=200.0,
                            "tap_guide_off_y 偏移(沿飞行,+=外)",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_anchor,
                            0.05,
                            0.0..=1.0,
                            "tap_guide_anchor 锚点(0顶/0.5中/1底)",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_grow,
                            0.05,
                            0.0..=3.0,
                            "tap_guide_grow 位移缩放",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_alpha,
                            5.0,
                            0.0..=255.0,
                            "tap_guide_alpha 透明度",
                        );
                        param(
                            ui,
                            &mut p.tap_guide_rot,
                            0.02,
                            -3.2..=3.2,
                            "tap_guide_rot 角度偏移",
                        );
                        // 判定位置 = judge-ring offset from the sensor radius.
                        param(
                            ui,
                            &mut p.tap_target_offset,
                            1.0,
                            -40.0..=80.0,
                            "tap_target_offset 判定位置",
                        );
                        // 判定区透明度 (white on-hit ring), 背景透明度 (pad panel).
                        param(
                            ui,
                            &mut p.judge_ring_alpha,
                            5.0,
                            0.0..=255.0,
                            "judge_ring_alpha 判定区透明度",
                        );
                        param(
                            ui,
                            &mut p.pad_bg_alpha,
                            5.0,
                            0.0..=255.0,
                            "pad_bg_alpha 背景透明度",
                        );
                        // pad大小 = outer circle radius multiple; occluding bg opacity.
                        param(
                            ui,
                            &mut p.pad_circle_scale,
                            0.01,
                            1.0..=2.0,
                            "pad_circle_scale pad大小",
                        );
                        // bg大小 = disc + cover radius multiple (defaults to the mask).
                        param(
                            ui,
                            &mut p.pad_bg_scale,
                            0.01,
                            1.0..=2.0,
                            "pad_bg_scale bg大小",
                        );
                        param(
                            ui,
                            &mut p.pad_outside_alpha,
                            5.0,
                            0.0..=255.0,
                            "pad_outside_alpha 圈外遮盖",
                        );
                        // 隐藏 notes / 感应区（配合视频背景只剩视频）。
                        // 判定点：黑点(最上层) + 相对贴图的偏移(tap/hold/hold尾)。
                        ui.checkbox(&mut p.judge_dot, "judge_dot 判定黑点(最上层)");
                        param(
                            ui,
                            &mut p.judge_dot_size,
                            0.5,
                            0.0..=20.0,
                            "judge_dot_size 黑点大小",
                        );
                        param(
                            ui,
                            &mut p.judge_off_tap,
                            1.0,
                            -300.0..=300.0,
                            "judge_off_tap tap判定偏移",
                        );
                        param(
                            ui,
                            &mut p.judge_off_hold,
                            1.0,
                            -300.0..=300.0,
                            "judge_off_hold hold判定偏移",
                        );
                        param(
                            ui,
                            &mut p.hold_guide_off,
                            1.0,
                            -300.0..=300.0,
                            "hold_guide_off hold辅助线偏移",
                        );
                        param(
                            ui,
                            &mut p.judge_off_hold_end,
                            1.0,
                            -300.0..=300.0,
                            "judge_off_hold_end hold尾判定偏移",
                        );
                        param(
                            ui,
                            &mut p.hold_end_guide_off,
                            1.0,
                            -300.0..=300.0,
                            "hold_end_guide_off hold尾辅助线偏移",
                        );
                    });
                    ui.collapsing("音效与击打特效", |ui| {
                        ui.checkbox(&mut p.answer_sfx, "正解音 (answer.wav)");
                        ui.checkbox(&mut p.hit_sfx, "击打音效");
                        ui.checkbox(&mut p.fx_timeline, "fx_timeline 特效跟随时间轴(暂停即冻结)");
                        ui.checkbox(&mut p.hit_fx, "hit_fx 启用(圆环+火花+闪光)");
                        ui.checkbox(&mut p.hit_fx_anim, "hit_fx_anim 用Flash影片播放(ui项目)");
                        ui.checkbox(
                            &mut p.hit_fx_anim_tint,
                            "hit_fx_anim_tint 按判定等级染色(关=用影片原色)",
                        );
                        ui.horizontal(|ui| {
                            ui.label("clip:");
                            ui.text_edit_singleline(&mut p.hit_fx_anim_clip);
                        });
                        param(ui, &mut p.hit_fx_size, 1.0, 8.0..=200.0, "hit_fx_size 大小");
                        param(
                            ui,
                            &mut p.hit_fx_duration,
                            0.01,
                            0.05..=1.5,
                            "hit_fx_duration 时长(s)",
                        );
                        param(
                            ui,
                            &mut p.hit_fx_alpha,
                            5.0,
                            0.0..=255.0,
                            "hit_fx_alpha 透明度",
                        );
                        param(ui, &mut p.hit_fx_ring, 0.5, 0.5..=20.0, "hit_fx_ring 线宽");
                        param(
                            ui,
                            &mut p.hit_fx_sparks,
                            1.0,
                            0.0..=32.0,
                            "hit_fx_sparks 火花数",
                        );
                        param(
                            ui,
                            &mut p.hit_fx_spark_len,
                            0.1,
                            0.0..=5.0,
                            "hit_fx_spark_len 火花长度",
                        );
                        param(ui, &mut p.hit_fx_grow, 0.05, 0.0..=4.0, "hit_fx_grow 扩散");
                        param(
                            ui,
                            &mut p.hit_fx_flash,
                            0.05,
                            0.0..=3.0,
                            "hit_fx_flash 中心闪光",
                        );
                    });
                    ui.collapsing("流速与播放", |ui| {
                        param(
                            ui,
                            &mut p.note_speed_default,
                            0.5,
                            1.0..=20.0,
                            "note_speed_default 默认流速",
                        );
                        param(
                            ui,
                            &mut p.touch_speed_default,
                            0.5,
                            1.0..=20.0,
                            "touch_speed_default 出生速度",
                        );
                        param(
                            ui,
                            &mut p.note_appear_rate,
                            0.005,
                            0.05..=0.6,
                            "note_appear_rate 出现率",
                        );
                        if !embedded {
                            let mut ap = app.autoplay;
                            if ui.checkbox(&mut ap, "AUTO (快捷键 O)").changed() {
                                crate::player::autoplay::set_on(app, ap);
                            }
                            let mut direct = app.autoplay_direct_core;
                            if ui.checkbox(&mut direct, "直传核心").changed() {
                                crate::player::autoplay::set_direct_core(app, direct);
                            }
                            // 不使用 lnmai-core：旧版 autoplay（星星自己滑，左下角显示 None）。
                            ui.checkbox(&mut p.no_core, "no_core 旧版autoplay(星星自己滑,不判定)");
                        }
                        ui.checkbox(
                            &mut p.autoplay_inner_screen,
                            "Autoplay 内屏模式 (K1-K8 → A1-A8)",
                        );
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
                    });
                    ui.collapsing("背景视频", |ui| {
                        ui.checkbox(&mut p.bg_video, "bg_video 启用视频背景");
                        ui.text_edit_singleline(&mut p.bg_video_path);
                        // 开始位置 (秒) = 歌曲 0 秒时对应的视频时间。
                        param(
                            ui,
                            &mut p.bg_video_start,
                            0.1,
                            0.0..=600.0,
                            "bg_video_start 起始秒(歌曲0处)",
                        );
                        // xy 位置 (设计像素，随 UI 缩放)
                        param(
                            ui,
                            &mut p.bg_video_x,
                            1.0,
                            -2000.0..=2000.0,
                            "bg_video_x 位置X",
                        );
                        param(
                            ui,
                            &mut p.bg_video_y,
                            1.0,
                            -2000.0..=2000.0,
                            "bg_video_y 位置Y",
                        );
                        // 缩放 / 透明度 / 帧率 / 提取高度
                        param(
                            ui,
                            &mut p.bg_video_scale,
                            0.02,
                            0.1..=4.0,
                            "bg_video_scale 缩放",
                        );
                        param(
                            ui,
                            &mut p.bg_video_alpha,
                            5.0,
                            0.0..=255.0,
                            "bg_video_alpha 透明度",
                        );
                        param(
                            ui,
                            &mut p.bg_video_fps,
                            1.0,
                            0.0..=120.0,
                            "bg_video_fps 帧率(0=原生)",
                        );
                        param(
                            ui,
                            &mut p.bg_video_height,
                            10.0,
                            120.0..=2160.0,
                            "bg_video_height 高度",
                        );
                        ui.checkbox(&mut p.bg_video_loop, "bg_video_loop 循环");
                    });
                });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("保存设置").clicked() {
                    match params::save(&p) {
                        Ok(path) => message = Some(format!("saved → {}", path.display())),
                        Err(e) => message = Some(format!("save failed: {e}")),
                    }
                }
                if ui.button("重新载入").clicked() {
                    p = params::load();
                    message = Some("reloaded".to_string());
                }
                if ui.button("恢复默认").clicked() {
                    p = Params::default();
                    message = Some("reset to defaults".to_string());
                }
            });
            if let Some(m) = &message {
                ui.label(m);
            }
        });

    app.show_params = open;
    if let Some(m) = message {
        ctx.data_mut(|d| d.insert_temp(message_id, m));
    }
    if embedded {
        p.no_core = app.params.no_core;
    }
    if p.play_speed_default != original_speed {
        app.set_play_speed(p.play_speed_default);
    }

    // Apply immediately: mirror into the app and the global (render reads it).
    app.params = p.clone();
    params::set(p);

    // The "Speed" sliders edit the persisted defaults; mirror them onto the
    // live runtime speeds so the change is visible at once.
    app.note_speed = app.params.note_speed_default;
    app.touch_speed = app.params.touch_speed_default;
}
