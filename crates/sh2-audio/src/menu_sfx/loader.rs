//! Asset loading and sound-ID resolution (`sddata.bin`, `voice.afs`, `sh2e/sound/extra/*.wav`)
//! for [`MenuAudioBank`] (`SH2-61`, `SH2-62`, `SH2-167`).

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use sh2_assets::formats::sound::{
    decode_afs_slice_to_wav, parse_afs_index, parse_sddata, WavEntry,
};
use sh2_core::AudioPackPreference;

use super::{MenuAudioBank, SfxSourceRef, CORE_VOICE_AFS_INDICES, VOICE_AFS_LRU_CAPACITY};
use crate::dsp_resample::{prepare_wav_entry_48k, PreparedSfx};
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
            voice_afs_index: None,
            data_root: Some(data_root.to_path_buf()),
            ee_root: ee_root.map(|p| p.to_path_buf()),
            entries: Vec::new(),
            prepared_sddata: Vec::new(),
            voice_afs_sfx: HashMap::new(),
            voice_afs_lru: VecDeque::new(),
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
            active_looping_2d_handles: HashMap::new(),
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
                            sfx_bank.footstep = Some(SfxSourceRef::SddataIndex(147));
                            sfx_bank.cancel = Some(SfxSourceRef::SddataIndex(291));
                            sfx_bank.flashlight_off = Some(SfxSourceRef::SddataIndex(292));
                            sfx_bank.cursor_move = Some(SfxSourceRef::SddataIndex(293));
                            sfx_bank.flashlight_on = Some(SfxSourceRef::SddataIndex(296));
                            sfx_bank.confirm = Some(SfxSourceRef::SddataIndex(297));
                            sfx_bank.prepared_sddata =
                                (0..bank.entries.len()).map(|_| OnceLock::new()).collect();
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
                            // Preload key streamed SFX entries (`CORE_VOICE_AFS_INDICES`)
                            for &entry_idx in &CORE_VOICE_AFS_INDICES {
                                if let Some(desc) = afs_idx.get(entry_idx) {
                                    if desc.length > 0 && desc.length <= 16 * 1024 * 1024 {
                                        let mut slice = vec![0u8; desc.length as usize];
                                        if file.seek(SeekFrom::Start(desc.offset as u64)).is_ok()
                                            && file.read_exact(&mut slice).is_ok()
                                        {
                                            if let Ok(wav) =
                                                decode_afs_slice_to_wav(&slice, entry_idx as u32)
                                            {
                                                sfx_bank.insert_voice_afs_entry(
                                                    entry_idx,
                                                    Arc::new(wav),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            sfx_bank.voice_afs_index = Some(afs_idx);
                            if sfx_bank.game_start.is_none()
                                && sfx_bank.voice_afs_sfx.contains_key(&24)
                            {
                                sfx_bank.game_start = Some(SfxSourceRef::VoiceAfsIndex(24));
                            }
                            if sfx_bank.save_sound.is_none()
                                && sfx_bank.voice_afs_sfx.contains_key(&35)
                            {
                                sfx_bank.save_sound = Some(SfxSourceRef::VoiceAfsIndex(35));
                            }
                            break;
                        }
                    }
                }
            }
        }

        // 3. Load SH2:EE standalone extra WAV overrides (`sh2e/sound/extra/*.wav`) when preferring SH2:EE (or as fallback)
        Self::load_extra_source_ref_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/g_start.wav",
            &mut sfx_bank.game_start,
        );
        Self::load_extra_source_ref_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/save_sound.wav",
            &mut sfx_bank.save_sound,
        );
        Self::load_extra_source_ref_override(
            data_root,
            ee_root,
            prefer_ee,
            "sound/extra/flashlight_on.wav",
            &mut sfx_bank.flashlight_on,
        );
        Self::load_extra_source_ref_override(
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

    pub(super) fn load_extra_source_ref_override(
        data_root: &Path,
        ee_root: Option<&Path>,
        prefer_ee: bool,
        rel_path: &str,
        slot: &mut Option<SfxSourceRef>,
    ) {
        if !prefer_ee && slot.is_some() {
            return;
        }
        let mut wav_slot = None;
        Self::load_extra_wav_override(data_root, ee_root, prefer_ee, rel_path, &mut wav_slot);
        if let Some(entry) = wav_slot {
            *slot = Some(SfxSourceRef::extra_wav(entry));
        }
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
            .or_else(|| {
                self.cursor_move
                    .as_ref()
                    .and_then(|src| self.resolve_source_ref(src))
            })
            .map(|e| e.sample_rate)
            .unwrap_or(0)
    }

    /// Looks up a `WavEntry` by its 0-based `sddata.bin` index (`0..417`).
    #[inline]
    pub fn get_sddata_entry(&self, index: usize) -> Option<&WavEntry> {
        self.entries.get(index)
    }

    /// Resolves a [`SfxSourceRef`] to its backing [`WavEntry`] without cloning PCM bytes (`SH2-179`).
    #[inline]
    pub fn resolve_source_ref<'a>(&'a self, src: &'a SfxSourceRef) -> Option<&'a WavEntry> {
        match src {
            SfxSourceRef::SddataIndex(idx) => self.get_sddata_entry(*idx),
            SfxSourceRef::VoiceAfsIndex(idx) => self.voice_afs_sfx.get(idx).map(Arc::as_ref),
            SfxSourceRef::ExtraWav { wav, .. } => Some(wav.as_ref()),
        }
    }

    /// Lazily prepares and caches a 48 kHz [`PreparedSfx`] (`Arc<[Frame]>`) for `sddata.bin[index]`,
    /// returning a zero-copy `Arc::clone` on all subsequent triggers (`SH2-179`).
    pub fn get_or_prepare_sddata(&self, index: usize) -> Option<PreparedSfx> {
        let wav = self.entries.get(index)?;
        if let Some(cell) = self.prepared_sddata.get(index) {
            return Some(cell.get_or_init(|| prepare_wav_entry_48k(wav)).clone());
        }
        Some(prepare_wav_entry_48k(wav))
    }

    /// Lazily prepares a 48 kHz [`PreparedSfx`] from a [`SfxSourceRef`], hitting the per-index cache
    /// for `sddata.bin` and standalone extra WAV overrides (`SH2-179`).
    pub fn prepare_source_ref(&self, src: &SfxSourceRef) -> Option<PreparedSfx> {
        match src {
            SfxSourceRef::SddataIndex(idx) => self.get_or_prepare_sddata(*idx),
            SfxSourceRef::VoiceAfsIndex(idx) => self
                .voice_afs_sfx
                .get(idx)
                .map(|wav| prepare_wav_entry_48k(wav)),
            SfxSourceRef::ExtraWav { wav, prepared } => {
                Some(prepared.get_or_init(|| prepare_wav_entry_48k(wav)).clone())
            }
        }
    }

    /// Resolves any game `se_id` (`u32`, either from the 666-entry `0x8A1ED8` `sddata.bin` table or the 32-entry `0x8A1E58` `voice.afs` table).
    pub fn resolve_se_id(&self, se_id: u32) -> Option<&WavEntry> {
        match se_id {
            10000 => self
                .cursor_move
                .as_ref()
                .and_then(|s| self.resolve_source_ref(s))
                .or_else(|| self.get_sddata_entry(293)),
            10002 => self
                .confirm
                .as_ref()
                .and_then(|s| self.resolve_source_ref(s))
                .or_else(|| self.get_sddata_entry(297)),
            10003 => self
                .cancel
                .as_ref()
                .and_then(|s| self.resolve_source_ref(s))
                .or_else(|| self.get_sddata_entry(291)),
            10051 => self
                .save_sound
                .as_ref()
                .and_then(|s| self.resolve_source_ref(s))
                .or_else(|| self.voice_afs_sfx.get(&35).map(Arc::as_ref)),
            15002 => self
                .game_start
                .as_ref()
                .and_then(|s| self.resolve_source_ref(s))
                .or_else(|| self.voice_afs_sfx.get(&24).map(Arc::as_ref)),
            _ => {
                if let Some(sd_idx) = resolve_se_id_to_sddata_index(se_id) {
                    if let Some(wav) = self.get_sddata_entry(sd_idx) {
                        return Some(wav);
                    }
                }
                if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
                    if let Some(wav) = self.voice_afs_sfx.get(&afs_idx) {
                        return Some(wav.as_ref());
                    }
                }
                None
            }
        }
    }

    /// Resolves any game `se_id` into a lazily cached 48 kHz [`PreparedSfx`] (`Arc<[Frame]>`) for
    /// zero-copy playback (`SH2-179`).
    pub fn resolve_prepared_se_id(&self, se_id: u32) -> Option<PreparedSfx> {
        match se_id {
            10000 => self
                .cursor_move
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
                .or_else(|| self.get_or_prepare_sddata(293)),
            10002 => self
                .confirm
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
                .or_else(|| self.get_or_prepare_sddata(297)),
            10003 => self
                .cancel
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
                .or_else(|| self.get_or_prepare_sddata(291)),
            10051 => self
                .save_sound
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
                .or_else(|| {
                    self.voice_afs_sfx
                        .get(&35)
                        .map(|w| prepare_wav_entry_48k(w))
                }),
            15002 => self
                .game_start
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
                .or_else(|| {
                    self.voice_afs_sfx
                        .get(&24)
                        .map(|w| prepare_wav_entry_48k(w))
                }),
            _ => {
                if let Some(sd_idx) = resolve_se_id_to_sddata_index(se_id) {
                    if let Some(prepared) = self.get_or_prepare_sddata(sd_idx) {
                        return Some(prepared);
                    }
                }
                if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
                    if let Some(wav) = self.voice_afs_sfx.get(&afs_idx) {
                        return Some(prepare_wav_entry_48k(wav));
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

    /// Moves `entry_index` to the most-recently-used position in [`Self::voice_afs_lru`].
    fn touch_voice_afs_lru(&mut self, entry_index: usize) {
        if let Some(pos) = self
            .voice_afs_lru
            .iter()
            .position(|&idx| idx == entry_index)
        {
            self.voice_afs_lru.remove(pos);
        }
        self.voice_afs_lru.push_back(entry_index);
    }

    /// Inserts a decoded `voice.afs` entry into [`Self::voice_afs_sfx`], enforcing
    /// [`VOICE_AFS_LRU_CAPACITY`] by evicting the least-recently-used non-core entry (`SH2-176`).
    pub fn insert_voice_afs_entry(&mut self, entry_index: usize, wav: Arc<WavEntry>) {
        self.voice_afs_sfx.insert(entry_index, wav);
        self.touch_voice_afs_lru(entry_index);
        while self.voice_afs_sfx.len() > VOICE_AFS_LRU_CAPACITY {
            let evict_pos = self
                .voice_afs_lru
                .iter()
                .position(|idx| !CORE_VOICE_AFS_INDICES.contains(idx))
                .unwrap_or(0);
            if let Some(evicted_idx) = self.voice_afs_lru.remove(evict_pos) {
                self.voice_afs_sfx.remove(&evicted_idx);
            } else {
                break;
            }
        }
    }

    /// Evicts all non-core cached `voice.afs` entries (retaining only [`CORE_VOICE_AFS_INDICES`])
    /// to reclaim multi-MB decoded voice/SFX buffers on room or stage transitions (`SH2-176`).
    pub fn clear_non_core_voice_afs_cache(&mut self) {
        self.voice_afs_sfx
            .retain(|idx, _| CORE_VOICE_AFS_INDICES.contains(idx));
        self.voice_afs_lru
            .retain(|idx| CORE_VOICE_AFS_INDICES.contains(idx));
    }

    /// Lazily loads and caches a single entry (`0..=158`) from `voice.afs` on disk (`SH2-62`, `SH2-176`).
    ///
    /// Reuses the cached [`sh2_assets::formats::sound::AfsArchiveIndex`] and returns `Arc<WavEntry>`
    /// so repeated lookups avoid both re-parsing the AFS header and cloning multi-MB PCM buffers.
    pub fn load_voice_afs_entry(&mut self, entry_index: usize) -> Option<Arc<WavEntry>> {
        if let Some(cached) = self.voice_afs_sfx.get(&entry_index).cloned() {
            self.touch_voice_afs_lru(entry_index);
            return Some(cached);
        }
        let afs_path = self.voice_afs_path.as_ref()?.clone();
        let mut file = File::open(&afs_path).ok()?;
        if self.voice_afs_index.is_none() {
            let mut header_buf = vec![0u8; 2048];
            let n = file.read(&mut header_buf).ok()?;
            header_buf.truncate(n);
            self.voice_afs_index = Some(parse_afs_index(&header_buf).ok()?);
        }
        let desc = *self.voice_afs_index.as_ref()?.get(entry_index)?;
        if desc.length == 0 || desc.length > 32 * 1024 * 1024 {
            return None;
        }
        let mut slice = vec![0u8; desc.length as usize];
        file.seek(SeekFrom::Start(desc.offset as u64)).ok()?;
        file.read_exact(&mut slice).ok()?;
        let wav = Arc::new(decode_afs_slice_to_wav(&slice, entry_index as u32).ok()?);
        self.insert_voice_afs_entry(entry_index, Arc::clone(&wav));
        Some(wav)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_wav(id: u32) -> WavEntry {
        WavEntry {
            id,
            channels: 1,
            sample_rate: 22050,
            bits_per_sample: 16,
            loops: false,
            loop_start: 0,
            loop_end: 0,
            data: vec![0u8; 64],
        }
    }

    fn empty_bank() -> MenuAudioBank {
        MenuAudioBank {
            loaded_source: AudioPackPreference::Auto,
            sddata_path: None,
            voice_afs_path: None,
            voice_afs_index: None,
            data_root: None,
            ee_root: None,
            entries: Vec::new(),
            prepared_sddata: Vec::new(),
            voice_afs_sfx: HashMap::new(),
            voice_afs_lru: VecDeque::new(),
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
            active_looping_2d_handles: HashMap::new(),
            active_spatial_handles: HashMap::new(),
            active_spatial_voices: HashMap::new(),
        }
    }

    #[test]
    fn voice_afs_cache_enforces_lru_cap_preserves_core_and_reuses_arc() {
        let mut bank = empty_bank();
        // Preload core entries
        for &core_idx in &CORE_VOICE_AFS_INDICES {
            bank.insert_voice_afs_entry(core_idx, Arc::new(dummy_wav(core_idx as u32)));
        }

        // Fill beyond VOICE_AFS_LRU_CAPACITY with non-core entries (100..120)
        for idx in 100..120 {
            bank.insert_voice_afs_entry(idx, Arc::new(dummy_wav(idx as u32)));
            assert!(
                bank.voice_afs_sfx.len() <= VOICE_AFS_LRU_CAPACITY,
                "voice_afs_sfx exceeded LRU cap {} (got {})",
                VOICE_AFS_LRU_CAPACITY,
                bank.voice_afs_sfx.len()
            );
        }

        // Core entries must never be evicted while non-core entries exist
        for &core_idx in &CORE_VOICE_AFS_INDICES {
            assert!(
                bank.voice_afs_sfx.contains_key(&core_idx),
                "core voice.afs entry {core_idx} was unexpectedly evicted"
            );
        }

        // Most recent non-core entries remain, oldest (100) was evicted
        assert!(!bank.voice_afs_sfx.contains_key(&100));
        assert!(bank.voice_afs_sfx.contains_key(&119));

        // Repeated load_voice_afs_entry returns the exact same Arc allocation (no PCM clone)
        let first = bank.load_voice_afs_entry(119).expect("cached entry 119");
        let second = bank.load_voice_afs_entry(119).expect("cached entry 119");
        assert!(Arc::ptr_eq(&first, &second));

        // Stage transition clears non-core entries while retaining core entries
        bank.clear_non_core_voice_afs_cache();
        assert_eq!(bank.voice_afs_sfx.len(), CORE_VOICE_AFS_INDICES.len());
        assert!(!bank.voice_afs_sfx.contains_key(&119));
        for &core_idx in &CORE_VOICE_AFS_INDICES {
            assert!(bank.voice_afs_sfx.contains_key(&core_idx));
        }
    }

    #[test]
    fn prepared_sddata_cache_reuses_arc_and_menu_cues_use_index_refs() {
        let mut bank = empty_bank();
        bank.entries = (0..300).map(|i| dummy_wav(i as u32)).collect();
        bank.prepared_sddata = (0..bank.entries.len()).map(|_| OnceLock::new()).collect();
        bank.cursor_move = Some(SfxSourceRef::SddataIndex(293));
        bank.flashlight_on = Some(SfxSourceRef::extra_wav(dummy_wav(900)));

        // Repeated sddata lookups return the exact same Arc<[Frame]> allocation (zero resampling/copy).
        let p1 = bank
            .get_or_prepare_sddata(293)
            .expect("prepared sddata[293]");
        let p2 = bank
            .resolve_prepared_se_id(10000)
            .expect("prepared se_id 10000 -> sddata[293]");
        assert_eq!(p1.sample_rate, 48_000);
        assert!(Arc::ptr_eq(&p1.frames, &p2.frames));

        // Standalone ExtraWav override also caches and reuses its prepared Arc<[Frame]>.
        let fl_ref = bank.flashlight_on.as_ref().expect("flashlight_on");
        let fl1 = bank.prepare_source_ref(fl_ref).expect("fl1");
        let fl2 = bank.prepare_source_ref(fl_ref).expect("fl2");
        assert!(Arc::ptr_eq(&fl1.frames, &fl2.frames));
    }
}
