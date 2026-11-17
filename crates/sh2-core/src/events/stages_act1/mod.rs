//! Retail Act 1 Stage Event Tables, Enemy/Item/Model Lists, Lifecycle Hooks, and Interactive
//! Riddles for Stages 1–27 (`SH2-113`: Town, Wood Side & Blue Creek Apartments, Pete's Bowl-O-Rama,
//! Heaven's Night, and Brookhaven Hospital Normal + Otherworld).
//!
//! Extracted 1:1 from `SLUS_202.28` (`StgOverlayGetStageData` at `0x00228670`) and retail PC
//! `sh2pc.exe` (`0x00494ec0`, jump table `0x00495048`, `.data` `0x008d4a00..0x008eebf0`), with
//! toggleable 1:1 vanilla PS2 vs SH2:EE fixes (`AGENTS.md` §3.4).

pub mod descriptor;
pub mod lifecycle;
pub mod riddles_apartments;
pub mod riddles_hospital_normal;
pub mod riddles_hospital_otherworld;
pub mod tables_apart_e1f_e2f;
pub mod tables_apart_e3f;
pub mod tables_apart_stair_out;
pub mod tables_apart_w1f_w2f;
pub mod tables_bowling_heaven;
pub mod tables_hospital_normal_1f_2f;
pub mod tables_hospital_normal_3f_rf;
pub mod tables_hospital_otherworld_1f_2f;
pub mod tables_hospital_otherworld_3f_bf;
pub mod tables_intro;
pub mod tables_town_east_entities;
pub mod tables_town_east_events;
pub mod tables_town_west_entities;
pub mod tables_town_west_events;
pub mod tables_trial;

pub use descriptor::*;
pub(crate) use descriptor::{en, ev, ev_sentinel, gi, hdr, mdl};
pub use lifecycle::*;
pub use riddles_apartments::*;
pub use riddles_hospital_normal::*;
pub use riddles_hospital_otherworld::*;

/// 1-based runtime stage IDs (`1..=27`) dispatched by `StgOverlayGetStageData`
/// (`SLUS_202.28` `0x00228670`, `sh2pc.exe` `0x00494ec0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Act1StageId {
    /// Stage `1`: `stg_tgs_trial.c` (E3/TGS Trial overlay).
    TgsTrial = 1,
    /// Stage `2`: `stg_toilet.c` (Restroom opening mirror scene).
    Toilet = 2,
    /// Stage `3`: `stg_ovservation.c` (Toluca Lake Observation Deck).
    Observation = 3,
    /// Stage `4`: `stg_forest.c` (Forest Trail & Graveyard — Angela meeting).
    Forest = 4,
    /// Stage `5`: `stg_town_east.c` (Silent Hill East — Sanders/Lindsey/Vachss/Katz/Martin St.).
    TownEast = 5,
    /// Stage `6`: `stg_apart_e1f.c` (Wood Side Apartments 1F).
    ApartE1f = 6,
    /// Stage `7`: `stg_apart_e2f.c` (Wood Side Apartments 2F — Clock, Butterfly Hole, Trash Chute).
    ApartE2f = 7,
    /// Stage `8`: `stg_apart_e3fw.c` (Wood Side Apartments 3F West).
    ApartE3fw = 8,
    /// Stage `9`: `stg_apart_e3fe.c` (Wood Side Apartments 3F East — Rm 307 PH Closet).
    ApartE3fe = 9,
    /// Stage `10`: `stg_apart_w1f.c` (Blue Creek Apartments 1F — Rm 105 Coin Desk, Rm 109 Angela).
    ApartW1f = 10,
    /// Stage `11`: `stg_apart_w2f.c` (Blue Creek Apartments 2F — Rm 203 Toilet Wallet & Safe).
    ApartW2f = 11,
    /// Stage `12`: `stg_apart_stair.c` (Apartments Stairwells — Pyramid Head First Boss).
    ApartStair = 12,
    /// Stage `13`: `stg_apart_out.c` (Wood Side Courtyard & Trash Pile).
    ApartOut = 13,
    /// Stage `14`: `stg_town_west.c` (Silent Hill West & Rosewater Park — Maria meeting).
    TownWest = 14,
    /// Stage `15`: `stg_bowling.c` (Pete's Bowl-O-Rama — Eddie & Laura).
    Bowling = 15,
    /// Stage `16`: `stg_to_heaven.c` (Back-Alley Passage to Heaven's Night).
    ToHeaven = 16,
    /// Stage `17`: `stg_heaven_night.c` (Heaven's Night Interior).
    HeavenNight = 17,
    /// Stage `18`: `stg_hospital_1f_f.c` (Brookhaven Hospital 1F Normal).
    Hospital1fNormal = 18,
    /// Stage `19`: `stg_hospital_2f_f.c` (Brookhaven Hospital 2F Normal — Teddy Bear & Carbon Paper).
    Hospital2fNormal = 19,
    /// Stage `20`: `stg_hospital_3f_f.c` (Brookhaven Hospital 3F Normal — Keypad, Louise Box, Shower Drain).
    Hospital3fNormal = 20,
    /// Stage `21`: `stg_hospital_rf_f.c` (Brookhaven Hospital Roof — Diary & Pyramid Head Ambush).
    HospitalRoofNormal = 21,
    /// Stage `22`: `stg_hospital_1fw_b.c` (Brookhaven Hospital 1F West Otherworld — Flesh Lips Boss).
    Hospital1fwOtherworld = 22,
    /// Stage `23`: `stg_hospital_1fe_b.c` (Brookhaven Hospital 1F East Otherworld).
    Hospital1feOtherworld = 23,
    /// Stage `24`: `stg_hospital_2f_b.c` (Brookhaven Hospital 2F Otherworld — Day Room Refrigerator).
    Hospital2fOtherworld = 24,
    /// Stage `25`: `stg_hospital_3f_b.c` (Brookhaven Hospital 3F Otherworld — Quiz Chest & Angel Door).
    Hospital3fOtherworld = 25,
    /// Stage `26`: `stg_hospital_bf_b.c` (Brookhaven Hospital B1 & Trick-or-Treat Elevator Radio Quiz).
    HospitalBfOtherworld = 26,
    /// Stage `27`: `stg_hospital_pass.c` (Brookhaven Basement Corridor — Pyramid Head Chase).
    HospitalPass = 27,
}

impl Act1StageId {
    /// Converts a raw 1-based `stage_id` (`1..=27`) into [`Act1StageId`].
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::TgsTrial),
            2 => Some(Self::Toilet),
            3 => Some(Self::Observation),
            4 => Some(Self::Forest),
            5 => Some(Self::TownEast),
            6 => Some(Self::ApartE1f),
            7 => Some(Self::ApartE2f),
            8 => Some(Self::ApartE3fw),
            9 => Some(Self::ApartE3fe),
            10 => Some(Self::ApartW1f),
            11 => Some(Self::ApartW2f),
            12 => Some(Self::ApartStair),
            13 => Some(Self::ApartOut),
            14 => Some(Self::TownWest),
            15 => Some(Self::Bowling),
            16 => Some(Self::ToHeaven),
            17 => Some(Self::HeavenNight),
            18 => Some(Self::Hospital1fNormal),
            19 => Some(Self::Hospital2fNormal),
            20 => Some(Self::Hospital3fNormal),
            21 => Some(Self::HospitalRoofNormal),
            22 => Some(Self::Hospital1fwOtherworld),
            23 => Some(Self::Hospital1feOtherworld),
            24 => Some(Self::Hospital2fOtherworld),
            25 => Some(Self::Hospital3fOtherworld),
            26 => Some(Self::HospitalBfOtherworld),
            27 => Some(Self::HospitalPass),
            _ => None,
        }
    }
}

/// User-customizable toggle flags for modern SH2:EE fixes in Act 1 (`AGENTS.md` §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act1VanillaFixesConfig {
    /// `OldManCoinFix.cpp`: hides `Coin [Old Man]` in the courtyard until `Canned juice` is dropped (`flag(95)`).
    pub old_man_coin_fix: bool,
    /// `TeddyBearLookFix.cpp`: stops James head-tracking `i_bear` (`0x0727`) after `Bent needle` pickup (`flag(170)`).
    pub teddy_bear_look_fix: bool,
    /// `ShowerRoomFlashlightFix.cpp`: uses player flashlight status in `R_HSP_SHOWER` (`0x40`) to prevent 1-frame flicker.
    pub shower_room_flashlight_fix: bool,
    /// `PuzzleAlignmentFixes.cpp`: aligns coin UVs and slot vertices in `R_APT_W_RM_105` (`0x22`).
    pub coin_puzzle_alignment_fix: bool,
}

impl Default for Act1VanillaFixesConfig {
    fn default() -> Self {
        Self::vanilla_ps2()
    }
}

impl Act1VanillaFixesConfig {
    /// Strict 1:1 original Team Silent PS2 / PC vanilla behavior (all modern fixes disabled).
    #[must_use]
    pub const fn vanilla_ps2() -> Self {
        Self {
            old_man_coin_fix: false,
            teddy_bear_look_fix: false,
            shower_room_flashlight_fix: false,
            coin_puzzle_alignment_fix: false,
        }
    }

    /// Enhanced configuration with all 4 SH2:EE Act 1 bug fixes enabled.
    #[must_use]
    pub const fn enhanced() -> Self {
        Self {
            old_man_coin_fix: true,
            teddy_bear_look_fix: true,
            shower_room_flashlight_fix: true,
            coin_puzzle_alignment_fix: true,
        }
    }
}

/// Ordered table of all 27 Act 1 stage descriptors (`Stages 1..=27`).
pub const ACT1_ALL_STAGES: [Act1StageDescriptor; 27] = [
    tables_trial::ACT1_STAGE_TGS_TRIAL,
    tables_intro::ACT1_STAGE_TOILET,
    tables_intro::ACT1_STAGE_OBSERVATION,
    tables_intro::ACT1_STAGE_FOREST,
    tables_town_east_entities::ACT1_STAGE_TOWN_EAST,
    tables_apart_e1f_e2f::ACT1_STAGE_APART_E1F,
    tables_apart_e1f_e2f::ACT1_STAGE_APART_E2F,
    tables_apart_e3f::ACT1_STAGE_APART_E3FW,
    tables_apart_e3f::ACT1_STAGE_APART_E3FE,
    tables_apart_w1f_w2f::ACT1_STAGE_APART_W1F,
    tables_apart_w1f_w2f::ACT1_STAGE_APART_W2F,
    tables_apart_stair_out::ACT1_STAGE_APART_STAIR,
    tables_apart_stair_out::ACT1_STAGE_APART_OUT,
    tables_town_west_entities::ACT1_STAGE_TOWN_WEST,
    tables_bowling_heaven::ACT1_STAGE_BOWLING,
    tables_bowling_heaven::ACT1_STAGE_TO_HEAVEN,
    tables_bowling_heaven::ACT1_STAGE_HEAVEN_NIGHT,
    tables_hospital_normal_1f_2f::ACT1_STAGE_HOSPITAL_1F_F,
    tables_hospital_normal_1f_2f::ACT1_STAGE_HOSPITAL_2F_F,
    tables_hospital_normal_3f_rf::ACT1_STAGE_HOSPITAL_3F_F,
    tables_hospital_normal_3f_rf::ACT1_STAGE_HOSPITAL_RF_F,
    tables_hospital_otherworld_1f_2f::ACT1_STAGE_HOSPITAL_1FW_B,
    tables_hospital_otherworld_1f_2f::ACT1_STAGE_HOSPITAL_1FE_B,
    tables_hospital_otherworld_1f_2f::ACT1_STAGE_HOSPITAL_2F_B,
    tables_hospital_otherworld_3f_bf::ACT1_STAGE_HOSPITAL_3F_B,
    tables_hospital_otherworld_3f_bf::ACT1_STAGE_HOSPITAL_BF_B,
    tables_hospital_otherworld_3f_bf::ACT1_STAGE_HOSPITAL_PASS,
];

/// Looks up the static retail [`Act1StageDescriptor`] for `stage_id` (`1..=27`).
///
/// Mirrors `StgOverlayGetStageData` (`SLUS_202.28` `0x00228670`, `sh2pc.exe` `0x00494ec0`).
#[must_use]
pub fn act1_stage_descriptor(stage_id: u8) -> Option<&'static Act1StageDescriptor> {
    if (1..=27).contains(&stage_id) {
        Some(&ACT1_ALL_STAGES[(stage_id - 1) as usize])
    } else {
        None
    }
}
