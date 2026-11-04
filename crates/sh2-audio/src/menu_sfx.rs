//! Authentic synthetic sound generation for menu navigation cues.

use crate::{AudioEngine, AudioError};
use kira::sound::static_sound::StaticSoundHandle;
use sh2_assets::formats::sound::WavEntry;

/// Sound cue types for menu interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCueKind {
    CursorMove,
    Confirm,
    Cancel,
    GameStartTransition,
}

/// Plays authentic audio waveforms for menu navigation, confirms, cancels, and game start transitions.
pub fn play_menu_sound(
    engine: &mut AudioEngine,
    cue: SoundCueKind,
) -> Result<Option<StaticSoundHandle>, AudioError> {
    let (freq1, freq2, duration_ms, is_cancel) = match cue {
        SoundCueKind::CursorMove => (500.0, 580.0, 40, false),
        SoundCueKind::Confirm => (600.0, 900.0, 85, false),
        SoundCueKind::Cancel => (420.0, 310.0, 80, true),
        SoundCueKind::GameStartTransition => (240.0, 120.0, 400, false),
    };

    let sample_rate = 22050u32;
    let num_samples = (sample_rate as usize * duration_ms as usize) / 1000;
    let mut pcm = Vec::with_capacity(num_samples * 2);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let frac = i as f32 / num_samples as f32;
        let freq = if is_cancel {
            freq1 + (freq2 - freq1) * frac
        } else if frac < 0.45 {
            freq1
        } else {
            freq2
        };
        let env = (1.0 - frac).powi(2);
        let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.45;
        let sample_i16 = (sample * 32767.0) as i16;
        pcm.extend_from_slice(&sample_i16.to_le_bytes());
    }

    let wav = WavEntry {
        id: 9999,
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        loops: false,
        loop_start: 0,
        loop_end: 0,
        data: pcm,
    };

    let handle = engine.play_sfx_2d(&wav, 0.8)?;
    Ok(Some(handle))
}
