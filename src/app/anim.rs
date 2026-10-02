//! General Adobe Animate (XFL/`.fla`) loader.
//!
//! Two projects are loaded at startup and their **movies/sprites are referenced
//! by name inside the project** — mirroring `ui.get("tap_perfect")` — rather
//! than by file path:
//!
//! * `ui` — the pad/gameplay effect library (`assets/ui`).
//! * `player_ui` — the player front-end built for Flash (`assets/player_ui`),
//!   addressed as `UI/page_start`, `UI/ui_row`, …
//!
//! ```no_run
//! crate::app::anim::with_player_ui(|ui| {
//!     let Some(ui) = ui else { return };
//!     if let Some(clip) = ui.get("UI/page_start") {
//!         clip.draw(0, &macroanimate::XflDrawXf::default());
//!     }
//! });
//! ```
//!
//! A project is resolved from [`platform::asset_dir`] (so it honours
//! `MAI2_ASSET_DIR` / Nix / mobile). Textures are built once at load; set
//! `MAI2_HOT_RELOAD=1` to reload a project when its file changes on disk.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use macroanimate::XflAsset;

use crate::app::platform;

/// The pad/gameplay effect project. Its movies are addressed by name.
const EFFECT_PROJECT: &[&str] = &["ui", "ui.xfl", "ui.fla"];
/// The player front-end project (symbols live under the `UI` folder).
const PLAYER_PROJECT: &[&str] = &["player_ui", "player_ui.xfl", "player_ui.fla"];

struct Loaded {
    path: PathBuf,
    mtime: Option<SystemTime>,
    asset: XflAsset,
}

thread_local! {
    static UI: RefCell<Option<Loaded>> = const { RefCell::new(None) };
    static PLAYER: RefCell<Option<Loaded>> = const { RefCell::new(None) };
}

/// Resolve a project under the asset dir (directory, then `.xfl`, then `.fla`).
fn project_path(candidates: &[&str]) -> PathBuf {
    let base = platform::asset_dir();
    for candidate in candidates {
        let path = base.join(candidate);
        if path.exists() {
            return path;
        }
    }
    base.join(candidates[1])
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn load(candidates: &[&str]) -> Option<Loaded> {
    let path = project_path(candidates);
    match XflAsset::load(&path) {
        Ok(asset) => {
            let mtime = mtime(&path);
            Some(Loaded { path, mtime, asset })
        }
        Err(e) => {
            eprintln!("anim: failed to load {}: {e}", path.display());
            None
        }
    }
}

fn hot_reload(slot: &'static std::thread::LocalKey<RefCell<Option<Loaded>>>, candidates: &[&str]) {
    if std::env::var("MAI2_HOT_RELOAD").is_err() {
        return;
    }
    let current = slot.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|loaded| (loaded.path.clone(), loaded.mtime))
    });
    let Some((path, old_mtime)) = current else {
        return;
    };
    if mtime(&path).is_some() && mtime(&path) != old_mtime {
        if let Some(loaded) = load(candidates) {
            slot.with(|cell| *cell.borrow_mut() = Some(loaded));
        }
    }
}

// ── Effect project (`ui`) ───────────────────────────────────────────────

/// Load the effect project. Safe to call after the window/GPU is ready.
pub fn reload() {
    if let Some(loaded) = load(EFFECT_PROJECT) {
        UI.with(|cell| *cell.borrow_mut() = Some(loaded));
    }
}

pub fn init() {
    reload();
}

pub fn tick() {
    hot_reload(&UI, EFFECT_PROJECT);
}

pub fn with<R>(f: impl FnOnce(Option<&XflAsset>) -> R) -> R {
    UI.with(|cell| f(cell.borrow().as_ref().map(|loaded| &loaded.asset)))
}

pub fn is_loaded() -> bool {
    UI.with(|cell| cell.borrow().is_some())
}

// ── Player front-end project (`player_ui`) ──────────────────────────────

/// Load the player front-end project. Safe after the window/GPU is ready.
pub fn reload_player_ui() {
    if let Some(loaded) = load(PLAYER_PROJECT) {
        PLAYER.with(|cell| *cell.borrow_mut() = Some(loaded));
    }
}

pub fn init_player_ui() {
    reload_player_ui();
}

pub fn tick_player_ui() {
    hot_reload(&PLAYER, PLAYER_PROJECT);
}

pub fn with_player_ui<R>(f: impl FnOnce(Option<&XflAsset>) -> R) -> R {
    PLAYER.with(|cell| f(cell.borrow().as_ref().map(|loaded| &loaded.asset)))
}

pub fn player_ui_loaded() -> bool {
    PLAYER.with(|cell| cell.borrow().is_some())
}
