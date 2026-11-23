//! Deterministic 1:1 Golden-Vector Parity Tests for `SH2-137`:
//! - `SeCallPosDistanceF` (`0x00192bc0`) / `SeCallPosDistance` (`0x00192a70` / `sh2pc 0x005155e0`)
//! - `SeCallPosDirection` (`0x00192990` / `sh2pc 0x00515e60`)
//! - `SeBgmCall` (`0x00194220`) fade-out override & `SeBgmManager` (`0x00194300`) ducking ramps
//! - `Se2dManager` (`0x001932f0`) & `Se3dManager` (`0x00193790`) channel pools
//! - Room acoustics `native-upgrade` Options toggle (`acoustics.rs`)

use glam::{Mat4, Vec3};
use sh2_assets::{
    AreaSizeType, CamMovementParams, CamMovementType, RoadArea, RoadRecord, RoadType,
};
use sh2_audio::*;
use sh2_core::AudioPackSource;

#[test]
fn test_se_call_pos_distance_f_dual_distance_product_knees() {
    let player = Vec3::ZERO;
    // Inside both 1500mm knees -> 1.0 * 1.0 = 1.0
    assert!(
        (se_call_pos_distance_f(
            Vec3::new(0.0, 0.0, 1500.0),
            player,
            Vec3::new(1500.0, 0.0, 0.0),
        ) - 1.0)
            .abs()
            < 1e-6
    );
    // Camera at 6500mm (0.5) & Source at 6500mm (0.5) -> 0.5 * 0.5 = 0.25
    let atten = se_call_pos_distance_f(
        Vec3::new(0.0, 0.0, 6500.0),
        player,
        Vec3::new(6500.0, 0.0, 0.0),
    );
    assert!((atten - 0.25).abs() < 1e-5, "Expected 0.25, got {atten}");
    assert_eq!(
        se_call_pos_distance(
            1.0,
            Vec3::new(0.0, 0.0, 6500.0),
            player,
            Vec3::new(6500.0, 0.0, 0.0),
        ),
        (255.0 - 0.25 * 255.0) as u8 // 191
    );
    // Camera beyond 11500mm -> 0.0 even if source is at player
    assert_eq!(
        se_call_pos_distance_f(Vec3::new(0.0, 0.0, 11_500.0), player, player),
        0.0
    );
    assert_eq!(
        se_call_pos_distance(1.0, Vec3::new(0.0, 0.0, 11_500.0), player, player),
        255
    );
}

#[test]
fn test_se_call_pos_direction_wvs_and_750mm_offset() {
    let wvs = Mat4::IDENTITY;
    // Exact center after -750.0mm camera-space X offset -> 0 deg
    assert_eq!(
        se_call_pos_direction(wvs, Vec3::new(750.0, 500.0, 2000.0)),
        0
    );
    // Pure +X (right of offset) with z=0 -> -70 deg
    assert_eq!(se_call_pos_direction(wvs, Vec3::new(2750.0, 0.0, 0.0)), -70);
    // Pure -X (left of offset) with z=0 -> +70 deg
    assert_eq!(se_call_pos_direction(wvs, Vec3::new(-1250.0, 0.0, 0.0)), 70);
    // Directly behind (z < 0, slightly right of offset) -> -140 deg
    let behind_right = se_call_pos_direction(wvs, Vec3::new(751.0, 0.0, -10_000.0));
    assert!(behind_right <= -139, "Expected ~-140, got {behind_right}");
    // Directly behind (z < 0, slightly left of offset) -> +140 deg
    let behind_left = se_call_pos_direction(wvs, Vec3::new(749.0, 0.0, -10_000.0));
    assert!(behind_left >= 139, "Expected ~+140, got {behind_left}");
}

#[test]
fn test_se_bgm_call_fade_override_and_ducking_ramps() {
    let mut mgr = BgmManager::new("/nonexistent", AudioPackSource::Ps2Original);
    mgr.current_play_id = 50_001;
    // Track 0: FadingOut with slow 6.0s (3) -> MUST accelerate to 3.0s (1)
    mgr.tracks[0] = BgmTrackState {
        status: BgmTrackStatus::FadingOut,
        fade_in: 1,
        fade_out: 3,
        volume: 0.8,
    };
    // Track 1: FadingOut with fast 1.5s (2) -> MUST NOT be overwritten
    mgr.tracks[1] = BgmTrackState {
        status: BgmTrackStatus::FadingOut,
        fade_in: 1,
        fade_out: 2,
        volume: 0.8,
    };
    // Track 2: FadingOut with instant (4) -> MUST NOT be overwritten
    mgr.tracks[2] = BgmTrackState {
        status: BgmTrackStatus::FadingOut,
        fade_in: 1,
        fade_out: 4,
        volume: 0.8,
    };
    // Track 3: On -> MUST transition to FadingOut with 3.0s (1)
    mgr.tracks[3] = BgmTrackState {
        status: BgmTrackStatus::On,
        fade_in: 1,
        fade_out: 3,
        volume: 1.0,
    };

    mgr.request_page(3);
    assert_eq!(mgr.tracks[0].status, BgmTrackStatus::FadingOut);
    assert_eq!(mgr.tracks[0].fade_out, 1);
    assert_eq!(mgr.tracks[1].fade_out, 2);
    assert_eq!(mgr.tracks[2].fade_out, 4);
    assert_eq!(mgr.tracks[3].status, BgmTrackStatus::FadingOut);
    assert_eq!(mgr.tracks[3].fade_out, 1);

    // `SeBgmManager` (`0x00194300`) global_bgm_fade ducking ramp at `dt / 2.0`
    assert_eq!(
        bgm_ducking_mode_from_game_state(5, 0),
        BgmDuckingMode::PausedOrMenu
    );
    assert_eq!(
        bgm_ducking_mode_from_game_state(4, 99),
        BgmDuckingMode::DramaDemo
    );
    assert_eq!(
        bgm_ducking_mode_from_game_state(4, 0),
        BgmDuckingMode::Normal
    );

    // Pause: 1.0 -> 0.5 at 0.5/s takes 1.0s
    let half_step = step_global_bgm_fade(1.0, BgmDuckingMode::PausedOrMenu, 0.5);
    assert!((half_step - 0.75).abs() < 1e-6);
    let paused = step_global_bgm_fade(half_step, BgmDuckingMode::PausedOrMenu, 1.0);
    assert!((paused - 0.5).abs() < 1e-6);

    // Demo: 0.5 -> 0.7 at 0.5/s takes 0.4s
    let demo = step_global_bgm_fade(paused, BgmDuckingMode::DramaDemo, 0.5);
    assert!((demo - 0.7).abs() < 1e-6);
}

#[test]
fn test_se_2d_manager_room_stop_ducking_and_slew() {
    let mut pool = SePoolManager::default();
    let listener = SeListenerContext {
        camera_pos: Vec3::ZERO,
        player_pos: Vec3::ZERO,
        wvs_matrix: Mat4::IDENTITY,
        room_id: 10,
    };
    let mut actions = Vec::new();

    // Register managed 2D SFX (flags & 8 != 0, flags & 3 == 0)
    let res = pool.call_pos(
        1.0,
        12_009,
        Some(Vec3::new(750.0, 0.0, 1000.0)),
        SE_FLAG_MANAGED_2D,
        &listener,
        &mut actions,
    );
    assert_eq!(res, SeCallResult::Played2d { se_id: 12_009 });
    // Duplicate rejected per `SLUS_202.28` `0x00192270` line 195
    assert_eq!(
        pool.call_pos(
            1.0,
            12_009,
            Some(Vec3::new(750.0, 0.0, 1000.0)),
            SE_FLAG_MANAGED_2D,
            &listener,
            &mut actions,
        ),
        SeCallResult::NotPlayed
    );

    // Tick 0.25s in PausedOrMenu: duck_fade ramps 0.0 -> 0.25, slew limit = (0.25 * 128) = 32
    // Initial last_attenuation_byte was 0; target at effective_vol = 0.25 is 191, slewed to 32!
    actions.clear();
    pool.tick_2d(&listener, BgmDuckingMode::PausedOrMenu, 0.25, &mut actions);
    assert_eq!(
        actions,
        vec![SeBackendAction::Update2d {
            se_id: 12_009,
            direction_deg: 0,
            attenuation_byte: 32,
        }]
    );
    assert!((pool.query_2d_timer(12_009, false) - 0.25).abs() < 1e-6);

    // Room change auto-stops the managed 2D slot
    actions.clear();
    let other_room = SeListenerContext {
        room_id: 11,
        ..listener
    };
    pool.tick_2d(&other_room, BgmDuckingMode::Normal, 0.016, &mut actions);
    assert_eq!(actions, vec![SeBackendAction::Stop2d { se_id: 12_009 }]);
    assert_eq!(pool.query_2d_timer(12_009, false), -1.0);
}

#[test]
fn test_se_3d_manager_priority1_cap_and_2d_fallback() {
    let change_list = vec![
        Se2d3dMapping {
            id_2d: 12_009,
            id_3d: 40_221,
            bank_id: 1,
        },
        Se2d3dMapping {
            id_2d: 12_019,
            id_3d: 40_220,
            bank_id: 1,
        },
        Se2d3dMapping {
            id_2d: 12_200,
            id_3d: 40_224,
            bank_id: 1,
        },
        Se2d3dMapping {
            id_2d: 12_201,
            id_3d: 40_225,
            bank_id: 1,
        },
        Se2d3dMapping {
            id_2d: 12_208,
            id_3d: 40_222,
            bank_id: 1,
        },
    ];
    let mut pool = SePoolManager::new(change_list);
    pool.active_3d_bank = 1;

    let listener = SeListenerContext {
        camera_pos: Vec3::ZERO,
        player_pos: Vec3::ZERO,
        wvs_matrix: Mat4::IDENTITY,
        room_id: 5,
    };
    let mut actions = Vec::new();

    // First 4 priority-1 calls promote 2D -> 3D (`se_3d_channel_max == 4`)
    for (i, &id_2d) in [12_009, 12_019, 12_200, 12_201].iter().enumerate() {
        let res = pool.call_pos(
            1.0,
            id_2d,
            Some(Vec3::new(750.0, 0.0, 1000.0)),
            SE_FLAG_PRIO_1,
            &listener,
            &mut actions,
        );
        assert!(matches!(res, SeCallResult::Played3d { channel, .. } if channel == i));
    }
    assert_eq!(pool.se_3d_channel_number, 4);
    assert!(pool.is_3d_playing(40_221));

    // 5th priority-1 call hits cap (4) -> stays 2D via `SeChange3Dto2D` fallback!
    let res5 = pool.call_pos(
        1.0,
        12_208,
        Some(Vec3::new(750.0, 0.0, 1000.0)),
        SE_FLAG_PRIO_1,
        &listener,
        &mut actions,
    );
    assert_eq!(res5, SeCallResult::Played2d { se_id: 12_208 });

    // `SeStop` on 2D ID 12_009 stops paired 3D channel 40_221 and decrements `se_3d_channel_number`
    actions.clear();
    pool.stop(12_009, &mut actions);
    assert_eq!(pool.se_3d_channel_number, 3);
    assert!(!pool.is_3d_playing(40_221));
    assert!(actions.contains(&SeBackendAction::Stop2d { se_id: 12_009 }));
    assert!(actions.contains(&SeBackendAction::Stop3d {
        channel: 0,
        se_id: 40_221,
    }));
}

#[test]
fn test_acoustics_native_upgrade_toggle_gate() {
    let area = RoadArea {
        x0: 0.0,
        z0: 0.0,
        x1: 1500.0,
        z1: 9000.0,
        x2: 1500.0,
        z2: 9000.0,
        min_hy: 0.0,
        max_hy: -2500.0,
    };
    let corridor = RoadRecord {
        lim_sw: area,
        lim_rd: area,
        kind_id: 0,
        flags: 0,
        area_size_type: AreaSizeType::Small,
        rd_type: RoadType::Road,
        mv_y_type: 0,
        ofs_watch_hy: -300.0,
        trace_btm_hy: -300.0,
        rd_dir_type: 0,
        projection: 448.0,
        proj_volume: 0.0,
        proj_sec: 0.5,
        cam_mv_type: CamMovementType::Chase,
        params: CamMovementParams::Chase(Default::default()),
    };

    // Strict 1:1 Vanilla PS2 (`enable_dynamic_room_acoustics = false`): dry Off preset
    let vanilla = evaluate_room_acoustics_gated(Some(&corridor), true, false);
    assert_eq!(vanilla.preset, EnvironmentalPreset::Off);
    assert_eq!(vanilla.wet_mix, 0.0);
    assert_eq!(vanilla.feedback, 0.0);

    // Native Enhanced (`enable_dynamic_room_acoustics = true`): corridor reverb active
    let enhanced = evaluate_room_acoustics_gated(Some(&corridor), true, true);
    assert_eq!(enhanced.preset, EnvironmentalPreset::Corridor);
    assert!(enhanced.wet_mix > 0.4);
}
