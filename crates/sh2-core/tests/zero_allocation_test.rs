//! Regression tests ensuring zero heap allocations during the 60 Hz simulation hot path.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;

thread_local! {
    static TRACKING_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

struct TrackingAllocator {
    alloc_count: AtomicUsize,
    is_enabled: AtomicBool,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            alloc_count: AtomicUsize::new(0),
            is_enabled: AtomicBool::new(false),
        }
    }

    fn enable(&self) {
        self.alloc_count.store(0, Ordering::SeqCst);
        self.is_enabled.store(true, Ordering::SeqCst);
        TRACKING_ACTIVE.with(|t| t.set(true));
    }

    fn disable(&self) {
        self.is_enabled.store(false, Ordering::SeqCst);
        TRACKING_ACTIVE.with(|t| t.set(false));
    }

    fn allocations(&self) -> usize {
        self.alloc_count.load(Ordering::SeqCst)
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.is_enabled.load(Ordering::Relaxed) && TRACKING_ACTIVE.with(|t| t.get()) {
            let count = self.alloc_count.fetch_add(1, Ordering::Relaxed);
            if count == 0 {
                let backtrace = std::backtrace::Backtrace::capture();
                eprintln!(
                    "[HOT PATH ALLOC DETECTED] size={} align={}\n{:?}",
                    layout.size(),
                    layout.align(),
                    backtrace
                );
            }
        }
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
    let mut arena = sh2_core::combat::CombatArena::new();

    // Warm-up ticks to settle any initial lazy initialization
    for i in 0..30 {
        sim.update(Duration::from_millis(16));
        arena.tick_isolated(false, false, false, 1.0 / 60.0, i % arena.enemies.len());
    }

    // Engage continuous locomotion & turning
    sim.input.state.left_stick_y = 1.0;
    sim.input.state.left_stick_x = 0.5;
    sim.input.state.is_running = true;

    for i in 0..120 {
        sim.update(Duration::from_millis(16));
        arena.tick_isolated(
            true,
            i % 15 == 0,
            false,
            1.0 / 60.0,
            i % arena.enemies.len(),
        );
    }

    let mut secondary = sh2_core::SecondaryBonePhysics::for_james();
    let mut locals_scratch = [[[0.0f32; 4]; 4]; 41];
    for m in &mut locals_scratch {
        m[0][0] = 1.0;
        m[1][1] = 1.0;
        m[2][2] = 1.0;
        m[3][3] = 1.0;
    }
    secondary.step(
        1.0 / 60.0,
        sim.scenario.player_position,
        sim.scenario.player_yaw,
    );

    TRACKER.enable();

    // Execute 240 simulated 240Hz render frames (1 full second of 240 Hz OLED gameplay + 60 Hz deterministic ticks)
    for i in 0..240 {
        let ticks = sim.update(Duration::from_nanos(4_166_667));
        for _ in 0..ticks {
            arena.tick_isolated(
                true,
                i % 20 == 0,
                false,
                1.0 / 60.0,
                i % arena.enemies.len(),
            );
            secondary.step(
                1.0 / 60.0,
                sim.scenario.player_position,
                sim.scenario.player_yaw,
            );
        }
        let alpha = sim.interpolation_alpha();
        let _pose = sim.interpolated_player_pose(alpha);
        let _ray = sim.interpolated_flashlight_ray(alpha);
        let _fog_t = sim.deterministic_fog_time(alpha);
        let _cursor = sim.animation.evaluate_blended_cursor_at_alpha(alpha);
        secondary.apply_to_locals_interpolated(&mut locals_scratch, alpha);
    }

    let allocs = TRACKER.allocations();
    TRACKER.disable();
    assert_eq!(
        allocs, 0,
        "Simulation tick must be strictly zero-allocation; observed {} heap allocations",
        allocs
    );
}
