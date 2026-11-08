//! Authentic sound effect loading and playback for `sddata.bin`, `voice.afs`, and `sh2e/sound/extra/*.wav` (`SH2-61`).

use crate::sfx_bank::{
    compute_se_call_pos_distance_f, footstep_sddata_indices, resolve_se_id_to_sddata_index,
    resolve_se_id_to_voice_afs_index, SfxSpatialEval,
};
use crate::{AudioEngine, AudioError};
use glam::Vec3;
use kira::sound::static_sound::StaticSoundHandle;
use sh2_assets::formats::sound::{
    decode_afs_slice_to_wav, parse_afs_index, parse_sddata, WavEntry,
};
use sh2_core::{AudioPackPreference, SurfaceMaterial};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Sound cue types for menu and gameplay interactions matching authentic PS2 / PC Silent Hill 2 IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCueKind {
    /// Authentic Cursor Move sound (game SE ID `10000` / `0x2710` / `sddata[293]`).
    CursorMove,
    /// Authentic Confirm selection sound (game SE ID `10002` / `0x2712` / `sddata[297]`).
    Confirm,
    /// Authentic Cancel / Back sound (game SE ID `10003` / `0x2713` / `sddata[291]`).
    Cancel,
    /// Authentic Game Start audio transition (game SE ID `15002` / `0x3A9A` / `g_start.wav` / `voice.afs[24]`).
    GameStartTransition,
    /// Authentic Save Point chime (game SE ID `10051` / `0x2743` / `save_sound.wav` / `voice.afs[35]`).
    SavePoint,
    /// SH2:EE Flashlight Turn On click (`sh2e/sound/extra/flashlight_on.wav` or `sddata[296]`).
    FlashlightOn,
    /// SH2:EE Flashlight Turn Off click (`sh2e/sound/extra/flashlight_off.wav` or `sddata[292]`).
    FlashlightOff,
    /// Authentic footstep loading sound (`sddata[280]` / `footstep_a`).
    Footstep,
}

/// Preloaded authentic sound effect bank loaded directly from retail Silent Hill 2 (`data/sound/`)
/// or SH2:EE 48 kHz lossless (`sh2e/sound/`) assets (`SH2-61`).
pub struct MenuAudioBank {
    /// Active audio pack preference used to load this bank.
    pub loaded_source: AudioPackPreference,
    /// Resolved on-disk path of the loaded `sddata.bin` container, if available.
    pub sddata_path: Option<PathBuf>,
    /// Resolved on-disk path of the `voice.afs` streamed SFX container, if available.
    pub voice_afs_path: Option<PathBuf>,
    /// All 417 RIFF WAVE entries extracted from `sddata.bin` (`0..=416`).
    pub entries: Vec<WavEntry>,
    /// Preloaded streamed SFX entries from `voice.afs` (keyed by AFS entry index `13..=44`).
    pub voice_afs_sfx: HashMap<usize, WavEntry>,
    /// Menu cursor move (`10000` / `sddata[293]`).
    pub cursor_move: Option<WavEntry>,
    /// Menu confirm (`10002` / `sddata[297]`).
    pub confirm: Option<WavEntry>,
    /// Menu cancel (`10003` / `sddata[291]`).
    pub cancel: Option<WavEntry>,
    /// Game start transition (`15002` / `g_start.wav` or `voice.afs[24]`).
    pub game_start: Option<WavEntry>,
    /// Save point chime (`10051` / `save_sound.wav` or `voice.afs[35]`).
    pub save_sound: Option<WavEntry>,
    /// Flashlight turn on (`sh2e/sound/extra/flashlight_on.wav` or `sddata[296]`).
    pub flashlight_on: Option<WavEntry>,
    /// Flashlight turn off (`sh2e/sound/extra/flashlight_off.wav` or `sddata[292]`).
    pub flashlight_off: Option<WavEntry>,
    /// Default footstep (`sddata[280]`).
    pub footstep: Option<WavEntry>,
    /// Alternating left/right footstep parity toggle so consecutive steps alternate between `(left, right)` samples.
    pub step_parity: bool,
}

impl MenuAudioBank {
    /// Attempts to load authentic 1:1 WAV and AFS sounds from the game directories,
    /// prioritizing SH2:EE 48 kHz lossless (`sh2e/sound/`) by default.
    pub fn load_from_dirs(data_root: &Path, ee_root: Option<&Path>) -> Self {
        Self::load_from_dirs_for_source(data_root, ee_root, AudioPackPreference::Auto)
    }

    /// Attempts to load authentic 1:1 WAV and AFS sounds from the game directories according to `source`:
    /// - `Auto` / `Sh2eeLossless`: prefers `sh2e/sound/sddata.bin`, `sh2e/sound/extra/*.wav`, and `sh2e/sound/adx/voice/voice.afs`, falling back to `data/sound/`.
    /// - `Ps2Original`: prefers `data/sound/sddata.bin` and `data/sound/adx/voice/voice.afs`, falling back to `sh2e/sound/`.
    pub fn load_from_dirs_for_source(
        data_root: &Path,
        ee_root: Option<&Path>,
        source: AudioPackPreference,
    ) -> Self {
        let mut sfx_bank = Self {
            loaded_source: source,
            sddata_path: None,
            voice_afs_path: None,
            entries: Vec::new(),
            voice_afs_sfx: HashMap::new(),
            cursor_move: None,
            confirm: None,
            cancel: None,
            game_start: None,
            save_sound: None,
            flashlight_on: None,
            flashlight_off: None,
            footstep: None,
            step_parity: false,
        };

        let prefer_ee = matches!(
            source,
            AudioPackPreference::Auto | AudioPackPreference::Sh2eeLossless
        );

        // 1. Load all 417 entries from `sddata.bin` according to source preference
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
                    if let Ok(bank) = parse_sddata(&bytes) {
                        if bank.entries.len() > 297 {
                            sfx_bank.footstep = Some(bank.entries[280].clone());
                            sfx_bank.cancel = Some(bank.entries[291].clone());
                            sfx_bank.flashlight_off = Some(bank.entries[292].clone());
                            sfx_bank.cursor_move = Some(bank.entries[293].clone());
                            sfx_bank.flashlight_on = Some(bank.entries[296].clone());
                            sfx_bank.confirm = Some(bank.entries[297].clone());
                            sfx_bank.entries = bank.entries;
                            sfx_bank.sddata_path = Some(candidate.clone());
                            tracing::info!(
                                "Loaded authentic 1:1 SFX bank ({} entries, {:?}) from {:?}",
                                sfx_bank.entries.len(),
                                source,
                                candidate
                            );
                            break;
                        }
                    }
                }
            }
        }

        // 2. Load streamed SFX entries (`24` = g_start, `35` = save_sound, `36` = subscreen) from `voice.afs`
        let afs_candidates = if prefer_ee {
            [
                ee_root.map(|p| p.join("sound/adx/voice/voice.afs")),
                Some(data_root.join("sound/adx/voice/voice.afs")),
            ]
        } else {
            [
                Some(data_root.join("sound/adx/voice/voice.afs")),
                ee_root.map(|p| p.join("sound/adx/voice/voice.afs")),
            ]
        };

        for candidate in afs_candidates.into_iter().flatten() {
            if candidate.exists() {
                if let Ok(mut file) = File::open(&candidate) {
                    // Read first 2 KB to parse the 159-entry AFS directory header without reading the full 280 MB file
                    let mut header_buf = vec![0u8; 2048];
                    if let Ok(n) = file.read(&mut header_buf) {
                        header_buf.truncate(n);
                        if let Ok(afs_idx) = parse_afs_index(&header_buf) {
                            sfx_bank.voice_afs_path = Some(candidate.clone());
                            // Preload key streamed SFX entries (`17` = chainsaw afs, `24` = g_start, `35` = save_sound, `36` = menu transition)
                            for &entry_idx in &[17usize, 24, 35, 36] {
                                if let Some(desc) = afs_idx.get(entry_idx) {
                                    if desc.length > 0 && desc.length <= 4 * 1024 * 1024 {
                                        let mut slice = vec![0u8; desc.length as usize];
                                        if file.seek(SeekFrom::Start(desc.offset as u64)).is_ok()
                                            && file.read_exact(&mut slice).is_ok()
                                        {
                                            if let Ok(wav) =
                                                decode_afs_slice_to_wav(&slice, entry_idx as u32)
                                            {
                                                sfx_bank.voice_afs_sfx.insert(entry_idx, wav);
                                            }
                                        }
                                    }
                                }
                            }
                            if sfx_bank.game_start.is_none() {
                                sfx_bank.game_start = sfx_bank.voice_afs_sfx.get(&24).cloned();
                            }
                            if sfx_bank.save_sound.is_none() {
                                sfx_bank.save_sound = sfx_bank.voice_afs_sfx.get(&35).cloned();
                            }
                            break;
                        }
                    }
                }
            }
        }

        // 3. Load SH2:EE standalone extra WAV overrides (`sh2e/sound/extra/*.wav`) when preferring SH2:EE (or as fallback)
        Self::load_extra_wav_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/g_start.wav",
            &mut sfx_bank.game_start,
        );
        Self::load_extra_wav_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/save_sound.wav",
            &mut sfx_bank.save_sound,
        );
        Self::load_extra_wav_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/flashlight_on.wav",
            &mut sfx_bank.flashlight_on,
        );
        Self::load_extra_wav_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/flashlight_off.wav",
            &mut sfx_bank.flashlight_off,
        );

        sfx_bank
    }

    fn load_extra_wav_override(
        data_root: &Path,
        ee_root: Option<&Path>,
        prefer_ee: bool,
        rel_path: &str,
        slot: &mut Option<WavEntry>,
    ) {
        // When `Ps2Original` is explicitly requested and `slot` was already populated from `data/` (`sddata.bin` or `voice.afs`),
        // keep the original PS2/PC sound rather than overwriting it with an SH2:EE extra WAV.
        if !prefer_ee && slot.is_some() {
            return;
        }

        let candidates = if prefer_ee {
            [
                ee_root.map(|p| p.join(rel_path)),
                Some(data_root.join(rel_path)),
            ]
        } else {
            [
                Some(data_root.join(rel_path)),
                ee_root.map(|p| p.join(rel_path)),
            ]
        };

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                if let Ok(bytes) = std::fs::read(&candidate) {
                    if let Ok(bank) = parse_sddata(&bytes) {
                        if let Some(entry) = bank.entries.into_iter().next() {
                            *slot = Some(entry);
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Returns the number of loaded `sddata.bin` entries (`417` when retail/SH2:EE assets are present).
    #[inline]
    pub fn sddata_entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the sample rate in Hz of entry `0` in `sddata.bin` (`48000` for SH2:EE lossless, `16000`/`22050` for PS2/PC).
    #[inline]
    pub fn primary_sample_rate(&self) -> u32 {
        self.entries
            .first()
            .or(self.cursor_move.as_ref())
            .map(|e| e.sample_rate)
            .unwrap_or(0)
    }

    /// Looks up a `WavEntry` by its 0-based `sddata.bin` index (`0..417`).
    #[inline]
    pub fn get_sddata_entry(&self, index: usize) -> Option<&WavEntry> {
        self.entries.get(index)
    }

    /// Resolves any game `se_id` (`u32`, either from the 666-entry `0x8A1ED8` `sddata.bin` table or the 32-entry `0x8A1E58` `voice.afs` table).
    pub fn resolve_se_id(&self, se_id: u32) -> Option<&WavEntry> {
        match se_id {
            10000 => self
                .cursor_move
                .as_ref()
                .or_else(|| self.get_sddata_entry(293)),
            10002 => self.confirm.as_ref().or_else(|| self.get_sddata_entry(297)),
            10003 => self.cancel.as_ref().or_else(|| self.get_sddata_entry(291)),
            10051 => self
                .save_sound
                .as_ref()
                .or_else(|| self.voice_afs_sfx.get(&35)),
            15002 => self
                .game_start
                .as_ref()
                .or_else(|| self.voice_afs_sfx.get(&24)),
            _ => {
                if let Some(sd_idx) = resolve_se_id_to_sddata_index(se_id) {
                    if let Some(wav) = self.get_sddata_entry(sd_idx) {
                        return Some(wav);
                    }
                }
                if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
                    if let Some(wav) = self.voice_afs_sfx.get(&afs_idx) {
                        return Some(wav);
                    }
                }
                None
            }
        }
    }

    /// Plays a sound effect by its 0-based `sddata.bin` index (`0..417`) at 2D `volume`.
    pub fn play_sddata_index(
        &self,
        engine: &mut AudioEngine,
        index: usize,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        if let Some(wav) = self.get_sddata_entry(index) {
            let handle = engine.play_sfx_2d(wav, volume)?;
            Ok(Some(handle))
        } else {
            synthesize_fallback_cue(engine, SoundCueKind::Footstep)
        }
    }

    /// Plays a game sound effect by its `se_id` (`0x5147E0` / `0x5148B0`, e.g., `11048` Handgun, `11051` Shotgun, `11032` Plank, `11040` Pipe/Kick).
    pub fn play_se_id(
        &self,
        engine: &mut AudioEngine,
        se_id: u32,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        if let Some(wav) = self.resolve_se_id(se_id) {
            let handle = engine.play_sfx_2d(wav, volume)?;
            Ok(Some(handle))
        } else {
            synthesize_fallback_cue(engine, SoundCueKind::Confirm)
        }
    }

    /// Plays a 3D positional sound effect by its `se_id` using `SeCallPosDistanceF` (`0x5155E0`) distance attenuation.
    pub fn play_se_id_3d(
        &self,
        engine: &mut AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        source_pos: Vec3,
        base_volume: f32,
    ) -> Result<(SfxSpatialEval, Option<StaticSoundHandle>), AudioError> {
        let eval =
            compute_se_call_pos_distance_f(listener_pos, listener_yaw, source_pos, base_volume);
        if eval.effective_gain <= 0.001 {
            return Ok((eval, None));
        }

        if let Some(wav) = self.resolve_se_id(se_id) {
            let handle = engine.play_sfx_3d(wav, source_pos, eval.effective_gain)?;
            Ok((eval, Some(handle)))
        } else {
            let handle = synthesize_fallback_cue(engine, SoundCueKind::Footstep)?;
            Ok((eval, handle))
        }
    }

    /// Plays a surface-material-specific footstep for James or Maria, alternating left/right step samples (`SH2-61`).
    ///
    /// Returns the `sddata.bin` index that was triggered.
    pub fn play_footstep(
        &mut self,
        engine: &mut AudioEngine,
        is_maria: bool,
        material: SurfaceMaterial,
        is_running: bool,
        volume: f32,
    ) -> Result<(usize, Option<StaticSoundHandle>), AudioError> {
        let (left_idx, right_idx) = footstep_sddata_indices(is_maria, material, is_running);
        let chosen_idx = if self.step_parity {
            right_idx
        } else {
            left_idx
        };
        self.step_parity = !self.step_parity;

        if let Some(wav) = self.get_sddata_entry(chosen_idx).or(self.footstep.as_ref()) {
            let handle = engine.play_sfx_2d(wav, volume)?;
            Ok((chosen_idx, Some(handle)))
        } else {
            let handle = synthesize_fallback_cue(engine, SoundCueKind::Footstep)?;
            Ok((chosen_idx, handle))
        }
    }

    /// Plays a flashlight toggle click (`FlashlightOn` when `turned_on == true`, `FlashlightOff` when `false`).
    pub fn play_flashlight_toggle(
        &self,
        engine: &mut AudioEngine,
        turned_on: bool,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.play(
            engine,
            if turned_on {
                SoundCueKind::FlashlightOn
            } else {
                SoundCueKind::FlashlightOff
            },
        )
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
            SoundCueKind::SavePoint => self.save_sound.as_ref(),
            SoundCueKind::FlashlightOn => self.flashlight_on.as_ref(),
            SoundCueKind::FlashlightOff => self.flashlight_off.as_ref(),
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
        SoundCueKind::SavePoint => (520.0, 780.0, 320, false),
        SoundCueKind::FlashlightOn => (820.0, 1100.0, 35, false),
        SoundCueKind::FlashlightOff => (980.0, 680.0, 35, true),
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
