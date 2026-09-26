//! Pure-macroquad player front-end.
//!
//! See [`crate::player_ui_main`] for the rationale. This module owns the boot
//! sequence, the shared [`UiCtx`] drawing context and the frame loop.

pub mod anim;
pub mod draw;
pub mod input;
pub mod library;
pub mod pages;
pub mod perf;
pub mod state;
pub mod theme;

use macroquad::prelude::*;
use macroquad::Window;

use crate::app::{self, audio, pad_svg, params, platform};
use crate::player;
use crate::player_ui::input::Input;
use crate::player_ui::library::Library;
use crate::player_ui::state::{Page, PlayerUiApp};

/// Window title override for the UI player.
pub fn window_conf() -> Conf {
    let mut conf = app::window_conf();
    conf.window_title = "LambdaDX Player".to_string();
    conf
}

/// Everything a page needs to draw itself: screen size, UI scale, font and the
/// frame clock. Rebuilt once per frame (the font handle is a cheap `Arc` clone).
pub struct UiCtx {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub font: Option<Font>,
    pub now: f64,
    pub dt: f32,
}

/// Candidate system fonts, most preferred first. `MAI2_FONT_PATH` wins.
fn font_candidates() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("MAI2_FONT_PATH") {
        out.push(PathBuf::from(p));
    }
    for p in [
        // Plain `.ttf` first: fontdue rejects many `.ttc` collections.
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto-cjk/NotoSansCJK-VF.otf.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyh.ttf",
    ] {
        out.push(PathBuf::from(p));
    }
    out
}

/// Load the first font that parses. Falls back to the bundled Arial (ASCII
/// only), then to macroquad's built-in font.
fn load_font() -> Option<Font> {
    for path in font_candidates() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if let Ok(font) = load_ttf_font_from_bytes(&bytes) {
            println!("player-ui: font {}", path.display());
            return Some(font);
        }
    }
    load_ttf_font_from_bytes(include_bytes!("../../assets/Arial.ttf")).ok()
}

/// Paint a minimal boot frame (uses macroquad's built-in font).
fn draw_loading(msg: &str) {
    clear_background(theme::VOID);
    let (w, h) = (screen_width(), screen_height());
    let size = 20.0_f32;
    let dims = measure_text(msg, None, size as u16, 1.0);
    draw_text(msg, w * 0.5 - dims.width * 0.5, h * 0.5, size, theme::TEXT_DIM);
    draw_rectangle(w * 0.5 - 120.0, h * 0.5 + 18.0, 240.0, 3.0, theme::BORDER);
    draw_rectangle(w * 0.5 - 120.0, h * 0.5 + 18.0, 60.0, 3.0, theme::ACCENT);
}

pub async fn run() {
    set_pc_assets_folder(&platform::asset_dir().to_string_lossy());

    // Paint a lightweight frame first so the window appears immediately instead
    // of staying blank through the (multi-second) font / texture / audio boot.
    draw_loading("LOADING  ·  LambdaDX Player");
    next_frame().await;

    // ── CLI / chart ────────────────────────────────────────────────────
    let args = match app::cli::parse_env() {
        Ok(Some(a)) => a,
        Ok(None) => {
            print!("{}", app::cli::help());
            return;
        }
        Err(e) => {
            eprintln!("error: {e}\n{}", app::cli::help());
            std::process::exit(2);
        }
    };

    let chart = {
        let _s = perf::Scope::new("boot.chart");
        match &args.chart {
            Some(path) => match app::chart::load_chart_from_path(path, args.diff) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: failed to load chart from {}: {e}", path.display());
                    std::process::exit(2);
                }
            },
            None => app::chart::load_generated_chart(args.diff).await,
        }
    };

    // ── Audio: explicit path > chart folder > bundled assets ───────────
    let (audio_source_name, audio_wav_pcm) = {
        let _s = perf::Scope::new("boot.audio");
        if let Some(path) = &args.audio {
            audio::load_audio_from_path(path)
        } else if let Some(track) = args
            .chart
            .as_deref()
            .filter(|p| p.is_dir())
            .and_then(app::chart::find_audio_in_dir)
        {
            audio::load_audio_from_path(&track)
        } else {
            // No explicit audio: library songs decode lazily at gameplay start.
            (None, None)
        }
    };

    // ── State + assets ─────────────────────────────────────────────────
    let mut app = PlayerUiApp::new(chart, audio_source_name, audio_wav_pcm);

    app.pad.params = perf::time("boot.params", params::load);
    params::set(app.pad.params.clone());
    // Gameplay options come from the same config (流速 etc.).
    app.pad.note_speed = app.pad.params.note_speed_default;
    app.pad.touch_speed = app.pad.params.touch_speed_default;
    app.pad.slide_fade_in = app.pad.params.slide_fade_in;

    match perf::time("boot.pad_svg", || {
        pad_svg::PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg"))
    }) {
        Ok(def) => app.pad.pad_svg = Some(def),
        Err(e) => app.error = Some(format!("pad.svg: {e}")),
    }

    {
        let _s = perf::Scope::new("boot.note_textures");
        player::render::load_note_textures(&mut app.pad).await;
    }
    match perf::time("boot.shader", app::ui::load_mask_material) {
        Ok(m) => app.pad.mask_material = Some(m),
        Err(e) => {
            eprintln!("player-ui: shader: {e}");
            app.error = Some(format!("Shader: {e}"));
        }
    }
    {
        let _s = perf::Scope::new("boot.answer_sfx");
        app.pad.answer_sfx = audio::load_answer_sfx().await;
    }

    {
        let _s = perf::Scope::new("boot.library");
        app.library = Library::load(platform::asset_dir().join("charts"));
    }
    if app.library.songs.is_empty() {
        app.error = Some(format!(
            "曲库为空：{} 中没有 maidata.txt",
            app.library.root.display()
        ));
    }

    // Dev harness: `MAI2_UI_PAGE=select|settings|game|pause` jumps straight to a
    // page, and `MAI2_UI_SHOT=path.png` writes one screenshot after ~1s then
    // exits. Used for headless visual checks.
    if let Ok(page) = std::env::var("MAI2_UI_PAGE") {
        if !app.library.songs.is_empty() {
            let _ = app.load_song(app.selected);
        }
        match page.as_str() {
            "select" => app.page = Page::SongSelect,
            "settings" => app.open_settings(),
            "game" => {
                let _ = app.begin_gameplay();
            }
            "pause" => {
                let _ = app.begin_gameplay();
                app.page = Page::Pause;
            }
            _ => {}
        }
        app.page_born = get_time() - 5.0;
    }
    if std::env::var("MAI2_UI_PARAMS").is_ok() {
        app.pad.show_params = true;
    }
    if std::env::var("MAI2_UI_AUTOPLAY").is_ok() {
        app.set_autoplay(true);
    }
    let shot_path = std::env::var("MAI2_UI_SHOT").ok();
    let shot_at = std::env::var("MAI2_UI_SHOT_AT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    // Dev harness: trigger a page switch at ~0.4s so a mid-transition frame can
    // be captured with `MAI2_UI_SHOT`.
    let goto_test = std::env::var("MAI2_UI_GOTO_TEST").is_ok();
    let mut goto_done = false;
    let start_time = get_time();

    let font = perf::time("boot.font", load_font);
    let mut input = Input::default();
    let mut last = get_time();

    // Prime egui so the F1 params panel gets correct input on first open.
    egui_macroquad::ui(|_| {});
    egui_macroquad::draw();

    loop {
        let now = get_time();
        let dt = ((now - last) as f32).clamp(0.0, 0.1);
        last = now;

        clear_background(theme::VOID);

        let ctx = UiCtx {
            w: screen_width(),
            h: screen_height(),
            scale: player::render::ui_scale(&app.pad),
            font: font.clone(),
            now,
            dt,
        };

        // F1 toggles the live params panel (egui), drawn on top of our UI.
        if is_key_pressed(KeyCode::F1) {
            app.pad.show_params = !app.pad.show_params;
        }

        input.begin_frame();
        app.poll_audio();
        app.poll_chart();
        pages::draw(&mut app, &mut input, &ctx);
        input.end_frame();

        // Dev harness page switch (equivalent to a button click mid-draw).
        if goto_test && !goto_done && now - start_time > 0.4 {
            app.go(Page::SongSelect);
            goto_done = true;
        }

        // Page transition: capture the outgoing scene once, then composite the
        // masked wipe (next scene left, previous scene sliding right).
        pages::capture_transition(&mut app, &ctx);
        pages::draw_transition(&mut app, &ctx);

        // Fill in cover art without stalling any single frame.
        app.library.pump_covers(6.0);

        audio::service_audio(&mut app.pad).await;
        if !app.scrubbing {
            app.pad.tick_cues();
        }
        app.tick_autoplay();
        player::engine::step_judge_engine(&mut app.pad);
        app.pad.tick_feedback();

        if app.pad.playback_pending && app.gameplay_presented {
            app.pad.finalize_playback_start();
        }
        app.gameplay_presented = app.page == Page::Gameplay;

        if app.pad.show_params {
            egui_macroquad::ui(|egui_ctx| player::params_panel::draw(egui_ctx, &mut app.pad));
            egui_macroquad::draw();
        }

        if let Some(path) = &shot_path {
            if now - start_time > shot_at {
                get_screen_data().export_png(path);
                println!("player-ui: wrote {path}");
                std::process::exit(0);
            }
        }

        next_frame().await;
    }
}

/// Kept for parity with the other bin; the UI player always opens a window.
pub fn unused(_: Window) {}
