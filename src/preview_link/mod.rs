//! Versioned LAN preview protocol shared by the Editor and Player processes.
pub mod devices;
pub mod discovery;
pub mod qr;
pub mod transport;

use crate::app::types::{ChartDoc, WavPcm};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 3;
pub const PORT: u16 = 47653;
pub const MAX_AUDIO: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub address: String,
    pub token: String,
}

impl Invitation {
    pub fn encode(&self) -> String {
        format!(
            "ldx-preview://{}?v={VERSION}&token={}",
            self.address, self.token
        )
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value
            .trim()
            .strip_prefix("ldx-preview://")
            .ok_or("需要 ldx-preview:// 连接码")?;
        let (address, query) = value.split_once('?').ok_or("连接码缺少配对信息")?;
        let mut version = None;
        let mut token = None;
        for (key, value) in query.split('&').filter_map(|part| part.split_once('=')) {
            match key {
                "v" => version = value.parse::<u32>().ok(),
                "token" => token = Some(value),
                _ => {}
            }
        }
        if version != Some(VERSION) {
            return Err("网络预览协议版本不兼容".into());
        }
        let token = token.ok_or("连接码缺少配对码")?;
        if address.is_empty() || token.len() != 32 || !token.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("连接地址或配对码格式错误".into());
        }
        Ok(Self {
            address: address.into(),
            token: token.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInfo {
    pub id: String,
    pub name: Option<String>,
    pub rate: u32,
    pub channels: u16,
    pub samples: usize,
}
impl AudioInfo {
    pub fn from_pcm(pcm: &WavPcm, name: Option<String>) -> Self {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(pcm.sample_rate.to_le_bytes());
        hash.update(pcm.channels.to_le_bytes());
        for chunk in pcm.samples.chunks(8192) {
            let bytes: Vec<u8> = chunk.iter().flat_map(|s| s.to_le_bytes()).collect();
            hash.update(bytes);
        }
        Self {
            id: format!("{:x}", hash.finalize()),
            name,
            rate: pcm.sample_rate,
            channels: pcm.channels,
            samples: pcm.samples.len(),
        }
    }
    pub fn decode(&self, bytes: &[u8]) -> Result<WavPcm, String> {
        if self.rate == 0
            || self.rate > 384_000
            || !(1..=8).contains(&self.channels)
            || self.samples > MAX_AUDIO / 2
            || self.samples % self.channels as usize != 0
            || bytes.len() != self.samples * 2
        {
            return Err("网络音频长度或格式错误".into());
        }
        let pcm = WavPcm {
            sample_rate: self.rate,
            channels: self.channels,
            samples: std::sync::Arc::new(
                bytes
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect(),
            ),
        };
        if Self::from_pcm(&pcm, None).id != self.id {
            return Err("网络音频校验失败".into());
        }
        Ok(pcm)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Control {
    Pause,
    Resume,
    Restart,
    Seek(f32),
    Autoplay(bool),
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EditorPlayback {
    pub time: f32,
    pub playing: bool,
    pub speed: f32,
}

impl EditorPlayback {
    pub fn valid(self) -> bool {
        self.time.is_finite()
            && self.speed.is_finite()
            && (0.1..=4.0).contains(&self.speed)
    }

    pub fn time_after(self, elapsed: std::time::Duration) -> f32 {
        self.time
            + if self.playing {
                elapsed.as_secs_f32() * self.speed
            } else {
                0.0
            }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Status {
    pub title: String,
    pub time: f32,
    pub duration: f32,
    pub playing: bool,
    pub preparing: bool,
    pub autoplay: bool,
    pub cached_audio: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
// External tagging lets serde_json deserialize floats directly even when a
// workspace dependency enables its arbitrary_precision feature.
#[serde(rename_all = "snake_case")]
pub enum Message {
    Heartbeat,
    Hello {
        version: u32,
        token: String,
        device: Device,
        role: String,
    },
    Welcome {
        version: u32,
        device: Device,
        token: String,
    },
    Load {
        request: u64,
        chart: ChartDoc,
        start: f32,
        autoplay: bool,
        speed: f32,
        audio: Option<AudioInfo>,
        /// Refresh the document while preserving the receiver's playback state.
        #[serde(default)]
        sync: bool,
    },
    Control {
        command: Control,
    },
    FollowEditor {
        playback: Option<EditorPlayback>,
    },
    Status {
        state: Status,
    },
    Ack {
        request: u64,
        error: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug)]
pub struct Packet {
    pub message: Message,
    pub bytes: Vec<u8>,
}
impl From<Message> for Packet {
    fn from(message: Message) -> Self {
        Self {
            message,
            bytes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_playback_messages_round_trip_through_transport() {
        for playback in [
            None,
            Some(EditorPlayback {
                time: 12.5,
                playing: true,
                speed: 0.75,
            }),
            Some(EditorPlayback {
                time: -2.0,
                playing: true,
                speed: 1.0,
            }),
        ] {
            let mut bytes = Vec::new();
            transport::write_packet(&mut bytes, &Message::FollowEditor { playback }.into())
                .unwrap();
            let packet = transport::read_packet(&mut bytes.as_slice()).unwrap();
            let Message::FollowEditor { playback: restored } = packet.message else {
                panic!("wrong packet");
            };
            assert_eq!(restored, playback);
            assert!(restored.is_none_or(EditorPlayback::valid));
        }
    }
    #[test]
    fn invitation_is_versioned_and_roundtrips() {
        let i = Invitation {
            address: "[::1]:47653".into(),
            token: "0123456789abcdef0123456789abcdef".into(),
        };
        assert_eq!(Invitation::parse(&i.encode()).unwrap().address, i.address);
        assert!(Invitation::parse(&i.encode().replace(&format!("v={VERSION}"), "v=0")).is_err());
        assert!(Invitation::parse("http://anything").is_err());
    }
    #[test]
    fn pcm_integrity_is_checked() {
        let pcm = WavPcm {
            sample_rate: 48000,
            channels: 2,
            samples: std::sync::Arc::new(vec![-32768, 32767, 42, -42]),
        };
        let info = AudioInfo::from_pcm(&pcm, None);
        let mut bytes: Vec<_> = pcm.samples.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(info.decode(&bytes).unwrap().samples, pcm.samples);
        bytes[0] ^= 1;
        assert!(info.decode(&bytes).is_err());
        assert!(info.decode(&bytes[..4]).is_err());
    }
}
