//! Persistent identity and remembered pairing records (never chart paths).
use super::{Device, Invitation};
use crate::app::platform;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remembered {
    pub device: Device,
    pub invitation: Option<Invitation>,
}
#[derive(Serialize, Deserialize)]
struct Saved {
    device: Device,
    token: String,
    peers: Vec<Remembered>,
}

pub struct Devices {
    file: String,
    saved: Saved,
}
impl Devices {
    pub fn load(role: &str) -> Result<Self, String> {
        let file = format!("preview-link-{role}.json");
        let saved = platform::read_cache_text(&file)
            .ok()
            .and_then(|s| serde_json::from_str::<Saved>(&s).ok());
        let saved = match saved {
            Some(saved) => saved,
            None => Saved {
                device: Device {
                    id: random_id()?,
                    name: format!("LambdaDX {role} ({})", std::env::consts::OS),
                },
                token: random_id()?,
                peers: Vec::new(),
            },
        };
        let store = Self { file, saved };
        store.save()?;
        Ok(store)
    }
    pub fn identity(&self) -> Device {
        self.saved.device.clone()
    }
    pub fn token(&self) -> &str {
        &self.saved.token
    }
    pub fn pin(&self) -> String {
        pairing_pin(self.token())
    }
    pub fn peers(&self) -> Vec<Remembered> {
        self.saved.peers.clone()
    }
    pub fn remember(
        &mut self,
        device: Device,
        invitation: Option<Invitation>,
    ) -> Result<(), String> {
        let invitation = invitation.or_else(|| {
            self.saved
                .peers
                .iter()
                .find(|p| p.device.id == device.id)
                .and_then(|p| p.invitation.clone())
        });
        self.saved.peers.retain(|p| p.device.id != device.id);
        self.saved
            .peers
            .insert(0, Remembered { device, invitation });
        self.saved.peers.truncate(16);
        self.save()
    }
    pub fn forget(&mut self, id: &str) -> Result<(), String> {
        self.saved.peers.retain(|p| p.device.id != id);
        self.save()
    }
    fn save(&self) -> Result<(), String> {
        let path = platform::cache_dir()?.join(&self.file);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        options
            .open(path)
            .and_then(|mut f| {
                f.write_all(
                    serde_json::to_string_pretty(&self.saved)
                        .unwrap()
                        .as_bytes(),
                )
            })
            .map_err(|e| e.to_string())
    }
}
fn random_id() -> Result<String, String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn pairing_pin(token: &str) -> String {
    format!(
        "{:06}",
        u32::from_str_radix(&token[..token.len().min(8)], 16).unwrap_or(0) % 1_000_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembered_connection_survives_restart_and_incoming_reconnect() {
        let file = std::env::temp_dir().join(format!("ldx-pairing-{}.json", random_id().unwrap()));
        let mut devices = Devices {
            file: file.to_string_lossy().into_owned(),
            saved: Saved {
                device: Device {
                    id: "editor".into(),
                    name: "Editor".into(),
                },
                token: random_id().unwrap(),
                peers: Vec::new(),
            },
        };
        let peer = Device {
            id: "phone".into(),
            name: "Player".into(),
        };
        let invitation = Invitation {
            address: "192.168.1.10:47653".into(),
            token: random_id().unwrap(),
        };
        devices
            .remember(peer.clone(), Some(invitation.clone()))
            .unwrap();
        devices.saved = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        devices.remember(peer, None).unwrap();
        let peers = devices.peers();
        assert_eq!(peers.len(), 1);
        let saved = peers[0].invitation.as_ref().unwrap();
        assert_eq!(saved.address, invitation.address);
        assert_eq!(saved.token, invitation.token);
        devices.forget("phone").unwrap();
        let saved: Saved = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(saved.peers.is_empty());
        std::fs::remove_file(file).unwrap();
    }
}
