//! Static Stage Tables (`Event_List`, `Pos_List`, `Item_List`, `Enemy_List`, `Stage_Data`),
//! Stage/Room Initialization Hooks, and Interactive `ev_prog` Riddles for Silent Hill Night,
//! Historical Society, Delusion, Toluca Prison, Labyrinth, Eddie Boss, Toluca Lake,
//! Lakeview Hotel (Normal + Otherworld + Fire + Endings), and *Born from a Wish* (`SH2-114`,
//! Stages `16` and `25..=58`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28` (`StgOverlayGetStageData` at `0x00228670`,
//! `src/Event/stage/stg_*.c`) and retail PC Director's Cut `sh2pc.exe` (`0x00494ec0`, jump table
//! `0x00495048`, `EventCheck` `0x0048e070`, `EventMainStandard` `0x0048e600`).

#![warn(missing_docs)]

pub mod retail_ctors;
pub mod retail_decode;
pub mod riddles_hotel_2f;
pub mod riddles_hotel_lobby_bar;
pub mod riddles_hotel_otherworld_wish;
pub mod riddles_labyrinth;
pub mod riddles_prison;
pub mod tables_endings;
pub mod tables_hospital_night;
pub mod tables_hotel_2f_3f;
pub mod tables_hotel_bf_1f;
pub mod tables_hotel_otherworld_a;
pub mod tables_hotel_otherworld_b;
pub mod tables_labyrinth_n_lake;
pub mod tables_labyrinth_w_e;
pub mod tables_prison_n;
pub mod tables_prison_s_bf;
pub mod tables_society_delusion;
pub mod tables_wish_a;
pub mod tables_wish_b;
pub mod wish_gravestone_render;

pub use retail_ctors::*;
pub use retail_decode::*;
pub use riddles_hotel_2f::*;
pub use riddles_hotel_lobby_bar::*;
pub use riddles_hotel_otherworld_wish::*;
pub use riddles_labyrinth::*;
pub use riddles_prison::*;
pub use tables_endings::*;
pub use tables_hospital_night::*;
pub use tables_hotel_2f_3f::*;
pub use tables_hotel_bf_1f::*;
pub use tables_hotel_otherworld_a::*;
pub use tables_hotel_otherworld_b::*;
pub use tables_labyrinth_n_lake::*;
pub use tables_labyrinth_w_e::*;
pub use tables_prison_n::*;
pub use tables_prison_s_bf::*;
pub use tables_society_delusion::*;
pub use tables_wish_a::*;
pub use tables_wish_b::*;
pub use wish_gravestone_render::*;

use crate::events::triggers::EventEvalInput;
use crate::inventory::InventoryManager;
use crate::types::{EventListEntry, ItemListEntry, StageDataHeader};

pub(crate) use retail_ctors::{en, ev, gi, hdr};

/// Canonical stage IDs ported in `SH2-114` (`StgOverlayGetStageData` `SLUS_202.28` `0x00228670`
/// and `sh2pc.exe` `0x00495048`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Act2StageId {
    /// Stage `16`: `stage_to_heaven` (`SLUS_202.28` `stg_to_heaven.c`, `sh2pc.exe` `0x008da6b0`).
    ToHeaven = 16,
    /// Stage `25`: `stage_hospital_3f_b` (`sh2pc.exe` `0x008d5fd8`).
    Hospital3fOtherworld = 25,
    /// Stage `26`: `stage_hospital_bf_b` (`sh2pc.exe` `0x008d5460`).
    HospitalBfOtherworld = 26,
    /// Stage `27`: `stage_hospital_pass` (`SLUS_202.28` `stg_hospital_pass.c`, `sh2pc.exe` `0x008d4ba0`).
    HospitalPass = 27,
    /// Stage `28`: `stage_society` (`SLUS_202.28` `stg_society.c`, `sh2pc.exe` `0x008d4950`).
    HistoricalSociety = 28,
    /// Stage `29`: `stage_delusion_2` (`SLUS_202.28` `stg_delusion_2.c`, `sh2pc.exe` `0x008d4648`).
    Delusion2 = 29,
    /// Stage `30`: `stage_delusion_3` (`SLUS_202.28` `stg_delusion_3.c`, `sh2pc.exe` `0x008d4208`).
    Delusion3 = 30,
    /// Stage `31`: `stage_prison_n` (`SLUS_202.28` `stg_prison_n.c`, `sh2pc.exe` `0x008d3518`).
    PrisonNorth = 31,
    /// Stage `32`: `stage_prison_s` (`SLUS_202.28` `stg_prison_s.c`, `sh2pc.exe` `0x008d2738`).
    PrisonSouth = 32,
    /// Stage `33`: `stage_prison_bf` (`SLUS_202.28` `stg_prison_bf.c`, `sh2pc.exe` `0x008d1c48`).
    PrisonBasement = 33,
    /// Stage `34`: `stage_labyrinth_w` (`SLUS_202.28` `stg_labyrinth_w.c`, `sh2pc.exe` `0x008d1470`).
    LabyrinthWest = 34,
    /// Stage `35`: `stage_labyrinth_e` (`SLUS_202.28` `stg_labyrinth_e.c`, `sh2pc.exe` `0x008d0230`).
    LabyrinthEast = 35,
    /// Stage `36`: `stage_labyrinth_n` (`SLUS_202.28` `stg_labyrinth_n.c`, `sh2pc.exe` `0x008cf2d8`).
    LabyrinthNorth = 36,
    /// Stage `37`: `stage_eddie_boss` (`SLUS_202.28` `stg_eddie_boss.c`, `sh2pc.exe` `0x008ccff8`).
    EddieBoss = 37,
    /// Stage `38`: `stage_lake` (`SLUS_202.28` `stg_lake.c`, `sh2pc.exe` `0x008cc948`).
    TolucaLake = 38,
    /// Stage `39`: `stage_hotel_bf_f` (`SLUS_202.28` `stg_hotel_bf_f.c`, `sh2pc.exe` `0x008cc640`).
    HotelBfNormal = 39,
    /// Stage `40`: `stage_hotel_1f_f` (`SLUS_202.28` `stg_hotel_1f_f.c`, `sh2pc.exe` `0x008cbcf8`).
    Hotel1fNormal = 40,
    /// Stage `41`: `stage_hotel_2f_f` (`SLUS_202.28` `stg_hotel_2f_f.c`, `sh2pc.exe` `0x008ca3d0`).
    Hotel2fNormal = 41,
    /// Stage `42`: `stage_hotel_3f_f` (`SLUS_202.28` `stg_hotel_3f_f.c`, `sh2pc.exe` `0x008c91a8`).
    Hotel3fNormal = 42,
    /// Stage `43`: `stage_hotel_bf_b` (`SLUS_202.28` `stg_hotel_bf_b.c`, `sh2pc.exe` `0x008c8c20`).
    HotelBfOtherworld = 43,
    /// Stage `44`: `stage_hotel_1f_b` (`SLUS_202.28` `stg_hotel_1f_b.c`, `sh2pc.exe` `0x008c81f8`).
    Hotel1fOtherworld = 44,
    /// Stage `45`: `stage_hotel_2f_b` (`SLUS_202.28` `stg_hotel_2f_b.c`, `sh2pc.exe` `0x008c7998`).
    Hotel2fOtherworld = 45,
    /// Stage `46`: `stage_hotel_3f_b` (`SLUS_202.28` `stg_hotel_3f_b.c`, `sh2pc.exe` `0x008c7200`).
    Hotel3fOtherworld = 46,
    /// Stage `47`: `stage_hotel_fire` (`SLUS_202.28` `stg_hotel_fire.c`, `sh2pc.exe` `0x008c6d88`).
    HotelFire = 47,
    /// Stage `48`: `stage_end_maria` (`sh2pc.exe` `0x007b12c0`).
    EndingMaria = 48,
    /// Stage `49`: `stage_end_suicide` (`sh2pc.exe` `0x007b1f98`).
    EndingInWater = 49,
    /// Stage `50`: `stage_end_rebirth` (`sh2pc.exe` `0x007b0598`).
    EndingRebirth = 50,
    /// Stage `51`: `stage_end_dog` (`sh2pc.exe` `0x008c6940`).
    EndingDog = 51,
    /// Stage `52`: `stage_end_leave` (`sh2pc.exe` `0x008c5f48`).
    EndingLeave = 52,
    /// Stage `53`: `stage_wish_heaven` (*Born from a Wish* Heaven's Night, `sh2pc.exe` `0x008c5de0`).
    WishHeavensNight = 53,
    /// Stage `54`: `stage_wish_end` (*Born from a Wish* Town Exterior, `sh2pc.exe` `0x008c5970`).
    WishTownExterior = 54,
    /// Stage `55`: `stage_wish_1f` (*Born from a Wish* Baldwin Mansion 1F, `sh2pc.exe` `0x008c4548`).
    WishMansion1f = 55,
    /// Stage `56`: `stage_wish_2f` (*Born from a Wish* Baldwin Mansion 2F/Attic, `sh2pc.exe` `0x008c3470`).
    WishMansion2f = 56,
    /// Stage `57`: `stage_wish_bf` (*Born from a Wish* Baldwin Mansion Basement, `sh2pc.exe` `0x008c2978`).
    WishMansionBasement = 57,
    /// Stage `58`: `stage_wish_ap` (*Born from a Wish* Blue Creek Pathway, `sh2pc.exe` `0x008c2460`).
    WishBlueCreek = 58,
}

/// Ordered list of all 35 Act 2 & sub-scenario stage IDs (`16`, `25..=58`).
pub const ALL_ACT2_STAGE_IDS: [u16; 35] = [
    16, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
];

impl Act2StageId {
    /// Converts a raw `1..=58` stage index into [`Act2StageId`] if covered by `SH2-114`.
    pub const fn from_u16(id: u16) -> Option<Self> {
        match id {
            16 => Some(Self::ToHeaven),
            25 => Some(Self::Hospital3fOtherworld),
            26 => Some(Self::HospitalBfOtherworld),
            27 => Some(Self::HospitalPass),
            28 => Some(Self::HistoricalSociety),
            29 => Some(Self::Delusion2),
            30 => Some(Self::Delusion3),
            31 => Some(Self::PrisonNorth),
            32 => Some(Self::PrisonSouth),
            33 => Some(Self::PrisonBasement),
            34 => Some(Self::LabyrinthWest),
            35 => Some(Self::LabyrinthEast),
            36 => Some(Self::LabyrinthNorth),
            37 => Some(Self::EddieBoss),
            38 => Some(Self::TolucaLake),
            39 => Some(Self::HotelBfNormal),
            40 => Some(Self::Hotel1fNormal),
            41 => Some(Self::Hotel2fNormal),
            42 => Some(Self::Hotel3fNormal),
            43 => Some(Self::HotelBfOtherworld),
            44 => Some(Self::Hotel1fOtherworld),
            45 => Some(Self::Hotel2fOtherworld),
            46 => Some(Self::Hotel3fOtherworld),
            47 => Some(Self::HotelFire),
            48 => Some(Self::EndingMaria),
            49 => Some(Self::EndingInWater),
            50 => Some(Self::EndingRebirth),
            51 => Some(Self::EndingDog),
            52 => Some(Self::EndingLeave),
            53 => Some(Self::WishHeavensNight),
            54 => Some(Self::WishTownExterior),
            55 => Some(Self::WishMansion1f),
            56 => Some(Self::WishMansion2f),
            57 => Some(Self::WishMansionBasement),
            58 => Some(Self::WishBlueCreek),
            _ => None,
        }
    }

    /// Returns the numeric stage ID (`16` or `25..=58`).
    #[inline]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// User-configurable toggles for SH2:EE fixes vs strict 1:1 original PS2/PC behavior (`AGENTS.md` §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Act2EnhancementToggles {
    /// SH2:EE `PrisonerTimer.cpp` (`0x004ae5f8`): doubles chanting prisoner step threshold at 60 Hz.
    pub prisoner_timer_60fps_fix: bool,
    /// SH2:EE `MusicBoxVolume.cpp` (`flag 0x1d8 = 472`): restores full NA/PS2 music box melody gain.
    pub music_box_volume_fix: bool,
    /// SH2:EE `VHSAudio.cpp`: plays second audio stream (`track 1`) during Room 312 `murder.sfd`/`.bik`.
    pub vhs_second_audio_track: bool,
    /// SH2:EE `Room312FlashlightFix.cpp` (`0x0048ebc8`): prevents Room `0xA2` flashlight inventory bleed.
    pub room_312_flashlight_fix: bool,
    /// SH2:EE `PuzzleAlignmentFixes.cpp` (`GravestoneBoardsFix`): aligns *Born from a Wish* gravestone UVs.
    pub gravestone_boards_alignment_fix: bool,
}

impl Act2EnhancementToggles {
    /// Strict 1:1 original vanilla behavior (all modern fixes disabled).
    pub const fn vanilla_ps2() -> Self {
        Self {
            prisoner_timer_60fps_fix: false,
            music_box_volume_fix: false,
            vhs_second_audio_track: false,
            room_312_flashlight_fix: false,
            gravestone_boards_alignment_fix: false,
        }
    }

    /// Enables all five SH2:EE Act 2 & *Born from a Wish* presentation/timing fixes.
    pub const fn enhanced() -> Self {
        Self {
            prisoner_timer_60fps_fix: true,
            music_box_volume_fix: true,
            vhs_second_audio_track: true,
            room_312_flashlight_fix: true,
            gravestone_boards_alignment_fix: true,
        }
    }
}

/// Complete static stage descriptor for a single Act 2 or *Born from a Wish* stage (`Stage_Data`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageTableAct2 {
    /// Numeric stage index (`16` or `25..=58`).
    pub stage_id: u16,
    /// Typed [`Act2StageId`] enum discriminant.
    pub stage_kind: Act2StageId,
    /// Canonical Team Silent symbol name (`stage_prison_n`, `stage_hotel_1f_f`, `stage_wish_1f`, ...).
    pub name: &'static str,
    /// Virtual address of `Stage_Data` in `sh2pc.exe`.
    pub retail_stage_data_va: u32,
    /// Virtual address of `Stage_Data` in `SLUS_202.28` (`0` for Director's Cut / non-overlay stages).
    pub ps2_stage_data_va: u32,
    /// `Stage_Data` pointer/callback header, projected from the retail 64-byte (`16 x u32`) record
    /// (see `hdr` in `retail_decode.rs` for the `word[15]`/`word[12]`/`word[14]` callback mapping).
    pub header: StageDataHeader,
    /// Raw retail `Event_List` slice terminated by [`EV_SENTINEL`].
    pub event_list: &'static [EventListEntry],
    /// Raw retail `ev_pos` spatial geometry byte buffer in the 10-byte `f32`/`f16` PC layout.
    /// Must be decoded via [`decode_retail_ev_pos_shape`]; it is NOT the 8-byte PS2 `i16` layout
    /// read by `EventEngine::tick_standard`'s shape tests.
    pub pos_list: &'static [u8],
    /// Virtual addresses of `ev_prog` coroutine handlers indexed by `EventListEntry::prog_index`.
    pub ev_prog_vas: &'static [u32],
    /// Lossless retail `Item_List` (`gi_list`) pickup entries.
    pub retail_items: &'static [RetailItemSpawn],
    /// Lossless retail `Enemy_List` (`en_list`) spawn entries.
    pub retail_enemies: &'static [RetailEnemySpawn],
}

impl StageTableAct2 {
    /// Active `Event_List` count excluding [`EV_SENTINEL`].
    #[inline]
    pub const fn active_event_count(&self) -> usize {
        self.event_list.len().saturating_sub(1)
    }

    /// Adapts `self.event_list` from retail `sh2pc.exe` trigger/result nibbles (`0x0048e1a9` &
    /// `0x0048e6af`) to `SLUS_202.28` [`crate::events::EventEngine::tick_standard`] nibbles.
    ///
    /// Flag-, button-, item- and `Always`-gated entries evaluate correctly; spatial shapes still
    /// reference [`Self::pos_list`] in the retail layout and must be resolved with
    /// [`decode_retail_ev_pos_shape`]. For carried-item triggers (`trg 7..=10`), use
    /// [`Self::engine_event_list_for_input`].
    pub fn engine_event_list(&self) -> Vec<EventListEntry> {
        self.event_list
            .iter()
            .copied()
            .map(normalize_retail_event_entry)
            .collect()
    }

    /// Adapts `self.event_list` for the current frame's `input` and `inventory`, preserving the
    /// retail `trg == 10` action-while-carrying auto-key-use path (`0x0048e1ee`).
    pub fn engine_event_list_for_input(
        &self,
        input: &EventEvalInput,
        inventory: &InventoryManager,
    ) -> Vec<EventListEntry> {
        self.event_list
            .iter()
            .copied()
            .map(|e| normalize_retail_event_entry_for_input(e, input, inventory))
            .collect()
    }

    /// Converts `self.retail_items` into a sentinel-terminated [`ItemListEntry`] buffer.
    pub fn ps2_item_list_with_sentinel(&self) -> Vec<ItemListEntry> {
        let mut out: Vec<ItemListEntry> = self
            .retail_items
            .iter()
            .map(RetailItemSpawn::to_ps2_clamped)
            .collect();
        out.push(ItemListEntry {
            flag: 0x3fff_0000,
            ..ItemListEntry::default()
        });
        out
    }
}

/// Looks up the static [`StageTableAct2`] for `stage_id` (`16` or `25..=58`).
pub const fn stage_table_act2(stage_id: u16) -> Option<&'static StageTableAct2> {
    match stage_id {
        16 => Some(&STAGE_TO_HEAVEN),
        25 => Some(&STAGE_HOSPITAL_3F_B),
        26 => Some(&STAGE_HOSPITAL_BF_B),
        27 => Some(&STAGE_HOSPITAL_PASS),
        28 => Some(&STAGE_SOCIETY),
        29 => Some(&STAGE_DELUSION_2),
        30 => Some(&STAGE_DELUSION_3),
        31 => Some(&STAGE_PRISON_N),
        32 => Some(&STAGE_PRISON_S),
        33 => Some(&STAGE_PRISON_BF),
        34 => Some(&STAGE_LABYRINTH_W),
        35 => Some(&STAGE_LABYRINTH_E),
        36 => Some(&STAGE_LABYRINTH_N),
        37 => Some(&STAGE_EDDIE_BOSS),
        38 => Some(&STAGE_LAKE),
        39 => Some(&STAGE_HOTEL_BF_F),
        40 => Some(&STAGE_HOTEL_1F_F),
        41 => Some(&STAGE_HOTEL_2F_F),
        42 => Some(&STAGE_HOTEL_3F_F),
        43 => Some(&STAGE_HOTEL_BF_B),
        44 => Some(&STAGE_HOTEL_1F_B),
        45 => Some(&STAGE_HOTEL_2F_B),
        46 => Some(&STAGE_HOTEL_3F_B),
        47 => Some(&STAGE_HOTEL_FIRE),
        48 => Some(&STAGE_END_MARIA),
        49 => Some(&STAGE_END_SUICIDE),
        50 => Some(&STAGE_END_REBIRTH),
        51 => Some(&STAGE_END_DOG),
        52 => Some(&STAGE_END_LEAVE),
        53 => Some(&STAGE_WISH_HEAVEN),
        54 => Some(&STAGE_WISH_END),
        55 => Some(&STAGE_WISH_1F),
        56 => Some(&STAGE_WISH_2F),
        57 => Some(&STAGE_WISH_BF),
        58 => Some(&STAGE_WISH_AP),
        _ => None,
    }
}
