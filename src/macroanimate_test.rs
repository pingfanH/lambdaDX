//! Test bin for the [`macroanimate`] crate — Adobe Animate **sparrow** and
//! **texture atlas** parser/renderer for macroquad.
//!
//! Usage:
//! ```text
//! macroanimate_test <DIR>                       auto-detect sparrow or atlas
//! macroanimate_test --sparrow <xml> <png>
//! macroanimate_test --atlas <spritemap.json> <Animation.json> <png>
//! ```
//! Env:
//!   * `MAI2_ANIMATE_DIR`  — default directory (`assets/animate`)
//!   * `MAI2_ANIMATE_ANIM` — animation name/prefix to start on (`idle`)
//!   * `MAI2_UI_SHOT` / `MAI2_UI_SHOT_AT` — write a screenshot and exit
//!
//! Keys: `Space` pause · `←/→` prev/next animation · `↑/↓` fps · `F1`? none.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use macroquad::prelude::*;
use macroquad::Window;
use macroanimate::{draw_part_mesh, get_texture_parts, parse_sparrow, parse_texture_atlas};

/// Where the animation frames come from.
enum Source {
    Sparrow { xml: String, name: String },
    Atlas { atlas: macroanimate::TextureAtlas },
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
macroanimate_test --atlas <spritemap.json> <Animation.json> <png>";

struct Launch {
    texture: PathBuf,
    source: SourceKind,
    start_anim: String,
}

enum SourceKind {
    Sparrow(PathBuf),
    Atlas { spritemap: PathBuf, animation: PathBuf },
}

fn parse(args: &[String]) -> Result<Launch, String> {
    let start_anim = std::env::var("MAI2_ANIMATE_ANIM").unwrap_or_else(|_| "idle".to_string());
    match args.first().map(String::as_str) {
        Some("--sparrow") => {
            let xml = args.get(1).ok_or("--sparrow needs <xml>")?;
            let png = args.get(2).ok_or("--sparrow needs <png>")?;
            Ok(Launch {
                texture: PathBuf::from(png),
                source: SourceKind::Sparrow(PathBuf::from(xml)),
                start_anim,
            })
        }
        Some("--atlas") => {
            let sm = args.get(1).ok_or("--atlas needs <spritemap.json>")?;
            let an = args.get(2).ok_or("--atlas needs <Animation.json>")?;
            let png = args.get(3).ok_or("--atlas needs <png>")?;
            Ok(Launch {
                texture: PathBuf::from(png),
                source: SourceKind::Atlas {
                    spritemap: PathBuf::from(sm),
                    animation: PathBuf::from(an),
                },
                start_anim,
            })
        }
        Some(dir) if !dir.starts_with('-') => auto_detect(Path::new(dir), start_anim),
        _ => {
            let dir = std::env::var("MAI2_ANIMATE_DIR").unwrap_or_else(|_| "assets/animate".into());
            auto_detect(Path::new(&dir), start_anim)
        }
    }
}

/// Look inside `dir` for a sparrow (`*.xml` + `*.png`) or atlas
/// (`*spritemap*.json` + `*Animation*.json` + `*.png`) set.
fn auto_detect(dir: &Path, start_anim: String) -> Result<Launch, String> {
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
        });
    }
    Err(format!(
        "no sparrow (.xml) or atlas (*spritemap*.json + *Animation*.json) in {}",
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
    // macroquad resolves texture paths relative to the assets folder, so point it
    // at the texture's directory and load by file name.
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
    };

    let mut anims = animation_names(&source);
    anims.sort();
    if anims.is_empty() {
        anims.push(launch.start_anim.clone());
    }
    let mut anim_idx = anims
        .iter()
        .position(|a| a == &launch.start_anim)
        .unwrap_or(0);
    println!("animations: {anims:?}");

    let mut frame = 0usize;
    let mut paused = false;
    let mut fps = 24.0f32;
    let mut acc = 0.0f32;
    let start = Instant::now();
    let shot_path = std::env::var("MAI2_UI_SHOT").ok();
    let shot_at: f64 = std::env::var("MAI2_UI_SHOT_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);

    loop {
        let dt = get_frame_time();
        clear_background(Color::from_rgba(24, 24, 28, 255));

        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
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
        if is_key_pressed(KeyCode::Up) {
            fps += 2.0;
        }
        if is_key_pressed(KeyCode::Down) {
            fps = (fps - 2.0).max(1.0);
        }

        let anim = anims[anim_idx].clone();
        let count = frame_count(&source, &anim).max(1);
        if !paused {
            acc += dt;
            while acc >= 1.0 / fps {
                acc -= 1.0 / fps;
                frame = (frame + 1) % count;
            }
        } else {
            frame %= count;
        }

        // Draw centred.
        let cx = screen_width() * 0.5;
        let cy = screen_height() * 0.5;

        match &source {
            Source::Sparrow { xml, .. } => {
                let frames = parse_sparrow(xml, &anim);
                if let Some(f) = frames.get(frame % frames.len().max(1)) {
                    draw_texture_ex(
                        &texture,
                        cx - f.frame_x,
                        cy - f.frame_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(f.x, f.y, f.width, f.height)),
                            dest_size: Some(vec2(f.width, f.height)),
                            ..Default::default()
                        },
                    );
                }
            }
            Source::Atlas { atlas } => {
                // MacroAnimate lays parts out on the animation canvas; map the
                // canvas centre onto the window centre.
                let ox = cx - atlas.canvas_w * 0.5;
                let oy = cy - atlas.canvas_h * 0.5;
                for part in get_texture_parts(atlas, &anim, frame) {
                    if let Some(sprite) = atlas.sprites.get(&part.sprite_name) {
                        draw_part_mesh(
                            &texture,
                            sprite,
                            &part.matrix,
                            atlas.sheet_w,
                            atlas.sheet_h,
                            ox,
                            oy,
                        );
                    }
                }
            }
        }

        draw_text(
            &format!("{anim}  frame {}/{}  {:.0}fps  {}", frame + 1, count, fps, if paused { "PAUSED" } else { "" }),
            12.0,
            24.0,
            20.0,
            Color::from_rgba(220, 220, 226, 255),
        );
        draw_text(
            "Space pause  Left/Right anim  Up/Down fps",
            12.0,
            screen_height() - 12.0,
            16.0,
            Color::from_rgba(150, 150, 158, 255),
        );

        if let Some(path) = &shot_path {
            if start.elapsed().as_secs_f64() > shot_at {
                get_screen_data().export_png(path);
                println!("wrote {path}");
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
    }
}

/// Same asset-directory resolution as the other bins.
fn assets_dir() -> PathBuf {
    std::env::var("MAI2_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"))
}
