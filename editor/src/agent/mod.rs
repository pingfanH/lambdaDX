//! One memory-only conversation per editor; dock panes never own a runtime.
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
}
impl Default for Session {
    fn default() -> Self {
        let loaded = settings::load();
        let error = loaded.as_ref().err().cloned();
        let applied = loaded.unwrap_or_default();
        Self {
            draft: applied.clone(),
            applied,
            enabled: false,
            input: String::new(),
            messages: Vec::new(),
            log: Vec::new(),
            stream: String::new(),
            state: State::Idle,
            run: None,
            proposal: None,
            revision: None,
            error,
            usage: "未知".into(),
        }
    }
}
impl Session {
    fn busy(&self) -> bool {
        matches!(
            self.state,
            State::Requesting | State::Streaming | State::ExecutingTools
        )
    }
    fn record(&mut self, title: impl Into<String>, text: impl Into<String>) {
        self.log.push((title.into(), text.into()));
        while self.log.len() > 128
            || self
                .log
                .iter()
                .map(|(a, b)| a.len() + b.len())
                .sum::<usize>()
                > MAX_CONTEXT_BYTES
        {
            self.log.remove(0);
        }
    }
    pub fn stop(&mut self) {
        if let Some(run) = self.run.take() {
            run.stop();
        }
        if self.busy() || self.state == State::AwaitingApproval {
            self.state = State::Cancelled;
            self.messages.push(Message::user(
                r#"{"editor_event":"Run stopped by user; unfinished work and pending proposals cancelled. No approval was granted."}"#,
            ));
            self.record("编辑器", "运行已停止，待审提案已撤销");
        }
        self.proposal = None;
    }
    fn clear(&mut self) {
        self.stop();
        self.messages.clear();
        self.log.clear();
        self.stream.clear();
        self.error = None;
        self.state = State::Idle;
        self.usage = "未知".into();
    }
    pub fn tick(&mut self, host: &Service, revision: u64) {
        if self.revision.is_some_and(|old| old != revision) {
            self.clear();
            self.record("编辑器", "文档已切换，会话已清空");
        }
        self.revision = Some(revision);
        let mut events = Vec::new();
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
                Event::State(state) => self.state = state,
                Event::Text(text) => {
                    if self.stream.len() + text.len() <= 64 * 1024 {
                        self.stream.push_str(&text);
                    }
                }
                Event::Usage(usage) => {
                    self.usage = usage
                        .and_then(|u| u.total_tokens)
                        .map(|n| format!("最近一轮 {n} tokens"))
                        .unwrap_or_else(|| "未知（provider 未报告完整 usage）".into())
                }
                Event::Tool { id, name, result } => {
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
                    self.state = state;
                    self.messages = messages;
                    self.error = error.map(|e| e.message);
                    if !self.stream.is_empty() {
                        let text = std::mem::take(&mut self.stream);
                        self.record("助手", text);
                    }
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
    }
    fn reviewed(&mut self, result: ProposalResult) {
        let event = serde_json::json!({"editor_review":result});
        self.messages.push(Message::user(event.to_string()));
        self.record(
            "编辑器审核结果（再次发送才会调用模型）",
            serde_json::to_string_pretty(&event).unwrap_or_default(),
        );
        self.state = State::Completed;
        self.proposal = None;
        self.run = None;
    }
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("内置 Agent · 模型连接", |ui| {
            if ui
                .checkbox(&mut self.enabled, "启用内置 Agent（本次编辑器会话）")
                .changed()
                && !self.enabled
            {
                self.stop();
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
                    settings::save(&self.draft)
                        .map_err(lambdadx_automation::contract::ToolError::invalid)
                }) {
                    Ok(()) => {
                        self.clear();
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
        app: &PadPreviewState,
    ) {
        ui.label(format!("Agent · {:?}", self.state));
        ui.small(format!(
            "{} · {}",
            self.applied.model, self.applied.base_url
        ));
        ui.small(format!("Usage: {}", self.usage));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.run.is_some(), egui::Button::new("Stop / 撤销待审提案"))
                .clicked()
            {
                self.stop();
            }
            if ui.button("清空会话").clicked() {
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
                for (title, text) in &self.log {
                    ui.collapsing(title, |ui| {
                        ui.label(text);
                    });
                }
                if !self.stream.is_empty() {
                    ui.label(&self.stream);
                }
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
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
            if let Err(error) = self.send(host, editor, app) {
                self.error = Some(error);
            }
        }
    }
    fn send(
        &mut self,
        host: &mut Service,
        editor: &EditorState,
        app: &PadPreviewState,
    ) -> Result<(), String> {
        let mut selected: Vec<_> = editor.selected.iter().copied().collect();
        selected.sort_unstable();
        if selected.len() > 256 {
            return Err("显式选区超过 256 个音符，请缩小选区".into());
        }
        let context = serde_json::json!({"selected_note_ids":selected.iter().map(u64::to_string).collect::<Vec<_>>(),"selection_is_explicit":true,"measure_units":"measure 1 is start; in 4/4 a beat is 0.25 measures","document_revision":app.document_revision()});
        let prompt = format!(
            "{}\n\nEditor selection context (data): {}",
            self.input.trim(),
            context
        );
        if self.messages.iter().map(Message::size).sum::<usize>() + prompt.len() > MAX_CONTEXT_BYTES
        {
            return Err("会话超过 256 KiB，请清空会话或缩小任务".into());
        }
        let provider = Genai::new(&self.applied).map_err(|e| e.message)?;
        let runtime = host.background().map_err(|e| e.to_string())?;
        self.stop();
        self.messages.push(Message::user(prompt));
        let text = std::mem::take(&mut self.input);
        self.record("用户", text);
        self.error = None;
        self.stream.clear();
        self.state = State::Requesting;
        self.run = Some(Run::start(
            &runtime,
            provider,
            host.client(),
            self.messages.clone(),
            self.applied.permissions(),
        ));
        Ok(())
    }
}
