//! Player side of LAN preview: pair, receive, prepare off-thread, then play.
use super::state::{Page, PlayerUiApp};
use crate::app::{
    self,
    types::{ChartDoc, Mode, WavPcm},
};
use crate::player::{
    engine::JudgeEngine,
    state::{PadPreviewState, PreparedEditedEngine},
};
use crate::preview_link::{
    self as link, AudioInfo, Control, EditorPlayback, Invitation, Message, Packet, Status,
    devices::{Devices, Remembered},
    discovery::Advertiser,
    transport::{Endpoint, Event},
};
use std::sync::mpsc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Action {
    Listen,
    Connect(Invitation),
    Disconnect,
    Forget(String),
    Scan,
    Manual,
}
#[derive(Default)]
pub struct UiState {
    pub action: Option<Action>,
    pub connected: bool,
    pub listening: bool,
    pub peer: String,
    pub address: String,
    pub pin: String,
    pub status: String,
    pub devices: Vec<Remembered>,
    pub connection_dialog: bool,
}
#[derive(Clone, Copy)]
struct Playback {
    start: f32,
    playing: bool,
    autoplay: bool,
    speed: f32,
    restart: f32,
    page: Page,
}
impl Playback {
    fn for_load(app: &PlayerUiApp, sync: bool, start: f32, autoplay: bool, speed: f32) -> Self {
        if sync && app.remote_preview {
            Self {
                start: app.pad.timeline_time(),
                playing: app.pad.mode == Mode::Playing,
                autoplay: app.pad.autoplay,
                speed: app.pad.play_speed,
                restart: app.gameplay_start,
                page: app.page,
            }
        } else {
            Self {
                start,
                playing: !sync,
                autoplay,
                speed,
                restart: start,
                page: Page::Gameplay,
            }
        }
    }
}
struct Prepared {
    chart: ChartDoc,
    engine: PreparedEditedEngine,
    audio: Option<(AudioInfo, WavPcm)>,
    playback: Playback,
}
struct Completion {
    generation: u64,
    request: u64,
    result: Result<Prepared, String>,
}
#[derive(Clone, Copy)]
struct Following {
    playback: EditorPlayback,
    received: Instant,
}
#[derive(Default)]
pub struct Controller {
    devices: Option<Devices>,
    endpoint: Option<Endpoint>,
    advertise: Option<Advertiser>,
    invitation: Option<Invitation>,
    pending: Option<mpsc::Receiver<Completion>>,
    generation: u64,
    audio: Option<(AudioInfo, WavPcm)>,
    last_status: f64,
    scan: Option<mpsc::Receiver<Result<Invitation, String>>>,
    following: Option<Following>,
}
impl Controller {
    pub fn update(&mut self, app: &mut PlayerUiApp, now: f64) {
        if self.devices.is_none() {
            match Devices::load("Player") {
                Ok(devices) => {
                    app.link_ui.devices = devices.peers();
                    app.link_ui.pin = devices.pin();
                    self.devices = Some(devices);
                }
                Err(e) => {
                    app.link_ui.status = e;
                    return;
                }
            }
        }
        #[cfg(target_os = "android")]
        if self.scan.is_none() {
            match super::android::poll_link_input() {
                Ok(Some(super::android::LinkInput::Text(value))) => {
                    match Invitation::parse(&value) {
                        Ok(i) => app.link_ui.action = Some(Action::Connect(i)),
                        Err(e) => app.link_ui.status = e,
                    }
                }
                Ok(Some(super::android::LinkInput::Frame(w, h, gray))) => {
                    let (tx, rx) = mpsc::channel();
                    self.scan = Some(rx);
                    std::thread::spawn(move || {
                        let _ = tx.send(link::qr::decode(w, h, &gray));
                    });
                }
                Ok(None) => {}
                Err(e) => app.link_ui.status = e,
            }
        }
        if let Some(result) = self.scan.as_ref().and_then(|r| r.try_recv().ok()) {
            self.scan = None;
            match result {
                Ok(i) => {
                    #[cfg(target_os = "android")]
                    let _ = super::android::close_link_scanner();
                    app.link_ui.action = Some(Action::Connect(i));
                }
                Err(e) => app.link_ui.status = e,
            }
        }
        if let Some(action) = app.link_ui.action.take() {
            if let Err(e) = self.action(app, action) {
                app.link_ui.status = e;
            }
        }
        for _ in 0..16 {
            let Some(event) = self.endpoint.as_ref().and_then(|e| e.poll()) else {
                break;
            };
            match event {
                Event::Connected(peer, token) => {
                    self.following = None;
                    app.link_ui.connected = true;
                    app.link_ui.peer = peer.name.clone();
                    app.link_ui.status = format!("已连接 {}", peer.name);
                    self.last_status = now - 1.0;
                    let invitation = self.invitation.clone().map(|mut i| {
                        i.token = token;
                        i
                    });
                    if let Err(e) = self.devices.as_mut().unwrap().remember(peer, invitation) {
                        app.link_ui.status = e;
                    }
                    app.link_ui.devices = self.devices.as_ref().unwrap().peers();
                }
                Event::Disconnected(error) => {
                    self.following = None;
                    app.link_ui.connected = false;
                    app.link_ui.status = format!("连接断开: {error}");
                    self.generation += 1;
                    self.pending = None;
                    app.preparing = false;
                    if app.remote_preview {
                        app.pause();
                    }
                }
                Event::Packet(packet) => self.packet(app, packet),
            }
        }
        let completed = self.pending.as_ref().map(|r| r.try_recv());
        if let Some(Ok(done)) = completed {
            self.pending = None;
            if done.generation == self.generation && app.link_ui.connected {
                app.preparing = false;
                let result = done.result.and_then(|prepared| self.install(app, prepared));
                let error = result.err();
                if let Some(e) = &error {
                    app.error = Some(e.clone());
                    app.pause();
                }
                self.send(Message::Ack {
                    request: done.request,
                    error,
                });
                self.last_status = now - 1.0;
            }
        } else if matches!(completed, Some(Err(mpsc::TryRecvError::Disconnected))) {
            self.pending = None;
            app.preparing = false;
            app.error = Some("网络谱面准备线程已退出，请重新发送".into());
            app.pause();
            self.send(Message::Error {
                message: app.error.clone().unwrap(),
            });
        }
        if app.link_ui.connected && now - self.last_status >= 0.25 {
            self.last_status = now;
            self.send(Message::Status {
                state: Status {
                    title: app.pad.chart.title.clone(),
                    time: app.pad.timeline_time(),
                    duration: app.pad.song_duration(),
                    playing: app.pad.mode == Mode::Playing,
                    preparing: app.preparing,
                    autoplay: app.pad.autoplay,
                    cached_audio: self.audio.as_ref().map(|(i, _)| i.id.clone()),
                    error: app.error.clone(),
                },
            });
        }
    }
    fn action(&mut self, app: &mut PlayerUiApp, action: Action) -> Result<(), String> {
        match action {
            Action::Listen => {
                if app.link_ui.listening {
                    return Ok(());
                }
                self.disconnect(app);
                let devices = self.devices.as_ref().unwrap();
                let endpoint = Endpoint::listen(
                    &format!("0.0.0.0:{}", link::PORT),
                    devices.token().into(),
                    devices.identity(),
                    "Player",
                )?;
                let port = endpoint.address.unwrap().port();
                app.link_ui.address = link::transport::local_address(port);
                self.advertise = Advertiser::start(devices.identity(), port).ok();
                app.link_ui.listening = true;
                app.link_ui.status = if self.advertise.is_some() {
                    "等待 Editor 搜索或手动连接".into()
                } else {
                    "监听已开启，设备发现端口被占用；可手动连接".into()
                };
                self.endpoint = Some(endpoint);
            }
            Action::Connect(invitation) => {
                self.disconnect(app);
                self.endpoint = Some(Endpoint::connect(
                    invitation.clone(),
                    self.devices.as_ref().unwrap().identity(),
                    "Player",
                ));
                app.link_ui.status = format!("连接 {}…", invitation.address);
                self.invitation = Some(invitation);
            }
            Action::Disconnect => self.disconnect(app),
            Action::Forget(id) => {
                self.devices.as_mut().unwrap().forget(&id)?;
                app.link_ui.devices = self.devices.as_ref().unwrap().peers();
            }
            Action::Scan => {
                #[cfg(target_os = "android")]
                super::android::open_link_scanner()?;
                #[cfg(not(target_os = "android"))]
                {
                    app.link_ui.connection_dialog = true;
                    app.link_ui.status = "在下方连接面板导入二维码图片或粘贴连接码".into();
                }
            }
            Action::Manual => {
                #[cfg(target_os = "android")]
                super::android::open_link_manual()?;
                #[cfg(not(target_os = "android"))]
                {
                    app.link_ui.connection_dialog = true;
                }
            }
        }
        Ok(())
    }
    fn disconnect(&mut self, app: &mut PlayerUiApp) {
        self.following = None;
        self.endpoint = None;
        self.advertise = None;
        self.invitation = None;
        self.pending = None;
        self.generation += 1;
        app.link_ui.connected = false;
        app.link_ui.listening = false;
        if app.remote_preview {
            app.preparing = false;
            app.pause();
        }
        app.link_ui.status = "已断开网络连接".into();
    }
    fn send(&self, message: Message) {
        if let Some(e) = &self.endpoint {
            let _ = e.send(message);
        }
    }
    fn packet(&mut self, app: &mut PlayerUiApp, packet: Packet) {
        match packet.message {
            Message::FollowEditor { playback } => {
                if let Err(error) = self.follow_editor(app, playback, Instant::now()) {
                    app.error = Some(error.clone());
                    self.send(Message::Error { message: error });
                }
            }
            Message::Load {
                request,
                chart,
                start,
                autoplay,
                speed,
                audio,
                sync,
            } => {
                if self.pending.is_some() {
                    self.send(Message::Ack {
                        request,
                        error: Some("Player 正在准备上一份谱面".into()),
                    });
                    return;
                }
                // Capture the Player clock before pausing it; the Editor's
                // cursor and possibly stale status are not the playback clock.
                let playback = Playback::for_load(app, sync, start, autoplay, speed);
                if app.pad.mode == Mode::Playing {
                    app.pad.toggle_play();
                }
                app.audio_reset_for_remote();
                app.pad.playback_pending = false;
                app.pad.audio_start_pending = false;
                app.pad.pending_audio_start = false;
                app.pad.stop_audio_if_any();
                app.remote_preview = true;
                app.error = None;
                app.go_instant(playback.page);
                app.preparing = true;
                let params = app.pad.params.clone();
                let cached = self.audio.clone();
                let generation = self.generation;
                let (tx, rx) = mpsc::channel();
                self.pending = Some(rx);
                std::thread::spawn(move || {
                    let result = prepare(chart, playback, audio, packet.bytes, cached, params);
                    let _ = tx.send(Completion {
                        generation,
                        request,
                        result,
                    });
                });
            }
            Message::Control { command } => {
                if !app.remote_preview {
                    self.send(Message::Error {
                        message: "请先发送预览谱面".into(),
                    });
                    return;
                }
                if app.preparing {
                    self.send(Message::Error {
                        message: "谱面准备中，请稍后控制播放".into(),
                    });
                    return;
                }
                match command {
                    Control::Pause => app.pause(),
                    Control::Resume => {
                        if app.pad.mode != Mode::Playing {
                            app.resume();
                        }
                    }
                    Control::Restart => app.restart(),
                    Control::Seek(t) if t.is_finite() => {
                        if let Err(e) = app.pad.try_seek_to(t) {
                            app.error = Some(e);
                        }
                    }
                    Control::Autoplay(on) => app.set_autoplay(on),
                    Control::Stop => {
                        self.following = None;
                        app.exit_to_select();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn follow_editor(
        &mut self,
        app: &mut PlayerUiApp,
        playback: Option<EditorPlayback>,
        now: Instant,
    ) -> Result<(), String> {
        if playback.is_some_and(|state| !state.valid()) {
            return Err("Editor 播放位置或速度无效".into());
        }
        let force = playback.is_some_and(|state| {
            self.following
                .is_none_or(|old| old.playback.speed != state.speed)
        });
        if playback.is_some_and(|state| state.playing)
            && self.following.is_some_and(|old| !old.playback.playing)
            && !app.backgrounded
            && !app::lifecycle::global().is_background()
        {
            app.lifecycle_paused = false;
        }
        self.following = playback.map(|playback| Following {
            playback,
            received: now,
        });
        self.apply_follow(app, now, force)
    }

    fn apply_follow(&self, app: &mut PlayerUiApp, now: Instant, force: bool) -> Result<(), String> {
        let Some(follow) = self.following else {
            return Ok(());
        };
        if !app.remote_preview
            || app.preparing
            || self.pending.is_some()
            || app.backgrounded
            || app.lifecycle_paused
            || app::lifecycle::global().is_background()
        {
            return Ok(());
        }
        let time = follow
            .playback
            .time_after(now.saturating_duration_since(follow.received))
            .clamp(app.pad.chart.playback_start_time(), app.pad.song_duration());
        let playing = follow.playback.playing;
        let was_playing = app.pad.mode == Mode::Playing;
        let seek = force
            || was_playing != playing
            || (app.pad.timeline_time() - time).abs() > if playing { 0.15 } else { 0.001 };
        if !playing && was_playing {
            app.pad.pause_at_wall_time(app.pad.now());
        }
        if app.pad.play_speed != follow.playback.speed {
            app.pad.set_play_speed(follow.playback.speed);
        }
        if seek {
            app.pad.try_seek_to(time)?;
        }
        if playing && app.pad.mode != Mode::Playing {
            let page = app.page;
            app.resume();
            if page == Page::Settings {
                app.page = page;
            }
            app.pad.playback_pending = true;
        }
        if playing && seek {
            app.gameplay_presented = false;
        }
        if !playing && !app.lifecycle_paused && app.page == Page::Pause {
            app.go_instant(Page::Gameplay);
        }
        Ok(())
    }

    fn install(&mut self, app: &mut PlayerUiApp, prepared: Prepared) -> Result<(), String> {
        let Prepared {
            chart,
            engine,
            audio,
            playback,
        } = prepared;
        let page = if app.page == Page::Settings {
            Page::Settings
        } else {
            playback.page
        };
        app.audio_reset_for_remote();
        let pcm = audio.as_ref().map(|(_, pcm)| pcm.clone());
        let name = audio.as_ref().and_then(|(info, _)| info.name.clone());
        app.pad.load_chart(chart, name, pcm, None, None);
        app.pad.cover_texture = None;
        app.pad.params.no_core = false;
        app::params::set(app.pad.params.clone());
        app.pad.autoplay = playback.autoplay;
        app.pad.autoplay_direct_core = false;
        app.pad.install_edited_engine(engine);
        app.pad.set_play_speed(playback.speed);
        app.gameplay_start = playback.restart
            .clamp(app.pad.chart.playback_start_time(), app.pad.song_duration());
        app.selected_level = Some(app.pad.chart.simai_level);
        app.pad.mode_song_offset = playback.start;
        app.pad.timeline_view_time = playback.start;
        let playing = playback.playing
            && !app.backgrounded
            && !app.lifecycle_paused
            && !app::lifecycle::global().is_background();
        app.pad.mode = if playing { Mode::Playing } else { Mode::Idle };
        app.pad.mode_wall_anchor = app.pad.now();
        app.pad.playback_pending = playing;
        app.pad.audio_start_pending = playing;
        app.pad.audio_seek_offset = Some(playback.start);
        app.gameplay_presented = false;
        app.page =
            if app.backgrounded || app.lifecycle_paused || app::lifecycle::global().is_background()
            {
                Page::Pause
            } else {
                page
            };
        app.status = format!("网络预览 · {}", app.pad.chart.title);
        self.audio = audio;
        self.apply_follow(app, Instant::now(), true)?;
        Ok(())
    }
}
fn prepare(
    chart: ChartDoc,
    mut playback: Playback,
    info: Option<AudioInfo>,
    bytes: Vec<u8>,
    cached: Option<(AudioInfo, WavPcm)>,
    params: app::params::Params,
) -> Result<Prepared, String> {
    if !playback.start.is_finite()
        || !playback.speed.is_finite()
        || !(0.1..=4.0).contains(&playback.speed)
    {
        return Err("预览位置或速度无效".into());
    }
    let audio = match info {
        Some(info) => {
            let pcm = if bytes.is_empty() {
                cached
                    .filter(|(i, _)| i.id == info.id)
                    .ok_or("音频缓存未命中，请重新发送谱面")?
                    .1
            } else {
                info.decode(&bytes)?
            };
            Some((info, pcm))
        }
        None => {
            if !bytes.is_empty() {
                return Err("音频数据缺少格式信息".into());
            }
            None
        }
    };
    let chart = app::templates::expand(&chart);
    let level = chart.simai_level.max(1);
    let compiled = crate::player::chart_compile::compile(&chart, level)?;
    let mut engine = JudgeEngine::load(&compiled.text, level)?;
    engine.bind_editor_chart(compiled.note_ids, compiled.slide_positions)?;
    let mut pad = PadPreviewState::new_at(
        chart.clone(),
        None,
        audio.as_ref().map(|(_, pcm)| pcm.clone()),
        0.0,
        None,
    );
    pad.forced_time = Some(0.0);
    pad.params = params;
    pad.autoplay = playback.autoplay;
    pad.autoplay_direct_core = false;
    let start = playback.start.clamp(pad.chart.playback_start_time(), pad.song_duration());
    playback.start = start;
    pad.forced_time = Some(start);
    pad.timeline_view_time = start;
    let engine = PreparedEditedEngine::new(
        &chart,
        engine,
        compiled.text,
        level,
        pad.edited_engine_context(),
    )?;
    Ok(Prepared {
        chart,
        engine,
        audio,
        playback,
    })
}

/// Desktop manual/QR-image dialog; Android uses the system camera and keyboard.
#[derive(Default)]
pub struct DesktopDialog {
    code: String,
    image: String,
}
impl DesktopDialog {
    pub fn draw(&mut self, ui: &mut egui_macroquad::egui::Ui, state: &mut UiState) {
        ui.label("粘贴 Editor 连接码");
        ui.text_edit_singleline(&mut self.code);
        if ui.button("连接").clicked() {
            match Invitation::parse(&self.code) {
                Ok(i) => state.action = Some(Action::Connect(i)),
                Err(e) => state.status = e,
            }
        }
        ui.label("或输入二维码图片路径（PNG / JPEG）");
        ui.text_edit_singleline(&mut self.image);
        if ui.button("扫描二维码图片").clicked() {
            match link::qr::from_image(std::path::Path::new(&self.image)) {
                Ok(i) => state.action = Some(Action::Connect(i)),
                Err(e) => state.status = e,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsaved_native_notes_and_first_survive_network_preparation() {
        let mut chart = app::maidata::from_maidata_level(
            "&first=1.25\n&inote_6=(120){4}1,2h[4:2],3-5[4:1],E\n",
            6,
        )
        .unwrap();
        app::maichart::assign_note_ids(&mut chart.notes);
        chart.notes[0].lane = 8;
        chart.notes[0].id = 999;
        let ids: Vec<_> = chart.notes.iter().map(|n| n.id).collect();
        chart.templates.push(app::types::TemplateDef {
            id: "t".into(),
            name: "test".into(),
            version: 1,
            notes: vec![chart.notes[0].clone()],
            duration: 1.0,
        });
        chart.template_instances.push(app::types::TemplateInstance {
            instance_id: "instance".into(),
            template_id: "t".into(),
            template_version: 1,
            anchor_time: 2.0,
            hidden: false,
        });
        chart
            .autoplay_overrides
            .push(app::autoplay_edit::AutoplayOverride {
                key: app::autoplay_edit::AutoplayKey::standalone(42),
                events: vec![crate::core::types::TimedInputEvent::SensorClick {
                    tp: 100_000,
                    area: crate::core::types::SensorArea::A4,
                }],
                curve: None,
            });
        // Exercise the same framing and serde options as the actual TCP path.
        let mut wire = Vec::new();
        link::transport::write_packet(
            &mut wire,
            &Message::Load {
                request: 1,
                chart,
                start: 0.75,
                autoplay: false,
                speed: 1.0,
                audio: None,
                sync: false,
            }
            .into(),
        )
        .unwrap();
        let Packet {
            message:
                Message::Load {
                    chart,
                    start,
                    autoplay,
                    speed,
                    audio,
                    ..
                },
            bytes,
        } = link::transport::read_packet(&mut wire.as_slice()).unwrap()
        else {
            panic!("wrong packet")
        };
        let ready = prepare(
            chart,
            Playback {
                start,
                autoplay,
                speed,
                playing: true,
                restart: start,
                page: Page::Gameplay,
            },
            audio,
            bytes,
            None,
            app::params::Params::default(),
        )
        .unwrap();
        assert_eq!(ready.chart.audio_offset, 1.25);
        assert_eq!(ready.playback.start, 0.75);
        assert_eq!(
            ready.engine.engine.chart_spec().unwrap().taps[0].slot,
            crate::core::types::OuterSlot::S8
        );
        assert_eq!(
            ready
                .chart
                .notes
                .iter()
                .take(ids.len())
                .map(|n| n.id)
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(ready.chart.notes.len(), ids.len() + 1);
        assert!(ready.chart.template_instances.is_empty());
        assert_eq!(ready.chart.autoplay_overrides.len(), 1);
        assert_eq!(ready.engine.context.time, 0.75);
    }
    #[test]
    fn repeated_preview_reuses_cached_pcm() {
        let chart = app::maidata::from_maidata_level("&inote_6=(120){4}1,E\n", 6).unwrap();
        let pcm = WavPcm {
            sample_rate: 48000,
            channels: 1,
            samples: std::sync::Arc::new(vec![0; 48000]),
        };
        let info = AudioInfo::from_pcm(&pcm, None);
        let ready = prepare(
            chart.clone(),
            Playback {
                start: 0.0,
                autoplay: true,
                speed: 1.0,
                playing: false,
                restart: 0.0,
                page: Page::Gameplay,
            },
            Some(info.clone()),
            vec![],
            Some((info.clone(), pcm.clone())),
            app::params::Params::default(),
        )
        .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &ready.audio.unwrap().1.samples,
            &pcm.samples
        ));
        assert!(
            prepare(
                chart,
                Playback {
                    start: 0.0,
                    autoplay: true,
                    speed: 1.0,
                    playing: false,
                    restart: 0.0,
                    page: Page::Gameplay
                },
                Some(info),
                vec![],
                None,
                app::params::Params::default()
            )
            .is_err()
        );
    }

    fn preview_app() -> PlayerUiApp {
        let chart =
            app::maidata::from_maidata_level("&inote_6=(120){4}1,2,3,4,5,6,7,8,E\n", 6).unwrap();
        let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
        pad.forced_time = Some(0.75);
        pad.timeline_view_time = 0.75;
        pad.play_speed = 0.8;
        let mut app = PlayerUiApp::from_pad(pad);
        app.remote_preview = true;
        app.gameplay_start = 0.25;
        app
    }

    #[test]
    fn measure_zero_can_be_prepared_installed_and_followed() {
        for playing in [false, true] {
            let mut app = preview_app();
            let mut controller = Controller::default();
            let start = app.pad.chart.playback_start_time();
            let ready = prepare(
                app.pad.chart.clone(),
                Playback {
                    start, playing, autoplay: true, speed: 1.0,
                    restart: start, page: Page::Gameplay,
                },
                None, vec![], None, app::params::Params::default(),
            ).unwrap();
            assert_eq!(ready.engine.context.time, start);
            controller.install(&mut app, ready).unwrap();
            assert_eq!(app.pad.mode_song_offset, start);
            assert_eq!(app.pad.timeline_view_time, start);
            assert_eq!(app.gameplay_start, start);
            assert_eq!(app.pad.mode == Mode::Playing, playing);
            app.pad.forced_time = Some(-1.0);
            controller.follow_editor(
                &mut app,
                Some(EditorPlayback { time: -1.0, playing, speed: 0.5 }),
                Instant::now(),
            ).unwrap();
            assert_eq!(app.pad.mode_song_offset, -1.0);
            assert_eq!(app.pad.timeline_view_time, -1.0);
            assert_eq!(app.pad.play_speed, 0.5);
            app.restart();
            assert_eq!(app.pad.mode_song_offset, start);
        }
    }

    #[test]
    fn following_seeks_and_shares_play_pause_and_speed_then_can_be_disabled() {
        let mut app = preview_app();
        let mut controller = Controller::default();
        let now = Instant::now();
        let state = EditorPlayback {
            time: 1.5,
            playing: false,
            speed: 0.5,
        };
        controller
            .follow_editor(&mut app, Some(state), now)
            .unwrap();
        assert_eq!(app.pad.timeline_time(), 1.5);
        assert_eq!(app.pad.mode, Mode::Idle);
        assert_eq!(app.pad.play_speed, 0.5);
        app.pad.forced_time = Some(1.5);
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    playing: true,
                    ..state
                }),
                now,
            )
            .unwrap();
        assert_eq!(app.pad.mode, Mode::Playing);
        assert_eq!(app.pad.mode_song_offset, 1.5);
        assert!(app.pad.playback_pending);
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 1.75,
                    ..state
                }),
                now,
            )
            .unwrap();
        assert_eq!(app.pad.mode, Mode::Idle);
        assert_eq!(app.pad.timeline_time(), 1.75);
        assert!(!app.pad.pending_audio_start);
        controller.follow_editor(&mut app, None, now).unwrap();
        controller
            .apply_follow(&mut app, now + std::time::Duration::from_secs(2), true)
            .unwrap();
        assert_eq!(app.pad.timeline_time(), 1.75);
    }

    #[test]
    fn following_coalesces_while_preparing_and_compensates_for_preparation_time() {
        let mut app = preview_app();
        let mut controller = Controller::default();
        app.preparing = true;
        let now = Instant::now();
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 0.0,
                    playing: false,
                    speed: 1.0,
                }),
                now,
            )
            .unwrap();
        let latest = EditorPlayback {
            time: 1.5,
            playing: true,
            speed: 0.5,
        };
        controller
            .follow_editor(&mut app, Some(latest), now)
            .unwrap();
        assert_eq!(app.pad.timeline_time(), 0.75);
        app.preparing = false;
        app.pad.forced_time = Some(2.0);
        controller
            .apply_follow(&mut app, now + std::time::Duration::from_secs(1), true)
            .unwrap();
        assert_eq!(app.pad.mode_song_offset, 2.0);
        assert_eq!(app.pad.mode, Mode::Playing);
        assert_eq!(app.pad.play_speed, 0.5);
    }

    #[test]
    fn following_does_not_reseek_for_an_ordinary_playback_heartbeat() {
        let mut app = preview_app();
        app.pad.mode = Mode::Playing;
        app.pad.mode_song_offset = 0.75;
        let mut controller = Controller::default();
        let now = Instant::now();
        controller.following = Some(Following {
            playback: EditorPlayback {
                time: 0.75,
                playing: true,
                speed: 0.8,
            },
            received: now,
        });
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 0.8,
                    playing: true,
                    speed: 0.8,
                }),
                now,
            )
            .unwrap();
        assert_eq!(app.pad.mode_song_offset, 0.75);
        assert!(!app.pad.playback_pending);
    }

    #[test]
    fn enabling_follow_aligns_a_running_player_immediately() {
        let mut app = preview_app();
        app.pad.mode = Mode::Playing;
        app.pad.mode_song_offset = 0.75;
        let mut controller = Controller::default();
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 0.8,
                    playing: true,
                    speed: 0.8,
                }),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(app.pad.mode_song_offset, 0.8);
        assert!(!app.gameplay_presented);
    }

    #[test]
    fn following_in_settings_updates_judgment_and_releases_the_frame_gate() {
        let mut app = preview_app();
        app.page = Page::Settings;
        let mut controller = Controller::default();
        let now = Instant::now();
        let state = EditorPlayback {
            time: 1.5,
            playing: true,
            speed: 1.0,
        };
        controller
            .follow_editor(&mut app, Some(state), now)
            .unwrap();
        assert_eq!(app.page, Page::Settings);
        assert!(app.updates_gameplay());
        assert!(app.pad.playback_pending);
        app.finish_gameplay_frame();
        assert!(app.gameplay_presented);
        assert!(app.pad.playback_pending);
        app.finish_gameplay_frame();
        assert!(!app.pad.playback_pending);
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    playing: false,
                    ..state
                }),
                now,
            )
            .unwrap();
        assert!(!app.updates_gameplay());
        assert_eq!(app.page, Page::Settings);
    }

    #[test]
    fn chart_packets_install_the_latest_follow_state_without_closing_settings() {
        for playing in [false, true] {
            let mut app = preview_app();
            let mut controller = Controller::default();
            let mut chart = app.pad.chart.clone();
            chart.notes[0].lane = 8;
            let mut wire = Vec::new();
            link::transport::write_packet(
                &mut wire,
                &Message::Load {
                    request: 42,
                    chart,
                    start: 0.25,
                    autoplay: true,
                    speed: 1.0,
                    audio: None,
                    sync: true,
                }
                .into(),
            )
            .unwrap();
            controller.packet(
                &mut app,
                link::transport::read_packet(&mut wire.as_slice()).unwrap(),
            );
            assert!(app.preparing);
            app.open_settings();
            assert!(app.preparing);
            let now = Instant::now();
            let state = EditorPlayback {
                time: 1.5,
                playing,
                speed: 0.5,
            };
            controller
                .follow_editor(&mut app, Some(EditorPlayback { time: 0.0, ..state }), now)
                .unwrap();
            controller
                .follow_editor(&mut app, Some(state), now)
                .unwrap();
            assert_eq!(app.pad.timeline_time(), 0.75);
            let completion = controller
                .pending
                .take()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(completion.request, 42);
            app.preparing = false;
            controller
                .install(&mut app, completion.result.unwrap())
                .unwrap();
            assert_eq!(app.pad.chart.notes[0].lane, 8);
            assert_eq!(app.page, Page::Settings);
            assert_eq!(app.pad.mode == Mode::Playing, playing);
            assert_eq!(app.pad.play_speed, 0.5);
            assert!((app.pad.mode_song_offset - 1.5).abs() < 0.1);
        }
    }

    #[test]
    fn disabling_follow_during_preparation_does_not_restore_the_queued_editor_clock() {
        let mut app = preview_app();
        app.page = Page::Pause;
        let ready = prepare(
            app.pad.chart.clone(),
            Playback::for_load(&app, true, 0.0, true, 1.0),
            None,
            vec![],
            None,
            app.pad.params.clone(),
        )
        .unwrap();
        let mut controller = Controller::default();
        app.preparing = true;
        let now = Instant::now();
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 1.5,
                    playing: true,
                    speed: 0.5,
                }),
                now,
            )
            .unwrap();
        controller.follow_editor(&mut app, None, now).unwrap();
        app.preparing = false;
        controller.install(&mut app, ready).unwrap();
        assert_eq!(app.pad.timeline_time(), 0.75);
        assert_eq!(app.pad.play_speed, 0.8);
        assert_eq!(app.pad.mode, Mode::Idle);
        assert_eq!(app.page, Page::Pause);
        assert!(controller.following.is_none());
    }

    #[test]
    fn preparing_in_the_background_cannot_start_following_audio() {
        let mut app = preview_app();
        app.pad.mode = Mode::Playing;
        let ready = prepare(
            app.pad.chart.clone(),
            Playback::for_load(&app, true, 0.0, true, 1.0),
            None,
            vec![],
            None,
            app.pad.params.clone(),
        )
        .unwrap();
        app.backgrounded = true;
        app.lifecycle_paused = true;
        let mut controller = Controller::default();
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 1.5,
                    playing: true,
                    speed: 1.0,
                }),
                Instant::now(),
            )
            .unwrap();
        controller.install(&mut app, ready).unwrap();
        assert_eq!(app.pad.mode, Mode::Idle);
        assert_eq!(app.page, Page::Pause);
        assert!(!app.pad.playback_pending);
        assert!(!app.pad.audio_start_pending);
        assert!(!app.pad.pending_audio_start);
        assert_eq!(app.pad.mode_song_offset, 0.75);
    }

    #[test]
    fn following_keeps_the_suspended_position_until_explicitly_resumed() {
        let mut app = preview_app();
        app.lifecycle_paused = true;
        let mut controller = Controller::default();
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 1.5,
                    playing: true,
                    speed: 1.0,
                }),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(app.pad.timeline_time(), 0.75);
        assert_eq!(app.pad.mode, Mode::Idle);
        assert!(!app.pad.pending_audio_start);
    }

    #[test]
    fn following_waits_for_the_actual_pending_chart_even_if_the_overlay_changes() {
        let mut app = preview_app();
        let mut controller = Controller::default();
        let (_tx, rx) = mpsc::channel();
        controller.pending = Some(rx);
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    time: 1.5,
                    playing: false,
                    speed: 1.0,
                }),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(app.pad.timeline_time(), 0.75);
    }

    #[test]
    fn editor_heartbeats_cannot_resume_playback_after_background_pause() {
        let mut app = preview_app();
        let mut controller = Controller::default();
        let now = Instant::now();
        app.lifecycle_paused = true;
        let state = EditorPlayback {
            time: 1.5,
            playing: true,
            speed: 1.0,
        };
        controller
            .follow_editor(&mut app, Some(state), now)
            .unwrap();
        assert_eq!(app.pad.mode, Mode::Idle);
        controller
            .follow_editor(&mut app, Some(state), now)
            .unwrap();
        assert_eq!(app.pad.mode, Mode::Idle);
        controller
            .follow_editor(
                &mut app,
                Some(EditorPlayback {
                    playing: false,
                    ..state
                }),
                now,
            )
            .unwrap();
        controller
            .follow_editor(&mut app, Some(state), now)
            .unwrap();
        assert_eq!(app.pad.mode, Mode::Playing);
        assert!(!app.lifecycle_paused);
    }

    #[test]
    fn invalid_follow_messages_leave_the_last_good_clock_unchanged() {
        let mut app = preview_app();
        let mut controller = Controller::default();
        let now = Instant::now();
        for (time, speed) in [
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (f32::NEG_INFINITY, 1.0),
            (1.0, f32::INFINITY),
            (1.0, 0.0),
        ] {
            assert!(
                controller
                    .follow_editor(
                        &mut app,
                        Some(EditorPlayback {
                            time,
                            playing: true,
                            speed
                        }),
                        now
                    )
                    .is_err()
            );
            assert!(controller.following.is_none());
            assert_eq!(app.pad.timeline_time(), 0.75);
        }
    }

    #[test]
    fn automatic_refresh_preserves_player_position_mode_and_restart_point() {
        for playing in [false, true] {
            let mut app = preview_app();
            app.page = if playing { Page::Gameplay } else { Page::Pause };
            app.pad.mode = if playing { Mode::Playing } else { Mode::Idle };
            app.pad.autoplay = playing;
            // The sender cursor, speed and autoplay must not override an
            // existing preview. Exercise the actual load wire format too.
            let mut chart = app.pad.chart.clone();
            chart.notes[0].lane = 8;
            chart.notes.remove(1);
            let mut bytes = Vec::new();
            link::transport::write_packet(
                &mut bytes,
                &Message::Load {
                    request: 7,
                    chart,
                    start: 0.0,
                    autoplay: !playing,
                    speed: 1.5,
                    audio: None,
                    sync: true,
                }
                .into(),
            )
            .unwrap();
            let Packet {
                message:
                    Message::Load {
                        chart,
                        start,
                        autoplay,
                        speed,
                        audio,
                        sync,
                        ..
                    },
                bytes,
            } = link::transport::read_packet(&mut bytes.as_slice()).unwrap()
            else {
                panic!("wrong packet")
            };
            let playback = Playback::for_load(&app, sync, start, autoplay, speed);
            let ready =
                prepare(chart, playback, audio, bytes, None, app.pad.params.clone()).unwrap();
            assert_eq!(ready.engine.context.time, 0.75);
            Controller::default().install(&mut app, ready).unwrap();
            assert_eq!(app.pad.chart.notes[0].lane, 8);
            assert_eq!(app.pad.chart.notes.len(), 7);
            assert_eq!(app.pad.timeline_time(), 0.75);
            assert_eq!(app.pad.mode == Mode::Playing, playing);
            assert_eq!(app.pad.autoplay, playing);
            assert_eq!(app.pad.play_speed, 0.8);
            assert_eq!(app.gameplay_start, 0.25);
            assert_eq!(app.pad.playback_pending, playing);
            assert_eq!(app.pad.audio_start_pending, playing);
            assert_eq!(app.page, if playing { Page::Gameplay } else { Page::Pause });
        }
    }

    #[test]
    fn first_sync_is_paused_and_explicit_preview_still_starts() {
        let mut app = preview_app();
        app.remote_preview = false;
        let sync = Playback::for_load(&app, true, 0.5, true, 1.0);
        assert!(!sync.playing);
        assert_eq!(sync.start, 0.5);
        app.remote_preview = true;
        let play = Playback::for_load(&app, false, 0.0, true, 1.2);
        assert!(play.playing);
        assert_eq!(play.start, 0.0);
        assert_eq!(play.restart, 0.0);
        assert_eq!(play.speed, 1.2);
    }
}
