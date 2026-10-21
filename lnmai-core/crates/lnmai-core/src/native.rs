//! Native ABI 3, matching native/lnmai_native.h and LnmaiCore.FFI.nativeDispatch.
//! Inputs are borrowed for the call; results own their records and error text.
use std::ffi::{c_char, CString};
use std::ptr;

use crate::areas::{ButtonZone, SensorArea};
use crate::chart_loader::ChartSpec;
use crate::events::{AudioCommand, RenderCommand};
use crate::ffi::{registry, Session};
use crate::input_model::{GameState, TimedInputBatch, TimedInputEvent};
use crate::natural::Natural;
use crate::runtime_score::grade_index;
use crate::time::TimePoint;
use crate::types::{JudgeEvent, JudgeEventKind, JudgePhase, SlideKind};

pub const SENSOR_AREAS: [SensorArea; 33] = {
    use SensorArea::*;
    [
        A1, A2, A3, A4, A5, A6, A7, A8, B1, B2, B3, B4, B5, B6, B7, B8, C, D1, D2, D3, D4, D5, D6,
        D7, D8, E1, E2, E3, E4, E5, E6, E7, E8,
    ]
};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LnmaiInputEvent {
    pub time_us: i64,
    pub kind: u32,
    pub position: u32,
    pub is_down: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LnmaiRequest {
    pub abi_version: u32,
    pub struct_size: u32,
    pub time_us: i64,
    pub level_index: u32,
    pub reserved: u32,
    pub events: *const LnmaiInputEvent,
    pub event_count: u64,
    pub text: *const c_char,
    pub text_len: u64,
}

impl Default for LnmaiRequest {
    fn default() -> Self {
        Self {
            abi_version: 3,
            struct_size: std::mem::size_of::<Self>() as u32,
            time_us: 0,
            level_index: 1,
            reserved: 0,
            events: ptr::null(),
            event_count: 0,
            text: ptr::null(),
            text_len: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LnmaiRecord {
    pub tag: u64,
    pub words: [u64; 8],
}

#[derive(Default)]
pub struct LnmaiResult {
    records: Vec<LnmaiRecord>,
    error: Option<CString>,
    failed: bool,
}

impl LnmaiResult {
    fn emit(&mut self, tag: u64, values: &[u64]) {
        if self.failed {
            return;
        }
        if self.records.try_reserve(1).is_err() {
            self.failed = true;
            return;
        }
        let mut words = [0; 8];
        words[..values.len()].copy_from_slice(values);
        self.records.push(LnmaiRecord { tag, words });
    }
    fn fail(&mut self, message: &str) -> u32 {
        // C error strings expose text only up to the first NUL.
        self.error = CString::new(message.split('\0').next().unwrap_or("")).ok();
        1
    }
    fn status(&self) -> u32 {
        if self.failed {
            2
        } else {
            0
        }
    }
}

fn area_index(area: SensorArea) -> u64 {
    SENSOR_AREAS.iter().position(|a| *a == area).unwrap() as u64
}
fn kind_index(kind: JudgeEventKind) -> u64 {
    match kind {
        JudgeEventKind::Tap => 0,
        JudgeEventKind::Hold => 1,
        JudgeEventKind::Slide => 2,
        JudgeEventKind::Touch => 3,
        JudgeEventKind::Break => 4,
    }
}
fn flags(a: bool, b: bool) -> u64 {
    u64::from(a) + 2 * u64::from(b)
}
fn optional_nat(value: Option<Natural>) -> u64 {
    value.map(|n| n.to_word()).unwrap_or(u64::MAX)
}
fn optional_bool(value: Option<bool>) -> u64 {
    value.map(u64::from).unwrap_or(u64::MAX)
}

// UInt64.ofNat/ofInt reduce modulo 2^64 at the native boundary.
fn word_nat(n: &Natural) -> u64 {
    n.to_word()
}
fn word_int(n: &crate::integer::Integer) -> u64 {
    n.to_word()
}

fn emit_score(out: &mut LnmaiResult, state: &GameState) {
    let s = &state.score;
    out.emit(
        1,
        &[
            state.current_time.micros.to_word(),
            crate::scheduler::is_terminated(state) as u64,
        ],
    );
    out.emit(
        2,
        &[
            word_nat(&s.combo),
            word_nat(&s.p_combo),
            word_nat(&s.c_p_combo),
            word_nat(&s.total_base),
            word_nat(&s.total_extra),
            word_nat(&s.earned_base),
            word_nat(&s.earned_extra),
            word_nat(&s.earned_classic_extra),
        ],
    );
    out.emit(
        3,
        &[
            word_nat(&s.lost_base),
            word_nat(&s.lost_extra),
            word_nat(&s.lost_classic_extra),
            word_int(&s.dx_score),
            word_nat(&s.max_dx_score),
            word_nat(&s.fast_count),
            word_nat(&s.late_count),
        ],
    );
    for (kind, counts) in [
        &s.counts.tap,
        &s.counts.hold,
        &s.counts.slide,
        &s.counts.touch,
        &s.counts.break_,
    ]
    .iter()
    .enumerate()
    {
        for (grade, count) in counts.iter().enumerate() {
            out.emit(4, &[kind as u64, grade as u64, word_nat(count)]);
        }
    }
}

fn emit_step(
    out: &mut LnmaiResult,
    state: &GameState,
    events: &[JudgeEvent],
    audio: &[AudioCommand],
    render: &[RenderCommand],
) {
    emit_score(out, state);
    for e in events {
        let pos = e
            .position
            .button_zone()
            .map(|z| z.to_index() as u64 + 1)
            .unwrap_or(0)
            + 256
                * e.position
                    .sensor_area()
                    .map(|a| area_index(a) + 1)
                    .unwrap_or(0);
        out.emit(
            10,
            &[
                kind_index(e.kind),
                if e.phase == JudgePhase::Head { 0 } else { 1 },
                grade_index(e.grade) as u64,
                e.diff.micros.to_word(),
                e.note_index.to_word(),
                pos,
                flags(e.is_break, e.is_ex),
                e.multiple.to_word(),
            ],
        );
    }
    for command in audio {
        match command.clone() {
            AudioCommand::PlayJudgeSfx {
                kind,
                grade,
                is_break,
                at_time,
                note_index,
            } => out.emit(
                20,
                &[
                    0,
                    kind_index(kind),
                    grade_index(grade) as u64,
                    is_break as u64,
                    at_time.micros.to_word(),
                    note_index.to_word(),
                ],
            ),
            AudioCommand::PlaySlideCue {
                note_index,
                track_index,
                is_break,
                at_time,
            } => out.emit(
                20,
                &[
                    1,
                    note_index.to_word(),
                    track_index.to_word(),
                    is_break as u64,
                    at_time.micros.to_word(),
                ],
            ),
            AudioCommand::PlayTouchHoldBody {
                note_index,
                at_time,
            } => out.emit(20, &[2, note_index.to_word(), at_time.micros.to_word()]),
        }
    }
    for command in render {
        match command.clone() {
            RenderCommand::ShowJudgeResult {
                kind,
                grade,
                is_break,
                diff,
                note_index,
            } => out.emit(
                30,
                &[
                    0,
                    kind_index(kind),
                    grade_index(grade) as u64,
                    is_break as u64,
                    diff.micros.to_word(),
                    note_index.to_word(),
                ],
            ),
            RenderCommand::UpdateSlideProgress {
                note_index,
                remaining,
            } => out.emit(30, &[1, note_index.to_word(), remaining.to_word()]),
            RenderCommand::UpdateSlideTrackProgress {
                note_index,
                track_index,
                remaining,
            } => out.emit(
                30,
                &[
                    2,
                    note_index.to_word(),
                    track_index.to_word(),
                    remaining.to_word(),
                ],
            ),
            RenderCommand::HideAllSlideBars { note_index } => {
                out.emit(30, &[3, note_index.to_word()])
            }
            RenderCommand::HideSlideBars {
                note_index,
                end_index,
            } => out.emit(30, &[4, note_index.to_word(), end_index.to_word()]),
            RenderCommand::HideSlideTrackBars {
                note_index,
                track_index,
                end_index,
            } => out.emit(
                30,
                &[
                    5,
                    note_index.to_word(),
                    track_index.to_word(),
                    end_index.to_word(),
                ],
            ),
        }
    }
}

fn emit_plan(out: &mut LnmaiResult, chart: &ChartSpec) {
    for group in crate::default_tactic::autoplay_plan_from_chart(chart).groups {
        out.emit(50, &[group.id.to_word(), group.kind.to_word()]);
        for part in group.parts {
            out.emit(
                51,
                &[
                    part.runtime_note_index.to_word(),
                    part.role.to_word(),
                    optional_nat(part.track_index),
                ],
            );
            for event in part.events {
                match event {
                    TimedInputEvent::ButtonClick(t, z) => {
                        out.emit(52, &[0, t.micros.to_word(), z.to_index() as u64])
                    }
                    TimedInputEvent::ButtonHold(t, z, down) => out.emit(
                        52,
                        &[1, t.micros.to_word(), z.to_index() as u64, down as u64],
                    ),
                    TimedInputEvent::SensorClick(t, a) => {
                        out.emit(52, &[2, t.micros.to_word(), area_index(a)])
                    }
                    TimedInputEvent::SensorHold(t, a, down) => {
                        out.emit(52, &[3, t.micros.to_word(), area_index(a), down as u64])
                    }
                }
            }
        }
    }
}

fn emit_chart(out: &mut LnmaiResult, chart: &ChartSpec) {
    out.emit(100, &[optional_bool(chart.slide_skipping)]);
    for n in &chart.taps {
        out.emit(
            101,
            &[
                n.note_index.to_word(),
                n.timing.micros.to_word(),
                n.slot.to_index() as u64,
                flags(n.is_break, n.is_ex),
                n.button_queue_index.to_word(),
            ],
        );
    }
    for n in &chart.holds {
        out.emit(
            102,
            &[
                n.note_index.to_word(),
                n.timing.micros.to_word(),
                n.slot.to_index() as u64,
                n.length.micros.to_word(),
                flags(n.is_break, n.is_ex),
                n.is_touch as u64,
                optional_bool(n.is_classic),
                n.button_queue_index.to_word(),
            ],
        );
        out.emit(
            112,
            &[
                optional_nat(n.touch_hold_group_id.clone()),
                optional_nat(n.touch_hold_group_size.clone()),
            ],
        );
    }
    for n in &chart.touches {
        out.emit(
            103,
            &[
                n.note_index.to_word(),
                n.timing.micros.to_word(),
                area_index(n.sensor_pos),
                n.is_break as u64,
                optional_nat(n.source_group_id.clone()),
                optional_nat(n.source_group_index.clone()),
                optional_nat(n.source_group_size.clone()),
                n.touch_queue_index.to_word(),
            ],
        );
        out.emit(
            113,
            &[
                optional_nat(n.touch_group_id.clone()),
                optional_nat(n.touch_group_size.clone()),
            ],
        );
    }
    for n in &chart.touch_holds {
        out.emit(
            104,
            &[
                n.note_index.to_word(),
                n.timing.micros.to_word(),
                area_index(n.sensor_pos),
                n.length.micros.to_word(),
                flags(n.is_break, n.is_ex),
                optional_nat(n.source_group_id.clone()),
                optional_nat(n.source_group_index.clone()),
                optional_nat(n.source_group_size.clone()),
            ],
        );
        out.emit(
            114,
            &[
                n.touch_queue_index.to_word(),
                optional_nat(n.touch_group_id.clone()),
                optional_nat(n.touch_group_size.clone()),
                optional_nat(n.touch_hold_group_id.clone()),
                optional_nat(n.touch_hold_group_size.clone()),
            ],
        );
    }
    for n in &chart.slide_heads {
        out.emit(
            105,
            &[
                n.note_index.to_word(),
                n.logical_slide_id.to_word(),
                n.timing.micros.to_word(),
                n.slot.to_index() as u64,
                flags(n.is_break, n.is_ex),
            ],
        );
    }
    for n in &chart.slides {
        let kind = match n.slide_kind {
            SlideKind::Single => 0,
            SlideKind::Wifi => 1,
            SlideKind::ConnPart => 2,
        };
        out.emit(
            106,
            &[
                n.note_index.to_word(),
                n.logical_slide_id.to_word(),
                n.head_timing.micros.to_word(),
                n.start_timing.micros.to_word(),
                n.length.micros.to_word(),
                n.slot.to_index() as u64,
                kind,
                flags(n.is_break, n.is_ex),
            ],
        );
        out.emit(
            107,
            &[
                optional_nat(n.parent_note_index.clone()),
                n.total_judge_queue_len.to_word(),
                n.track_count.to_word(),
                n.judge_at
                    .as_ref()
                    .map(|t| t.micros.to_word())
                    .unwrap_or(i64::MIN as u64),
                n.multiple.to_word(),
                flags(n.is_classic, n.is_slide_no_head),
                flags(n.is_conn_slide, n.is_group_head),
                flags(n.is_group_end, n.parent_finished),
            ],
        );
        out.emit(117, &[n.parent_pending_finish as u64]);
        for (track, queue) in n.judge_queues.iter().enumerate() {
            out.emit(108, &[track as u64]);
            for a in queue {
                let mask = a
                    .target_areas
                    .iter()
                    .fold(0, |mask, a| mask | (1 << area_index(*a)));
                out.emit(
                    109,
                    &[
                        mask,
                        (a.policy == crate::types::AreaPolicy::And) as u64,
                        flags(a.is_last, a.is_skippable),
                        a.arrow_progress_when_on.to_word(),
                        a.arrow_progress_when_finished.to_word(),
                    ],
                );
            }
        }
    }
}

fn dispatch(
    handle: u64,
    operation: u32,
    request: &LnmaiRequest,
    events: &[LnmaiInputEvent],
    text: &str,
    out: &mut LnmaiResult,
) -> u32 {
    let mut reg = registry().lock().unwrap();
    let reg = &mut *reg;
    if operation == 0 {
        // Reserve output before allocating the handle or changing the registry.
        out.emit(0, &[reg.next_handle]);
        if out.failed {
            return 2;
        }
        reg.create_empty_handle();
        return 0;
    }
    if operation == 7 {
        return if reg.erase_handle_state(handle) {
            0
        } else {
            out.fail(&format!("unknown runtime handle: {handle}"))
        };
    }
    let Some(session) = reg.sessions.get_mut(&handle) else {
        return out.fail(&format!("unknown runtime handle: {handle}"));
    };
    if matches!(session, Session::Empty) {
        if operation != 1 {
            return out.fail("session has no loaded chart");
        }
        let chart = match crate::simai::frontend::frontend_lowered_chart(
            text,
            (request.level_index as usize).into(),
        ) {
            Ok(chart) => chart,
            Err(error) => return out.fail(&error.message),
        };
        let state = crate::chart_loader::build_game_state(&chart);
        emit_score(out, &state);
        if !out.failed {
            let tactic = crate::default_tactic::autoplay_plan_from_chart(&chart).events();
            reg.seek_origins.insert(handle, (state.clone(), tactic));
            *session = Session::Loaded { spec: chart, state };
        }
        return out.status();
    }
    let Session::Loaded { spec, state } = session else {
        unreachable!()
    };
    let batch = || TimedInputBatch {
        current_time: TimePoint::from_micros(request.time_us),
        events: events
            .iter()
            .map(|e| {
                let t = TimePoint::from_micros(e.time_us);
                match e.kind {
                    0 => TimedInputEvent::ButtonClick(
                        t,
                        ButtonZone::of_index(e.position as usize).unwrap(),
                    ),
                    1 => TimedInputEvent::ButtonHold(
                        t,
                        ButtonZone::of_index(e.position as usize).unwrap(),
                        e.is_down != 0,
                    ),
                    2 => TimedInputEvent::SensorClick(t, SENSOR_AREAS[e.position as usize]),
                    _ => TimedInputEvent::SensorHold(
                        t,
                        SENSOR_AREAS[e.position as usize],
                        e.is_down != 0,
                    ),
                }
            })
            .collect(),
    };
    match operation {
        2 => {
            if request.time_us < state.current_time.micros {
                return out.fail("clock moved backwards: use seek");
            }
            let (next, events, audio, render) = crate::scheduler::step_frame_timed(state, &batch());
            emit_step(out, &next, &events, &audio, &render);
            if !out.failed {
                *state = next;
            }
        }
        3 => {
            let target = TimePoint::from_micros(request.time_us);
            let Some((initial, tactic)) = reg.seek_origins.get(&handle) else {
                return out.fail("seek requires a native-loaded session");
            };
            let inputs = batch();
            let events = if inputs.events.is_empty() {
                tactic
            } else {
                &inputs.events
            };
            let (next, render) = crate::runtime_seek::seek(
                spec,
                initial,
                state,
                target.clone(),
                events,
                request.level_index != 0,
            );
            let retained: std::collections::HashSet<_> =
                crate::runtime_seek::note_indices(&crate::runtime_seek::chart_from(spec, target))
                    .into_iter()
                    .collect();
            emit_step(out, &next, &[], &[], &render);
            for id in crate::runtime_seek::note_indices(spec) {
                out.emit(
                    if retained.contains(&id) { 40 } else { 41 },
                    &[id.to_word()],
                );
            }
            if !out.failed {
                *state = next;
            }
        }
        4 => emit_plan(out, spec),
        5 => emit_chart(out, spec),
        6 => {
            for slide in &state.slides {
                out.emit(
                    110,
                    &[
                        slide.params.note_index.to_word(),
                        slide.head_timing.micros.to_word(),
                        slide.total_judge_queue_len.to_word(),
                        slide.track_count.to_word(),
                        slide.initial_queue_remaining.to_word(),
                    ],
                );
            }
        }
        _ => return out.fail("unsupported native operation"),
    }
    out.status()
}

/// # Safety
/// Non-null pointers must be aligned, readable for their declared lengths and
/// valid for the call. The output slot must be writable. Free its result once.
#[no_mangle]
pub unsafe extern "C" fn lnmai_v3_call(
    handle: u64,
    operation: u32,
    request: *const LnmaiRequest,
    out: *mut *mut LnmaiResult,
) -> u32 {
    if out.is_null() {
        return 1;
    }
    unsafe {
        *out = ptr::null_mut();
    }
    if request.is_null() {
        return 1;
    }
    let r = unsafe { &*request };
    if r.abi_version != 3
        || r.struct_size as usize != std::mem::size_of::<LnmaiRequest>()
        || r.reserved != 0
        || operation > 7
        || (operation == 3 && r.time_us < 0)
        || (r.event_count != 0 && r.events.is_null())
        || (r.text_len != 0 && r.text.is_null())
        || r.event_count > (isize::MAX as usize / std::mem::size_of::<LnmaiInputEvent>()) as u64
        || r.text_len > isize::MAX as u64
    {
        return 1;
    }
    let events = if r.event_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(r.events, r.event_count as usize) }
    };
    for e in events {
        if e.reserved != 0
            || e.kind > 3
            || e.position >= (if e.kind < 2 { 8 } else { 33 })
            || e.is_down > 1
        {
            return 1;
        }
    }
    let text = if r.text_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(r.text.cast::<u8>(), r.text_len as usize) }
    };
    let layout = std::alloc::Layout::new::<LnmaiResult>();
    let result = unsafe { std::alloc::alloc(layout).cast::<LnmaiResult>() };
    if result.is_null() {
        return 2;
    }
    unsafe {
        result.write(LnmaiResult::default());
        *out = result;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch(
            handle,
            operation,
            r,
            events,
            &String::from_utf8_lossy(text),
            unsafe { &mut *result },
        )
    }))
    .unwrap_or(3)
}

/// # Safety
/// result must be a live result from this library and count must be writable.
#[no_mangle]
pub unsafe extern "C" fn lnmai_v3_records(
    result: *const LnmaiResult,
    count: *mut u64,
) -> *const LnmaiRecord {
    let result = unsafe { &*result };
    unsafe {
        *count = result.records.len() as u64;
    }
    if result.records.is_empty() {
        ptr::null()
    } else {
        result.records.as_ptr()
    }
}

/// # Safety
/// result must be null or a live result from this library.
#[no_mangle]
pub unsafe extern "C" fn lnmai_v3_error(result: *const LnmaiResult) -> *const c_char {
    const FALLBACK: &[u8] = b"native FFI call failed\0";
    if !result.is_null() {
        if let Some(error) = &unsafe { &*result }.error {
            return error.as_ptr();
        }
    }
    FALLBACK.as_ptr().cast()
}

/// # Safety
/// result must be null or a result from this library that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn lnmai_v3_free_result(result: *mut LnmaiResult) {
    if !result.is_null() {
        drop(unsafe { Box::from_raw(result) });
    }
}
