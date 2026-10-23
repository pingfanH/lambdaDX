//! UDP discovery advertises identity and TCP port only, never pairing secrets.
use super::{Device, VERSION};
use serde::{Deserialize, Serialize};
use std::{
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
pub const PORT: u16 = 47654;
const QUERY: &[u8] = b"LDX_PREVIEW_DISCOVER_V1";
#[derive(Debug, Clone)]
pub struct Found {
    pub device: Device,
    pub address: String,
}
#[derive(Serialize, Deserialize)]
struct Advert {
    protocol: String,
    version: u32,
    device: Device,
    port: u16,
}
pub struct Advertiser {
    stop: Arc<AtomicBool>,
    socket: Arc<Mutex<Option<UdpSocket>>>,
}
impl Advertiser {
    pub fn start(device: Device, port: u16) -> Result<Self, String> {
        let socket = UdpSocket::bind(("0.0.0.0", PORT)).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&Advert {
            protocol: "LDXP".into(),
            version: VERSION,
            device,
            port,
        })
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let socket = Arc::new(Mutex::new(Some(socket)));
        let responding = socket.clone();
        thread::spawn(move || {
            let mut buffer = [0; 1024];
            while !stopped.load(Ordering::Relaxed) {
                {
                    let guard = responding.lock().unwrap();
                    let Some(socket) = guard.as_ref() else { break };
                    if let Ok((len, addr)) = socket.recv_from(&mut buffer) {
                        if &buffer[..len] == QUERY {
                            let _ = socket.send_to(&bytes, addr);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        Ok(Self { stop, socket })
    }
}
impl Drop for Advertiser {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.socket.lock().unwrap().take();
    }
}
pub fn search() -> mpsc::Receiver<Result<Vec<Found>, String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(search_at(
            &[
                SocketAddr::from(([255, 255, 255, 255], PORT)),
                SocketAddr::from(([127, 0, 0, 1], PORT)),
            ],
            Duration::from_secs(2),
        ));
    });
    rx
}
fn search_at(targets: &[SocketAddr], duration: Duration) -> Result<Vec<Found>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(150)))
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut next = start;
    let mut found: Vec<Found> = Vec::new();
    let mut buffer = [0; 4096];
    while start.elapsed() < duration {
        if Instant::now() >= next {
            for target in targets {
                let _ = socket.send_to(QUERY, target);
            }
            next = Instant::now() + Duration::from_millis(500);
        }
        if let Ok((len, address)) = socket.recv_from(&mut buffer) {
            if let Ok(advert) = serde_json::from_slice::<Advert>(&buffer[..len]) {
                if advert.protocol == "LDXP"
                    && advert.version == VERSION
                    && advert.port != 0
                    && !found.iter().any(|f| f.device.id == advert.device.id)
                {
                    found.push(Found {
                        device: advert.device,
                        address: SocketAddr::new(address.ip(), advert.port).to_string(),
                    });
                }
            }
        }
    }
    Ok(found)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_returns_sender_address_without_credentials() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target = socket.local_addr().unwrap();
        let responder = thread::spawn(move || {
            let mut b = [0; 1024];
            let (n, addr) = socket.recv_from(&mut b).unwrap();
            assert_eq!(&b[..n], QUERY);
            let bytes = serde_json::to_vec(&Advert {
                protocol: "LDXP".into(),
                version: VERSION,
                device: Device {
                    id: "phone".into(),
                    name: "Player".into(),
                },
                port: 47653,
            })
            .unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("token"));
            socket.send_to(&bytes, addr).unwrap();
        });
        let found = search_at(&[target], Duration::from_millis(300)).unwrap();
        responder.join().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].address, "127.0.0.1:47653");
    }
}
