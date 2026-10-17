use macroquad::file::load_file;
use std::fs;
use std::path::{Path, PathBuf};

const ASSET_DIR_ENV: &str = "MAI2_ASSET_DIR";

/// Runtime directory containing files addressed through macroquad's asset API.
/// Nix sets this to the immutable package resource tree; local builds keep the
/// repository's conventional `assets/` directory.
pub fn asset_dir() -> PathBuf {
    std::env::var_os(ASSET_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

/// Cross-platform asset loading helper.
/// - Android/iOS: packaged assets via `load_file(name)`.
/// - Desktop: try packaged path first, then local fallback paths.
pub async fn load_asset_bytes(name: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = load_file(name).await {
        return Ok(bytes);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let fallbacks = [
            asset_dir().join(name),
            Path::new(env!("CARGO_MANIFEST_DIR")).join(name),
            Path::new(name).to_path_buf(),
            Path::new("assets").join(name),
        ];
        for p in fallbacks {
            if let Ok(bytes) = fs::read(&p) {
                return Ok(bytes);
            }
        }
    }

    Err(format!("asset not found: {name}"))
}

/// Writable data root for both mobile and desktop.
/// Can be overridden with `MAI2_DATA_DIR`.
pub fn data_root_dir() -> PathBuf {
    if let Ok(v) = std::env::var("MAI2_DATA_DIR") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // On mobile we avoid compile-time manifest paths and use runtime writable dir.
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join("mai2chart_data");
        }
        return PathBuf::from(".").join("mai2chart_data");
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }
}

pub fn output_dir() -> Result<PathBuf, String> {
    let primary = data_root_dir().join("output");
    if fs::create_dir_all(&primary).is_ok() {
        return Ok(primary);
    }

    // Fallback to temp dir to avoid hard failures on restricted paths.
    let fallback = std::env::temp_dir().join("mai2chart_output");
    fs::create_dir_all(&fallback).map_err(|e| format!("create output dir: {e}"))?;
    Ok(fallback)
}

pub fn write_output_text(name: &str, content: &str) -> Result<PathBuf, String> {
    let path = output_dir()?.join(name);
    fs::write(&path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

pub fn read_output_text(name: &str) -> Result<String, String> {
    let path = output_dir()?.join(name);
    fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
}

const CACHE_DIR_ENV: &str = "MAI2_CACHE_DIR";

/// Per-user cache root (`~/.lambdadx`): settings, saved layouts and the last
/// opened chart live here, outside the repository. Override with
/// `MAI2_CACHE_DIR`; falls back to a temp directory when there is no home.
pub fn cache_dir() -> Result<PathBuf, String> {
    let root = std::env::var_os(CACHE_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".lambdadx")))
        .unwrap_or_else(|| std::env::temp_dir().join("lambdadx"));
    fs::create_dir_all(&root).map_err(|e| format!("create cache dir {}: {e}", root.display()))?;
    Ok(root)
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").filter(|value| !value.is_empty()).map(PathBuf::from)
    }
}

pub fn write_cache_text(name: &str, content: &str) -> Result<PathBuf, String> {
    let path = cache_dir()?.join(name);
    fs::write(&path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Reads a cached file, falling back to the legacy `output/` copy so settings
/// written before the cache existed are still picked up once.
pub fn read_cache_text(name: &str) -> Result<String, String> {
    let path = cache_dir()?.join(name);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => read_output_text(name),
        Err(error) => Err(format!("read {}: {e}", path.display(), e = error)),
    }
}
