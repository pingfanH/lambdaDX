//! Bounded TCP frames: magic + JSON length + binary length, all big endian.
//! Blocking network and large PCM copies stay on workers, never the frame loop.
use super::{Device, Invitation, MAX_AUDIO, Message, Packet, VERSION};
use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
const MAX_JSON: usize = 32 * 1024 * 1024;
const MAGIC: &[u8; 4] = b"LDXP";

#[derive(Debug)]
pub enum Event {
    Connected(Device, String),
    Packet(Packet),
    Disconnected(String),
}
pub struct Endpoint {
    tx: SyncSender<Packet>,
    rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
    listener: Option<Arc<Mutex<Option<TcpListener>>>>,
    pub address: Option<SocketAddr>,
}
impl Endpoint {
    pub fn listen(
        address: &str,
        token: String,
        device: Device,
        role: &'static str,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind(address).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let listener = Arc::new(Mutex::new(Some(listener)));
        let accepting = listener.clone();
        let (tx, outgoing) = mpsc::sync_channel(4);
        let (events, rx) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let accepted = accepting.lock().unwrap().as_ref().map(|l| l.accept());
                let Some(accepted) = accepted else { break };
                match accepted {
                    Ok((mut stream, _)) => {
                        let result = server_handshake(&mut stream, &token, &device, role);
                        match result {
                            Ok(peer) => {
                                // Commands queued for a disconnected peer must not reach its replacement.
                                while outgoing.try_recv().is_ok() {}
                                if events
                                    .try_send(Event::Connected(peer, token.clone()))
                                    .is_err()
                                {
                                    break;
                                }
                                let error = session(stream, &outgoing, &events, &stopped);
                                if events.try_send(Event::Disconnected(error)).is_err() {
                                    break;
                                }
                            }
                            Err(error) => {
                                let _ = events.try_send(Event::Disconnected(error));
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(e) => {
                        let _ = events.try_send(Event::Disconnected(e.to_string()));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            tx,
            rx,
            stop,
            listener: Some(listener),
            address: Some(address),
        })
    }
    pub fn connect(invitation: Invitation, device: Device, role: &'static str) -> Self {
        let (tx, outgoing) = mpsc::sync_channel(4);
        let (events, rx) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        thread::spawn(move || {
            let result = (|| {
                let mut stream = connect(&invitation.address)?;
                configure(&stream)?;
                write_packet(
                    &mut stream,
                    &Message::Hello {
                        version: VERSION,
                        token: invitation.token,
                        device,
                        role: role.into(),
                    }
                    .into(),
                )?;
                let (peer, token) = match read_limited(&mut stream, 4096, 0)?.message {
                    Message::Welcome {
                        version,
                        device,
                        token,
                    } if version == VERSION => (device, token),
                    Message::Error { message } => return Err(message),
                    _ => return Err("网络预览握手不兼容".into()),
                };
                events
                    .try_send(Event::Connected(peer, token))
                    .map_err(|e| e.to_string())?;
                Ok(session(stream, &outgoing, &events, &stopped))
            })();
            let _ = events.try_send(Event::Disconnected(result.unwrap_or_else(|e| e)));
        });
        Self {
            tx,
            rx,
            stop,
            listener: None,
            address: None,
        }
    }
    pub fn send(&self, packet: impl Into<Packet>) -> Result<(), String> {
        self.tx
            .try_send(packet.into())
            .map_err(|e| format!("网络发送队列忙或连接已关闭: {e}"))
    }
    pub fn poll(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Release the listening port now, rather than on the worker's next
        // poll, so switching connection direction can immediately bind again.
        if let Some(listener) = &self.listener {
            listener.lock().unwrap().take();
        }
    }
}
fn connect(address: &str) -> Result<TcpStream, String> {
    let mut error = "连接地址无效".into();
    for addr in address
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .take(4)
    {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
            Ok(s) => return Ok(s),
            Err(e) => error = e.to_string(),
        }
    }
    Err(error)
}
fn configure(stream: &TcpStream) -> Result<(), String> {
    // Accepted sockets may inherit the listener's nonblocking mode (macOS).
    // Workers use blocking framed reads with timeouts, so normalize both ends.
    stream
        .set_nonblocking(false)
        .and_then(|_| stream.set_nodelay(true))
        .and_then(|_| stream.set_read_timeout(Some(Duration::from_secs(5))))
        .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(30))))
        .map_err(|e| e.to_string())
}
fn server_handshake(
    stream: &mut TcpStream,
    token: &str,
    device: &Device,
    role: &str,
) -> Result<Device, String> {
    configure(stream)?;
    // Reject large unauthenticated frames before allocating their bodies.
    let packet = read_limited(stream, 4096, 0)?;
    match packet.message {
        Message::Hello {
            version,
            token: supplied,
            device: peer,
            role: peer_role,
        } if version == VERSION
            && (supplied == token || supplied == super::devices::pairing_pin(token))
            && peer_role != role =>
        {
            write_packet(
                stream,
                &Message::Welcome {
                    version: VERSION,
                    device: device.clone(),
                    token: token.into(),
                }
                .into(),
            )?;
            Ok(peer)
        }
        _ => {
            let error = "配对码错误或协议版本不兼容";
            let _ = write_packet(
                stream,
                &Message::Error {
                    message: error.into(),
                }
                .into(),
            );
            thread::sleep(Duration::from_millis(250));
            Err(error.into())
        }
    }
}
fn session(
    mut stream: TcpStream,
    outgoing: &Receiver<Packet>,
    events: &SyncSender<Event>,
    stop: &AtomicBool,
) -> String {
    // Both endpoints send heartbeats even while the editor is idle. A lost
    // Wi-Fi connection times out, while ordinary idle sessions stay paired.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut reader = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => return e.to_string(),
    };
    let events = events.clone();
    let (done, finished) = mpsc::channel();
    let read_thread = thread::spawn(move || {
        let error = loop {
            match read_packet(&mut reader) {
                Ok(Packet {
                    message: Message::Heartbeat,
                    ..
                }) => {}
                Ok(packet) => {
                    if events.try_send(Event::Packet(packet)).is_err() {
                        break "网络接收队列溢出".into();
                    }
                }
                Err(error) => break error,
            }
        };
        let _ = done.send(error);
    });
    let mut heartbeat = Instant::now();
    let error = loop {
        if stop.load(Ordering::Relaxed) {
            break "已断开".into();
        }
        if let Ok(error) = finished.try_recv() {
            break error;
        }
        if heartbeat.elapsed() >= Duration::from_secs(5) {
            if let Err(error) = write_packet(&mut stream, &Message::Heartbeat.into()) {
                break error;
            }
            heartbeat = Instant::now();
        }
        match outgoing.recv_timeout(Duration::from_millis(50)) {
            Ok(packet) => {
                if let Err(error) = write_packet(&mut stream, &packet) {
                    break error;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break "已断开".into(),
        }
    };
    let _ = stream.shutdown(Shutdown::Both);
    let _ = read_thread.join();
    error
}
pub fn write_packet(writer: &mut impl Write, packet: &Packet) -> Result<(), String> {
    let json = serde_json::to_vec(&packet.message).map_err(|e| e.to_string())?;
    if json.len() > MAX_JSON || packet.bytes.len() > MAX_AUDIO {
        return Err("网络预览数据超过大小限制".into());
    }
    writer
        .write_all(MAGIC)
        .and_then(|_| writer.write_all(&(json.len() as u32).to_be_bytes()))
        .and_then(|_| writer.write_all(&(packet.bytes.len() as u32).to_be_bytes()))
        .and_then(|_| writer.write_all(&json))
        .and_then(|_| writer.write_all(&packet.bytes))
        .map_err(|e| e.to_string())
}
pub fn read_packet(reader: &mut impl Read) -> Result<Packet, String> {
    read_limited(reader, MAX_JSON, MAX_AUDIO)
}
fn read_limited(
    reader: &mut impl Read,
    max_json: usize,
    max_audio: usize,
) -> Result<Packet, String> {
    let mut header = [0; 12];
    reader.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..4] != MAGIC {
        return Err("不是 LambdaDX 网络预览协议".into());
    }
    let json_len = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    let audio_len = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;
    if json_len > max_json || audio_len > max_audio {
        return Err("网络预览数据超过大小限制".into());
    }
    let mut json = vec![0; json_len];
    reader.read_exact(&mut json).map_err(|e| e.to_string())?;
    let message = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    let mut bytes = vec![0; audio_len];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(Packet { message, bytes })
}
pub fn local_address(port: u16) -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| {
            s.connect("192.0.2.1:9").ok()?;
            Some(s.local_addr().ok()?.ip())
        })
        .map(|ip| format!("{ip}:{port}"))
        .unwrap_or_else(|| format!("127.0.0.1:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    fn event(endpoint: &Endpoint) -> Event {
        let start = Instant::now();
        loop {
            if let Some(e) = endpoint.poll() {
                return e;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn separate_endpoints_pair_transfer_and_reconnect() {
        let token = "0123456789abcdef0123456789abcdef";
        let editor = Device {
            id: "editor".into(),
            name: "Desktop".into(),
        };
        let server =
            Endpoint::listen("127.0.0.1:0", token.into(), editor.clone(), "Editor").unwrap();
        let invitation = Invitation {
            address: server.address.unwrap().to_string(),
            token: token.into(),
        };
        for _ in 0..2 {
            let client = Endpoint::connect(
                invitation.clone(),
                Device {
                    id: "phone".into(),
                    name: "Android".into(),
                },
                "Player",
            );
            assert!(matches!(event(&client), Event::Connected(d, _) if d == editor));
            assert!(matches!(event(&server), Event::Connected(d, _) if d.id == "phone"));
            server
                .send(Packet {
                    message: Message::Control {
                        command: super::super::Control::Seek(12.5),
                    },
                    bytes: vec![1, 2, 3],
                })
                .unwrap();
            let received = event(&client);
            assert!(
                matches!(&received, Event::Packet(Packet { message: Message::Control { command: super::super::Control::Seek(t) }, bytes }) if *t == 12.5 && bytes == &[1,2,3]),
                "{received:?}"
            );
            drop(client);
            assert!(matches!(event(&server), Event::Disconnected(_)));
        }
    }
    #[test]
    fn editor_pairs_to_player_by_pin_then_reconnects_with_saved_token() {
        let token = "123456789abcdef0123456789abcdef01";
        let player = Device {
            id: "phone".into(),
            name: "Player".into(),
        };
        let server =
            Endpoint::listen("127.0.0.1:0", token.into(), player.clone(), "Player").unwrap();
        let address = server.address.unwrap().to_string();
        for credential in [super::super::devices::pairing_pin(token), token.into()] {
            let editor = Endpoint::connect(
                Invitation {
                    address: address.clone(),
                    token: credential,
                },
                Device {
                    id: "editor".into(),
                    name: "Editor".into(),
                },
                "Editor",
            );
            assert!(
                matches!(event(&editor), Event::Connected(device, saved) if device == player && saved == token)
            );
            assert!(matches!(event(&server), Event::Connected(_, _)));
            server
                .send(Message::Status {
                    state: super::super::Status {
                        time: 1.25,
                        ..Default::default()
                    },
                })
                .unwrap();
            assert!(
                matches!(event(&editor), Event::Packet(Packet { message: Message::Status { state }, .. }) if state.time == 1.25)
            );
            drop(editor);
            assert!(matches!(event(&server), Event::Disconnected(_)));
        }
    }
    #[test]
    fn listener_can_be_reopened_immediately() {
        let device = Device {
            id: "p".into(),
            name: "Player".into(),
        };
        let server =
            Endpoint::listen("127.0.0.1:0", "token".into(), device.clone(), "Player").unwrap();
        let address = server.address.unwrap();
        drop(server);
        Endpoint::listen(&address.to_string(), "token".into(), device, "Player").unwrap();
    }
    #[test]
    fn idle_pair_stays_connected_and_heartbeats_do_not_reach_ui() {
        let device = Device {
            id: "p".into(),
            name: "Player".into(),
        };
        let server =
            Endpoint::listen("127.0.0.1:0", "token".into(), device.clone(), "Player").unwrap();
        let client = Endpoint::connect(
            Invitation {
                address: server.address.unwrap().to_string(),
                token: "token".into(),
            },
            device,
            "Editor",
        );
        assert!(matches!(event(&client), Event::Connected(_, _)));
        assert!(matches!(event(&server), Event::Connected(_, _)));
        thread::sleep(Duration::from_secs(6));
        let client_event = client.poll();
        let server_event = server.poll();
        assert!(
            client_event.is_none() && server_event.is_none(),
            "client={client_event:?}; server={server_event:?}"
        );
        client
            .send(Message::Control {
                command: super::super::Control::Pause,
            })
            .unwrap();
        assert!(matches!(event(&server), Event::Packet(_)));
    }
    #[test]
    fn bad_pairing_and_oversized_headers_are_rejected() {
        let server = Endpoint::listen(
            "127.0.0.1:0",
            "secret".into(),
            Device {
                id: "e".into(),
                name: "e".into(),
            },
            "Editor",
        )
        .unwrap();
        let client = Endpoint::connect(
            Invitation {
                address: server.address.unwrap().to_string(),
                token: "wrong".into(),
            },
            Device {
                id: "p".into(),
                name: "p".into(),
            },
            "Player",
        );
        assert!(matches!(event(&client), Event::Disconnected(_)));
        let mut bytes = Vec::from(*MAGIC);
        bytes.extend(u32::MAX.to_be_bytes());
        bytes.extend(0u32.to_be_bytes());
        assert!(read_packet(&mut bytes.as_slice()).is_err());
    }
}
