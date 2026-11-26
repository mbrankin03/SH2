//! Golden-vector tests for `SH2-167`:
//! - PS2 `VbWvsMatrix` handedness (`SeCallPosDirection` `0x00192990` / `0x00515e60`)
//! - Dual-distance 3D attenuation passthrough & 2D handle updates (`SeCallPosDistanceF` `0x00192bc0`)
//! - Single `0.7` cutscene BGM ducking (`SeBgmManager` `0x00194300` / `0x00516430`)
//! - Pocket Radio 4-track driver mapping & retail `sddata[360..=363]` (`FUN_00518680` `0x00518680`)
//! - Chainsaw WAV slicing (`slice_chainsaw_wav` / `compute_chainsaw_wav_slice`)
//! - `Se2dManager` retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`) overrides, `±dt * 0.5` ramp,
//!   and `shSdSeChange == -1` auto-stop.

use glam::{Mat4, Vec3};
use sh2_assets::WavEntry;
use sh2_audio::{
    radio_driver_to_volume_pan, se_call_pos_direction, slice_chainsaw_wav, BgmDuckingMode,
    BgmManager, MenuAudioBank, Se2d3dMapping, SeBackendAction, SeCallResult, SeListenerContext,
    SePoolManager, RADIO_STATIC_SDDATA_INDICES, SE_2D_0X3CF0_FIXED_Y, SE_2D_0X3CF0_FIXED_Z,
    SE_2D_0X44C6_FLAGS, SE_2D_0X44C6_POSITION, SE_2D_OVERRIDE_ID_0X3CF0, SE_2D_OVERRIDE_ID_0X44C6,
    SE_2D_OVERRIDE_ROOM_0XA7, SE_2D_SLOT_COUNT, SE_CHAINSAW_IDLE_LOOP, SE_FLAG_MANAGED_2D,
    SE_FLAG_PRIO_1,
};
use sh2_core::camera::ps2_wvs_matrix_from_look_at;
use sh2_core::scenario::ScenarioKind;
use sh2_core::SimulationCore;

#[test]
fn test_sh2_167_ps2_wvs_matrix_forward_sound_pans_to_zero() {
    let eye = Vec3::new(0.0, -1600.0, 0.0);
    let target = Vec3::new(0.0, -1600.0, 2000.0);
    let up = -Vec3::Y;

    // Right-handed look_at_rh (-Z forward) mistakenly flips forward sounds behind the listener (~±140°).
    let rh_view = Mat4::look_at_rh(eye, target, up);
    let sound_ahead = Vec3::new(0.0, -1600.0, 5000.0);
    let rh_dir = se_call_pos_direction(rh_view, sound_ahead);
    assert!(
        rh_dir.abs() >= 135,
        "look_at_rh flips forward sounds behind listener; got {rh_dir}"
    );

    // PS2 VbWvsMatrix (+Z forward, +Y down, +X right) pans forward sounds to exact 0°.
    let ps2_wvs = ps2_wvs_matrix_from_look_at(eye, target, up);
    assert_eq!(se_call_pos_direction(ps2_wvs, sound_ahead), 0);
    assert_eq!(
        se_call_pos_direction(ps2_wvs, Vec3::new(750.0, -1600.0, 5000.0)),
        0
    );
    // Right (+X) -> -70°, Left (-X) -> +70°
    assert_eq!(
        se_call_pos_direction(ps2_wvs, Vec3::new(5750.0, -1600.0, 0.0)),
        -70
    );
    assert_eq!(
        se_call_pos_direction(ps2_wvs, Vec3::new(-4250.0, -1600.0, 0.0)),
        70
    );

    // Also verify SimulationCore::active_ps2_wvs_matrix produces +Z forward along the active camera ray.
    let sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let snap = sim.active_camera_snapshot(1.0);
    let wvs = sim.active_ps2_wvs_matrix(1.0);
    let forward = (snap.target - snap.eye).normalize_or(Vec3::Z);
    let ahead_world = snap.eye + forward * 5000.0;
    let ahead_view = wvs * ahead_world.extend(1.0);
    assert!(
        ahead_view.z > 4900.0,
        "active_ps2_wvs_matrix must place forward targets on +Z, got z={}",
        ahead_view.z
    );
}

#[test]
fn test_sh2_167_dual_distance_3d_passthrough_and_2d_direction() {
    let change_list = vec![Se2d3dMapping {
        id_2d: 12_009,
        id_3d: 40_221,
        bank_id: 1,
    }];
    let mut pool = SePoolManager::new(change_list);
    pool.active_3d_bank = 1;

    // Camera 6500 mm from player (f_cam = 0.5), source 6500 mm from player (f_src = 0.5)
    // Dual-distance product = 0.5 * 0.5 = 0.25 => attenuation_byte = (255 - 0.25*255) = 191.
    let camera_pos = Vec3::new(0.0, 0.0, -6500.0);
    let player_pos = Vec3::ZERO;
    let wvs = ps2_wvs_matrix_from_look_at(camera_pos, player_pos, -Vec3::Y);
    let listener = SeListenerContext {
        camera_pos,
        player_pos,
        wvs_matrix: wvs,
        room_id: 10,
    };

    let mut actions = Vec::new();
    let res = pool.call_pos(
        1.0,
        12_009,
        Some(Vec3::new(0.0, 0.0, 6500.0)),
        SE_FLAG_PRIO_1,
        &listener,
        &mut actions,
    );
    assert_eq!(
        res,
        SeCallResult::Played3d {
            channel: 0,
            se_id: 40_221
        }
    );
    assert_eq!(actions.len(), 1);
    match actions[0] {
        SeBackendAction::Play3d {
            channel,
            se_id,
            attenuation_byte,
            ..
        } => {
            assert_eq!(channel, 0);
            assert_eq!(se_id, 40_221);
            assert_eq!(
                attenuation_byte, 191,
                "must preserve dual-distance attenuation (191), not d_cam=0 (127)"
            );
        }
        other => panic!("expected Play3d, got {other:?}"),
    }

    // 2D managed sound emits non-zero direction_deg on Play2d and subsequent Update2d.
    actions.clear();
    let res2d = pool.call_pos(
        1.0,
        12_100,
        Some(Vec3::new(5750.0, 0.0, -6500.0)),
        SE_FLAG_MANAGED_2D,
        &listener,
        &mut actions,
    );
    assert_eq!(res2d, SeCallResult::Played2d { se_id: 12_100 });
    assert!(matches!(
        actions[0],
        SeBackendAction::Play2d {
            se_id: 12_100,
            direction_deg: -70,
            ..
        }
    ));
}

#[test]
fn test_sh2_167_single_0_7_cutscene_bgm_ducking() {
    let mut bgm = BgmManager::new("", sh2_core::AudioPackPreference::Auto);
    bgm.set_ducking_mode(BgmDuckingMode::DramaDemo);
    // Step 2.0 seconds: ±dt * 0.5 ramp settles cleanly at exact 0.7 (never double-ducked to 0.245).
    for _ in 0..120 {
        let _ = bgm.tick(5, Vec3::ZERO, 1.0, None, 1.0 / 60.0, None);
    }
    assert!(
        (bgm.global_bgm_fade - 0.7).abs() < 1e-4,
        "DramaDemo BGM ducking must settle at 0.7, got {}",
        bgm.global_bgm_fade
    );
}

#[test]
fn test_sh2_167_pocket_radio_driver_mapping_and_retail_sddata() {
    assert_eq!(RADIO_STATIC_SDDATA_INDICES, [360, 361, 362, 363]);
    let (gain_max, pan_center) = radio_driver_to_volume_pan(127, 64);
    assert!((gain_max - 1.0).abs() < 1e-6);
    assert!((pan_center - (64.0 / 127.0)).abs() < 1e-6);
    let (gain_zero, pan_left) = radio_driver_to_volume_pan(0, 0);
    assert_eq!(gain_zero, 0.0);
    assert_eq!(pan_left, 0.0);

    // If retail assets are present on disk, verify all 4 radio static tracks load from sddata[360..=363].
    let data_root =
        std::path::Path::new("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data");
    if data_root.join("sound/sddata.bin").exists() {
        let bank = MenuAudioBank::load_from_dirs(data_root, None);
        let tracks = bank
            .radio_static_wavs()
            .expect("retail sddata.bin must contain radio static entries 360..=363");
        for (idx, wav) in tracks.iter().enumerate() {
            assert!(
                !wav.data.is_empty(),
                "radio static track {idx} (sddata[{}]) must have non-empty PCM data",
                RADIO_STATIC_SDDATA_INDICES[idx]
            );
        }
    }
}

#[test]
fn test_sh2_167_chainsaw_wav_slicing() {
    let wav_48k = WavEntry {
        id: 352,
        sample_rate: 48_000,
        channels: 1,
        bits_per_sample: 16,
        loops: true,
        loop_start: 40_000,
        loop_end: 80_000,
        data: vec![0u8; 0x30000],
    };
    let fixed = slice_chainsaw_wav(&wav_48k, SE_CHAINSAW_IDLE_LOOP, false, true);
    assert_eq!(fixed.data.len(), 0x30000 - 0x12180);
    assert_eq!(fixed.loop_start, 40_000 - (0x12180 / 2) as u32);

    let vanilla = slice_chainsaw_wav(&wav_48k, SE_CHAINSAW_IDLE_LOOP, false, false);
    assert_eq!(vanilla.data.len(), 0x30000);
    assert_eq!(vanilla.loop_start, 40_000);
}

#[test]
fn test_sh2_167_se2d_manager_retail_fun_00516b90_parity() {
    let mut pool = SePoolManager::default();
    let listener_a7 = SeListenerContext {
        camera_pos: Vec3::ZERO,
        player_pos: Vec3::new(100.0, 0.0, -5000.0),
        wvs_matrix: Mat4::IDENTITY,
        room_id: SE_2D_OVERRIDE_ROOM_0XA7,
    };
    let mut actions = Vec::new();

    // 1. Room 0xa7 + SE 0x44c6 override + ±dt * 0.5 ramp
    let res = pool.call_pos(
        0.5,
        SE_2D_OVERRIDE_ID_0X44C6,
        Some(Vec3::ZERO),
        SE_FLAG_MANAGED_2D,
        &listener_a7,
        &mut actions,
    );
    assert_eq!(
        res,
        SeCallResult::Played2d {
            se_id: SE_2D_OVERRIDE_ID_0X44C6
        }
    );
    // Set last_attenuation_byte = 40 so the `> 8` decrement (`-= 8` -> 32) is observable
    pool.slots_2d[0].as_mut().unwrap().last_attenuation_byte = 40;
    actions.clear();
    pool.tick_2d(&listener_a7, BgmDuckingMode::Normal, 0.5, &mut actions);
    let slot0 = pool.slots_2d[0].unwrap();
    assert_eq!(slot0.position, Some(SE_2D_0X44C6_POSITION));
    assert!((slot0.base_volume - 1.0).abs() < 1e-6);
    assert_eq!(slot0.flags, SE_2D_0X44C6_FLAGS);
    // ±dt * 0.5 ramp: 0.0 + 0.5 * 0.5 = 0.25 (NOT 0.5!)
    assert!(
        (slot0.duck_fade - 0.25).abs() < 1e-6,
        "retail FUN_00516b90 ramps duck_fade at ±dt*0.5; got {}",
        slot0.duck_fade
    );

    // 2. SE 0x3cf0 override: player_pos.z >= -10000 -> (player.x, 2400, -10000); z < -10000 -> None
    actions.clear();
    pool.call_pos(
        1.0,
        SE_2D_OVERRIDE_ID_0X3CF0,
        None,
        SE_FLAG_MANAGED_2D,
        &listener_a7,
        &mut actions,
    );
    actions.clear();
    pool.tick_2d(&listener_a7, BgmDuckingMode::Normal, 0.016, &mut actions);
    assert_eq!(
        pool.slots_2d[1].unwrap().position,
        Some(Vec3::new(100.0, SE_2D_0X3CF0_FIXED_Y, SE_2D_0X3CF0_FIXED_Z))
    );

    let listener_below = SeListenerContext {
        player_pos: Vec3::new(100.0, 0.0, -12_000.0),
        ..listener_a7
    };
    actions.clear();
    pool.tick_2d(&listener_below, BgmDuckingMode::Normal, 0.016, &mut actions);
    assert_eq!(pool.slots_2d[1].unwrap().position, None);

    // 3. `shSdSeChange == -1` auto-stop (`FUN_00516b90` lines 5114–5116)
    actions.clear();
    let mut finished = [false; SE_2D_SLOT_COUNT];
    finished[1] = true;
    pool.tick_2d_with_finished(
        &listener_below,
        BgmDuckingMode::Normal,
        0.016,
        &finished,
        &mut actions,
    );
    assert!(pool.slots_2d[1].is_none(), "finished 2D slot must be freed");
    assert!(actions.contains(&SeBackendAction::Stop2d {
        se_id: SE_2D_OVERRIDE_ID_0X3CF0
    }));
}
