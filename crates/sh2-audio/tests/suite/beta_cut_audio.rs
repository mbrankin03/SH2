//! Beta/cut audio catalog (`sddata[360..=364]`, `voice.afs` cut streams, `CutAudioRestorer`,
//! and Hotel Music Box 7th layer restoration) tests (`SH2-62`).

use sh2_audio::*;

#[test]
fn test_sh2_62_unreferenced_sddata_catalog_and_cut_audio_restorer() {
    // 1. Verify that `sddata[360..=364]` (`UNREFERENCED_SDDATA_ENTRIES`) are the exact 5 entries
    // in `0..417` that have zero references in `SE_ID_TO_SDDATA_INDEX` (`0x8A1ED8` 666 pairs).
    let mut referenced = [false; SDDATA_ENTRY_COUNT];
    for &(_, sd_idx) in &SE_ID_TO_SDDATA_INDEX {
        referenced[sd_idx as usize] = true;
    }
    let unreferenced_indices: Vec<usize> = (0..SDDATA_ENTRY_COUNT)
        .filter(|&i| !referenced[i])
        .collect();
    assert_eq!(
        unreferenced_indices,
        vec![360, 361, 362, 363, 364],
        "Expected exactly sddata[360..=364] (internal IDs 5360..=5364) to be unreferenced in SE_PAIRS"
    );
    for (i, entry) in UNREFERENCED_SDDATA_ENTRIES.iter().enumerate() {
        assert_eq!(entry.sddata_index, Some(360 + i));
        assert_eq!(entry.internal_driver_id, Some(5360 + i as u16));
        assert_eq!(entry.primary_se_id, None);
        assert!(entry.loops);
    }

    // 2. Verify `LAURA_LETTER_VOICEOVER_SLICES` (`PlayUnusedAudio.cpp` 11-page `SCENE_45` sequence)
    assert_eq!(LAURA_LETTER_VOICEOVER_SLICES.len(), 11);
    assert_eq!(LAURA_LETTER_VOICEOVER_SLICES[0].start_sec, 0.0);
    assert!((LAURA_LETTER_VOICEOVER_SLICES[10].end_sec - 67.2).abs() < 1e-4);
    for i in 0..10 {
        assert!(
            LAURA_LETTER_VOICEOVER_SLICES[i].end_sec
                <= LAURA_LETTER_VOICEOVER_SLICES[i + 1].start_sec + 1e-4,
            "Expected non-overlapping slice boundary between page {i} and {}",
            i + 1
        );
        assert!(LAURA_LETTER_VOICEOVER_SLICES[i].duration_sec() >= 1.8);
    }

    // 3. Verify `CutAudioRestorer` state machine (`CS_HTL_LAURA_PIANO`, `CS_APT_RPT_CLOSET`, `CS_HSP_BOSS_FINISH`)
    let mut restorer = CutAudioRestorer::new();

    // When `enable_restored_cut_audio == false`, nothing triggers
    assert_eq!(
        restorer.tick(
            false,
            CS_HTL_LAURA_PIANO_ID,
            R_HTL_RESTAURANT_ID,
            1650.0,
            3,
            0,
            0.016
        ),
        CutAudioRestoreAction::None
    );

    // When `enable_restored_cut_audio == true`:
    // - In `CS_APT_RPT_CLOSET` (`0x10`) at `timer = 1905.0`, triggers `PlayClosetCutsceneBonusAudio` once
    assert_eq!(
        restorer.tick(true, CS_APT_RPT_CLOSET_ID, 0x1A, 1905.0, 3, 0, 0.016),
        CutAudioRestoreAction::PlayClosetCutsceneBonusAudio
    );
    assert_eq!(
        restorer.tick(true, CS_APT_RPT_CLOSET_ID, 0x1A, 1910.0, 3, 0, 0.016),
        CutAudioRestoreAction::None
    );

    // - In `CS_HTL_LAURA_PIANO` (`0x3B`) in `R_HTL_RESTAURANT` (`0x9E`):
    //   Before `timer >= 1600.0` or `fade_state != 3` -> None
    assert_eq!(
        restorer.tick(
            true,
            CS_HTL_LAURA_PIANO_ID,
            R_HTL_RESTAURANT_ID,
            1500.0,
            3,
            0,
            0.016
        ),
        CutAudioRestoreAction::None
    );
    //   At `timer = 1600.0, fade_state = 3` -> triggers slice 0 (`0.0s..7.0s`)
    assert_eq!(
        restorer.tick(
            true,
            CS_HTL_LAURA_PIANO_ID,
            R_HTL_RESTAURANT_ID,
            1600.0,
            3,
            0,
            0.016
        ),
        CutAudioRestoreAction::PlayLauraLetterSlice(0)
    );
    //   After 7.1s elapses on slice 0, slice 1 (`7.2s..12.4s`) auto-chains (`PlayUnusedAudio.cpp` line 183)
    assert_eq!(
        restorer.tick(
            true,
            CS_HTL_LAURA_PIANO_ID,
            R_HTL_RESTAURANT_ID,
            1680.0,
            3,
            1,
            7.1
        ),
        CutAudioRestoreAction::PlayLauraLetterSlice(1)
    );
    //   Advancing pages (`text_wait_status` transitions `1 -> 0`) triggers slices 2..=10
    for expected_slice in 2..=10 {
        let _ = restorer.tick(
            true,
            CS_HTL_LAURA_PIANO_ID,
            R_HTL_RESTAURANT_ID,
            1700.0 + expected_slice as f32 * 10.0,
            3,
            1,
            0.1,
        );
        assert_eq!(
            restorer.tick(
                true,
                CS_HTL_LAURA_PIANO_ID,
                R_HTL_RESTAURANT_ID,
                1705.0 + expected_slice as f32 * 10.0,
                3,
                0,
                0.1
            ),
            CutAudioRestoreAction::PlayLauraLetterSlice(expected_slice)
        );
    }
    //   Exiting the letter (`timer = 2069.0`) emits `StopLauraLetterVoiceover`
    assert_eq!(
        restorer.tick(
            true,
            CS_HTL_LAURA_PIANO_ID,
            R_HTL_RESTAURANT_ID,
            2069.0,
            3,
            0,
            0.016
        ),
        CutAudioRestoreAction::StopLauraLetterVoiceover
    );

    // 4. Verify on-disk decoding of `voice.afs[119]` (`SCENE_45`) and `sankaku_toujyou_2.wav` / `machi.wav` when assets exist
    let resolver = sh2_assets::AssetPathResolver::resolve();
    if !resolver.data_dir.join("sound/sddata.bin").exists() {
        return;
    }
    let mut bank = MenuAudioBank::load_from_dirs(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    let laura_wav = bank
        .load_voice_afs_entry(VOICE_AFS_LAURA_LETTER_INDEX)
        .expect("Expected voice.afs[119] (SCENE_45) to decode cleanly");
    assert!(laura_wav.sample_rate >= 22050);
    let slice_0 = sh2_assets::slice_wav_time_range(
        &laura_wav,
        LAURA_LETTER_VOICEOVER_SLICES[0].start_sec,
        LAURA_LETTER_VOICEOVER_SLICES[0].end_sec,
    );
    assert!(!slice_0.data.is_empty());

    if resolver
        .sh2e_dir
        .as_ref()
        .is_some_and(|p| p.join("sound/extra/sankaku_toujyou_2.wav").exists())
    {
        assert!(
            bank.closet_bonus_audio.is_some(),
            "Expected sankaku_toujyou_2.wav to be loaded into closet_bonus_audio"
        );
        assert!(
            bank.machi_steps_audio.is_some(),
            "Expected machi.wav to be loaded into machi_steps_audio"
        );
    }
}

#[test]
fn test_sh2_62_music_box_volume_7th_layer_restoration_and_voice_afs_catalog() {
    // 1. Verify expanded `CUT_AND_RARE_AUDIO_CATALOG` (29 entries including satsugai, mary_let_fix, m_deai_tsuika, bp_mar_tsuika)
    assert_eq!(CUT_AND_RARE_AUDIO_CATALOG.len(), 29);
    assert!(CUT_AND_RARE_AUDIO_CATALOG
        .iter()
        .any(|e| e.voice_afs_index == Some(VOICE_AFS_SATSUGAI_INDEX)
            && e.primary_se_id == Some(0xEAD4)));
    assert!(CUT_AND_RARE_AUDIO_CATALOG.iter().any(|e| e.voice_afs_index
        == Some(VOICE_AFS_MARY_LETTER_FULL_INDEX)
        && e.primary_se_id == Some(0xEAC1)));
    assert!(CUT_AND_RARE_AUDIO_CATALOG.iter().any(|e| e.voice_afs_index
        == Some(VOICE_AFS_MARIA_DEAI_TSUIKA_INDEX)
        && e.primary_se_id == Some(0xEAB3)));
    assert!(CUT_AND_RARE_AUDIO_CATALOG.iter().any(|e| e.voice_afs_index
        == Some(VOICE_AFS_BORN_FROM_A_WISH_TSUIKA_INDEX)
        && e.primary_se_id == Some(0xEAEC)));

    // 2. Verify `MusicBoxVolume.cpp` Stage 41 (`Stg_hotel_2f_f`) BGM page & situation page restoration
    let mut flags_vanilla = BgmEventFlags {
        flag_61b: 0x01, // Music box puzzle solved
        enable_restored_cut_audio: false,
        ..Default::default()
    };
    assert_eq!(pick_bgm_page(41, 0, &flags_vanilla), 36); // Vanilla: Page 36 (`hotel/bgm_121.aix`)
    assert_eq!(eval_stage_ev_bgm_control(41, 0, &flags_vanilla), 0);

    flags_vanilla.enable_restored_cut_audio = true;
    assert_eq!(pick_bgm_page(41, 0, &flags_vanilla), 34); // Restored: Page 34 (`hotel/bgm_112.aix`, `shBgmCall(0x22)`)
    assert_eq!(eval_stage_ev_bgm_control(41, 0, &flags_vanilla), 4);

    // 3. Verify `evaluate_sound_areas_with_restoration` on Stage 40 (Hotel 1F) and Stage 41 (Hotel 2F)
    let hall_1f_pos = glam::Vec3::new(-60000.0, -500.0, 20000.0);
    let eval_40_vanilla = evaluate_sound_areas_with_restoration(40, hall_1f_pos, 4, false);
    assert!(eval_40_vanilla.matched_record_indices.contains(&6));
    assert_eq!(eval_40_vanilla.track_fades[4], (1, 1)); // Vanilla: loud Track 4 active in 1F hallway
    assert_eq!(eval_40_vanilla.track_fades[6], (0, 0)); // Vanilla: Track 6 silent

    let eval_40_restored = evaluate_sound_areas_with_restoration(40, hall_1f_pos, 4, true);
    assert_eq!(eval_40_restored.track_fades[4], (0, 0)); // Restored: loud Track 4 muted in 1F hallway
    assert_eq!(eval_40_restored.track_fades[6], (1, 1)); // Restored: quieter Track 6 (7th layer) active!

    // Stage 40 Record 19 (1F Lobby): verify its exact bounds directly from STAGE_SOUND_DATA[40][19]
    let rec19 = &STAGE_SOUND_DATA[40][19];
    let lobby_1f_pos = glam::Vec3::new(
        (rec19.x0 + rec19.x2) * 0.5,
        (rec19.min_y + rec19.max_y) * 0.5,
        (rec19.z0 + rec19.z2) * 0.5,
    );
    let eval_40_lobby = evaluate_sound_areas_with_restoration(40, lobby_1f_pos, 4, true);
    assert!(eval_40_lobby.matched_record_indices.contains(&19));
    assert_eq!(eval_40_lobby.track_fades[4], (1, 1)); // Full-volume Track 4 remains active in the 1F Lobby

    // Stage 41 Record 3 (2F hallway connected to lobby): `x in [57600, 62000], z in [-20600, -19400], y in [-1800, 50]`
    let hall_2f_pos = glam::Vec3::new(60000.0, -500.0, -20000.0);
    let eval_41_vanilla = evaluate_sound_areas_with_restoration(41, hall_2f_pos, 4, false);
    assert!(eval_41_vanilla.matched_record_indices.contains(&3));
    assert_eq!(eval_41_vanilla.track_fades[6], (0, 0)); // Vanilla: silent in 2F hallway

    let eval_41_restored = evaluate_sound_areas_with_restoration(41, hall_2f_pos, 4, true);
    assert_eq!(eval_41_restored.track_fades[6], (1, 1)); // Restored: quieter Track 6 plays in 2F lobby-connected hallway!
}
