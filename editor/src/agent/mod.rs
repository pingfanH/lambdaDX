//! One conversation per editor, with a local transcript; panes never own a runtime.
mod context;
mod history;
use context::{compact_for_next_run, compact_review};
mod settings;
#[cfg(test)]
mod tests;
use crate::{automation::Service, player::state::PadPreviewState, state::EditorState};
use egui_macroquad::egui;
use lambdadx_automation::{
    agent::{
        Event, MAX_CONTEXT_BYTES, Message, Run, State,
        provider::{Genai, ProviderKind, Settings},
    },
    contract::{ProposalResult, ProposalState},
};
use std::time::{Duration, Instant};

pub struct Session {
    draft: Settings,
    applied: Settings,
    enabled: bool,
    input: String,
    messages: Vec<Message>,
    log: Vec<(String, String)>,
    stream: String,
    state: State,
    run: Option<Run>,
    proposal: Option<String>,
    revision: Option<u64>,
    error: Option<String>,
    usage: String,
    history: history::Store,
    history_dirty: bool,
    history_saved_at: Instant,
    history_error: Option<String>,
}
impl Default for Session {
    fn default() -> Self {
        let loaded = settings::load();
        let error = loaded.as_ref().err().cloned();
        let history = history::Store::open();
        let history_error = history.as_ref().err().cloned();
        let mut session = Self::restore(loaded.unwrap_or_default(), history.unwrap_or_default());
        session.error = error;
        if history_error.is_some() {
            session.history_error = history_error;
        }
        session
    }
}
impl Session {
    fn restore(saved: settings::Saved, history: history::Store) -> Self {
        let applied = saved.connection;
        let loaded = history.load();
        let history_error = loaded.as_ref().err().cloned();
        let saved_history = loaded.unwrap_or_default();
        let mut session = Self {
            draft: applied.clone(),
            applied,
            enabled: saved.enabled,
            input: String::new(),
            messages: Vec::new(),
            log: saved_history.log,
            stream: saved_history.stream,
            state: State::Idle,
            run: None,
            proposal: None,
            revision: None,
            error: None,
            usage: "未知".into(),
            history,
            history_dirty: false,
            history_saved_at: Instant::now(),
            history_error,
        };
        session.flush_stream("助手（上次运行未完成）");
        if !session.log.is_empty() {
            session.record(
                "编辑器",
                if saved_history.interrupted {
                    "已恢复本地记录。上次运行已中断，待审提案未恢复；再次发送将开始新的模型会话。"
                } else {
                    "已恢复本地记录。再次发送将开始新的模型会话，并重新读取当前谱面。"
                },
            );
        }
        session
    }
    fn busy(&self) -> bool {
        matches!(
            self.state,
            State::Requesting | State::Streaming | State::ExecutingTools
        )
    }
    fn record(&mut self, title: impl Into<String>, text: impl Into<String>) {
        self.log.push((title.into(), text.into()));
        history::trim(&mut self.log);
        self.history_dirty = true;
    }
    fn flush_stream(&mut self, title: &str) {
        if !self.stream.is_empty() {
            let text = std::mem::take(&mut self.stream);
            self.record(title, text);
        }
    }
    fn save_history(&mut self, force: bool) {
        if !self.history_dirty
            || (!force && self.history_saved_at.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.history_saved_at = Instant::now();
        match self.history.save(
            &self.log,
            &self.stream,
            self.busy() || self.state == State::AwaitingApproval,
        ) {
            Ok(()) => {
                self.history_dirty = false;
                self.history_error = None;
            }
            Err(error) => self.history_error = Some(error),
        }
    }
    pub fn stop(&mut self) {
        let active = self.busy() || self.state == State::AwaitingApproval;
        if let Some(run) = self.run.take() {
            run.stop();
        }
        if active {
            self.flush_stream("助手（已中断）");
            self.state = State::Cancelled;
            self.messages.push(Message::user(
                r#"{"editor_event":"Run stopped by user; unfinished work and pending proposals cancelled. No approval was granted."}"#,
            ));
            self.record("编辑器", "运行已停止，待审提案已撤销");
        }
        self.proposal = None;
        self.save_history(active);
    }
    /// The editor exits without dropping UI state; flush before that exit path.
    pub fn checkpoint(&mut self) {
        self.save_history(true);
    }
    fn clear(&mut self) {
        self.stop();
        self.messages.clear();
        self.log.clear();
        self.stream.clear();
        self.error = None;
        self.state = State::Idle;
        self.usage = "未知".into();
        self.history_dirty = true;
        self.save_history(true);
    }
    fn reset_context(&mut self, reason: &str) {
        self.stop();
        self.messages.clear();
        self.error = None;
        self.state = State::Idle;
        self.usage = "未知".into();
        self.record("编辑器", reason);
        self.save_history(true);
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.stop();
        }
        // Persist only confirmed settings, never an unfinished connection draft.
        if let Err(error) = settings::save(&self.applied, enabled) {
            self.error = Some(error);
        }
    }
    pub fn tick(&mut self, host: &Service, revision: u64) {
        if self.revision.is_some_and(|old| old != revision) {
            self.reset_context("文档已切换，模型会话已重置，之前的对话保留在本地记录中。");
        }
        self.revision = Some(revision);
        let mut events = Vec::new();
        let mut save_now = false;
        if let Some(run) = &self.run {
            events.extend(run.events.try_iter().take(128));
            if let Ok(event) = run.finished.try_recv() {
                // Completion can race the first drain; all preceding tool
                // events must be consumed before releasing the run owner.
                events.extend(run.events.try_iter());
                events.push(event);
            }
        }
        for event in events {
            match event {
                Event::State(state) => {
                    self.state = state;
                    self.history_dirty = true;
                }
                Event::Text(text) => {
                    if self.stream.len() + text.len() <= 64 * 1024 {
                        self.stream.push_str(&text);
                        self.history_dirty = true;
                    }
                }
                Event::Usage(usage) => {
                    self.usage = usage
                        .and_then(|u| u.total_tokens)
                        .map(|n| format!("最近一轮 {n} tokens"))
                        .unwrap_or_else(|| "未知（provider 未报告完整 usage）".into())
                }
                Event::Tool { id, name, result } => {
                    save_now = true;
                    self.flush_stream("助手");
                    if name == "chart_propose_edit" {
                        if let Ok(value) = &result {
                            self.proposal = value
                                .get("proposal_id")
                                .and_then(|v| v.as_str())
                                .map(str::to_owned);
                        }
                    }
                    let text = match result {
                        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_default(),
                        Err(error) => format!("{:?}: {}", error.code, error.message),
                    };
                    self.record(format!("工具 {name} · {id}"), text);
                }
                Event::Finished {
                    state,
                    messages,
                    error,
                } => {
                    save_now = true;
                    self.state = state;
                    self.messages = messages;
                    self.error = error.map(|e| e.message);
                    self.flush_stream("助手");
                    if let Some(error) = self.error.clone() {
                        self.record("错误", error);
                    }
                    self.history_dirty = true;
                    if state != State::AwaitingApproval {
                        self.run = None;
                    }
                }
            }
        }
        if self.state == State::AwaitingApproval {
            if let (Some(id), Some(run)) = (&self.proposal, &self.run) {
                match host.agent_proposal_status(id, &run.session()) {
                    Ok(result)
                        if !matches!(
                            result.state,
                            ProposalState::Preparing | ProposalState::AwaitingApproval
                        ) =>
                    {
                        self.reviewed(result);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        save_now = true;
                        let event = serde_json::json!({"editor_review_error":error});
                        self.messages.push(Message::user(event.to_string()));
                        self.record("编辑器审核已结束", event.to_string());
                        self.error = Some(error.message);
                        self.state = State::Failed;
                        self.proposal = None;
                        self.run = None;
                    }
                }
            }
        }
        // Text checkpoints and retries are throttled; tool/terminal events save now.
        self.save_history(save_now);
    }
    fn reviewed(&mut self, result: ProposalResult) {
        self.messages.push(Message::user(compact_review(&result)));
        let event = serde_json::json!({"editor_review":result});
        self.record(
            "编辑器审核结果（再次发送才会调用模型）",
            serde_json::to_string_pretty(&event).unwrap_or_default(),
        );
        self.state = State::Completed;
        self.proposal = None;
        self.run = None;
        self.save_history(true);
    }
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("内置 Agent · 模型连接", |ui| {
            if ui
                .checkbox(&mut self.enabled, "启用内置 Agent（记住此设置）")
                .changed()
            {
                self.set_enabled(self.enabled);
            }
            ui.label("发送后，对话、显式选区 ID 和按需查询的谱面数据将发送到下面的 endpoint。");
            ui.small("启动时自动读取当前目录或上级目录的 .env；同名进程环境变量优先。");
            if ui.button("重新读取 .env 模型配置").clicked() {
                match self.draft.clone().with_environment() {
                    Ok(settings) => {
                        self.draft = settings;
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.message),
                }
            }
            egui::ComboBox::from_id_salt("agent-provider")
                .selected_text(format!("{:?}", self.draft.provider))
                .show_ui(ui, |ui| {
                    for kind in [
                        ProviderKind::OpenAI,
                        ProviderKind::OpenAICompatible,
                        ProviderKind::Ollama,
                    ] {
                        ui.selectable_value(&mut self.draft.provider, kind, format!("{kind:?}"));
                    }
                });
            ui.label("Model");
            ui.text_edit_singleline(&mut self.draft.model);
            ui.label("Base URL");
            ui.text_edit_singleline(&mut self.draft.base_url);
            ui.label("凭据环境变量名（不填写密钥）");
            ui.text_edit_singleline(&mut self.draft.credential_env);
            ui.checkbox(&mut self.draft.allow_focus, "允许定位视图");
            ui.checkbox(
                &mut self.draft.allow_proposals,
                "允许创建提案（人工确认后才应用）",
            );
            ui.label("兼容端点与本地模型的工具支持取决于实际模型。更改连接或权限后需确认设置。");
            if ui.button("确认 endpoint 与权限并保存配置").clicked() {
                match self.draft.validate().and_then(|()| {
                    settings::save(&self.draft, self.enabled)
                        .map_err(lambdadx_automation::contract::ToolError::invalid)
                }) {
                    Ok(()) => {
                        self.reset_context("模型连接或权限已更新，后续消息使用新的模型会话。");
                        self.applied = self.draft.clone();
                    }
                    Err(error) => self.error = Some(error.message),
                }
            }
            if self.draft != self.applied {
                ui.label("连接/权限有待确认的更改，暂不能发送");
            }
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        });
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        host: &mut Service,
        editor: &EditorState,
        _app: &PadPreviewState,
    ) {
        ui.label(format!("Agent · {:?}", self.state));
        ui.small(format!(
            "{} · {}",
            self.applied.model, self.applied.base_url
        ));
        ui.small(format!("Usage: {}", self.usage));
        ui.small("对话自动保存在本机；重启后恢复记录，模型从新会话开始。");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.run.is_some(), egui::Button::new("Stop / 撤销待审提案"))
                .clicked()
            {
                self.stop();
            }
            if ui.button("清空会话与本地记录").clicked() {
                self.clear();
            }
        });
        if !self.enabled {
            ui.label("请在设置 → 模型连接中启用 Agent 并确认 endpoint");
        }
        let log_height = (ui.available_height() - 170.0).max(60.0);
        egui::ScrollArea::vertical()
            .id_salt("agent-log")
            .max_height(log_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (index, (title, text)) in self.log.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.collapsing(title, |ui| {
                            ui.label(text);
                        });
                    });
                }
                if !self.stream.is_empty() {
                    ui.label(&self.stream);
                }
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                if let Some(error) = &self.history_error {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        format!("本地对话记录读写失败：{error}"),
                    );
                }
            });
        if self.state == State::AwaitingApproval {
            host.proposal_ui(ui, editor);
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.input)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .char_limit(16 * 1024)
                .hint_text("例如：检查选中音符所在小节，给出修改建议"),
        );
        let ready = self.enabled
            && !self.busy()
            && self.state != State::AwaitingApproval
            && self.draft == self.applied
            && !self.input.trim().is_empty();
        if ui
            .add_enabled(ready, egui::Button::new("发送到所选 endpoint"))
            .clicked()
        {
            if let Err(error) = self.send(host) {
                self.record("错误", error.clone());
                self.error = Some(error);
                self.save_history(true);
            }
        }
    }
    fn send(&mut self, host: &mut Service) -> Result<(), String> {
        let prompt = self.input.trim().to_owned();
        let mut messages = compact_for_next_run(&self.messages);
        if messages.iter().map(Message::size).sum::<usize>() + prompt.len() > MAX_CONTEXT_BYTES {
            return Err("会话超过 256 KiB，请清空会话或缩小任务".into());
        }
        let provider = Genai::new(&self.applied).map_err(|e| e.message)?;
        let runtime = host.background().map_err(|e| e.to_string())?;
        self.stop();
        messages.push(Message::user(prompt));
        self.messages = messages;
        let text = std::mem::take(&mut self.input);
        self.record("用户", text);
        self.error = None;
        self.stream.clear();
        self.state = State::Requesting;
        self.save_history(true);
        self.run = Some(Run::start_with_context(
            &runtime,
            provider,
            host.client(),
            self.messages.clone(),
            self.applied.permissions(),
        ));
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
        self.checkpoint();
    }
}
