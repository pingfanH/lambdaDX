//! LambdaDX editor: egui layout and text with the shared live pad renderer.

#![allow(dead_code, unused_variables, unused_imports)]

#[path = "app/mod.rs"]
mod app;
#[path = "core/mod.rs"]
mod core;
#[path = "../editor/egui/mod.rs"]
mod editor_egui;
#[path = "player/mod.rs"]
mod player;

use macroquad::Window;
use macroquad::prelude::*;

use app::pad_svg;
use editor_egui::{DESIGN_H, DESIGN_W, State as EguiState};
use player::state::PadPreviewState;

fn editor_window_conf() -> Conf {
    let mut conf = app::window_conf();
    conf.window_title = "LambdaDX Editor".to_string();
    conf.window_resizable = true;
    conf
}

fn main() {
    Window::from_config(editor_window_conf(), run());
}

async fn run() {
    set_pc_assets_folder(&app::platform::asset_dir().to_string_lossy());
    let mut app = PadPreviewState::new(app::chart::fallback_chart(), None, None);
    app.params = app::params::load();
    app::params::set(app.params.clone());
    app.note_speed = app.params.note_speed_default;
    app.touch_speed = app.params.touch_speed_default;
    app.slide_fade_in = app.params.slide_fade_in;
    if let Ok(def) = pad_svg::PadSvgDef::from_svg_str(include_str!("../assets/pad.svg")) {
        app.pad_svg = Some(def);
    }
    player::render::load_note_textures(&mut app).await;
    if let Ok(m) = app::ui::load_mask_material() {
        app.mask_material = Some(m);
    }
    if let Ok(m) = app::ui::load_break_shine_material() {
        app.break_shine_material = Some(m);
    }

    let mut ui = EguiState::new();
    let shot_path = std::env::var("MAI2_EDITOR_SHOT").ok();
    let shot_at = std::env::var("MAI2_EDITOR_SHOT_AT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.6);
    let start_time = get_time();

    loop {
        app::anim::tick();
        let events = player::input::collect_pointer_events();
        let pointer = player::input::pointer::ui_pointer(&events);
        app::egui_bridge::pointer(pointer.pos, pointer.down, pointer.pressed, pointer.released);
        let mut pixels_per_point = 1.0;
        app::egui_bridge::ui(|ctx| {
            ui.draw(ctx);
            ui.finish_layout(ctx);
            pixels_per_point = ctx.pixels_per_point();
        });

        // egui owns the window coordinate system. The pad uses the same screen
        // coordinates, so no fixed 1570x856 camera or letterbox is involved.
        app::viewport::set(0.0, 0.0, 1.0);
        let mut pad = ui.layout.pad;
        pad.cx *= pixels_per_point;
        pad.cy *= pixels_per_point;
        pad.outer_r *= pixels_per_point;
        pad.outer_r /= app::params::pad_circle_scale().max(1.0);
        let pad_events: Vec<_> = {
            events
                .into_iter()
                .filter(|e| {
                    let p = e.position;
                    (p.x - pad.cx).hypot(p.y - pad.cy) <= pad.outer_r
                        || app.active_pointer_zones.contains_key(&e.id)
                })
                .collect()
        };
        let scale = (pad.outer_r / (DESIGN_H * 0.42)).max(0.1);
        app.ui_scale_override = Some(scale);
        app::params::set_pad_scale(scale * app::params::pad_zoom());
        player::input::handle_touch_controls(&mut app, pad, &pad_events);
        app.tick_feedback();

        clear_background(Color::from_rgba(29, 29, 30, 255));
        let preview = ui.layout.preview;
        draw_rectangle(
            0.0,
            0.0,
            ui.sidebar_width * pixels_per_point,
            screen_height(),
            Color::from_rgba(34, 34, 34, 255),
        );
        let left = (preview.left() * pixels_per_point)
            .floor()
            .clamp(0.0, screen_width()) as i32;
        let top = (preview.top() * pixels_per_point)
            .floor()
            .clamp(0.0, screen_height()) as i32;
        let right = (preview.right() * pixels_per_point)
            .ceil()
            .clamp(0.0, screen_width()) as i32;
        let bottom = (preview.bottom() * pixels_per_point)
            .ceil()
            .clamp(0.0, screen_height()) as i32;
        unsafe { get_internal_gl() }.quad_gl.scissor(Some((
            left,
            top,
            (right - left).max(0),
            (bottom - top).max(0),
        )));
        player::render::pad::draw_pad_sim(&app, pad.cx, pad.cy, pad.outer_r, scale);
        unsafe { get_internal_gl() }.quad_gl.scissor(None);
        app::egui_bridge::draw();

        if let Some(path) = &shot_path {
            if get_time() - start_time > shot_at {
                get_screen_data().export_png(path);
                println!("editor: wrote {path}");
                std::process::exit(0);
            }
        }
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn egui_editor_uses_responsive_design_constants() {
        assert_eq!(DESIGN_W, 1570.0);
        assert_eq!(DESIGN_H, 856.0);
        assert!(editor_egui::SIDEBAR_MIN < editor_egui::SIDEBAR_DEFAULT);
    }
}
