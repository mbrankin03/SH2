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

    // Navigate to Audio Tab (Tab 1)
    controller.switch_options_tab(true);
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

    // When only one pack is present, Audio tab has 8 rows (Row 7 = Restored Cut Audio, Audio Pack row hidden)
    let menu_single = OptionsMenu::new_with_audio_packs(AppConfig::default(), status_ps2);
    let mut menu_single = menu_single;
    menu_single.active_tab = OptionsTab::Audio;
    assert_eq!(menu_single.item_count(), 8);
    assert_eq!(menu_single.item_display(7).unwrap().0, "Restored Cut Audio");
    assert!(menu_single.item_display(8).is_none());

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
    controller.switch_options_tab(true); // Audio tab
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Audio);
    assert_eq!(controller.options_menu.item_count(), 9);

    // Row 7 is "Restored Cut Audio" defaulting to OFF (Vanilla) (`SH2-62`)
    let (cut_label, cut_val) = controller.options_menu.item_display(7).unwrap();
    assert_eq!(cut_label, "Restored Cut Audio");
    assert_eq!(cut_val, "OFF (Vanilla)");

    // Row 8 is "Audio Pack" defaulting to Auto
    controller.options_menu.selected_index = 8;
    let (label, val0) = controller.options_menu.item_display(8).unwrap();
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
        controller.options_menu.item_display(8).unwrap().1,
        "SH2:EE Lossless (48 kHz)"
    );

    // Cycle Right -> Ps2Original
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.audio.audio_pack_source,
        AudioPackPreference::Ps2Original
    );
    assert_eq!(
        controller.options_menu.item_display(8).unwrap().1,
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
