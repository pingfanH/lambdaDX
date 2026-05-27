use macroquad::file::load_file;
use std::fs;
use std::path::{Path, PathBuf};

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
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join(name),
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
