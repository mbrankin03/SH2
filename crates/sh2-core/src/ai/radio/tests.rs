//! Deterministic golden-vector unit tests for `RadioNoise` (`0x001ab300`) and `enSetRadioVolume` (`0x00143890`).

use glam::{Mat4, Vec3};

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
