//! BGM constants, fade tables, track state types, and event flags (`SeBgmManager`).

/// Number of stereo tracks managed in lockstep by `SeBgmManager` (`0x1F837D8`, 7 tracks).
pub const BGM_MAX_TRACKS: usize = 7;

/// Maximum number of overlapping `_SOUND_DATA` bounding boxes accumulated per frame (`w_p->get_num <= 8`).
pub const MAX_MATCHED_SOUND_AREAS: usize = 8;

/// Horizontal player half-width margin (`w_p->rad = 50.0`, `0x42480000`) used in `sndGetSoundAryByCharaPos`.
pub const SOUND_AREA_HALF_WIDTH: f32 = 50.0;

/// Squared XZ distance threshold (`6000.0^2 = 36,000,000.0` at `0x6C5F5C`) for enemy proximity situation `3`.
pub const ENEMY_NEAR_DIST_SQ: f32 = 36_000_000.0;

/// Player health ratio threshold (`0.20` at `0x63345C`) for low-health situation `2`.
pub const LOW_HEALTH_RATIO_THRESHOLD: f32 = 0.20;

/// Fade priority table (`fade_prio[5]` at `0x8A7750` in `sh2pc.exe`):
/// index = fade type (`0` = off, `1` = 3.0s, `2` = 1.5s, `3` = 6.0s, `4` = instant),
/// value = priority (`0` lowest .. `4` highest).
pub const FADE_PRIO: [i32; 5] = [0, 2, 3, 1, 4];

/// Fade duration table in seconds (`track_fade_time[5]` at `0x002b8860` in `SLUS_202.28`, `0x8A7764` in `sh2pc.exe`):
/// - Type `0`: `0.0s` (inactive)
/// - Type `1`: `3.0s` (normal fade)
/// - Type `2`: `1.5s` (fast fade)
/// - Type `3`: `6.0s` (slow fade)
/// - Type `4`: `0.0s` (instant step)
pub const FADE_TIME_SECS: [f32; 5] = [0.0, 3.0, 1.5, 6.0, 0.0];

/// Situation preset table for `situation_page` `6..=15` (`0x8A9EB8` in `sh2pc.exe`).
/// Each entry specifies `(fade_in[7], fade_out[7])`.
pub const SITUATION_PRESETS_6_TO_15: [([u8; 7], [u8; 7]); 10] = [
    ([2, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0]), // Page  6: Track 0
    ([2, 2, 0, 0, 0, 0, 0], [1, 1, 0, 0, 0, 0, 0]), // Page  7: Tracks 0, 1
    ([2, 0, 2, 0, 0, 0, 0], [1, 0, 1, 0, 0, 0, 0]), // Page  8: Tracks 0, 2
    ([2, 2, 2, 0, 0, 0, 0], [1, 1, 1, 0, 0, 0, 0]), // Page  9: Tracks 0, 1, 2
    ([2, 0, 0, 2, 0, 0, 0], [1, 0, 0, 1, 0, 0, 0]), // Page 10: Tracks 0, 3
    ([2, 2, 0, 2, 0, 0, 0], [1, 1, 0, 1, 0, 0, 0]), // Page 11: Tracks 0, 1, 3
    ([2, 0, 2, 2, 0, 0, 0], [1, 0, 1, 1, 0, 0, 0]), // Page 12: Tracks 0, 2, 3
    ([2, 2, 2, 2, 0, 0, 0], [1, 1, 1, 1, 0, 0, 0]), // Page 13: Tracks 0, 1, 2, 3
    ([0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0, 0]), // Page 14: All tracks fade out
    ([1, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0]), // Page 15: Track 0 (3s fade-in)
];

/// Track fade state machine (`Track.stat` at `0x1F837D8 + i*8` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BgmTrackStatus {
    /// Track is silent (`vol == 0.0`).
    #[default]
    Off = 0,
    /// Track is ramping up toward `1.0` at `FADE_TIME_SECS[fade_in]`.
    FadingIn = 1,
    /// Track is at full volume (`vol == 1.0`).
    On = 2,
    /// Track is ramping down toward `0.0` at `FADE_TIME_SECS[fade_out]`.
    FadingOut = 3,
}

/// State of a single stereo BGM layer (`8` bytes in `sh2pc.exe` at `0x1F837D8 + i*8`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BgmTrackState {
    /// Current state (`Off`, `FadingIn`, `On`, `FadingOut`).
    pub status: BgmTrackStatus,
    /// Fade-in speed type (`1..=4`).
    pub fade_in: u8,
    /// Fade-out speed type (`1..=4`).
    pub fade_out: u8,
    /// Normalized track volume (`0.0..=1.0`).
    pub volume: f32,
}

impl Default for BgmTrackState {
    fn default() -> Self {
        Self {
            status: BgmTrackStatus::Off,
            fade_in: 0,
            fade_out: 0,
            volume: 0.0,
        }
    }
}

/// Result of evaluating `_SOUND_DATA` spatial volume areas via [`super::evaluate_sound_areas`] (`sndGetSoundAryByCharaPos`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundAreaEvaluation {
    /// Number of matched `_SOUND_DATA` boxes (`1..=8`), or `1` when using default record `0` or situation preset.
    pub matched_box_count: usize,
    /// Stack-allocated buffer of matched `_SOUND_DATA` record indices within the stage table
    /// (`matched_record_indices[..matched_record_count]`; empty when situation preset `>= 6`, `SH2-179`).
    pub matched_record_indices: [usize; MAX_MATCHED_SOUND_AREAS],
    /// Number of valid entries in [`Self::matched_record_indices`] (`0..=MAX_MATCHED_SOUND_AREAS`).
    pub matched_record_count: usize,
    /// Whether no area bounding box (`1..N`) matched and record `0` (stage default) was used.
    pub used_default_record: bool,
    /// Resolved highest-priority `(fade_in, fade_out)` command per track (`0` = track should be off).
    pub track_fades: [(u8, u8); BGM_MAX_TRACKS],
}

impl SoundAreaEvaluation {
    /// Returns the active slice of matched `_SOUND_DATA` record indices (`0..=MAX_MATCHED_SOUND_AREAS`).
    #[inline]
    #[must_use]
    pub fn matched_records(&self) -> &[usize] {
        &self.matched_record_indices[..self.matched_record_count.min(MAX_MATCHED_SOUND_AREAS)]
    }
}

/// Event/story flags consulted by `SeBgmChange` (`0x515C20`) and stage `EvBgmControl` functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BgmEventFlags {
    /// `0x1F7D5E4` byte flags (stage 2/3/4 prologue & forest progression).
    pub flag_5e4: u8,
    /// `0x1F7D5E8` dword flags (apartments clock/courtyard/stairwell progression).
    pub flag_5e8: u32,
    /// `0x1F7D5EC` dword flags (Rosewater Park / Maria meeting progression).
    pub flag_5ec: u32,
    /// `0x1F7D5F0` dword flags (Town West / Heaven's Night / Bowling progression).
    pub flag_5f0: u32,
    /// `0x1F7D608` dword flags (Labyrinth / Catacombs progression).
    pub flag_608: u32,
    /// `0x1F7D60C` dword flags (Eddie boss progression).
    pub flag_60c: u32,
    /// `0x1F7D60E` byte flags (Labyrinth Angela/Abstract Daddy progression).
    pub flag_60e: u8,
    /// `0x1F7D618` dword flags (Hotel 1F music box Cinderella/Snow White/Little Mermaid).
    pub flag_618: u32,
    /// `0x1F7D61B` byte flags (Hotel 1F music box completed / staircase).
    pub flag_61b: u8,
    /// `0x1F7D61C` dword flags (Hotel Basement / 1F Dark / Final Boss progression).
    pub flag_61c: u32,
    /// `0x1F7D620` dword flags (Hotel Room 312 tape watched `0x8000` / radio / general triggers).
    pub flag_620: u32,
    /// `0x1F7D622` byte flags (Hotel Room 1F corridor trigger).
    pub flag_622: u8,
    /// `0x1F7D62C` dword flags (global BGM mute/cutscene inhibit flags `0xC00`, `0x800`).
    pub flag_62c: u32,
    /// Whether the Hotel 1F lobby music box is currently open/active (`0x1D85FA4`).
    pub hotel_music_box_active: bool,
    /// Whether an active cutscene/event flag (`0x934D94 & 0x40`) is asserted.
    pub event_mode_active: bool,
    /// Active `DramaDemo` number (`DramaDemoGetNo()` `0x004a02b0`, e.g. `Some(3)` for `haka_agl`
    /// Angela Graveyard cutscene in Stage 4 `EvBgmControl` `0x005a0260`).
    pub active_demo_no: Option<u32>,
    /// Optional explicit override for `EvBgmControl` return value.
    pub ev_bgm_override: Option<u32>,
    /// Whether SH2:EE restored cut/unused audio (`MusicBoxVolume.cpp` 7th-layer Hotel 1F/2F music box attenuation) is enabled (`SH2-62`).
    pub enable_restored_cut_audio: bool,
    /// Retail `sh2pc.exe` Bowling BGM start delay flag (`DAT_01f80bd0`, set in `SeBgmChange`
    /// `0x00516028:4512-4515` for stage 15 when `(flag_5f0 & 0x8000_0000) == 0`, checked in
    /// `FUN_00516430` `0x00516430:4666` before starting `pending_load_id`).
    pub bowling_bgm_delay: bool,
    /// Retail `sh2pc.exe` active game mode (`DAT_00932178`, checked in `FUN_00516430` `0x00516430:4757`:
    /// `game_mode == 0x0b` forces situation page `15`).
    pub game_mode: u8,
}

impl BgmEventFlags {
    /// Extracts [`BgmEventFlags`] from the 2048-byte `flag_work` / `game_flag` bitset
    /// (`sh2pc.exe` `0x01f7a9e0`, `SLUS_202.28` `0x01126430`) used by `SeBgmChange` (`0x00515fd0`)
    /// and stage `EvBgmControl` routines.
    ///
    /// Byte offsets relative to `flag_work` base (`0x01f7a9e0` / `0x01126430`):
    /// - `+0x04` -> `flag_5e4` (`u8`)
    /// - `+0x08` -> `flag_5e8` (`u32`)
    /// - `+0x0C` -> `flag_5ec` (`u32`)
    /// - `+0x10` -> `flag_5f0` (`u32`)
    /// - `+0x28` -> `flag_608` (`u32`)
    /// - `+0x2C` -> `flag_60c` (`u32`)
    /// - `+0x2E` -> `flag_60e` (`u8`)
    /// - `+0x38` -> `flag_618` (`u32`)
    /// - `+0x3B` -> `flag_61b` (`u8`)
    /// - `+0x3C` -> `flag_61c` (`u32`)
    /// - `+0x40` -> `flag_620` (`u32`)
    /// - `+0x42` -> `flag_622` (`u8`)
    /// - `+0x4C` -> `flag_62c` (`u32`)
    pub fn from_event_flag_bytes(
        bytes: &[u8],
        hotel_music_box_active: bool,
        event_mode_active: bool,
        enable_restored_cut_audio: bool,
    ) -> Self {
        let read_u8 = |off: usize| -> u8 { bytes.get(off).copied().unwrap_or(0) };
        let read_u32 = |off: usize| -> u32 {
            bytes
                .get(off..off + 4)
                .and_then(|s| s.try_into().ok())
                .map(u32::from_le_bytes)
                .unwrap_or(0)
        };
        Self {
            flag_5e4: read_u8(0x04),
            flag_5e8: read_u32(0x08),
            flag_5ec: read_u32(0x0C),
            flag_5f0: read_u32(0x10),
            flag_608: read_u32(0x28),
            flag_60c: read_u32(0x2C),
            flag_60e: read_u8(0x2E),
            flag_618: read_u32(0x38),
            flag_61b: read_u8(0x3B),
            flag_61c: read_u32(0x3C),
            flag_620: read_u32(0x40),
            flag_622: read_u8(0x42),
            flag_62c: read_u32(0x4C),
            hotel_music_box_active,
            event_mode_active,
            active_demo_no: None,
            ev_bgm_override: None,
            enable_restored_cut_audio,
            bowling_bgm_delay: false,
            game_mode: 0,
        }
    }
}
