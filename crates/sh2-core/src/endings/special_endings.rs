//! Special Endings: Rebirth Ritual Items, Dog Key Observation Room, and Blue Gem 3-Location UFO Sequence.
//!
//! Verified 1:1 against:
//! - PS2 Prototype ELF (`SLUS_202.28`):
//!   - `EvProgUseDogKey` (`0x01f01e80`, `stg_hotel_3f_b.c`) & `EvProgEndingDog` (`0x01f01e80`, `stg_end_dog.c`).
//! - Retail PC Binary (`sh2pc.exe` v1.0 Director's Cut):
//!   - Rebirth 4 ritual items (`74, 75, 76, 77` checked at `0x00576491..0x005764c8` & `0x00576658`).
//!   - Dog Key pickup (`Stage 14 ev[9]` @ `0x008db428`, Room `0x0060`) & Observation Room unlock
//!     (`Stage 46 ev[8]` @ `0x008c7178`, Room `0x006e`, `EvProgUseDogKey` `0x00575910` -> Stage 52 `0x00574430`).
//!   - Blue Gem pickup (`Stage 2 ev[2]` @ `0x008e7d08`, Room `0x0024`, sets `Flag 724`).
//!   - Blue Gem Location 1: Dark Brookhaven Garden (`Stage 22 ev[24]` @ `0x008d6f98`, Room `0x0120`,
//!     `EvProgUfoNiwa` `0x0058c2e0`, `ufoniwa01..04.tex`, stream `0xeafa`, sets `Flag 725`).
//!   - Blue Gem Location 2: Toluca Lake Dock (`Stage 38 ev[11]` @ `0x008cc8a8`, Room `0x00b2`,
//!     `EvProgUfoMizu` `0x0057e820`, `ufomizu01..04.tex`, stream `0xeafa`, sets `Flag 726`).
//!   - Blue Gem Location 3: Lakeview Hotel Room 312 pre-VCR (`Stage 42 ev[21]` @ `0x008c9040`, Room `0x00ca`,
//!     requires `Flag 726 && !Flag 476`, `EvProgUfoMado` `0x00578c50`, 26 textures at `0x008c95a8`,
//!     35 comic/staff slides at `0x008c9490..0x008c95a8`, stream `0xeafc`, sets `Flag 727`, records `0x20`).

use crate::events::EventFlags;

use super::playing_info::{FLAG_ENDING_DOG, FLAG_ENDING_UFO};
use super::secret_weapons::{FLAG_SPAWN_BLUE_GEM, FLAG_SPAWN_DOG_KEY};

/// Item ID: `Dog key` (`25` in `ITEM_CATALOG`, `0x19` at `0x00575920`).
pub const ITEM_DOG_KEY: u8 = 25;
/// Item ID: `Book: "Lost Memories"` (`74` in `ITEM_CATALOG`, bit `10` of `item.flag[2]` at `0x00576491`).
pub const ITEM_BOOK_LOST_MEMORIES: u8 = 74;
/// Item ID: `Book: "Crimson Ceremony"` (`75` in `ITEM_CATALOG`, bit `11` at `0x005764b2`).
pub const ITEM_BOOK_CRIMSON_CEREMONY: u8 = 75;
/// Item ID: `White chrism` (`76` in `ITEM_CATALOG`, bit `12` at `0x005764bc`).
pub const ITEM_WHITE_CHRISM: u8 = 76;
/// Item ID: `Obsidian goblet` (`77` in `ITEM_CATALOG`, bit `13` at `0x005764c3`).
pub const ITEM_OBSIDIAN_GOBLET: u8 = 77;
/// Item ID: `Blue Gem` (`78` in `ITEM_CATALOG`, `0x4e` in `Stage 22/38/42` `EventListEntry`).
pub const ITEM_BLUE_GEM: u8 = 78;

/// All 4 ritual item IDs required for the *Rebirth* ending (`0x00576491..0x005764c8`).
pub const REBIRTH_RITUAL_ITEM_IDS: [u8; 4] = [
    ITEM_WHITE_CHRISM,
    ITEM_BOOK_LOST_MEMORIES,
    ITEM_OBSIDIAN_GOBLET,
    ITEM_BOOK_CRIMSON_CEREMONY,
];

/// Event flag: Videotape watched in Hotel Room 312 (`Flag 476` in `Stage 42 ev[21]` @ `0x008c9040`; blocks UFO ending).
pub const FLAG_HOTEL_VIDEOTAPE_WATCHED: usize = 476;
/// Event flag: `Dog key` picked up (`Flag 515` in `Stage 14 ev[9]` @ `0x008db428`).
pub const FLAG_DOG_KEY_PICKED_UP: usize = 515;
/// Event flag: Hotel 3F Observation Room unlocked with `Dog key` (`Flag 516` in `Stage 46 ev[8]` @ `0x008c7178`).
pub const FLAG_OBSERVATION_ROOM_UNLOCKED: usize = 516;
/// Event flag: `Blue Gem` picked up in Observation Deck restroom (`Flag 724` in `Stage 2 ev[2]` @ `0x008e7d08`).
pub const FLAG_BLUE_GEM_PICKED_UP: usize = 724;
/// Event flag: `Blue Gem` pulsed at Dark Brookhaven Garden (`Flag 725` in `Stage 22 ev[24]` @ `0x008d6f98`).
pub const FLAG_BLUE_GEM_HOSPITAL_GARDEN: usize = 725;
/// Event flag: `Blue Gem` pulsed at Toluca Lake Dock (`Flag 726` in `Stage 38 ev[11]` @ `0x008cc8a8`).
pub const FLAG_BLUE_GEM_TOLUCA_DOCK: usize = 726;

/// Room ID: Observation Deck Restroom (`0x0024` in `Stage 2 ev[2]` @ `0x008e7d08`).
pub const ROOM_RESTROOM_BLUE_GEM: u16 = 0x0024;
/// Room ID: West South Vale doghouse lot (`0x0060` in `Stage 14 ev[9]` @ `0x008db428`).
pub const ROOM_WEST_TOWN_DOGHOUSE: u16 = 0x0060;
/// Room ID: Hotel 3F corridor outside Observation Room (`0x006e` in `Stage 46 ev[8]` @ `0x008c7178`).
pub const ROOM_HOTEL_3F_OBSERVATION_CORRIDOR: u16 = 0x006e;
/// Room ID: Dark Brookhaven Hospital Garden (`0x0120` in `Stage 22 ev[24]` @ `0x008d6f98`).
pub const ROOM_DARK_HOSPITAL_GARDEN: u16 = 0x0120;
/// Room ID: Toluca Lake Dock before rowboat (`0x00b2` in `Stage 38 ev[11]` @ `0x008cc8a8`).
pub const ROOM_TOLUCA_LAKE_DOCK: u16 = 0x00b2;
/// Room ID: Lakeview Hotel 3F Room 312 (`0x00ca` in `Stage 42 ev[21]` @ `0x008c9040`).
pub const ROOM_HOTEL_312: u16 = 0x00ca;

/// Sound effect ID played when unlocking the Observation Room door with `Dog key` (`0x4270` at `0x00575919`).
pub const SE_DOG_KEY_UNLOCK: u32 = 0x4270;
/// Voice/stream ID for Blue Gem UFO pulsing hum (`0xeafa` = `60154` = `voice.afs[154]` at `0x0057e900`).
pub const SE_UFO_GEM_PULSE_STREAM: u32 = 0xeafa;
/// Voice/stream ID for the full UFO Ending comic + Silent Hill Song sequence (`0xeafc` = `60156` = `voice.afs[156]` at `0x00578d32`).
pub const SE_UFO_ENDING_STREAM: u32 = 0xeafc;

/// Returns how many of the 4 *Rebirth* ritual items (`74..=77`) are present in `owned_item_ids` (`0..=4`).
pub fn count_rebirth_ritual_items(owned_item_ids: &[u8]) -> u8 {
    let mut count = 0u8;
    for ritual_id in REBIRTH_RITUAL_ITEM_IDS {
        if owned_item_ids.contains(&ritual_id) {
            count += 1;
        }
    }
    count
}

/// Evaluates using the `Dog key` (`25`) in Lakeview Hotel 3F (`Stage 46 ev[8]` @ `0x008c7178`, `EvProgUseDogKey` `0x00575910`).
///
/// When valid, sets `FLAG_OBSERVATION_ROOM_UNLOCKED` (`516`) and `FLAG_ENDING_DOG` (`622`) and transitions to Stage `52`.
pub fn try_trigger_dog_ending(
    current_stage: u8,
    current_room: u16,
    has_dog_key: bool,
    flags: &mut EventFlags,
) -> bool {
    if current_stage == 46
        && current_room == ROOM_HOTEL_3F_OBSERVATION_CORRIDOR
        && has_dog_key
        && flags.get(FLAG_SPAWN_DOG_KEY)
    {
        flags.set(FLAG_OBSERVATION_ROOM_UNLOCKED, true);
        flags.set(FLAG_ENDING_DOG, true);
        true
    } else {
        false
    }
}

/// Each of the 3 sequential locations where the `Blue Gem` (`78`) resonates (`Stage 22 -> Stage 38 -> Stage 42`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueGemLocation {
    /// Step 1: Dark Brookhaven Hospital courtyard garden after Flesh Lips (`Stage 22` / `24`, Room `0x0120`).
    HospitalGarden,
    /// Step 2: Toluca Lake dock after Eddie boss fight, before boarding boat (`Stage 38` / `39`, Room `0x00b2`).
    TolucaLakeDock,
    /// Step 3: Lakeview Hotel Room 312 window BEFORE watching the videotape (`Stage 42` / `43`, Room `0x00ca`).
    HotelRoom312,
}

impl BlueGemLocation {
    /// Four `.tex` overlay frames used for the Blue Gem pulsing vignette (`0x007d6638..0x007d66a8` / `0x007d6608..0x007d6628`).
    pub const fn vignette_textures(self) -> &'static [&'static str] {
        match self {
            Self::HospitalGarden => &[
                "data/pic/ufo/ufoniwa01.tex",
                "data/pic/ufo/ufoniwa02.tex",
                "data/pic/ufo/ufoniwa03.tex",
                "data/pic/ufo/ufoniwa04.tex",
            ],
            Self::TolucaLakeDock => &[
                "data/pic/ufo/ufomizu01.tex",
                "data/pic/ufo/ufomizu02.tex",
                "data/pic/ufo/ufomizu03.tex",
                "data/pic/ufo/ufomizu04.tex",
            ],
            Self::HotelRoom312 => &[
                "data/pic/ufo/ufomado01.tex",
                "data/pic/ufo/ufomado02.tex",
                "data/pic/ufo/ufomado03.tex",
            ],
        }
    }
}

/// Outcome of using the `Blue Gem` (`78`) at the player's current stage and room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueGemUseResult {
    /// Nothing happens in this room or prerequisites were not met (or Room 312 videotape was already watched).
    NoEffect,
    /// Blue Gem resonated at Location 1 or 2 (`SE_UFO_GEM_PULSE_STREAM` `0xeafa`), advancing the quest flag.
    Pulsed(BlueGemLocation),
    /// Blue Gem resonated a 3rd time in Room 312 pre-VCR, launching the UFO Ending comic-slide sequence!
    TriggeredUfoEnding,
}

/// Evaluates using the `Blue Gem` (`Item 78`) and updates [`EventFlags`] (`725`, `726`, `727`).
pub fn use_blue_gem(
    current_stage: u8,
    current_room: u16,
    has_blue_gem: bool,
    flags: &mut EventFlags,
) -> BlueGemUseResult {
    if !has_blue_gem || !flags.get(FLAG_SPAWN_BLUE_GEM) {
        return BlueGemUseResult::NoEffect;
    }

    if matches!(current_stage, 22 | 24)
        && current_room == ROOM_DARK_HOSPITAL_GARDEN
        && !flags.get(FLAG_BLUE_GEM_HOSPITAL_GARDEN)
    {
        flags.set(FLAG_BLUE_GEM_HOSPITAL_GARDEN, true);
        return BlueGemUseResult::Pulsed(BlueGemLocation::HospitalGarden);
    }

    if matches!(current_stage, 38 | 39)
        && current_room == ROOM_TOLUCA_LAKE_DOCK
        && flags.get(FLAG_BLUE_GEM_HOSPITAL_GARDEN)
        && !flags.get(FLAG_BLUE_GEM_TOLUCA_DOCK)
    {
        flags.set(FLAG_BLUE_GEM_TOLUCA_DOCK, true);
        return BlueGemUseResult::Pulsed(BlueGemLocation::TolucaLakeDock);
    }

    if matches!(current_stage, 42 | 43)
        && current_room == ROOM_HOTEL_312
        && flags.get(FLAG_BLUE_GEM_TOLUCA_DOCK)
        && !flags.get(FLAG_HOTEL_VIDEOTAPE_WATCHED)
    {
        flags.set(FLAG_ENDING_UFO, true);
        return BlueGemUseResult::TriggeredUfoEnding;
    }

    BlueGemUseResult::NoEffect
}
