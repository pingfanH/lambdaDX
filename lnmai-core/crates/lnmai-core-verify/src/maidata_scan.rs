//! Maidata line and consecutive-event scans shared with the runtime facade.
//! All cursors refer to one immutable input; no operation owns a remaining tail.
//! UTF-8/header parsing, exact event keys, Natural ids and container writes are
//! point adapters. No adapter may collect a chart body or process a token group.

/// Half-open input interval. Kernels construct start <= end <= input length.
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Reads inspect one line or one already-parsed header. `header` splits only
/// at the first '=' and trims its two fields; it never reads subsequent lines.
pub trait LineRead<Header> {
    fn is_blank(&self, index: usize) -> bool;
    fn starts_with_amp(&self, index: usize) -> bool;
    fn header(&self, index: usize) -> Option<Header>;
    fn is_chart_header(&self, header: &Header) -> bool;
}

/// Each write appends one item or one line. `begin_chart` retains the header's
/// value as the initial body and parses its level, with invalid suffix -> zero.
/// `append_body_line` adds exactly '\n' then the ORIGINAL, untrimmed input line.
/// `append_chart` merely appends the already assembled body/level record.
pub trait LineWrite<Header, Fields, Charts, Body> {
    fn append_field(&self, fields: Fields, header: Header) -> Fields;
    fn begin_chart(&self, header: Header) -> Body;
    fn append_body_line(&self, body: Body, index: usize) -> Body;
    fn append_chart(&self, charts: Charts, body: Body) -> Charts;
}

pub enum LinePlan<Header> {
    Skip(usize),
    Metadata(Header, usize),
    Chart(Header, Span),
}

/// Find the first following '&' line without consuming it. Require start <= len
/// and len equal to the borrowed source length. Empty/whitespace body lines stay.
pub fn chart_body_end<Header, Ops>(len: usize, start: usize, ops: &Ops) -> usize
where
    Ops: LineRead<Header>,
{
    let mut cursor = start;
    while cursor < len {
        if ops.starts_with_amp(cursor) {
            break;
        }
        cursor += 1;
    }
    cursor
}

/// One header decision and its precise next cursor. Require cursor < len.
/// Blank takes priority; non-'&' lines outside a chart body are ignored.
pub fn plan_line<Header, Ops>(len: usize, cursor: usize, ops: &Ops) -> LinePlan<Header>
where
    Ops: LineRead<Header>,
{
    // cursor < len <= usize::MAX establishes that this increment is safe.
    let next = cursor + 1;
    if ops.is_blank(cursor) {
        LinePlan::Skip(next)
    } else if ops.starts_with_amp(cursor) {
        match ops.header(cursor) {
            None => LinePlan::Skip(next),
            Some(header) => {
                if ops.is_chart_header(&header) {
                    let end = chart_body_end::<Header, Ops>(len, next, ops);
                    LinePlan::Chart(header, Span { start: next, end })
                } else {
                    LinePlan::Metadata(header, next)
                }
            }
        }
    } else {
        LinePlan::Skip(next)
    }
}

/// Full line collection. Output prefixes are FORWARD, unlike the original
/// Lean helper's reverse accumulators. Public runtime entry supplies empty ones.
/// Repeated metadata keys and chart levels remain distinct ordered appends.
pub fn collect_lines<Header, Fields, Charts, Body, Ops>(
    len: usize,
    mut fields: Fields,
    mut charts: Charts,
    ops: Ops,
) -> (Fields, Charts)
where
    Ops: LineRead<Header> + LineWrite<Header, Fields, Charts, Body>,
{
    let mut cursor = 0usize;
    while cursor < len {
        match plan_line::<Header, Ops>(len, cursor, &ops) {
            LinePlan::Skip(next) => cursor = next,
            LinePlan::Metadata(header, next) => {
                fields = ops.append_field(fields, header);
                cursor = next;
            }
            LinePlan::Chart(header, range) => {
                let mut body = ops.begin_chart(header);
                let mut line = range.start;
                while line < range.end {
                    body = ops.append_body_line(body, line);
                    line += 1;
                }
                charts = ops.append_chart(charts, body);
                cursor = range.end;
            }
        }
    }
    (fields, charts)
}

/// Only distinctions used by the Each classifier; all other kinds are Other.
pub enum TouchKind {
    Slide,
    Touch,
    TouchHold,
    Other,
}

/// Each read observes one token, or compares the exact event-key fields of two
/// tokens. `same_event` performs no scan and may not depend on group metadata.
pub trait TouchRead {
    fn same_event(&self, head: usize, index: usize) -> bool;
    fn kind(&self, index: usize) -> TouchKind;
    fn source_group_index_nonzero(&self, index: usize) -> bool;
    fn is_slide_no_head(&self, index: usize) -> bool;
}

/// Append exactly one source token, either unchanged or changing only the three
/// group fields. GroupId remains arbitrary precision; it is never cast to usize.
/// The two bounded usize fields are converted exactly by the runtime adapter.
pub trait TouchWrite<GroupId, Output> {
    fn append_unchanged(&self, output: Output, source: usize) -> Output;
    fn append_tagged(
        &self,
        output: Output,
        source: usize,
        group_id: &GroupId,
        group_index: usize,
        group_size: usize,
    ) -> Output;
    fn next_group(&self, group_id: GroupId) -> GroupId;
}

pub struct TouchGroupPlan {
    pub range: Span,
    pub timing_note_count: usize,
    pub no_head_slide_count: usize,
}

/// Include the head unconditionally, then stop at the FIRST different event.
/// Equal event keys after that boundary belong to a later group. Require start < len.
pub fn same_event_end<Ops>(len: usize, start: usize, ops: &Ops) -> usize
where
    Ops: TouchRead,
{
    let mut end = start + 1;
    while end < len {
        if !ops.same_event(start, end) {
            break;
        }
        end += 1;
    }
    end
}

/// Range discovery and both counters are kernel control flow, not callbacks.
/// Expanded slide children contribute to neither count, including headless ones.
pub fn plan_touch_group<Ops>(len: usize, start: usize, ops: &Ops) -> TouchGroupPlan
where
    Ops: TouchRead,
{
    let end = same_event_end(len, start, ops);
    let mut cursor = start;
    let mut timing_note_count = 0usize;
    let mut no_head_slide_count = 0usize;
    while cursor < end {
        match ops.kind(cursor) {
            TouchKind::Slide => {
                if !ops.source_group_index_nonzero(cursor) {
                    timing_note_count += 1;
                    if ops.is_slide_no_head(cursor) {
                        no_head_slide_count += 1;
                    }
                }
            }
            _ => timing_note_count += 1,
        }
        cursor += 1;
    }
    TouchGroupPlan {
        range: Span { start, end },
        timing_note_count,
        no_head_slide_count,
    }
}

/// Emit this range once, in source order, preserving all untagged payloads.
/// Only tagged tokens advance group_index; group_size is the timing count.
pub fn emit_touch_group<GroupId, Output, Ops>(
    plan: &TouchGroupPlan,
    group_id: &GroupId,
    mut output: Output,
    ops: &Ops,
) -> Output
where
    Ops: TouchRead + TouchWrite<GroupId, Output>,
{
    let non_no_head_count = plan
        .timing_note_count
        .saturating_sub(plan.no_head_slide_count);
    let touch_is_each = plan.timing_note_count > 1 && non_no_head_count != 1;
    let touch_hold_is_each = plan.timing_note_count > 1;
    let mut cursor = plan.range.start;
    let mut group_index = 0usize;
    while cursor < plan.range.end {
        let should_tag = match ops.kind(cursor) {
            TouchKind::Touch => touch_is_each,
            TouchKind::TouchHold => touch_hold_is_each,
            _ => false,
        };
        if should_tag {
            output = ops.append_tagged(
                output,
                cursor,
                group_id,
                group_index,
                plan.timing_note_count,
            );
            group_index += 1;
        } else {
            output = ops.append_unchanged(output, cursor);
        }
        cursor += 1;
    }
    output
}

/// Consecutive groups only. Each iteration consumes at least its head, even if
/// no token is tagged. Output grows by exactly len items; no tail is copied.
pub fn annotate_touch_each<GroupId, Output, Ops>(
    len: usize,
    mut group_id: GroupId,
    mut output: Output,
    ops: Ops,
) -> Output
where
    Ops: TouchRead + TouchWrite<GroupId, Output>,
{
    let mut cursor = 0usize;
    while cursor < len {
        let plan = plan_touch_group(len, cursor, &ops);
        output = emit_touch_group(&plan, &group_id, output, &ops);
        cursor = plan.range.end;
        group_id = ops.next_group(group_id);
    }
    output
}
