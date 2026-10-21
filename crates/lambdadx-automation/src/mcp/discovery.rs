//! Private per-user discovery. Endpoints are constructed from a port, never a URL.
use serde::{Deserialize, Serialize};
use std::{fs, io::{self, Write}, path::{Path, PathBuf}};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record { pub instance_id: String, pub port: u16, pub token: String, pub pid: u32 }
impl Record { pub fn endpoint(&self) -> String { format!("http://127.0.0.1:{}/mcp", self.port) } }
pub fn directory() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?;
    Ok(PathBuf::from(home).join(".local/state/lambdadx/mcp"))
}
pub fn valid_id(id: &str) -> bool { !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') }
fn private(path: &Path, directory: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || metadata.is_dir() != directory { return Err(io::Error::other("Invalid discovery entry")); }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        let home = fs::metadata(std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?)?;
        if metadata.uid() != home.uid() || metadata.mode() & 0o077 != 0 { return Err(io::Error::other("Discovery permissions must be private to this user")); }
    }
    Ok(())
}
pub struct Registration { path: PathBuf }
impl Registration {
    pub fn create(record: &Record) -> io::Result<Self> { Self::in_directory(&directory()?, record) }
    pub fn in_directory(dir: &Path, record: &Record) -> io::Result<Self> {
        if !valid_id(&record.instance_id) { return Err(io::Error::other("Invalid instance ID")); }
        #[cfg(not(unix))] return Err(io::Error::other("Private MCP discovery currently requires Unix permissions"));
        #[cfg(unix)] {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
            private(dir, true)?;
            let path = dir.join(format!("{}.json", record.instance_id));
            let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
            if let Err(error) = file.write_all(&serde_json::to_vec(record)?).and_then(|()| file.sync_all()) { let _ = fs::remove_file(&path); return Err(error); }
            Ok(Self { path })
        }
    }
}
impl Drop for Registration { fn drop(&mut self) { let _ = fs::remove_file(&self.path); } }
pub fn read(id: &str) -> io::Result<Record> { read_in(&directory()?, id) }
pub fn read_in(dir: &Path, id: &str) -> io::Result<Record> {
    if !valid_id(id) { return Err(io::Error::other("Invalid instance ID")); }
    private(dir, true)?;
    let path = dir.join(format!("{id}.json")); private(&path, false)?;
    if fs::metadata(&path)?.len() > 4096 { return Err(io::Error::other("Oversized discovery record")); }
    let record: Record = serde_json::from_slice(&fs::read(path)?)?;
    if record.instance_id != id || record.port == 0 || record.token.len() != 64 || !record.token.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(io::Error::other("Invalid discovery record")); }
    Ok(record)
}
