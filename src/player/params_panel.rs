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
                    param(ui, &mut p.slide_join_fillet_frac, 0.05, 0.0..=1.5, "join_fillet_frac");
                    param(ui, &mut p.slide_join_fillet_min, 2.0, 0.0..=80.0, "join_fillet_min");
                    param(ui, &mut p.slide_join_fillet_max, 2.0, 0.0..=160.0, "join_fillet_max");
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
                    param(ui, &mut p.touch_stall_frac, 0.01, 0.0..=0.9, "touch_stall_frac");
                    param(ui, &mut p.touch_move_ramp, 0.05, 0.0..=0.9, "touch_move_ramp");
                    param(ui, &mut p.touchhold_cross_base, 2.0, 8.0..=320.0, "touchhold_cross");
                    param(ui, &mut p.touchhold_border_base, 4.0, 8.0..=420.0, "touchhold_border");
                    param(ui, &mut p.touchhold_start_dist, 1.0, 0.0..=140.0, "touchhold_start");
                    param(ui, &mut p.touchhold_end_dist, 1.0, 0.0..=140.0, "touchhold_end");
                    param(ui, &mut p.touchhold_scale, 0.05, 0.1..=3.0, "touchhold_scale");
                    param(ui, &mut p.touchhold_rot_offset, 0.05, -3.2..=3.2, "touchhold_rot");
                });

            ui.separator();
            ui.heading("Speed (live)");
            param(ui, &mut app.note_speed, 0.5, 1.0..=20.0, "note_speed");
            param(ui, &mut app.touch_speed, 0.5, 1.0..=20.0, "touch_speed");

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
}
