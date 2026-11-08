//! Authentic sound effect loading and playback for menu navigation cues.

use crate::{AudioEngine, AudioError};
use kira::sound::static_sound::StaticSoundHandle;
use sh2_assets::formats::sound::WavEntry;
use sh2_core::AudioPackPreference;
use std::path::Path;

/// Sound cue types for menu interactions matching authentic PS2 / PC Silent Hill 2 IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCueKind {
    /// Authentic Cursor Move sound (game SE ID 10000 / sddata293).
    CursorMove,
    /// Authentic Confirm selection sound (game SE ID 10002 / sddata297).
    Confirm,
    /// Authentic Cancel / Back sound (game SE ID 10003 / sddata291).
    Cancel,
    /// Authentic Game Start audio transition (game SE ID 15002 / g_start.wav).
    GameStartTransition,
    /// Authentic footstep loading sound (sddata280 / footstep_a).
    Footstep,
}

/// Preloaded authentic menu sound effects loaded directly from retail Silent Hill 2 / SH2:EE assets.
pub struct MenuAudioBank {
    pub cursor_move: Option<WavEntry>,
    pub confirm: Option<WavEntry>,
    pub cancel: Option<WavEntry>,
    pub game_start: Option<WavEntry>,
    pub footstep: Option<WavEntry>,
}

impl MenuAudioBank {
    /// Attempts to load authentic 1:1 WAV sounds from the game directories,
    /// prioritizing SH2:EE 48 kHz lossless (`sh2e/sound/`) by default.
    pub fn load_from_dirs(data_root: &Path, ee_root: Option<&Path>) -> Self {
        Self::load_from_dirs_for_source(data_root, ee_root, AudioPackPreference::Auto)
    }

    /// Attempts to load authentic 1:1 WAV sounds from the game directories according to `source`:
    /// - `Auto` / `Sh2eeLossless`: prefers `sh2e/sound/sddata.bin` and `sh2e/sound/extra/g_start.wav`, falling back to `data/sound/`.
    /// - `Ps2Original`: prefers `data/sound/sddata.bin` and `data/sound/extra/g_start.wav`, falling back to `sh2e/sound/`.
    pub fn load_from_dirs_for_source(
        data_root: &Path,
        ee_root: Option<&Path>,
        source: AudioPackPreference,
    ) -> Self {
        let mut sfx_bank = Self {
            cursor_move: None,
            confirm: None,
            cancel: None,
            game_start: None,
            footstep: None,
        };

        let prefer_ee = matches!(
            source,
            AudioPackPreference::Auto | AudioPackPreference::Sh2eeLossless
        );

        // 1. Try loading sddata.bin according to source preference
        let sddata_candidates = if prefer_ee {
            [
                ee_root.map(|p| p.join("sound/sddata.bin")),
                Some(data_root.join("sound/sddata.bin")),
            ]
        } else {
            [
                Some(data_root.join("sound/sddata.bin")),
                ee_root.map(|p| p.join("sound/sddata.bin")),
            ]
        };

        for candidate in sddata_candidates.into_iter().flatten() {
            if candidate.exists() {
                if let Ok(bytes) = std::fs::read(&candidate) {
                    if let Ok(bank) = sh2_assets::formats::sound::parse_sddata(&bytes) {
                        // In sddata.bin:
                        // sddata280 = Footstep
                        // sddata291 = Menu Back (Cancel, 10003)
                        // sddata293 = Menu Move (Cursor, 10000)
                        // sddata297 = Menu Select (Confirm, 10002)
                        if bank.entries.len() > 297 {
                            sfx_bank.footstep = Some(bank.entries[280].clone());
                            sfx_bank.cancel = Some(bank.entries[291].clone());
                            sfx_bank.cursor_move = Some(bank.entries[293].clone());
                            sfx_bank.confirm = Some(bank.entries[297].clone());
                            tracing::info!(
                                "Loaded authentic 1:1 menu navigation and footstep SFX ({:?}) from {:?}",
                                source,
                                candidate
                            );
                            break;
                        }
                    }
                }
            }
        }

        // 2. Try loading game start sound (sh2e/sound/extra/g_start.wav or data/sound/extra/g_start.wav)
        let gstart_candidates = if prefer_ee {
            [
                ee_root.map(|p| p.join("sound/extra/g_start.wav")),
                Some(data_root.join("sound/extra/g_start.wav")),
            ]
        } else {
            [
                Some(data_root.join("sound/extra/g_start.wav")),
                ee_root.map(|p| p.join("sound/extra/g_start.wav")),
            ]
        };

        for candidate in gstart_candidates.into_iter().flatten() {
            if candidate.exists() {
                if let Ok(bytes) = std::fs::read(&candidate) {
                    if let Ok(bank) = sh2_assets::formats::sound::parse_sddata(&bytes) {
                        if let Some(entry) = bank.entries.into_iter().next() {
                            sfx_bank.game_start = Some(entry);
                            tracing::info!(
                                "Loaded authentic 1:1 game start SFX (15002) from {:?}",
                                candidate
                            );
                            break;
                        }
                    }
                }
            }
        }

        sfx_bank
    }

    /// Play a sound cue through the audio engine using authentic audio.
    pub fn play(
        &self,
        engine: &mut AudioEngine,
        cue: SoundCueKind,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        let entry = match cue {
            SoundCueKind::CursorMove => self.cursor_move.as_ref(),
            SoundCueKind::Confirm => self.confirm.as_ref(),
            SoundCueKind::Cancel => self.cancel.as_ref(),
            SoundCueKind::GameStartTransition => self.game_start.as_ref(),
            SoundCueKind::Footstep => self.footstep.as_ref(),
        };

        if let Some(wav) = entry {
            let handle = engine.play_sfx_2d(wav, 1.0)?;
            Ok(Some(handle))
        } else {
            // Fallback synthetic generator if asset was unavailable
            synthesize_fallback_cue(engine, cue)
        }
    }
}

/// Fallback synthetic waveform generator when retail asset files are absent.
fn synthesize_fallback_cue(
    engine: &mut AudioEngine,
    cue: SoundCueKind,
) -> Result<Option<StaticSoundHandle>, AudioError> {
    let (freq1, freq2, duration_ms, is_cancel) = match cue {
        SoundCueKind::CursorMove => (500.0, 580.0, 40, false),
        SoundCueKind::Confirm => (600.0, 900.0, 85, false),
        SoundCueKind::Cancel => (420.0, 310.0, 80, true),
        SoundCueKind::GameStartTransition => (240.0, 120.0, 400, false),
        SoundCueKind::Footstep => (160.0, 80.0, 120, true),
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

/// Helper function to play menu sound using an optional bank.
pub fn play_menu_sound(
    engine: &mut AudioEngine,
    bank: Option<&MenuAudioBank>,
    cue: SoundCueKind,
) -> Result<Option<StaticSoundHandle>, AudioError> {
    if let Some(b) = bank {
        b.play(engine, cue)
    } else {
        synthesize_fallback_cue(engine, cue)
    }
}
