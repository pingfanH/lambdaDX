use super::*;
use crate::app::types::{self, ChartDoc, Note as ChartNote, NoteType, zone};
use lambdadx_automation::bridge::Session;
use std::{collections::{BTreeSet, HashSet, VecDeque}, sync::mpsc, time::{Duration, Instant}};

const TTL: Duration = Duration::from_secs(600);
pub(super) struct Proposal {
    pub result: ProposalResult,
    session: Session,
    payload: Value,
    operation: String,
    created: Instant,
    baseline: Option<snapshot::Snapshot>,
    candidate: Option<ChartDoc>,
    pub text: Option<String>,
    format: Option<mpsc::Receiver<Result<String, String>>>,
    source: Option<crate::io::ChartSource>,
    auto_save: bool,
    previewing: bool,
    #[cfg(feature = "editor-agent")]
    cursor: Option<lambdadx_automation::agent::cursor::Context>,
}
impl Proposal {
    pub fn immediate(&self) -> bool {
        #[cfg(feature = "editor-agent")]
        { return self.cursor.is_some(); }
        #[cfg(not(feature = "editor-agent"))]
        { false }
    }
    pub fn source_diff(&self) -> Option<(&str, &str)> { Some((self.baseline.as_ref()?.text.as_deref()?, self.text.as_deref()?)) }
    pub fn previewing(&self) -> bool { self.previewing }
    fn pending(&self) -> bool { matches!(self.result.state, ProposalState::Preparing | ProposalState::AwaitingApproval) }
    fn finish(&mut self, state: ProposalState, error: Option<ToolError>) {
        self.result.state = state; self.result.error = error;
        self.baseline = None; self.candidate = None; self.text = None; self.format = None;
        self.previewing = false;
    }
}
#[derive(Default)]
pub(super) struct Store {
    records: VecDeque<Proposal>, next: u64,
    #[cfg(feature = "editor-agent")]
    cursor_grant: Option<(Session, lambdadx_automation::agent::cursor::Context)>,
}
fn err(code: ErrorCode, message: &str) -> ToolError { ToolError::new(code, message) }
pub(super) fn matches(s: &snapshot::Snapshot, editor: &EditorState, app: &PadPreviewState) -> bool {
    s.chart == app.chart && !editor.simai_editor.modified()
        && s.text.as_deref() == editor.simai_editor.automation_source(&app.chart).0.then_some(editor.simai_editor.text.as_str())
}
impl Store {
    #[cfg(feature = "editor-agent")]
    pub fn arm_cursor(&mut self, session: Session, context: lambdadx_automation::agent::cursor::Context) {
        self.cursor_grant = Some((session, context));
    }
    pub fn preview(&mut self, app: &mut PadPreviewState, idle: bool) {
        // Terminal proposals (reject, stop, expiry or document conflict) remove
        // only the playback layer; the canonical chart is never rolled back.
        if app.has_chart_preview() && !self.pending().is_some_and(|p| p.previewing) {
            app.set_chart_preview(None);
        }
        if !idle || app.mode != types::Mode::Idle { return; }
        let Some(p) = self.records.iter_mut().find(|p| p.immediate() && !p.previewing
            && p.result.state == ProposalState::AwaitingApproval) else { return; };
        app.set_chart_preview(p.candidate.clone());
        p.previewing = true;
    }
    pub fn pending(&self) -> Option<&Proposal> { self.records.iter().find(|p| p.pending()) }
    pub fn status(&self, id: &str, session: &Session) -> ToolResult<ProposalResult> {
        self.records.iter().find(|p| p.result.proposal_id == id && p.session.id() == session.id())
            .map(|p| p.result.clone()).ok_or_else(|| err(ErrorCode::DocumentChanged, "Proposal expired or belongs to another session"))
    }
    pub fn create(&mut self, request: ProposeRequest, session: Session, snapshots: &snapshot::Store, editor: &EditorState, app: &PadPreviewState, idle: bool) -> ToolResult<ProposalResult> {
        request.validate()?;
        #[cfg(feature = "editor-agent")]
        let cursor = self.cursor_grant.as_ref().filter(|(owner, _)| owner.id() == session.id() && owner.active()).map(|(_, c)| c.clone());
        #[cfg(feature = "editor-agent")]
        if let Some(context) = &cursor {
            if request.target != context.target || request.edits.iter().any(|edit| !matches!(edit, Edit::AddNotes { notes } if notes.iter().all(|n| n.measure >= context.measure))) {
                return Err(err(ErrorCode::PermissionDenied, "Cursor insertion can only add notes at the pinned destination"));
            }
        }
        let payload = serde_json::to_value(&request).map_err(|e| ToolError::invalid(e.to_string()))?;
        if let Some(p) = self.records.iter().find(|p| p.session.id() == session.id() && p.operation == request.operation_id) {
            return if p.payload == payload { Ok(p.result.clone()) } else { Err(ToolError::invalid("operation_id already used with different arguments")) };
        }
        if self.pending().is_some() { return Err(err(ErrorCode::EditorBusy, "Another proposal is awaiting review")); }
        if !idle || editor.file_dialog_active || editor.keyboard.ime || editor.keyboard.owns_input() || app.mode != types::Mode::Idle {
            return Err(err(ErrorCode::EditorBusy, "Pause playback and finish active input before proposing"));
        }
        if editor.simai_editor.modified() { return Err(err(ErrorCode::UnappliedDraft, "Apply or discard the Simai draft first")); }
        let baseline = snapshots.get(&request.target)?;
        if !matches(baseline, editor, app) { return Err(err(ErrorCode::DocumentChanged, "Document differs from proposal snapshot")); }
        if baseline.text.is_some() && !baseline.matches_chart { return Err(err(ErrorCode::DocumentChanged, "Simai source is not synchronized")); }
        let (candidate, changes, removed_overrides) = construct(&baseline.chart, &request.edits)?;
        #[cfg(feature = "editor-agent")]
        if let Some(context) = &cursor {
            cursor_end(&changes, context.step_measures)?;
        }
        if changes.is_empty() { return Err(ToolError::invalid("Proposal makes no changes")); }
        self.next += 1;
        let result = ProposalResult { proposal_id: format!("proposal-{}-{}", session.id(), self.next), target: request.target, state: if baseline.text.is_some() { ProposalState::Preparing } else { ProposalState::AwaitingApproval }, summary: request.summary, changes, warnings: vec![format!("Removed {removed_overrides} note-owned Autoplay overrides. No file is saved until existing save rules run after user approval.")], error: None, applied_target: None };
        if serde_json::to_vec(&result).map_err(|e| ToolError::invalid(e.to_string()))?.len() > MAX_RESULT_BYTES { return Err(ToolError::invalid("Proposal diff exceeds 64 KiB; use a smaller batch")); }
        while self.records.len() >= 32 { self.records.pop_front(); }
        self.records.push_back(Proposal { result: result.clone(), session, payload, operation: request.operation_id, created: Instant::now(), baseline: Some(baseline.clone()), candidate: Some(candidate), text: None, format: None, source: editor.source.clone(), auto_save: editor.auto_save, previewing: false,
            #[cfg(feature = "editor-agent")]
            cursor,
        });
        Ok(result)
    }
    pub fn tick(&mut self, snapshots: &snapshot::Store, editor: &EditorState, app: &PadPreviewState) {
        for p in &mut self.records {
            if !p.pending() { continue; }
            if !p.session.active() { p.finish(ProposalState::Cancelled, None); continue; }
            if p.created.elapsed() >= TTL { p.finish(ProposalState::Expired, None); continue; }
            if !snapshots.owns_document(&p.result.target) || p.baseline.as_ref().is_none_or(|s| !matches(s, editor, app)) {
                p.finish(ProposalState::Failed, Some(err(ErrorCode::DocumentChanged, "Document changed during review"))); continue;
            }
            if let Some(receiver) = &p.format {
                match receiver.try_recv() {
                    Ok(Ok(text)) => { p.text = Some(text); p.format = None; p.result.state = ProposalState::AwaitingApproval; }
                    Ok(Err(e)) => p.finish(ProposalState::Failed, Some(ToolError::new(ErrorCode::FormatPreservationFailed, e))),
                    Err(mpsc::TryRecvError::Disconnected) => p.finish(ProposalState::Failed, Some(err(ErrorCode::Cancelled, "Preflight worker stopped"))),
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }
        while self.records.front().is_some_and(|p| !p.pending() && p.created.elapsed() >= TTL) { self.records.pop_front(); }
    }
    pub fn schedule(&mut self, editor: &EditorState, app: &mut PadPreviewState, sync: &mut crate::runtime::ChartSync, idle: bool) {
        if !idle || app.mode != types::Mode::Idle { return; }
        let Some(p) = self.records.iter_mut().find(|p| p.result.state == ProposalState::Preparing && p.format.is_none()) else { return; };
        let Some(document) = editor.simai_editor.document() else { p.finish(ProposalState::Failed, Some(err(ErrorCode::DocumentChanged, "Source disappeared"))); return; };
        if let Some(receiver) = sync.preflight(app, document, p.candidate.as_ref().unwrap().clone()) { p.format = Some(receiver); }
    }
    pub fn reject(&mut self) { if let Some(p) = self.records.iter_mut().find(|p| p.pending()) { p.finish(ProposalState::Rejected, None); } }
    pub fn approve(&mut self, editor: &mut EditorState, app: &mut PadPreviewState, snapshots: &mut snapshot::Store, idle: bool, allow_auto_save: bool) -> ToolResult<()> {
        self.tick(snapshots, editor, app);
        let p = self.records.iter_mut().find(|p| p.result.state == ProposalState::AwaitingApproval).ok_or_else(|| err(ErrorCode::DocumentChanged, "No current proposal ready for approval"))?;
        if !idle || editor.file_dialog_active || editor.keyboard.ime || editor.keyboard.owns_input() || editor.template_session.is_some() || app.mode != types::Mode::Idle { return Err(err(ErrorCode::EditorBusy, "Finish active input and pause before applying")); }
        if p.source != editor.source || p.auto_save != editor.auto_save { p.source = editor.source.clone(); p.auto_save = editor.auto_save; return Err(err(ErrorCode::DocumentChanged, "Save target or auto-save setting changed; review and confirm again")); }
        if editor.auto_save && !allow_auto_save { return Err(err(ErrorCode::PermissionDenied, "Explicit auto-save consent required")); }
        #[cfg(feature = "editor-agent")]
        let cursor_end = p.cursor.as_ref().map(|c| cursor_end(&p.result.changes, c.step_measures)).transpose()?;
        let candidate = p.candidate.take().unwrap();
        editor.push_history_entry(app.chart.clone().into());
        editor.placement_sequence.clear(); editor.last_edit = None;
        app.chart = candidate;
        if p.previewing { app.set_chart_preview(None); }
        if let Some(text) = p.text.take() { editor.simai_editor.install_validated(&app.chart, text); }
        editor.selected.retain(|id| app.chart.notes.iter().any(|n| n.id == *id));
        #[cfg(feature = "editor-agent")]
        if let Some(end) = cursor_end {
            editor.selected = p.result.changes.iter().filter(|d| d.before.is_none() && d.after.is_some())
                .map(|d| note_id(&d.id).expect("validated note ID")).collect();
            editor.keyboard.cursor.measure = end;
            editor.keyboard.initialized = true;
            editor.keyboard.detached = true;
            editor.keyboard.range = None;
            editor.editing_slide = None;
            editor.status = Some(format!("已在光标处生成并选中 {} 个音符（可撤销）", editor.selected.len()));
        }
        editor.hovered = None; editor.keyboard.candidate = None;
        if editor.editing_slide.is_some_and(|id| !app.chart.notes.iter().any(|n| n.id == id && n.note_type == NoteType::Slide)) {
            editor.editing_slide = None;
            editor.editing_slide_idx = 0;
        }
        if let Some(note) = editor.editing_slide.and_then(|id| app.chart.notes.iter().find(|n| n.id == id)) {
            editor.editing_slide_idx = editor.editing_slide_idx.min(note.slide.len().saturating_sub(1));
        }
        p.result.applied_target = snapshots.capture(editor, app, false).ok().map(|c| c.target);
        p.finish(ProposalState::Applied, None);
        Ok(())
    }
}
#[cfg(feature = "editor-agent")]
fn cursor_end(changes: &[NoteDiff], step: f64) -> ToolResult<f32> {
    let last = changes.iter().filter(|d| d.before.is_none()).filter_map(|d| d.after.as_ref())
        .map(|n| n.measure).max_by(f64::total_cmp).ok_or_else(|| ToolError::invalid("No new notes"))?;
    let end = number(last + step)?;
    if !step.is_finite() || step <= 0.0 || end as f64 <= last {
        return Err(ToolError::invalid("Cursor cannot advance beyond the inserted notes"));
    }
    Ok(end)
}
fn zone_id(name: &str) -> ToolResult<u8> {
    (1..=33).find(|id| zone::id_to_zone(*id).is_some_and(|z| z.to_string() == name)).ok_or_else(|| ToolError::invalid("Unknown touch/waypoint zone"))
}
fn locate<'a>(chart: &'a mut ChartDoc, id: &str) -> ToolResult<&'a mut ChartNote> {
    let id = note_id(id)?;
    chart.notes.iter_mut().find(|n| n.id == id && n.template_source.is_none()).ok_or_else(|| ToolError::invalid("Unknown authored note ID"))
}
fn position(n: &mut ChartNote, measure: Option<f64>, ring: Option<u8>, touch: Option<&str>) -> ToolResult<()> {
    if ring.is_some() && touch.is_some() { return Err(ToolError::invalid("Choose ring_lane or touch_zone")); }
    let is_touch = types::note_is_touch(n);
    if (is_touch && ring.is_some()) || (!is_touch && touch.is_some()) { return Err(ToolError::invalid("Zone does not match note type")); }
    if let Some(m) = measure { n.time = number(m)?; }
    if let Some(lane) = ring {
        if !(1..=8).contains(&lane) { return Err(ToolError::invalid("ring_lane must be 1..8")); }
        crate::timeline::geometry::move_note(n, n.time, lane as i32 - n.lane as i32);
    }
    if let Some(zone) = touch { n.lane = zone_id(zone)?; }
    Ok(())
}
fn properties(n: &mut ChartNote, p: &NoteProperties) -> ToolResult<()> {
    if let Some(v) = p.hold_duration_measures {
        if !matches!(n.note_type, NoteType::Hold | NoteType::TouchHold) {
            return Err(ToolError::invalid("hold_duration_measures is only valid for Hold or TouchHold"));
        }
        n.hold_duration = number(v)?;
    }
    if let Some(v) = p.hi_speed { n.hi_speed = number(v)?; }
    if let Some(v) = p.is_break { n.is_break = v; }
    if let Some(v) = p.is_ex {
        if matches!(n.note_type, NoteType::Touch | NoteType::TouchHold | NoteType::Slide) {
            return Err(ToolError::invalid("is_ex is only valid for ring notes"));
        }
        n.is_ex = v;
    }
    if let Some(v) = p.is_star { if n.note_type == NoteType::Slide { return Err(ToolError::invalid("Slide star is derived")); } n.is_star = v; }
    if let Some(v) = p.is_fake_rotate { n.is_fake_rotate = v; }
    if let Some(v) = p.is_hanabi {
        if !matches!(n.note_type, NoteType::Tap | NoteType::Hold) {
            return Err(ToolError::invalid("is_hanabi is only valid for tap or hold notes"));
        }
        n.is_hanabi = v;
    }
    if let Some(v) = p.is_tapless {
        if !matches!(n.note_type, NoteType::Tap | NoteType::Hold) {
            return Err(ToolError::invalid("is_tapless is only valid for tap or hold notes"));
        }
        n.is_tapless = v;
    }
    if let Some(slides) = &p.slides {
        if n.note_type != NoteType::Slide {
            return Err(ToolError::invalid("slides are only valid for Slide notes"));
        }
        if slides.len() > 32 { return Err(ToolError::invalid("Too many trails")); }
        n.slide = slides.iter().map(|s| {
            if s.segments.len() > 64 { return Err(ToolError::invalid("Too many slide segments")); }
            let segments = s.segments.iter().map(|seg| {
                if seg.points.len() > 64 { return Err(ToolError::invalid("Too many waypoints")); }
                Ok(types::SlideSegment { shape: serde_json::from_value(Value::String(seg.shape.clone())).map_err(|_| ToolError::invalid("Unknown slide shape"))?, points: seg.points.iter().map(|p| Ok(types::SlidePoint { zone: zone::id_to_zone(zone_id(&p.zone)?).unwrap(), beat_offset: number(p.stored_beat_offset)? })).collect::<ToolResult<_>>()? })
            }).collect::<ToolResult<Vec<_>>>()?;
            Ok(types::Slide { runtime_parts: segments.len().max(1), segments, slide_duration: number(s.delay_measures + s.motion_duration_measures)?, slide_start_delay: number(s.delay_measures)?, slide_is_break: s.is_break })
        }).collect::<ToolResult<_>>()?;
    }
    Ok(())
}
fn construct(base: &ChartDoc, edits: &[Edit]) -> ToolResult<(ChartDoc, Vec<NoteDiff>, usize)> {
    let mut chart = base.clone(); let mut touched = HashSet::new(); let mut deleted = HashSet::new();
    let mut next = base.notes.iter().map(|n| n.id).max().unwrap_or(0);
    for edit in edits {
        match edit {
            Edit::TransformNotes { note_ids, delta_measures, lane_rotation } => for id in note_ids {
                let n = locate(&mut chart, id)?; if !touched.insert(n.id) { return Err(ToolError::invalid("Note targeted more than once")); }
                let time = number(n.time as f64 + delta_measures)?;
                if time < 1.0 { return Err(ToolError::invalid("Transform would precede measure 1")); }
                crate::timeline::geometry::move_note(n, time, *lane_rotation);
            },
            Edit::DeleteNotes { note_ids } => {
                for id in note_ids { let n = locate(&mut chart, id)?; if !touched.insert(n.id) { return Err(ToolError::invalid("Duplicate target")); } deleted.insert(n.id); }
                chart.notes.retain(|n| !deleted.contains(&n.id));
            }
            Edit::UpdateNotes { notes } => for update in notes {
                let n = locate(&mut chart, &update.id)?; if !touched.insert(n.id) { return Err(ToolError::invalid("Duplicate target")); }
                position(n, update.measure, update.ring_lane, update.touch_zone.as_deref())?; properties(n, &update.properties)?;
            },
            Edit::AddNotes { notes } => for input in notes {
                next = next.checked_add(1).ok_or_else(|| ToolError::invalid("Note ID space exhausted"))?;
                let touch = matches!(input.note_type, NoteKind::Touch | NoteKind::TouchHold);
                if (touch && input.touch_zone.is_none()) || (!touch && input.ring_lane.is_none()) { return Err(ToolError::invalid("Explicit lane or touch zone required")); }
                let mut n = ChartNote { id: next, lane: 1, time: 1.0, hi_speed: 1.0, is_touch: touch, note_type: match input.note_type { NoteKind::Tap => NoteType::Tap, NoteKind::Hold => NoteType::Hold, NoteKind::Touch => NoteType::Touch, NoteKind::TouchHold => NoteType::TouchHold, NoteKind::Slide => NoteType::Slide }, ..Default::default() };
                position(&mut n, Some(input.measure), input.ring_lane, input.touch_zone.as_deref())?; properties(&mut n, &input.properties)?; chart.notes.push(n);
            }
        }
    }
    let before = chart.autoplay_overrides.len();
    chart.autoplay_overrides.retain(|o| o.key.is_standalone() || !deleted.contains(&o.key.note_id));
    for o in &chart.autoplay_overrides { if !o.key.is_standalone() && touched.contains(&o.key.note_id) { return Err(err(ErrorCode::UnsupportedOperation, "Editing notes with authored Autoplay tracks is not supported")); } }
    crate::io::validate(&chart).map_err(|e| ToolError::new(ErrorCode::ValidationFailed, e))?;
    crate::app::maichart::recompute_each(&mut chart.notes); crate::app::maichart::mark_double_stars(&mut chart.notes);
    let ids: BTreeSet<_> = base.notes.iter().chain(&chart.notes).map(|n| n.id).collect();
    let before_by_id: std::collections::HashMap<_, _> = base.notes.iter().map(|n| (n.id, n)).collect();
    let after_by_id: std::collections::HashMap<_, _> = chart.notes.iter().map(|n| (n.id, n)).collect();
    let mut changes = Vec::new();
    for id in ids {
        let a = before_by_id.get(&id).copied(); let b = after_by_id.get(&id).copied();
        let before = a.map(query::dto).transpose()?; let after = b.map(query::dto).transpose()?;
        if serde_json::to_value(&before).unwrap() != serde_json::to_value(&after).unwrap() { changes.push(NoteDiff { id: id.to_string(), before, after }); }
    }
    if changes.len() > MAX_IDS {
        return Err(ToolError::invalid("Derived changes exceed the 256-note proposal limit"));
    }
    let snap = snapshot::Snapshot { target: SnapshotRef { document_id: "candidate".into(), snapshot_token: "candidate".into() }, order: (0..chart.notes.len()).collect(), chart: chart.clone(), text: None, draft: false, matches_chart: false, created: Instant::now(), bytes: 0 };
    let report = query::check(&snap, &CheckRequest { schema_version: 1, target: snap.target.clone(), range: None, level: CheckLevel::Structure, offset: 0, limit: 128 }, false)?;
    if report.total != 0 { return Err(err(ErrorCode::ValidationFailed, &report.diagnostics[0].message)); }
    let removed = before - chart.autoplay_overrides.len();
    Ok((chart, changes, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (Store, snapshot::Store, Session, EditorState, PadPreviewState) {
        let mut chart = crate::app::chart::fallback_chart();
        chart.notes = vec![ChartNote { id: 1, time: 1.0, lane: 1, hi_speed: 1.0, ..Default::default() }, ChartNote { id: 2, time: 2.0, lane: 2, hi_speed: 1.0, ..Default::default() }];
        chart.templates.clear(); chart.template_instances.clear(); chart.autoplay_overrides.clear();
        let editor = EditorState::default();
        let app = PadPreviewState::new_at(chart, None, None, 0.0, None);
        let mut snapshots = snapshot::Store::new(); snapshots.observe(&editor, &app);
        (Store::default(), snapshots, Session::default(), editor, app)
    }
    fn request(snapshots: &mut snapshot::Store, editor: &EditorState, app: &PadPreviewState, edits: Value) -> ProposeRequest {
        let ctx = snapshots.capture(editor, app, false).unwrap();
        serde_json::from_value(json!({"schema_version":1,"target":ctx.target,"operation_id":"op","summary":"test","edits":edits})).unwrap()
    }
    #[test]
    fn mixed_batch_is_atomic_and_validates_properties() {
        let (_, _, _, _, app) = fixture();
        let edits: Vec<Edit> = serde_json::from_value(json!([
            {"kind":"transform_notes","note_ids":["1"],"delta_measures":0.25,"lane_rotation":1},
            {"kind":"update_notes","notes":[{"id":"2","ring_lane":4,"properties":{"is_break":true}}]},
            {"kind":"add_notes","notes":[{"note_type":"tap","measure":3,"ring_lane":5}]}
        ])).unwrap();
        let before = app.chart.clone();
        let (candidate, changes, _) = construct(&before, &edits).unwrap();
        assert_eq!(changes.len(), 3); assert_eq!(candidate.notes[0].time, 1.25); assert_eq!(candidate.notes[0].lane, 2);
        assert!(candidate.notes[1].is_break); assert_eq!(candidate.notes[2].id, 3);
        let (deleted, _, _) = construct(&candidate, &[Edit::DeleteNotes { note_ids: vec!["3".into()] }]).unwrap();
        assert_eq!(deleted.notes.len(), 2);
        let mut bad = edits.clone(); bad.push(Edit::DeleteNotes { note_ids: vec!["999".into()] });
        assert!(construct(&before, &bad).is_err()); assert_eq!(app.chart, before);
        let invalid: Vec<Edit> = serde_json::from_value(json!([{"kind":"update_notes","notes":[{"id":"1","properties":{"hold_duration_measures":0.5}}]}])).unwrap();
        assert!(construct(&before, &invalid).is_err());
    }
    #[test]
    fn lifecycle_conflict_consent_idempotence_and_session_isolation() {
        let (mut store, mut snapshots, session, mut editor, mut app) = fixture();
        let request = request(&mut snapshots, &editor, &app, json!([{"kind":"delete_notes","note_ids":["1"]}]));
        let p = store.create(request.clone(), session.clone(), &snapshots, &editor, &app, true).unwrap();
        assert_eq!(store.create(request.clone(), session.clone(), &snapshots, &editor, &app, true).unwrap().proposal_id, p.proposal_id);
        assert!(store.status(&p.proposal_id, &Session::default()).is_err());
        let mut reused = request.clone(); reused.summary = "different".into();
        assert!(store.create(reused, session.clone(), &snapshots, &editor, &app, true).is_err());
        editor.auto_save = true;
        assert!(store.approve(&mut editor, &mut app, &mut snapshots, true, false).is_err());
        assert_eq!(store.approve(&mut editor, &mut app, &mut snapshots, true, false).unwrap_err().code, ErrorCode::PermissionDenied);
        store.reject(); assert_eq!(app.chart.notes.len(), 2); assert!(!editor.dirty);
        let mut next = request.clone(); next.operation_id = "next".into();
        let p = store.create(next, session.clone(), &snapshots, &editor, &app, true).unwrap();
        app.chart.notes[0].lane = 8; store.tick(&snapshots, &editor, &app);
        assert_eq!(store.status(&p.proposal_id, &session).unwrap().state, ProposalState::Failed);
    }
    #[test]
    fn proposal_owns_baseline_past_snapshot_eviction_and_expires_or_cancels() {
        let (mut store, mut snapshots, session, editor, app) = fixture();
        let r = request(&mut snapshots, &editor, &app, json!([{"kind":"delete_notes","note_ids":["1"]}]));
        let p = store.create(r, session.clone(), &snapshots, &editor, &app, true).unwrap();
        for _ in 0..10 { snapshots.capture(&editor, &app, false).unwrap(); }
        store.tick(&snapshots, &editor, &app);
        assert_eq!(store.status(&p.proposal_id, &session).unwrap().state, ProposalState::AwaitingApproval);
        session.cancel(); store.tick(&snapshots, &editor, &app);
        assert_eq!(store.status(&p.proposal_id, &session).unwrap().state, ProposalState::Cancelled);
        let session = Session::default();
        let r = request(&mut snapshots, &editor, &app, json!([{"kind":"delete_notes","note_ids":["1"]}]));
        let p = store.create(r, session.clone(), &snapshots, &editor, &app, true).unwrap();
        store.records.back_mut().unwrap().created = Instant::now() - TTL;
        // Keep an earlier terminal record so cleanup does not remove this one.
        store.tick(&snapshots, &editor, &app);
        assert_eq!(store.status(&p.proposal_id, &session).unwrap().state, ProposalState::Expired);
    }
    #[cfg(feature = "backend-lean")]
    #[test]
    fn source_preflight_and_one_undo_redo_preserve_exact_text() {
        let (mut store, mut snapshots, session, mut editor, mut app) = fixture();
        let text = "|| 注释 🎵\r\n(120){4}1,2h[1:2],3h[2:9],4-6[4:1],E\r\n";
        app.chart = crate::simai_editor::parse_draft(text, 120.0, 6).unwrap();
        crate::app::maichart::assign_note_ids(&mut app.chart.notes);
        editor.simai_editor.load(&app.chart, text.into());
        snapshots.observe(&editor, &app);
        let before = app.chart.clone();
        let id = before.notes[0].id.to_string();
        let r = request(&mut snapshots, &editor, &app, json!([{"kind":"transform_notes","note_ids":[id],"delta_measures":0.0,"lane_rotation":1}]));
        let p = store.create(r, session.clone(), &snapshots, &editor, &app, true).unwrap();
        let candidate = store.records.back().unwrap().candidate.as_ref().unwrap();
        let after_text = editor.simai_editor.document().unwrap().update_strict(candidate).unwrap();
        assert!(after_text.contains("|| 注释 🎵\r\n")); assert!(after_text.contains("[1:2]")); assert!(after_text.contains("[2:9]"));
        let (tx, rx) = mpsc::channel(); tx.send(Ok(after_text.clone())).unwrap();
        store.records.back_mut().unwrap().format = Some(rx);
        store.tick(&snapshots, &editor, &app);
        store.approve(&mut editor, &mut app, &mut snapshots, true, true).unwrap();
        assert_eq!(store.status(&p.proposal_id, &session).unwrap().state, ProposalState::Applied);
        assert_eq!(editor.undo.len(), 1); let after = app.chart.clone();
        editor.undo(&mut app.chart); assert_eq!(app.chart, before); assert_eq!(editor.simai_editor.text, text);
        editor.redo(&mut app.chart); assert_eq!(app.chart, after); assert_eq!(editor.simai_editor.text, after_text);
    }
}
