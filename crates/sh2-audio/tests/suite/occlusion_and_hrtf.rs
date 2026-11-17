//! Real-time `.cld` raycast acoustic occlusion, 100 ms threshold smoothing, corridor reverb bleed,
//! and Binaural HRTF 3D headphone spatialization tests (`SH2-84`).

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use glam::Vec3;
use kira::Frame;
use sh2_assets::formats::cld::{parse_cld, FloorTriangle, WallSegment};
use sh2_audio::*;
use sh2_core::collision::{cld_material_barrier_weight, CollisionController, CollisionWorld};
use sh2_core::{
    listener_head_position, SpatialAudioMode, JAMES_HEAD_OFFSET_Y, MARIA_HEAD_OFFSET_Y,
};

#[test]
fn test_sh2_84_cld_raycast_wall_door_and_slab_occlusion() {
    // 0. Verify exact retail `.mdl` bind eye heights (`vcMakeHeroHeadPos` `0x0019e320`) & unmasked `.cld` material 99
    assert!((JAMES_HEAD_OFFSET_Y - (-844.6)).abs() < 1e-3);
    assert!((MARIA_HEAD_OFFSET_Y - (-825.6)).abs() < 1e-3);
    assert!((cld_material_barrier_weight(99) - 1.0).abs() < 1e-5);
    assert!((cld_material_barrier_weight(7) - 0.85).abs() < 1e-5);
    assert!((cld_material_barrier_weight(5) - 1.15).abs() < 1e-5);

    // 1. Synthetic room with a 600x600 solid center pillar (-300..300, -300..300)
    let mut world = CollisionWorld::create_synthetic_room(6000.0, 6000.0, 2000.0, 600.0, 600.0);

    // Clear hallway line-of-sight along X = -1200 (west of the center pillar)
    let player_clear = Vec3::new(-1200.0, 0.0, -1500.0);
    let emitter_clear = Vec3::new(-1200.0, -400.0, 1500.0);
    let clear_params = evaluate_world_occlusion(&world, emitter_clear, player_clear, false);
    assert!(!clear_params.occluded);
    assert_eq!(clear_params.wall_hit_count, 0);
    assert!((clear_params.dry_cutoff_hz - UNOCCLUDED_CUTOFF_HZ).abs() < 1e-3);
    assert_eq!(clear_params.dry_attenuation_db, 0.0);
    assert!((clear_params.dry_gain_scale - 1.0).abs() < 1e-5);
    assert!((clear_params.wet_reverb_bleed_scale - 1.0).abs() < 1e-5);

    // Occluded across the center pillar (2 distinct walls 600u apart: Z = -300 and Z = +300)
    let player_south = Vec3::new(0.0, 0.0, -1500.0);
    let emitter_north = Vec3::new(0.0, -400.0, 1500.0);
    let pillar_params = evaluate_world_occlusion(&world, emitter_north, player_south, false);
    assert!(pillar_params.occluded);
    assert_eq!(pillar_params.wall_hit_count, 2);
    assert!((pillar_params.dry_cutoff_hz - OCCLUDED_MIN_CUTOFF_HZ).abs() < 1e-3);
    assert!((pillar_params.dry_attenuation_db - OCCLUDED_MAX_ATTENUATION_DB).abs() < 1e-3);
    assert!(pillar_params.wet_reverb_bleed_scale >= 0.85);

    // 2. Single wooden door (`wldofs` `group == 1`, `.cld` `material == 7`) vs `cedofs` (`group == 2`) vs `swdofs` (`group == 3`)
    world.walls.clear();
    world.add_extra_wall(WallSegment::new(
        [-800.0, 0.0],
        [800.0, 0.0],
        -1500.0,
        0.0,
        1, // wldofs main wall/door group (`SLUS_202.28` `clCheckHitEyeVector` `0x00139420`)
        7, // Wood door material
    ));
    let door_params = evaluate_world_occlusion(
        &world,
        Vec3::new(0.0, -450.0, 1200.0),
        Vec3::new(0.0, 0.0, -1200.0),
        false,
    );
    assert!(door_params.occluded);
    assert_eq!(door_params.wall_hit_count, 1);
    assert!((door_params.dry_cutoff_hz - 850.0).abs() < 1.0);
    assert!((door_params.dry_attenuation_db - (-6.0)).abs() < 0.1);
    assert!((door_params.wet_reverb_bleed_scale - 1.0).abs() < 1e-5);

    // `cedofs` (`group == 2` upper wall/lintel) MUST occlude (`clCheckHitEyeVector` `0x0013958c`),
    // while `swdofs` (`group == 3` character-only low walk curb) MUST be skipped!
    world.extra_walls.clear();
    world.add_extra_wall(WallSegment::new(
        [-800.0, 0.0],
        [800.0, 0.0],
        -1600.0,
        -600.0,
        2,
        0,
    ));
    let ced_params = evaluate_world_occlusion(
        &world,
        Vec3::new(0.0, -850.0, 1200.0),
        Vec3::new(0.0, 0.0, -1200.0),
        false,
    );
    assert!(
        ced_params.occluded && ced_params.wall_hit_count == 1,
        "SLUS_202.28 cedofs (group == 2) upper wall/lintel must occlude acoustic rays"
    );

    world.extra_walls.clear();
    world.add_extra_wall(WallSegment::new(
        [-800.0, 0.0],
        [800.0, 0.0],
        -1500.0,
        0.0,
        3,
        0,
    ));
    let swd_params = evaluate_world_occlusion(
        &world,
        Vec3::new(0.0, -850.0, 1200.0),
        Vec3::new(0.0, 0.0, -1200.0),
        false,
    );
    assert!(
        !swd_params.occluded,
        "SLUS_202.28 swdofs (group == 3) character-walk curb must NOT occlude sight/acoustic rays"
    );

    // 3. Multi-story stairwell floor slab occlusion (including floor-level 2F emitter right on `Y = -1600.0`!)
    world.extra_walls.clear();
    world.floors.clear();
    // 1F walkable floor at Y = 0.0 (must NOT falsely occlude!) + 2F floor deck at Y = -1600.0
    world.floors.push(FloorTriangle::new(
        [
            [-1000.0, 0.0, -1000.0],
            [1000.0, 0.0, -1000.0],
            [1000.0, 0.0, 1000.0],
        ],
        0,
    ));
    world.floors.push(FloorTriangle::new(
        [
            [-1000.0, -1600.0, -1000.0],
            [1000.0, -1600.0, -1000.0],
            [1000.0, -1600.0, 1000.0],
        ],
        0,
    ));
    let ctrl = CollisionController::new(world, Default::default());
    let slab_params = evaluate_controller_occlusion(
        &ctrl,
        Vec3::new(200.0, -2400.0, -200.0), // Upstairs above deck
        Vec3::new(200.0, 0.0, -200.0),     // James on 1F (`head_y = -844.6`)
        false,
    );
    assert!(slab_params.occluded);
    assert_eq!(slab_params.slab_hit_count, 1);
    assert!((OCCLUDED_MIN_CUTOFF_HZ..=OCCLUDED_MAX_CUTOFF_HZ).contains(&slab_params.dry_cutoff_hz));

    // Floor-level upstairs emitter authored directly on the 2F floor (`emitter_pos.y == -1600.0`, e.g. `SE_APART_E3FE_FOOTSTEPS`)
    let floor_level_2f = evaluate_controller_occlusion(
        &ctrl,
        Vec3::new(200.0, -1600.0, -200.0),
        Vec3::new(200.0, 0.0, -200.0),
        false,
    );
    assert!(
        floor_level_2f.occluded && floor_level_2f.slab_hit_count == 1,
        "Emitter sitting directly on 2F floor slab (y = -1600) must be occluded by 2F slab!"
    );

    // 4. `SLUS_202.28` `clCheckHitEyeVectorWall` (`0x00139b90`) & `clCheckHitEyesOnlyFloorThru` (`0x00139010`):
    //    `material == 9` (chainlink fence), `11` (Toluca Prison iron cell bars / Labyrinth open metal floor grate),
    //    and `12` (invisible walk-containment boundary in `er01.cld`/`rr21.cld`) MUST be acoustically `Thru` (unoccluded)!
    for thru_mat in [9u32, 11, 12] {
        let mut thru_world = CollisionWorld::new();
        thru_world.add_extra_wall(WallSegment::new(
            [-1000.0, 0.0],
            [1000.0, 0.0],
            -1600.0,
            0.0,
            1,
            thru_mat,
        ));
        thru_world.floors.push(FloorTriangle::new(
            [
                [-1000.0, -1600.0, -1000.0],
                [1000.0, -1600.0, -1000.0],
                [1000.0, -1600.0, 1000.0],
            ],
            thru_mat,
        ));
        let thru_wall_eval = evaluate_world_occlusion(
            &thru_world,
            Vec3::new(0.0, -500.0, 1200.0),
            Vec3::new(0.0, 0.0, -1200.0),
            false,
        );
        assert!(
            !thru_wall_eval.occluded,
            "SLUS_202.28 Thru wall material {thru_mat} (0x00139b90) must NOT muffle sound"
        );
        let thru_grate_eval = evaluate_world_occlusion(
            &thru_world,
            Vec3::new(200.0, -2400.0, -200.0),
            Vec3::new(200.0, 0.0, -200.0),
            false,
        );
        assert!(
            !thru_grate_eval.occluded,
            "SLUS_202.28 Thru floor grate material {thru_mat} (0x00139010) must NOT muffle vertical Mandarin/stairwell sound"
        );
    }
}

#[test]
fn test_sh2_84_retail_apartment_and_hospital_cld_occlusion() {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let ap01_path = resolver.data_dir.join("bg/ap/ap01.cld");
    if !Path::new(&ap01_path).exists() {
        return;
    }

    let ap01_bytes = fs::read(&ap01_path).expect("read ap01.cld");
    let cld = parse_cld(&ap01_bytes).expect("parse ap01.cld");
    let world = CollisionWorld::from_cld(&cld);
    assert!(!world.walls.is_empty());

    // Pick a real full-height solid interior apartment wall in `ap01.cld` (`y_top <= -1100`, `y_bottom >= -25`)
    let sample_wall = world
        .walls
        .iter()
        .find(|w| {
            w.group != 3
                && !sh2_core::collision::is_cld_material_thru(w.material)
                && w.y_top <= -1100.0
                && w.y_bottom >= -25.0
                && ((w.end[0] - w.start[0]).hypot(w.end[1] - w.start[1]) > 500.0)
        })
        .copied()
        .expect("ap01.cld must contain full-height apartment walls");

    let mid_x = 0.5 * (sample_wall.start[0] + sample_wall.end[0]);
    let mid_z = 0.5 * (sample_wall.start[1] + sample_wall.end[1]);
    let dx = sample_wall.end[0] - sample_wall.start[0];
    let dz = sample_wall.end[1] - sample_wall.start[1];
    let len = dx.hypot(dz);
    let nx = -dz / len;
    let nz = dx / len;

    // Place James on one side of the apartment wall and a monster emitter on the opposite side
    let james_feet = Vec3::new(mid_x + nx * 600.0, -15.0, mid_z + nz * 600.0);
    let monster_pos = Vec3::new(mid_x - nx * 600.0, -500.0, mid_z - nz * 600.0);

    let occ = evaluate_world_occlusion(&world, monster_pos, james_feet, false);
    assert!(
        occ.occluded,
        "Ray across ap01.cld apartment wall at James head height ({}) must be occluded",
        james_feet.y + JAMES_HEAD_OFFSET_Y
    );
    assert!(
        (OCCLUDED_MIN_CUTOFF_HZ..=OCCLUDED_MAX_CUTOFF_HZ).contains(&occ.dry_cutoff_hz),
        "Occluded cutoff {} Hz must lie within 650..=850 Hz",
        occ.dry_cutoff_hz
    );
    assert!(
        (OCCLUDED_MAX_ATTENUATION_DB..=OCCLUDED_MIN_ATTENUATION_DB)
            .contains(&occ.dry_attenuation_db),
        "Occluded direct-path attenuation {} dB must lie within -10..=-6 dB",
        occ.dry_attenuation_db
    );
}

#[test]
fn test_sh2_84_dry_lpf_muffling_vs_wet_corridor_reverb_bleed_and_100ms_smoothing() {
    let fs = 48_000u32;
    let n = 9_600usize; // 200 ms @ 48 kHz

    // Synthesize a 4 kHz high-frequency scraping tone (`f_hi = 4000 Hz`)
    let mut hi_frames = Vec::with_capacity(n);
    for i in 0..n {
        let t = (i as f32) / (fs as f32);
        let s = (2.0 * std::f32::consts::PI * 4_000.0 * t).sin() * 0.8;
        hi_frames.push(Frame::new(s, s));
    }
    let frames_arc: Arc<[Frame]> = Arc::from(hi_frames);

    let flag_stereo = Arc::new(AtomicBool::new(false));

    // 1. Compare Unoccluded vs Wall-Occluded (`750 Hz` cutoff, `-8 dB` direct cut, `1.0` wet bleed)
    let init_clear = SpatialVoiceInitParams {
        base_distance_gain: 1.0,
        stereo_pan: 0.0,
        angles: HrtfSphericalAngles::default(),
        occlusion: OcclusionDspParams::unoccluded(),
    };
    let occ_wall = evaluate_occlusion_dsp(&sh2_core::AcousticOcclusionHit {
        wall_hit_count: 1,
        slab_hit_count: 0,
        closest_t: 0.5,
        primary_material: 0,
        barrier_density: 1.15,
    });
    let init_occluded = SpatialVoiceInitParams {
        occlusion: occ_wall,
        ..init_clear
    };

    let ctrl_clear = SpatialVoiceControl::new(&init_clear, Arc::clone(&flag_stereo));
    let ctrl_occ = SpatialVoiceControl::new(&init_occluded, Arc::clone(&flag_stereo));

    let mut core_clear = SpatialDryWetCore::new(Arc::clone(&frames_arc), fs, None, &init_clear);
    let mut core_occ = SpatialDryWetCore::new(Arc::clone(&frames_arc), fs, None, &init_occluded);

    let dt = 1.0 / (fs as f64);
    let mut dry_energy_clear = 0.0f64;
    let mut dry_energy_occ = 0.0f64;
    let mut wet_energy_clear = 0.0f64;
    let mut wet_energy_occ = 0.0f64;

    for i in 0..4_800 {
        let (dc, wc) = core_clear.process_pair(dt, &ctrl_clear);
        let (do_f, wo_f) = core_occ.process_pair(dt, &ctrl_occ);
        if i >= 480 {
            dry_energy_clear += (dc.left * dc.left + dc.right * dc.right) as f64;
            dry_energy_occ += (do_f.left * do_f.left + do_f.right * do_f.right) as f64;
            wet_energy_clear += (wc.left * wc.left + wc.right * wc.right) as f64;
            wet_energy_occ += (wo_f.left * wo_f.left + wo_f.right * wo_f.right) as f64;
        }
    }

    // 4 kHz dry energy behind a 798 Hz 2-pole Butterworth LPF + -7 dB direct cut must drop by > 98% (> 18 dB)!
    assert!(
        dry_energy_occ < dry_energy_clear * 0.02,
        "4 kHz dry signal must be heavily muffled by wall occlusion: clear={dry_energy_clear:.4}, occ={dry_energy_occ:.4}"
    );
    // Wet corridor reverb send must remain unmuffled (~100% preserved)!
    let wet_ratio = wet_energy_occ / wet_energy_clear.max(1e-9);
    assert!(
        (wet_ratio - 1.0).abs() < 0.05,
        "Wet corridor reverb bleed must be preserved across a corner wall: ratio={wet_ratio:.3}"
    );

    // 2. Verify smooth 100 ms (`4,800` samples @ 48 kHz) cutoff ramp with zero doorway clicks
    let mut filter = OcclusionFilterState::new(UNOCCLUDED_CUTOFF_HZ, 1.0);
    filter.set_target(750.0, db_to_linear(-8.0));

    let mut max_step_delta = 0.0f32;
    let mut prev_out = 0.0f32;
    let mut fc_at_50ms = 0.0f32;

    for i in 0..5_200 {
        let t = (i as f32) / (fs as f32);
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        let out = filter.process_frame(Frame::new(s, s), fs as f32);
        if i > 0 {
            max_step_delta = max_step_delta.max((out.left - prev_out).abs());
        }
        prev_out = out.left;
        if i == 2_400 {
            // Exactly 50 ms (halfway through 100 ms transition)
            fc_at_50ms = filter.current_cutoff_hz();
        }
    }

    assert!(
        fc_at_50ms > 1_800.0 && fc_at_50ms < 6_500.0,
        "At 50 ms midpoint, log-frequency cutoff ({fc_at_50ms:.1} Hz) must be smoothly mid-transition"
    );
    assert!(
        (filter.current_cutoff_hz() - 750.0).abs() < 1.0,
        "After 100 ms, cutoff must converge cleanly to 750 Hz"
    );
    // Maximum theoretical sample-to-sample step of a 0.5-amp 440 Hz sine at 48 kHz is `2*pi*440/48000 * 0.5 ≈ 0.0288`
    assert!(
        max_step_delta < 0.045,
        "100 ms doorway occlusion transition must produce zero audio clicks (max step={max_step_delta:.5})"
    );
}

#[test]
fn test_sh2_84_binaural_hrtf_itd_ild_and_vertical_elevation_cues() {
    let fs = 48_000.0f32;
    let james_feet = Vec3::ZERO;
    let head = listener_head_position(james_feet, false); // (0, -844.6, 0)

    // 1. Verify listener spherical angle & Woodworth ITD mapping in SH2 `+Y`-down world coordinates
    // Screen-right / right ear at yaw = 0 is +X (matching `SeCallPosDirection` `0x00192990` & `Mat4::look_at_rh`)
    let ang_right =
        compute_listener_spherical_angles(head, 0.0, 0.0, head + Vec3::new(2000.0, 0.0, 0.0));
    let ang_left =
        compute_listener_spherical_angles(head, 0.0, 0.0, head + Vec3::new(-2000.0, 0.0, 0.0));
    assert!(ang_right.itd_seconds > 0.000_60 && ang_right.itd_seconds < 0.000_70);
    assert!((ang_right.itd_seconds + ang_left.itd_seconds).abs() < 1e-6);

    // Above head in SH2 `+Y`-down means MORE NEGATIVE `Y` (`y = -2830` -> 2000 units = 4m overhead)
    let ang_above =
        compute_listener_spherical_angles(head, 0.0, 0.0, head + Vec3::new(0.0, -2000.0, 500.0));
    let ang_below =
        compute_listener_spherical_angles(head, 0.0, 0.0, head + Vec3::new(0.0, 2000.0, 500.0));
    assert!(
        ang_above.elevation_rad > 1.1,
        "Overhead enemy must yield positive elevation"
    );
    assert!(
        ang_below.elevation_rad < -1.1,
        "Sub-grate enemy must yield negative elevation"
    );

    // 2. Verify ITD impulse arrival lead & frequency-dependent head-shadow ILD for Right (`+90°`) emitter
    let mut hrtf_right = BinauralHrtfProcessor::new();
    hrtf_right.update_angles(ang_right.azimuth_rad, 0.0, fs, true);

    let mut first_peak_r = None;
    let mut first_peak_l = None;
    for i in 0..128 {
        let imp = if i == 0 { 1.0 } else { 0.0 };
        let out = hrtf_right.process_frame(Frame::new(imp, imp));
        if first_peak_r.is_none() && out.right.abs() > 0.15 {
            first_peak_r = Some(i);
        }
        if first_peak_l.is_none() && out.left.abs() > 0.05 {
            first_peak_l = Some(i);
        }
    }
    let lead_samples = first_peak_l.unwrap() as isize - first_peak_r.unwrap() as isize;
    assert!(
        (28..=35).contains(&lead_samples),
        "Woodworth ITD at 48 kHz (+90 deg) must delay contralateral left ear by ~31 samples (got {lead_samples})"
    );

    // Measure ILD at 5 kHz (strong head shadow) vs 250 Hz (low-frequency skull diffraction)
    let measure_ear_rms = |freq_hz: f32, az: f32, el: f32| -> (f64, f64) {
        let mut proc = BinauralHrtfProcessor::new();
        proc.update_angles(az, el, fs, true);
        let mut sum_l = 0.0f64;
        let mut sum_r = 0.0f64;
        for i in 0..4_800 {
            let s = (2.0 * std::f32::consts::PI * freq_hz * (i as f32) / fs).sin();
            let f = proc.process_frame(Frame::new(s, s));
            if i >= 800 {
                sum_l += (f.left * f.left) as f64;
                sum_r += (f.right * f.right) as f64;
            }
        }
        (sum_l.sqrt(), sum_r.sqrt())
    };

    let (l_250, r_250) = measure_ear_rms(250.0, ang_right.azimuth_rad, 0.0);
    let (l_5k, r_5k) = measure_ear_rms(5_000.0, ang_right.azimuth_rad, 0.0);
    let ild_250_ratio = r_250 / l_250.max(1e-9);
    let ild_5k_ratio = r_5k / l_5k.max(1e-9);
    assert!(
        ild_5k_ratio > 3.0 && ild_5k_ratio > ild_250_ratio * 1.8,
        "High-frequency 5 kHz head shadow ILD ({ild_5k_ratio:.2}x) must substantially exceed 250 Hz diffraction ILD ({ild_250_ratio:.2}x)"
    );

    // 3. Verify Pinna Elevation Cues: Above (`+el`) boosts 6.5 kHz Blauert overhead band relative to Below (`-el`)
    let (above_l_6k, _) = measure_ear_rms(6_500.0, 0.0, ang_above.elevation_rad);
    let (below_l_6k, _) = measure_ear_rms(6_500.0, 0.0, ang_below.elevation_rad);
    assert!(
        above_l_6k > below_l_6k * 1.85,
        "Overhead source must exhibit significantly brighter 6.5 kHz pinna elevation energy than below-floor source ({above_l_6k:.3} vs {below_l_6k:.3})"
    );

    // 4. Verify Front vs Behind Pinna Rim Shadowing at 6 kHz
    let (front_l_6k, _) = measure_ear_rms(6_000.0, 0.0, 0.0);
    let (behind_l_6k, _) = measure_ear_rms(6_000.0, std::f32::consts::PI, 0.0);
    assert!(
        front_l_6k > behind_l_6k * 1.6,
        "Rear source (180 deg) must exhibit pinna-rim high-frequency shadowing vs frontal source ({front_l_6k:.3} vs {behind_l_6k:.3})"
    );

    // 5. Verify live atomic switching between ClassicPs2Stereo / BinauralHrtf AND Wall Occlusion OFF / ON
    let hrtf_flag = Arc::new(AtomicBool::new(false));
    let occ_flag = Arc::new(AtomicBool::new(false)); // Purist default: OFF (Original PS2)
    let occ_wall = evaluate_occlusion_dsp(&sh2_core::AcousticOcclusionHit {
        wall_hit_count: 1,
        slab_hit_count: 0,
        closest_t: 0.5,
        primary_material: 0,
        barrier_density: 1.0,
    });
    let init_toggle = SpatialVoiceInitParams {
        base_distance_gain: 1.0,
        stereo_pan: 0.8,
        angles: ang_right,
        occlusion: occ_wall,
    };
    let ctrl = SpatialVoiceControl::new_with_flags(
        &init_toggle,
        Arc::clone(&hrtf_flag),
        Arc::clone(&occ_flag),
        false,
    );
    assert_eq!(
        ctrl.active_spatial_mode(),
        SpatialAudioMode::ClassicPs2Stereo
    );
    assert!(!ctrl.is_occlusion_enabled());
    hrtf_flag.store(true, Ordering::Relaxed);
    assert_eq!(ctrl.active_spatial_mode(), SpatialAudioMode::BinauralHrtf);

    // Verify `occ_flag == false` keeps `SpatialDryWetCore` at unoccluded 20 kHz, and toggling `true` ramps to muffled!
    let frames_4k: Arc<[Frame]> = Arc::from(
        (0..9_600)
            .map(|i| {
                let s = (2.0 * std::f32::consts::PI * 4_000.0 * (i as f32) / 48_000.0).sin() * 0.7;
                Frame::new(s, s)
            })
            .collect::<Vec<_>>(),
    );
    let mut toggle_core = SpatialDryWetCore::new_with_occlusion_enabled(
        frames_4k,
        48_000,
        None,
        &init_toggle,
        ctrl.is_occlusion_enabled(),
    );
    toggle_core.sync_from_control(&ctrl, 48_000.0);
    assert!((toggle_core.occlusion_filter.current_cutoff_hz() - UNOCCLUDED_CUTOFF_HZ).abs() < 1.0);
    occ_flag.store(true, Ordering::Relaxed);
    toggle_core.sync_from_control(&ctrl, 48_000.0);
    for _ in 0..5_000 {
        let _ = toggle_core.process_pair(1.0 / 48_000.0, &ctrl);
    }
    assert!(
        toggle_core.occlusion_filter.current_cutoff_hz() <= OCCLUDED_MAX_CUTOFF_HZ + 1.0,
        "Toggling Wall Occlusion ON live must ramp dry cutoff down to <= 850 Hz (got {})",
        toggle_core.occlusion_filter.current_cutoff_hz()
    );

    // 6. Verify 192 kHz Woodworth ITD delay (`~126` samples) does not saturate (`HRTF_DELAY_BUFFER_LEN == 256`)
    //    and non-finite (`NaN` / `Inf`) inputs never poison IIR filter integrators!
    let mut hrtf_192k = BinauralHrtfProcessor::new();
    hrtf_192k.update_angles(std::f32::consts::FRAC_PI_2, 0.0, 192_000.0, true);
    let mut peak_r_192 = None;
    let mut peak_l_192 = None;
    for i in 0..200 {
        let imp = if i == 0 { 1.0 } else { 0.0 };
        let out = hrtf_192k.process_frame(Frame::new(imp, imp));
        if peak_r_192.is_none() && out.right.abs() > 0.15 {
            peak_r_192 = Some(i);
        }
        if peak_l_192.is_none() && out.left.abs() > 0.05 {
            peak_l_192 = Some(i);
        }
    }
    let lead_192k = peak_l_192.unwrap() as isize - peak_r_192.unwrap() as isize;
    assert!(
        (122..=130).contains(&lead_192k),
        "192 kHz Woodworth ITD at +90 deg must delay left ear by ~126 samples without 128-sample truncation (got {lead_192k})"
    );

    // Non-finite input guard verification
    let nan_hit = sh2_core::collision::raycast_acoustic_occlusion(
        Vec3::splat(f32::NAN),
        head,
        std::iter::empty(),
        std::iter::empty(),
    );
    assert!(!nan_hit.is_occluded());
    let nan_ang = compute_listener_spherical_angles(head, f32::NAN, f32::INFINITY, head);
    assert!(nan_ang.azimuth_rad.is_finite() && nan_ang.itd_seconds.is_finite());
    hrtf_192k.update_angles(f32::NAN, f32::INFINITY, f32::NAN, false);
    let safe_out = hrtf_192k.process_frame(Frame::new(f32::NAN, f32::INFINITY));
    assert!(safe_out.left.is_finite() && safe_out.right.is_finite());
}
