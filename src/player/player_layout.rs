use macroquad::color::Color;
use macroquad::prelude::{draw_text, screen_height, screen_width};
use lambda_dx::state::AppState;
use lambda_dx::types::{Layout, PadGeom, RectF, UiButton};
use crate::state::PlayerState;
use crate::ui::draw_pad_panel;

pub fn ui_scale(app: &PlayerState) -> f32 {
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
pub fn compute_layout(app: &PlayerState) -> Layout {
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
pub fn draw_layout(app: &PlayerState, layout: Layout, pad: PadGeom, _buttons: &[UiButton]) {
    // Header bg (egui toolbar overlays on top)
    // draw_rectangle(layout.header.x, layout.header.y, layout.header.w, layout.header.h,
    //     Color::from_rgba(17, 24, 39, 255));

    // Waveform threshold control
    let s = ui_scale(app);
    draw_text(&format!("Wave threshold: {:.2}  [/] keys", app.waveform_threshold), layout.header.x + 14.0 * s, layout.header.y + 80.0 * s, 16.0 * s, Color::from_rgba(125, 211, 252, 200));

    draw_pad_panel(app, layout.pad, pad);
}