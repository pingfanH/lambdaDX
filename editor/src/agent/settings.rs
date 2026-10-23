use lambdadx_automation::agent::provider::Settings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub connection: Settings,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub insert_at_cursor: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Current(Saved),
    Legacy(Settings),
}

fn path() -> Result<PathBuf, String> {
    crate::app::platform::cache_dir().map(|dir| dir.join("editor_agent.json"))
}
pub fn load() -> Result<Saved, String> {
    if cfg!(test) {
        return Ok(Saved::default());
    }
    let mut saved = load_at(&path()?)?;
    saved.connection = saved.connection.with_environment().map_err(|e| e.message)?;
    Ok(saved)
}

fn load_at(path: &Path) -> Result<Saved, String> {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > 8192 {
            return Err("Agent 配置超过 8 KiB".into());
        }
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.len() > 8192 {
                return Err("Agent 配置超过 8 KiB".into());
            }
            let saved = match serde_json::from_slice(&bytes).map_err(|e| e.to_string())? {
                Stored::Current(saved) => saved,
                Stored::Legacy(connection) => Saved {
                    connection,
                    enabled: false,
                    insert_at_cursor: false,
                },
            };
            saved.connection.validate().map_err(|e| e.message)?;
            Ok(saved)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Saved::default()),
        Err(e) => Err(e.to_string()),
    }
}
/// Settings contains only endpoint/model and an environment variable *name*.
pub fn save(settings: &Settings, enabled: bool, insert_at_cursor: bool) -> Result<(), String> {
    settings.validate().map_err(|e| e.message)?;
    if cfg!(test) {
        return Ok(());
    }
    save_at(&path()?, settings, enabled, insert_at_cursor)
}

fn save_at(path: &Path, settings: &Settings, enabled: bool, insert_at_cursor: bool) -> Result<(), String> {
    settings.validate().map_err(|e| e.message)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&Saved {
            connection: settings.clone(),
            enabled,
            insert_at_cursor,
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Session, history, tests::TestDir};

    #[test]
    fn enabled_and_disabled_survive_restart_with_connection_and_permissions() {
        let dir = TestDir::new();
        let path = dir.file("settings.json");
        let connection = Settings {
            allow_focus: true,
            allow_proposals: true,
            model: "saved-model".into(),
            ..Settings::default()
        };
        for enabled in [true, false, true] {
            save_at(&path, &connection, enabled, enabled).unwrap();
            let saved = load_at(&path).unwrap();
            assert_eq!(saved.connection, connection);
            let session = Session::restore(saved, history::Store::default());
            assert_eq!(session.enabled, enabled);
            assert_eq!(session.insert_at_cursor, enabled);
            assert_eq!(session.applied, connection);
            assert!(session.run.is_none());
        }
    }

    #[test]
    fn legacy_connection_is_preserved_when_enabled_preference_is_first_saved() {
        let dir = TestDir::new();
        let path = dir.file("settings.json");
        let connection = Settings {
            model: "legacy-model".into(),
            allow_proposals: true,
            ..Settings::default()
        };
        std::fs::write(&path, serde_json::to_vec(&connection).unwrap()).unwrap();
        let saved = load_at(&path).unwrap();
        assert!(!saved.enabled);
        assert_eq!(saved.connection, connection);
        assert!(!saved.insert_at_cursor);
        save_at(&path, &saved.connection, true, false).unwrap();
        let saved = load_at(&path).unwrap();
        assert!(saved.enabled);
        assert_eq!(saved.connection, connection);
    }
}
