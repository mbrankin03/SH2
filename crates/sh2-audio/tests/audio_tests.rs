// Real hardware audio playback output tests require an OS audio server (DirectSound/WASAPI/ALSA/PulseAudio).
// On headless CI systems, skip physical audio device tests on Windows runners where hardware endpoints are absent.
#![cfg(not(target_os = "windows"))]

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use sh2_assets::{AdxHeader, DecodedAdx, DialogueMessage, WavEntry};
    use sh2_audio::*;
    use std::time::Duration;

    fn make_test_wav(sample_rate: u32, channels: u16, loops: bool) -> WavEntry {
        // Generate 0.1s test tone (440Hz sine wave)
        let total_samples = (sample_rate as f32 * 0.1) as usize;
        let mut pcm_bytes = Vec::with_capacity(total_samples * channels as usize * 2);

        for i in 0..total_samples {
            let t = i as f32 / sample_rate as f32;
            let val = ((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 10000.0) as i16;
            let bytes = val.to_le_bytes();
            for _ in 0..channels {
                pcm_bytes.extend_from_slice(&bytes);
            }
        }

        WavEntry {
            id: 11000,
            channels,
            sample_rate,
            bits_per_sample: 16,
            data: pcm_bytes,
            loops,
            loop_start: 0,
            loop_end: total_samples as u32,
        }
    }

    fn make_test_adx(sample_rate: u32, has_loop: bool) -> DecodedAdx {
        let total_samples = (sample_rate as f32 * 0.1) as usize;
        let mut frames = Vec::with_capacity(total_samples);
        let mut pcm_i16 = Vec::with_capacity(total_samples * 2);

        for i in 0..total_samples {
            let t = i as f32 / sample_rate as f32;
            let val = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            frames.push((val, val));
            let s16 = (val * 32767.0) as i16;
            pcm_i16.push(s16);
            pcm_i16.push(s16);
        }

        DecodedAdx {
            header: AdxHeader {
                channels: 2,
                sample_rate,
                total_samples: total_samples as u32,
                highpass_frequency: 500,
                version: 3,
                data_offset: 0x20,
                has_loop,
                loop_start_sample: if has_loop { 100 } else { 0 },
                loop_end_sample: if has_loop { total_samples as u32 } else { 0 },
            },
            pcm_i16,
            frames,
        }
    }

    #[test]
    fn test_audio_engine_initialization() {
        // Try initializing audio engine; in headless CI environments without ALSA/PulseAudio audio devices,
        // it may gracefully report an InitError.
        match AudioEngine::new() {
            Ok(engine) => {
                assert!(engine.is_active());
                assert_eq!(engine.current_preset(), EnvironmentalPreset::Off);
            }
            Err(e) => {
                println!("Skipping backend playback checks (headless device): {e}");
            }
        }
    }

    #[test]
    fn test_environmental_dsp_presets() {
        assert_eq!(EnvironmentalPreset::Off.reverb_params(), None);
        assert!(EnvironmentalPreset::FoggyStreets.reverb_params().is_some());
        assert!(EnvironmentalPreset::InteriorRoom.reverb_params().is_some());
        assert!(EnvironmentalPreset::Corridor.reverb_params().is_some());
        assert!(EnvironmentalPreset::CavernousEcho.reverb_params().is_some());

        if let Ok(mut engine) = AudioEngine::new() {
            engine.set_environmental_preset(EnvironmentalPreset::InteriorRoom);
            assert_eq!(engine.current_preset(), EnvironmentalPreset::InteriorRoom);
            engine.set_environmental_preset(EnvironmentalPreset::FoggyStreets);
            assert_eq!(engine.current_preset(), EnvironmentalPreset::FoggyStreets);
        }
    }

    #[test]
    fn test_sfx_and_spatial_playback() {
        if let Ok(mut engine) = AudioEngine::new() {
            let wav_mono = make_test_wav(22050, 1, false);
            let play_res = engine.play_sfx_2d(&wav_mono, 1.0);
            assert!(play_res.is_ok(), "2D sound effect playback should succeed");

            let wav_3d = make_test_wav(22050, 1, true);
            let play_3d = engine.play_sfx_3d(&wav_3d, Vec3::new(100.0, 0.0, 200.0), 0.8);
            assert!(
                play_3d.is_ok(),
                "3D spatialized sound effect playback should succeed"
            );

            engine.set_listener_transform(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
        }
    }

    #[test]
    fn test_bgm_streaming_and_looping() {
        if let Ok(mut engine) = AudioEngine::new() {
            let adx_bgm = make_test_adx(44100, true);
            let res = engine.play_bgm(&adx_bgm, 0.7);
            assert!(res.is_ok(), "BGM stream should start without error");

            engine.stop_bgm(Duration::from_millis(50));
        }
    }

    #[test]
    fn test_dialogue_subtitles_and_ducking() {
        if let Ok(mut engine) = AudioEngine::new() {
            let adx_voice = make_test_adx(44100, false);
            let message = DialogueMessage {
                id: 0,
                pages: vec![
                    "Mary...".to_string(),
                    "Could you really be in this town?".to_string(),
                ],
                text: "Mary...\x0cCould you really be in this town?".to_string(),
            };

            let res = engine.play_dialogue(&adx_voice, message, Some(101));
            assert!(res.is_ok(), "Dialogue playback should succeed");

            let total_dur = engine.active_dialogue().unwrap().total_duration;
            assert_eq!(
                engine.active_dialogue().unwrap().current_subtitle(),
                Some("Mary...")
            );

            // Advance time halfway through first page
            let dt1 = total_dur / 4;
            engine.update(dt1);
            assert_eq!(
                engine.active_dialogue().unwrap().current_subtitle(),
                Some("Mary...")
            );

            // Advance time into second page (now elapsed is 3/4 total_duration)
            let dt2 = total_dur / 2;
            engine.update(dt2);
            assert_eq!(
                engine.active_dialogue().unwrap().current_subtitle(),
                Some("Could you really be in this town?")
            );

            // Advance past total duration to complete dialogue
            let dt3 = total_dur / 2;
            engine.update(dt3);
            assert!(
                engine.active_dialogue().is_none(),
                "Dialogue should clear once finished"
            );
        }
    }

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
        // Track 2: 0x14 = bit0(0) -> off; Track 3: 0x15 = bit0(1), in=2+1=3 (6.0s), out=2+1=3 (6.0s); Track 4: 0x15 -> (3, 3)!
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

        // Now switch to Stage 47 (`Stg_hotel_fire`, Page 41 -> `hotel/bgm_113.aix`) and Stage 48 (`Stg_end_recovery`, Page 42 -> `end/bgm_002.adx`, single-stream ADX)!
        // In Stage 47 at `Vec3::ZERO` (Calm situation 0), Track 0 fades in (`0x0B`: fade_in = 2 (1.5s), fade_out = 2 (1.5s)).
        mgr.tick(47, Vec3::ZERO, 1.0, None, 1.5, None).unwrap();
        // Still fading out old page 47
        assert_eq!(mgr.current_page, 47);
        assert_eq!(mgr.target_page, 41);

        // Finish fading out all tracks (+3.5s) -> next tick loads Page 41 (`hotel/bgm_113.aix`)
        mgr.tick(47, Vec3::ZERO, 1.0, None, 3.5, None).unwrap();
        mgr.tick(47, Vec3::ZERO, 1.0, None, 0.016, None).unwrap();
        assert_eq!(mgr.current_page, 41);
        assert_eq!(mgr.loaded_rel_path, Some("hotel/bgm_113.aix"));

        // Now switch to Stage 48 (`Stg_end_recovery`, Page 42 -> `end/bgm_002.adx`, single-stream ADX):
        // First tick requests Page 42 and fades out Page 41's active Track 0; advancing +3.0s completes the fade-out so the next tick loads Page 42!
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

        // Verify case-insensitive path resolution and decoding across both EnhancedLossless (48 kHz)
        // and OriginalPs2 (24/44.1 kHz) packs for representative ADX and multi-layer AIX files,
        // including `end/bgm_002.adx` and `end/bgm_012.adx` (uppercase `BGM_002.adx` on disk in `data/sound/adx/end/`).
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

    #[test]
    fn test_ps2_low_rate_upsampling_and_warm_lowpass_filter() {
        // 1. A 16 kHz original PS2/PC SFX WAV should be upsampled 3x to 48 kHz and trimmed
        let wav_16k = make_test_wav(16000, 1, false);
        let (out_rate, out_frames) = sh2_audio::engine::wav_entry_to_frames_48k(&wav_16k);
        assert_eq!(out_rate, 48000);
        assert_eq!(out_frames.len(), 1600 * 3);

        // 2. A 48 kHz Enhanced Lossless SFX WAV should pass through untouched without resampling
        let wav_48k = make_test_wav(48000, 2, false);
        let (rate_48k, frames_48k) = sh2_audio::engine::wav_entry_to_frames_48k(&wav_48k);
        assert_eq!(rate_48k, 48000);
        assert_eq!(frames_48k.len(), 4800);

        // 3. A 22,050 Hz original ADX stream with a harsh 9.5 kHz Nyquist-adjacent tone
        // should have its high-frequency fizz strongly attenuated by the reconstruction filter
        let sr = 22050u32;
        let n = 2205usize; // 0.1s
        let mut frames = Vec::with_capacity(n);
        let mut pcm_i16 = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let hf = (t * 9500.0 * 2.0 * std::f32::consts::PI).sin() * 0.8;
            frames.push((hf, hf));
            let s = (hf * 32767.0) as i16;
            pcm_i16.push(s);
            pcm_i16.push(s);
        }
        let harsh_adx = DecodedAdx {
            header: AdxHeader {
                channels: 2,
                sample_rate: sr,
                total_samples: n as u32,
                highpass_frequency: 500,
                version: 3,
                data_offset: 0x20,
                has_loop: false,
                loop_start_sample: 0,
                loop_end_sample: 0,
            },
            pcm_i16,
            frames,
        };
        let (adx_rate, adx_frames) = sh2_audio::engine::adx_to_frames_48k(&harsh_adx);
        assert_eq!(adx_rate, 48000);
        // Skip first 100 samples of filter transient, measure RMS of 9.5 kHz tone after 6.2 kHz 4th-order Butterworth LPF
        let rms: f32 = (adx_frames[100..]
            .iter()
            .map(|f| f.left * f.left)
            .sum::<f32>()
            / (adx_frames.len() - 100) as f32)
            .sqrt();
        assert!(
            rms < 0.18,
            "Expected high-frequency 9.5 kHz harshness to be attenuated below 0.18 RMS, got {rms}"
        );
    }

    #[test]
    fn test_sh2_61_ps2_sfx_attenuation_formula_and_spatial_eval() {
        // 1. Verify exact PS2 attenuation formula: gain = min(1.0, 25.5 / (255.0 * (1.0 - v)))
        assert!((compute_ps2_sfx_attenuation_gain(1.0) - 1.0).abs() < 1e-6);
        assert!((compute_ps2_sfx_attenuation_gain(0.95) - 1.0).abs() < 1e-6);
        assert!((compute_ps2_sfx_attenuation_gain(0.90) - 1.0).abs() < 1e-5);
        // At v = 0.8: 25.5 / (255 * 0.2) = 25.5 / 51.0 = 0.5
        assert!((compute_ps2_sfx_attenuation_gain(0.80) - 0.5).abs() < 1e-5);
        // At v = 0.5: 25.5 / (255 * 0.5) = 0.2
        assert!((compute_ps2_sfx_attenuation_gain(0.50) - 0.2).abs() < 1e-5);
        // At v = 0.0: 25.5 / 255 = 0.1
        assert!((compute_ps2_sfx_attenuation_gain(0.0) - 0.1).abs() < 1e-5);

        // 2. Verify SeCallPosDistanceF (0x5155E0) inner (1500.0) and outer (11500.0) distance bounds
        let listener = Vec3::ZERO;
        let eval_inner =
            compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(0.0, 0.0, 1200.0), 1.0);
        assert!((eval_inner.proximity_v - 1.0).abs() < 1e-5);
        assert!((eval_inner.effective_gain - 1.0).abs() < 1e-5);
        assert_eq!(eval_inner.driver_attenuation_byte, 0);
        assert!(eval_inner.stereo_pan.abs() < 1e-5);

        // Halfway at d = 6500.0 -> v = 0.5, pan to right ear (listener facing +Z, right ear is -X)
        let eval_mid =
            compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(-6500.0, 0.0, 0.0), 1.0);
        assert!((eval_mid.proximity_v - 0.5).abs() < 1e-5);
        assert!((eval_mid.ps2_curve_gain - 0.2).abs() < 1e-5);
        assert!((eval_mid.effective_gain - 0.1).abs() < 1e-5);
        assert!((eval_mid.stereo_pan - 1.0).abs() < 1e-5);

        // Beyond outer cutoff (11500.0) -> v = 0.0, effective_gain = 0.0, byte = 255
        let eval_far =
            compute_se_call_pos_distance_f(listener, 0.0, Vec3::new(0.0, 0.0, 12000.0), 1.0);
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
        // and never map to non-footstep sound effects (such as 280 Shotgun Empty or 367 multi-step burst)
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
        // Verify authentic sh2pc.exe (0x54AFC1) & .cld floor geometry James footstep mappings:
        // - Concrete (.cld 0, 12): 11001 -> sddata[147]
        // - Wood (.cld 7 — Baldwin Mansion `ma`, Pete's Bowling Lanes `bw01`): 19907 / 11005 -> sddata[95, 151]
        // - Metal (.cld 5, 9, 11): 11000 -> sddata[146]
        // - Carpet (.cld 8 — Lakeview Hotel `rr` & Apartments `ap`): 11009 -> sddata[144]
        // - Water (.cld 6): 11003 -> sddata[148] / 83, 84
        // - Grass (.cld 2): 11002 -> sddata[142]
        // - Gravel (.cld 1, 3, 4): 11004 / 11007 -> sddata[150, 149]
        // - Blood/Glass (.cld 10): 11003/11008 -> sddata[148, 141]
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Concrete, false).0,
            147
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Wood, false),
            (95, 151)
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Metal, false).0,
            146
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Carpet, false).0,
            144
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Water, false).0,
            148
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Grass, false).0,
            142
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Gravel, false).0,
            150
        );
        assert_eq!(
            footstep_sddata_indices(false, sh2_core::SurfaceMaterial::Blood, false),
            (148, 141)
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

        // 4. Verify on-disk decoding of `voice.afs[133]` (`SCENE_45`) and `sankaku_toujyou_2.wav` / `machi.wav` when assets exist
        let resolver = sh2_assets::AssetPathResolver::resolve();
        if !resolver.data_dir.join("sound/sddata.bin").exists() {
            return;
        }
        let mut bank =
            MenuAudioBank::load_from_dirs(&resolver.data_dir, resolver.sh2e_dir.as_deref());
        let laura_wav = bank
            .load_voice_afs_entry(VOICE_AFS_LAURA_LETTER_INDEX)
            .expect("Expected voice.afs[133] (SCENE_45) to decode cleanly");
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
        assert!(CUT_AND_RARE_AUDIO_CATALOG
            .iter()
            .any(|e| e.voice_afs_index == Some(VOICE_AFS_MARY_LETTER_FULL_INDEX)
                && e.primary_se_id == Some(0xEAC1)));
        assert!(CUT_AND_RARE_AUDIO_CATALOG
            .iter()
            .any(|e| e.voice_afs_index == Some(VOICE_AFS_MARIA_DEAI_TSUIKA_INDEX)
                && e.primary_se_id == Some(0xEAB3)));
        assert!(CUT_AND_RARE_AUDIO_CATALOG
            .iter()
            .any(|e| e.voice_afs_index == Some(VOICE_AFS_BORN_FROM_A_WISH_TSUIKA_INDEX)
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
        // Stage 40 Record 6 (1F West hallway): `x in [-66800, -55600], z in [19200, 20800], y in [-2200, 50]`
        let hall_1f_pos = glam::Vec3::new(-60000.0, -500.0, 20000.0);
        let eval_40_vanilla =
            evaluate_sound_areas_with_restoration(40, hall_1f_pos, 4, false);
        assert!(eval_40_vanilla.matched_record_indices.contains(&6));
        assert_eq!(eval_40_vanilla.track_fades[4], (1, 1)); // Vanilla: loud Track 4 active in 1F hallway
        assert_eq!(eval_40_vanilla.track_fades[6], (0, 0)); // Vanilla: Track 6 silent

        let eval_40_restored =
            evaluate_sound_areas_with_restoration(40, hall_1f_pos, 4, true);
        assert_eq!(eval_40_restored.track_fades[4], (0, 0)); // Restored: loud Track 4 muted in 1F hallway
        assert_eq!(eval_40_restored.track_fades[6], (1, 1)); // Restored: quieter Track 6 (7th layer) active!

        // Stage 40 Record 19 (1F Lobby): verify its exact bounds directly from STAGE_SOUND_DATA[40][19]
        let rec19 = &STAGE_SOUND_DATA[40][19];
        let lobby_1f_pos = glam::Vec3::new(
            (rec19.x0 + rec19.x2) * 0.5,
            (rec19.min_y + rec19.max_y) * 0.5,
            (rec19.z0 + rec19.z2) * 0.5,
        );
        let eval_40_lobby =
            evaluate_sound_areas_with_restoration(40, lobby_1f_pos, 4, true);
        assert!(eval_40_lobby.matched_record_indices.contains(&19));
        assert_eq!(eval_40_lobby.track_fades[4], (1, 1)); // Full-volume Track 4 remains active in the 1F Lobby

        // Stage 41 Record 3 (2F hallway connected to lobby): `x in [57600, 62000], z in [-20600, -19400], y in [-1800, 50]`
        let hall_2f_pos = glam::Vec3::new(60000.0, -500.0, -20000.0);
        let eval_41_vanilla =
            evaluate_sound_areas_with_restoration(41, hall_2f_pos, 4, false);
        assert!(eval_41_vanilla.matched_record_indices.contains(&3));
        assert_eq!(eval_41_vanilla.track_fades[6], (0, 0)); // Vanilla: silent in 2F hallway

        let eval_41_restored =
            evaluate_sound_areas_with_restoration(41, hall_2f_pos, 4, true);
        assert_eq!(eval_41_restored.track_fades[6], (1, 1)); // Restored: quieter Track 6 plays in 2F lobby-connected hallway!
    }
}
