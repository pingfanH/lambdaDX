//! Nonblocking picker requests; native UI stays on the main thread, file IO off it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[cfg(target_os = "macos")]
mod macos;

const HISTORY_FILE: &str = "editor_file_dialogs.json";

pub type DialogResult = Result<Option<PathBuf>, String>;

/// Whether [`Dialogs::start`] can present a real native picker. Linux and
/// Windows use the editor's in-app path prompt instead.
pub const fn native_picker_available() -> bool {
    cfg!(target_os = "macos")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Open,
    Save { save_as: bool, native: bool },
    Background,
}

#[derive(Default)]
pub struct Dialogs {
    pending: Option<(Request, Receiver<DialogResult>)>,
    worker: Option<Sender<Job>>,
}

impl Dialogs {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn warm_up(&mut self) {
        if let Err(error) = self.ensure_worker() {
            eprintln!("[file-dialog] {error}");
        }
        #[cfg(target_os = "macos")]
        macos::warm_up();
    }

    pub fn start(&mut self, request: Request, hint: Option<PathBuf>) -> Result<(), String> {
        if self.busy() {
            return Err("A file dialog is already open".into());
        }
        self.ensure_worker()?;
        let (reply, rx) = mpsc::channel();
        if self
            .worker
            .as_ref()
            .unwrap()
            .send(Job {
                request,
                hint,
                reply,
            })
            .is_err()
        {
            self.worker = None;
            return Err("File dialog worker exited; try again".into());
        }
        self.pending = Some((request, rx));
        Ok(())
    }

    fn ensure_worker(&mut self) -> Result<(), String> {
        if self.worker.is_none() {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("editor-file-dialog".into())
                .spawn(move || run_worker(rx))
                .map_err(|error| format!("Start file dialog worker: {error}"))?;
            self.worker = Some(tx);
        }
        Ok(())
    }

    #[cfg(test)]
    fn start_with(
        &mut self,
        request: Request,
        pick: impl FnOnce() -> DialogResult + Send + 'static,
    ) -> Result<(), String> {
        if self.busy() {
            return Err("A file dialog is already open".into());
        }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("editor-file-dialog".into())
            .spawn(move || {
                let _ = tx.send(pick());
            })
            .map_err(|error| format!("Start file dialog: {error}"))?;
        self.pending = Some((request, rx));
        Ok(())
    }

    pub fn poll(&mut self) -> Option<(Request, DialogResult)> {
        let (request, rx) = self.pending.as_ref()?;
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("File dialog worker exited".into()),
        };
        let request = *request;
        self.pending = None;
        Some((request, result))
    }
}

struct Job {
    request: Request,
    hint: Option<PathBuf>,
    reply: Sender<DialogResult>,
}

struct Selection {
    path: Option<PathBuf>,
    directory: Option<PathBuf>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct History {
    open: Option<PathBuf>,
    save: Option<PathBuf>,
    background: Option<PathBuf>,
}

impl History {
    fn slot(&mut self, request: Request) -> &mut Option<PathBuf> {
        match request {
            Request::Open => &mut self.open,
            Request::Save { .. } => &mut self.save,
            Request::Background => &mut self.background,
        }
    }

    fn remember(&mut self, request: Request, selection: &Selection) -> bool {
        if selection.path.is_none() || selection.directory.is_none() {
            return false;
        }
        let slot = self.slot(request);
        if *slot == selection.directory {
            return false;
        }
        *slot = selection.directory.clone();
        true
    }

    fn directory(&mut self, request: Request, hint: Option<&Path>) -> PathBuf {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let cwd = std::env::current_dir().ok();
        // A deleted remembered directory is not a file hint. Do not silently
        // replace it with its parent ahead of the current chart's directory.
        self.slot(request)
            .as_deref()
            .filter(|path| path.is_dir())
            .map(Path::to_path_buf)
            .or_else(|| hint.and_then(existing_directory))
            .or_else(|| home.as_deref().and_then(existing_directory))
            .or_else(|| cwd.as_deref().and_then(existing_directory))
            .unwrap_or_else(std::env::temp_dir)
    }
}

fn existing_directory(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let directory = if path.is_dir() { path } else { path.parent()? };
    directory.canonicalize().ok().filter(|path| path.is_dir())
}

fn run_worker(jobs: Receiver<Job>) {
    use crate::app::platform;
    let mut history: History = platform::read_output_text(HISTORY_FILE)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    for job in jobs {
        let directory = history.directory(job.request, job.hint.as_deref());
        let result = pick(job.request, directory);
        let changed = result
            .as_ref()
            .is_ok_and(|selection| history.remember(job.request, selection));
        let result = result.and_then(|selection| normalize_path(job.request, selection.path));
        let _ = job.reply.send(result);
        if changed {
            // One worker owns history and writes in order; rapid selections
            // cannot let an older write replace the latest directory.
            let saved = platform::output_dir().and_then(|dir| {
                let text = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;
                super::atomic_write(&dir.join(HISTORY_FILE), &text)
            });
            if let Err(error) = saved {
                eprintln!("[file-dialog] Save directories: {error}");
            }
        }
    }
}

fn pick(request: Request, directory: PathBuf) -> Result<Selection, String> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = mpsc::channel();
        macos::present(request, directory, tx);
        rx.recv()
            .map_err(|_| "Native file dialog closed without a result".to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (request, directory);
        Err("Native file dialog is unavailable on this platform".into())
    }
}

fn normalize_path(request: Request, path: Option<PathBuf>) -> DialogResult {
    path.map(|mut path| {
        if let Request::Save { save_as, native } = request {
            if path.extension().is_none() {
                path.set_extension(if save_as && native { "json" } else { "txt" });
            }
            if !path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("txt") || (save_as && ext.eq_ignore_ascii_case("json"))
            }) {
                return Err(if save_as {
                    "Save As requires .txt or .json"
                } else {
                    "Simai export requires a .txt filename"
                }
                .into());
            }
        }
        Ok(path)
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion(dialogs: &mut Dialogs) -> (Request, DialogResult) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(result) = dialogs.poll() {
                return result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "dialog worker did not complete"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn pending_picker_does_not_block_frames_or_allow_another_picker() {
        let mut dialogs = Dialogs::default();
        let (release, wait) = mpsc::channel();
        dialogs
            .start_with(Request::Open, move || {
                wait.recv().unwrap();
                Ok(Some(PathBuf::from("/tmp/chart/maidata.txt")))
            })
            .unwrap();
        // The worker cannot finish until after the simulated frames have run.
        for _ in 0..10 {
            assert!(dialogs.poll().is_none());
            assert!(dialogs.busy());
        }
        assert!(
            dialogs
                .start_with(Request::Open, || panic!("duplicate picker"))
                .is_err()
        );
        release.send(()).unwrap();
        let (request, result) = completion(&mut dialogs);
        assert_eq!(request, Request::Open);
        assert_eq!(
            result.unwrap(),
            Some(PathBuf::from("/tmp/chart/maidata.txt"))
        );
        assert!(!dialogs.busy());
        assert!(dialogs.poll().is_none());
    }

    #[test]
    fn cancellation_and_failure_release_the_pending_request() {
        let mut dialogs = Dialogs::default();
        let request = Request::Save {
            save_as: true,
            native: false,
        };
        dialogs.start_with(request, || Ok(None)).unwrap();
        assert_eq!(completion(&mut dialogs), (request, Ok(None)));
        assert!(!dialogs.busy());
        dialogs
            .start_with(Request::Open, || Err("picker unavailable".into()))
            .unwrap();
        assert_eq!(completion(&mut dialogs).1, Err("picker unavailable".into()));
        assert!(!dialogs.busy());
    }

    #[test]
    fn history_survives_reload_and_keeps_picker_purposes_separate() {
        let mut history = History::default();
        let save = Request::Save {
            save_as: true,
            native: true,
        };
        for (request, directory) in [
            (Request::Open, "/charts"),
            (save, "/exports"),
            (Request::Background, "/pictures"),
        ] {
            assert!(history.remember(
                request,
                &Selection {
                    path: Some(PathBuf::from(directory).join("selected")),
                    directory: Some(PathBuf::from(directory)),
                }
            ));
        }
        assert!(!history.remember(
            Request::Open,
            &Selection {
                path: None,
                directory: None
            }
        ));
        let mut restored: History =
            serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
        assert_eq!(
            restored.slot(Request::Open).as_deref(),
            Some(Path::new("/charts"))
        );
        assert_eq!(
            restored
                .slot(Request::Save {
                    save_as: false,
                    native: false
                })
                .as_deref(),
            Some(Path::new("/exports"))
        );
        assert_eq!(
            restored.slot(Request::Background).as_deref(),
            Some(Path::new("/pictures"))
        );
        let fallback = std::env::temp_dir().canonicalize().unwrap();
        restored.open = Some(fallback.join(format!("missing-dialog-dir-{}", std::process::id())));
        assert_eq!(restored.directory(Request::Open, Some(&fallback)), fallback);
        assert!(existing_directory(Path::new("")).is_none());
    }

    #[test]
    fn save_paths_keep_format_validation_after_native_selection() {
        let save = Request::Save {
            save_as: true,
            native: true,
        };
        let export = Request::Save {
            save_as: false,
            native: true,
        };
        assert_eq!(
            normalize_path(save, Some("chart".into())).unwrap(),
            Some("chart.json".into())
        );
        assert_eq!(
            normalize_path(export, Some("chart".into())).unwrap(),
            Some("chart.txt".into())
        );
        assert!(normalize_path(save, Some("chart.TXT".into())).is_ok());
        assert!(normalize_path(export, Some("chart.json".into())).is_err());
        assert!(normalize_path(save, Some("chart.png".into())).is_err());
        assert_eq!(normalize_path(export, None).unwrap(), None);
    }
}
