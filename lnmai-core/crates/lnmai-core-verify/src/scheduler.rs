//! Scheduler click ownership, frame order, and touch-group selection.
//!
//! The runtime keeps unbounded per-zone click counters and evaluates the
//! availability comparisons with its own arithmetic. This kernel fixes the
//! ownership decision itself: a button click is consumed before the mapped
//! sensor fallback, a consumed button suppresses that fallback, and input that
//! the current context does not accept consumes nothing.

/// Which counters one note consumes for a single frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickUse {
    pub used_button: bool,
    pub used_sensor: bool,
}

/// Button-then-sensor precedence. The two availability arguments are the
/// caller's used-less-than-available comparisons.
pub fn consume_click(
    allow_input: bool,
    button_available: bool,
    sensor_available: bool,
) -> ClickUse {
    if allow_input && button_available {
        ClickUse {
            used_button: true,
            used_sensor: false,
        }
    } else if allow_input && sensor_available {
        ClickUse {
            used_button: false,
            used_sensor: true,
        }
    } else {
        ClickUse {
            used_button: false,
            used_sensor: false,
        }
    }
}

/// The tap family accepts a queue click through the button or, when the button
/// is exhausted, through the fallback sensor area mapped from that button.
pub fn tap_queue_has_unused_click(button_available: bool, fallback_sensor_available: bool) -> bool {
    button_available || fallback_sensor_available
}

/// Touch families may also accept the button ring mapped from the sensor area.
/// The last argument is none when the area has no outer button zone.
#[allow(clippy::manual_unwrap_or, clippy::manual_unwrap_or_default)]
pub fn touch_input_available(
    button_ring_for_touch: bool,
    sensor_available: bool,
    outer_zone_available: Option<bool>,
) -> bool {
    let ring_available = match outer_zone_available {
        Some(available) => available,
        None => false,
    };
    sensor_available || (button_ring_for_touch && ring_available)
}

/// Processing order differs from the order in which events are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    Tap,
    Hold,
    Slide,
    Touch,
    TouchHold,
}

/// Thread the entire scheduler accumulator through each pass. In particular,
/// touch-holds receive the touch pass's cursor, frontiers, and group updates.
/// Note transitions and arithmetic belong to the caller's pass adapter.
pub fn process_frame<S, F>(state: S, step: F) -> S
where
    F: Fn(FramePhase, S) -> S,
{
    let state = step(FramePhase::Tap, state);
    let state = step(FramePhase::Hold, state);
    let state = step(FramePhase::Slide, state);
    let state = step(FramePhase::Touch, state);
    step(FramePhase::TouchHold, state)
}

/// Opaque batches keep unbounded runtime payloads out of the extracted kernel.
pub struct FrameEvents<E> {
    pub tap: E,
    pub hold: E,
    pub slide: E,
    pub touch: E,
    pub touch_hold: E,
}

/// `stepFrame` reports slide events last, even though slides run third.
/// The append adapter must preserve the order within both input batches.
pub fn combine_events<E, F>(events: FrameEvents<E>, append: F) -> E
where
    F: Fn(E, E) -> E,
{
    let out = append(events.tap, events.hold);
    let out = append(out, events.touch);
    let out = append(out, events.touch_hold);
    append(out, events.slide)
}

/// Slide commands precede body audio and judgment commands. The two append
/// adapters preserve order and are responsible for their buffer representations.
#[allow(clippy::too_many_arguments)]
pub fn combine_commands<A, R, FA, FR>(
    slide_audio: A,
    body_audio: A,
    event_audio: A,
    slide_render: R,
    event_render: R,
    append_audio: FA,
    append_render: FR,
) -> (A, R)
where
    FA: Fn(A, A) -> A,
    FR: Fn(R, R) -> R,
{
    let audio = append_audio(slide_audio, body_audio);
    let audio = append_audio(audio, event_audio);
    let render = append_render(slide_render, event_render);
    (audio, render)
}

/// The runtime supplies identifier equality and the strict-majority predicate
/// using its arbitrary-precision naturals. Majority is relevant only on a match.
pub struct TouchGroupProbe {
    pub matches_group: bool,
    pub has_majority: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchGroupAction {
    Append,
    Update { index: usize, keep_result: bool },
}

/// Select only the first matching group, including when that group has no
/// majority. A later duplicate must neither supply a result nor be updated.
/// An absent group is appended; an existing majority keeps its stored grade
/// and diff while the caller increments the count and replaces the size.
pub fn touch_group_plan<F>(len: usize, probe: F) -> TouchGroupAction
where
    F: Fn(usize) -> TouchGroupProbe,
{
    let mut index = 0;
    while index < len {
        let group = probe(index);
        if group.matches_group {
            return TouchGroupAction::Update {
                index,
                keep_result: group.has_majority,
            };
        }
        index += 1;
    }
    TouchGroupAction::Append
}
