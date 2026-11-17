//! Lakeview Hotel Normal BF, 1F Lobby, and 3F Room 312 Riddles (`Stages 39, 40, 42`).
//!
//! Covers Bar Venus Tears (`Can Opener` / `Lightbulb` / `Bar Key`), Lobby 3 Music Boxes
//! (`EvProgOrgel` `0x0057c2a0`, SH2:EE `MusicBoxVolume.cpp`), and Room 312 VCR `Videotape`
//! (`EvProgMemoryOfMurder` `0x00577dd0`, SH2:EE `VHSAudio.cpp` & `Room312FlashlightFix.cpp`).

use crate::config::RiddleDifficulty;
use crate::events::EventFlags;
use crate::inventory::InventoryManager;

/// Flag `385`: Can of lightbulbs opened with Can Opener (`item 70`, `EvProgGetLightbulb` `0x0057d3a0`).
pub const FLAG_BAR_CAN_OPENED: usize = 385;
/// Flag `386`: Lightbulb (`item 71`) screwed into Bar Venus Tears lantern (`EvProgUseLightbulb` `0x0057d880`).
pub const FLAG_BAR_LANTERN_LIT: usize = 386;
/// Flag `387`: Bar Venus Tears exit unlocked with Bar Key (`item 46`, `EvProgUseBarKey` `0x0057d8b0`).
pub const FLAG_BAR_DOOR_UNLOCKED: usize = 387;

/// Base flag `448` (`0x01f7aa18` bit `0`): Hotel Lobby Music Box state word (`EvProgOrgel` `0x0057c2a0`).
pub const FLAG_MUSIC_BOX_WORD_BASE: usize = 448;
/// Flag `472` (`0x01f7aa18` bit `24`, `0x1d8` in `MusicBoxVolume.cpp`): Music Box puzzle solved & melody playing (`0x0057cc1a`).
pub const FLAG_MUSIC_BOX_SOLVED: usize = 472;

/// Flag `475`: Room 312 VCR Videotape (`murder.sfd`/`.bik`) watched (`EvProgMemoryOfMurder` `0x00577dd0`).
pub const FLAG_ROOM312_VCR_WATCHED: usize = 475;
/// Flag `476`: Videotape (`item 23`) inserted into Room 312 VCR (`Stage 42 ev[0]`).
pub const FLAG_ROOM312_VIDEOTAPE_INSERTED: usize = 476;
/// Flag `527`: Lakeview Hotel transitioned from Normal to Otherworld (`0x00577dd0`).
pub const FLAG_HOTEL_TRANSITIONED_TO_OTHERWORLD: usize = 527;
/// Flag `625`: Room 312 post-VCR dark state (`0x0048e14e` & `0x00577dd0`).
pub const FLAG_ROOM312_POST_VCR_DARK: usize = 625;
/// Flag `627`: Room 312 Laura/Mary radio transition state (`0x00577dd0`).
pub const FLAG_ROOM312_RADIO_EVENT: usize = 627;

/// Inventory item ID: Videotape (`23`).
pub const ITEM_VIDEOTAPE: u8 = 23;
/// Inventory item ID: Key to hotel room 312 (`43`).
pub const ITEM_HOTEL_312_KEY: u8 = 43;
/// Inventory item ID: Bar key (`46`).
pub const ITEM_BAR_KEY: u8 = 46;
/// Inventory item ID: Hotel stairway key (`48`, hidden inside the lobby music box).
pub const ITEM_HOTEL_STAIRWAY_KEY: u8 = 48;
/// Inventory item ID: "Little Mermaid" music box (`67`, model `0x0719`).
pub const ITEM_MUSIC_BOX_LITTLE_MERMAID: u8 = 67;
/// Inventory item ID: "Cinderella" music box (`68`, model `0x071a`).
pub const ITEM_MUSIC_BOX_CINDERELLA: u8 = 68;
/// Inventory item ID: "Snow White" music box (`69`, model `0x0718`).
pub const ITEM_MUSIC_BOX_SNOW_WHITE: u8 = 69;
/// Inventory item ID: Can opener (`70`).
pub const ITEM_CAN_OPENER: u8 = 70;
/// Inventory item ID: Lightbulb (`71`).
pub const ITEM_LIGHTBULB: u8 = 71;

/// Sound effect ID: Room 312 VCR tape insert (`0x44c9` at `0x00577dd0`).
pub const SE_VCR_INSERT: u32 = 0x44c9;
/// Sound effect ID: Music box placed in lobby frame (`0x4dc2` at `0x0057c560`).
pub const SE_MUSIC_BOX_PLACE: u32 = 0x4dc2;
/// BGM track ID: Hotel Lobby Music Box melody (`0x22 = 34` at `0x0057cc30`).
pub const BGM_MUSIC_BOX_MELODY: u32 = 34;

/// Opens the Bar Venus Tears can of lightbulbs using Can Opener (`item 70`, `0x0057d3a0`),
/// consuming `70`, setting `flag 385`, and awarding Lightbulb (`item 71`).
pub fn apply_bar_can_opener(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !inventory.has_item(ITEM_CAN_OPENER) {
        return false;
    }
    inventory.item_lose(ITEM_CAN_OPENER);
    flags.set(FLAG_BAR_CAN_OPENED, true);
    inventory.give_raw(ITEM_LIGHTBULB, 1);
    true
}

/// Places Lightbulb (`item 71`) into the Bar Venus Tears bar-side lantern (`0x0057d880`),
/// consuming `71` and setting `flag 386` so the locked door key-hole becomes visible.
pub fn apply_bar_lightbulb(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !inventory.has_item(ITEM_LIGHTBULB) {
        return false;
    }
    inventory.item_lose(ITEM_LIGHTBULB);
    flags.set(FLAG_BAR_LANTERN_LIT, true);
    true
}

/// Unlocks Bar Venus Tears exit with Bar Key (`item 46`, `0x0057d8b0`), requiring `flag 386`.
pub fn apply_bar_key(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !flags.get(FLAG_BAR_LANTERN_LIT) || !inventory.has_item(ITEM_BAR_KEY) {
        return false;
    }
    flags.set(FLAG_BAR_DOOR_UNLOCKED, true);
    true
}

/// Which of the three Music Boxes is seated in a Hotel Lobby slot (`EvProgOrgel` `0x0057c2a0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicBoxKind {
    /// Item `69` (`0x45`, model `0x0718`): "Snow White" music box (`0x0057c510`, nibble `0x3`).
    SnowWhite,
    /// Item `67` (`0x43`, model `0x0719`): "Little Mermaid" music box (`0x0057c60c`, nibble `0x5`).
    LittleMermaid,
    /// Item `68` (`0x44`, model `0x071a`): "Cinderella" music box (`0x0057c6dc`, nibble `0x9`).
    Cinderella,
}

impl MusicBoxKind {
    /// Converts inventory item ID (`67..=69`) to [`MusicBoxKind`].
    pub const fn from_item_id(item_id: u8) -> Option<Self> {
        match item_id {
            ITEM_MUSIC_BOX_CINDERELLA => Some(Self::Cinderella),
            ITEM_MUSIC_BOX_LITTLE_MERMAID => Some(Self::LittleMermaid),
            ITEM_MUSIC_BOX_SNOW_WHITE => Some(Self::SnowWhite),
            _ => None,
        }
    }

    /// Corresponding inventory item ID (`67..=69`).
    pub const fn item_id(self) -> u8 {
        match self {
            Self::Cinderella => ITEM_MUSIC_BOX_CINDERELLA,
            Self::LittleMermaid => ITEM_MUSIC_BOX_LITTLE_MERMAID,
            Self::SnowWhite => ITEM_MUSIC_BOX_SNOW_WHITE,
        }
    }

    const fn seat_nibble(self) -> u32 {
        match self {
            Self::SnowWhite => 0x3,
            Self::LittleMermaid => 0x5,
            Self::Cinderella => 0x9,
        }
    }
}

/// Interactive state of the Lakeview Hotel 1F Lobby 3 Music Boxes puzzle (`EvProgOrgel` `0x0057c2a0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HotelMusicBoxPuzzle {
    seats: [Option<MusicBoxKind>; 3],
}

/// Result of pressing the Hotel Lobby Music Box play switch (`0x0057c980..0x0057cc50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicBoxPlayResult {
    /// `true` if all three seats satisfy `riddle` difficulty (`flag 472` set, Hotel Stairway Key `48` awarded).
    pub solved: bool,
    /// BGM track started (`Some(34)` = `0x22` `"bgm_112.aix"` when solved).
    pub bgm_id: Option<u32>,
    /// Stage BGM channel bank selected for Lakeview Hotel 2F (`4` = `0x0010_0000` track 7 when
    /// `music_box_volume_fix` is enabled per SH2:EE `MusicBoxVolume.cpp`, `0` in vanilla PC).
    pub hotel_2f_bgm_bank: u8,
}

impl HotelMusicBoxPuzzle {
    /// Creates an empty 3-seat Music Box frame.
    pub const fn new() -> Self {
        Self { seats: [None; 3] }
    }

    /// Places `box_kind` into `seat` (`0` = left, `1` = center, `2` = right, `0x0057c510`),
    /// consuming the item from `inventory` and updating `0x01f7aa18` (`flags 448..=471`).
    pub fn place_box(
        &mut self,
        seat: usize,
        box_kind: MusicBoxKind,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> Option<u32> {
        if seat >= 3 || self.seats[seat].is_some() || !inventory.has_item(box_kind.item_id()) {
            return None;
        }
        inventory.item_lose(box_kind.item_id());
        self.seats[seat] = Some(box_kind);
        self.sync_flags(flags);
        Some(SE_MUSIC_BOX_PLACE)
    }

    /// Removes a single seated box at `seat` (`0..=2`) back to `inventory` (`0x0057c7a9..0x0057c8f3`).
    pub fn remove_box(
        &mut self,
        seat: usize,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> Option<MusicBoxKind> {
        if seat >= 3 {
            return None;
        }
        let kind = self.seats[seat].take()?;
        inventory.give_raw(kind.item_id(), 1);
        self.sync_flags(flags);
        Some(kind)
    }

    /// Retrieves all currently seated boxes back to `inventory`.
    pub fn return_boxes_to_inventory(
        &mut self,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) {
        for slot in &mut self.seats {
            if let Some(kind) = slot.take() {
                inventory.give_raw(kind.item_id(), 1);
            }
        }
        self.sync_flags(flags);
    }

    /// Evaluates the 3 seats against `riddle` difficulty (`sh2pc.exe` `0x0057c983..0x0057c9dc`):
    /// - `Easy`: any permutation of all 3 boxes (`0x1000 && 0x10000 && 0x100000`).
    /// - `Normal`: `[LittleMermaid, SnowWhite, Cinderella]` (`0x4000 && 0x20000 && 0x800000`).
    /// - `Hard` / `Extra`: `[SnowWhite, LittleMermaid, Cinderella]` (`0x2000 && 0x40000 && 0x800000`).
    pub fn play_melody(
        &mut self,
        riddle: RiddleDifficulty,
        music_box_volume_fix: bool,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> MusicBoxPlayResult {
        let solved = match riddle {
            RiddleDifficulty::Easy => self.seats.iter().all(Option::is_some),
            RiddleDifficulty::Normal => {
                self.seats
                    == [
                        Some(MusicBoxKind::LittleMermaid),
                        Some(MusicBoxKind::SnowWhite),
                        Some(MusicBoxKind::Cinderella),
                    ]
            }
            RiddleDifficulty::Hard | RiddleDifficulty::Extra => {
                self.seats
                    == [
                        Some(MusicBoxKind::SnowWhite),
                        Some(MusicBoxKind::LittleMermaid),
                        Some(MusicBoxKind::Cinderella),
                    ]
            }
        };
        if solved {
            flags.set(FLAG_MUSIC_BOX_SOLVED, true);
            inventory.give_raw(ITEM_HOTEL_STAIRWAY_KEY, 1);
            let hotel_2f_bgm_bank = if music_box_volume_fix { 4 } else { 0 };
            MusicBoxPlayResult {
                solved: true,
                bgm_id: Some(BGM_MUSIC_BOX_MELODY),
                hotel_2f_bgm_bank,
            }
        } else {
            MusicBoxPlayResult {
                solved: false,
                bgm_id: None,
                hotel_2f_bgm_bank: 0,
            }
        }
    }

    fn sync_flags(&self, flags: &mut EventFlags) {
        for bit in 0..24 {
            flags.set(FLAG_MUSIC_BOX_WORD_BASE + bit, false);
        }
        for (seat, entry) in self.seats.iter().enumerate() {
            if let Some(kind) = entry {
                flags.set(FLAG_MUSIC_BOX_WORD_BASE + seat, true);
                let nib = kind.seat_nibble();
                let shift_base = 12 + seat * 4;
                for b in 0..4 {
                    if ((nib >> b) & 1) != 0 {
                        flags.set(FLAG_MUSIC_BOX_WORD_BASE + shift_base + b, true);
                    }
                }
            }
        }
    }
}

/// Result of playing `Videotape` (`item 23`) in Room 312's VCR (`EvProgMemoryOfMurder` `0x00577dd0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room312VcrPlayback {
    /// FMV stream asset (`"murder.sfd"` / `"murder.bik"`).
    pub movie_name: &'static str,
    /// Selected FMV audio stream index (`1` when `vhs_second_audio_track` is enabled per `VHSAudio.cpp`,
    /// `0` in vanilla PC).
    pub audio_track_index: u8,
    /// Destination stage after the cutscene (`Act2StageId::Hotel3fOtherworld = 46`).
    pub next_stage_id: u16,
    /// Insert sound effect (`0x44c9`).
    pub se_id: u32,
}

/// Inserts `Videotape` (`item 23`) into the Room 312 VCR (`0x00577dd0`), setting transition flags
/// `475, 476, 527, 625, 627` and returning [`Room312VcrPlayback`].
pub fn trigger_room312_vcr(
    vhs_second_audio_track: bool,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<Room312VcrPlayback> {
    if !inventory.has_item(ITEM_VIDEOTAPE) {
        return None;
    }
    inventory.item_lose(ITEM_VIDEOTAPE);
    for flag in [
        FLAG_ROOM312_VCR_WATCHED,
        FLAG_ROOM312_VIDEOTAPE_INSERTED,
        FLAG_HOTEL_TRANSITIONED_TO_OTHERWORLD,
        FLAG_ROOM312_POST_VCR_DARK,
        FLAG_ROOM312_RADIO_EVENT,
    ] {
        flags.set(flag, true);
    }
    Some(Room312VcrPlayback {
        movie_name: "murder.sfd",
        audio_track_index: u8::from(vhs_second_audio_track),
        next_stage_id: 46,
        se_id: SE_VCR_INSERT,
    })
}

/// Evaluates whether the flashlight toggle is permitted in Room 312 (`room 0xa2`, `0x0048ebc8`,
/// SH2:EE `Room312FlashlightFix.cpp`).
pub const fn can_toggle_flashlight_in_room312(
    in_room_312_after_vcr: bool,
    in_inventory_menu: bool,
    room_312_flashlight_fix: bool,
) -> bool {
    if !in_room_312_after_vcr {
        return true;
    }
    in_inventory_menu && !room_312_flashlight_fix
}
