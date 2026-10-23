//! Keep idle-frame work from becoming quadratic in the chart slide count.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Relaxed);
        System.realloc(ptr, layout, size)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn idle_frame_allocations(slides: usize) -> usize {
    let text = format!("&inote_1=(120){{4}}{}E", "1-5[4:1],".repeat(slides));
    let spec = lnmai_core::simai::frontend::frontend_lowered_chart(&text, 1.into()).unwrap();
    let state = lnmai_core::chart_loader::build_game_state(&spec);
    let batch = lnmai_core::input_model::TimedInputBatch::default();
    let before = ALLOCATIONS.load(Relaxed);
    let result = lnmai_core::scheduler::step_frame_timed(&state, &batch);
    std::hint::black_box(&result);
    ALLOCATIONS.load(Relaxed) - before
}

#[test]
fn doubling_slides_keeps_frame_allocations_linear() {
    let small = idle_frame_allocations(64);
    let large = idle_frame_allocations(128);
    assert!(
        large < small * 3,
        "doubling slides must not copy the full chart for every slide: {small} -> {large} allocations"
    );
}
