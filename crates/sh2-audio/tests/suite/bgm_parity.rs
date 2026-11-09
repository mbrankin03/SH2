//! 1:1 BGM & multi-channel AIX room area mixing integration tests (`SH2-60` / `SH2-58`).

use glam::Vec3;
use sh2_audio::*;

#[test]
fn test_bgm_files_and_pages_descriptors_integrity() {
    assert_eq!(BGM_FILES.len(), 47);
    assert_eq!(BGM_PAGES.len(), 48);
    assert_eq!(STAGE_GLB_CRD.len(), 59);
    assert_eq!(STAGE_SOUND_DATA.len(), 59);

    // Page 0 is the (-1, -1) silence page
    assert_eq!(bgm_page_entry(0).load_id, -1);
    assert_eq!(bgm_page_entry(0).start_id, -1);

    // Every non-silence BGM page (`1..=47`) must reference a valid BGM_FILES entry
    for (page_idx, page) in BGM_PAGES.iter().enumerate() {
        if page_idx == 0 {
            continue;
        }
        assert!(
            page.load_id > 0,
            "BGM_PAGES[{page_idx}] should have positive load_id"
        );
        let file = find_bgm_file_by_load_id(page.load_id);
        assert!(
            file.is_some(),
            "BGM_PAGES[{page_idx}] load_id {} must exist in BGM_FILES",
            page.load_id
        );
        let vol = master_volume_for_start_id(page.start_id as u32);
        assert!(
            (0.1..=1.0).contains(&vol),
            "BGM_PAGES[{page_idx}] start_id {} master volume {vol} out of range",
            page.start_id
        );
    }
}

#[test]
fn test_bgm_room_id_lookup_parity() {
    // Stage 2 (`Stg_toilet`, Restroom) uses glb_crd = 6
    assert_eq!(stage_glb_crd(2), 6);
    // At spawn (-19606.6, 20429.55): block 0x1D0 -> room 0x01
    assert_eq!(bgm_room_id(6, -19606.6, 20429.55), 0x01);
    // Stage 3 (`Stg_observation`) uses glb_crd = 5 -> room 0x02
    assert_eq!(stage_glb_crd(3), 5);
    assert_eq!(bgm_room_id(5, 0.0, -5000.0), 0x02);
    // Stage 22 (`Stg_hospital_1fw_b`) uses glb_crd = 10:
    assert_eq!(stage_glb_crd(22), 10);
    // block 0x10C (x in [-160000, -120000], z in [-80000, -40000]) -> room 0x34 (Page 18)
    assert_eq!(bgm_room_id(10, -140_000.0, -60_000.0), 0x34);
    // block 0x10E (x in [-160000, -120000], z in [-40000, 0]) -> room 0x30 (Page 19)
    assert_eq!(bgm_room_id(10, -140_000.0, -20_000.0), 0x30);
}

#[test]
fn test_pick_bgm_page_se_bgm_change_parity() {
    let mut flags = BgmEventFlags::default();

    // Stage 2 (`Stg_toilet`):
    // - Initial state (`flag_5e4 == 0`): Page 46 (`forest/bgm_114_a.adx`)
    assert_eq!(pick_bgm_page(2, 1, &flags), 46);
    // - After mirror cutscene (`flag_5e4 = 0x02`): Page 47 (`forest/bgm_114_b.aix`)
    flags.flag_5e4 = 0x02;
    assert_eq!(pick_bgm_page(2, 1, &flags), 47);
    // - Bit 7 set (`flag_5e4 = 0x80`): Silence (Page 0)
    flags.flag_5e4 = 0x80;
    assert_eq!(pick_bgm_page(2, 1, &flags), 0);

    // Reset `flag_5e4` for Stage 3 (`Stg_observation`): Page 47 (`forest/bgm_114_b.aix`)
    flags.flag_5e4 = 0x00;
    assert_eq!(pick_bgm_page(3, 2, &flags), 47);

    // Stage 6 (`Stg_apart_e1f`):
    // - Room 0x1E (when `flag_5e8 & 0x0020_0000 == 0`) -> Page 7 (`apart/bgm_101.aix`)
    assert_eq!(pick_bgm_page(6, 0x1E, &flags), 7);
    // - Room 0x01 -> Page 4 (`apart/bgm_118.aix`)
    assert_eq!(pick_bgm_page(6, 0x01, &flags), 4);

    // Stage 22 (`Stg_hospital_1fw_b`):
    assert_eq!(pick_bgm_page(22, 0x34, &flags), 18);
    assert_eq!(pick_bgm_page(22, 0x30, &flags), 19);
    assert_eq!(pick_bgm_page(22, 0x4E, &flags), 20);

    // Stage 40 (`Stg_hotel_1f_f`):
    // - Before music box (`flag_61b & 1 == 0`): Page 35 (`hotel/bgm_009.adx`)
    assert_eq!(pick_bgm_page(40, 0x99, &flags), 35);
    // - Music box active (`flag_61b = 1`): Page 34 (`hotel/bgm_121.aix`)
    flags.flag_61b = 0x01;
    assert_eq!(pick_bgm_page(40, 0x99, &flags), 34);

    // Stage 43 (`Stg_hotel_bf_b`, Boiler Room):
    // - `flag_61c == 0` -> Page 38 (`hotel/bgm_108.aix`)
    flags.flag_61c = 0x00;
    assert_eq!(pick_bgm_page(43, 0xBD, &flags), 38);
    // - `flag_61c & 0x04 != 0` -> Page 44 (`hotel/bgm_120.aix`)
    flags.flag_61c = 0x04;
    assert_eq!(pick_bgm_page(43, 0xBD, &flags), 44);
    // - `room_id == 0xAC` -> Page 39 (`hotel/bgm_124.aix`)
    assert_eq!(pick_bgm_page(43, 0xAC, &flags), 39);

    // Stage 45 (`Stg_hotel_2f_b`): Page 40 (`hotel/bgm_122.aix`)
    assert_eq!(pick_bgm_page(45, 0xAF, &flags), 40);

    // Stage 47 (`Stg_hotel_fire`, Final Boss): Page 41 (`hotel/bgm_113.aix`)
    assert_eq!(pick_bgm_page(47, 0xC8, &flags), 41);

    // Stages 48..=51 (Endings): Page 42 (`end/bgm_002.adx`)
    for stg in 48..=51 {
        assert_eq!(pick_bgm_page(stg, 0x8F, &flags), 42);
    }
}

#[test]
fn test_evaluate_situation_page_bgm_page_set_parity() {
    let mut flags = BgmEventFlags::default();

    // 1. Calm (0) when HP >= 20% and no enemy within 6000 units
    assert_eq!(evaluate_situation_page(18, 0x26, 1.0, None, &flags), 0);
    assert_eq!(
        evaluate_situation_page(18, 0x26, 0.25, Some(6000.1), &flags),
        0
    );

    // 2. LowHealth (2) when HP < 20% (0.20) and no enemy within 6000 units
    assert_eq!(evaluate_situation_page(18, 0x26, 0.19, None, &flags), 2);

    // 3. EnemyNear (3) when an active enemy is within 6000 units (takes priority over LowHealth)
    assert_eq!(
        evaluate_situation_page(18, 0x26, 0.10, Some(5999.9), &flags),
        3
    );
    assert_eq!(
        evaluate_situation_page(18, 0x26, 1.0, Some(1200.0), &flags),
        3
    );

    // 4. High-priority EvBgmControl override (> 3) takes precedence over EnemyNear & LowHealth:
    // Stage 43 (`Stg_hotel_bf_b`) with `flag_61c & 0x04 != 0` returns 4
    flags.flag_61c = 0x04;
    assert_eq!(
        evaluate_situation_page(43, 0xBD, 0.10, Some(500.0), &flags),
        4
    );

    // Stage 40 (`Stg_hotel_1f_f`) Music Box puzzle (`hotel_music_box_active = true`, `flag_618` bits -> pages 6..=13)
    flags.hotel_music_box_active = true;
    for bits in 0u32..8u32 {
        let mut f618 = 0u32;
        if (bits & 1) != 0 {
            f618 |= 0x0022_2000;
        }
        if (bits & 2) != 0 {
            f618 |= 0x0044_4000;
        }
        if (bits & 4) != 0 {
            f618 |= 0x0088_8000;
        }
        flags.flag_618 = f618;
        let sit = evaluate_situation_page(40, 0x99, 1.0, None, &flags);
        assert_eq!(sit, 6 + bits);
    }
}

#[test]
fn test_sound_area_evaluation_and_priority_mixing() {
    // Stage 3 (`ObservationDeck`):
    // At spawn (0.0, -100.0, -5000.0) in Situation 0 (`Calm`), record #4 matches:
    // chanstat = [0, 0x100020, 0x14, 0x15, 0x15, 0, 0]
    let eval_ob = evaluate_sound_areas(3, Vec3::new(0.0, -100.0, -5000.0), 0);
    assert_eq!(eval_ob.matched_box_count, 1);
    assert!(!eval_ob.used_default_record);
    assert_eq!(eval_ob.track_fades[0], (0, 0));
    assert_eq!(eval_ob.track_fades[2], (0, 0));
    assert_eq!(eval_ob.track_fades[3], (3, 3));
    assert_eq!(eval_ob.track_fades[4], (3, 3));

    // Moving to (-5000.0, -100.0, -10000.0) transitions into record #1 (`x: -15300..3500, z: -16700..-5750`),
    // where Track 2 is `0x15` (3, 3), Track 3 is `0x14` (0, 0), and Track 4 is `0x15` (3, 3)!
    let eval_ob_path = evaluate_sound_areas(3, Vec3::new(-5000.0, -100.0, -10000.0), 0);
    assert_eq!(eval_ob_path.matched_box_count, 1);
    assert_eq!(eval_ob_path.track_fades[2], (3, 3));
    assert_eq!(eval_ob_path.track_fades[3], (0, 0));
    assert_eq!(eval_ob_path.track_fades[4], (3, 3));

    // Outside all bounding boxes falls back to record #0 (`used_default_record == true`)
    let eval_fallback = evaluate_sound_areas(3, Vec3::new(-90000.0, -100.0, 90000.0), 0);
    assert!(eval_fallback.used_default_record);
    assert_eq!(eval_fallback.matched_box_count, 1);

    // Situation presets 6..=15 (`SITUATION_PRESETS_6_TO_15` at `0x8A9EB8`)
    let eval_preset_9 = evaluate_sound_areas(40, Vec3::ZERO, 9);
    assert_eq!(eval_preset_9.track_fades[0], (2, 1));
    assert_eq!(eval_preset_9.track_fades[1], (2, 1));
    assert_eq!(eval_preset_9.track_fades[2], (2, 1));
    assert_eq!(eval_preset_9.track_fades[3], (0, 0));
}

#[test]
fn test_bgm_manager_fade_state_machine_and_page_crossfade() {
    let mut mgr = BgmManager::new(
        "/nonexistent_game_root",
        sh2_core::AudioPackSource::EnhancedLossless,
    );
    mgr.event_flags.flag_5e4 = 0x02;

    // Tick Stage 3 (`ObservationDeck`, Page 47 -> `forest/bgm_114_b.aix`) at spawn `(0, -100, -5000)`
    let spawn = Vec3::new(0.0, -100.0, -5000.0);
    mgr.tick(3, spawn, 1.0, None, 0.0, None).unwrap();
    assert_eq!(mgr.current_page, 47);
    assert_eq!(mgr.loaded_rel_path, Some("forest/bgm_114_b.aix"));
    assert_eq!(mgr.tracks[3].status, BgmTrackStatus::FadingIn);
    assert_eq!(mgr.tracks[4].status, BgmTrackStatus::FadingIn);
    assert_eq!(mgr.tracks[2].status, BgmTrackStatus::Off);

    // Advance 3.0s (half of fade type 3 = 6.0s)
    mgr.tick(3, spawn, 1.0, None, 3.0, None).unwrap();
    assert!((mgr.tracks[3].volume - 0.5).abs() < 1e-4);
    assert_eq!(mgr.tracks[3].status, BgmTrackStatus::FadingIn);

    // Advance another 3.0s -> reaches full volume 1.0 (`BgmTrackStatus::On`)
    mgr.tick(3, spawn, 1.0, None, 3.0, None).unwrap();
    assert!((mgr.tracks[3].volume - 1.0).abs() < 1e-4);
    assert_eq!(mgr.tracks[3].status, BgmTrackStatus::On);

    // Walk to `(-5000, -100, -10000)` where Track 2 fades in (type 3 = 6.0s) and Track 3 fades out (type 3 = 6.0s)
    let path_pos = Vec3::new(-5000.0, -100.0, -10000.0);
    mgr.tick(3, path_pos, 1.0, None, 3.0, None).unwrap();
    assert_eq!(mgr.tracks[2].status, BgmTrackStatus::FadingIn);
    assert!((mgr.tracks[2].volume - 0.5).abs() < 1e-4);
    assert_eq!(mgr.tracks[3].status, BgmTrackStatus::FadingOut);
    assert!((mgr.tracks[3].volume - 0.5).abs() < 1e-4);

    // Now switch to Stage 47 (`Stg_hotel_fire`, Page 41 -> `hotel/bgm_113.aix`) and Stage 48 (`Stg_end_recovery`, Page 42 -> `end/bgm_002.adx`, single-stream ADX)
    mgr.tick(47, Vec3::ZERO, 1.0, None, 1.5, None).unwrap();
    assert_eq!(mgr.current_page, 47);
    assert_eq!(mgr.target_page, 41);

    // Finish fading out all tracks (+3.5s) -> next tick loads Page 41 (`hotel/bgm_113.aix`)
    mgr.tick(47, Vec3::ZERO, 1.0, None, 3.5, None).unwrap();
    mgr.tick(47, Vec3::ZERO, 1.0, None, 0.016, None).unwrap();
    assert_eq!(mgr.current_page, 41);
    assert_eq!(mgr.loaded_rel_path, Some("hotel/bgm_113.aix"));

    // Now switch to Stage 48 (`Stg_end_recovery`, Page 42 -> `end/bgm_002.adx`, single-stream ADX)
    mgr.tick(48, Vec3::ZERO, 1.0, None, 3.0, None).unwrap();
    mgr.tick(48, Vec3::ZERO, 1.0, None, 0.016, None).unwrap();
    assert_eq!(mgr.current_page, 42);
    assert_eq!(mgr.loaded_rel_path, Some("end/bgm_002.adx"));
    assert!(mgr.loaded_is_adx);
    assert_eq!(mgr.tracks[0].status, BgmTrackStatus::On);
    assert!((mgr.tracks[0].volume - 1.0).abs() < 1e-4);
}

#[test]
fn test_dual_source_adx_and_aix_decoding_from_retail_and_sh2e_assets() {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let game_root = &resolver.game_root;
    if !game_root.join("data/sound/adx").exists() {
        println!(
            "Skipping on-disk retail/sh2e BGM asset test (assets not present at {:?})",
            game_root
        );
        return;
    }

    let sample_files = [
        ("forest/bgm_114_a.adx", 1usize),
        ("forest/bgm_114_b.aix", 6usize),
        ("apart/bgm_118.aix", 5usize),
        ("hospital/bgm_123.aix", 2usize),
        ("hospital/bgm_106.aix", 4usize),
        ("hotel/bgm_122.aix", 5usize),
        ("end/bgm_002.adx", 1usize),
        ("end/bgm_012.adx", 1usize),
    ];

    for &(rel_path, expected_layers) in &sample_files {
        let (ee_path, ee_layers) = decode_bgm_layers_from_disk(
            game_root,
            rel_path,
            sh2_core::AudioPackSource::EnhancedLossless,
        )
        .unwrap_or_else(|e| panic!("Failed to decode EnhancedLossless {rel_path}: {e}"));
        assert!(
            ee_path.to_string_lossy().contains("sh2e/sound/adx"),
            "Expected sh2e path for {rel_path}, got {}",
            ee_path.display()
        );
        assert_eq!(
            ee_layers.len(),
            expected_layers,
            "Layer count mismatch for sh2e {rel_path}"
        );
        assert_eq!(
            ee_layers[0].header.sample_rate, 48000,
            "Expected 48 kHz sample rate for sh2e {rel_path}"
        );
        assert!(!ee_layers[0].frames.is_empty());

        let (orig_path, orig_layers) = decode_bgm_layers_from_disk(
            game_root,
            rel_path,
            sh2_core::AudioPackSource::OriginalPs2,
        )
        .unwrap_or_else(|e| panic!("Failed to decode OriginalPs2 {rel_path}: {e}"));
        assert!(
            orig_path.to_string_lossy().contains("data/sound/adx"),
            "Expected data/sound/adx path for {rel_path}, got {}",
            orig_path.display()
        );
        assert_eq!(
            orig_layers.len(),
            expected_layers,
            "Layer count mismatch for original {rel_path}"
        );
        assert!(
            matches!(
                orig_layers[0].header.sample_rate,
                22050 | 24000 | 44100 | 48000
            ),
            "Unexpected sample rate {} for original {rel_path}",
            orig_layers[0].header.sample_rate
        );
        assert!(!orig_layers[0].frames.is_empty());
    }
}
