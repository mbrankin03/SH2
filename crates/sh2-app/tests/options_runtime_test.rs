//! Integration tests for SH2-19: Options Menu Features, Engine Bindings & Edge Cases.

use sh2_app::menu_controller::AppMenuController;
use sh2_core::config::AppConfig;
use sh2_ui::menu::{MenuNavAction, MenuSoundCue, TitlePhase};
use sh2_ui::options::{OptionsMenu, OptionsTab};
use std::fs;

#[test]
fn test_options_audio_device_selection_and_enumeration() {
    let test_dir = std::env::temp_dir().join("sh2_test_options_audio_dev");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let config_path = test_dir.join("config.json");

    let mut controller = AppMenuController::new(&config_path);
    controller.title_sm.phase = TitlePhase::OptionsMenu;

    // Provide mocked discovered audio devices
    controller.options_menu.set_audio_devices(vec![
        "Default (System)".into(),
        "MiniFuse 2 Line Out".into(),
        "AD103 High Definition Audio Controller Digital Stereo (HDMI)".into(),
        "Built-in Analog Audio".into(),
    ]);

    // Navigate to Audio Options (row 2 on Root O P T I O N S)
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Root);
    controller.options_menu.selected_index = 2;
    controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Audio);

    // Audio Row Index 4: Output Device
    controller.options_menu.selected_index = 4;
    let (label, val) = controller.options_menu.item_display(4).unwrap();
    assert_eq!(label, "Output Device");
    assert_eq!(val, "Default (System)");

    // Cycle Right -> MiniFuse 2
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller
            .options_menu
            .draft_config
            .audio
            .selected_output_device
            .as_deref(),
        Some("MiniFuse 2 Line Out")
    );

    // Cycle Right -> HDMI
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller
            .options_menu
            .draft_config
            .audio
            .selected_output_device
            .as_deref(),
        Some("AD103 High Definition Audio Controller Digital Stereo (HDMI)")
    );

    // Commit changes
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert!(config_path.exists());

    // Verify atomic file read
    let loaded = AppConfig::load_from_file(&config_path);
    assert_eq!(
        loaded.audio.selected_output_device.as_deref(),
        Some("AD103 High Definition Audio Controller Digital Stereo (HDMI)")
    );

    // Re-instantiate controller from persisted file and verify draft preservation
    let mut fresh_controller = AppMenuController::new(&config_path);
    assert_eq!(
        fresh_controller
            .options_menu
            .draft_config
            .audio
            .selected_output_device
            .as_deref(),
        Some("AD103 High Definition Audio Controller Digital Stereo (HDMI)")
    );

    // Reset to defaults
    fresh_controller.options_menu.reset_to_defaults();
    assert_eq!(
        fresh_controller
            .options_menu
            .draft_config
            .audio
            .selected_output_device,
        None
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_options_reset_to_defaults_and_discard() {
    let mut menu = OptionsMenu::new(AppConfig::default());
    assert!(!menu.is_dirty);

    // Alter multiple video and audio settings
    menu.draft_config.video.grain_intensity = 10;
    menu.draft_config.video.vsync = false;
    menu.draft_config.audio.master_volume = 0.4;
    menu.is_dirty = true;

    // Discard reverts back to original
    menu.discard_changes();
    assert!(!menu.is_dirty);
    assert_eq!(menu.draft_config.video.grain_intensity, 65);
    assert!(menu.draft_config.video.vsync);
    assert_eq!(menu.draft_config.audio.master_volume, 1.0);

    // Reset to factory defaults modifies draft and flags dirty
    menu.draft_config.game.bullet_multiplier = sh2_core::config::BulletMultiplier::Three;
    menu.commit_changes();
    assert_eq!(
        menu.original_config.game.bullet_multiplier,
        sh2_core::config::BulletMultiplier::Three
    );

    menu.reset_to_defaults();
    assert!(menu.is_dirty);
    assert_eq!(
        menu.draft_config.game.bullet_multiplier,
        sh2_core::config::BulletMultiplier::One
    );
}

#[test]
fn test_sh2_56_high_refresh_framerate_cap_options_and_pacer() {
    use sh2_core::config::FrameRateCap;
    use sh2_core::FramePacer;

    let mut menu = OptionsMenu::new(AppConfig::default());
    assert_eq!(menu.active_tab, OptionsTab::Root);
    menu.navigate_vertical(1); // Advanced Options
    assert!(menu.enter_submenu());
    assert_eq!(menu.active_tab, OptionsTab::Video);

    // Row 3 in Video tab is Frame Rate Cap
    menu.selected_index = 3;
    let (label, val) = menu.item_display(3).unwrap();
    assert_eq!(label, "Frame Rate Cap");
    assert_eq!(val, "60 FPS (Authentic)");

    // Cycle Right through 120 -> 144 -> 240 -> Uncapped -> 30 -> 60
    let expected = [
        (FrameRateCap::Fps120, "120 FPS (High Refresh)", Some(120)),
        (FrameRateCap::Fps144, "144 FPS (High Refresh)", Some(144)),
        (FrameRateCap::Fps240, "240 FPS (OLED Ultra)", Some(240)),
        (FrameRateCap::Uncapped, "Uncapped (VRR)", None),
        (FrameRateCap::Fps30, "30 FPS (PS2 Cutscene Rate)", Some(30)),
        (FrameRateCap::Fps60, "60 FPS (Authentic)", Some(60)),
    ];

    let mut pacer = FramePacer::default();
    for (cap, exp_label, exp_fps) in expected {
        menu.adjust_value(1);
        assert_eq!(menu.draft_config.video.framerate_cap, cap);
        assert_eq!(menu.item_display(3).unwrap().1, exp_label);
        assert_eq!(cap.target_fps(), exp_fps);

        pacer.set_cap(cap);
        assert_eq!(pacer.cap, cap);
    }
}

#[test]
fn test_sh2_59_dual_source_audio_pack_detection_and_options_toggle() {
    use sh2_core::config::{AudioPackDetectionStatus, AudioPackPreference};

    let test_dir = std::env::temp_dir().join("sh2_59_audio_pack_detection_test");
    let _ = fs::remove_dir_all(&test_dir);

    // 1. Only PS2 / PC Retail audio directory present (`data/sound/adx/` and `data/sound/sddata.bin`)
    let root_ps2_only = test_dir.join("ps2_only");
    fs::create_dir_all(root_ps2_only.join("data/sound/adx")).unwrap();
    fs::write(root_ps2_only.join("data/sound/sddata.bin"), b"RIFF").unwrap();

    let status_ps2 = AudioPackDetectionStatus::detect_from_root(&root_ps2_only);
    assert!(!status_ps2.sh2ee_lossless_present);
    assert!(status_ps2.ps2_original_present);
    assert!(!status_ps2.both_detected());
    assert_eq!(
        status_ps2.resolve_source(AudioPackPreference::Auto),
        AudioPackPreference::Ps2Original
    );

    // When only one pack is present, Audio tab has 11 rows (Row 6 = 3D Spatial Audio, Row 7 = Wall Occlusion, Row 9 = Restored Cut Audio, Row 10 = Dynamic Room Acoustics, Audio Pack row hidden)
    let menu_single = OptionsMenu::new_with_audio_packs(AppConfig::default(), status_ps2);
    let mut menu_single = menu_single;
    menu_single.active_tab = OptionsTab::Audio;
    assert_eq!(menu_single.item_count(), 11);
    assert_eq!(menu_single.item_display(6).unwrap().0, "3D Spatial Audio");
    assert_eq!(menu_single.item_display(7).unwrap().0, "Wall Occlusion");
    assert_eq!(menu_single.item_display(9).unwrap().0, "Restored Cut Audio");
    assert_eq!(
        menu_single.item_display(10).unwrap().0,
        "Dynamic Room Acoustics"
    );
    assert!(menu_single.item_display(11).is_none());

    // 2. Only SH2:EE Lossless audio directory present (`sh2e/sound/adx/`)
    let root_ee_only = test_dir.join("ee_only");
    fs::create_dir_all(root_ee_only.join("sh2e/sound/adx")).unwrap();
    fs::create_dir_all(root_ee_only.join("sh2e/sound/extra")).unwrap();

    let status_ee = AudioPackDetectionStatus::detect_from_root(&root_ee_only);
    assert!(status_ee.sh2ee_lossless_present);
    assert!(!status_ee.ps2_original_present);
    assert!(status_ee.sh2ee_extra_present);
    assert!(!status_ee.both_detected());
    assert_eq!(
        status_ee.resolve_source(AudioPackPreference::Auto),
        AudioPackPreference::Sh2eeLossless
    );

    // 3. Both SH2:EE (`sh2e/sound/`) and Original PS2/PC (`data/sound/`) present
    let root_dual = test_dir.join("dual_packs");
    fs::create_dir_all(root_dual.join("sh2e/sound/adx")).unwrap();
    fs::create_dir_all(root_dual.join("data/sound/adx")).unwrap();

    let status_dual = AudioPackDetectionStatus::detect_from_root(&root_dual);
    assert!(status_dual.sh2ee_lossless_present);
    assert!(status_dual.ps2_original_present);
    assert!(status_dual.both_detected());

    let cfg_path = test_dir.join("dual_config.json");
    let mut controller = AppMenuController::new_with_audio_packs(&cfg_path, status_dual);
    controller
        .options_menu
        .set_audio_devices(vec!["Default (System)".into(), "Stereo Out".into()]);
    controller.title_sm.phase = TitlePhase::OptionsMenu;
    controller.options_menu.selected_index = 2;
    controller.handle_nav(MenuNavAction::Confirm); // Enter Audio Options submenu
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Audio);
    assert_eq!(controller.options_menu.item_count(), 12);

    // Row 6 is "3D Spatial Audio" defaulting to Classic PS2 Stereo (`SH2-84`)
    let (spatial_label, spatial_val) = controller.options_menu.item_display(6).unwrap();
    assert_eq!(spatial_label, "3D Spatial Audio");
    assert_eq!(spatial_val, "Classic PS2 Stereo");

    // Row 7 is "Wall Occlusion" defaulting to OFF (Original PS2) (`SH2-84`)
    let (occ_label, occ_val) = controller.options_menu.item_display(7).unwrap();
    assert_eq!(occ_label, "Wall Occlusion");
    assert_eq!(occ_val, "OFF (Original PS2)");

    // Row 9 is "Restored Cut Audio" defaulting to OFF (Vanilla) (`SH2-62`)
    let (cut_label, cut_val) = controller.options_menu.item_display(9).unwrap();
    assert_eq!(cut_label, "Restored Cut Audio");
    assert_eq!(cut_val, "OFF (Vanilla)");

    // Row 10 is "Dynamic Room Acoustics" defaulting to OFF (Original PS2) (`SH2-137`)
    let (ac_label, ac_val) = controller.options_menu.item_display(10).unwrap();
    assert_eq!(ac_label, "Dynamic Room Acoustics");
    assert_eq!(ac_val, "OFF (Original PS2)");

    // Row 11 is "Audio Pack" defaulting to Auto
    controller.options_menu.selected_index = 11;
    let (label, val0) = controller.options_menu.item_display(11).unwrap();
    assert_eq!(label, "Audio Pack");
    assert_eq!(val0, "Auto (Prefer SH2:EE Lossless)");
    assert_eq!(
        controller.resolved_audio_pack_source(),
        AudioPackPreference::Sh2eeLossless
    );

    // Cycle Right -> Sh2eeLossless
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.audio.audio_pack_source,
        AudioPackPreference::Sh2eeLossless
    );
    assert_eq!(
        controller.options_menu.item_display(11).unwrap().1,
        "SH2:EE Lossless (48 kHz)"
    );

    // Cycle Right -> Ps2Original
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.audio.audio_pack_source,
        AudioPackPreference::Ps2Original
    );
    assert_eq!(
        controller.options_menu.item_display(11).unwrap().1,
        "Original PS2 / PC (24/44.1 kHz)"
    );

    // Commit and verify resolved engine source & disk persistence
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(
        controller.resolved_audio_pack_source(),
        AudioPackPreference::Ps2Original
    );

    let reloaded = AppConfig::load_from_file(&cfg_path);
    assert_eq!(
        reloaded.audio.audio_pack_source,
        AudioPackPreference::Ps2Original
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_sh2_93_options_controls_sync_and_live_camera_wiring() {
    use sh2_core::config::{CameraCyclePreset, GameplayCameraMode, OtsShoulderSide, ReticleMode};
    use sh2_core::modern_camera::{OtsReticleState, PlayerMeshVisibility};
    use sh2_core::scenario::ScenarioKind;
    use sh2_core::SimulationCore;

    let test_dir = std::env::temp_dir().join("sh2_93_options_camera_wiring_test");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let cfg_path = test_dir.join("camera_config.json");

    let mut controller = AppMenuController::new(&cfg_path);
    let mut sim_core = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim_core.apply_control_options(&controller.options_menu.original_config.controls);
    assert_eq!(
        sim_core.active_camera_mode(),
        GameplayCameraMode::ClassicPs2Rail
    );

    // Navigate Options -> Controls (rows 5..=10 are SH2-93 modern camera settings)
    controller.title_sm.phase = TitlePhase::OptionsMenu;
    controller.options_menu.active_tab = OptionsTab::Controls;

    // Row 5: Default Camera -> Over-the-Shoulder 3D
    controller.options_menu.selected_index = 5;
    controller.handle_nav(MenuNavAction::Right);
    // Row 6: R3 Cycle Preset -> OTS / FPS Only (`Right` x3 from `AllThree`)
    controller.options_menu.selected_index = 6;
    controller.handle_nav(MenuNavAction::Right);
    controller.handle_nav(MenuNavAction::Right);
    controller.handle_nav(MenuNavAction::Right);
    // Row 7: OTS Shoulder -> Left Shoulder
    controller.options_menu.selected_index = 7;
    controller.handle_nav(MenuNavAction::Right);
    // Row 8: Aiming Reticle -> Subtle Always
    controller.options_menu.selected_index = 8;
    controller.handle_nav(MenuNavAction::Right);
    // Row 9: First-Person FOV -> 85° (`Right` x2 from 75°)
    controller.options_menu.selected_index = 9;
    controller.handle_nav(MenuNavAction::Right);
    controller.handle_nav(MenuNavAction::Right);
    // Row 10: First-Person Sway -> OFF (`Right` x1 from ON)
    controller.options_menu.selected_index = 10;
    controller.handle_nav(MenuNavAction::Right);

    // Commit via Confirm -> produces `config_applied_pending` just like Pause/Title menu in `sh2-app`
    assert_eq!(
        controller.handle_nav(MenuNavAction::Confirm),
        MenuSoundCue::Confirm
    );
    let committed = controller
        .config_applied_pending
        .take()
        .expect("Committed Options must populate config_applied_pending");
    assert_eq!(
        committed.controls.default_camera_mode,
        GameplayCameraMode::OverTheShoulder
    );
    assert_eq!(
        committed.controls.camera_cycle_preset,
        CameraCyclePreset::OtsAndFirstPerson
    );
    assert_eq!(committed.controls.ots_shoulder, OtsShoulderSide::Left);
    assert_eq!(committed.controls.ots_reticle, ReticleMode::AlwaysSubtle);
    assert_eq!(committed.controls.first_person_fov_deg, 85);
    assert!(!committed.controls.first_person_head_bob);

    // Apply to `SimulationCore` and step crossfade
    sim_core.apply_control_options(&committed.controls);
    for _ in 0..20 {
        sim_core.tick();
    }
    assert_eq!(
        sim_core.active_camera_mode(),
        GameplayCameraMode::OverTheShoulder
    );
    assert_eq!(
        sim_core.modern_camera.ots.shoulder_side,
        OtsShoulderSide::Left
    );
    assert!(matches!(
        sim_core.active_reticle_state(),
        OtsReticleState::SubtleDot { .. }
    ));

    // Press `V` (`press_camera_mode_cycle`) with `OtsAndFirstPerson` preset -> enters `FirstPerson` at 85° FOV!
    sim_core.input.press_camera_mode_cycle();
    for _ in 0..22 {
        sim_core.tick();
    }
    assert_eq!(
        sim_core.active_camera_mode(),
        GameplayCameraMode::FirstPerson
    );
    assert_eq!(
        sim_core.player_mesh_visibility(),
        PlayerMeshVisibility::HideHeadOnly
    );
    assert!((sim_core.active_fov_y(1.0).to_degrees() - 85.0).abs() < 0.25);
    assert!((sim_core.active_near_clip(1.0) - 8.0).abs() < 1e-3);

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_sh2_84_binaural_hrtf_options_toggle_and_persistence() {
    use sh2_core::config::SpatialAudioMode;

    let test_dir = std::env::temp_dir().join("sh2_84_binaural_hrtf_options_test");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let cfg_path = test_dir.join("hrtf_config.json");

    let mut controller = AppMenuController::new(&cfg_path);
    controller.title_sm.phase = TitlePhase::OptionsMenu;
    controller.options_menu.selected_index = 2;
    controller.handle_nav(MenuNavAction::Confirm); // Enter Audio Options submenu
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Audio);

    // Row 6: 3D Spatial Audio defaults to Classic PS2 Stereo
    controller.options_menu.selected_index = 6;
    let (label, val0) = controller.options_menu.item_display(6).unwrap();
    assert_eq!(label, "3D Spatial Audio");
    assert_eq!(val0, "Classic PS2 Stereo");
    assert_eq!(
        controller
            .options_menu
            .draft_config
            .audio
            .spatial_audio_mode,
        SpatialAudioMode::ClassicPs2Stereo
    );

    // Toggle Right -> Binaural HRTF (3D Headphones), live-previewing via `config_applied_pending`
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller
            .options_menu
            .draft_config
            .audio
            .spatial_audio_mode,
        SpatialAudioMode::BinauralHrtf
    );
    assert_eq!(
        controller.options_menu.item_display(6).unwrap().1,
        "Binaural HRTF (3D Headphones)"
    );
    assert_eq!(
        controller
            .config_applied_pending
            .as_ref()
            .unwrap()
            .audio
            .spatial_audio_mode,
        SpatialAudioMode::BinauralHrtf
    );

    // Row 7: Wall Occlusion defaults to OFF (Original PS2) for strict 1:1 vanilla parity, toggles to ON (.cld Raycast)
    controller.options_menu.selected_index = 7;
    let (occ_label, occ_val0) = controller.options_menu.item_display(7).unwrap();
    assert_eq!(occ_label, "Wall Occlusion");
    assert_eq!(occ_val0, "OFF (Original PS2)");
    assert!(
        !controller
            .options_menu
            .draft_config
            .audio
            .enable_acoustic_occlusion
    );
    controller.handle_nav(MenuNavAction::Right);
    assert!(
        controller
            .options_menu
            .draft_config
            .audio
            .enable_acoustic_occlusion
    );
    assert_eq!(
        controller.options_menu.item_display(7).unwrap().1,
        "ON (.cld Raycast)"
    );
    assert!(
        controller
            .config_applied_pending
            .as_ref()
            .unwrap()
            .audio
            .enable_acoustic_occlusion
    );

    // Commit and verify JSON persistence roundtrip for both Binaural HRTF and Wall Occlusion
    assert_eq!(
        controller.handle_nav(MenuNavAction::Confirm),
        MenuSoundCue::Confirm
    );
    let reloaded = AppConfig::load_from_file(&cfg_path);
    assert_eq!(
        reloaded.audio.spatial_audio_mode,
        SpatialAudioMode::BinauralHrtf
    );
    assert!(reloaded.audio.enable_acoustic_occlusion);

    // Toggle Left on Row 7 -> OFF (Original PS2) and Row 6 -> Classic PS2 Stereo
    controller.handle_nav(MenuNavAction::Left);
    assert!(
        !controller
            .options_menu
            .draft_config
            .audio
            .enable_acoustic_occlusion
    );
    controller.options_menu.selected_index = 6;
    controller.handle_nav(MenuNavAction::Left);
    assert_eq!(
        controller
            .options_menu
            .draft_config
            .audio
            .spatial_audio_mode,
        SpatialAudioMode::ClassicPs2Stereo
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_sh2_136_aces_tonemapping_options_toggle_and_persistence() {
    use sh2_core::config::EnhancedPreset;

    let test_dir = std::env::temp_dir().join("sh2_136_aces_tonemapping_options_test");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let cfg_path = test_dir.join("aces_config.json");

    let mut controller = AppMenuController::new(&cfg_path);
    controller.title_sm.phase = TitlePhase::OptionsMenu;
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Root);
    controller.options_menu.selected_index = 1;
    controller.handle_nav(MenuNavAction::Confirm); // Enter Advanced Options submenu
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Video);

    // Row 12 in Video tab is ACES Tonemapping (default ON for Native Enhanced)
    controller.options_menu.selected_index = 12;
    let (label, val0) = controller.options_menu.item_display(12).unwrap();
    assert_eq!(label, "ACES Tonemapping");
    assert_eq!(val0, "ON (Filmic HDR)");
    assert!(controller.options_menu.draft_config.video.aces_tonemapping);

    // Toggle OFF -> strict PS2 Linear clamp, commit & verify JSON roundtrip
    controller.handle_nav(MenuNavAction::Right);
    assert!(!controller.options_menu.draft_config.video.aces_tonemapping);
    assert_eq!(
        controller.options_menu.item_display(12).unwrap().1,
        "OFF (PS2 Linear)"
    );
    assert_eq!(
        controller.handle_nav(MenuNavAction::Confirm),
        MenuSoundCue::Confirm
    );
    let reloaded = AppConfig::load_from_file(&cfg_path);
    assert!(!reloaded.video.aces_tonemapping);

    // Verify Enhanced tab row 11 toggles the same field and respects version presets
    controller.options_menu.active_tab = OptionsTab::Enhanced;
    controller.options_menu.selected_index = 0;
    controller
        .options_menu
        .draft_config
        .apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert!(!controller.options_menu.draft_config.video.aces_tonemapping);
    controller
        .options_menu
        .draft_config
        .apply_enhanced_preset(EnhancedPreset::Sh2ee);
    assert!(!controller.options_menu.draft_config.video.aces_tonemapping);
    controller
        .options_menu
        .draft_config
        .apply_enhanced_preset(EnhancedPreset::NativeEnhanced);
    assert!(controller.options_menu.draft_config.video.aces_tonemapping);

    controller.options_menu.selected_index = 11;
    assert_eq!(
        controller.options_menu.item_display(11).unwrap().0,
        "ACES Tonemapping"
    );
    controller.handle_nav(MenuNavAction::Left);
    assert!(!controller.options_menu.draft_config.video.aces_tonemapping);

    let _ = fs::remove_dir_all(&test_dir);
}
