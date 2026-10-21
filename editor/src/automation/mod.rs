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

pub struct Service {
    client: Client,
    inbox: Inbox,
    snapshots: snapshot::Store,
    proposals: proposal::Store,
    consent_auto_save: bool,
    approve_requested: bool,
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
        }
    }
    pub fn client(&self) -> Client {
        self.client.clone()
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
            core_current,
        };
        self.inbox.drain(&mut host);
    }

    pub fn maintenance(&mut self, editor: &mut EditorState, app: &mut PadPreviewState, sync: &mut crate::runtime::ChartSync, idle: bool) {
        self.snapshots.observe(editor, app);
        self.proposals.tick(&self.snapshots, editor, app);
        if std::mem::take(&mut self.approve_requested) {
            if let Err(e) = self.proposals.approve(editor, app, &mut self.snapshots, idle, self.consent_auto_save) { editor.status = Some(e.message); }
            self.consent_auto_save = false;
        }
        self.proposals.schedule(editor, app, sync, idle);
    }

    pub fn review_window(&mut self, ctx: &egui::Context, editor: &EditorState) {
        if self.proposals.pending().is_some() {
            egui::Window::new("Automation · 修改审核").id(egui::Id::new("automation-review")).default_width(520.0).show(ctx, |ui| self.proposal_ui(ui, editor));
        }
    }

    pub fn proposal_ui(&mut self, ui: &mut egui::Ui, editor: &EditorState) {
        let Some(p) = self.proposals.pending() else { ui.label("没有等待审核的修改提案"); return; };
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
            if ui.button("拒绝").clicked() { self.proposals.reject(); self.consent_auto_save = false; }
        });
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
            Request::Propose(r) => value(self.proposals.create(r, self.session.clone(), self.snapshots, self.editor, self.app, self.idle)?),
            Request::ProposalStatus(r) => value(self.proposals.status(&r.proposal_id, &self.session)?),
            Request::Context(_) => {
                if !self.idle {
                    return Err(ToolError::new(
                        ErrorCode::EditorBusy,
                        "Finish the active edit before capturing a snapshot",
                    ));
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
