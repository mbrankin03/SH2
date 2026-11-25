//! Deterministic golden-vector tests for the `Effect2` Impact Queue Blood System (`SH2-79`).

use glam::Vec3;

use super::*;
use crate::config::BloodColor;

#[test]
fn subtractive_blood_colors_match_sh2pc_tables() {
    assert_eq!(
        BLOOD_COLOR_SUBTRACTIVE_RGB_GS[BloodColor::Normal as usize],
        [40.0, 255.0, 230.0]
    );
    assert_eq!(
        BLOOD_COLOR_SUBTRACTIVE_RGB_GS[BloodColor::Green as usize],
        [180.0, 4.0, 150.0]
    );
    assert_eq!(
        BLOOD_COLOR_SUBTRACTIVE_RGB_GS[BloodColor::Violet as usize],
        [24.0, 230.0, 23.0]
    );
    assert_eq!(
        BLOOD_COLOR_SUBTRACTIVE_RGB_GS[BloodColor::Black as usize],
        [194.0, 210.0, 180.0]
    );

    let normal = blood_color_subtractive_rgb_norm(BloodColor::Normal);
    assert!((normal[0] - 40.0 / 255.0).abs() < 1e-6);
    assert!((normal[1] - 1.0).abs() < 1e-6);
    assert!((normal[2] - 230.0 / 255.0).abs() < 1e-6);
}

#[test]
fn three_stage_death_pool_envelope_matches_slus_motion_info() {
    // Stage 0..2s: scale 0 -> 0.3, alpha 0 -> 0
    let (s0, a0) = evaluate_three_stage_envelope(2.0, &BLOOD_02_MOTION_INFO);
    assert!((s0 - 0.3).abs() < 1e-5);
    assert!(a0.abs() < 1e-5);

    // Stage 2..8s: scale 0.3 -> 0.8, alpha 0 -> 0.35
    let (s1, a1) = evaluate_three_stage_envelope(8.0, &BLOOD_02_MOTION_INFO);
    assert!((s1 - 0.8).abs() < 1e-5);
    assert!((a1 - 0.35).abs() < 1e-5);

    // Stage 8..16s: scale 0.8 -> 1.0, alpha 0.35 -> 1.0
    let (s2, a2) = evaluate_three_stage_envelope(16.0, &BLOOD_02_MOTION_INFO);
    assert!((s2 - 1.0).abs() < 1e-5);
    assert!((a2 - 1.0).abs() < 1e-5);
}

#[test]
fn falling_blood_drop_spawns_floor_stain_on_floor_strike() {
    let mut sys = BloodEffectSystem::new();
    let cfg = BloodSystemConfig::default();

    sys.post_impact(ImpactDescriptor {
        kind: ImpactKind::BloodParticle,
        position_mm: Vec3::new(0.0, -400.0, 0.0),
        velocity_mm_s: Vec3::new(100.0, -200.0, 0.0),
        yaw_rad: 0.0,
        mode: 1, // falling drop
    });
    assert_eq!(sys.active_counts().0, 1);

    // Step at 60 Hz until the drop strikes floor y = 0.0
    for _ in 0..60 {
        sys.step(
            BLOOD_DT_60HZ,
            |_| 0.0,
            Vec3::new(5000.0, 0.0, 5000.0),
            0.0,
            false,
            cfg,
        );
    }

    let (particles, stains, _, _) = sys.active_counts();
    assert_eq!(particles, 0);
    assert_eq!(stains, 1);
}

#[test]
fn death_pool_spawns_3_pools_and_charges_10_bloody_footprints() {
    let mut sys = BloodEffectSystem::new();
    let cfg = BloodSystemConfig::default();

    sys.post_enemy_death_pool(Vec3::ZERO);

    // Advance 6 seconds at 30 Hz so all 3 pools from _Time_Table spawn and expand
    for _ in 0..180 {
        sys.step(
            BLOOD_DT_30HZ,
            |_| 0.0,
            Vec3::new(5000.0, 0.0, 5000.0),
            0.0,
            false,
            cfg,
        );
    }
    assert_eq!(sys.active_counts().2, 3);

    // Step James into the pool center to charge BloodStick00
    sys.step(BLOOD_DT_30HZ, |_| 0.0, Vec3::ZERO, 0.0, false, cfg);
    assert_eq!(sys.bloody_steps_remaining(), MAX_BLOODY_STEPS);

    // Walk away and trigger 12 heel strikes -> should stamp exactly 10 footprints
    for i in 0..12 {
        sys.step(
            BLOOD_DT_30HZ,
            |_| 0.0,
            Vec3::new(2000.0 + i as f32 * 300.0, 0.0, 0.0),
            0.0,
            true,
            cfg,
        );
    }
    assert_eq!(sys.bloody_steps_remaining(), 0);
    assert_eq!(sys.active_counts().3, usize::from(MAX_BLOODY_STEPS));
}

#[test]
fn room_transition_respects_vanilla_blood_clear_vs_persistent_decals() {
    let mut sys = BloodEffectSystem::new();
    let persistent_cfg = BloodSystemConfig {
        persistent_room_decals: true,
        ..Default::default()
    };

    sys.post_impact(ImpactDescriptor {
        kind: ImpactKind::FloorStain,
        position_mm: Vec3::new(100.0, 0.0, 100.0),
        velocity_mm_s: Vec3::ZERO,
        yaw_rad: 0.0,
        mode: 0,
    });
    assert_eq!(sys.active_counts().1, 1);

    // Persistent mode preserves & restores room decals on revisit
    sys.on_room_change("er01", "er02", true);
    assert_eq!(sys.active_counts().1, 0);
    sys.on_room_change("er02", "er01", true);
    assert_eq!(sys.active_counts().1, 1);

    // Vanilla PS2 mode calls blood_clear() on room exit
    sys.on_room_change("er01", "er02", false);
    assert_eq!(sys.active_counts().1, 0);
    sys.on_room_change("er02", "er01", false);
    assert_eq!(sys.active_counts().1, 0);
    let _ = persistent_cfg;
}
