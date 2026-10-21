//! Safe adapter for the ABI-3 C bridge. No Lean object layout is visible here.
use crate::session::{LnmaiError, Result};
use crate::types::*;
use std::{
    ffi::{CStr, c_char},
    ptr, slice,
};

#[repr(C)]
struct InputEvent {
    time_us: i64,
    kind: u32,
    position: u32,
    is_down: u32,
    reserved: u32,
}
#[repr(C)]
struct Request {
    abi_version: u32,
    struct_size: u32,
    time_us: i64,
    level_index: u32,
    reserved: u32,
    events: *const InputEvent,
    event_count: u64,
    text: *const u8,
    text_len: u64,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            abi_version: 3,
            struct_size: size_of::<Self>() as u32,
            time_us: 0,
            level_index: 0,
            reserved: 0,
            events: ptr::null(),
            event_count: 0,
            text: ptr::null(),
            text_len: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Record {
    tag: u64,
    w: [u64; 8],
}
enum Output {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_requests_are_rejected_before_runtime_dispatch() {
        let requests = [
            (
                0,
                Request {
                    abi_version: 2,
                    ..Default::default()
                },
            ),
            (
                0,
                Request {
                    struct_size: 0,
                    ..Default::default()
                },
            ),
            (
                0,
                Request {
                    reserved: 1,
                    ..Default::default()
                },
            ),
            (
                0,
                Request {
                    event_count: 1,
                    ..Default::default()
                },
            ),
            (
                0,
                Request {
                    text_len: 1,
                    ..Default::default()
                },
            ),
            (
                3,
                Request {
                    time_us: -1,
                    ..Default::default()
                },
            ),
            (8, Request::default()),
        ];
        for (operation, request) in requests {
            let mut output = ptr::null_mut();
            assert_ne!(
                unsafe { lnmai_v3_call(0, operation, &request, &mut output) },
                0
            );
            assert!(output.is_null());
        }
        let input = InputEvent {
            time_us: 0,
            kind: 2,
            position: 33,
            is_down: 0,
            reserved: 0,
        };
        let request = Request {
            events: &input,
            event_count: 1,
            ..Default::default()
        };
        let mut output = ptr::null_mut();
        assert_ne!(unsafe { lnmai_v3_call(0, 2, &request, &mut output) }, 0);
        assert!(output.is_null());
        unsafe { lnmai_v3_free_result(ptr::null_mut()) };
    }
}
unsafe extern "C" {
    fn lnmai_v3_call(
        handle: u64,
        operation: u32,
        request: *const Request,
        out: *mut *mut Output,
    ) -> u32;
    fn lnmai_v3_records(out: *const Output, count: *mut u64) -> *const Record;
    fn lnmai_v3_error(out: *const Output) -> *const c_char;
    fn lnmai_v3_free_result(out: *mut Output);
}
struct OwnedOutput(*mut Output);
impl Drop for OwnedOutput {
    fn drop(&mut self) {
        unsafe { lnmai_v3_free_result(self.0) }
    }
}
impl OwnedOutput {
    fn records(&self) -> &[Record] {
        let mut count = 0;
        let p = unsafe { lnmai_v3_records(self.0, &mut count) };
        if count == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(p, count as usize) }
        }
    }
}
fn error(message: impl Into<String>) -> LnmaiError {
    LnmaiError {
        json: message.into(),
    }
}
fn call(handle: u64, op: u32, request: Request) -> Result<OwnedOutput> {
    crate::session::ensure_runtime().map_err(|_| error("failed to initialize Lean runtime"))?;
    let mut p = ptr::null_mut();
    let status = unsafe { lnmai_v3_call(handle, op, &request, &mut p) };
    let out = OwnedOutput(p);
    if status != 0 {
        let message = if p.is_null() {
            format!("native FFI status {status}")
        } else {
            unsafe { CStr::from_ptr(lnmai_v3_error(p)) }
                .to_string_lossy()
                .into_owned()
        };
        return Err(error(message));
    }
    if p.is_null() {
        return Err(error("native FFI returned null output"));
    }
    Ok(out)
}
const BUTTONS: [ButtonZone; 8] = [
    ButtonZone::K1,
    ButtonZone::K2,
    ButtonZone::K3,
    ButtonZone::K4,
    ButtonZone::K5,
    ButtonZone::K6,
    ButtonZone::K7,
    ButtonZone::K8,
];
const SLOTS: [OuterSlot; 8] = [
    OuterSlot::S1,
    OuterSlot::S2,
    OuterSlot::S3,
    OuterSlot::S4,
    OuterSlot::S5,
    OuterSlot::S6,
    OuterSlot::S7,
    OuterSlot::S8,
];
const AREAS: [SensorArea; 33] = [
    SensorArea::A1,
    SensorArea::A2,
    SensorArea::A3,
    SensorArea::A4,
    SensorArea::A5,
    SensorArea::A6,
    SensorArea::A7,
    SensorArea::A8,
    SensorArea::B1,
    SensorArea::B2,
    SensorArea::B3,
    SensorArea::B4,
    SensorArea::B5,
    SensorArea::B6,
    SensorArea::B7,
    SensorArea::B8,
    SensorArea::C,
    SensorArea::D1,
    SensorArea::D2,
    SensorArea::D3,
    SensorArea::D4,
    SensorArea::D5,
    SensorArea::D6,
    SensorArea::D7,
    SensorArea::D8,
    SensorArea::E1,
    SensorArea::E2,
    SensorArea::E3,
    SensorArea::E4,
    SensorArea::E5,
    SensorArea::E6,
    SensorArea::E7,
    SensorArea::E8,
];
const KINDS: [JudgeEventKind; 5] = [
    JudgeEventKind::Tap,
    JudgeEventKind::Hold,
    JudgeEventKind::Slide,
    JudgeEventKind::Touch,
    JudgeEventKind::Break,
];
fn at<T: Copy>(items: &[T], index: u64) -> Result<T> {
    items
        .get(index as usize)
        .copied()
        .ok_or_else(|| error("invalid native enum tag"))
}
fn optional(word: u64) -> Option<u64> {
    (word != u64::MAX).then_some(word)
}
fn encode(event: &TimedInputEvent) -> InputEvent {
    let (time_us, kind, position, is_down) = match event {
        TimedInputEvent::ButtonClick { tp, zone } => (
            *tp,
            0,
            BUTTONS.iter().position(|z| z == zone).unwrap(),
            false,
        ),
        TimedInputEvent::ButtonHold { tp, zone, is_down } => (
            *tp,
            1,
            BUTTONS.iter().position(|z| z == zone).unwrap(),
            *is_down,
        ),
        TimedInputEvent::SensorClick { tp, area } => {
            (*tp, 2, AREAS.iter().position(|z| z == area).unwrap(), false)
        }
        TimedInputEvent::SensorHold { tp, area, is_down } => (
            *tp,
            3,
            AREAS.iter().position(|z| z == area).unwrap(),
            *is_down,
        ),
    };
    InputEvent {
        time_us,
        kind,
        position: position as u32,
        is_down: is_down as u32,
        reserved: 0,
    }
}
fn decode_event(w: &[u64; 8]) -> Result<TimedInputEvent> {
    let tp = w[1] as i64;
    Ok(match w[0] {
        0 => TimedInputEvent::ButtonClick {
            tp,
            zone: at(&BUTTONS, w[2])?,
        },
        1 => TimedInputEvent::ButtonHold {
            tp,
            zone: at(&BUTTONS, w[2])?,
            is_down: w[3] != 0,
        },
        2 => TimedInputEvent::SensorClick {
            tp,
            area: at(&AREAS, w[2])?,
        },
        3 => TimedInputEvent::SensorHold {
            tp,
            area: at(&AREAS, w[2])?,
            is_down: w[3] != 0,
        },
        _ => return Err(error("invalid native event")),
    })
}
pub(crate) fn create() -> Result<u64> {
    let out = call(0, 0, Request::default())?;
    out.records()
        .iter()
        .find(|r| r.tag == 0)
        .map(|r| r.w[0])
        .ok_or_else(|| error("missing native handle"))
}
pub(crate) fn load(handle: u64, text: &str, level: u32) -> Result<()> {
    call(
        handle,
        1,
        Request {
            text: text.as_ptr(),
            text_len: text.len() as u64,
            level_index: level,
            ..Default::default()
        },
    )?;
    Ok(())
}
pub(crate) fn free(handle: u64) -> Result<()> {
    call(handle, 7, Request::default())?;
    Ok(())
}
fn decode_step(records: &[Record]) -> Result<RuntimeStepLightResult> {
    let counts = NoteTypeJudgeCounts {
        tap_count: Default::default(),
        hold_count: Default::default(),
        slide_count: Default::default(),
        touch_count: Default::default(),
        break_count: Default::default(),
    };
    let mut result = RuntimeStepLightResult {
        events: vec![],
        audio_commands: vec![],
        render_commands: vec![],
        score: ScoreState {
            combo: 0,
            p_combo: 0,
            c_p_combo: 0,
            total_base: 0,
            total_extra: 0,
            earned_base: 0,
            earned_extra: 0,
            earned_classic_extra: 0,
            lost_base: 0,
            lost_extra: 0,
            lost_classic_extra: 0,
            dx_score: 0,
            max_dx_score: 0,
            fast_count: 0,
            late_count: 0,
            counts,
        },
        current_time: 0,
        terminated: false,
    };
    let mut seen = [false; 3];
    for Record { tag, w } in records {
        let s = &mut result.score;
        match tag {
            1 => {
                seen[0] = true;
                result.current_time = w[0] as i64;
                result.terminated = w[1] != 0;
            }
            2 => {
                seen[1] = true;
                s.combo = w[0];
                s.p_combo = w[1];
                s.c_p_combo = w[2];
                s.total_base = w[3];
                s.total_extra = w[4];
                s.earned_base = w[5];
                s.earned_extra = w[6];
                s.earned_classic_extra = w[7];
            }
            3 => {
                seen[2] = true;
                s.lost_base = w[0];
                s.lost_extra = w[1];
                s.lost_classic_extra = w[2];
                s.dx_score = w[3] as i64;
                s.max_dx_score = w[4];
                s.fast_count = w[5];
                s.late_count = w[6];
            }
            4 => {
                let map = match w[0] {
                    0 => &mut s.counts.tap_count,
                    1 => &mut s.counts.hold_count,
                    2 => &mut s.counts.slide_count,
                    3 => &mut s.counts.touch_count,
                    4 => &mut s.counts.break_count,
                    _ => return Err(error("invalid count family")),
                };
                map.insert(at(&ALL_JUDGE_GRADES, w[1])?, w[2]);
            }
            10 => {
                let button = (w[5] & 255)
                    .checked_sub(1)
                    .map(|v| at(&BUTTONS, v))
                    .transpose()?;
                let sensor = ((w[5] >> 8) & 255)
                    .checked_sub(1)
                    .map(|v| at(&AREAS, v))
                    .transpose()?;
                result.events.push(JudgeEvent {
                    kind: at(&KINDS, w[0])?,
                    phase: if w[1] == 0 {
                        JudgePhase::Head
                    } else {
                        JudgePhase::Tail
                    },
                    grade: at(&ALL_JUDGE_GRADES, w[2])?,
                    diff: w[3] as i64,
                    note_index: w[4],
                    position: RuntimePos { button, sensor },
                    is_break: w[6] & 1 != 0,
                    is_ex: w[6] & 2 != 0,
                    multiple: w[7],
                });
            }
            20 => result.audio_commands.push(match w[0] {
                0 => AudioCommand::PlayJudgeSfx {
                    kind: at(&KINDS, w[1])?,
                    grade: at(&ALL_JUDGE_GRADES, w[2])?,
                    is_break: w[3] != 0,
                    at_time: w[4] as i64,
                    note_index: w[5],
                },
                1 => AudioCommand::PlaySlideCue {
                    note_index: w[1],
                    track_index: w[2],
                    is_break: w[3] != 0,
                    at_time: w[4] as i64,
                },
                2 => AudioCommand::PlayTouchHoldBody {
                    note_index: w[1],
                    at_time: w[2] as i64,
                },
                _ => return Err(error("invalid audio command")),
            }),
            30 => result.render_commands.push(match w[0] {
                0 => RenderCommand::ShowJudgeResult {
                    kind: at(&KINDS, w[1])?,
                    grade: at(&ALL_JUDGE_GRADES, w[2])?,
                    is_break: w[3] != 0,
                    diff: w[4] as i64,
                    note_index: w[5],
                },
                1 => RenderCommand::UpdateSlideProgress {
                    note_index: w[1],
                    remaining: w[2],
                },
                2 => RenderCommand::UpdateSlideTrackProgress {
                    note_index: w[1],
                    track_index: w[2],
                    remaining: w[3],
                },
                3 => RenderCommand::HideAllSlideBars { note_index: w[1] },
                4 => RenderCommand::HideSlideBars {
                    note_index: w[1],
                    end_index: w[2],
                },
                5 => RenderCommand::HideSlideTrackBars {
                    note_index: w[1],
                    track_index: w[2],
                    end_index: w[3],
                },
                _ => return Err(error("invalid render command")),
            }),
            40 | 41 => (),
            _ => return Err(error("unexpected native result record")),
        }
    }
    if !seen.iter().all(|v| *v) {
        return Err(error("incomplete native score result"));
    }
    Ok(result)
}
pub(crate) fn step(handle: u64, batch: &TimedInputBatch) -> Result<RuntimeStepLightResult> {
    let events: Vec<_> = batch.events.iter().map(encode).collect();
    let out = call(
        handle,
        2,
        Request {
            time_us: batch.current_time,
            events: events.as_ptr(),
            event_count: events.len() as u64,
            ..Default::default()
        },
    )?;
    decode_step(out.records())
}
pub(crate) fn seek(
    handle: u64,
    target: i64,
    events: &[TimedInputEvent],
    keep_held: bool,
) -> Result<SeekResult> {
    let events: Vec<_> = events.iter().map(encode).collect();
    let out = call(
        handle,
        3,
        Request {
            time_us: target,
            level_index: u32::from(keep_held),
            events: events.as_ptr(),
            event_count: events.len() as u64,
            ..Default::default()
        },
    )?;
    let step = decode_step(out.records())?;
    Ok(SeekResult {
        score: step.score,
        current_time: step.current_time,
        terminated: step.terminated,
        render_commands: step.render_commands,
        retained_note_indices: out
            .records()
            .iter()
            .filter(|r| r.tag == 40)
            .map(|r| r.w[0])
            .collect(),
        skipped_note_indices: out
            .records()
            .iter()
            .filter(|r| r.tag == 41)
            .map(|r| r.w[0])
            .collect(),
    })
}
pub(crate) fn plan(handle: u64) -> Result<AutoplayPlan> {
    let out = call(handle, 4, Request::default())?;
    let mut plan = AutoplayPlan {
        version: 1,
        chart_fingerprint: String::new(),
        groups: vec![],
    };
    for Record { tag, w } in out.records() {
        match tag {
            50 => plan.groups.push(AutoplayGroup {
                id: w[0],
                kind: w[1],
                runtime_note_indices: vec![],
                parts: vec![],
                depends_on_group_ids: vec![],
            }),
            51 => {
                let g = plan
                    .groups
                    .last_mut()
                    .ok_or_else(|| error("part without group"))?;
                g.runtime_note_indices.push(w[0]);
                g.parts.push(AutoplayPart {
                    runtime_note_index: w[0],
                    role: w[1],
                    track_index: optional(w[2]),
                    events: vec![],
                });
            }
            52 => {
                let p = plan
                    .groups
                    .last_mut()
                    .and_then(|g| g.parts.last_mut())
                    .ok_or_else(|| error("event without part"))?;
                p.events.push(decode_event(w)?);
            }
            _ => return Err(error("invalid plan record")),
        }
    }
    Ok(plan)
}
pub(crate) fn bindings(handle: u64) -> Result<Vec<RuntimeSlideBinding>> {
    let out = call(handle, 6, Request::default())?;
    out.records()
        .iter()
        .map(|r| {
            if r.tag != 110 {
                return Err(error("invalid binding record"));
            }
            Ok(RuntimeSlideBinding {
                note_index: r.w[0],
                head_timing: r.w[1] as i64,
                total_judge_queue_len: r.w[2],
                track_count: r.w[3],
                initial_queue_remaining: r.w[4],
            })
        })
        .collect()
}

pub(crate) fn chart(handle: u64) -> Result<ChartSpec> {
    let out = call(handle, 5, Request::default())?;
    let mut c = ChartSpec {
        taps: vec![],
        holds: vec![],
        touches: vec![],
        touch_holds: vec![],
        slide_heads: vec![],
        slides: vec![],
        slide_skipping: None,
    };
    for Record { tag, w } in out.records() {
        match tag {
            100 => c.slide_skipping = optional(w[0]).map(|v| v != 0),
            101 => c.taps.push(TapChartNote {
                note_index: w[0],
                timing: w[1] as i64,
                slot: at(&SLOTS, w[2])?,
                is_break: w[3] & 1 != 0,
                is_ex: w[3] & 2 != 0,
                button_queue_index: w[4],
            }),
            102 => c.holds.push(HoldChartNote {
                note_index: w[0],
                timing: w[1] as i64,
                slot: at(&SLOTS, w[2])?,
                length: w[3] as i64,
                is_break: w[4] & 1 != 0,
                is_ex: w[4] & 2 != 0,
                is_touch: w[5] != 0,
                is_classic: optional(w[6]).map(|v| v != 0),
                button_queue_index: w[7],
                touch_hold_group_id: None,
                touch_hold_group_size: None,
            }),
            112 => {
                let n = c
                    .holds
                    .last_mut()
                    .ok_or_else(|| error("hold metadata without hold"))?;
                n.touch_hold_group_id = optional(w[0]);
                n.touch_hold_group_size = optional(w[1]);
            }
            103 => c.touches.push(TouchChartNote {
                note_index: w[0],
                timing: w[1] as i64,
                sensor_pos: at(&AREAS, w[2])?,
                is_break: w[3] != 0,
                source_group_id: optional(w[4]),
                source_group_index: optional(w[5]),
                source_group_size: optional(w[6]),
                touch_queue_index: w[7],
                touch_group_id: None,
                touch_group_size: None,
            }),
            113 => {
                let n = c
                    .touches
                    .last_mut()
                    .ok_or_else(|| error("touch metadata without touch"))?;
                n.touch_group_id = optional(w[0]);
                n.touch_group_size = optional(w[1]);
            }
            104 => c.touch_holds.push(TouchHoldChartNote {
                note_index: w[0],
                timing: w[1] as i64,
                sensor_pos: at(&AREAS, w[2])?,
                length: w[3] as i64,
                is_break: w[4] & 1 != 0,
                is_ex: w[4] & 2 != 0,
                source_group_id: optional(w[5]),
                source_group_index: optional(w[6]),
                source_group_size: optional(w[7]),
                touch_queue_index: 0,
                touch_group_id: None,
                touch_group_size: None,
                touch_hold_group_id: None,
                touch_hold_group_size: None,
            }),
            114 => {
                let n = c
                    .touch_holds
                    .last_mut()
                    .ok_or_else(|| error("touch-hold metadata without note"))?;
                n.touch_queue_index = w[0];
                n.touch_group_id = optional(w[1]);
                n.touch_group_size = optional(w[2]);
                n.touch_hold_group_id = optional(w[3]);
                n.touch_hold_group_size = optional(w[4]);
            }
            105 => c.slide_heads.push(SlideHeadChartNote {
                note_index: w[0],
                logical_slide_id: w[1],
                timing: w[2] as i64,
                slot: at(&SLOTS, w[3])?,
                is_break: w[4] & 1 != 0,
                is_ex: w[4] & 2 != 0,
            }),
            106 => c.slides.push(SlideChartNote {
                note_index: w[0],
                logical_slide_id: w[1],
                head_timing: w[2] as i64,
                start_timing: w[3] as i64,
                length: w[4] as i64,
                slot: at(&SLOTS, w[5])?,
                slide_kind: at(
                    &[
                        RuntimeSlideKind::Single,
                        RuntimeSlideKind::Wifi,
                        RuntimeSlideKind::ConnPart,
                    ],
                    w[6],
                )?,
                is_break: w[7] & 1 != 0,
                is_ex: w[7] & 2 != 0,
                is_classic: false,
                is_slide_no_head: false,
                is_conn_slide: false,
                parent_note_index: None,
                is_group_head: false,
                is_group_end: false,
                parent_finished: false,
                parent_pending_finish: false,
                total_judge_queue_len: 0,
                track_count: 0,
                judge_at: None,
                multiple: 1,
                judge_queues: vec![],
                debug_simai: None,
            }),
            107 => {
                let n = c
                    .slides
                    .last_mut()
                    .ok_or_else(|| error("slide metadata without slide"))?;
                n.parent_note_index = optional(w[0]);
                n.total_judge_queue_len = w[1];
                n.track_count = w[2];
                n.judge_at = (w[3] != i64::MIN as u64).then_some(w[3] as i64);
                n.multiple = w[4];
                n.is_classic = w[5] & 1 != 0;
                n.is_slide_no_head = w[5] & 2 != 0;
                n.is_conn_slide = w[6] & 1 != 0;
                n.is_group_head = w[6] & 2 != 0;
                n.is_group_end = w[7] & 1 != 0;
                n.parent_finished = w[7] & 2 != 0;
            }
            117 => {
                c.slides
                    .last_mut()
                    .ok_or_else(|| error("slide flag without slide"))?
                    .parent_pending_finish = w[0] != 0
            }
            108 => {
                let n = c
                    .slides
                    .last_mut()
                    .ok_or_else(|| error("track without slide"))?;
                if w[0] != n.judge_queues.len() as u64 {
                    return Err(error("invalid track order"));
                }
                n.judge_queues.push(vec![]);
            }
            109 => {
                let q = c
                    .slides
                    .last_mut()
                    .and_then(|n| n.judge_queues.last_mut())
                    .ok_or_else(|| error("area without track"))?;
                q.push(SlideAreaSpec {
                    target_areas: AREAS
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| w[0] & (1u64 << i) != 0)
                        .map(|(_, v)| *v)
                        .collect(),
                    policy: at(&[AreaPolicy::Or, AreaPolicy::And], w[1])?,
                    is_last: w[2] & 1 != 0,
                    is_skippable: w[2] & 2 != 0,
                    arrow_progress_when_on: w[3],
                    arrow_progress_when_finished: w[4],
                });
            }
            _ => return Err(error("invalid chart record")),
        }
    }
    Ok(c)
}
