//! Read dotenv values without changing the process environment or persisting secrets.
use super::provider::{ProviderKind, Settings};
use crate::contract::{ToolError, ToolResult};
use std::{collections::HashMap, path::Path};

struct Dotenv(HashMap<String, String>);

impl Dotenv {
    fn read(start: &Path) -> ToolResult<Self> {
        for dir in start.ancestors() {
            let path = dir.join(".env");
            match std::fs::metadata(&path) {
                Ok(_) => {
                    let entries = dotenvy::from_path_iter(path)
                        .map_err(|_| ToolError::invalid("Cannot read Agent .env file"))?;
                    let mut values = HashMap::new();
                    for entry in entries {
                        let (key, value) = entry
                            .map_err(|_| ToolError::invalid("Invalid Agent .env file syntax"))?;
                        values.entry(key).or_insert(value);
                    }
                    return Ok(Self(values));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(ToolError::invalid("Cannot read Agent .env file")),
            }
        }
        Ok(Self(HashMap::new()))
    }

    fn value(&self, name: &str, process: &impl Fn(&str) -> Option<String>) -> Option<String> {
        process(name)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                self.0
                    .get(name)
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
            })
    }
}

fn read() -> ToolResult<Dotenv> {
    let cwd =
        std::env::current_dir().map_err(|_| ToolError::invalid("Cannot locate Agent .env file"))?;
    Dotenv::read(&cwd)
}

fn apply(
    mut settings: Settings,
    values: &Dotenv,
    process: &impl Fn(&str) -> Option<String>,
) -> ToolResult<Settings> {
    // LDX_AGENT_PROVIDER historically contains the OpenAI-compatible endpoint URL.
    if let Some(endpoint) = values.value("LDX_AGENT_PROVIDER", process) {
        settings.base_url = endpoint;
        settings.provider = ProviderKind::OpenAICompatible;
    }
    if let Some(model) = values.value("LDX_AGENT_MODEL", process) {
        settings.model = model;
    }
    if values.value("LDX_AGENT_SECRET", process).is_some() {
        settings.credential_env = "LDX_AGENT_SECRET".into();
    }
    settings.validate()?;
    Ok(settings)
}

pub(super) fn settings(settings: Settings) -> ToolResult<Settings> {
    apply(settings, &read()?, &|name| std::env::var(name).ok())
}

pub(super) fn credential(name: &str) -> ToolResult<String> {
    // A supplied process credential does not depend on an unrelated .env parsing.
    if let Ok(value) = std::env::var(name) {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    read()?.value(name, &|_| None).ok_or_else(|| {
        ToolError::invalid("Configured credential environment variable is unavailable in the process environment and .env")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenv_supplies_model_endpoint_and_credential_name_without_persisting_secret() {
        let values = Dotenv(HashMap::from([
            ("LDX_AGENT_PROVIDER".into(), "https://example.com/v1".into()),
            ("LDX_AGENT_MODEL".into(), "deepseek/example".into()),
            ("LDX_AGENT_SECRET".into(), "fixture-secret".into()),
        ]));
        let configured = apply(Settings::default(), &values, &|_| None).unwrap();
        assert_eq!(configured.provider, ProviderKind::OpenAICompatible);
        assert_eq!(configured.base_url, "https://example.com/v1");
        assert_eq!(configured.model, "deepseek/example");
        assert_eq!(configured.credential_env, "LDX_AGENT_SECRET");
        assert!(
            !serde_json::to_string(&configured)
                .unwrap()
                .contains("fixture-secret")
        );
        assert_eq!(
            values
                .value(&configured.credential_env, &|_| None)
                .as_deref(),
            Some("fixture-secret")
        );
    }

    #[test]
    fn process_overrides_dotenv_and_saved_permissions_survive() {
        let values = Dotenv(HashMap::from([(
            "LDX_AGENT_MODEL".into(),
            "file-model".into(),
        )]));
        let saved = Settings {
            allow_focus: true,
            allow_proposals: true,
            ..Settings::default()
        };
        let configured = apply(saved, &values, &|name| {
            (name == "LDX_AGENT_MODEL").then(|| "process-model".into())
        })
        .unwrap();
        assert_eq!(configured.model, "process-model");
        assert!(configured.allow_focus && configured.allow_proposals);
        assert_eq!(configured.credential_env, "OPENAI_API_KEY");
    }

    #[test]
    fn nearest_dotenv_supports_quotes_export_and_named_credentials() {
        let dir = std::env::temp_dir().join(format!("lambdadx-env-{}", std::process::id()));
        let child = dir.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(dir.join(".env"), "export LDX_AGENT_MODEL='quoted model'\nLDX_AGENT_SECRET=fixture-secret # comment\nCUSTOM_KEY=custom-fixture\n").unwrap();
        let values = Dotenv::read(&child).unwrap();
        assert_eq!(
            values.value("LDX_AGENT_MODEL", &|_| None).as_deref(),
            Some("quoted model")
        );
        assert_eq!(
            values.value("LDX_AGENT_SECRET", &|_| None).as_deref(),
            Some("fixture-secret")
        );
        assert_eq!(
            values.value("CUSTOM_KEY", &|_| None).as_deref(),
            Some("custom-fixture")
        );
        std::fs::write(child.join(".env"), "LDX_AGENT_MODEL=nearest\n").unwrap();
        let values = Dotenv::read(&child).unwrap();
        assert_eq!(
            values.value("LDX_AGENT_MODEL", &|_| None).as_deref(),
            Some("nearest")
        );
        assert!(values.value("LDX_AGENT_SECRET", &|_| None).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
