//! Catalog and restoration engine for Silent Hill 2 cut/unused retail audio, SH2:EE restored cues,
//! and PS2 Prototype (`July 13, 2001 SLUS_202.28` & `E3 May 1, 2001 SLPM_123.45`) soundbanks (`SH2-62`).
//!
//! # Ground Truth & Binary References
//! 1. **Unreferenced `sddata.bin` Waveforms (`sh2pc.exe` `0x8A67E0` vs `0x8A1ED8`)**:
//!    - Out of 417 RIFF WAVE entries (`0..=416`) and 699 internal driver IDs (`5000..=5698`),
//!      **5 looping radio static waveforms (`sddata[360..=364]`, internal IDs `5360..=5364`)**
//!      are completely absent from the 666-entry `SE_PAIRS` table (`0x8A1ED8`) and unreachable in retail.
//!    - **27 rare 3D environmental/secret waveforms (`sddata[367..=393]`)** are omitted from standard
//!      `10000..=27999` 2D SE calls and only reachable via `0x9C40..=0x9D59` (`40000+`) 3D positional calls.
//! 2. **Unused Hotel Restaurant Voiceover (`PlayUnusedAudio.cpp` — `voice.afs` Entry `119` / `SCENE_45`)**:
//!    - In retail `data/sound/adx/voice/voice.afs` and `sh2e/sound/adx/voice/voice.afs`, AFS entry `119`
//!      (driver ID `0xEAD7` = `0xEA60 + 119`, `SCENE_45`, `67.63s`) is Mary's full voiceover reading her letter to Laura
//!      during `CS_HTL_LAURA_PIANO` (`0x3B`) in `R_HTL_RESTAURANT` (`0x9E`), which was muted/untriggered in vanilla.
//!    - Sliced into **11 timed segments** (`seqStart[11]` / `seqEnd[11]` in `PlayUnusedAudio.cpp` spanning `0.0s..67.2s`)
//!      synced to the 11 letter text pages once `cutscene_timer >= 1600.0` and `fade_state == 3`.
//!    - By contrast, when James arrives at Lakeview Hotel by rowboat (`CS_LAKE_BOAT_ARRIVE` = `0x4D` in `stage_lake`),
//!      his subtitle in `stage_lake_msg_e.mes[0]` (*"This place hasn't changed at all in three years..."*) is **text-only**:
//!      `voice.afs[134]` (`bort_2`, `0xEAE6`, `64.67s`) contains only water lapping, oar strokes, and dock creaks,
//!      and `data/demo/boat2/hhh_jms.cls` (`96` bytes) has an empty mouth lip-sync track (`10` bytes vs `14,488` bytes in `boat1`).
//! 3. **Cutscene & Gameplay Bonus SFX (`PlayWavSound.cpp`)**:
//!    - `sh2e/sound/extra/sankaku_toujyou_2.wav`: Apartment Closet cutscene (`CS_APT_RPT_CLOSET`) bonus audio (`timer > 1900.0`).
//!    - `sh2e/sound/extra/machi.wav`: Lying Figure street footsteps during `CS_HSP_BOSS_FINISH` in `R_TOWN_EAST`.
//! 4. **PS2 Prototype Sound Architecture (`SLUS_202.28` July 13, 2001 & `SLPM_123.45` E3 2001)**:
//!    - **`.sdb` (`data/sound/snd_data/*.sdb`)**: 64-byte `_SOUND_DATA` spatial BGM mixing tables (`SeSoundDataLoad` at `0x221460`).
//!      - E3 2001 `tgs_trial.sdb` is 4,800 bytes (**75 spatial records**, 7.5x larger than July 13 / retail's 10-record `tgs_trial.sdb`)
//!        and E3 `snd_data.mgf` includes **13 cut 2-letter stage `.sdb` files** (`ap.sdb`, `hp.sdb`, `rr.sdb`, `ru.sdb`, etc.).
//!    - **`.sfc` (`data/sound/s_force/*.sfc`)**: 33 S-Force 3D positional 16-bit PCM banks in `DATA/SOUND.MGF` (`se_file_list$1329` at `0x2b8570`,
//!      loaded by `SeSoundEffect3dLoad` at `0x192fd0`), including developer test bank `iketani.sfc` (index 15, named after Takayuki Iketani)
//!      and `eddie_boss.sfc` (index 27).
//!    - **`IOP/SD0712/SOUND.DAT` & `SOUNDCD.IRX`**: 79 `IECSsreV` / `IECSigaV` SPU2 ADPCM banks (`1,422` VAG samples in July 13 vs `26` banks / `269` VAGs in E3),
//!      `64` cutscene VAG streams (`0x13980`), and `252` BGM VAG stream layers (`0x13b80`).

use crate::bgm_data::SoundDataRecord;

/// AFS entry index (`119`) inside `data/sound/adx/voice/voice.afs` and `sh2e/sound/adx/voice/voice.afs`
/// containing Mary's unused voiceover reading her letter to Laura (`SCENE_45`, driver ID `0xEAD7` = `0xEA60 + 119`).
pub const VOICE_AFS_LAURA_LETTER_INDEX: usize = 119;

/// AFS entry index (`116`) inside `voice.afs` containing the standalone Room 312 VHS videotape
/// audio stream (`satsugai`, driver ID `0xEAD4` = `0xEA60 + 116`).
pub const VOICE_AFS_SATSUGAI_INDEX: usize = 116;

/// AFS entry index (`97`) inside `voice.afs` containing Mary's full ending letter monologue
/// (`mary_let_fix`, driver ID `0xEAC1` = `0xEA60 + 97`).
pub const VOICE_AFS_MARY_LETTER_FULL_INDEX: usize = 97;

/// AFS entry index (`83`) inside `voice.afs` containing the Rosewater Park Maria first meeting
/// additional dialogue stream (`m_deai_tsuika`, driver ID `0xEAB3` = `0xEA60 + 83`).
pub const VOICE_AFS_MARIA_DEAI_TSUIKA_INDEX: usize = 83;

/// AFS entry index (`140`) inside `voice.afs` containing the *Born From A Wish* additional
/// dialogue stream (`bp_mar_tsuika`, driver ID `0xEAEC` = `0xEA60 + 140`).
pub const VOICE_AFS_BORN_FROM_A_WISH_TSUIKA_INDEX: usize = 140;

/// Cutscene ID (`CS_HTL_LAURA_PIANO` = `0x3B` = `59`) for the Lakeview Hotel Restaurant piano/letter cutscene (`PlayUnusedAudio.cpp`).
pub const CS_HTL_LAURA_PIANO_ID: u32 = 0x3B;

/// Room ID (`R_HTL_RESTAURANT` = `0x9E` = `158`) for the Lakeview Hotel Restaurant (`PlayUnusedAudio.cpp`).
pub const R_HTL_RESTAURANT_ID: u32 = 0x9E;

/// Cutscene ID (`CS_APT_RPT_CLOSET` = `0x10` = `16`) for the Wood Side Apartments Room 307 Pyramid Head closet cutscene (`PlayWavSound.cpp`).
pub const CS_APT_RPT_CLOSET_ID: u32 = 0x10;

/// Cutscene ID (`CS_HSP_BOSS_FINISH` = `0x26` = `38`) for the post-Hospital Town East Lying Figure street transition (`PlayWavSound.cpp`).
pub const CS_HSP_BOSS_FINISH_ID: u32 = 0x26;

/// Category of a cataloged cut, unused, or rare retail/SH2:EE sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutSfxCategory {
    /// Completely unreachable in retail `sh2pc.exe` (absent from the 666-entry `0x8A1ED8` `SE_PAIRS` table).
    UnreferencedRetailSddata,
    /// Omitted from standard `10000..=27999` 2D SE calls; only reachable via `0x9C40..=0x9D59` (`40000+`) 3D positional calls.
    Hidden3dPositionalOnly,
    /// Unused / alternate cutscene or SFX stream inside `sound/adx/voice/voice.afs`.
    UnusedVoiceAfsStream,
    /// Standalone SH2:EE restored cutscene / gameplay WAV (`sh2e/sound/extra/*.wav`).
    RestoredExtraWav,
}

impl CutSfxCategory {
    /// Human-readable category tag for the Debug Audio Studio HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::UnreferencedRetailSddata => "UNREFERENCED RETAIL SDDATA",
            Self::Hidden3dPositionalOnly => "RARE 3D-ONLY POSITIONAL (40000+)",
            Self::UnusedVoiceAfsStream => "UNUSED VOICE.AFS STREAM",
            Self::RestoredExtraWav => "RESTORED SH2:EE EXTRA WAV",
        }
    }
}

/// Metadata descriptor for an unreferenced, hidden, or restored sound cue in `sddata.bin`, `voice.afs`, or `sh2e/sound/extra/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutSfxCatalogEntry {
    /// Category of the cut/rare audio asset.
    pub category: CutSfxCategory,
    /// 0-based `sddata.bin` entry index (`0..=416`), if stored in `sddata.bin`.
    pub sddata_index: Option<usize>,
    /// Internal `sh2pc.exe` driver ID (`5000..=5698` at `0x8A67E0`), if applicable.
    pub internal_driver_id: Option<u16>,
    /// Primary game `se_id` (`u16` in `0x8A1ED8` or `0x8A1E58`), or `None` if completely unreferenced in `SE_PAIRS`.
    pub primary_se_id: Option<u16>,
    /// 0-based `voice.afs` archive index (`0..=158`), if stored in `voice.afs`.
    pub voice_afs_index: Option<usize>,
    /// Optional relative path under `sh2e/` or `data/` (e.g. `"sound/extra/sankaku_toujyou_2.wav"`).
    pub extra_rel_path: Option<&'static str>,
    /// Retail `sddata.bin` byte offset (`data/sound/sddata.bin`), if applicable.
    pub retail_byte_offset: u32,
    /// Retail `sddata.bin` payload size in bytes, if applicable.
    pub retail_byte_length: u32,
    /// Whether the waveform loops continuously.
    pub loops: bool,
    /// Concise title and context from `AudioScripts/SFX/SFX-Map.csv` / `Dialog-Map.csv`.
    pub title: &'static str,
}

/// The 5 RIFF WAVE entries in retail `sddata.bin` (`sddata[360..=364]`, internal IDs `5360..=5364`)
/// that have **zero references** in `sh2pc.exe`'s 666-entry `SE_PAIRS` table (`0x8A1ED8`).
pub const UNREFERENCED_SDDATA_ENTRIES: [CutSfxCatalogEntry; 5] = [
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnreferencedRetailSddata,
        sddata_index: Some(360),
        internal_driver_id: Some(5360),
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00D6_6E8E,
        retail_byte_length: 64_740,
        loops: true,
        title: "sddata360 (ID 5360): Unused Radio Static Loop Variant 1",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnreferencedRetailSddata,
        sddata_index: Some(361),
        internal_driver_id: Some(5361),
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00D7_6B74,
        retail_byte_length: 70_824,
        loops: true,
        title: "sddata361 (ID 5361): Unused Radio Static Loop Variant 2",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnreferencedRetailSddata,
        sddata_index: Some(362),
        internal_driver_id: Some(5362),
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00D8_801E,
        retail_byte_length: 200_788,
        loops: true,
        title: "sddata362 (ID 5362): Unused Radio Static Loop Variant 3",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnreferencedRetailSddata,
        sddata_index: Some(363),
        internal_driver_id: Some(5363),
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00DB_9074,
        retail_byte_length: 104_402,
        loops: true,
        title: "sddata363 (ID 5363): Unused Radio Static Loop Variant 4",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnreferencedRetailSddata,
        sddata_index: Some(364),
        internal_driver_id: Some(5364),
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00DD_2848,
        retail_byte_length: 104_402,
        loops: true,
        title: "sddata364 (ID 5364): Unused Radio Static Loop Variant 5",
    },
];

/// Curated catalog of all cut, unreferenced, rare 3D-only (`40000+`), and restored audio cues (`SH2-62`)
/// available for instant auditioning in the Audio Debug Studio.
pub const CUT_AND_RARE_AUDIO_CATALOG: [CutSfxCatalogEntry; 29] = [
    // 1..5: The 5 completely unreferenced retail sddata.bin WAVs (internal IDs 5360..5364)
    UNREFERENCED_SDDATA_ENTRIES[0],
    UNREFERENCED_SDDATA_ENTRIES[1],
    UNREFERENCED_SDDATA_ENTRIES[2],
    UNREFERENCED_SDDATA_ENTRIES[3],
    UNREFERENCED_SDDATA_ENTRIES[4],
    // 6..18: Notable 3D-only (`40000+` / `0x9C40+`) hidden/environmental cues (internal IDs 5367..5393)
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(367),
        internal_driver_id: Some(5367),
        primary_se_id: Some(0x9C51), // 40017
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00DF_456A,
        retail_byte_length: 95_728,
        loops: false,
        title: "sddata367 (40017 / 0x9C51): Mystery Rapid Footsteps Burst",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(368),
        internal_driver_id: Some(5368),
        primary_se_id: Some(0x9CC7), // 40135
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00E0_BB5C,
        retail_byte_length: 62_310,
        loops: false,
        title: "sddata368 (40135 / 0x9CC7): Basement Pig / Creature Snarl",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(369),
        internal_driver_id: Some(5369),
        primary_se_id: Some(0x9CEF), // 40175
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00E1_AEC4,
        retail_byte_length: 59_558,
        loops: false,
        title: "sddata369 (40175 / 0x9CEF): Hotel Secret Crying Noise A",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(370),
        internal_driver_id: Some(5370),
        primary_se_id: Some(0x9CF6), // 40182
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00E2_976C,
        retail_byte_length: 112_934,
        loops: false,
        title: "sddata370 (40182 / 0x9CF6): Hotel Secret Crying Noise B",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(372),
        internal_driver_id: Some(5372),
        primary_se_id: Some(0x9C6C), // 40044
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00E5_7B0E,
        retail_byte_length: 41_900,
        loops: false,
        title: "sddata372 (40044 / 0x9C6C): Apartment Secret Door Knob Rattle",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(375),
        internal_driver_id: Some(5375),
        primary_se_id: Some(0x9CFE), // 40190
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00EA_713C,
        retail_byte_length: 118_116,
        loops: false,
        title: "sddata375 (40190 / 0x9CFE): Prison Gallows Ritual Chant",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(378),
        internal_driver_id: Some(5378),
        primary_se_id: Some(0x9C49), // 40009
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00EE_9E38,
        retail_byte_length: 185_862,
        loops: false,
        title: "sddata378 (40009 / 0x9C49): Apartment Man Screaming",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(382),
        internal_driver_id: Some(5382),
        primary_se_id: Some(0x9CF8), // 40184
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00F6_2F7C,
        retail_byte_length: 30_854,
        loops: false,
        title: "sddata382 (40184 / 0x9CF8): Mary Whispering 'James!'",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(383),
        internal_driver_id: Some(5383),
        primary_se_id: Some(0x9CF7), // 40183
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00F6_A804,
        retail_byte_length: 110_694,
        loops: false,
        title: "sddata383 (40183 / 0x9CF7): Hotel Dying Breath / Gasp",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(386),
        internal_driver_id: Some(5386),
        primary_se_id: Some(0x9C42), // 40002
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00FB_5E54,
        retail_byte_length: 61_516,
        loops: false,
        title: "sddata386 (40002 / 0x9C42): Unidentified Distorted Vocal ('WTF')",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(387),
        internal_driver_id: Some(5387),
        primary_se_id: Some(0x9C70), // 40048
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00FC_4EA2,
        retail_byte_length: 147_546,
        loops: false,
        title: "sddata387 (40048 / 0x9C70): Room 209 Secret Whisper ('See my dead wife...')",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(388),
        internal_driver_id: Some(5388),
        primary_se_id: Some(0x9C73), // 40051
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x00FE_8EFE,
        retail_byte_length: 194_662,
        loops: false,
        title: "sddata388 (40051 / 0x9C73): Historical Society Stairwell Foghorn",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::Hidden3dPositionalOnly,
        sddata_index: Some(390),
        internal_driver_id: Some(5390),
        primary_se_id: Some(0x9C88), // 40072
        voice_afs_index: None,
        extra_rel_path: None,
        retail_byte_offset: 0x0102_AD9E,
        retail_byte_length: 170_438,
        loops: false,
        title: "sddata390 (40072 / 0x9C88): Toluca Lake Monster Groan",
    },
    // 19..23: Unused / Special `voice.afs` Cutscene & Mystery Streams (`Build-Dialog-Files.bat` packing order)
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAD7),
        voice_afs_index: Some(VOICE_AFS_LAURA_LETTER_INDEX),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[119] (SCENE_45 / 0xEAD7): Unused Mary Letter to Laura Voiceover (67.6s)",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAD2),
        voice_afs_index: Some(114),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[114] (sankaku_toujyou / 0xEAD2): Apartment Closet Pyramid Head Appear (84.9s)",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAD1),
        voice_afs_index: Some(113),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[113] (sankaku_jisatsu / 0xEAD1): Unused/Mystery Pyramid Head Cue (5.5s)",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEA85),
        voice_afs_index: Some(37),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[37] (splay_at_48 / 0xEA85): Mystery Spray Attack Cue (3.5s)",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAE6),
        voice_afs_index: Some(134),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[134] (bort_2 / 0xEAE6): Hotel Boat Arrival Ambient (Text-Only Monologue, 64.7s)",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAD4),
        voice_afs_index: Some(VOICE_AFS_SATSUGAI_INDEX),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[116] (satsugai / 0xEAD4): Room 312 Videotape Standalone Audio Stream",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAC1),
        voice_afs_index: Some(VOICE_AFS_MARY_LETTER_FULL_INDEX),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[97] (mary_let_fix / 0xEAC1): Mary's Full Ending Letter Monologue",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAB3),
        voice_afs_index: Some(VOICE_AFS_MARIA_DEAI_TSUIKA_INDEX),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[83] (m_deai_tsuika / 0xEAB3): Rosewater Park Maria Additional Stream",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::UnusedVoiceAfsStream,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: Some(0xEAEC),
        voice_afs_index: Some(VOICE_AFS_BORN_FROM_A_WISH_TSUIKA_INDEX),
        extra_rel_path: None,
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "voice.afs[140] (bp_mar_tsuika / 0xEAEC): Born From A Wish Additional Stream",
    },
    // 28..29: SH2:EE Restored Cutscene Bonus WAVs (`sh2e/sound/extra/`)
    CutSfxCatalogEntry {
        category: CutSfxCategory::RestoredExtraWav,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: Some("sound/extra/sankaku_toujyou_2.wav"),
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "sankaku_toujyou_2.wav: Restored Apt Closet Cutscene Bonus Audio",
    },
    CutSfxCatalogEntry {
        category: CutSfxCategory::RestoredExtraWav,
        sddata_index: None,
        internal_driver_id: None,
        primary_se_id: None,
        voice_afs_index: None,
        extra_rel_path: Some("sound/extra/machi.wav"),
        retail_byte_offset: 0,
        retail_byte_length: 0,
        loops: false,
        title: "machi.wav: Restored Lying Figure Street Footsteps (CS_HSP_BOSS_FINISH)",
    },
];

/// A single timed voiceover slice of Mary's unused letter to Laura (`voice.afs` entry `119` / `SCENE_45`)
/// synchronized to the 11 text pages in `CS_HTL_LAURA_PIANO` (`PlayUnusedAudio.cpp`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LauraLetterVoiceSlice {
    /// 0-based page index (`0..=10`).
    pub page_index: usize,
    /// Start timestamp in seconds within `voice.afs[119]` (`SCENE_45`).
    pub start_sec: f32,
    /// End timestamp in seconds within `voice.afs[119]` (`SCENE_45`).
    pub end_sec: f32,
    /// Transcript line spoken by Mary in this slice.
    pub subtitle_excerpt: &'static str,
}

impl LauraLetterVoiceSlice {
    /// Duration of this voiceover slice in seconds (`end_sec - start_sec`).
    #[inline]
    pub const fn duration_sec(&self) -> f32 {
        self.end_sec - self.start_sec
    }
}

/// Exact 11-page timestamp table from SH2:EE `Patches/PlayUnusedAudio.cpp` (`seqStart[11]` / `seqEnd[11]`)
/// for restoring Mary's voiceover reading her letter to Laura in the Lakeview Hotel Restaurant (`voice.afs[119]` / `SCENE_45`).
pub const LAURA_LETTER_VOICEOVER_SLICES: [LauraLetterVoiceSlice; 11] = [
    LauraLetterVoiceSlice {
        page_index: 0,
        start_sec: 0.0,
        end_sec: 7.0,
        subtitle_excerpt: "My dearest Laura, I'm leaving this letter with Rachel to give to you after I'm gone.",
    },
    LauraLetterVoiceSlice {
        page_index: 1,
        start_sec: 7.2,
        end_sec: 12.4,
        subtitle_excerpt: "I'm far away now. In a quiet, beautiful place.",
    },
    LauraLetterVoiceSlice {
        page_index: 2,
        start_sec: 13.0,
        end_sec: 16.9,
        subtitle_excerpt: "Please forgive me for not saying goodbye before I left.",
    },
    LauraLetterVoiceSlice {
        page_index: 3,
        start_sec: 17.1,
        end_sec: 19.0,
        subtitle_excerpt: "Be well, Laura.",
    },
    LauraLetterVoiceSlice {
        page_index: 4,
        start_sec: 19.2,
        end_sec: 22.0,
        subtitle_excerpt: "Don't be too hard on the sisters.",
    },
    LauraLetterVoiceSlice {
        page_index: 5,
        start_sec: 22.5,
        end_sec: 34.5,
        subtitle_excerpt: "And Laura, about James... I know you hate him because you think he isn't nice to me, but please give him a chance.",
    },
    LauraLetterVoiceSlice {
        page_index: 6,
        start_sec: 35.2,
        end_sec: 46.2,
        subtitle_excerpt: "It's true he may be a little surly sometimes, and he doesn't laugh much. But underneath he's really a sweet person.",
    },
    LauraLetterVoiceSlice {
        page_index: 7,
        start_sec: 47.2,
        end_sec: 52.2,
        subtitle_excerpt: "Laura... I love you like my very own daughter.",
    },
    LauraLetterVoiceSlice {
        page_index: 8,
        start_sec: 53.4,
        end_sec: 60.2,
        subtitle_excerpt: "If things had worked out differently, I was hoping to adopt you.",
    },
    LauraLetterVoiceSlice {
        page_index: 9,
        start_sec: 61.2,
        end_sec: 63.5,
        subtitle_excerpt: "Happy 8th birthday, Laura.",
    },
    LauraLetterVoiceSlice {
        page_index: 10,
        start_sec: 64.0,
        end_sec: 67.2,
        subtitle_excerpt: "Your friend forever, Mary",
    },
];

/// Action emitted by [`CutAudioRestorer::tick`] when a restored cutscene cue should be played or stopped (`SH2-62`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutAudioRestoreAction {
    /// No state transition on this tick.
    None,
    /// Play slice `page_index` (`0..=10`) of `voice.afs[133]` (`SCENE_45` Mary's letter to Laura).
    PlayLauraLetterSlice(usize),
    /// Stop any currently playing `SCENE_45` voiceover slice (e.g. player skipped page or cutscene ended).
    StopLauraLetterVoiceover,
    /// Trigger `sh2e/sound/extra/sankaku_toujyou_2.wav` during `CS_APT_RPT_CLOSET` (`timer > 1900.0`).
    PlayClosetCutsceneBonusAudio,
    /// Trigger `sh2e/sound/extra/machi.wav` during `CS_HSP_BOSS_FINISH` (`R_TOWN_EAST`).
    PlayPostHospitalLyingFigureSteps,
}

/// Stateful controller replicating SH2:EE's `Patches/PlayUnusedAudio.cpp` and `ClosetCutsceneBonusAudio` (`PlayWavSound.cpp`) (`SH2-62`).
#[derive(Debug, Clone, Default)]
pub struct CutAudioRestorer {
    /// Whether the `CS_HTL_LAURA_PIANO` letter sequence is currently active.
    pub laura_letter_active: bool,
    /// Next letter page slice index (`0..=10`) awaiting playback, or `11` when all 11 pages have played.
    pub next_laura_slice_index: usize,
    /// Remaining playback time (in seconds) before the current letter slice finishes or advances.
    pub active_slice_remaining_sec: f32,
    /// Last seen `text_Wait_Status` (`0` = text typing/advancing, `1` = waiting at page prompt).
    pub last_text_wait_status: u8,
    /// Whether `sankaku_toujyou_2.wav` has already been triggered during the current `CS_APT_RPT_CLOSET` playback.
    pub closet_bonus_triggered: bool,
    /// Whether `machi.wav` has already been triggered during the current `CS_HSP_BOSS_FINISH` playback.
    pub machi_steps_triggered: bool,
}

impl CutAudioRestorer {
    /// Creates a new idle [`CutAudioRestorer`].
    pub const fn new() -> Self {
        Self {
            laura_letter_active: false,
            next_laura_slice_index: 0,
            active_slice_remaining_sec: 0.0,
            last_text_wait_status: 0,
            closet_bonus_triggered: false,
            machi_steps_triggered: false,
        }
    }

    /// Resets all tracked cutscene flags.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Evaluates cutscene restoration triggers for the current frame (`SH2-62`):
    ///
    /// - When `enable_restored_cut_audio` is `false`, immediately resets state and returns [`CutAudioRestoreAction::None`]
    ///   (or [`CutAudioRestoreAction::StopLauraLetterVoiceover`] if toggled off mid-playback).
    /// - In `CS_HTL_LAURA_PIANO` (`0x3B`) inside `R_HTL_RESTAURANT` (`0x9E`):
    ///   - Starts slice `0` (`0.0s..2.5s`) once `cutscene_timer >= 1600.0` and `fade_state == 3`.
    ///   - Advances through slices `1..=10` as each text page advances (`text_wait_status` transitions `1 -> 0` or slice timer completes).
    /// - In `CS_APT_RPT_CLOSET` (`0x10`):
    ///   - Emits [`CutAudioRestoreAction::PlayClosetCutsceneBonusAudio`] once `cutscene_timer > 1900.0`.
    /// - In `CS_HSP_BOSS_FINISH` (`0x26`):
    ///   - Emits [`CutAudioRestoreAction::PlayPostHospitalLyingFigureSteps`] once `cutscene_timer > 100.0`.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        enable_restored_cut_audio: bool,
        cutscene_id: u32,
        room_id: u32,
        cutscene_timer: f32,
        fade_state: u8,
        text_wait_status: u8,
        dt_sec: f32,
    ) -> CutAudioRestoreAction {
        if !enable_restored_cut_audio {
            let was_active = self.laura_letter_active;
            self.reset();
            return if was_active {
                CutAudioRestoreAction::StopLauraLetterVoiceover
            } else {
                CutAudioRestoreAction::None
            };
        }

        // 1. Apartment Closet Cutscene Bonus Audio (`CS_APT_RPT_CLOSET`, `PlayWavSound.cpp`)
        if cutscene_id == CS_APT_RPT_CLOSET_ID {
            if cutscene_timer > 1900.0 && !self.closet_bonus_triggered {
                self.closet_bonus_triggered = true;
                return CutAudioRestoreAction::PlayClosetCutsceneBonusAudio;
            }
            return CutAudioRestoreAction::None;
        } else {
            self.closet_bonus_triggered = false;
        }

        // 2. Post-Hospital Lying Figure Street Footsteps (`CS_HSP_BOSS_FINISH`, `PlayWavSound.cpp`)
        if cutscene_id == CS_HSP_BOSS_FINISH_ID {
            if cutscene_timer > 100.0 && !self.machi_steps_triggered {
                self.machi_steps_triggered = true;
                return CutAudioRestoreAction::PlayPostHospitalLyingFigureSteps;
            }
            return CutAudioRestoreAction::None;
        } else {
            self.machi_steps_triggered = false;
        }

        // 3. Lakeview Hotel Restaurant Mary's Letter to Laura (`CS_HTL_LAURA_PIANO`, `PlayUnusedAudio.cpp`)
        if cutscene_id != CS_HTL_LAURA_PIANO_ID || room_id != R_HTL_RESTAURANT_ID {
            let was_active = self.laura_letter_active;
            self.laura_letter_active = false;
            self.next_laura_slice_index = 0;
            self.active_slice_remaining_sec = 0.0;
            self.last_text_wait_status = 0;
            return if was_active {
                CutAudioRestoreAction::StopLauraLetterVoiceover
            } else {
                CutAudioRestoreAction::None
            };
        }

        // Cutscene exited or jumped past letter reading (`cutscene_timer == 2069.0` or `0.0`)
        if (cutscene_timer - 2069.0).abs() < 0.5
            || (self.laura_letter_active && cutscene_timer < 1.0)
        {
            let was_active = self.laura_letter_active;
            self.laura_letter_active = false;
            self.next_laura_slice_index = LAURA_LETTER_VOICEOVER_SLICES.len();
            self.active_slice_remaining_sec = 0.0;
            return if was_active {
                CutAudioRestoreAction::StopLauraLetterVoiceover
            } else {
                CutAudioRestoreAction::None
            };
        }

        // Start slice 0 when `cutscene_timer >= 1600.0` and `fade_state == 3`
        if !self.laura_letter_active && cutscene_timer >= 1600.0 && fade_state == 3 {
            self.laura_letter_active = true;
            self.next_laura_slice_index = 1;
            self.active_slice_remaining_sec = LAURA_LETTER_VOICEOVER_SLICES[0].duration_sec();
            self.last_text_wait_status = text_wait_status;
            return CutAudioRestoreAction::PlayLauraLetterSlice(0);
        }

        if self.laura_letter_active {
            self.active_slice_remaining_sec =
                (self.active_slice_remaining_sec - dt_sec.max(0.0)).max(0.0);

            // Advance to next page slice when player advances text page (`last == 1 && current == 0`)
            // or for page 1 immediately after slice 0 finishes (`PlayUnusedAudio.cpp` lines 163-195).
            let page_turned = self.last_text_wait_status == 1 && text_wait_status == 0;
            let auto_chain_slice_1 =
                self.next_laura_slice_index == 1 && self.active_slice_remaining_sec <= 0.0;
            self.last_text_wait_status = text_wait_status;

            if (page_turned || auto_chain_slice_1)
                && self.next_laura_slice_index < LAURA_LETTER_VOICEOVER_SLICES.len()
            {
                let idx = self.next_laura_slice_index;
                self.next_laura_slice_index += 1;
                self.active_slice_remaining_sec = LAURA_LETTER_VOICEOVER_SLICES[idx].duration_sec();
                return CutAudioRestoreAction::PlayLauraLetterSlice(idx);
            }
        }

        CutAudioRestoreAction::None
    }
}

/// Metadata entry for the 33 PS2 S-Force 3D positional audio `.sfc` banks in `SLUS_202.28` (`se_file_list$1329` at `0x2b8570`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ps2PrototypeSfcEntry {
    /// Index in `se_file_list$1329` (`0..=34`).
    pub list_index: usize,
    /// Relative path inside `DATA/SOUND.MGF`.
    pub path: &'static str,
    /// Byte offset inside July 13, 2001 `DATA/SOUND.MGF`.
    pub mgf_offset: u32,
    /// Size in bytes inside July 13, 2001 `DATA/SOUND.MGF`.
    pub byte_length: u32,
    /// Notes on stage usage or developer attribution.
    pub notes: &'static str,
}

/// Notable PS2 Prototype `.sfc` S-Force 3D positional audio banks (`SLUS_202.28` `0x2b8570`).
pub const PS2_PROTOTYPE_SFC_CATALOG: [Ps2PrototypeSfcEntry; 8] = [
    Ps2PrototypeSfcEntry {
        list_index: 1,
        path: "data/sound/s_force/town_east_a.sfc",
        mgf_offset: 0x0000_0000,
        byte_length: 446_512,
        notes: "Town East (prologue fog streets) S-Force 3D PCM bank",
    },
    Ps2PrototypeSfcEntry {
        list_index: 2,
        path: "data/sound/s_force/apart_e.sfc",
        mgf_offset: 0x0006_D800,
        byte_length: 616_576,
        notes: "Wood Side Apartments S-Force 3D PCM bank",
    },
    Ps2PrototypeSfcEntry {
        list_index: 5,
        path: "data/sound/s_force/apart_stair.sfc",
        mgf_offset: 0x001F_A800,
        byte_length: 500_784,
        notes: "Apartment Stairwell Pyramid Head Boss S-Force 3D PCM bank",
    },
    Ps2PrototypeSfcEntry {
        list_index: 9,
        path: "data/sound/s_force/hospital_1f_f.sfc",
        mgf_offset: 0x0037_8000,
        byte_length: 646_208,
        notes: "Brookhaven Hospital 1F Normal S-Force 3D PCM bank (largest .sfc)",
    },
    Ps2PrototypeSfcEntry {
        list_index: 15,
        path: "data/sound/s_force/iketani.sfc",
        mgf_offset: 0x0059_1000,
        byte_length: 528_512,
        notes: "Developer test soundbank named after Sound Designer Takayuki Iketani",
    },
    Ps2PrototypeSfcEntry {
        list_index: 24,
        path: "data/sound/s_force/labyrinth_e.sfc",
        mgf_offset: 0x009B_B800,
        byte_length: 528_512,
        notes: "Labyrinth East S-Force 3D PCM bank",
    },
    Ps2PrototypeSfcEntry {
        list_index: 27,
        path: "data/sound/s_force/eddie_boss.sfc",
        mgf_offset: 0x00B3_F800,
        byte_length: 590_096,
        notes: "Eddie Boss Meat Locker S-Force 3D PCM bank",
    },
    Ps2PrototypeSfcEntry {
        list_index: 28,
        path: "data/sound/s_force/hotel_1f_f.sfc",
        mgf_offset: 0x00BD_0000,
        byte_length: 416_336,
        notes: "Lakeview Hotel 1F S-Force 3D PCM bank",
    },
];

/// Metadata for the 14 cut/prototype `.sdb` spatial BGM mixing files in E3 2001 (`SLPM_123.45` `data_sound_snd_data.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ps2PrototypeSdbSummary {
    /// `.sdb` filename in E3 `data/sound/snd_data/`.
    pub filename: &'static str,
    /// Offset inside E3 `snd_data.mgf` (`DATA.MGF + 340,490,240`).
    pub snd_data_mgf_offset: u32,
    /// File size in bytes.
    pub byte_length: u32,
    /// Number of 64-byte `_SOUND_DATA` records (`byte_length / 64`).
    pub record_count: usize,
    /// Comparison notes against July 13 (`SLUS_202.28`) and retail PC (`0x8BC8C0`).
    pub notes: &'static str,
}

/// Catalog of the E3 2001 (`SLPM_123.45`) cut 2-letter stage `.sdb` files and 75-record `tgs_trial.sdb`.
pub const E3_PROTOTYPE_SDB_CATALOG: [Ps2PrototypeSdbSummary; 15] = [
    Ps2PrototypeSdbSummary {
        filename: "tgs_trial.sdb",
        snd_data_mgf_offset: 141_312,
        byte_length: 4_800,
        record_count: 75,
        notes: "E3/TGS Trial 75-box spatial BGM layout (shrunk to 10 boxes in July 13 & Retail Stage 1)",
    },
    Ps2PrototypeSdbSummary {
        filename: "ap.sdb",
        snd_data_mgf_offset: 0,
        byte_length: 6_848,
        record_count: 107,
        notes: "Cut unified Apartments ('ap') 107-box table (split into apart_e1f..w2f before July 13)",
    },
    Ps2PrototypeSdbSummary {
        filename: "hp.sdb",
        snd_data_mgf_offset: 40_960,
        byte_length: 7_680,
        record_count: 120,
        notes: "Cut unified Brookhaven Hospital ('hp') 120-box table (split into hospital_bf..rf before July 13)",
    },
    Ps2PrototypeSdbSummary {
        filename: "rr.sdb",
        snd_data_mgf_offset: 102_400,
        byte_length: 2_496,
        record_count: 39,
        notes: "Cut unified Lakeview Hotel ('rr') 39-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "ru.sdb",
        snd_data_mgf_offset: 112_640,
        byte_length: 2_688,
        record_count: 42,
        notes: "Cut unified Labyrinth ('ru') 42-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "bw.sdb",
        snd_data_mgf_offset: 8_192,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Bowling Alley ('bw') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "ca.sdb",
        snd_data_mgf_offset: 12_288,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Town East ('ca') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "cb.sdb",
        snd_data_mgf_offset: 16_384,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Town ('cb') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "cc.sdb",
        snd_data_mgf_offset: 20_480,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Town West ('cc') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "cd.sdb",
        snd_data_mgf_offset: 24_576,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Rosewater Park ('cd') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "er.sdb",
        snd_data_mgf_offset: 28_672,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Restroom ('er') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "ob.sdb",
        snd_data_mgf_offset: 81_920,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Observation Deck ('ob') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "ps.sdb",
        snd_data_mgf_offset: 88_064,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Prison ('ps') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "qp.sdb",
        snd_data_mgf_offset: 94_208,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early stage ('qp') 10-box table",
    },
    Ps2PrototypeSdbSummary {
        filename: "th.sdb",
        snd_data_mgf_offset: 147_456,
        byte_length: 640,
        record_count: 10,
        notes: "Cut early Historical Society ('th') 10-box table",
    },
];

/// Representative subset of E3 2001 `tgs_trial.sdb` (`SLPM_123.45`, `DATA.MGF + 340,631,552`)
/// `_SOUND_DATA` spatial bounding boxes extracted directly from the E3 trial binary (`SH2-62`),
/// enabling 3D wireframe inspection and comparison against retail `STAGE_01_SOUND_DATA` even when
/// the 2 GB E3 ISO is not mounted on disk.
pub const E3_TGS_TRIAL_SDB_SAMPLE_RECORDS: [SoundDataRecord; 11] = [
    // Record #0: Default fallback record (sentinel 9900000256.0)
    SoundDataRecord {
        x0: 9900000256.0,
        z0: 9900000256.0,
        x1: 9900000256.0,
        z1: 9900000256.0,
        x2: 9900000256.0,
        z2: 9900000256.0,
        max_y: 9900000256.0,
        min_y: 9900000256.0,
        flags: 0,
        chanstat: [0x21, 0, 0, 0, 0, 0, 0],
    },
    // Record #1: E3 TGS Trial West Corridor Box 1
    SoundDataRecord {
        x0: -25400.0,
        z0: -17550.0,
        x1: -25400.0,
        z1: -22950.0,
        x2: -9050.0,
        z2: -22950.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0x20, 0, 0, 0, 0, 0],
    },
    // Record #2: E3 TGS Trial Room 101 Box
    SoundDataRecord {
        x0: -23600.0,
        z0: -22750.0,
        x1: -23600.0,
        z1: -29950.0,
        x2: -13550.0,
        z2: -29950.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0x21, 0x20, 0, 0, 0, 0],
    },
    // Record #3: E3 TGS Trial Room 102 Box
    SoundDataRecord {
        x0: -24250.0,
        z0: -12050.0,
        x1: -24250.0,
        z1: -17850.0,
        x2: -17650.0,
        z2: -17850.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0x20, 0x21, 0, 0, 0, 0],
    },
    // Record #4: E3 TGS Trial Room 103 Box
    SoundDataRecord {
        x0: -17900.0,
        z0: -10250.0,
        x1: -17900.0,
        z1: -17750.0,
        x2: -13300.0,
        z2: -17750.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0, 0x21, 0x20, 0, 0, 0],
    },
    // Record #5: E3 TGS Trial Room 104 Box
    SoundDataRecord {
        x0: -13550.0,
        z0: -11950.0,
        x1: -13550.0,
        z1: -17800.0,
        x2: -8900.0,
        z2: -17800.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0x21, 0, 0x21, 0, 0, 0],
    },
    // Record #6: E3 TGS Trial Central Hallway Box
    SoundDataRecord {
        x0: -9250.0,
        z0: -4900.0,
        x1: -9250.0,
        z1: -22900.0,
        x2: -5150.0,
        z2: -22900.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0x21, 0x21, 0, 0, 0, 0],
    },
    // Record #7: E3 TGS Trial East Wing Box A
    SoundDataRecord {
        x0: 14700.0,
        z0: 22950.0,
        x1: 14700.0,
        z1: 17550.0,
        x2: 31050.0,
        z2: 17550.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0, 0x21, 0x21, 0, 0, 0],
    },
    // Record #8: E3 TGS Trial East Wing Box B
    SoundDataRecord {
        x0: 54700.0,
        z0: 22950.0,
        x1: 54700.0,
        z1: 17550.0,
        x2: 71050.0,
        z2: 17550.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x20, 0x21, 0x21, 0x21, 0, 0, 0],
    },
    // Record #9: E3 TGS Trial North Wing Box C
    SoundDataRecord {
        x0: 94700.0,
        z0: 62950.0,
        x1: 94700.0,
        z1: 57550.0,
        x2: 111050.0,
        z2: 57550.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0x21, 0, 0, 0x21, 0, 0],
    },
    // Record #10: E3 TGS Trial Far Upper Wing Box D
    SoundDataRecord {
        x0: 130850.0,
        z0: 100600.0,
        x1: 130850.0,
        z1: 82600.0,
        x2: 134950.0,
        z2: 82600.0,
        max_y: 0.0,
        min_y: -3000.0,
        flags: 0,
        chanstat: [0x21, 0, 0x21, 0, 0x21, 0, 0],
    },
];
