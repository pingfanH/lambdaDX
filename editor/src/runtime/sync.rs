//! Publish edits before taking a snapshot or touching the native core.

use std::sync::{Arc, mpsc};

use crate::app::{
    maichart,
    types::{ChartDoc, Note, NoteType},
};
use crate::player::engine::JudgeEngine;
use crate::player::state::{
    EditedEngineContext, PadPreviewState, PreparedEditedEngine, RetiredEditedEngine,
};

#[derive(Clone)]
struct Version {
    chart: Arc<ChartDoc>,
    document: u64,
}

impl Version {
    fn matches(&self, app: &PadPreviewState) -> bool {
        self.document == app.document_revision()
            && self.chart.simai_level == app.chart.simai_level
            && self.chart.bpms == app.chart.bpms
            && self.chart.notes == app.chart.notes
            && self.chart.templates == app.chart.templates
            && self.chart.template_instances == app.chart.template_instances
            && self.chart.autoplay_overrides == app.chart.autoplay_overrides
    }
}

#[derive(PartialEq, Eq)]
struct EachInput {
    time: u32,
    lane: u8,
    star: bool,
    tapless: bool,
    trails: usize,
}

impl From<&Note> for EachInput {
    fn from(note: &Note) -> Self {
        Self {
            time: note.time.to_bits(),
            lane: note.lane,
            star: note.note_type == NoteType::Slide,
            tapless: note.is_tapless,
            trails: note.slide.len(),
        }
    }
}

enum Retired {
    Engine(RetiredEditedEngine),
    Prepared(PreparedEditedEngine),
}

impl Retired {
    fn dispose(self) {
        match self {
            Self::Engine(engine) => drop(engine),
            Self::Prepared(prepared) => drop(prepared),
        }
    }
}

enum Task {
    Build {
        version: Version,
        context: EditedEngineContext,
    },
    Restore {
        version: Version,
        prepared: PreparedEditedEngine,
        context: EditedEngineContext,
    },
    Dispose,
}

struct Work {
    retired: Vec<Retired>,
    task: Task,
}

enum Completion {
    Ready {
        version: Version,
        result: Result<PreparedEditedEngine, String>,
    },
    Disposed,
}

fn build(version: &Version, context: EditedEngineContext) -> Result<PreparedEditedEngine, String> {
    let level = version.chart.simai_level.max(1);
    let chart = crate::app::templates::expand(&version.chart);
    let compiled = crate::core_chart::compile(&chart, level)?;
    let mut engine = JudgeEngine::load(&compiled.text, level)?;
    engine.bind_editor_chart(compiled.note_ids, compiled.slide_positions)?;
    PreparedEditedEngine::new(&chart, engine, compiled.text, level, context)
}

/// One worker and one outstanding job: Lean serializes every native operation,
/// including destruction, so old sessions must leave the UI before a build.
pub struct ChartSync {
    version: Version,
    each_inputs: Vec<EachInput>,
    document: u64,
    current: bool,
    failed: bool,
    busy: bool,
    ready: Option<Completion>,
    retired: Vec<Retired>,
    sender: mpsc::Sender<Work>,
    receiver: mpsc::Receiver<Completion>,
}

impl ChartSync {
    pub fn new(chart: &mut ChartDoc) -> Self {
        let (sender, requests) = mpsc::channel::<Work>();
        let (responses, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("editor-core-sync".into())
            .spawn(move || {
                while let Ok(work) = requests.recv() {
                    for retired in work.retired {
                        retired.dispose();
                    }
                    let completion = match work.task {
                        Task::Build { version, context } => {
                            let result = build(&version, context);
                            Completion::Ready { version, result }
                        }
                        Task::Restore {
                            version,
                            mut prepared,
                            context,
                        } => {
                            let result = prepared.restore(context).map(|()| prepared);
                            Completion::Ready { version, result }
                        }
                        Task::Dispose => Completion::Disposed,
                    };
                    if responses.send(completion).is_err() {
                        break;
                    }
                }
            })
            .expect("editor core worker must start");
        Self::with_channels(chart, sender, receiver)
    }

    fn with_channels(
        chart: &mut ChartDoc,
        sender: mpsc::Sender<Work>,
        receiver: mpsc::Receiver<Completion>,
    ) -> Self {
        maichart::recompute_each(&mut chart.notes);
        Self {
            each_inputs: chart.notes.iter().map(EachInput::from).collect(),
            version: Version {
                chart: Arc::new(chart.clone()),
                document: 0,
            },
            document: 0,
            current: true,
            failed: false,
            busy: false,
            ready: None,
            retired: Vec::new(),
            sender,
            receiver,
        }
    }

    fn detach(&mut self, app: &mut PadPreviewState) {
        if let Some(engine) = app.detach_edited_engine() {
            self.retired.push(Retired::Engine(engine));
        }
    }

    fn send(&mut self, task: Task) -> Result<(), String> {
        self.sender
            .send(Work {
                retired: std::mem::take(&mut self.retired),
                task,
            })
            .map_err(|_| "core update worker stopped".to_string())?;
        self.busy = true;
        Ok(())
    }

    /// Frame-critical path: invalidate old state and poll, never wait or clone
    /// the chart. The live document is already sufficient to draw the note.
    pub fn update(
        &mut self,
        app: &mut PadPreviewState,
        gesture_active: bool,
    ) -> Result<bool, String> {
        app.refresh_playback_chart();
        if self.document != app.document_revision() {
            self.document = app.document_revision();
            self.version = Version {
                chart: Arc::new(app.chart.clone()),
                document: self.document,
            };
            self.current = true;
            self.failed = false;
        }
        if !self.version.matches(app) {
            self.current = false;
            self.detach(app);
        }
        if self.busy {
            match self.receiver.try_recv() {
                Ok(completion) => {
                    self.busy = false;
                    self.ready = Some(completion);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.busy = false;
                    return Err("core update worker stopped".into());
                }
            }
        }
        if !gesture_active {
            if let Some(Completion::Ready { version, result }) = self.ready.take() {
                if version.matches(app) {
                    let prepared = match result {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            self.failed = true;
                            return Err(error);
                        }
                    };
                    if app.accepts_edited_engine(prepared.context) {
                        app.install_edited_engine(prepared);
                        self.version = version;
                        self.current = true;
                    } else {
                        self.send(Task::Restore {
                            version,
                            prepared,
                            context: app.edited_engine_context(),
                        })?;
                    }
                } else if let Ok(prepared) = result {
                    self.retired.push(Retired::Prepared(prepared));
                }
            }
        }
        Ok(self.current && !gesture_active && self.version.matches(app))
    }

    /// Called only after next_frame has presented the confirmed edit. Coalesce
    /// continuous changes and take at most one snapshot per outstanding build.
    pub fn schedule(
        &mut self,
        app: &mut PadPreviewState,
        gesture_active: bool,
    ) -> Result<(), String> {
        self.refresh_each(&mut app.chart);
        if self.busy || self.ready.is_some() || gesture_active {
            return Ok(());
        }
        if self.version.matches(app) && (self.current || self.failed) {
            if !self.retired.is_empty() {
                self.send(Task::Dispose)?;
            }
            return Ok(());
        }
        self.current = false;
        self.failed = false;
        self.detach(app);
        self.version = Version {
            chart: Arc::new(app.chart.clone()),
            document: app.document_revision(),
        };
        #[cfg(any(feature = "backend-lean", feature = "backend-rust"))]
        self.send(Task::Build {
            version: self.version.clone(),
            context: app.edited_engine_context(),
        })?;
        #[cfg(not(any(feature = "backend-lean", feature = "backend-rust")))]
        {
            let mut cues = crate::player::cues::CueTrack::from_chart(app.playback_chart());
            cues.reset(app.timeline_time().max(0.0));
            app.cue_track = Some(cues);
            self.current = true;
        }
        Ok(())
    }

    fn refresh_each(&mut self, chart: &mut ChartDoc) {
        if self.each_inputs.len() == chart.notes.len()
            && self
                .each_inputs
                .iter()
                .zip(&chart.notes)
                .all(|(saved, note)| *saved == EachInput::from(note))
        {
            return;
        }
        maichart::recompute_each(&mut chart.notes);
        self.each_inputs.clear();
        self.each_inputs
            .extend(chart.notes.iter().map(EachInput::from));
    }
}

#[cfg(all(test, feature = "backend-lean"))]
mod tests {
    use super::*;
    use crate::state::{EditorState, HistoryEntry, Tool};

    fn fixture() -> PadPreviewState {
        let source = "&inote_6=(120){4}1h[2:1],2-5[4:1],3,E\n";
        let chart = crate::app::maidata::from_maidata_level(source, 6).unwrap();
        let mut pad = super::super::initialize_player(
            PadPreviewState::new_at(chart, None, None, 0.0, None),
            Some(source.into()),
            Some(6),
        );
        pad.forced_time = Some(0.0);
        pad
    }

    fn controlled(
        pad: &mut PadPreviewState,
    ) -> (ChartSync, mpsc::Receiver<Work>, mpsc::Sender<Completion>) {
        let (sender, requests) = mpsc::channel();
        let (responses, receiver) = mpsc::channel();
        (
            ChartSync::with_channels(&mut pad.chart, sender, receiver),
            requests,
            responses,
        )
    }

    fn complete(work: Work) -> Completion {
        for retired in work.retired {
            retired.dispose();
        }
        match work.task {
            Task::Build { version, context } => {
                let result = build(&version, context);
                Completion::Ready { version, result }
            }
            Task::Restore {
                version,
                mut prepared,
                context,
            } => {
                let result = prepared.restore(context).map(|()| prepared);
                Completion::Ready { version, result }
            }
            Task::Dispose => Completion::Disposed,
        }
    }

    #[test]
    #[ignore = "placement latency probe; run without concurrent tests"]
    fn placement_latency_matches_the_live_document_path() {
        use std::time::Instant;

        for count in [32, 512, 2048] {
            for keyboard in [false, true] {
                let mut pad = fixture();
                let seed = pad.chart.notes.clone();
                pad.chart.notes = (0..count)
                    .map(|index| {
                        let mut note = seed[index % seed.len()].clone();
                        note.id = index as u64 + 1;
                        note.time = 1.0 + index as f32 * 0.25;
                        note
                    })
                    .collect();
                let version = Version {
                    chart: Arc::new(pad.chart.clone()),
                    document: 0,
                };
                let prepared = build(&version, pad.edited_engine_context()).unwrap();
                drop(pad.detach_edited_engine());
                pad.install_edited_engine(prepared);
                let (mut sync, requests, _responses) = controlled(&mut pad);
                let original_snapshot = Arc::clone(&sync.version.chart);
                let mut editor = EditorState {
                    tool: Tool::Tap,
                    ..Default::default()
                };
                editor.keyboard.cursor.measure = 4.0;
                editor.keyboard.cursor.zone = 7;
                let mut input_samples = Vec::new();
                let mut frame_samples = Vec::new();
                let mut first_frame = std::time::Duration::ZERO;
                for iteration in 0..129 {
                    let start = Instant::now();
                    if keyboard {
                        crate::keyboard::edit::place(&mut editor, &mut pad.chart);
                    } else {
                        editor.place_note(&mut pad.chart, 4.0, 7);
                    }
                    let input = start.elapsed();
                    assert!(!sync.update(&mut pad, false).unwrap());
                    let frame = start.elapsed();
                    if iteration == 0 {
                        first_frame = frame;
                    } else {
                        input_samples.push(input);
                        frame_samples.push(frame);
                    }
                    assert_eq!(pad.chart.notes.len(), count + 1);
                    assert!(Arc::ptr_eq(&sync.version.chart, &original_snapshot));
                    pad.chart.notes.pop();
                }
                assert!(requests.try_recv().is_err());
                input_samples.sort_unstable();
                frame_samples.sort_unstable();
                eprintln!(
                    "placement keyboard={keyboard} notes={count} first={first_frame:?} input_p50={:?} frame_p50={:?} frame_p95={:?}",
                    input_samples[64], frame_samples[64], frame_samples[121],
                );
            }
        }
    }

    #[test]
    fn placement_frame_neither_snapshots_nor_waits_for_the_core_worker() {
        let mut pad = fixture();
        let (mut sync, requests, responses) = controlled(&mut pad);
        let original_snapshot = Arc::clone(&sync.version.chart);
        let mut editor = EditorState {
            tool: Tool::Tap,
            ..Default::default()
        };
        let id = editor.place_note(&mut pad.chart, 4.0, 7);
        assert!(!sync.update(&mut pad, false).unwrap());
        assert!(Arc::ptr_eq(&sync.version.chart, &original_snapshot));
        assert!(
            requests.try_recv().is_err(),
            "no work may start before the note is drawn"
        );
        assert!(
            !pad.has_engine(),
            "UI seek/step/drop must not compete with a load"
        );
        assert!(pad.chart.notes.iter().any(|note| note.id == id));
        assert!(
            matches!(editor.undo.last(), Some(HistoryEntry::RemoveNote(saved)) if *saved == id)
        );
        sync.schedule(&mut pad, false).unwrap();
        let pending = requests.try_recv().unwrap();
        assert_eq!(pending.retired.len(), 1);
        for time in [0.25, 0.75, 0.5] {
            pad.try_seek_to(time).unwrap();
            assert!(!sync.update(&mut pad, false).unwrap());
            sync.schedule(&mut pad, false).unwrap();
            assert!(
                requests.try_recv().is_err(),
                "only one native operation may be outstanding"
            );
        }
        responses.send(complete(pending)).ok().unwrap();
        assert!(!sync.update(&mut pad, false).unwrap());
    }

    #[test]
    fn undo_before_scheduling_restores_the_original_core() {
        let mut pad = fixture();
        let (mut sync, requests, responses) = controlled(&mut pad);
        let mut editor = EditorState {
            tool: Tool::Tap,
            ..Default::default()
        };
        editor.place_note(&mut pad.chart, 4.0, 7);
        assert!(!sync.update(&mut pad, false).unwrap());
        editor.undo(&mut pad.chart);
        assert!(!sync.update(&mut pad, false).unwrap());
        sync.schedule(&mut pad, false).unwrap();
        let work = requests.try_recv().unwrap();
        assert!(matches!(work.task, Task::Build { .. }));
        responses.send(complete(work)).ok().unwrap();
        assert!(sync.update(&mut pad, false).unwrap());
        assert!(pad.has_engine());
    }

    #[test]
    fn each_visuals_refresh_while_a_core_build_is_pending() {
        let mut pad = fixture();
        let (mut sync, requests, _responses) = controlled(&mut pad);
        let mut editor = EditorState {
            tool: Tool::Tap,
            ..Default::default()
        };
        let first = editor.place_note(&mut pad.chart, 4.0, 7);
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        let _pending = requests.try_recv().unwrap();
        let second = editor.place_note(&mut pad.chart, 4.0, 8);
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        for id in [first, second] {
            assert!(
                pad.chart
                    .notes
                    .iter()
                    .find(|note| note.id == id)
                    .unwrap()
                    .is_each
            );
        }
        pad.chart.notes.pop();
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        assert!(
            !pad.chart
                .notes
                .iter()
                .find(|note| note.id == first)
                .unwrap()
                .is_each
        );
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn stale_completions_cannot_replace_newer_edits() {
        let mut pad = fixture();
        let (mut sync, requests, responses) = controlled(&mut pad);
        pad.chart.notes.last_mut().unwrap().lane = 4;
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        let first = requests.try_recv().unwrap();
        pad.chart.notes.last_mut().unwrap().lane = 8;
        responses.send(complete(first)).ok().unwrap();
        assert!(!sync.update(&mut pad, false).unwrap());
        assert!(!pad.has_engine());
        sync.schedule(&mut pad, false).unwrap();
        let newest = requests.try_recv().unwrap();
        assert_eq!(
            newest.retired.len(),
            1,
            "dispose stale native sessions on the worker"
        );
        responses.send(complete(newest)).ok().unwrap();
        assert!(sync.update(&mut pad, false).unwrap());
        let spec = pad.judge_engine.as_ref().unwrap().chart_spec().unwrap();
        assert_eq!(spec.taps[0].slot, crate::core::types::OuterSlot::S8);
    }

    #[test]
    fn completion_reseeks_with_latest_clock_and_autoplay_settings() {
        let mut pad = fixture();
        let (mut sync, requests, responses) = controlled(&mut pad);
        pad.chart.notes.last_mut().unwrap().lane = 8;
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        let completion = complete(requests.try_recv().unwrap());
        pad.autoplay = true;
        pad.autoplay_direct_core = true;
        pad.try_seek_to(0.75).unwrap();
        responses.send(completion).ok().unwrap();
        assert!(!sync.update(&mut pad, false).unwrap());
        assert!(!pad.has_engine());
        let restore = requests.try_recv().unwrap();
        assert!(matches!(&restore.task, Task::Restore { context, .. } if context.time == 0.75));
        responses.send(complete(restore)).ok().unwrap();
        assert!(sync.update(&mut pad, false).unwrap());
        assert_eq!(pad.timeline_view_time, 0.75);
        assert!(pad.autoplay && pad.autoplay_direct_core);
        assert!(
            !pad.hidden_notes.contains(&pad.chart.notes[0].id),
            "ongoing hold remains visible"
        );
        assert!(
            !pad.hidden_notes.contains(&pad.chart.notes[1].id),
            "ongoing slide remains visible"
        );
    }

    #[test]
    fn reopening_the_same_chart_invalidates_pending_results() {
        let mut pad = fixture();
        let (mut sync, requests, responses) = controlled(&mut pad);
        pad.chart.notes.last_mut().unwrap().lane = 8;
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        let completion = complete(requests.try_recv().unwrap());
        let source = "&inote_6=(120){4}1h[2:1],2-5[4:1],8,E\n";
        let chart = pad.chart.clone();
        pad.load_chart(chart, None, None, Some(source.into()), Some(6));
        responses.send(completion).ok().unwrap();
        assert!(sync.update(&mut pad, false).unwrap());
        assert_eq!(
            sync.retired.len(),
            1,
            "identical content from an old document must not be installed"
        );
        assert_eq!(pad.timeline_view_time, 0.0);
    }

    #[test]
    fn failed_builds_do_not_loop_and_the_next_valid_edit_recovers() {
        let mut pad = fixture();
        let (mut sync, requests, responses) = controlled(&mut pad);
        pad.chart.notes.last_mut().unwrap().lane = 0;
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        responses
            .send(complete(requests.try_recv().unwrap()))
            .ok()
            .unwrap();
        assert!(sync.update(&mut pad, false).is_err());
        sync.schedule(&mut pad, false).unwrap();
        assert!(requests.try_recv().is_err());
        pad.chart.notes.last_mut().unwrap().lane = 7;
        sync.update(&mut pad, false).unwrap();
        sync.schedule(&mut pad, false).unwrap();
        responses
            .send(complete(requests.try_recv().unwrap()))
            .ok()
            .unwrap();
        assert!(sync.update(&mut pad, false).unwrap());
    }
}
