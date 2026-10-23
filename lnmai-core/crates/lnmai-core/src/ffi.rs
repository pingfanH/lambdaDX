//! FFI JSON API and process-local session registry.
//!
//! Mirrors the exported surface of `LnmaiCore/FFI.lean`: every entry point
//! returns a JSON envelope (`{"ok":true,"result":...}` or
//! `{"ok":false,"error":{...}}`). The Lean implementation stores sessions in a
//! `Std.Mutex`-guarded registry; this port guards the same three registry
//! fields together, including handle allocation and seek origins.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::areas::{ButtonZone, OuterSlot, SensorArea};
use crate::chart_loader::ChartSpec;
use crate::events::{AudioCommand, RenderCommand};
use crate::input_model::{GameState, TimedInputBatch, TimedInputEvent};
use crate::lifecycle::{TapFamilyNote, TapState};
use crate::natural::Natural;
use crate::runtime_score::RuntimeScoreState;
use crate::time::{Duration, TimePoint};
use crate::types::{JudgeEvent, JudgeEventKind, JudgeGrade, RuntimePos};

/// `LnmaiCore.FFI.RuntimeStepResult`.
#[derive(Debug, Clone, Default)]
pub struct RuntimeStepResult {
    pub state: GameState,
    pub events: Vec<JudgeEvent>,
    pub audio_commands: Vec<AudioCommand>,
    pub render_commands: Vec<RenderCommand>,
    pub terminated: bool,
}

impl RuntimeStepResult {
    pub fn to_json(&self) -> Value {
        crate::json_encode::Encode::to_json(self)
    }
}

/// `LnmaiCore.FFI.RuntimeStepLightResult`.
#[derive(Debug, Clone, Default)]
pub struct RuntimeStepLightResult {
    pub events: Vec<JudgeEvent>,
    pub audio_commands: Vec<AudioCommand>,
    pub render_commands: Vec<RenderCommand>,
    pub score: RuntimeScoreState,
    pub current_time: TimePoint,
    pub terminated: bool,
}

impl RuntimeStepLightResult {
    pub fn to_json(&self) -> Value {
        crate::json_encode::Encode::to_json(self)
    }
}

/// `LnmaiCore.FFI.LoadedChartSummary`; counts remain unbounded when decoded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadedChartSummary {
    pub tap_count: Natural,
    pub hold_count: Natural,
    pub touch_count: Natural,
    pub touch_hold_count: Natural,
    pub slide_head_count: Natural,
    pub slide_count: Natural,
}

impl LoadedChartSummary {
    pub fn from_chart(spec: &ChartSpec) -> Self {
        Self {
            tap_count: spec.taps.len().into(),
            hold_count: spec.holds.len().into(),
            touch_count: spec.touches.len().into(),
            touch_hold_count: spec.touch_holds.len().into(),
            slide_head_count: spec.slide_heads.len().into(),
            slide_count: spec.slides.len().into(),
        }
    }

    pub fn to_json(&self) -> Value {
        crate::json_encode::Encode::to_json(self)
    }
}

// ---------------------------------------------------------------------------
// Envelope helpers
// ---------------------------------------------------------------------------

pub fn ok_json(payload: Value) -> String {
    crate::lean_json::compress(&json!({ "ok": true, "result": payload }))
}

pub fn error_json(code: &str, message: &str) -> String {
    crate::lean_json::compress(
        &json!({ "ok": false, "error": { "code": code, "message": message } }),
    )
}

// ---------------------------------------------------------------------------
// Scalar encoders
// ---------------------------------------------------------------------------

pub fn natural_json(n: impl std::borrow::Borrow<crate::natural::Natural>) -> Value {
    let n = n.borrow();
    Value::Number(n.to_string().parse().expect("natural is a JSON number"))
}

pub fn integer_json(n: impl std::borrow::Borrow<crate::integer::Integer>) -> Value {
    let n = n.borrow();
    Value::Number(n.to_string().parse().expect("integer is a JSON number"))
}

/// Boundary encoder for a raw num-bigint integer. Only the JSON/FFI boundary
/// and its tests use it; runtime arithmetic goes through Integer.
pub fn bigint_json(n: &num_bigint::BigInt) -> Value {
    Value::Number(n.to_string().parse().expect("integer is a JSON number"))
}

/// Boundary encoder for a raw num-bigint natural, used by the JSON boundary
/// tests only.
pub fn biguint_json(n: &num_bigint::BigUint) -> Value {
    Value::Number(n.to_string().parse().expect("natural is a JSON number"))
}

fn duration_json(d: Duration) -> Value {
    json!(d.to_micros())
}

fn time_point_json(t: TimePoint) -> Value {
    json!(t.to_micros())
}

pub fn grade_str(g: JudgeGrade) -> &'static str {
    crate::types::judge_grade_code(g)
}

fn event_kind_str(k: JudgeEventKind) -> &'static str {
    match k {
        JudgeEventKind::Tap => "Tap",
        JudgeEventKind::Hold => "Hold",
        JudgeEventKind::Slide => "Slide",
        JudgeEventKind::Touch => "Touch",
        JudgeEventKind::Break => "Break",
    }
}

fn sensor_str(a: SensorArea) -> &'static str {
    crate::areas::sensor_area_code(a)
}

fn button_str(z: ButtonZone) -> &'static str {
    crate::areas::button_zone_code(z)
}

fn slot_str(s: OuterSlot) -> &'static str {
    crate::areas::outer_slot_code(s)
}

fn pos_json(p: RuntimePos) -> Value {
    match p {
        RuntimePos::Button(z) => json!({ "button": button_str(z) }),
        RuntimePos::Sensor(a) => json!({ "sensor": sensor_str(a) }),
    }
}

pub fn judge_event_json(e: &JudgeEvent) -> Value {
    json!({
        "kind": event_kind_str(e.kind),
        "phase": match e.phase { crate::types::JudgePhase::Head => "head", crate::types::JudgePhase::Tail => "tail" },
        "isEX": e.is_ex,
        "grade": grade_str(e.grade),
        "diff": duration_json(e.diff.clone()),
        "position": pos_json(e.position),
        "noteIndex": e.note_index,
        "isBreak": e.is_break,
        "multiple": e.multiple,
    })
}

pub fn audio_cmd_json(c: &crate::events::AudioCommand) -> Value {
    match c {
        crate::events::AudioCommand::PlayJudgeSfx {
            kind,
            grade,
            is_break,
            at_time,
            note_index,
        } => json!({
            "PlayJudgeSfx": {
                "kind": event_kind_str(*kind),
                "grade": grade_str(*grade),
                "isBreak": is_break,
                "atTime": time_point_json(at_time.clone()),
                "noteIndex": note_index,
            }
        }),
        crate::events::AudioCommand::PlayTouchHoldBody {
            note_index,
            at_time,
        } => json!({
            "PlayTouchHoldBody": { "noteIndex": note_index, "atTime": time_point_json(at_time.clone()) }
        }),
        crate::events::AudioCommand::PlaySlideCue {
            note_index,
            track_index,
            is_break,
            at_time,
        } => json!({
            "PlaySlideCue": {
                "noteIndex": note_index,
                "trackIndex": track_index,
                "isBreak": is_break,
                "atTime": time_point_json(at_time.clone()),
            }
        }),
    }
}

pub fn render_cmd_json(c: &crate::events::RenderCommand) -> Value {
    match c {
        crate::events::RenderCommand::ShowJudgeResult {
            kind,
            grade,
            is_break,
            diff,
            note_index,
        } => json!({
            "ShowJudgeResult": {
                "kind": event_kind_str(*kind),
                "grade": grade_str(*grade),
                "isBreak": is_break,
                "diff": duration_json(diff.clone()),
                "noteIndex": note_index,
            }
        }),
        crate::events::RenderCommand::UpdateSlideProgress {
            note_index,
            remaining,
        } => json!({
            "UpdateSlideProgress": { "noteIndex": note_index, "remaining": remaining }
        }),
        crate::events::RenderCommand::UpdateSlideTrackProgress {
            note_index,
            track_index,
            remaining,
        } => json!({
            "UpdateSlideTrackProgress": {
                "noteIndex": note_index, "trackIndex": track_index, "remaining": remaining
            }
        }),
        crate::events::RenderCommand::HideAllSlideBars { note_index } => json!({
            "HideAllSlideBars": { "noteIndex": note_index }
        }),
        crate::events::RenderCommand::HideSlideBars {
            note_index,
            end_index,
        } => json!({
            "HideSlideBars": { "noteIndex": note_index, "endIndex": end_index }
        }),
        crate::events::RenderCommand::HideSlideTrackBars {
            note_index,
            track_index,
            end_index,
        } => json!({
            "HideSlideTrackBars": {
                "noteIndex": note_index, "trackIndex": track_index, "endIndex": end_index
            }
        }),
    }
}

pub(crate) fn grade_counts_json(counts: &[crate::natural::Natural; 15]) -> Value {
    let names = [
        "Miss",
        "LateGood",
        "LateGreat3rd",
        "LateGreat2nd",
        "LateGreat",
        "LatePerfect3rd",
        "LatePerfect2nd",
        "Perfect",
        "FastPerfect2nd",
        "FastPerfect3rd",
        "FastGreat",
        "FastGreat2nd",
        "FastGreat3rd",
        "FastGood",
        "TooFast",
    ];
    let mut obj = serde_json::Map::new();
    for (i, name) in names.iter().enumerate() {
        obj.insert((*name).to_string(), natural_json(&counts[i]));
    }
    Value::Object(obj)
}

pub(crate) fn score_json(s: &RuntimeScoreState) -> Value {
    json!({
        "combo": natural_json(&s.combo),
        "pCombo": natural_json(&s.p_combo),
        "cPCombo": natural_json(&s.c_p_combo),
        "totalBase": natural_json(&s.total_base),
        "totalExtra": natural_json(&s.total_extra),
        "earnedBase": natural_json(&s.earned_base),
        "earnedExtra": natural_json(&s.earned_extra),
        "earnedClassicExtra": natural_json(&s.earned_classic_extra),
        "lostBase": natural_json(&s.lost_base),
        "lostExtra": natural_json(&s.lost_extra),
        "lostClassicExtra": natural_json(&s.lost_classic_extra),
        "dxScore": integer_json(&s.dx_score),
        "maxDxScore": natural_json(&s.max_dx_score),
        "fastCount": natural_json(&s.fast_count),
        "lateCount": natural_json(&s.late_count),
        "counts": {
            "tapCount": grade_counts_json(&s.counts.tap),
            "holdCount": grade_counts_json(&s.counts.hold),
            "slideCount": grade_counts_json(&s.counts.slide),
            "touchCount": grade_counts_json(&s.counts.touch),
            "breakCount": grade_counts_json(&s.counts.break_),
        },
    })
}

// ---------------------------------------------------------------------------
// Chart IR encoders
// ---------------------------------------------------------------------------

fn slide_area_spec_json(spec: &crate::simai::slide_tables::SlideAreaSpec) -> Value {
    crate::json_encode::Encode::to_json(&spec)
}

fn slide_kind_str(k: crate::types::SlideKind) -> &'static str {
    match k {
        crate::types::SlideKind::Single => "Single",
        crate::types::SlideKind::Wifi => "Wifi",
        crate::types::SlideKind::ConnPart => "ConnPart",
    }
}

pub fn chart_spec_json(spec: &ChartSpec) -> Value {
    crate::json_encode::Encode::to_json(&spec)
}

pub(crate) fn slide_chart_note_json(n: &crate::chart_loader::SlideChartNote) -> Value {
    json!({
        "headTiming": time_point_json(n.head_timing.clone()), "slot": slot_str(n.slot),
        "length": duration_json(n.length.clone()), "startTiming": time_point_json(n.start_timing.clone()),
        "groupStartTiming": n.group_start_timing.clone().map(time_point_json),
        "slideKind": slide_kind_str(n.slide_kind), "isClassic": n.is_classic,
        "isSlideNoHead": n.is_slide_no_head, "isConnSlide": n.is_conn_slide,
        "parentNoteIndex": n.parent_note_index, "isGroupHead": n.is_group_head,
        "isGroupEnd": n.is_group_end, "parentFinished": n.parent_finished,
        "parentPendingFinish": n.parent_pending_finish,
        "totalJudgeQueueLen": n.total_judge_queue_len, "trackCount": n.track_count,
        "judgeAt": n.judge_at.clone().map(time_point_json), "isBreak": n.is_break, "isEX": n.is_ex,
        "multiple": n.multiple, "logicalSlideId": n.logical_slide_id, "noteIndex": n.note_index,
        "sdezSlideType": n.sdez_slide_type, "sdezEnd": n.sdez_end,
        "judgeQueues": n.judge_queues.iter().map(|q|
            q.iter().map(slide_area_spec_json).collect::<Vec<_>>()).collect::<Vec<_>>(),
        // Lean tuples are nested `Prod`, so a 3-tuple serializes as `[a,[b,c]]`.
        "debugSimai": n.debug_simai.as_ref().map(|(raw, key, just)| json!([raw, [key, just]])),
    })
}

pub fn normalized_chart_json(chart: &crate::simai::ir::NormalizedChart) -> Value {
    crate::json_encode::Encode::to_json(&chart)
}

// ---------------------------------------------------------------------------
// TimedInputBatch decoder
// ---------------------------------------------------------------------------

pub fn parse_timed_input_batch(value: &Value) -> Result<TimedInputBatch, String> {
    crate::json_decode::Decode::decode(value)
}

// ---------------------------------------------------------------------------
// Session registry
// ---------------------------------------------------------------------------

/// `LnmaiCore.FFI.RuntimeSession`. Seek origins belong to the registry.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Default)]
pub enum RuntimeSession {
    #[default]
    Empty,
    Loaded {
        spec: ChartSpec,
        state: GameState,
    },
}

pub(crate) use RuntimeSession as Session;

/// `LnmaiCore.FFI.RuntimeRegistry`. All mutations take place under one mutex.
#[derive(Debug, Clone, Default)]
pub struct RuntimeRegistry {
    pub next_handle: u64,
    pub sessions: HashMap<u64, RuntimeSession>,
    pub seek_origins: HashMap<u64, (GameState, Vec<TimedInputEvent>)>,
}

impl RuntimeRegistry {
    /// Lean `{}` starts handles at one; derived `default` starts at zero.
    pub fn with_field_defaults() -> Self {
        Self {
            next_handle: 1,
            ..Self::default()
        }
    }
}

impl RuntimeRegistry {
    pub fn create_empty_handle(&mut self) -> u64 {
        let handle = self.next_handle;
        self.next_handle = handle.wrapping_add(1);
        self.sessions.insert(handle, Session::Empty);
        // Lean preserves seekOrigins here, including on UInt64 wraparound.
        handle
    }

    pub fn insert_handle_state(&mut self, state: GameState) -> u64 {
        let handle = self.next_handle;
        self.next_handle = handle.wrapping_add(1);
        self.seek_origins
            .insert(handle, (state.clone(), Vec::new()));
        self.sessions.insert(
            handle,
            Session::Loaded {
                spec: ChartSpec::default(),
                state,
            },
        );
        handle
    }

    pub fn insert_loaded_handle(&mut self, spec: ChartSpec) -> u64 {
        let handle = self.next_handle;
        self.install_loaded_chart(handle, spec);
        self.next_handle = handle.wrapping_add(1);
        handle
    }

    fn install_loaded_chart(&mut self, handle: u64, spec: ChartSpec) {
        let state = crate::chart_loader::build_game_state(&spec);
        let tactic = crate::default_tactic::autoplay_plan_from_chart(&spec).events();
        self.seek_origins.insert(handle, (state.clone(), tactic));
        self.sessions
            .insert(handle, Session::Loaded { spec, state });
    }

    pub fn get_handle_state(&self, handle: u64) -> Result<&GameState, String> {
        match self.sessions.get(&handle) {
            Some(Session::Loaded { state, .. }) => Ok(state),
            session => Err(session_error(handle, session)),
        }
    }

    pub fn get_handle_chart_spec(&self, handle: u64) -> Result<&ChartSpec, String> {
        match self.sessions.get(&handle) {
            Some(Session::Loaded { spec, .. }) => Ok(spec),
            session => Err(session_error(handle, session)),
        }
    }

    pub fn update_handle_state(&mut self, handle: u64, next: GameState) -> Result<(), String> {
        match self.sessions.get_mut(&handle) {
            Some(Session::Loaded { state, .. }) => {
                *state = next;
                Ok(())
            }
            session => Err(session_error(handle, session.as_deref())),
        }
    }

    pub fn load_handle_chart(&mut self, handle: u64, spec: ChartSpec) -> Result<(), String> {
        match self.sessions.get(&handle) {
            None => Err(format!("unknown runtime handle: {handle}")),
            Some(Session::Loaded { .. }) => Err(format!(
                "runtime handle {handle} already has a loaded chart"
            )),
            Some(Session::Empty) => {
                self.install_loaded_chart(handle, spec);
                Ok(())
            }
        }
    }

    pub fn unload_handle_chart(&mut self, handle: u64) -> Result<(), String> {
        match self.sessions.get_mut(&handle) {
            None => Err(format!("unknown runtime handle: {handle}")),
            Some(Session::Empty) => Err(format!(
                "runtime handle {handle} already has no loaded chart"
            )),
            Some(slot) => {
                *slot = Session::Empty;
                self.seek_origins.remove(&handle);
                Ok(())
            }
        }
    }

    pub fn erase_handle_state(&mut self, handle: u64) -> bool {
        let present = self.sessions.remove(&handle).is_some();
        if present {
            self.seek_origins.remove(&handle);
        }
        present
    }
}

fn session_error(handle: u64, session: Option<&Session>) -> String {
    if session.is_some() {
        format!("runtime handle {handle} has no loaded chart")
    } else {
        format!("unknown runtime handle: {handle}")
    }
}

pub(crate) fn registry() -> &'static Mutex<RuntimeRegistry> {
    static REG: OnceLock<Mutex<RuntimeRegistry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(RuntimeRegistry::with_field_defaults()))
}

// ---------------------------------------------------------------------------
// SDEZ170 runtime registry
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SdezRegistry {
    pub next_handle: u64,
    pub sessions: std::collections::HashMap<u64, SdezLoaded>,
}

/// A loaded SDEZ session keeps the parsed chart so seek can rebuild the
/// runtime without re-parsing (or re-lowering) the Simai source.
pub struct SdezLoaded {
    pub chart: ChartSpec,
    pub runtime: crate::sdez::SdezRuntime,
}

impl SdezRegistry {
    fn with_field_defaults() -> Self {
        Self {
            next_handle: 1,
            sessions: std::collections::HashMap::new(),
        }
    }
}

fn sdez_registry() -> &'static Mutex<SdezRegistry> {
    static REG: OnceLock<Mutex<SdezRegistry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(SdezRegistry::with_field_defaults()))
}

pub fn create_sdez_session_handle() -> String {
    let mut reg = sdez_registry().lock().unwrap();
    let handle = reg.next_handle;
    reg.next_handle = handle.wrapping_add(1);
    ok_json(json!({ "handle": handle.to_string() }))
}

pub fn load_sdez_chart_text(
    handle: u64,
    content: &str,
    level_index: u32,
    autoplay: u8,
) -> String {
    let chart = match crate::simai::frontend::frontend_lowered_chart(
        content,
        (level_index as usize).into(),
    ) {
        Ok(chart) => chart,
        Err(error) => return error_json("parse_error", &error.message),
    };
    load_sdez_chart(handle, &chart, autoplay)
}

/// Build a fresh SDEZ runtime from an already parsed chart. Hosts can reuse
/// their loaded chart for playback and seeks without parsing its text again.
pub fn load_sdez_chart(handle: u64, chart: &ChartSpec, autoplay: u8) -> String {
    let mut runtime = crate::sdez::SdezRuntime::from_chart(chart);
    runtime.autoplay = match autoplay {
        1 => crate::sdez::AutoplayMode::Critical,
        2 => crate::sdez::AutoplayMode::Perfect,
        3 => crate::sdez::AutoplayMode::Great,
        4 => crate::sdez::AutoplayMode::Good,
        5 => crate::sdez::AutoplayMode::Miss,
        _ => crate::sdez::AutoplayMode::None,
    };
    sdez_registry()
        .lock()
        .unwrap()
        .sessions
        .insert(handle, SdezLoaded { chart: chart.clone(), runtime });
    ok_json(json!({ "handle": handle.to_string(), "state": "loaded" }))
}

fn etiming_str(timing: crate::sdez::ETiming) -> &'static str {
    use crate::sdez::ETiming::*;
    match timing {
        TooFast => "TooFast",
        FastGood => "FastGood",
        FastGreat3rd => "FastGreat3rd",
        FastGreat2nd => "FastGreat2nd",
        FastGreat => "FastGreat",
        FastPerfect2nd => "FastPerfect2nd",
        FastPerfect => "FastPerfect",
        Critical => "Critical",
        LatePerfect => "LatePerfect",
        LatePerfect2nd => "LatePerfect2nd",
        LateGreat => "LateGreat",
        LateGreat2nd => "LateGreat2nd",
        LateGreat3rd => "LateGreat3rd",
        LateGood => "LateGood",
        TooLate => "TooLate",
        End => "End",
    }
}

fn sdez_kind_str(kind: crate::sdez::SdezNoteKind) -> &'static str {
    match kind {
        crate::sdez::SdezNoteKind::Tap => "tap",
        crate::sdez::SdezNoteKind::Hold => "hold",
        crate::sdez::SdezNoteKind::Slide => "slide",
        crate::sdez::SdezNoteKind::Touch => "touch",
    }
}

pub fn step_sdez_handle(
    handle: u64,
    current_msec: f32,
    buttons_down: u16,
    buttons_push: u16,
    panel_state: u64,
    panel_new_frame: bool,
) -> String {
    let mut reg = sdez_registry().lock().unwrap();
    let Some(session) = reg.sessions.get_mut(&handle) else {
        return error_json(
            "invalid_runtime_handle",
            &format!("unknown SDEZ runtime handle: {handle}"),
        );
    };
    let runtime = &mut session.runtime;
    runtime
        .input
        .update_panel(panel_state, panel_new_frame);
    runtime.input.set_buttons(buttons_down, buttons_push);
    let events = runtime.step(current_msec);
    let slides = runtime.slide_progress();
    let score = &runtime.score;
    ok_json(json!({
        "events": events.iter().map(|event| json!({
            "noteIndex": event.note_index,
            "kind": sdez_kind_str(event.kind),
            "phase": match event.phase {
                crate::sdez::SdezPhase::Head => "head",
                crate::sdez::SdezPhase::Tail => "tail",
            },
            "etiming": etiming_str(event.judged.timing),
            "diffMsec": event.judged.diff_msec,
            "isBreak": event.is_break,
            "isEX": event.is_ex,
            "button": event.button,
            "sensor": event.sensor,
            "isWifi": event.is_wifi,
        })).collect::<Vec<_>>(),
        "currentTime": current_msec,
        "slides": slides.iter().map(|slide| json!({
            "noteIndex": slide.note_index,
            "isWifi": slide.is_wifi,
            "progress": slide.progress,
            "tracks": slide.tracks,
        })).collect::<Vec<_>>(),
        "score": {
            "combo": score.combo,
            "maxCombo": score.max_combo,
            "counts": score.counts,
            "totalBase": score.total_base,
            "earnedBase": score.earned_base,
            "totalBreakBonus": score.total_break_bonus,
            "earnedBreakBonus": score.earned_break_bonus,
            "dxScore": score.dx_score,
        },
    }))
}

pub fn free_sdez_handle(handle: u64) -> String {
    let removed = sdez_registry().lock().unwrap().sessions.remove(&handle);
    if removed.is_some() {
        ok_json(json!({ "freed": true }))
    } else {
        error_json(
            "invalid_runtime_handle",
            &format!("unknown SDEZ runtime handle: {handle}"),
        )
    }
}

pub fn set_sdez_autoplay(handle: u64, autoplay: u8) -> String {
    let mut reg = sdez_registry().lock().unwrap();
    let Some(session) = reg.sessions.get_mut(&handle) else {
        return error_json(
            "invalid_runtime_handle",
            &format!("unknown SDEZ runtime handle: {handle}"),
        );
    };
    let runtime = &mut session.runtime;
    runtime.autoplay = match autoplay {
        1 => crate::sdez::AutoplayMode::Critical,
        2 => crate::sdez::AutoplayMode::Perfect,
        3 => crate::sdez::AutoplayMode::Great,
        4 => crate::sdez::AutoplayMode::Good,
        5 => crate::sdez::AutoplayMode::Miss,
        _ => crate::sdez::AutoplayMode::None,
    };
    ok_json(json!({ "autoplay": autoplay }))
}

/// Rebuild the runtime from the cached chart, dropping all judgments and
/// progress. Seek uses this to avoid re-parsing the chart on every drag frame.
pub fn reset_sdez_handle(handle: u64, autoplay: u8) -> String {
    let mut reg = sdez_registry().lock().unwrap();
    let Some(session) = reg.sessions.get_mut(&handle) else {
        return error_json(
            "invalid_runtime_handle",
            &format!("unknown SDEZ runtime handle: {handle}"),
        );
    };
    let mut runtime = crate::sdez::SdezRuntime::from_chart(&session.chart);
    runtime.autoplay = autoplay_mode(autoplay);
    session.runtime = runtime;
    ok_json(json!({ "handle": handle.to_string(), "state": "reset" }))
}

fn autoplay_mode(autoplay: u8) -> crate::sdez::AutoplayMode {
    match autoplay {
        1 => crate::sdez::AutoplayMode::Critical,
        2 => crate::sdez::AutoplayMode::Perfect,
        3 => crate::sdez::AutoplayMode::Great,
        4 => crate::sdez::AutoplayMode::Good,
        5 => crate::sdez::AutoplayMode::Miss,
        _ => crate::sdez::AutoplayMode::None,
    }
}

fn loaded_summary_json(spec: &ChartSpec) -> Value {
    LoadedChartSummary::from_chart(spec).to_json()
}

pub fn step_result_json(
    events: &[JudgeEvent],
    audio: &[crate::events::AudioCommand],
    render: &[crate::events::RenderCommand],
    score: &RuntimeScoreState,
    current_time: TimePoint,
    terminated: bool,
) -> Value {
    json!({
        "events": events.iter().map(judge_event_json).collect::<Vec<_>>(),
        "audioCommands": audio.iter().map(audio_cmd_json).collect::<Vec<_>>(),
        "renderCommands": render.iter().map(render_cmd_json).collect::<Vec<_>>(),
        "score": score_json(score),
        "currentTime": time_point_json(current_time),
        "terminated": terminated,
    })
}

// ---------------------------------------------------------------------------
// Exported entry points (Rust API; thin C ABI can wrap these)
// ---------------------------------------------------------------------------

pub const FFI_ABI_VERSION: u64 = 2;

pub fn ffi_version_json() -> String {
    ok_json(json!({ "abiVersion": FFI_ABI_VERSION, "schema": "lnmai-core-ffi-json" }))
}

/// `lnmai_parse_lowered_chart_json`.
pub fn parse_lowered_chart_json(content: &str, level_index: u32) -> String {
    match crate::simai::frontend::frontend_lowered_chart(content, (level_index as usize).into()) {
        Ok(spec) => ok_json(chart_spec_json(&spec)),
        Err(e) => parse_error_json(&e),
    }
}

/// `lnmai_parse_normalized_chart_json`.
pub fn parse_normalized_chart_json(content: &str, level_index: u32) -> String {
    match crate::simai::frontend::frontend_normalized_chart(content, (level_index as usize).into())
    {
        Ok(chart) => ok_json(normalized_chart_json(&chart)),
        Err(e) => parse_error_json(&e),
    }
}

/// `lnmai_parse_frontend_semantic_chart_json`.
pub fn parse_frontend_semantic_chart_json(content: &str, level_index: u32) -> String {
    match crate::simai::frontend::parse_frontend_semantic_chart(
        content,
        (level_index as usize).into(),
    ) {
        Ok(semantic) => ok_json(json!({
            "normalized": normalized_chart_json(&semantic.normalized),
            "lowered": chart_spec_json(&semantic.lowered),
        })),
        Err(e) => parse_error_json(&e),
    }
}

/// `lnmai_default_tactic_from_chart_json`.
///
/// Decodes a `ChartSpec` JSON (as produced by `lnmai_parse_lowered_chart_json`)
/// and returns the default autoplay tactic:
/// `{"ok":true,"result":{"events":[ ...TimedInputEvent... ]}}`.
pub fn default_tactic_from_chart_json(chart_spec_json: &str) -> String {
    match parse_chart_spec_json(chart_spec_json) {
        Ok(spec) => {
            let events = crate::default_tactic::default_tactic_from_chart(&spec);
            ok_json(json!({
                "events": events.iter().map(timed_event_json).collect::<Vec<_>>(),
            }))
        }
        Err(e) => error_json("invalid_chart_spec_json", &e),
    }
}

/// `lnmai_create_empty_session_handle`.
pub fn create_empty_session_handle() -> String {
    let handle = registry().lock().unwrap().create_empty_handle();
    ok_json(json!({ "handle": handle.to_string(), "state": "empty" }))
}

/// `lnmai_load_chart_into_session_from_text`.
pub fn load_chart_into_session_from_text(handle: u64, content: &str, level_index: u32) -> String {
    let spec = match crate::simai::frontend::frontend_lowered_chart(
        content,
        (level_index as usize).into(),
    ) {
        Ok(spec) => spec,
        Err(e) => return parse_error_json(&e),
    };
    load_chart_into_session(handle, spec)
}

fn load_chart_into_session(handle: u64, spec: ChartSpec) -> String {
    let mut reg = registry().lock().unwrap();
    let summary = loaded_summary_json(&spec);
    match reg.load_handle_chart(handle, spec) {
        Err(error) => error_json("invalid_session_state", &error),
        Ok(()) => {
            ok_json(json!({ "handle": handle.to_string(), "state": "loaded", "summary": summary }))
        }
    }
}

/// Load a bare ChartSpec into an empty session.
pub fn load_chart_into_session_from_json(handle: u64, chart_spec_json: &str) -> String {
    match parse_chart_spec_json(chart_spec_json) {
        Err(e) => error_json("invalid_chart_spec_json", &e),
        Ok(spec) => load_chart_into_session(handle, spec),
    }
}

pub fn create_game_state_handle(chart_spec_json: &str) -> String {
    match parse_chart_spec_json(chart_spec_json) {
        Err(e) => error_json("invalid_chart_spec_json", &e),
        Ok(spec) => {
            let mut reg = registry().lock().unwrap();
            let handle = reg.insert_loaded_handle(spec);
            ok_json(json!({ "handle": handle.to_string() }))
        }
    }
}

pub fn build_game_state_json(chart_spec_json: &str) -> String {
    match parse_chart_spec_json(chart_spec_json) {
        Err(e) => error_json("invalid_chart_spec_json", &e),
        Ok(spec) => ok_json(game_state_json(&crate::chart_loader::build_game_state(
            &spec,
        ))),
    }
}

/// `lnmai_unload_chart_from_session`.
pub fn unload_chart_from_session(handle: u64) -> String {
    let mut reg = registry().lock().unwrap();
    match reg.unload_handle_chart(handle) {
        Err(error) => error_json("invalid_session_state", &error),
        Ok(()) => ok_json(json!({ "handle": handle.to_string(), "state": "empty" })),
    }
}

/// `lnmai_get_lowered_chart_json_by_handle`.
pub fn get_lowered_chart_json_by_handle(handle: u64) -> String {
    let reg = registry().lock().unwrap();
    match reg.sessions.get(&handle) {
        Some(Session::Loaded { spec, .. }) => ok_json(chart_spec_json(spec)),
        session => error_json("invalid_session_state", &session_error(handle, session)),
    }
}

/// `lnmai_free_game_state_handle`.
pub fn free_game_state_handle(handle: u64) -> String {
    let removed = registry().lock().unwrap().erase_handle_state(handle);
    if removed {
        ok_json(json!({ "freed": true }))
    } else {
        error_json(
            "invalid_runtime_handle",
            &format!("unknown runtime handle: {handle}"),
        )
    }
}

/// `lnmai_step_game_state_handle_light`.
pub fn step_game_state_handle_light(handle: u64, batch_json: &str) -> String {
    let batch: TimedInputBatch = match crate::json_decode::from_str(batch_json) {
        Ok(b) => b,
        Err(e) => return error_json("invalid_runtime_json", &e),
    };
    let mut reg = registry().lock().unwrap();
    match reg.sessions.get_mut(&handle) {
        Some(Session::Loaded { state, .. }) => {
            let (next, events, audio, render) =
                crate::scheduler::step_frame_timed_owned(std::mem::take(state), &batch);
            let result = step_result_json(
                &events,
                &audio,
                &render,
                &next.score,
                next.current_time.clone(),
                crate::scheduler::is_terminated(&next),
            );
            *state = next;
            ok_json(result)
        }
        session => error_json(
            "invalid_runtime_handle",
            &session_error(handle, session.as_deref()),
        ),
    }
}

/// Restore a loaded session at the target time through the library registry.
pub fn seek_game_state_handle(handle: u64, batch_json: &str, keep_held: bool) -> String {
    let batch: TimedInputBatch = match crate::json_decode::from_str(batch_json) {
        Ok(b) => b,
        Err(e) => return error_json("invalid_runtime_json", &e),
    };
    let target = batch.current_time.clone();
    let mut reg = registry().lock().unwrap();
    let spec = match reg.get_handle_chart_spec(handle) {
        Ok(spec) => spec.clone(),
        Err(error) => return error_json("invalid_runtime_handle", &error),
    };
    let previous = match reg.get_handle_state(handle) {
        Ok(state) => state.clone(),
        Err(error) => return error_json("invalid_runtime_handle", &error),
    };
    let (initial, tactic) = {
        let Some((initial, tactic)) = reg.seek_origins.get(&handle) else {
            return error_json("invalid_runtime_handle", "seek requires a loaded session");
        };
        (initial.clone(), tactic.clone())
    };
    let inputs = if batch.events.is_empty() {
        &tactic
    } else {
        &batch.events
    };
    let (next, render) = crate::runtime_seek::seek(
        &spec,
        &initial,
        &previous,
        target.clone(),
        inputs,
        keep_held,
    );
    let retained_ids =
        crate::runtime_seek::note_indices(&crate::runtime_seek::chart_from(&spec, target));
    let retained_set: std::collections::HashSet<_> = retained_ids.iter().cloned().collect();
    let retained: Vec<u64> = retained_ids.iter().map(|id| id.to_word()).collect();
    let skipped: Vec<u64> = crate::runtime_seek::note_indices(&spec)
        .into_iter()
        .filter(|id| !retained_set.contains(id))
        .map(|id| id.to_word())
        .collect();
    let result = json!({
        "score": score_json(&next.score),
        "currentTime": time_point_json(next.current_time.clone()),
        "terminated": crate::scheduler::is_terminated(&next),
        "retainedNoteIndices": retained,
        "skippedNoteIndices": skipped,
        "renderCommands": render.iter().map(render_cmd_json).collect::<Vec<_>>(),
    });
    if let Err(error) = reg.update_handle_state(handle, next) {
        return error_json("invalid_runtime_handle", &error);
    }
    ok_json(result)
}

/// Autoplay plan for the chart loaded in this session.
pub fn get_autoplay_plan_json_by_handle(handle: u64) -> String {
    let reg = registry().lock().unwrap();
    match reg.get_handle_chart_spec(handle) {
        Ok(spec) => ok_json(autoplay_plan_json(
            &crate::default_tactic::autoplay_plan_from_chart(spec),
        )),
        Err(error) => error_json("invalid_runtime_handle", &error),
    }
}

/// Runtime slide bindings for the loaded session.
pub fn get_slide_bindings_json_by_handle(handle: u64) -> String {
    let reg = registry().lock().unwrap();
    match reg.get_handle_state(handle) {
        Ok(state) => ok_json(json!(
            state
                .slides
                .iter()
                .map(|slide| json!({
                    "noteIndex": slide.params.note_index.to_word(),
                    "headTiming": slide.head_timing.micros.to_word(),
                    "totalJudgeQueueLen": slide.total_judge_queue_len.to_word(),
                    "trackCount": slide.track_count.to_word(),
                    "initialQueueRemaining": slide.initial_queue_remaining.to_word(),
                }))
                .collect::<Vec<_>>()
        )),
        Err(error) => error_json("invalid_runtime_handle", &error),
    }
}

// ---------------------------------------------------------------------------
// ChartSpec JSON decoder (for `load_..._from_json`)
// ---------------------------------------------------------------------------

fn parse_chart_spec_json(content: &str) -> Result<ChartSpec, String> {
    crate::chart_loader::parse_chart_json_string(content)
}

fn tap_state_json(s: TapState) -> Value {
    crate::json_encode::Encode::to_json(&s)
}

fn common_params_json(p: &crate::lifecycle::CommonNoteParams) -> Value {
    crate::json_encode::Encode::to_json(&p)
}

pub(crate) fn tap_family_json(n: &TapFamilyNote) -> Value {
    match n {
        TapFamilyNote::Tap(t) => json!({
            "kind": "tap",
            "params": common_params_json(&t.params),
            "lane": slot_str(t.lane),
            "state": tap_state_json(t.state),
            "buttonQueueIndex": t.button_queue_index,
        }),
        TapFamilyNote::SlideHead(h) => json!({
            "kind": "slideHead",
            "params": common_params_json(&h.params),
            "lane": slot_str(h.lane),
            "state": tap_state_json(h.state),
            "logicalSlideId": h.logical_slide_id,
            "buttonQueueIndex": h.button_queue_index,
        }),
    }
}

pub fn timed_event_json(e: &TimedInputEvent) -> Value {
    match e {
        TimedInputEvent::ButtonClick(tp, z) => {
            json!({ "buttonClick": { "tp": time_point_json(tp.clone()), "zone": button_str(*z) } })
        }
        TimedInputEvent::ButtonHold(tp, z, down) => {
            json!({ "buttonHold": { "tp": time_point_json(tp.clone()), "zone": button_str(*z), "isDown": down } })
        }
        TimedInputEvent::SensorClick(tp, a) => {
            json!({ "sensorClick": { "tp": time_point_json(tp.clone()), "area": sensor_str(*a) } })
        }
        TimedInputEvent::SensorHold(tp, a, down) => {
            json!({ "sensorHold": { "tp": time_point_json(tp.clone()), "area": sensor_str(*a), "isDown": down } })
        }
    }
}

pub fn game_state_json(st: &GameState) -> Value {
    crate::json_encode::Encode::to_json(&st)
}

/// `lnmai_get_game_state_json_by_handle`.
pub fn get_game_state_json_by_handle(handle: u64) -> String {
    let reg = registry().lock().unwrap();
    match reg.sessions.get(&handle) {
        Some(Session::Loaded { state, .. }) => ok_json(game_state_json(state)),
        session => error_json("invalid_runtime_handle", &session_error(handle, session)),
    }
}

/// Stateless stepping accepts the complete state serialized by Lean.
pub fn step_game_state_json(state_json: &str, batch_json: &str) -> String {
    let state: GameState = match crate::json_decode::from_str(state_json) {
        Ok(state) => state,
        Err(error) => return error_json("invalid_runtime_json", &error),
    };
    let batch: TimedInputBatch = match crate::json_decode::from_str(batch_json) {
        Ok(batch) => batch,
        Err(error) => return error_json("invalid_runtime_json", &error),
    };
    let (next, events, audio, render) = crate::scheduler::step_frame_timed(&state, &batch);
    let terminated = crate::scheduler::is_terminated(&next);
    ok_json(
        RuntimeStepResult {
            state: next,
            events,
            audio_commands: audio,
            render_commands: render,
            terminated,
        }
        .to_json(),
    )
}

/// `lnmai_step_game_state_handle` (full: returns the whole `GameState`).
pub fn step_game_state_handle(handle: u64, batch_json: &str) -> String {
    let batch: TimedInputBatch = match crate::json_decode::from_str(batch_json) {
        Ok(b) => b,
        Err(e) => return error_json("invalid_runtime_json", &e),
    };
    let mut reg = registry().lock().unwrap();
    match reg.sessions.get_mut(&handle) {
        Some(Session::Loaded { state, .. }) => {
            let (next, events, audio, render) = crate::scheduler::step_frame_timed(state, &batch);
            let result = json!({
                "state": game_state_json(&next),
                "terminated": crate::scheduler::is_terminated(&next),
                "events": events.iter().map(judge_event_json).collect::<Vec<_>>(),
                "audioCommands": audio.iter().map(audio_cmd_json).collect::<Vec<_>>(),
                "renderCommands": render.iter().map(render_cmd_json).collect::<Vec<_>>(),
            });
            *state = next;
            ok_json(result)
        }
        session => error_json(
            "invalid_runtime_handle",
            &session_error(handle, session.as_deref()),
        ),
    }
}

fn span_json(span: &crate::simai::syntax::SourceSpan) -> Value {
    json!({"start": {"line": span.start.line, "column": span.start.column},
        "stop": {"line": span.stop.line, "column": span.stop.column}})
}

/// Preserve source spans and syntax details for in-process frontend callers.
pub fn parse_error_json(error: &crate::simai::syntax::ParseError) -> String {
    use crate::simai::syntax::ParseErrorKind::*;
    let kind = match error.kind {
        InvalidSyntax => "invalidSyntax",
        InvalidShape => "invalidShape",
        InvalidEndPosition => "invalidEndPosition",
        InvalidTurnPosition => "invalidTurnPosition",
        InvalidChainTiming => "invalidChainTiming",
    };
    crate::lean_json::compress(
        &json!({"ok": false, "error": {"code": "parse_error", "message": error.message},
        "details": {"kind": kind, "rawText": error.raw_text, "message": error.message,
            "span": error.span.as_ref().map(span_json)}}),
    )
}

fn inspection_json(i: &crate::simai::ir::FrontendChartInspection) -> Value {
    crate::json_encode::Encode::to_json(&i)
}

pub fn parse_frontend_chart_json(content: &str, level: u32) -> String {
    parse_frontend_chart_mode_json(content, level, false)
}

pub fn parse_frontend_chart_mode_json(content: &str, level: u32, is_classic: bool) -> String {
    match crate::simai::frontend::parse_frontend_chart_result_with_mode(
        content,
        (level as usize).into(),
        is_classic,
    ) {
        Err(e) => parse_error_json(&e),
        Ok(r) => ok_json(
            json!({"semantic": {"normalized": normalized_chart_json(&r.semantic.normalized),
            "lowered": chart_spec_json(&r.semantic.lowered)}, "inspection": inspection_json(&r.inspection)}),
        ),
    }
}

pub fn parse_frontend_inspection_chart_json(content: &str, level: u32) -> String {
    match crate::simai::frontend::parse_frontend_inspection_chart(content, (level as usize).into())
    {
        Err(e) => parse_error_json(&e),
        Ok(r) => ok_json(inspection_json(&r)),
    }
}

pub fn parse_lowered_chart_mode_json(content: &str, level: u32, is_classic: bool) -> String {
    match crate::simai::frontend::frontend_lowered_chart_with_mode(
        content,
        (level as usize).into(),
        is_classic,
    ) {
        Err(e) => parse_error_json(&e),
        Ok(r) => ok_json(chart_spec_json(&r)),
    }
}

pub fn autoplay_plan_json(plan: &crate::default_tactic::AutoplayPlan) -> Value {
    crate::json_encode::Encode::to_json(&plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIDATA: &str = "&first=0\n&inote_1=\n(120)\n1,2,3,\n";

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn version_ok() {
        let v = parse(&ffi_version_json());
        assert_eq!(v["ok"], true);
        assert_eq!(v["result"]["abiVersion"], 2);
    }

    #[test]
    fn parse_lowered_ok() {
        let v = parse(&parse_lowered_chart_json(MAIDATA, 1));
        assert_eq!(v["ok"], true);
        assert_eq!(v["result"]["taps"].as_array().unwrap().len(), 3);
        assert_eq!(v["result"]["taps"][0]["slot"], "S1");
    }

    #[test]
    fn parse_error_envelope() {
        let v = parse(&parse_lowered_chart_json("&inote_1=\n(120)\n1-9,", 1));
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "parse_error");
    }

    #[test]
    fn session_roundtrip_and_step() {
        let created = parse(&create_empty_session_handle());
        let handle = created["result"]["handle"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert_eq!(created["result"]["state"], "empty");

        let loaded = parse(&load_chart_into_session_from_text(handle, MAIDATA, 1));
        assert_eq!(loaded["result"]["state"], "loaded");
        assert_eq!(loaded["result"]["summary"]["tapCount"], 3);

        let lowered = parse(&get_lowered_chart_json_by_handle(handle));
        assert_eq!(lowered["ok"], true);

        // Step with a click at time 0 on K1 → first tap judged Perfect.
        let batch = r#"{"currentTime":0,"events":[{"buttonClick":{"tp":0,"zone":"K1"}}]}"#;
        let stepped = parse(&step_game_state_handle_light(handle, batch));
        assert_eq!(stepped["ok"], true);
        assert_eq!(stepped["result"]["events"].as_array().unwrap().len(), 1);
        assert_eq!(stepped["result"]["events"][0]["grade"], "Perfect");
        assert_eq!(stepped["result"]["score"]["combo"], 1);
        assert_eq!(
            stepped["result"]["score"]["counts"]["tapCount"]["Perfect"],
            1
        );

        let unloaded = parse(&unload_chart_from_session(handle));
        assert_eq!(unloaded["result"]["state"], "empty");
        let freed = parse(&free_game_state_handle(handle));
        assert_eq!(freed["result"]["freed"], true);
    }

    #[test]
    fn light_session_preserves_owned_state_across_mixed_note_frames() {
        let spec = crate::simai::frontend::frontend_lowered_chart(
            "&inote_1=(120){4}1,2h[4:1],A1/B1,C2h[4:1],3-7[4:1],5w1[4:1],E",
            1.into(),
        ).unwrap();
        let events = crate::default_tactic::autoplay_plan_from_chart(&spec).events();
        let handle = registry().lock().unwrap().insert_loaded_handle(spec.clone());
        let mut expected = crate::chart_loader::build_game_state(&spec);
        let mut cursor = 0;
        let mut completed = false;
        for frame in 0..160 {
            let now = crate::time::TimePoint::from_micros(frame * 50_000);
            let start = cursor;
            while cursor < events.len() && events[cursor].at().micros <= now.micros {
                cursor += 1;
            }
            let batch = TimedInputBatch {
                current_time: now,
                events: events[start..cursor].to_vec(),
            };
            let (next, judged, audio, render) = crate::scheduler::step_frame_timed(&expected, &batch);
            let expected_result = step_result_json(
                &judged, &audio, &render, &next.score, next.current_time.clone(),
                crate::scheduler::is_terminated(&next),
            );
            let actual = parse(&step_game_state_handle_light(
                handle, &crate::json_encode::Encode::to_json(&batch).to_string(),
            ));
            assert_eq!(actual["ok"], true);
            assert_eq!(actual["result"], expected_result, "frame {frame}");
            assert_eq!(registry().lock().unwrap().get_handle_state(handle).unwrap(), &next);
            completed |= crate::scheduler::is_terminated(&next);
            expected = next;
        }
        assert!(completed);
        assert_eq!(parse(&free_game_state_handle(handle))["ok"], true);
    }

    #[test]
    fn step_before_load_is_error() {
        let created = parse(&create_empty_session_handle());
        let handle = created["result"]["handle"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let bad = parse(&step_game_state_handle_light(
            handle,
            r#"{"currentTime":0,"events":[]}"#,
        ));
        assert_eq!(bad["ok"], false);
        assert_eq!(bad["error"]["code"], "invalid_runtime_handle");
    }

    #[test]
    fn full_state_roundtrip() {
        let created = parse(&create_empty_session_handle());
        let handle = created["result"]["handle"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        parse(&load_chart_into_session_from_text(handle, MAIDATA, 1));

        let batch = r#"{"currentTime":0,"events":[{"buttonClick":{"tp":0,"zone":"K1"}}]}"#;
        let stepped = parse(&step_game_state_handle(handle, batch));
        assert_eq!(stepped["ok"], true);
        assert_eq!(stepped["result"]["events"][0]["grade"], "Perfect");
        assert_eq!(stepped["result"]["state"]["currentTime"], 0);
        assert_eq!(stepped["result"]["state"]["score"]["combo"], 1);
        assert_eq!(
            stepped["result"]["state"]["tapQueues"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            stepped["result"]["state"]["touchQueues"]
                .as_array()
                .unwrap()
                .len(),
            33
        );

        let state = parse(&get_game_state_json_by_handle(handle));
        assert_eq!(state["ok"], true);
        assert_eq!(state["result"]["score"]["counts"]["tapCount"]["Perfect"], 1);
        assert!(state["result"]["slides"].is_array());

        parse(&free_game_state_handle(handle));
    }

    #[test]
    fn sdez_session_roundtrip() {
        let created = parse(&create_sdez_session_handle());
        let handle = created["result"]["handle"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let loaded = parse(&load_sdez_chart_text(handle, MAIDATA, 1, 1));
        assert_eq!(loaded["ok"], true);

        let stepped = parse(&step_sdez_handle(handle, 110.0, 0, 0, 0, true));
        assert_eq!(stepped["ok"], true);
        let events = stepped["result"]["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["etiming"], "Critical");
        assert_eq!(events[0]["kind"], "tap");

        assert_eq!(parse(&free_sdez_handle(handle))["ok"], true);
    }
}
