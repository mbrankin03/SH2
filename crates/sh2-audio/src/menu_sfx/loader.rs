//! Asset loading and sound-ID resolution (`sddata.bin`, `voice.afs`, `sh2e/sound/extra/*.wav`)
//! for [`MenuAudioBank`] (`SH2-61`, `SH2-62`, `SH2-167`).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sh2_assets::formats::sound::{
    decode_afs_slice_to_wav, parse_afs_index, parse_sddata, WavEntry,
};
use sh2_core::AudioPackPreference;

use super::MenuAudioBank;
use crate::sfx_bank::{resolve_se_id_to_sddata_index, resolve_se_id_to_voice_afs_index};

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
            data_root: Some(data_root.to_path_buf()),
            ee_root: ee_root.map(|p| p.to_path_buf()),
            entries: Vec::new(),
            voice_afs_sfx: HashMap::new(),
            cursor_move: None,
            confirm: None,
            cancel: None,
            game_start: None,
            save_sound: None,
            flashlight_on: None,
            flashlight_off: None,
            closet_bonus_audio: None,
            machi_steps_audio: None,
            footstep: None,
            step_parity: false,
            last_footstep_handle: None,
            active_cut_audio_handle: None,
            active_spatial_handles: HashMap::new(),
            active_spatial_voices: HashMap::new(),
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
                            sfx_bank.footstep = Some(bank.entries[147].clone());
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
                            // Preload key streamed SFX entries (`17` = chainsaw afs, `24` = g_start, `27` = 0x4682 Prison South Ritual,
                            // `35` = save_sound, `36` = menu transition, `38` = 0x427B Prison 0x68, `42` = 0x4982 Society 0x5c, `44` = 0x3E80 TV static)
                            for &entry_idx in &[17usize, 24, 27, 35, 36, 38, 42, 44] {
                                if let Some(desc) = afs_idx.get(entry_idx) {
                                    if desc.length > 0 && desc.length <= 16 * 1024 * 1024 {
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
        Self::load_extra_wav_override(
            data_root,
            ee_root,
            true,
            "sound/extra/sankaku_toujyou_2.wav",
            &mut sfx_bank.closet_bonus_audio,
        );
        Self::load_extra_wav_override(
            data_root,
            ee_root,
            true,
            "sound/extra/machi.wav",
            &mut sfx_bank.machi_steps_audio,
        );

        sfx_bank
    }

    pub(super) fn load_extra_wav_override(
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

    /// Returns the 4 looping Pocket Radio static waveforms from `sddata.bin[360..=363]`
    /// (`sh2pc.exe` `FUN_00518680` `0x00518680` / `DAT_008a67cc..0x008a67dc`, `SH2-167`).
    pub fn radio_static_wavs(&self) -> Option<[&WavEntry; 4]> {
        let [i0, i1, i2, i3] = crate::engine::RADIO_STATIC_SDDATA_INDICES;
        Some([
            self.entries.get(i0)?,
            self.entries.get(i1)?,
            self.entries.get(i2)?,
            self.entries.get(i3)?,
        ])
    }

    /// Lazily loads and caches a single entry (`0..=158`) from `voice.afs` on disk (`SH2-62`).
    pub fn load_voice_afs_entry(&mut self, entry_index: usize) -> Option<WavEntry> {
        if let Some(cached) = self.voice_afs_sfx.get(&entry_index) {
            return Some(cached.clone());
        }
        let afs_path = self.voice_afs_path.as_ref()?;
        let mut file = File::open(afs_path).ok()?;
        let mut header_buf = vec![0u8; 2048];
        let n = file.read(&mut header_buf).ok()?;
        header_buf.truncate(n);
        let afs_idx = parse_afs_index(&header_buf).ok()?;
        let desc = afs_idx.get(entry_index)?;
        if desc.length == 0 || desc.length > 32 * 1024 * 1024 {
            return None;
        }
        let mut slice = vec![0u8; desc.length as usize];
        file.seek(SeekFrom::Start(desc.offset as u64)).ok()?;
        file.read_exact(&mut slice).ok()?;
        let wav = decode_afs_slice_to_wav(&slice, entry_index as u32).ok()?;
        self.voice_afs_sfx.insert(entry_index, wav.clone());
        Some(wav)
    }
}
