//! The slide-body pass shared with the runtime scheduler.
//!
//! Slots before `index` have already received their lifecycle step. Parent
//! effects and readiness refresh still update the entire live collection, so
//! both earlier and later slots observe those effects. Only the index advances;
//! no cached pending tail can overwrite a parent update.

/// Forward-order buffers and the next slot to step. The runtime adapter reverses
/// the reference's initial accumulators before entering this kernel.
pub struct SlideWork<S, E, A, R> {
    pub slides: S,
    pub events: E,
    pub audio: A,
    pub render: R,
    pub index: usize,
}

/// Each method is one atomic adapter, never an iteration or a complete pass.
/// Named methods avoid higher-ranked borrowed closure bounds in extraction.
/// `read` clones one slot; `replace` clones only its replacement. Parent and
/// refresh operations preserve slot count and order. Append preserves both
/// batches' order; allocation/representation conditions belong to the adapter.
/// The lifecycle event batch contains zero or one event.
pub trait SlideSchedule<S, N, E, A, R> {
    fn read(&self, slides: &S, index: usize) -> N;
    fn step(&self, note: &N) -> (N, E, A, R);
    fn replace(&self, slides: S, index: usize, note: &N) -> S;
    fn force_direct(&self, slides: S, before: &N, after: &N) -> (S, R);
    fn end_ancestors(&self, slides: S, before: &N, after: &N) -> (S, R);
    fn refresh(&self, slides: S) -> S;
    fn append_events(&self, accumulated: E, batch: E) -> E;
    fn append_audio(&self, accumulated: A, batch: A) -> A;
    fn append_render(&self, accumulated: R, batch: R) -> R;
}

/// The guard and length-preserving adapters justify every slot read/write and
/// increment. `len - index` decreases, bounding the pass by pending slot count
/// without computing the reference's potentially overflowing `pending + 1`.
pub fn process<S, N, E, A, R, Ops>(
    len: usize,
    mut work: SlideWork<S, E, A, R>,
    ops: Ops,
) -> SlideWork<S, E, A, R>
where
    Ops: SlideSchedule<S, N, E, A, R>,
{
    while work.index < len {
        let before = ops.read(&work.slides, work.index);
        let (after, events, audio, render) = ops.step(&before);
        work.slides = ops.replace(work.slides, work.index, &after);
        work.render = ops.append_render(work.render, render);
        let (slides, direct_render) = ops.force_direct(work.slides, &before, &after);
        work.slides = slides;
        work.render = ops.append_render(work.render, direct_render);
        let (slides, ancestor_render) = ops.end_ancestors(work.slides, &before, &after);
        work.slides = slides;
        work.render = ops.append_render(work.render, ancestor_render);
        work.slides = ops.refresh(work.slides);
        work.events = ops.append_events(work.events, events);
        work.audio = ops.append_audio(work.audio, audio);
        work.index += 1;
    }
    work
}
