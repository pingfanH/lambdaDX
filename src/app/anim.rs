//! General Adobe Animate (XFL/`.fla`) loader.
//!
//! One project is loaded at startup and its **movies/sprites are referenced by
//! name inside the project** — mirroring `ui.get("tap_perfect")` — rather than
//! by file path:
//!
//! ```no_run
//! crate::app::anim::with(|ui| {
//!     let Some(ui) = ui else { return };
//!     if let Some(clip) = ui.get("tap_perfect") {
//!         clip.draw(0, &macroanimate::XflDrawXf { pos: (100.0, 100.0), ..Default::default() });
//!     }
//! });
//! ```
//!
//! The project file is resolved from [`platform::asset_dir`] (so it honours
//! `MAI2_ASSET_DIR` / Nix / mobile). Textures are built once at load; set
//! `MAI2_HOT_RELOAD=1` to reload the project when its file changes on disk.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use macroanimate::XflAsset;

use crate::app::platform;

/// The single project supported for now. Its movies are addressed by name.
/// Prefer the unpacked directory (the live project you edit in Animate), then
/// an `.xfl`, then a packed `.fla`.
const PROJECT: &[&str] = &["ui", "ui.xfl", "ui.fla"];

struct Loaded {
    path: PathBuf,
    mtime: Option<SystemTime>,
    asset: XflAsset,
}

thread_local! {
    static UI: RefCell<Option<Loaded>> = const { RefCell::new(None) };
}

/// Resolve the project file under the asset dir (`.fla`, then directory).
fn project_path() -> PathBuf {
    let base = platform::asset_dir();
    for candidate in PROJECT {
        let path = base.join(candidate);
        if path.exists() {
            return path;
        }
    }
    base.join(PROJECT[1])
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Load (or reload) the project. Safe to call after the window/GPU is ready.
pub fn reload() {
    let path = project_path();
    match XflAsset::load(&path) {
        Ok(asset) => {
            let mtime = mtime(&path);
            UI.with(|cell| {
                *cell.borrow_mut() = Some(Loaded {
                    path,
                    mtime,
                    asset,
                })
            });
        }
        Err(e) => {
            eprintln!("anim: failed to load {}: {e}", path.display());
        }
    }
}

/// Load the project at startup.
pub fn init() {
    reload();
}

/// Hot reload (dev only): reload when the project file changes on disk.
pub fn tick() {
    if std::env::var("MAI2_HOT_RELOAD").is_err() {
        return;
    }
    let current = UI.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|loaded| (loaded.path.clone(), loaded.mtime))
    });
    let Some((path, old_mtime)) = current else {
        return;
    };
    let now = mtime(&path);
    if now.is_some() && now != old_mtime {
        reload();
    }
}

/// Run `f` with the loaded project, if any. `ui.get("…")` lives on the value.
pub fn with<R>(f: impl FnOnce(Option<&XflAsset>) -> R) -> R {
    UI.with(|cell| f(cell.borrow().as_ref().map(|loaded| &loaded.asset)))
}

/// Whether the project is loaded.
pub fn is_loaded() -> bool {
    UI.with(|cell| cell.borrow().is_some())
}
