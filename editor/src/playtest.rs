//! Network preview controller. Never wait for network or PCM encoding on a frame.
use crate::preview_link::{
    self as link, AudioInfo, Control, EditorPlayback, Invitation, Message, Packet, Status,
    devices::{Devices, Remembered},
    transport::{Endpoint, Event},
};
use crate::{
    app::types::{ChartDoc, WavPcm},
    player::state::PadPreviewState,
    state::EditorState,
};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const SYNC_CHECK: Duration = Duration::from_millis(100);
const SYNC_DEBOUNCE: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy)]
pub struct Request {
    pub start: f32,
    pub autoplay: bool,
}
#[derive(Debug, Clone)]
pub enum Action {
    Listen,
    Search,
    Connect(Invitation),
    Disconnect,
    Forget(String),
    Control(Control),
    JumpToPlayer,
}
#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub action: Option<Action>,
    pub listening: bool,
    pub connected: bool,
    pub address: String,
    pub invitation: String,
    pub peer: String,
    pub status: String,
    pub remote: Status,
    pub devices: Vec<Remembered>,
    pub sending: bool,
    pub qr: Option<(usize, Vec<bool>)>,
    pub searching: bool,
    pub found: Vec<link::discovery::Found>,
    pub manual_address: String,
    pub pairing_pin: String,
    /// Opt-out, so a newly connected Player follows edits by default.
    pub sync_disabled: bool,
}

struct Source {
    chart: ChartDoc,
    document: u64,
    pcm: Option<WavPcm>,
    name: Option<String>,
    template: Option<(String, f32)>,
}
impl Source {
    fn capture(editor: &EditorState, pad: &PadPreviewState) -> Self {
        Self {
            chart: pad.chart.clone(),
            document: pad.document_revision(),
            pcm: pad.audio_wav_pcm.clone(),
            name: pad.audio_source_name.clone(),
            template: editor
                .template_session
                .as_ref()
                .map(|s| (s.template_id.clone(), s.offset)),
        }
    }
    fn matches(&self, editor: &EditorState, pad: &PadPreviewState) -> bool {
        let same_audio = match (&self.pcm, &pad.audio_wav_pcm) {
            (None, None) => true,
            (Some(old), Some(new)) => {
                Arc::ptr_eq(&old.samples, &new.samples)
                    && old.sample_rate == new.sample_rate
                    && old.channels == new.channels
            }
            _ => false,
        };
        self.document == pad.document_revision()
            && self.chart == pad.chart
            && same_audio
            && self.name == pad.audio_source_name
            && self
                .template
                .as_ref()
                .map(|(id, offset)| (id.as_str(), *offset))
                == editor
                    .template_session
                    .as_ref()
                    .map(|s| (s.template_id.as_str(), s.offset))
    }
}

/// One latest snapshot, at most ten comparisons per second and one in-flight
/// transfer. Edits made during a transfer remain dirty until the next send.
#[derive(Default)]
struct EditSync {
    observed: Option<Source>,
    next_check: Option<Instant>,
    changed_at: Option<Instant>,
}
impl EditSync {
    fn observe(&mut self, editor: &EditorState, pad: &PadPreviewState, now: Instant) {
        if self.next_check.is_some_and(|at| now < at) {
            return;
        }
        self.next_check = Some(now + SYNC_CHECK);
        if self
            .observed
            .as_ref()
            .is_none_or(|s| !s.matches(editor, pad))
        {
            self.observed = Some(Source::capture(editor, pad));
            self.changed_at = Some(now);
        }
    }
    fn due(&self, now: Instant) -> bool {
        self.changed_at
            .is_some_and(|at| now.duration_since(at) >= SYNC_DEBOUNCE)
    }
    fn sent(&mut self, editor: &EditorState, pad: &PadPreviewState, now: Instant) {
        self.observed = Some(Source::capture(editor, pad));
        self.next_check = Some(now + SYNC_CHECK);
        self.changed_at = None;
    }
}
pub fn available(editor: &EditorState) -> bool {
    editor.check_slide_placement_finished().is_ok()
        && !editor.keyboard.owns_input()
        && editor.selection_transform.is_none()
        && editor.dragging.is_none()
        && editor.handle_grab.is_none()
        && editor.hold_placement.is_none()
        && !editor.file_dialog_active
        && editor.pending_action.is_none()
}
struct Encoded {
    generation: u64,
    result: Result<(Packet, Option<(AudioInfo, WavPcm)>), String>,
}

#[derive(Default)]
struct ProgressSync {
    sent: Option<(Option<EditorPlayback>, Instant)>,
}
impl ProgressSync {
    fn update(&mut self, playback: Option<EditorPlayback>, now: Instant) -> bool {
        let due = self
            .sent
            .is_none_or(|(previous, at)| match (previous, playback) {
                (Some(old), Some(new)) => {
                    let elapsed = now.saturating_duration_since(at);
                    let drift = (new.time - old.time_after(elapsed)).abs();
                    old.playing != new.playing
                        || old.speed != new.speed
                        || elapsed >= Duration::from_millis(50)
                            && (drift > if new.playing { 0.08 } else { 0.001 }
                                || new.playing && elapsed >= Duration::from_secs(1))
                }
                (None, None) => false,
                _ => true,
            });
        if due {
            self.sent = Some((playback, now));
        }
        due
    }
}

#[derive(Default)]
pub struct Controller {
    endpoint: Option<Endpoint>,
    devices: Option<Devices>,
    encoding: Option<mpsc::Receiver<Encoded>>,
    audio: Option<(AudioInfo, WavPcm)>,
    generation: u64,
    request: u64,
    pending: Option<u64>,
    discovery: Option<mpsc::Receiver<Result<Vec<link::discovery::Found>, String>>>,
    invitation: Option<Invitation>,
    edit_sync: EditSync,
    progress_sync: ProgressSync,
    loaded: bool,
}
impl Controller {
    pub fn update(&mut self, editor: &mut EditorState, pad: &mut PadPreviewState) {
        if self.devices.is_none() {
            match Devices::load("Editor") {
                Ok(devices) => {
                    editor.player_link.devices = devices.peers();
                    self.devices = Some(devices);
                }
                Err(error) => {
                    editor.player_link.status = error;
                    return;
                }
            }
        }
        if let Some(action) = editor.player_link.action.take() {
            let result = match action {
                Action::Listen => self.listen(&mut editor.player_link),
                Action::Search => {
                    if self.discovery.is_none() {
                        self.discovery = Some(link::discovery::search());
                        editor.player_link.searching = true;
                        editor.player_link.status = "正在搜索局域网 Player…".into();
                    }
                    Ok(())
                }
                Action::Connect(invitation) => {
                    self.endpoint = None;
                    self.encoding = None;
                    self.generation += 1;
                    self.pending = None;
                    editor.player_link.connected = false;
                    editor.player_link.listening = false;
                    editor.player_link.sending = false;
                    editor.player_link.status = format!("连接 {}…", invitation.address);
                    self.endpoint = Some(Endpoint::connect(
                        invitation.clone(),
                        self.devices.as_ref().unwrap().identity(),
                        "Editor",
                    ));
                    self.invitation = Some(invitation);
                    Ok(())
                }
                Action::Disconnect => {
                    self.endpoint = None;
                    self.encoding = None;
                    self.invitation = None;
                    self.generation += 1;
                    self.pending = None;
                    editor.player_link.listening = false;
                    editor.player_link.connected = false;
                    editor.player_link.sending = false;
                    editor.player_link.status = "已停止网络预览".into();
                    Ok(())
                }
                Action::Forget(id) => {
                    let result = self.devices.as_mut().unwrap().forget(&id);
                    editor.player_link.devices = self.devices.as_ref().unwrap().peers();
                    result
                }
                Action::Control(command) => {
                    if pad.params.player_follow_editor {
                        match command {
                            Control::Pause | Control::Stop => {
                                if pad.mode == crate::app::types::Mode::Playing {
                                    crate::runtime::toggle_play(editor, pad);
                                }
                            }
                            Control::Resume => {
                                if pad.mode != crate::app::types::Mode::Playing {
                                    crate::runtime::toggle_play(editor, pad);
                                }
                            }
                            Control::Restart => crate::runtime::restart_playback(editor, pad),
                            Control::Seek(time) => pad.seek_to(time),
                            _ => {}
                        }
                        if matches!(command, Control::Stop | Control::Autoplay(_)) {
                            self.send(Message::Control { command })
                        } else {
                            Ok(())
                        }
                    } else {
                        self.send(Message::Control { command })
                    }
                }
                Action::JumpToPlayer => {
                    if available(editor) {
                        pad.seek_to(editor.player_link.remote.time);
                        crate::runtime::follow_playhead(editor, editor.player_link.remote.time);
                    }
                    Ok(())
                }
            };
            if let Err(error) = result {
                editor.player_link.status = error;
            }
        }
        if let Some(result) = self.discovery.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.discovery = None;
            editor.player_link.searching = false;
            match result {
                Ok(found) => {
                    editor.player_link.status = format!(
                        "找到 {} 个 Player；首次连接请输入设备上的配对码",
                        found.len()
                    );
                    editor.player_link.found = found;
                }
                Err(error) => editor.player_link.status = error,
            }
        }
        for _ in 0..16 {
            let Some(event) = self.endpoint.as_ref().and_then(|e| e.poll()) else {
                break;
            };
            match event {
                Event::Connected(peer, token) => {
                    self.generation += 1;
                    self.pending = None;
                    self.encoding = None;
                    self.edit_sync = EditSync::default();
                    self.progress_sync = ProgressSync::default();
                    self.loaded = false;
                    editor.player_link.connected = true;
                    editor.player_link.sending = false;
                    editor.player_link.remote = Status::default();
                    editor.player_link.peer = peer.name.clone();
                    editor.player_link.status = format!("已连接 {}", peer.name);
                    let invitation = self.invitation.clone().map(|mut invitation| {
                        invitation.token = token;
                        invitation
                    });
                    if let Err(e) = self.devices.as_mut().unwrap().remember(peer, invitation) {
                        editor.player_link.status = e;
                    }
                    editor.player_link.devices = self.devices.as_ref().unwrap().peers();
                }
                Event::Disconnected(error) => {
                    editor.player_link.connected = false;
                    editor.player_link.sending = false;
                    editor.player_link.status = format!("连接断开: {error}");
                    self.pending = None;
                    self.encoding = None;
                    self.generation += 1;
                }
                Event::Packet(packet) => match packet.message {
                    Message::Status { state } => editor.player_link.remote = state,
                    Message::Ack { request, error } if self.pending == Some(request) => {
                        self.pending = None;
                        editor.player_link.sending = false;
                        if error.is_none() {
                            self.loaded = true;
                            self.progress_sync = ProgressSync::default();
                            // Ack precedes the next status packet on the wire.
                            editor.player_link.remote.cached_audio =
                                self.audio.as_ref().map(|(i, _)| i.id.clone());
                        }
                        editor.player_link.status = error
                            .map(|e| format!("Player 拒绝预览: {e}"))
                            .unwrap_or_else(|| "Player 已载入当前谱面".into());
                    }
                    Message::Error { message } => {
                        editor.player_link.status = message;
                        if self.encoding.is_none() {
                            self.pending = None;
                            editor.player_link.sending = false;
                        }
                    }
                    _ => {}
                },
            }
        }
        let completed = self.encoding.as_ref().map(|rx| rx.try_recv());
        if let Some(Ok(ready)) = completed {
            self.encoding = None;
            if ready.generation == self.generation && editor.player_link.connected {
                match ready.result {
                    Ok((packet, audio)) => {
                        self.audio = audio;
                        if let Err(e) = self.endpoint.as_ref().unwrap().send(packet) {
                            editor.player_link.status = e;
                            editor.player_link.sending = false;
                            self.pending = None;
                        }
                    }
                    Err(e) => {
                        editor.player_link.status = e;
                        editor.player_link.sending = false;
                        self.pending = None;
                    }
                }
            }
        } else if matches!(completed, Some(Err(mpsc::TryRecvError::Disconnected))) {
            self.encoding = None;
            self.pending = None;
            editor.player_link.sending = false;
            editor.player_link.status = "谱面编码线程已退出，请重新发送".into();
        }
        if let Some(request) = editor.playtest_request.take() {
            if let Err(error) = self.preview(editor, pad, request, false) {
                editor.player_link.status = error;
            } else if pad.params.player_follow_editor {
                pad.start_playback_at(request.start);
            }
        }
        let now = Instant::now();
        if editor.player_link.connected
            && (!editor.player_link.sync_disabled
                || pad.params.player_follow_editor && !self.loaded)
            && available(editor)
            && !editor.simai_editor.modified()
        {
            self.edit_sync.observe(editor, pad, now);
            if self.edit_sync.due(now) && !editor.player_link.sending && self.encoding.is_none() {
                let request = Request {
                    start: pad.timeline_time(),
                    autoplay: true,
                };
                if let Err(error) = self.preview(editor, pad, request, true) {
                    // Retry on the next edit or explicit preview, not every frame.
                    self.edit_sync.sent(editor, pad, now);
                    editor.player_link.status = format!("自动同步失败: {error}");
                }
            }
        }
        if editor.player_link.connected && self.loaded {
            let playback = pad.params.player_follow_editor.then(|| EditorPlayback {
                time: pad.timeline_time(),
                playing: matches!(
                    pad.mode,
                    crate::app::types::Mode::Playing | crate::app::types::Mode::Recording
                ) && !pad.scrubbing,
                speed: pad.play_speed,
            });
            if self.progress_sync.update(playback, now) {
                if let Err(error) = self.send(Message::FollowEditor { playback }) {
                    self.progress_sync = ProgressSync::default();
                    editor.player_link.status = error;
                }
            }
        }
    }
    fn listen(&mut self, ui: &mut UiState) -> Result<(), String> {
        if ui.listening {
            return Ok(());
        }
        self.endpoint = None;
        self.invitation = None;
        self.encoding = None;
        self.pending = None;
        self.generation += 1;
        ui.connected = false;
        ui.sending = false;
        let devices = self.devices.as_ref().unwrap();
        let endpoint = Endpoint::listen(
            &format!("0.0.0.0:{}", link::PORT),
            devices.token().into(),
            devices.identity(),
            "Editor",
        )?;
        ui.address = link::transport::local_address(endpoint.address.unwrap().port());
        let invitation = Invitation {
            address: ui.address.clone(),
            token: devices.token().into(),
        };
        ui.invitation = invitation.encode();
        ui.qr = link::qr::modules(&ui.invitation).ok();
        ui.listening = true;
        ui.status = "等待 Player 扫码连接".into();
        self.endpoint = Some(endpoint);
        Ok(())
    }
    fn send(&self, message: Message) -> Result<(), String> {
        self.endpoint
            .as_ref()
            .ok_or("请先连接 Player")?
            .send(message)
    }
    fn preview(
        &mut self,
        editor: &mut EditorState,
        pad: &mut PadPreviewState,
        request: Request,
        sync: bool,
    ) -> Result<(), String> {
        if !editor.player_link.connected {
            return Err("请先搜索 Player 连接，或让 Player 扫描编辑器二维码".into());
        }
        if editor.player_link.sending || self.encoding.is_some() {
            return Err("上一份谱面仍在传输或载入".into());
        }
        if !available(editor) {
            return Err("请先确认或取消当前编辑操作".into());
        }
        crate::simai_editor::apply(editor, &mut pad.chart)
            .map_err(|e| format!("Simai 草稿: {e}"))?;
        let chart = crate::templates::document(editor, &pad.chart);
        let pcm = pad.audio_wav_pcm.clone();
        let name = pad.audio_source_name.clone();
        if pcm
            .as_ref()
            .is_some_and(|p| p.samples.len() > link::MAX_AUDIO / 2)
        {
            return Err("音频超过网络预览 256 MiB 上限".into());
        }
        let speed = pad.play_speed;
        let cached = editor.player_link.remote.cached_audio.clone();
        let previous = self.audio.clone();
        let generation = self.generation;
        self.request += 1;
        let request_id = self.request;
        self.pending = Some(request_id);
        let (tx, rx) = mpsc::channel();
        self.encoding = Some(rx);
        self.edit_sync.sent(editor, pad, Instant::now());
        editor.player_link.sending = true;
        editor.player_link.status = if sync {
            "正在同步谱面修改…"
        } else {
            "正在传送未保存谱面…"
        }
        .into();
        std::thread::spawn(move || {
            let result = (|| {
                let audio = pcm.map(|pcm| {
                    let info = previous
                        .filter(|(_, old)| {
                            Arc::ptr_eq(&old.samples, &pcm.samples)
                                && old.sample_rate == pcm.sample_rate
                                && old.channels == pcm.channels
                        })
                        .map(|(info, _)| info)
                        .unwrap_or_else(|| AudioInfo::from_pcm(&pcm, name));
                    (info, pcm)
                });
                let mut bytes = Vec::new();
                if let Some((info, pcm)) = &audio {
                    if info.samples > link::MAX_AUDIO / 2 {
                        return Err("音频超过网络预览 256 MiB 上限".into());
                    }
                    if cached.as_deref() != Some(info.id.as_str()) {
                        bytes = pcm.samples.iter().flat_map(|s| s.to_le_bytes()).collect();
                    }
                }
                Ok((
                    Packet {
                        message: Message::Load {
                            request: request_id,
                            chart,
                            start: request.start,
                            autoplay: request.autoplay,
                            speed,
                            audio: audio.as_ref().map(|(i, _)| i.clone()),
                            sync,
                        },
                        bytes,
                    },
                    audio,
                ))
            })();
            let _ = tx.send(Encoded { generation, result });
        });
        Ok(())
    }
}

pub fn menu(
    ui: &mut egui_macroquad::egui::Ui,
    editor: &mut EditorState,
    pad: &mut PadPreviewState,
) {
    use egui_macroquad::egui;
    let enabled = available(editor) && editor.player_link.connected && !editor.player_link.sending;
    let state = &mut editor.player_link;
    ui.label("网络 Player · 独立进程 / 局域网设备");
    ui.label("Player → 设置 → 网络预览 → 允许 Editor 搜索连接");
    if ui
        .add_enabled(
            !state.searching,
            egui::Button::new(if state.searching {
                "搜索中…"
            } else {
                "搜索 Player 设备"
            }),
        )
        .clicked()
    {
        state.action = Some(Action::Search);
    }
    ui.horizontal(|ui| {
        ui.label("首次配对码");
        ui.add(egui::TextEdit::singleline(&mut state.pairing_pin).desired_width(90.0));
    });
    let valid_pin =
        state.pairing_pin.len() == 6 && state.pairing_pin.bytes().all(|b| b.is_ascii_digit());
    for found in &state.found {
        let saved = state
            .devices
            .iter()
            .find(|p| p.device.id == found.device.id)
            .and_then(|p| p.invitation.as_ref());
        ui.horizontal(|ui| {
            ui.label(format!("{} · {}", found.device.name, found.address));
            if ui
                .add_enabled(saved.is_some() || valid_pin, egui::Button::new("连接"))
                .clicked()
            {
                state.action = Some(Action::Connect(Invitation {
                    address: found.address.clone(),
                    token: saved
                        .map(|i| i.token.clone())
                        .unwrap_or_else(|| state.pairing_pin.clone()),
                }));
            }
        });
    }
    ui.collapsing("手动输入 Player 地址", |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.manual_address).hint_text("192.168.1.10:47653"),
        );
        if ui
            .add_enabled(
                valid_pin && !state.manual_address.trim().is_empty(),
                egui::Button::new("用配对码连接"),
            )
            .clicked()
        {
            state.action = Some(Action::Connect(Invitation {
                address: state.manual_address.trim().into(),
                token: state.pairing_pin.clone(),
            }));
        }
    });
    ui.separator();
    if !state.listening {
        if ui.button("开启连接 / 显示二维码").clicked() {
            state.action = Some(Action::Listen);
        }
    } else {
        ui.label(format!("地址: {}", state.address));
        if !state.connected {
            ui.label("Player → 设置 → 网络预览 → 扫码连接");
            if let Some((side, cells)) = &state.qr {
                let cell = (224.0 / *side as f32).floor().max(2.0);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(*side as f32 * cell, *side as f32 * cell),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 0.0, egui::Color32::WHITE);
                for y in 0..*side {
                    for x in 0..*side {
                        if cells[y * side + x] {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.min + egui::vec2(x as f32 * cell, y as f32 * cell),
                                    egui::vec2(cell, cell),
                                ),
                                0.0,
                                egui::Color32::BLACK,
                            );
                        }
                    }
                }
            }
            if ui.button("复制连接码（手动连接）").clicked() {
                ui.ctx().copy_text(state.invitation.clone());
            }
        }
    }
    if state.listening || state.connected {
        if ui.button("关闭网络连接").clicked() {
            state.action = Some(Action::Disconnect);
        }
    }
    ui.label(&state.status);
    let mut auto_sync = !state.sync_disabled;
    ui.checkbox(&mut auto_sync, "自动同步编辑到 Player");
    state.sync_disabled = !auto_sync;
    if ui
        .checkbox(
            &mut pad.params.player_follow_editor,
            "Player 跟随编辑器进度与播放",
        )
        .changed()
    {
        crate::app::params::set(pad.params.clone());
        if let Err(error) = crate::app::params::save(&pad.params) {
            editor.status = Some(format!("保存 Player 跟随设置失败: {error}"));
        }
    }
    ui.separator();
    for (label, current, autoplay) in [
        ("从当前位置游玩", true, false),
        ("从头游玩", false, false),
        ("从当前位置自动预览", true, true),
        ("从头自动预览", false, true),
    ] {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            editor.playtest_request = Some(Request {
                start: if current { pad.timeline_time() } else { pad.chart.playback_start_time() },
                autoplay,
            });
        }
    }
    if state.connected {
        ui.label(format!(
            "{} · {} · {:.2}/{:.2}s",
            state.peer, state.remote.title, state.remote.time, state.remote.duration
        ));
        ui.horizontal(|ui| {
            for (text, command) in [
                ("暂停", Control::Pause),
                ("继续", Control::Resume),
                ("重播", Control::Restart),
                ("停止", Control::Stop),
            ] {
                if ui.button(text).clicked() {
                    state.action = Some(Action::Control(command));
                }
            }
        });
        if ui.button("Player 跳到编辑器当前位置").clicked() {
            state.action = Some(Action::Control(Control::Seek(pad.timeline_time())));
        }
        if ui.button("编辑器定位到 Player 进度").clicked() {
            state.action = Some(Action::JumpToPlayer);
        }
        if let Some(error) = &state.remote.error {
            ui.label(error);
        }
    }
    if !state.devices.is_empty() {
        ui.separator();
        ui.label("记住的设备");
        for saved in state.devices.clone() {
            ui.horizontal(|ui| {
                ui.label(&saved.device.name);
                if let Some(invitation) = saved.invitation {
                    if ui.small_button("重连").clicked() {
                        state.action = Some(Action::Connect(invitation));
                    }
                } else {
                    ui.label("由 Player 扫码重连");
                }
                if ui.small_button("移除").clicked() {
                    state.action = Some(Action::Forget(saved.device.id.clone()));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad() -> PadPreviewState {
        let chart =
            crate::app::maidata::from_maidata_level("&inote_6=(120){4}1,2,3,E\n", 6).unwrap();
        let mut pad = PadPreviewState::new_at(chart, None, None, 0.0, None);
        pad.forced_time = Some(0.0);
        pad
    }

    #[test]
    fn progress_follow_tracks_seek_pause_speed_and_disable_without_streaming_frames() {
        let now = Instant::now();
        let mut sync = ProgressSync::default();
        assert!(sync.update(None, now));
        assert!(!sync.update(None, now + Duration::from_secs(2)));
        let paused = EditorPlayback {
            time: 2.0,
            playing: false,
            speed: 1.0,
        };
        assert!(sync.update(Some(paused), now));
        assert!(!sync.update(Some(paused), now + Duration::from_secs(1)));
        let seek = EditorPlayback {
            time: 4.0,
            ..paused
        };
        assert!(sync.update(Some(seek), now + Duration::from_millis(100)));
        assert!(!sync.update(Some(paused), now + Duration::from_millis(120)));
        assert!(sync.update(Some(paused), now + Duration::from_millis(160)));
        let playing = EditorPlayback {
            playing: true,
            ..seek
        };
        assert!(sync.update(Some(playing), now + Duration::from_secs(1)));
        assert!(!sync.update(
            Some(EditorPlayback {
                time: 4.5,
                ..playing
            }),
            now + Duration::from_millis(1500)
        ));
        assert!(sync.update(
            Some(EditorPlayback {
                time: 5.0,
                ..playing
            }),
            now + Duration::from_secs(2)
        ));
        assert!(sync.update(
            Some(EditorPlayback {
                speed: 0.5,
                time: 5.0,
                ..playing
            }),
            now + Duration::from_secs(2)
        ));
        assert!(sync.update(
            Some(EditorPlayback {
                time: 5.0,
                ..paused
            }),
            now + Duration::from_secs(2)
        ));
        assert!(sync.update(None, now + Duration::from_secs(2)));
        assert!(!sync.update(None, now + Duration::from_secs(3)));
    }

    #[test]
    fn edits_during_transfer_are_coalesced_and_undo_is_not_missed() {
        let mut pad = pad();
        let mut editor = EditorState::default();
        let now = Instant::now();
        let mut sync = EditSync::default();
        sync.observe(&editor, &pad, now);
        assert!(!sync.due(now));
        assert!(sync.due(now + SYNC_DEBOUNCE));
        sync.sent(&editor, &pad, now);
        let revision = pad.document_revision();
        editor.selected.insert(pad.chart.notes[0].id);
        editor.delete_selection(&mut pad.chart);
        assert_eq!(pad.document_revision(), revision);
        let edited = now + Duration::from_millis(200);
        sync.observe(&editor, &pad, edited);
        // Still dirty while the previous transfer is in flight.
        assert!(sync.due(edited + SYNC_DEBOUNCE));
        editor.undo(&mut pad.chart);
        let undone = edited + Duration::from_millis(200);
        sync.observe(&editor, &pad, undone);
        assert!(!sync.due(undone));
        assert!(sync.due(undone + SYNC_DEBOUNCE));
        sync.sent(&editor, &pad, undone + SYNC_DEBOUNCE);
        // Selection and cursor movement do not resend the chart.
        editor.selected.clear();
        pad.timeline_view_time = 1.0;
        sync.observe(&editor, &pad, undone + Duration::from_secs(1));
        assert!(!sync.due(undone + Duration::from_secs(2)));
        editor.redo(&mut pad.chart);
        sync.observe(&editor, &pad, undone + Duration::from_secs(2));
        assert!(sync.due(undone + Duration::from_secs(3)));
    }

    #[test]
    fn source_detects_unsaved_metadata_and_replaced_audio() {
        let editor = EditorState::default();
        let mut pad = pad();
        pad.audio_wav_pcm = Some(WavPcm {
            sample_rate: 48000,
            channels: 1,
            samples: Arc::new(vec![0; 10]),
        });
        let source = Source::capture(&editor, &pad);
        pad.audio_wav_pcm = source.pcm.clone();
        assert!(source.matches(&editor, &pad));
        pad.chart.audio_offset = 1.25;
        assert!(!source.matches(&editor, &pad));
        pad.chart = source.chart.clone();
        pad.audio_wav_pcm.as_mut().unwrap().samples = Arc::new(vec![1; 10]);
        assert!(!source.matches(&editor, &pad));
    }
}
