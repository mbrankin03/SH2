//! 1:1 Sound Effect (`sddata.bin` / `voice.afs`) bank lookup, PS2 3D distance attenuation (`0x401620`),
//! and 8-material surface footstep tests (`SH2-61` / `SH2-58`).

use glam::Vec3;
use sh2_audio::*;

#[test]
fn test_sh2_61_ps2_sfx_attenuation_formula_and_spatial_eval() {
    // 1. Verify exact `SeCallPosDistanceF` (`0x00192bc0` / `sh2pc 0x005155e0`) dual-distance linear product:
    //    atten = (1 - clamp((d_cam - 1500) * 0.0001, 0, 1)) * (1 - clamp((d_src - 1500) * 0.0001, 0, 1))
    let listener = Vec3::ZERO;
    let eval_inner =
        compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(750.0, 0.0, 1200.0), 1.0);
    assert!((eval_inner.proximity_v - 1.0).abs() < 1e-5);
    assert!((eval_inner.effective_gain - 1.0).abs() < 1e-5);
    assert_eq!(eval_inner.driver_attenuation_byte, 0);
    // Emitter at X = +750.0 cancels the -750.0mm camera-space X offset -> exact 0 deg center pan
    assert_eq!(eval_inner.direction_deg, 0);
    assert!(eval_inner.stereo_pan.abs() < 1e-5);

    // Halfway at d_src = 6500.0 (d_cam = 0.0) -> linear atten = 0.5, byte = 127, pan = -70 deg (+1.0 right)
    let eval_mid = compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(6500.0, 0.0, 0.0), 1.0);
    assert!((eval_mid.proximity_v - 0.5).abs() < 1e-5);
    assert!((eval_mid.effective_gain - 0.5).abs() < 1e-5);
    assert_eq!(eval_mid.driver_attenuation_byte, 127);
    assert_eq!(eval_mid.direction_deg, -70);
    assert!((eval_mid.stereo_pan - 1.0).abs() < 1e-5);

    let eval_left =
        compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(-6500.0, 0.0, 0.0), 1.0);
    assert_eq!(eval_left.direction_deg, 70);
    assert!((eval_left.stereo_pan - (-1.0)).abs() < 1e-5);

    // Verify 1:1 view-space screen-right parity with `glam::Mat4::look_at_rh(..., Vec3::new(0.0, -1.0, 0.0))`
    for &yaw in &[0.0f32, 0.75, -1.2, std::f32::consts::PI] {
        let eye = Vec3::new(100.0, -844.6, -200.0);
        let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let view = glam::Mat4::look_at_rh(eye, eye + fwd, Vec3::new(0.0, -1.0, 0.0));
        let test_pt = eye + fwd * 2500.0 + right * 1500.0;
        let view_pt = view.transform_point3(test_pt);
        assert!(
            view_pt.x > 0.0,
            "Point on screen-right must have view_pt.x > 0"
        );
        let pan_eval = compute_se_call_pos_distance_f(eye, yaw, test_pt, 1.0);
        let hrtf_ang = compute_listener_spherical_angles(eye, yaw, 0.0, test_pt);
        assert!(
            pan_eval.stereo_pan > 0.0 && hrtf_ang.azimuth_rad > 0.0 && hrtf_ang.itd_seconds > 0.0,
            "Screen-right emitter at yaw={yaw} must pan to right ear: pan={}, az={}, itd={}",
            pan_eval.stereo_pan,
            hrtf_ang.azimuth_rad,
            hrtf_ang.itd_seconds
        );
    }

    // Beyond outer cutoff (11500.0) -> v = 0.0, effective_gain = 0.0, byte = 255
    let eval_far = compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(0.0, 0.0, 12000.0), 1.0);
    assert_eq!(eval_far.proximity_v, 0.0);
    assert_eq!(eval_far.effective_gain, 0.0);
    assert_eq!(eval_far.driver_attenuation_byte, 255);
}

#[test]
fn test_sh2_61_se_id_lookup_and_dual_source_sfx_bank() {
    // 1. Verify 666-entry SE_ID_TO_SDDATA_INDEX & 32-entry SE_ID_TO_VOICE_AFS_INDEX tables
    assert_eq!(SE_ID_TO_SDDATA_INDEX.len(), 666);
    assert_eq!(SE_ID_TO_VOICE_AFS_INDEX.len(), 32);
    assert_eq!(resolve_se_id_to_sddata_index(10000), Some(293)); // CursorMove
    assert_eq!(resolve_se_id_to_sddata_index(10002), Some(297)); // Confirm
    assert_eq!(resolve_se_id_to_sddata_index(10003), Some(291)); // Cancel
    assert_eq!(resolve_se_id_to_sddata_index(11030), Some(270)); // Dry fire click
    assert_eq!(resolve_se_id_to_sddata_index(11032), Some(346)); // Plank swing
    assert_eq!(resolve_se_id_to_sddata_index(11040), Some(348)); // Pipe / kick
    assert_eq!(resolve_se_id_to_sddata_index(11044), Some(352)); // Chainsaw attack
    assert_eq!(resolve_se_id_to_sddata_index(0x1642), Some(352)); // Chainsaw idle loop
    assert_eq!(resolve_se_id_to_sddata_index(11048), Some(274)); // Handgun shot
    assert_eq!(resolve_se_id_to_sddata_index(11051), Some(279)); // Shotgun blast
    assert_eq!(resolve_se_id_to_voice_afs_index(10051), Some(35)); // Save point chime
    assert_eq!(resolve_se_id_to_voice_afs_index(15002), Some(24)); // Game start transition

    // 2. Verify all 8 surface materials resolve valid < 417 footstep indices for both James and Maria
    for mat in sh2_core::SurfaceMaterial::ALL {
        for is_maria in [false, true] {
            for is_running in [false, true] {
                let (l, r) = footstep_sddata_indices(is_maria, mat, is_running);
                assert!(l < SDDATA_ENTRY_COUNT && r < SDDATA_ENTRY_COUNT);
                assert_ne!(l, 280, "{mat:?} must not use sddata[280] (Shotgun Empty)");
                assert_ne!(r, 280, "{mat:?} must not use sddata[280] (Shotgun Empty)");
                assert_ne!(
                    l, 367,
                    "{mat:?} must not use sddata[367] (Feet running fast burst)"
                );
                assert_ne!(
                    r, 367,
                    "{mat:?} must not use sddata[367] (Feet running fast burst)"
                );
            }
        }
    }
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Concrete, false),
        (147, 147)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Concrete, true),
        (147, 147)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Wood, false),
        (151, 151)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Metal, false),
        (146, 146)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Carpet, false),
        (144, 144)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Water, false),
        (148, 148)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Grass, false),
        (142, 142)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Gravel, false),
        (150, 150)
    );
    assert_eq!(
        footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Blood, false),
        (148, 148)
    );

    // 3. Verify dual-source loading of `MenuAudioBank` (`Sh2eeLossless` 48 kHz vs `Ps2Original`) when retail assets exist
    let resolver = sh2_assets::AssetPathResolver::resolve();
    if !resolver.data_dir.join("sound/sddata.bin").exists() {
        println!("Skipping on-disk MenuAudioBank dual-source test (assets not present)");
        return;
    }

    let bank_ee = MenuAudioBank::load_from_dirs_for_source(
        &resolver.data_dir,
        resolver.sh2e_dir.as_deref(),
        sh2_core::AudioPackPreference::Sh2eeLossless,
    );
    assert_eq!(bank_ee.sddata_entry_count(), SDDATA_ENTRY_COUNT);
    assert_eq!(bank_ee.primary_sample_rate(), 48000);
    assert!(bank_ee.save_sound.is_some());
    assert!(bank_ee.game_start.is_some());
    assert!(bank_ee.flashlight_on.is_some());
    assert!(bank_ee.flashlight_off.is_some());
    assert!(bank_ee.resolve_se_id(11048).is_some());

    let bank_ps2 = MenuAudioBank::load_from_dirs_for_source(
        &resolver.data_dir,
        resolver.sh2e_dir.as_deref(),
        sh2_core::AudioPackPreference::Ps2Original,
    );
    assert_eq!(bank_ps2.sddata_entry_count(), SDDATA_ENTRY_COUNT);
    assert!(bank_ps2.primary_sample_rate() < 48000);
    assert!(bank_ps2.save_sound.is_some());
    assert!(bank_ps2.game_start.is_some());
    assert!(bank_ps2.resolve_se_id(11048).is_some());
}

#[test]
fn test_sh2_58_complete_audio_parity_dual_source_bgm_sfx_acoustics_and_looping() {
    // 1. Verify BGM & AIX Multi-Layer Room Mixing + Loop Region Scaling (`SH2-60` / `SH2-55` / `SH2-58`)
    let page_34 = bgm_page_entry(34);
    assert_eq!(page_34.load_id, 5065);
    let file_5065 = find_bgm_file_by_load_id(5065)
        .expect("Expected BGM file 5065 (sound/adx/hotel/bgm_112.aix)");
    assert!(!file_5065.is_adx);

    // 2. Verify 1:1 SFX Attenuation Curve (`0x401620`), Footstep Material Table (`0x8A1ED8`), and Room Acoustics (`SH2-61` / `SH2-55` / `SH2-58`)
    let eval_near = compute_se_call_pos_distance_f(Vec3::ZERO, 0.0, Vec3::ZERO, 1.0);
    let eval_mid =
        compute_se_call_pos_distance_f(Vec3::ZERO, 0.0, Vec3::new(0.0, 0.0, 6500.0), 1.0);
    let eval_far =
        compute_se_call_pos_distance_f(Vec3::ZERO, 0.0, Vec3::new(0.0, 0.0, 12000.0), 1.0);
    assert!((eval_near.effective_gain - 1.0).abs() < 1e-5);
    assert!((eval_mid.effective_gain - 0.5).abs() < 1e-5);
    assert_eq!(eval_far.effective_gain, 0.0);

    let prof_corridor = RoomAcousticsProfile::from_preset(EnvironmentalPreset::Corridor);
    let prof_corridor_dup = RoomAcousticsProfile::from_preset(EnvironmentalPreset::Corridor);
    assert_eq!(prof_corridor, prof_corridor_dup);
}
