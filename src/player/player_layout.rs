use crate::state::PlayerState;
use crate::ui::draw_pad_panel;
use lambda_dx::state::AppState;
use lambda_dx::types::{Layout, PadGeom, RectF, UiButton};
use macroquad::prelude::{screen_height, screen_width};

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
    let margin = if app.mobile_ui { 12.0 } else { 20.0 } * scale;
    let header_h = 76.0 * scale;

    let header = RectF {
        x: margin,
        y: margin,
        w: sw - margin * 2.0,
        h: header_h,
    };

    let pad = RectF {
        x: margin,
        y: header_h,
        w: sw - margin * 2.0,
        h: sh - header_h - margin,
    };
    Layout {
        header,
        timeline: None,
        pad,
    }
}
pub fn draw_layout(app: &PlayerState, layout: Layout, pad: PadGeom, _buttons: &[UiButton]) {
    draw_pad_panel(app, layout.pad, pad);
}
