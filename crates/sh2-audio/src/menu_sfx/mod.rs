//! Authentic sound effect loading and playback for `sddata.bin`, `voice.afs`, and `sh2e/sound/extra/*.wav` (`SH2-61`).
//!
//! Decomposed into focused submodules:
//! - [`loader`]: `sddata.bin`, `voice.afs`, and `sh2e/sound/extra/*.wav` loading and ID resolution.
//! - [`spatial`]: 2D panned and 3D `.cld`-occluded / Binaural HRTF tracked voice playback and updates.
//! - [`playback`]: 2D SFX, surface-material footsteps, flashlight clicks, and cut/unused audio previews.

mod loader;
mod playback;
mod spatial;

use crate::spatial_voice::SpatialVoiceHandle;
use crate::{AudioEngine, AudioError};
use kira::sound::static_sound::StaticSoundHandle;
use sh2_assets::formats::sound::{AfsArchiveIndex, WavEntry};
use sh2_core::AudioPackPreference;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

/// Maximum number of decoded `voice.afs` entries retained simultaneously in
/// [`MenuAudioBank::voice_afs_sfx`] (`SH2-176`).
///
/// Covers the 8 core preloaded streamed SFX entries ([`CORE_VOICE_AFS_INDICES`]) plus up to 8
/// recently accessed stage/cutscene entries before least-recently-used non-core entries are evicted.
pub const VOICE_AFS_LRU_CAPACITY: usize = 16;

/// Core streamed SFX indices in `voice.afs` preloaded at bank initialization and preserved across
/// [`MenuAudioBank::clear_non_core_voice_afs_cache`] (`SH2-176`):
/// `17` (chainsaw), `24` (`g_start`), `27` (Prison South Ritual), `35` (`save_sound`),
/// `36` (menu transition), `38` (Prison `0x68`), `42` (Society `0x5c`), `44` (TV static).
pub const CORE_VOICE_AFS_INDICES: [usize; 8] = [17, 24, 27, 35, 36, 38, 42, 44];

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
    /// Authentic footstep loading sound (`sddata[147]` / `11001` Concrete footstep).
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
    /// Cached parsed directory header of `voice.afs` so on-demand lookups never re-read/re-parse the 2 KB header (`SH2-176`).
    pub voice_afs_index: Option<AfsArchiveIndex>,
    /// Resolved `data/` directory path for on-demand extra WAV loading (`SH2-62`).
    pub data_root: Option<PathBuf>,
    /// Resolved `sh2e/` directory path for on-demand extra WAV loading (`SH2-62`).
    pub ee_root: Option<PathBuf>,
    /// All 417 RIFF WAVE entries extracted from `sddata.bin` (`0..=416`).
    pub entries: Vec<WavEntry>,
    /// Bounded LRU cache of decoded `voice.afs` entries (`Arc<WavEntry>` avoids cloning multi-MB PCM payloads; `SH2-176`).
    pub voice_afs_sfx: HashMap<usize, Arc<WavEntry>>,
    /// Recency order for [`Self::voice_afs_sfx`] (front = least recently used, back = most recently used).
    pub(super) voice_afs_lru: VecDeque<usize>,
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
    /// Restored Apartment Closet cutscene bonus audio (`sh2e/sound/extra/sankaku_toujyou_2.wav`) (`SH2-62`).
    pub closet_bonus_audio: Option<WavEntry>,
    /// Restored Lying Figure street footsteps (`sh2e/sound/extra/machi.wav`) (`SH2-62`).
    pub machi_steps_audio: Option<WavEntry>,
    /// Default footstep (`sddata[147]` / `11001`).
    pub footstep: Option<WavEntry>,
    /// Alternating left/right footstep parity toggle so consecutive steps alternate between `(left, right)` samples.
    pub step_parity: bool,
    /// Active footstep sound handle (`0x518270`: repeating a footstep stops the previous footstep tail so steps never stack).
    pub last_footstep_handle: Option<StaticSoundHandle>,
    /// Active voiceover/cut-audio audition handle so starting a new slice cleanly fades out the previous slice (`SH2-62`).
    pub active_cut_audio_handle: Option<StaticSoundHandle>,
    /// Active 3D spatial sound handles keyed by `se_id` (`0x515910` `SePosMove` / `0x515740` `SePosSet`).
    pub active_spatial_handles: HashMap<u32, StaticSoundHandle>,
    /// Active 3D `.cld`-occluded & HRTF/PS2 spatial voice handles keyed by `se_id` (`SH2-84`).
    pub active_spatial_voices: HashMap<u32, SpatialVoiceHandle>,
}

impl Drop for MenuAudioBank {
    fn drop(&mut self) {
        // Ensure any looped 3D voices (`SpatialVoiceControl`) or cut-audio previews stop cleanly
        // when `MenuAudioBank` is replaced during live audio-pack hot-swapping (`SH2-84`).
        self.stop_all_spatial_handles();
        self.stop_active_cut_audio();
    }
}

/// Fallback synthetic waveform generator when retail asset files are absent.
pub(super) fn synthesize_fallback_cue(
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
