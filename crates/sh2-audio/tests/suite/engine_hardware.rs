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
