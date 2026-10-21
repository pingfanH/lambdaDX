use lambdadx_automation::agent::provider::Settings;
use std::path::PathBuf;

fn path() -> Result<PathBuf, String> {
    crate::app::platform::cache_dir().map(|dir| dir.join("editor_agent.json"))
}
pub fn load() -> Result<Settings, String> {
    if cfg!(test) {
        return Ok(Settings::default());
    }
    let path = path()?;
    if let Ok(metadata) = std::fs::metadata(&path) {
        if metadata.len() > 8192 {
            return Err("Agent 配置超过 8 KiB".into());
        }
    }
    match std::fs::read(&path) {
        Ok(bytes) => {
            if bytes.len() > 8192 {
                return Err("Agent 配置超过 8 KiB".into());
            }
            let settings: Settings = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            settings.validate().map_err(|e| e.message)?;
            Ok(settings)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.to_string()),
    }
}
/// Settings contains only endpoint/model and an environment variable *name*.
pub fn save(settings: &Settings) -> Result<(), String> {
    settings.validate().map_err(|e| e.message)?;
    if cfg!(test) {
        return Ok(());
    }
    let path = path()?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        e.to_string()
    })
}
