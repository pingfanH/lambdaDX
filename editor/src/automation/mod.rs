//! Main-thread adapter. Background callers only own the bounded bridge client.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("Editor automation currently supports desktop targets only");
use crate::{player::state::PadPreviewState, state::EditorState};
use lambdadx_automation::{
    bridge::{self, Client, EditorHost, Inbox},
    contract::*,
};
use serde::Serialize;
use serde_json::Value;
use egui_macroquad::egui;

mod query;
mod snapshot;
mod proposal;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "editor-agent"))]
mod agent_tests;
#[cfg(all(test, feature = "editor-agent"))]
mod preview_tests;

pub struct Service {
    client: Client,
    inbox: Inbox,
    snapshots: snapshot::Store,
    proposals: proposal::Store,
    consent_auto_save: bool,
    approve_requested: bool,
    reject_requested: bool,
    #[cfg(any(feature = "editor-mcp", feature = "editor-agent"))]
    runtime: Option<lambdadx_automation::runtime::BackgroundRuntime>,
    #[cfg(feature = "editor-mcp")]
    mcp: Option<lambdadx_automation::mcp::http::Server>,
    #[cfg(feature = "editor-mcp")]
    starting: Option<std::sync::mpsc::Receiver<std::io::Result<lambdadx_automation::mcp::http::Server>>>,
    adapter_error: Option<String>,
}
impl Service {
    pub fn new() -> Self {
        let (client, inbox) = bridge::channel();
        Self {
            client,
            inbox,
            snapshots: snapshot::Store::new(),
            proposals: proposal::Store::default(),
            consent_auto_save: false,
            approve_requested: false,
            reject_requested: false,
            #[cfg(any(feature = "editor-mcp", feature = "editor-agent"))]
            runtime: None,
            #[cfg(feature = "editor-mcp")]
            mcp: None,
            #[cfg(feature = "editor-mcp")]
            starting: None,
            adapter_error: None,
        }
    }
    pub fn client(&self) -> Client {
        self.client.clone()
    }
    /// Pin a private snapshot and grant cursor additions only to this run.
    #[cfg(feature = "editor-agent")]
    pub fn prepare_cursor(&mut self, client: &Client, editor: &mut EditorState, app: &PadPreviewState) -> ToolResult<lambdadx_automation::agent::cursor::Context> {
        use crate::app::types::{Mode, zone};
        if self.proposals.pending().is_some() || editor.keyboard.owns_input() || editor.file_dialog_active || editor.template_session.is_some() || app.mode != Mode::Idle {
            return Err(ToolError::new(ErrorCode::EditorBusy, "暂停播放并结束当前编辑后再直接生成"));
        }
        if editor.simai_editor.modified() {
            return Err(ToolError::new(ErrorCode::UnappliedDraft, "请先应用或放弃 Simai 草稿"));
        }
        let cursor = editor.keyboard.cursor;
        if !cursor.measure.is_finite() || cursor.measure < 1.0 { return Err(ToolError::invalid("光标位置无效")); }
        self.snapshots.observe(editor, app);
        if editor.simai_editor.has_history_source() { editor.simai_editor.sync(&app.chart); }
        let snapshot = self.snapshots.capture(editor, app, false)?;
        let context = lambdadx_automation::agent::cursor::Context {
            target: snapshot.target,
            measure: cursor.measure as f64,
            ring_lane: ((cursor.zone.saturating_sub(1)) % 8) + 1,
            touch_zone: cursor.touch.then(|| zone::id_to_zone(cursor.zone).map(|z| z.to_string())).flatten(),
            step_measures: 1.0 / editor.snap_division.max(1) as f64,
            bpm: app.chart.bpms.iter().filter(|b| b.measure <= cursor.measure).max_by(|a, b| a.measure.total_cmp(&b.measure)).map_or(app.chart.bpm, |b| b.bpm) as f64,
        };
        self.proposals.arm_cursor(client.session(), context.clone());
        Ok(context)
    }
    #[cfg(any(feature = "editor-mcp", feature = "editor-agent"))]
    pub fn background(&mut self) -> std::io::Result<lambdadx_automation::runtime::Handle> {
        if self.runtime.is_none() { self.runtime = Some(lambdadx_automation::runtime::BackgroundRuntime::new()?); }
        Ok(self.runtime.as_ref().unwrap().handle())
    }
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("MCP 与 Agent").show(ui, |ui| {
            #[cfg(feature = "editor-mcp")]
            {
                if let Some(receiver) = &self.starting {
                    match receiver.try_recv() {
                        Ok(Ok(server)) => { self.mcp = Some(server); self.starting = None; },
                        Ok(Err(e)) => { self.adapter_error = Some(e.to_string()); self.starting = None; },
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.adapter_error = Some("MCP 启动任务结束".into()); self.starting = None; },
                        _ => {}
                    }
                }
                if let Some(server) = &self.mcp {
                    ui.label(format!("Endpoint: {}", server.endpoint));
                    ui.label(format!("实例: {} · 会话: {}", server.instance_id, server.session_count()));
                    ui.monospace(format!("lambdadx_mcp --instance {}", server.instance_id));
                    let mut permissions = server.permissions();
                    let mut changed = ui.checkbox(&mut permissions.focus, "允许定位视图").changed();
                    changed |= ui.checkbox(&mut permissions.propose, "允许创建修改提案（仍需人工审核）").changed();
                    if changed { server.set_permissions(permissions); }
                    if ui.button("关闭 MCP 并撤销所有会话权限").clicked() { self.mcp = None; }
                } else if self.starting.is_some() { ui.label("正在启动 MCP…"); }
                else if ui.button("启用本机 MCP（默认只读）").clicked() {
                    match self.background() {
                        Ok(runtime) => {
                            let client = self.client(); let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                            runtime.spawn(async move { let _ = sender.send(lambdadx_automation::mcp::http::Server::start(client).await); });
                            self.starting = Some(receiver);
                        },
                        Err(e) => self.adapter_error = Some(e.to_string()),
                    }
                }
            }
            #[cfg(not(feature = "editor-mcp"))]
            ui.label("此构建未启用 editor-mcp");
            if let Some(error) = &self.adapter_error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
        });
    }
    /// Called after ChartSync publishes the canonical chart. No work is queued
    /// by default, and an idle frame never clones or scans the chart.
    pub fn tick(
        &mut self,
        editor: &mut EditorState,
        app: &PadPreviewState,
        idle: bool,
        core_current: bool,
    ) {
        self.snapshots.observe(editor, app);
        self.proposals.tick(&self.snapshots, editor, app);
        let mut host = Host {
            snapshots: &mut self.snapshots,
            proposals: &mut self.proposals,
            session: self.client.session(),
            editor,
            app,
            idle,
            core_current: core_current && !app.has_chart_preview(),
        };
        self.inbox.drain(&mut host);
    }

    pub fn maintenance(&mut self, editor: &mut EditorState, app: &mut PadPreviewState, sync: &mut crate::runtime::ChartSync, idle: bool) {
        self.snapshots.observe(editor, app);
        self.proposals.tick(&self.snapshots, editor, app);
        if std::mem::take(&mut self.reject_requested) {
            self.proposals.reject();
            self.approve_requested = false;
            self.consent_auto_save = false;
        }
        self.proposals.preview(app, idle);
        if std::mem::take(&mut self.approve_requested) {
            if let Err(e) = self.proposals.approve(editor, app, &mut self.snapshots, idle, self.consent_auto_save) { editor.status = Some(e.message); }
            self.consent_auto_save = false;
        }
        self.proposals.schedule(editor, app, sync, idle);
        if self.proposals.pending().is_none() { self.consent_auto_save = false; }
    }

    pub fn review_window(&mut self, ctx: &egui::Context, editor: &EditorState, app: &PadPreviewState) {
        if self.proposals.pending().is_some_and(|p| p.immediate()) {
            egui::Area::new(egui::Id::new("agent-cursor-preview"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 80.0))
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(320.0);
                        self.cursor_preview_ui(ui, editor, app.mode == crate::app::types::Mode::Idle);
                    });
                });
        } else if self.proposals.pending().is_some() {
            egui::Window::new("Automation · 修改审核").id(egui::Id::new("automation-review")).default_width(520.0).show(ctx, |ui| self.proposal_ui(ui, editor));
        }
    }

    fn cursor_preview_ui(&mut self, ui: &mut egui::Ui, editor: &EditorState, paused: bool) {
        let Some(p) = self.proposals.pending() else { return; };
        ui.strong("配置虚影预览");
        ui.label(&p.result.summary);
        let count = p.result.changes.iter().filter(|d| d.before.is_none() && d.after.is_some()).count();
        if p.previewing() {
            ui.label(format!("{count} 个音符 · 可正常播放试听"));
            ui.small("接受后写入谱面、选中音符并推进光标；拒绝会移除虚影。");
        } else {
            ui.horizontal(|ui| { ui.spinner(); ui.label("正在校验配置并准备试听…"); });
        }
        let ready = p.previewing() && p.result.state == ProposalState::AwaitingApproval;
        if editor.auto_save {
            ui.checkbox(&mut self.consent_auto_save, "接受后允许按当前自动保存规则写回");
        }
        if !paused { ui.small("暂停播放后可接受；播放中也可拒绝。"); }
        ui.horizontal(|ui| {
            if ui.add_enabled(ready && paused && (!editor.auto_save || self.consent_auto_save), egui::Button::new("接受")).clicked() { self.approve_requested = true; }
            if ui.button("拒绝").clicked() { self.reject_requested = true; }
        });
    }

    #[cfg(feature = "editor-agent")]
    pub fn agent_proposal_status(&self, id: &str, session: &bridge::Session) -> ToolResult<ProposalResult> {
        self.proposals.status(id, session)
    }
    pub fn proposal_ui(&mut self, ui: &mut egui::Ui, editor: &EditorState) {
        let Some(p) = self.proposals.pending() else { ui.label("没有等待审核的修改提案"); return; };
        if p.immediate() { ui.label("配置将在虚影气泡中预览；可试听后接受或拒绝。"); return; }
        ui.heading("修改提案：需要人工确认");
        ui.label(&p.result.summary);
        ui.label(format!("{:?} · {} 个音符差异", p.result.state, p.result.changes.len()));
        for warning in &p.result.warnings { ui.label(warning); }
        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            for d in &p.result.changes {
                ui.collapsing(format!("音符 {}", d.id), |ui| {
                    ui.monospace(format!("之前：{}", serde_json::to_string(&d.before).unwrap()));
                    ui.monospace(format!("之后：{}", serde_json::to_string(&d.after).unwrap()));
                });
            }
            if let Some((before, after)) = p.source_diff() {
                ui.collapsing("精确 Simai 原文：之前 / 之后", |ui| { ui.label("之前"); ui.monospace(before); ui.separator(); ui.label("之后"); ui.monospace(after); });
            }
        });
        if editor.auto_save { ui.checkbox(&mut self.consent_auto_save, "允许本次修改按现有自动保存规则写回当前来源"); }
        let ready = p.result.state == ProposalState::AwaitingApproval && (!editor.auto_save || self.consent_auto_save);
        ui.horizontal(|ui| {
            if ui.add_enabled(ready, egui::Button::new("确认应用（可撤销）")).clicked() { self.approve_requested = true; }
            if ui.button("拒绝").clicked() { self.reject_requested = true; }
        });
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.client.session().cancel();
        #[cfg(feature = "editor-mcp")]
        { self.mcp = None; self.starting = None; }
        #[cfg(any(feature = "editor-mcp", feature = "editor-agent"))]
        if let Some(runtime) = self.runtime.take() { runtime.shutdown(); }
    }
}
struct Host<'a> {
    snapshots: &'a mut snapshot::Store,
    proposals: &'a mut proposal::Store,
    session: bridge::Session,
    editor: &'a mut EditorState,
    app: &'a PadPreviewState,
    idle: bool,
    core_current: bool,
}
fn value<T: Serialize>(data: T) -> ToolResult<Value> {
    serde_json::to_value(data)
        .map_err(|e| ToolError::new(ErrorCode::ValidationFailed, e.to_string()))
}
impl EditorHost for Host<'_> {
    fn execute_scoped(&mut self, request: Request, session: bridge::Session) -> ToolResult<Value> { self.session = session; self.execute(request) }
    fn execute(&mut self, request: Request) -> ToolResult<Value> {
        request.validate()?;
        if self.editor.template_session.is_some() {
            return Err(ToolError::new(
                ErrorCode::UnsupportedOperation,
                "Template isolation is not supported; return to the main chart",
            ));
        }
        match request {
            Request::LibrarySearch(_) | Request::LibraryRead(_) => Err(ToolError::new(
                ErrorCode::UnsupportedOperation, "Library requests must run on the background library worker",
            )),
            Request::Propose(r) => value(self.proposals.create(r, self.session.clone(), self.snapshots, self.editor, self.app, self.idle)?),
            Request::ProposalStatus(r) => value(self.proposals.status(&r.proposal_id, &self.session)?),
            Request::Context(_) => {
                if !self.idle {
                    return Err(ToolError::new(
                        ErrorCode::EditorBusy,
                        "Finish the active edit before capturing a snapshot",
                    ));
                }
                // Graphical edits can leave clean source stale while the Simai
                // pane is hidden. Rebase it before exposing a new snapshot,
                // rather than depending on whether that pane has been drawn.
                // Keep absent source and user drafts intact; sync also retains
                // errors when the chart cannot be represented by Simai.
                if self.app.mode == crate::app::types::Mode::Idle
                    && !self.editor.keyboard.ime
                    && self.editor.simai_editor.has_history_source()
                {
                    self.editor.simai_editor.sync(&self.app.chart);
                }
                value(
                    self.snapshots
                        .capture(self.editor, self.app, self.core_current)?,
                )
            }
            Request::QueryNotes(r) => value(query::notes(self.snapshots.get(&r.target)?, &r)?),
            Request::ReadSimai(r) => value(query::simai(self.snapshots.get(&r.target)?, &r)?),
            Request::Check(r) => value(query::check(
                self.snapshots.get(&r.target)?,
                &r,
                self.app.judge_engine.is_some(),
            )?),
            Request::Focus(r) => {
                if !self.idle {
                    return Err(ToolError::new(
                        ErrorCode::EditorBusy,
                        "Finish the active edit before focusing",
                    ));
                }
                let snapshot = self.snapshots.get(&r.target)?;
                if snapshot.chart != self.app.chart
                    || snapshot.text.as_deref()
                        != self
                            .editor
                            .simai_editor
                            .automation_source(&self.app.chart)
                            .0
                            .then_some(self.editor.simai_editor.text.as_str())
                {
                    return Err(ToolError::new(
                        ErrorCode::DocumentChanged,
                        "The live chart or Simai draft differs from this snapshot",
                    ));
                }
                if let Some(error) = query::tempo_error(&snapshot.chart) {
                    return Err(ToolError::new(ErrorCode::ValidationFailed, error));
                }
                let bpms = if snapshot.chart.bpms.is_empty() {
                    vec![crate::app::types::BpmChange {
                        measure: 1.0,
                        bpm: snapshot.chart.bpm,
                    }]
                } else {
                    snapshot.chart.bpms.clone()
                };
                let start = crate::app::types::measure_to_secs(r.range.start_measure as f32, &bpms);
                let end = crate::app::types::measure_to_secs(r.range.end_measure as f32, &bpms);
                if !start.is_finite() || !end.is_finite() {
                    return Err(ToolError::invalid(
                        "Focus range overflows the playback clock",
                    ));
                }
                // Scroll only. Leave playback, selection and history untouched.
                self.editor.view_start = start;
                self.editor.follow_playhead = false;
                value(FocusResult {
                    target: r.target,
                    range: r.range,
                })
            }
        }
    }
}
