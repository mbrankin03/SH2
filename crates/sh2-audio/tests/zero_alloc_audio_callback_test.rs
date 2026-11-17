//! Strict `SH2-49` & `SH2-84` zero-allocation real-time audio callback verification.
//!
//! Installs a global tracking allocator and executes 48,000 real-time audio callback samples
//! (`1.0 s` at `48 kHz`) across `.cld` wall occlusion transitions, 100 ms log-cutoff ramps,
//! Binaural HRTF ITD/ILD/pinna filtering, Classic PS2 Stereo toggling, and wet corridor reverb
//! send synthesis — asserting strictly **zero** heap allocations on the audio callback path.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use glam::Vec3;
use kira::Frame;
use sh2_audio::*;
use sh2_core::collision::CollisionWorld;

thread_local! {
    static AUDIO_CB_TRACKING_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

struct AudioCallbackTrackingAllocator {
    alloc_count: AtomicUsize,
    is_enabled: AtomicBool,
}

impl AudioCallbackTrackingAllocator {
    const fn new() -> Self {
        Self {
            alloc_count: AtomicUsize::new(0),
            is_enabled: AtomicBool::new(false),
        }
    }

    fn enable(&self) {
        self.alloc_count.store(0, Ordering::SeqCst);
        self.is_enabled.store(true, Ordering::SeqCst);
        AUDIO_CB_TRACKING_ACTIVE.with(|t| t.set(true));
    }

    fn disable(&self) {
        self.is_enabled.store(false, Ordering::SeqCst);
        AUDIO_CB_TRACKING_ACTIVE.with(|t| t.set(false));
    }

    fn allocations(&self) -> usize {
        self.alloc_count.load(Ordering::SeqCst)
    }
}

unsafe impl GlobalAlloc for AudioCallbackTrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.is_enabled.load(Ordering::Relaxed) && AUDIO_CB_TRACKING_ACTIVE.with(|t| t.get()) {
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static ALLOC_TRACKER: AudioCallbackTrackingAllocator = AudioCallbackTrackingAllocator::new();

#[test]
fn test_sh2_84_zero_heap_allocation_on_realtime_audio_callback_thread() {
    let fs = 48_000u32;
    let dt = 1.0 / (fs as f64);

    // Pre-allocate static waveform & collision world on the main/setup thread (outside real-time callback)
    let mut waveform = Vec::with_capacity(8_192);
    for i in 0..8_192 {
        let t = (i as f32) / (fs as f32);
        let s = (2.0 * std::f32::consts::PI * 330.0 * t).sin() * 0.4
            + (2.0 * std::f32::consts::PI * 3_200.0 * t).sin() * 0.3;
        waveform.push(Frame::new(s, s));
    }
    let frames_arc: Arc<[Frame]> = Arc::from(waveform);
    let loop_region = LoopRegionSamples::from_source_samples(512, 7_680, fs, fs, 8_192);

    let world = CollisionWorld::create_synthetic_room(6000.0, 6000.0, 2000.0, 800.0, 800.0);
    let hrtf_flag = Arc::new(AtomicBool::new(true));
    let occ_flag = Arc::new(AtomicBool::new(true));

    let init = SpatialVoiceInitParams {
        base_distance_gain: 0.85,
        stereo_pan: 0.0,
        angles: HrtfSphericalAngles::default(),
        occlusion: OcclusionDspParams::unoccluded(),
    };
    let control = SpatialVoiceControl::new_with_flags(
        &init,
        Arc::clone(&hrtf_flag),
        Arc::clone(&occ_flag),
        false,
    );
    let mut dry_core = SpatialDryWetCore::new(Arc::clone(&frames_arc), fs, loop_region, &init);
    let mut wet_core = SpatialWetSendCore::new(frames_arc, fs, loop_region, &init);

    // Enter strict real-time callback allocation guard (`SH2-49`)
    ALLOC_TRACKER.enable();

    let mut sink_acc = 0.0f32;
    // Simulate 375 blocks of 128 frames = 48,000 stereo frames (1 full second at 48 kHz)
    for block in 0..375usize {
        // Orbit monster around pillar (crossing occluded <-> unoccluded doorway thresholds & Above/Below elevation)
        let angle = (block as f32) * 0.05;
        let emitter_pos = Vec3::new(
            angle.sin() * 1800.0,
            -600.0 - 500.0 * (angle * 0.7).cos(),
            angle.cos() * 1800.0,
        );
        let listener_feet = Vec3::new(0.0, 0.0, -2000.0);
        let occ = evaluate_world_occlusion(&world, emitter_pos, listener_feet, false);
        let head = sh2_core::listener_head_position(listener_feet, false);
        let angles = compute_listener_spherical_angles(head, 0.0, 0.0, emitter_pos);
        let eval = compute_se_call_pos_distance_f(listener_feet, 0.0, emitter_pos, 1.0);

        // Toggle Binaural HRTF <-> Classic PS2 Stereo and Wall Occlusion ON <-> OFF mid-stream (zero-alloc)
        hrtf_flag.store(block % 64 != 32, Ordering::Relaxed);
        occ_flag.store(block % 96 != 48, Ordering::Relaxed);

        control.update_spatial_and_occlusion(&SpatialVoiceInitParams {
            base_distance_gain: eval.effective_gain,
            stereo_pan: eval.stereo_pan,
            angles,
            occlusion: occ,
        });

        // Audio callback `on_start_processing`
        dry_core.sync_from_control(&control, fs as f32);

        // Audio callback `process` loop (128 frames per hardware period)
        for _ in 0..128 {
            let (dry, _wet_inline) = dry_core.process_pair(dt, &control);
            let wet = wet_core.process_wet_only(dt, &control);
            sink_acc += dry.left + dry.right + wet.left + wet.right;
        }
    }

    let observed_allocs = ALLOC_TRACKER.allocations();
    ALLOC_TRACKER.disable();

    assert!(sink_acc.is_finite());
    assert_eq!(
        observed_allocs, 0,
        "Real-time audio callback DSP & .cld occlusion raycast must allocate 0 bytes on heap (observed {observed_allocs})"
    );
}
