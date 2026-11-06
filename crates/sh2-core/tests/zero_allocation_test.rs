//! Regression tests ensuring zero heap allocations during the 60 Hz simulation hot path.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;

struct TrackingAllocator {
    alloc_count: AtomicUsize,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            alloc_count: AtomicUsize::new(0),
        }
    }

    fn reset(&self) {
        self.alloc_count.store(0, Ordering::SeqCst);
    }

    fn allocations(&self) -> usize {
        self.alloc_count.load(Ordering::SeqCst)
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_count.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static TRACKER: TrackingAllocator = TrackingAllocator::new();

#[test]
fn test_zero_allocation_simulation_tick() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);

    // Warm-up tick to settle any initial lazy initialization
    for _ in 0..10 {
        sim.update(Duration::from_millis(16));
    }

    // Engage continuous locomotion & turning
    sim.input.state.left_stick_y = 1.0;
    sim.input.state.left_stick_x = 0.5;
    sim.input.state.is_running = true;

    for _ in 0..10 {
        sim.update(Duration::from_millis(16));
    }

    TRACKER.reset();

    // Execute 60 simulated 60Hz ticks (1 full second of active gameplay)
    for _ in 0..60 {
        sim.update(Duration::from_millis(16));
    }

    let allocs = TRACKER.allocations();
    assert_eq!(
        allocs, 0,
        "Simulation tick must be strictly zero-allocation; observed {} heap allocations",
        allocs
    );
}
