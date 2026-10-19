mod lambdadx_effects;
mod lambdadx_preview_controls;

use std::path::PathBuf;

use lambdadx_effects::{EffectInstance, EffectLibrary, Judgment};
use lambdadx_preview_controls::{PANEL_HEIGHT, PlaybackControls};
use macroquad::Window;
use macroquad::prelude::*;

const BAR_HEIGHT: f32 = 58.0;

fn main() {
    Window::from_config(window_conf(), run());
}

fn window_conf() -> Conf {
    Conf {
        window_title: "LambdaDX Effect Preview".to_string(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

async fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let argument = |name: &str| {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    };
    let effect_name = argument("--effect").unwrap_or_else(|| "FX_GAM_Notes_Tap_00".to_string());
    let fixed_time = argument("--time").map(|value| {
        value
            .parse::<f32>()
            .expect("--time requires seconds")
            .max(0.0)
    });
    let screenshot = argument("--screenshot");
    let judgment = argument("--judgment")
        .map(|value| {
            Judgment::parse(&value)
                .expect("--judgment requires good, great, perfect, critical-perfect, or raw")
        })
        .unwrap_or_default();
    let root = std::env::var_os("LAMBDADX_EFFECTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/extracted/lambdadx_replay_1600_master/effects")
        });
    let library = match EffectLibrary::load(&root).await {
        Ok(library) => library,
        Err(error) => {
            eprintln!("effect preview failed: {error}");
            return;
        }
    };
    if library.is_empty() {
        eprintln!("effect library is empty: {}", root.display());
        return;
    }

    let mut selected = (0..library.len())
        .find(|index| library.effect(*index).unwrap().name == effect_name)
        .unwrap_or_else(|| panic!("unknown effect: {effect_name}"));
    let mut playback = library.new_instance(selected).expect("first effect exists");
    playback.judgment = judgment;
    if let Some(time) = fixed_time {
        playback.elapsed = time;
    }
    let speed = argument("--speed")
        .map(|value| {
            let speed = value
                .parse::<f32>()
                .expect("--speed requires a positive number");
            assert!(
                speed.is_finite() && speed > 0.0,
                "--speed requires a positive number"
            );
            speed.clamp(0.05, 4.0)
        })
        .unwrap_or(1.0);
    let mut controls = PlaybackControls::new(fixed_time.is_some(), speed);
    lambdadx_preview_controls::configure();
    let initial_zoom = argument("--zoom")
        .map(|value| {
            let zoom = value
                .parse::<f32>()
                .expect("--zoom requires a positive number");
            assert!(
                zoom.is_finite() && zoom > 0.0,
                "--zoom requires a positive number"
            );
            zoom.clamp(0.08, 5.0)
        })
        .unwrap_or(0.8);
    let mut zoom = initial_zoom;
    let mut pan = vec2(0.0, 0.0);
    let mut last_mouse = None;
    let mut frames = 0;

    loop {
        let dt = get_frame_time().min(0.05);
        let mouse = vec2(mouse_position().0, mouse_position().1);
        let end = preview_duration(&library, &playback, selected);
        let mut keyboard_captured = false;
        let mut pointer_captured = false;
        egui_macroquad::ui(|ctx| {
            controls.ui(ctx, &mut playback, end);
            keyboard_captured = ctx.wants_keyboard_input();
            pointer_captured = ctx.wants_pointer_input();
        });

        if !keyboard_captured && is_key_pressed(KeyCode::Right) {
            select_effect(&library, &mut selected, &mut playback, 1);
        }
        if !keyboard_captured && is_key_pressed(KeyCode::Left) {
            select_effect(&library, &mut selected, &mut playback, -1);
        }
        if !keyboard_captured && is_key_pressed(KeyCode::Space) {
            controls.paused = !controls.paused;
        }
        if !keyboard_captured && is_key_pressed(KeyCode::R) {
            playback.reset(0xA11CE5EEDu64 + selected as u64 * 104729);
        }
        if !keyboard_captured && is_key_pressed(KeyCode::Key0) {
            zoom = initial_zoom;
            pan = Vec2::ZERO;
        }

        let toolbar_action = if is_mouse_button_pressed(MouseButton::Left) {
            toolbar_hit(mouse)
        } else {
            ToolbarAction::None
        };
        match toolbar_action {
            ToolbarAction::Previous => select_effect(&library, &mut selected, &mut playback, -1),
            ToolbarAction::Next => select_effect(&library, &mut selected, &mut playback, 1),
            ToolbarAction::Replay => playback.reset(0xA11CE5EEDu64 + selected as u64 * 104729),
            ToolbarAction::Pause => controls.paused = !controls.paused,
            ToolbarAction::Judgment(judgment) => playback.judgment = judgment,
            ToolbarAction::None => {}
        }

        let wheel = mouse_wheel().1;
        let over_canvas =
            !pointer_captured && mouse.y > BAR_HEIGHT && mouse.y < screen_height() - PANEL_HEIGHT;
        if wheel.abs() > 0.01 && over_canvas {
            let center = preview_center();
            let world_at_cursor = (mouse - center - pan) / zoom;
            zoom = (zoom * (wheel * 0.1).exp()).clamp(0.08, 5.0);
            pan = mouse - center - world_at_cursor * zoom;
        }

        if is_mouse_button_pressed(MouseButton::Left) && over_canvas {
            last_mouse = Some(mouse);
        } else if is_mouse_button_down(MouseButton::Left) && over_canvas {
            if let Some(previous) = last_mouse {
                pan += mouse - previous;
                last_mouse = Some(mouse);
            }
        } else {
            last_mouse = None;
        }

        if !keyboard_captured && is_key_pressed(KeyCode::LeftBracket) {
            controls.step(&mut playback, -1.0, end);
        }
        if !keyboard_captured && is_key_pressed(KeyCode::RightBracket) {
            controls.step(&mut playback, 1.0, end);
        }
        let end = preview_duration(&library, &playback, selected);
        controls.advance(&mut playback, dt, end);

        draw_preview(&library, &playback, selected, &controls, zoom, pan);
        egui_macroquad::draw();
        frames += 1;
        if frames >= 3
            && let Some(path) = screenshot.as_ref()
        {
            get_screen_data().export_png(path);
            println!("Captured {effect_name} at {:.3}s: {path}", playback.elapsed);
            return;
        }
        next_frame().await;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolbarAction {
    None,
    Previous,
    Next,
    Replay,
    Pause,
    Judgment(Judgment),
}

fn toolbar_hit(mouse: Vec2) -> ToolbarAction {
    let y = 12.0;
    if Rect::new(12.0, y, 54.0, 34.0).contains(mouse) {
        ToolbarAction::Previous
    } else if Rect::new(72.0, y, 54.0, 34.0).contains(mouse) {
        ToolbarAction::Next
    } else if Rect::new(132.0, y, 76.0, 34.0).contains(mouse) {
        ToolbarAction::Replay
    } else if Rect::new(214.0, y, 76.0, 34.0).contains(mouse) {
        ToolbarAction::Pause
    } else {
        for (index, judgment) in Judgment::ALL.into_iter().enumerate() {
            if judgment_rect(index).contains(mouse) {
                return ToolbarAction::Judgment(judgment);
            }
        }
        ToolbarAction::None
    }
}

fn select_effect(
    library: &EffectLibrary,
    selected: &mut usize,
    playback: &mut EffectInstance,
    direction: i32,
) {
    *selected = if direction >= 0 {
        (*selected + 1) % library.len()
    } else {
        (*selected + library.len() - 1) % library.len()
    };
    let judgment = playback.judgment;
    *playback = library
        .new_instance(*selected)
        .expect("selected effect exists");
    playback.judgment = judgment;
}

fn preview_center() -> Vec2 {
    vec2(
        screen_width() * 0.5,
        (BAR_HEIGHT + screen_height() - PANEL_HEIGHT) * 0.5,
    )
}

fn preview_duration(library: &EffectLibrary, playback: &EffectInstance, selected: usize) -> f32 {
    let duration = playback.duration();
    if duration.is_finite() {
        duration
    } else {
        library
            .effect(selected)
            .unwrap()
            .systems
            .iter()
            .map(|system| system.duration / system.sim_speed.max(0.001))
            .fold(1.0, f32::max)
            * 2.0
    }
}

fn judgment_rect(index: usize) -> Rect {
    let offsets = [0.0, 74.0, 148.0, 222.0, 360.0];
    Rect::new(
        screen_width() - 442.0 + offsets[index],
        12.0,
        if index == 3 { 134.0 } else { 70.0 },
        34.0,
    )
}

fn draw_preview(
    library: &EffectLibrary,
    playback: &EffectInstance,
    selected: usize,
    controls: &PlaybackControls,
    zoom: f32,
    pan: Vec2,
) {
    clear_background(Color::from_rgba(16, 19, 23, 255));
    let center = preview_center() + pan;
    draw_circle_lines(
        center.x,
        center.y,
        120.0 * zoom,
        1.0,
        Color::from_rgba(52, 60, 66, 120),
    );
    draw_line(
        center.x - 8.0,
        center.y,
        center.x + 8.0,
        center.y,
        1.0,
        Color::from_rgba(90, 100, 108, 130),
    );
    draw_line(
        center.x,
        center.y - 8.0,
        center.x,
        center.y + 8.0,
        1.0,
        Color::from_rgba(90, 100, 108, 130),
    );
    library.draw(playback, center, zoom);

    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        BAR_HEIGHT,
        Color::from_rgba(25, 29, 34, 255),
    );
    draw_line(
        0.0,
        BAR_HEIGHT,
        screen_width(),
        BAR_HEIGHT,
        1.0,
        Color::from_rgba(62, 68, 74, 255),
    );

    draw_toolbar_button(Rect::new(12.0, 12.0, 54.0, 34.0), "Prev", false);
    draw_toolbar_button(Rect::new(72.0, 12.0, 54.0, 34.0), "Next", false);
    draw_toolbar_button(Rect::new(132.0, 12.0, 76.0, 34.0), "Replay", false);
    draw_toolbar_button(
        Rect::new(214.0, 12.0, 76.0, 34.0),
        if controls.paused { "Resume" } else { "Pause" },
        controls.paused,
    );

    let name = library
        .effect(selected)
        .map(|effect| effect.name.as_str())
        .unwrap_or("unknown");
    let name = fit_label(name, (judgment_rect(0).x - 326.0).max(0.0));
    draw_text(
        &name,
        310.0,
        35.0,
        19.0,
        Color::from_rgba(230, 233, 235, 255),
    );
    for (index, judgment) in Judgment::ALL.into_iter().enumerate() {
        draw_toolbar_button(
            judgment_rect(index),
            judgment.label(),
            playback.judgment == judgment,
        );
        if let Some(color) = judgment.color() {
            let rect = judgment_rect(index);
            draw_rectangle(
                rect.x + 8.0,
                rect.y + rect.h - 4.0,
                rect.w - 16.0,
                2.0,
                Color::new(color[0], color[1], color[2], 1.0),
            );
        }
    }
    let status = format!(
        "{}/{}    {} systems    {} particles    {:.2}x{}",
        selected + 1,
        library.len(),
        playback.system_count(),
        playback.particle_count(),
        zoom,
        if controls.paused { "    PAUSED" } else { "" }
    );
    let status_width = measure_text(&status, None, 15, 1.0).width;
    draw_text(
        &status,
        (screen_width() - status_width - 20.0).max(310.0),
        screen_height() - PANEL_HEIGHT - 14.0,
        15.0,
        Color::from_rgba(155, 164, 170, 255),
    );
}

fn fit_label(text: &str, width: f32) -> String {
    if measure_text(text, None, 19, 1.0).width <= width {
        return text.to_string();
    }
    let mut text = text.to_string();
    while !text.is_empty() {
        text.pop();
        let candidate = format!("{text}...");
        if measure_text(&candidate, None, 19, 1.0).width <= width {
            return candidate;
        }
    }
    String::new()
}

fn draw_toolbar_button(rect: Rect, label: &str, active: bool) {
    let color = if active {
        Color::from_rgba(65, 88, 89, 255)
    } else {
        Color::from_rgba(40, 46, 51, 255)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        Color::from_rgba(86, 94, 101, 255),
    );
    let metrics = measure_text(label, None, 14, 1.0);
    draw_text(
        label,
        rect.x + (rect.w - metrics.width) * 0.5,
        rect.y + (rect.h + metrics.height * 0.65) * 0.5,
        14.0,
        Color::from_rgba(222, 226, 228, 255),
    );
}
