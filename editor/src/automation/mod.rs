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

mod query;
mod snapshot;
#[cfg(test)]
mod tests;

pub struct Service {
    client: Client,
    inbox: Inbox,
    snapshots: snapshot::Store,
}
impl Service {
    pub fn new() -> Self {
        let (client, inbox) = bridge::channel();
        Self {
            client,
            inbox,
            snapshots: snapshot::Store::new(),
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
        let mut host = Host {
            snapshots: &mut self.snapshots,
            editor,
            app,
            idle,
            core_current,
        };
        self.inbox.drain(&mut host);
    }
}
struct Host<'a> {
    snapshots: &'a mut snapshot::Store,
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
    fn execute(&mut self, request: Request) -> ToolResult<Value> {
        request.validate()?;
        if self.editor.template_session.is_some() {
            return Err(ToolError::new(
                ErrorCode::UnsupportedOperation,
                "Template isolation is not supported; return to the main chart",
            ));
        }
        match request {
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
