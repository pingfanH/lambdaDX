//! A bounded local transcript. Live runs, model context and credentials are never restored.
use lambdadx_automation::agent::MAX_CONTEXT_BYTES;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

// JSON escaping can take six bytes per byte of transcript text.
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub log: Vec<(String, String)>,
    pub stream: String,
    pub interrupted: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    transcript: Saved,
}

#[derive(Default)]
pub struct Store {
    path: Option<PathBuf>,
}
impl Store {
    pub fn open() -> Result<Self, String> {
        // Unit tests must never read or overwrite a user's transcript.
        if cfg!(test) {
            return Ok(Self { path: None });
        }
        Ok(Self::at(
            crate::app::platform::cache_dir()?.join("editor_agent_history.json"),
        ))
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    pub fn load(&self) -> Result<Saved, String> {
        let Some(path) = &self.path else {
            return Ok(Saved::default());
        };
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Saved::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Agent 对话记录超过 2 MiB".into());
        }
        let file: File = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if file.version != 1 {
            return Err("不支持的 Agent 对话记录版本".into());
        }
        let mut saved = file.transcript;
        if saved.stream.len() > 64 * 1024 {
            return Err("Agent 流式记录超过 64 KiB".into());
        }
        trim(&mut saved.log);
        Ok(saved)
    }

    pub fn save(
        &self,
        log: &[(String, String)],
        stream: &str,
        interrupted: bool,
    ) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let bytes = serde_json::to_vec_pretty(&File {
            version: 1,
            transcript: Saved {
                log: log.to_vec(),
                stream: stream.into(),
                interrupted,
            },
        })
        .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Agent 对话记录超过 2 MiB".into());
        }
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp).map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            drop(file);
            fs::rename(&temp, path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

pub fn trim(log: &mut Vec<(String, String)>) {
    let mut bytes = log.iter().map(|(a, b)| a.len() + b.len()).sum::<usize>();
    let mut remove = 0;
    while log.len() - remove > MAX_ENTRIES || bytes > MAX_CONTEXT_BYTES {
        bytes -= log[remove].0.len() + log[remove].1.len();
        remove += 1;
    }
    log.drain(..remove);
}
