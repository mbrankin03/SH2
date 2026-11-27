//! Hardware audio playback integration tests (`AudioEngine`, `kira`, `cpal`).
//!
//! Real hardware audio playback output tests require an OS audio server (ALSA/PulseAudio/PipeWire/CoreAudio).
//! On headless Windows CI runners where physical audio endpoints are absent, only these hardware-device tests
//! are skipped while all pure DSP, looping, acoustics, BGM, and SFX tests execute across all platforms.

use super::common::{make_test_adx, make_test_wav};
use glam::Vec3;
use sh2_assets::DialogueMessage;
use sh2_audio::*;
use std::time::Duration;

#[test]
fn test_audio_engine_initialization() {
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
            colored_pages: Vec::new(),
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

/// Verifies 1:1 `first_toilet` (`demo_no = 1`, `sh2pc.exe` `0x008e7ed0`: `stream_no = 60117` ->
/// `FUN_00514b40` `0x00514b40` `voice.afs[117]`, `stream_start = 246.0f`) decoding, non-silent PCM
/// speech window (`11.02s..14.59s` = frames `576..684`, subtitle `579..786`), BGM ducking exemption
/// (`SeBgmManager` `0x00194300` / `0x00516430`), and simultaneous playback alongside `forest/bgm_114_a.adx` (`SH2-188`).
#[test]
fn test_first_toilet_voice_afs_117_decode_and_playback() {
    // `first_toilet` (`demo_no = 1`) is exempt from `0.7` cutscene BGM ducking in both
    // `SLUS_202.28` (`0x00194300`) and retail `sh2pc.exe` (`0x00516430`).
    assert!(is_bgm_demo_duck_exempt(1));
    assert_eq!(
        bgm_ducking_mode_from_game_state(4, 1),
        BgmDuckingMode::Normal
    );

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let candidates = [
        resolver.game_root.join("sh2e/sound/adx/voice/voice.afs"),
        resolver.game_root.join("data/sound/adx/voice/voice.afs"),
    ];
    let bgm_path = resolver
        .game_root
        .join("sh2e/sound/adx/forest/bgm_114_a.adx");
    let mut decoded_any = false;
    for path in &candidates {
        if !path.exists() {
            continue;
        }
        let mut file = std::fs::File::open(path).expect("open voice.afs");
        let wav = sh2_assets::decode_afs_entry_from_reader(&mut file, 117)
            .unwrap_or_else(|e| panic!("decode voice.afs[117] from {}: {e}", path.display()));
        assert_eq!(wav.sample_rate, 44_100, "voice.afs[117] must be 44.1 kHz");
        assert_eq!(wav.channels, 2, "voice.afs[117] must be stereo");
        assert_eq!(wav.bits_per_sample, 16, "voice.afs[117] must be 16-bit PCM");
        let (play_rate, frames) = wav_entry_to_frames_48k(&wav);
        assert_eq!(play_rate, 44_100);
        assert!(
            frames.len() > 600_000,
            "voice.afs[117] must have ~658k frames (~14.92s), got {}",
            frames.len()
        );
        // Verify non-silent speech window (`11.0s..14.6s` = frames `485_100..643_860`,
        // corresponding to `first_toilet` frames `576..684` / subtitle `579..786`).
        let speech_peak = frames[485_100..frames.len().min(643_860)]
            .iter()
            .map(|f| f.left.abs().max(f.right.abs()))
            .fold(0.0f32, f32::max);
        assert!(
            speech_peak > 0.25,
            "voice.afs[117] ({}) speech window must contain audible non-silent PCM (peak={speech_peak})",
            path.display()
        );
        decoded_any = true;

        if let Ok(mut engine) = AudioEngine::new() {
            engine.set_master_volume(0.8);
            engine.set_voice_volume(0.8);
            if bgm_path.exists() {
                engine
                    .play_bgm_file_with_fade(&bgm_path, false, 0.40, Duration::ZERO)
                    .expect("forest/bgm_114_a.adx must start alongside voice");
            }
            engine.play_cutscene_voice(&wav).unwrap_or_else(|e| {
                panic!("play_cutscene_voice failed for {}: {e}", path.display())
            });
            assert!(
                engine.is_voice_playing(),
                "voice.afs[117] must enter Playing state alongside BGM"
            );
            std::thread::sleep(Duration::from_millis(250));
            let pos = engine
                .voice_playback_position_secs()
                .expect("voice playback position must be Some while playing");
            assert!(
                pos > 0.0,
                "voice.afs[117] playback clock must advance alongside BGM (got {pos})"
            );
            engine.stop_voice(Duration::from_millis(20));
            engine.stop_bgm(Duration::from_millis(20));
        }
    }
    if resolver.data_dir.exists() {
        assert!(
            decoded_any,
            "Expected at least one retail voice.afs on disk"
        );
    }
}
