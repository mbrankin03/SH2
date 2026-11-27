//! Deterministic golden-vector unit tests for `RadioNoise` (`0x001ab300`) and `enSetRadioVolume` (`0x00143890`).

use glam::{Mat4, Vec3};

use crate::ai::creatures::EnemySpecies;

use super::*;

#[test]
fn test_en_set_radio_volume_species_and_state_weights() {
    // Pyramid Head (kind 4): sub_type 0 => 1.0, non-zero sub_type => 0.0
    assert_eq!(en_set_radio_volume(4, 0, false, false, false), 1.0);
    assert_eq!(en_set_radio_volume(4, 1, true, true, false), 0.0);
    // Silent bosses/enemies: Twin PH (5), Flesh Lips (7), Eddie (9), Mary (11)
    for kind in [5u8, 7, 9, 11] {
        assert_eq!(en_set_radio_volume(kind, 0, true, true, false), 0.0);
    }
    // Inactive or dead => 0.0
    assert_eq!(en_set_radio_volume(1, 0, false, true, false), 0.0);
    assert_eq!(en_set_radio_volume(1, 0, true, false, false), 0.0);
    // Creeper (kind 3) => 0.5
    assert_eq!(en_set_radio_volume(3, 0, true, true, false), 0.5);
    // Standing normal enemy => 1.0, downed => 0.8
    assert_eq!(en_set_radio_volume(1, 0, true, true, false), 1.0);
    assert_eq!(en_set_radio_volume(1, 0, true, true, true), 0.8);
}

#[test]
fn test_radio_cosine_curve_and_3_enemy_4_track_mixing() {
    // Cosine boundaries: 0mm -> 1.0, 6250mm -> cos(pi/4) = sqrt(2)/2, 12500mm -> 0.0, >12500mm -> 0.0
    assert!((radio_cosine_attenuation(0.0) - 1.0).abs() < 1e-6);
    assert!((radio_cosine_attenuation(6250.0) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    assert!(radio_cosine_attenuation(12_500.0).abs() < 1e-5);
    assert_eq!(radio_cosine_attenuation(12_501.0), 0.0);

    let mut radio = RadioNoiseEvaluator::default();
    let enemies = [
        RadioEnemySample {
            kind: 1,
            sub_type: 0,
            position: Vec3::ZERO,
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: 2,
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 6250.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: 4,
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 100.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
    ];

    let out = radio.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &enemies, 3.0);
    let cos_100 = radio_cosine_attenuation(100.0);
    let cos_6250 = radio_cosine_attenuation(6250.0);
    let exp_t0 = (1.0 + cos_100 * 0.4 / 3.0).min(1.0);
    let exp_t1 = (cos_100 * 0.4 / 3.0 + cos_6250 * 1.0 / 6.0).min(1.0);
    let exp_t2 = (1.0 + cos_100 * 0.4 / 3.0 + cos_6250 * 1.0 / 6.0).min(1.0);
    let exp_t3 = (cos_100 * 1.0 / 3.0).min(1.0);
    assert!((out.track_volumes[0] - exp_t0).abs() < 1e-4);
    assert!((out.track_volumes[1] - exp_t1).abs() < 1e-4);
    assert!((out.track_volumes[2] - exp_t2).abs() < 1e-4);
    assert!((out.track_volumes[3] - exp_t3).abs() < 1e-4);
}

#[test]
fn test_radio_modes_1_and_2_and_ducking() {
    let mut radio = RadioNoiseEvaluator {
        mode: RadioMode::WorldSource {
            position: [0.0, 0.0, 1500.0],
        },
        ..Default::default()
    };
    let out1 = radio.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &[], 0.016);
    assert!((out1.track_volumes[0] - 0.8).abs() < 0.02);
    assert_eq!(out1.track_volumes[1], 0.0);
    assert!((out1.track_volumes[2] - 0.8).abs() < 0.02);

    let mut radio2 = RadioNoiseEvaluator {
        mode: RadioMode::ForcedFirstEnemy,
        ..Default::default()
    };
    let downed = [RadioEnemySample {
        kind: 1,
        sub_type: 0,
        position: Vec3::new(0.0, 0.0, 50_000.0),
        is_spawned_active: true,
        is_alive: true,
        is_downed: true,
    }];
    let out2_half =
        radio2.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &downed, 1.0);
    assert!((out2_half.track_volumes[0] - 0.4).abs() < 1e-5);
    assert_eq!(out2_half.driver_pans, [64; 4]);
    let out2_full =
        radio2.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &downed, 2.0);
    assert!((out2_full.track_volumes[0] - 0.8).abs() < 1e-5);

    let mut radio_duck = RadioNoiseEvaluator {
        ducking_state: RadioDuckingState::MapOrMemo,
        ..Default::default()
    };
    let out_duck = radio_duck.evaluate(true, Vec3::ZERO, &[Vec3::ZERO], 3.0);
    assert!((out_duck.global_duck - 0.25).abs() < 1e-5);
    assert!((out_duck.volume - 0.25).abs() < 1e-5);
}

#[test]
fn test_room_insect_maps_to_kind_zero_and_is_ignored_by_radio_noise() {
    // Ground truth: enTransID (SLUS_202.28 0x00141b30) & FUN_004a5ba0 (sh2pc.exe 0x004a5ba0)
    // map only 0x200..=0x20d and 0x421 (kinds 1..=14); RoomInsect (0x20f) returns 0.
    let insect_kind = en_kind_from_species(EnemySpecies::RoomInsect);
    assert_eq!(insect_kind, 0);

    // Mode 0 (Normal): 3 very close RoomInsects must NOT occupy any of the 3 nearest slots
    // or evict real enemies that are further away.
    let real_enemies = [
        RadioEnemySample {
            kind: en_kind_from_species(EnemySpecies::LyingFigure),
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 1000.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: en_kind_from_species(EnemySpecies::Mannequin),
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 2000.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: en_kind_from_species(EnemySpecies::Nurse),
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 3000.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
    ];
    let mut with_insects = vec![
        RadioEnemySample {
            kind: insect_kind,
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 100.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: insect_kind,
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 200.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: insect_kind,
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 300.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
    ];
    with_insects.extend_from_slice(&real_enemies);

    let mut radio_only_real = RadioNoiseEvaluator::default();
    let out_real = radio_only_real.evaluate_enemies(
        true,
        Vec3::ZERO,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &real_enemies,
        3.0,
    );
    let mut radio_with_insects = RadioNoiseEvaluator::default();
    let out_with_insects = radio_with_insects.evaluate_enemies(
        true,
        Vec3::ZERO,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &with_insects,
        3.0,
    );
    assert_eq!(out_with_insects.nearest_enemy_dist, 1000.0);
    assert_eq!(out_with_insects.track_volumes, out_real.track_volumes);
    assert_eq!(out_with_insects.driver_volumes, out_real.driver_volumes);

    // Mode 2 (ForcedFirstEnemy): RoomInsect must NOT be matched even when it is first in the slice.
    // A standing RoomInsect (would yield 1.0 if matched) before a downed Lying Figure (0.8) must
    // be skipped so Mode 2 targets 0.8.
    let mut radio_forced = RadioNoiseEvaluator {
        mode: RadioMode::ForcedFirstEnemy,
        ..Default::default()
    };
    let forced_mix = [
        RadioEnemySample {
            kind: insect_kind,
            sub_type: 0,
            position: Vec3::ZERO,
            is_spawned_active: true,
            is_alive: true,
            is_downed: false,
        },
        RadioEnemySample {
            kind: en_kind_from_species(EnemySpecies::LyingFigure),
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 50_000.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: true,
        },
    ];
    let out_forced = radio_forced.evaluate_enemies(
        true,
        Vec3::ZERO,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &forced_mix,
        3.0,
    );
    assert!((out_forced.track_volumes[0] - 0.8).abs() < 1e-5);
    assert!((out_forced.track_volumes[2] - 0.8).abs() < 1e-5);

    // Mode 2 (ForcedFirstEnemy) with ONLY a downed RoomInsect (would yield 0.8 if matched)
    // must fall back to the default 1.0 when no mapped enemy (kind != 0) exists.
    let mut radio_forced_only_insect = RadioNoiseEvaluator {
        mode: RadioMode::ForcedFirstEnemy,
        ..Default::default()
    };
    let only_downed_insect = [RadioEnemySample {
        kind: insect_kind,
        sub_type: 0,
        position: Vec3::ZERO,
        is_spawned_active: true,
        is_alive: true,
        is_downed: true,
    }];
    let out_forced_fallback = radio_forced_only_insect.evaluate_enemies(
        true,
        Vec3::ZERO,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &only_downed_insect,
        3.0,
    );
    assert!((out_forced_fallback.track_volumes[0] - 1.0).abs() < 1e-5);
    assert!((out_forced_fallback.track_volumes[2] - 1.0).abs() < 1e-5);
}

#[test]
fn test_sh2pc_retail_radio_tables_threat_gating_and_geometric_decay() {
    // Verify exact `sh2pc.exe` `FUN_0048c710` (`0x0088a0f8` / `0x0088a108`) tables:
    assert_eq!(SH2PC_RADIO_TRACK_WEIGHTS[1], [1.0, 1.0, 0.6, 0.8]);
    assert_eq!(
        SH2PC_RADIO_TRACK_RADII_MM[1],
        [12_500.0, 5_000.0, 1_250.0, 7_500.0]
    );
    assert_eq!(SH2PC_RADIO_TRACK_WEIGHTS[4], [0.5, 0.5, 0.5, 1.0]);
    assert_eq!(
        SH2PC_RADIO_TRACK_RADII_MM[4],
        [10_000.0, 9_000.0, 8_000.0, 10_000.0]
    );
    // Inner deadzone `< 1.5` mm returns exact `1.0`.
    assert_eq!(sh2pc_radio_cosine_attenuation(1.0, 12_500.0), 1.0);
    assert_eq!(sh2pc_radio_cosine_attenuation(13_000.0, 12_500.0), 0.0);

    // Single Lying Figure at 1000 mm has threat score 3 (< 9) -> track 3 stays gated at 0.0.
    let mut radio = RadioNoiseEvaluator::default();
    let one_scu = [RadioEnemySample {
        kind: 1,
        sub_type: 0,
        position: Vec3::new(0.0, 0.0, 1000.0),
        is_spawned_active: true,
        is_alive: true,
        is_downed: false,
    }];
    let out_one = radio.evaluate_enemies_retail_pc(
        true,
        Vec3::ZERO,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &one_scu,
        5.0,
    );
    assert!(out_one.track_volumes[0] > 0.9);
    assert_eq!(out_one.track_volumes[3], 0.0);

    // Three Lying Figures within 7500 mm sum to threat score 9 (>= 9) -> track 3 opens!
    let three_scu = [one_scu[0]; 3];
    let out_three = radio.evaluate_enemies_retail_pc(
        true,
        Vec3::ZERO,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &three_scu,
        5.0,
    );
    assert!(out_three.track_volumes[3] > 0.5);
}
