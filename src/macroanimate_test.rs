//! Test bin for the [`macroanimate`] crate — Adobe Animate **sparrow**,
//! **texture atlas** and **XFL/FLA** parser/renderer for macroquad.
//!
//! Usage:
//! ```text
//! macroanimate_test <DIR>                       auto-detect sparrow / atlas / XFL
//! macroanimate_test --sparrow <xml> <png>
//! macroanimate_test --atlas <spritemap.json> <Animation.json> <png>
//! macroanimate_test --xfl <xfl-dir-or-fla>
//! ```
//! Flags (any position): `--anim <name>` · `--zoom <f>` · `--speed <f>` ·
//! `--fps <f>` (absolute, disables frame-rate sync).
//!
//! Env:
//!   * `MAI2_ANIMATE_DIR`   — default directory (`assets/animate`)
//!   * `MAI2_ANIMATE_ANIM`  — animation name/prefix to start on (`idle`)
//!   * `MAI2_ANIMATE_ZOOM`  — initial zoom (default 1.0)
//!   * `MAI2_ANIMATE_SPEED` — initial speed multiplier (default 1.0)
//!   * `MAI2_ANIMATE_FPS`   — absolute playback fps override
//!   * `MAI2_UI_SHOT` / `MAI2_UI_SHOT_AT` — write a screenshot and exit
//!
//! Playback advances at the source's **native frame rate** (`speed` scales it),
//! so a 60 fps XFL and a 24 fps sparrow play at their intended real-time speed.
//!
//! Keys: `Space` pause · `←/→` prev/next animation · `↑/↓` speed ·
//! `=`/`-` or wheel zoom · **left-drag** pan · `0` reset view · `R` reset all.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use macroquad::prelude::*;
use macroquad::Window;
use macroanimate::{XflAsset, XflDrawXf, draw_part_mesh, get_texture_parts, parse_sparrow, parse_texture_atlas};

/// Playback / view configuration (CLI/env seeded, adjustable at runtime).
#[derive(Clone, Copy)]
struct Config {
    /// Uniform view scale around the window centre.
    zoom: f32,
    /// View translation in window pixels (drag to move).
    pan: (f32, f32),
    /// Multiplier applied on top of the source's native frame rate.
    speed: f32,
    /// Absolute playback fps; when set, frame-rate sync is disabled.
    fps_override: Option<f32>,
    paused: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            zoom: 1.0,
            pan: (0.0, 0.0),
            speed: 1.0,
            fps_override: None,
            paused: false,
        }
    }
}

/// Where the animation frames come from.
enum Source {
    Sparrow { xml: String, name: String },
    Atlas { atlas: macroanimate::TextureAtlas },
    Xfl { asset: XflAsset, dir: PathBuf },
}

impl Source {
    /// The frame rate this source was authored at, used to keep real-time speed
    /// consistent between exports.
    fn native_fps(&self) -> f32 {
        match self {
            Source::Xfl { asset, .. } => asset.frame_rate(),
            Source::Atlas { atlas } => atlas.frame_rate,
            Source::Sparrow { .. } => 24.0,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let launch = match parse(&args) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    Window::from_config(window_conf(), run(launch));
}

const USAGE: &str = "macroanimate_test <DIR>\n\
macroanimate_test --sparrow <xml> <png>\n\
macroanimate_test --atlas <spritemap.json> <Animation.json> <png>\n\
macroanimate_test --xfl <xfl-dir-or-fla>";

struct Launch {
    texture: PathBuf,
    source: SourceKind,
    start_anim: String,
    config: Config,
}

enum SourceKind {
    Sparrow(PathBuf),
    Atlas { spritemap: PathBuf, animation: PathBuf },
    Xfl(PathBuf),
}

fn parse(args: &[String]) -> Result<Launch, String> {
    let config = Config {
        zoom: arg_or_env(args, "--zoom", "MAI2_ANIMATE_ZOOM")
            .map(|v| v.parse())
            .transpose()
            .map_err(|_| "invalid --zoom".to_string())?
            .unwrap_or(1.0),
        pan: (0.0, 0.0),
        speed: arg_or_env(args, "--speed", "MAI2_ANIMATE_SPEED")
            .map(|v| v.parse())
            .transpose()
            .map_err(|_| "invalid --speed".to_string())?
            .unwrap_or(1.0),
        fps_override: arg_or_env(args, "--fps", "MAI2_ANIMATE_FPS")
            .map(|v| v.parse())
            .transpose()
            .map_err(|_| "invalid --fps".to_string())?,
        paused: false,
    };
    let start_anim = arg_or_env(args, "--anim", "MAI2_ANIMATE_ANIM")
        .unwrap_or_else(|| "idle".to_string());

    // Explicit source flags win; otherwise the first positional argument.
    let (texture, source) = if let Some(i) = args.iter().position(|a| a == "--sparrow") {
        let xml = args.get(i + 1).ok_or("--sparrow needs <xml>")?;
        let png = args.get(i + 2).ok_or("--sparrow needs <png>")?;
        (
            PathBuf::from(png),
            SourceKind::Sparrow(PathBuf::from(xml)),
        )
    } else if let Some(i) = args.iter().position(|a| a == "--atlas") {
        let sm = args.get(i + 1).ok_or("--atlas needs <spritemap.json>")?;
        let an = args.get(i + 2).ok_or("--atlas needs <Animation.json>")?;
        let png = args.get(i + 3).ok_or("--atlas needs <png>")?;
        (
            PathBuf::from(png),
            SourceKind::Atlas {
                spritemap: PathBuf::from(sm),
                animation: PathBuf::from(an),
            },
        )
    } else if let Some(i) = args.iter().position(|a| a == "--xfl") {
        let path = args.get(i + 1).ok_or("--xfl needs <dir-or-fla>")?;
        (PathBuf::new(), SourceKind::Xfl(PathBuf::from(path)))
    } else {
        let dir = positionals(args)
            .into_iter()
            .next()
            .or_else(|| std::env::var("MAI2_ANIMATE_DIR").ok())
            .unwrap_or_else(|| "assets/animate".to_string());
        return auto_detect(Path::new(&dir), start_anim, config);
    };

    Ok(Launch {
        texture,
        source,
        start_anim,
        config,
    })
}

/// Positional (non-flag) arguments, skipping the value of every `--flag`.
fn positionals(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            i += 2; // flag + value
        } else {
            out.push(args[i].clone());
            i += 1;
        }
    }
    out
}

fn arg_or_env(args: &[String], flag: &str, env: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| std::env::var(env).ok())
}

/// Look inside `dir` for a sparrow (`*.xml` + `*.png`), atlas
/// (`*spritemap*.json` + `*Animation*.json` + `*.png`) or XFL/`.fla` set.
fn auto_detect(dir: &Path, start_anim: String, config: Config) -> Result<Launch, String> {
    // An XFL project (unpacked directory or `.fla`/`.zip` archive).
    if dir.join("DOMDocument.xml").is_file() || ext_is(dir, &["fla", "zip"]) {
        return Ok(Launch {
            texture: PathBuf::new(),
            source: SourceKind::Xfl(dir.to_path_buf()),
            start_anim,
            config,
        });
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .collect::<Vec<_>>();

    let png = entries
        .iter()
        .find(|p| ext_is(p, &["png"]))
        .cloned()
        .ok_or_else(|| format!("no .png texture in {}", dir.display()))?;
    let xml = entries.iter().find(|p| ext_is(p, &["xml"])).cloned();
    let spritemap = entries
        .iter()
        .find(|p| {
            ext_is(p, &["json"])
                && file_name_lower(p).contains("spritemap")
        })
        .cloned();
    let animation = entries
        .iter()
        .find(|p| {
            ext_is(p, &["json"])
                && file_name_lower(p).contains("animation")
        })
        .cloned();

    if let Some(xml) = xml {
        return Ok(Launch {
            texture: png,
            source: SourceKind::Sparrow(xml),
            start_anim,
            config,
        });
    }
    if let (Some(spritemap), Some(animation)) = (spritemap, animation) {
        return Ok(Launch {
            texture: png,
            source: SourceKind::Atlas {
                spritemap,
                animation,
            },
            start_anim,
            config,
        });
    }
    Err(format!(
        "no sparrow (.xml), atlas (*spritemap*.json + *Animation*.json) or XFL in {}",
        dir.display()
    ))
}

fn ext_is(p: &Path, exts: &[&str]) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn file_name_lower(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn window_conf() -> Conf {
    Conf {
        window_title: "macroanimate test".to_string(),
        window_width: 960,
        window_height: 640,
        high_dpi: true,
        ..Default::default()
    }
}

async fn run(launch: Launch) {
    // ── XFL / FLA projects are self-contained: build every bitmap texture up
    // front and hand the atlas + texture map to the draw loop. ──
    let (source, texture) = if let SourceKind::Xfl(path) = &launch.source {
        let asset = match XflAsset::load(path) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(2);
            }
        };
        let dir = if path.is_dir() {
            path.clone()
        } else {
            path.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        let clips: Vec<&str> = asset.clips().collect();
        println!(
            "xfl: {} clips @ {}fps: {clips:?}",
            clips.len(),
            asset.frame_rate()
        );
        (Source::Xfl { asset, dir }, None)
    } else {
        // macroquad resolves texture paths relative to the assets folder, so
        // point it at the texture's directory and load by file name.
        let tex_dir = launch
            .texture
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        set_pc_assets_folder(&tex_dir.to_string_lossy());
        let tex_name = launch
            .texture
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let texture = match load_texture(&tex_name).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: cannot load texture {}: {e}", launch.texture.display());
                std::process::exit(2);
            }
        };
        texture.set_filter(FilterMode::Linear);

        let source = match &launch.source {
            SourceKind::Sparrow(xml_path) => match std::fs::read_to_string(xml_path) {
                Ok(xml) => Source::Sparrow {
                    xml,
                    name: launch.start_anim.clone(),
                },
                Err(e) => {
                    eprintln!("error: cannot read {}: {e}", xml_path.display());
                    std::process::exit(2);
                }
            },
            SourceKind::Atlas {
                spritemap,
                animation,
            } => {
                let sm = read_or_exit(spritemap);
                let an = read_or_exit(animation);
                let atlas = parse_texture_atlas(&sm, &an);
                println!(
                    "atlas: {} sprites, {} animations, sheet {}x{}, canvas {}x{}",
                    atlas.sprites.len(),
                    atlas.animations.len(),
                    atlas.sheet_w,
                    atlas.sheet_h,
                    atlas.canvas_w,
                    atlas.canvas_h
                );
                Source::Atlas { atlas }
            }
            SourceKind::Xfl(_) => unreachable!(),
        };
        (source, Some(texture))
    };

    let mut anims = animation_names(&source);
    anims.sort();
    if anims.is_empty() {
        anims.push(launch.start_anim.clone());
    }
    // Pick the requested clip; if it is missing (or a static 1-frame clip that
    // would look like "Space does nothing"), fall back to the longest one so
    // there is always something to play.
    let by_name = anims.iter().position(|a| a == &launch.start_anim);
    let longest = |anims: &[String]| {
        anims
            .iter()
            .enumerate()
            .max_by_key(|(_, a)| frame_count(&source, a))
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let mut anim_idx = match by_name {
        Some(i) if frame_count(&source, &anims[i]) > 1 => i,
        _ => longest(&anims),
    };
    println!(
        "animations: {anims:?}  (playing `{}`, {} frames)",
        anims[anim_idx],
        frame_count(&source, &anims[anim_idx])
    );

    let mut frame = 0usize;
    let mut cfg = launch.config;
    let mut acc = 0.0f32;
    let mut prev_mouse: Option<(f32, f32)> = None;
    let start = Instant::now();
    let shot_path = std::env::var("MAI2_UI_SHOT").ok();
    let shot_at: f64 = std::env::var("MAI2_UI_SHOT_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);

    // Base frame rate used to keep playback real-time across exports.
    let native_fps = source.native_fps().max(1.0);
    println!(
        "native frame rate {}fps · zoom {:.2} · speed {:.2}×{}",
        native_fps,
        cfg.zoom,
        cfg.speed,
        match cfg.fps_override {
            Some(f) => format!(" · fps override {f}"),
            None => String::new(),
        }
    );

    loop {
        let dt = get_frame_time();
        clear_background(Color::from_rgba(24, 24, 28, 255));

        if is_key_pressed(KeyCode::Space) {
            cfg.paused = !cfg.paused;
        }
        if is_key_pressed(KeyCode::Right) {
            anim_idx = (anim_idx + 1) % anims.len();
            frame = 0;
            acc = 0.0;
        }
        if is_key_pressed(KeyCode::Left) {
            anim_idx = (anim_idx + anims.len() - 1) % anims.len();
            frame = 0;
            acc = 0.0;
        }
        // Speed multiplier of the native frame rate.
        if is_key_pressed(KeyCode::Up) {
            cfg.speed = (cfg.speed + 0.1).min(8.0);
        }
        if is_key_pressed(KeyCode::Down) {
            cfg.speed = (cfg.speed - 0.1).max(0.1);
        }
        // Zoom: keys, wheel, reset.
        if is_key_pressed(KeyCode::Equal) {
            cfg.zoom = (cfg.zoom * 1.1).min(8.0);
        }
        if is_key_pressed(KeyCode::Minus) {
            cfg.zoom = (cfg.zoom / 1.1).max(0.05);
        }
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.01 {
            cfg.zoom = (cfg.zoom * (1.0 + wheel * 0.1)).clamp(0.05, 8.0);
        }
        // Pan: drag with the left mouse button.
        let mouse = mouse_position();
        if is_mouse_button_down(MouseButton::Left) {
            if let Some(prev) = prev_mouse {
                cfg.pan.0 += mouse.0 - prev.0;
                cfg.pan.1 += mouse.1 - prev.1;
            }
        }
        prev_mouse = Some(mouse);
        if is_key_pressed(KeyCode::Key0) {
            cfg.zoom = 1.0;
            cfg.pan = (0.0, 0.0);
        }
        if is_key_pressed(KeyCode::R) {
            cfg.zoom = 1.0;
            cfg.pan = (0.0, 0.0);
            cfg.speed = 1.0;
            cfg.fps_override = None;
        }

        let anim = anims[anim_idx].clone();
        let count = frame_count(&source, &anim).max(1);
        // Real-time playback: honour the source's authored frame rate unless an
        // absolute fps override is set.
        let playback_fps = cfg.fps_override.unwrap_or(native_fps * cfg.speed).max(0.1);
        let step = 1.0 / playback_fps;
        if !cfg.paused {
            acc += dt;
            while acc >= step {
                acc -= step;
                frame = (frame + 1) % count;
            }
        } else {
            frame %= count;
        }

        // Draw centred, with a uniform zoom about the window centre and a pan.
        let cx = screen_width() * 0.5;
        let cy = screen_height() * 0.5;
        let zoom = cfg.zoom;
        let (pan_x, pan_y) = cfg.pan;
        let zoom_origin = |base: (f32, f32)| {
            (
                cx + pan_x + (base.0 - cx) * zoom,
                cy + pan_y + (base.1 - cy) * zoom,
            )
        };

        match &source {
            Source::Sparrow { xml, .. } => {
                let texture = texture.as_ref().expect("sparrow texture");
                let frames = parse_sparrow(xml, &anim);
                if let Some(f) = frames.get(frame % frames.len().max(1)) {
                    draw_texture_ex(
                        texture,
                        cx + pan_x - f.frame_x * zoom,
                        cy + pan_y - f.frame_y * zoom,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(f.x, f.y, f.width, f.height)),
                            dest_size: Some(vec2(f.width * zoom, f.height * zoom)),
                            ..Default::default()
                        },
                    );
                }
            }
            Source::Atlas { atlas } => {
                let texture = texture.as_ref().expect("atlas texture");
                // MacroAnimate lays parts out on the animation canvas; map the
                // canvas centre onto the window centre, then apply zoom.
                let (ox, oy) = zoom_origin((
                    cx - atlas.canvas_w * 0.5,
                    cy - atlas.canvas_h * 0.5,
                ));
                for part in get_texture_parts(atlas, &anim, frame) {
                    if let Some(sprite) = atlas.sprites.get(&part.sprite_name) {
                        draw_part_mesh(
                            texture,
                            sprite,
                            &scale_matrix(&part.matrix, zoom),
                            atlas.sheet_w,
                            atlas.sheet_h,
                            ox,
                            oy,
                        );
                    }
                }
            }
            Source::Xfl { asset, dir: _ } => {
                // XFL coordinates are top-left based; centre the project canvas.
                let (ox, oy) = zoom_origin((cx, cy));
                if let Some(clip) = asset.get(&anim) {
                    clip.draw(
                        frame,
                        &XflDrawXf {
                            pos: (ox, oy),
                            scale: zoom,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        draw_text(
            &format!(
                "{anim}  frame {}/{}  {playback_fps:.0}fps  {:.2}x  zoom {zoom:.2}  {}",
                frame + 1,
                count,
                cfg.speed,
                if cfg.paused { "PAUSED" } else { "" }
            ),
            12.0,
            24.0,
            20.0,
            Color::from_rgba(220, 220, 226, 255),
        );
        draw_text(
            "Space pause  Left/Right anim  Up/Down speed  =/- wheel zoom  drag move  0 reset view  R reset all",
            12.0,
            screen_height() - 12.0,
            16.0,
            Color::from_rgba(150, 150, 158, 255),
        );

        if let Some(path) = &shot_path {
            if start.elapsed().as_secs_f64() > shot_at {
                get_screen_data().export_png(path);
                println!(
                    "wrote {path} (`{anim}` frame {frame}, {playback_fps:.1}fps, zoom {zoom:.2})"
                );
                std::process::exit(0);
            }
        }

        next_frame().await;
    }
}

fn read_or_exit(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", path.display());
            std::process::exit(2);
        }
    }
}

/// All animation names available in the source.
fn animation_names(source: &Source) -> Vec<String> {
    match source {
        Source::Atlas { atlas } => atlas.animations.keys().cloned().collect(),
        Source::Xfl { asset, .. } => asset.clips().map(String::from).collect(),
        Source::Sparrow { xml, .. } => {
            let mut names = Vec::new();
            for line in xml.lines() {
                let line = line.trim();
                if !line.starts_with("<SubTexture") {
                    continue;
                }
                let Some(ns) = line.find("name=\"") else {
                    continue;
                };
                let start = ns + 6;
                let Some(ne) = line[start..].find('"') else {
                    continue;
                };
                let name = &line[start..start + ne];
                // Sparrow frame names are `<anim><digits>`; strip the digits.
                let prefix: String = name
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .to_string();
                if !prefix.is_empty() && !names.contains(&prefix) {
                    names.push(prefix);
                }
            }
            names
        }
    }
}

/// Number of frames in an animation.
fn frame_count(source: &Source, anim: &str) -> usize {
    match source {
        Source::Sparrow { xml, .. } => parse_sparrow(xml, anim).len(),
        Source::Atlas { atlas } => atlas
            .animations
            .get(anim)
            .map(|(_, dur, _, _)| *dur)
            .unwrap_or(1),
        Source::Xfl { asset, .. } => asset.get(anim).map(|c| c.frames()).unwrap_or(1),
    }
}

/// Uniformly scale a Flash affine matrix (linear part + translation).
fn scale_matrix(m: &[f32; 6], s: f32) -> [f32; 6] {
    [m[0] * s, m[1] * s, m[2] * s, m[3] * s, m[4] * s, m[5] * s]
}

/// Same asset-directory resolution as the other bins.
fn assets_dir() -> PathBuf {
    std::env::var("MAI2_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"))
}
