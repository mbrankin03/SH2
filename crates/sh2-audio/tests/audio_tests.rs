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
}
