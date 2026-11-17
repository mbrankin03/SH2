//! Lakeview Hotel Otherworld (`Stages 43..=47`, `Stage 16`) & *Born from a Wish* (`Stages 53..=58`)
//! Riddles, Room Triggers, and Gravestone 3-Board Puzzle.
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28` (`stg_hotel_bf_b.c`..`stg_hotel_fire.c`,
//! `stg_to_heaven.c`), retail PC Director's Cut `sh2pc.exe` (`0x00570c70..0x00577b70`), and
//! SH2:EE `PuzzleAlignmentFixes.cpp` (`GravestoneBoardsFix`).

use super::{BoardRotation, GravestoneBoard};
use crate::events::EventFlags;
use crate::inventory::InventoryManager;

/// Flag `485` (`0x01f7aa1c` bit `5`): Left door examined in Otherworld Hotel 1F (`EvProgEggHoleOnDoor` `0x005768c0`).
pub const FLAG_EGG_DOOR_LEFT_TARGET: usize = 485;
/// Flag `486` (`0x01f7aa1c` bit `6`): Right door examined in Otherworld Hotel 1F (`0x005768c0`).
pub const FLAG_EGG_DOOR_RIGHT_TARGET: usize = 486;
/// Flag `491` (`0x01f7aa1c` bit `11` = `0x800`): Rust-Colored Egg (`item 72`, model `0x0740`) mounted on left door (`0x00576c13`).
pub const FLAG_EGG_RUST_PLACED_LEFT: usize = 491;
/// Flag `492` (`0x01f7aa1c` bit `12` = `0x1000`): Scarlet Egg (`item 73`, model `0x0741`) mounted on left door (`0x00576c45`).
pub const FLAG_EGG_SCARLET_PLACED_LEFT: usize = 492;
/// Flag `493` (`0x01f7aa1c` bit `13` = `0x2000`): Rust-Colored Egg (`item 72`, model `0x0740`) mounted on right door (`0x00576c77`).
pub const FLAG_EGG_RUST_PLACED_RIGHT: usize = 493;
/// Flag `494` (`0x01f7aa1c` bit `14` = `0x4000`): Scarlet Egg (`item 73`, model `0x0741`) mounted on right door (`0x00576ca9`).
pub const FLAG_EGG_SCARLET_PLACED_RIGHT: usize = 494;
/// Flag `497` (`0x1f1`): Rust-Colored Egg (`item 72`) inserted (`Stage 44 ev[21,27]`).
pub const FLAG_RUST_EGG_INSERTED: usize = 497;
/// Flag `498` (`0x1f2`): Scarlet Egg (`item 73`) inserted (`Stage 44 ev[22,28]`).
pub const FLAG_SCARLET_EGG_INSERTED: usize = 498;
/// Flag `500` (`0x01f7aa1c` bit `20`): 2F Reading Room (`room 0xb0`) headphone tape recorder heard (`EvProgNotifyDeath` `0x005759b0`).
pub const FLAG_READING_ROOM_HEADPHONES_HEARD: usize = 500;
/// Flag `516`: Dog Key (`item 25`) unlocked 3F Observation Room door (`Stage 46 ev[8]`, `EvProgUseDogKey` `0x00575910`).
pub const FLAG_DOG_KEY_DOOR_UNLOCKED: usize = 516;
/// Flag `726`: Blue Gem (`item 78`) Toluca Lake dock UFO channeling (`Stage 38 ev[11]`).
pub const FLAG_TOLUCA_DOCK_BLUE_GEM_USED: usize = 726;

/// Flag `669`: *Born from a Wish* Heaven's Night Chinese Cleaver (`item 17`) / Revolver event (`Stage 53`).
pub const FLAG_WISH_HEAVENS_NIGHT_READY: usize = 669;
/// Base flag `672` (`0x01f7aa34` bit `0`): Baldwin Mansion Gravestone 3-Board state word (`0x00572d40`).
pub const FLAG_WISH_GRAVESTONE_WORD_BASE: usize = 672;
/// Flag `704` (`0x01f7aa38` bit `0`): Baldwin Mansion Gravestone 3-Board puzzle solved (`0x00573717`).
pub const FLAG_WISH_GRAVESTONE_SOLVED: usize = 704;
/// Flag `705`: Acacia Key (`item 82`) unlocked Baldwin Mansion 2F Attic door (`Stage 56 ev[33]`).
pub const FLAG_WISH_ACACIA_DOOR_UNLOCKED: usize = 705;
/// Flag `707`: Mansion 2F unlit candlestick inspected (`Stage 56 ev[50..51]`).
pub const FLAG_WISH_CANDLE_INSPECTED: usize = 707;
/// Flag `709`: Matches (`item 83`) lit Mansion 2F attic candlestick (`Stage 56 ev[53]`).
pub const FLAG_WISH_CANDLE_LIT: usize = 709;
/// Flag `710` (`0x01f7aa38` bit `6`): Birthday Card (`item 84`) picked up (`0x00571181`).
pub const FLAG_WISH_BIRTHDAY_CARD_FOUND: usize = 710;
/// Flag `722` (`0x01f7aa38` bit `18`): White Liquid (`item 85`) delivered to Ernest (`0x005728d6`).
pub const FLAG_WISH_ERNEST_LIQUID_DELIVERED: usize = 722;

/// Inventory item ID: Chinese cleaver (`17`, model `0x074a`).
pub const ITEM_CHINESE_CLEAVER: u8 = 17;
/// Inventory item ID: Dog key (`25`, `Stage 46 ev[8]`).
pub const ITEM_DOG_KEY: u8 = 25;
/// Inventory item ID: Rust-colored egg (`72`, `ITEM_CATALOG[71]`).
pub const ITEM_RUST_COLORED_EGG: u8 = 72;
/// Inventory item ID: Scarlet egg (`73`, `ITEM_CATALOG[72]`).
pub const ITEM_SCARLET_EGG: u8 = 73;
/// Inventory item ID: Book: "Crimson Ceremony" (`75`, model `0x0713`).
pub const ITEM_CRIMSON_CEREMONY: u8 = 75;
/// Inventory item ID: Blue Gem (`78`, model `0x074b`).
pub const ITEM_BLUE_GEM: u8 = 78;
/// Inventory item ID: White Board (`79`, `0x4f` in `sh2pc.exe` `0x005734f0`, placed model `0x0753`).
pub const ITEM_WHITE_BOARD: u8 = 79;
/// Inventory item ID: Black Board (`80`, `0x50` in `sh2pc.exe` `0x00573516`, placed model `0x0754`).
pub const ITEM_BLACK_BOARD: u8 = 80;
/// Inventory item ID: Red Board (`81`, `0x51` in `sh2pc.exe` `0x00573533`, placed model `0x0755`).
pub const ITEM_RED_BOARD: u8 = 81;
/// Inventory item ID: Acacia key (`82`, model `0x074f`).
pub const ITEM_ACACIA_KEY: u8 = 82;
/// Inventory item ID: Matches (`83`, model `0x0750`).
pub const ITEM_MATCHES: u8 = 83;
/// Inventory item ID: Birthday card and present (`84`, model `0x0748`).
pub const ITEM_BIRTHDAY_CARD: u8 = 84;
/// Inventory item ID: White liquid (`85`, `0x55` at `sh2pc.exe` `0x00570d22`).
pub const ITEM_WHITE_LIQUID: u8 = 85;

/// Sound effect ID: Gravestone board removed (`0x4290` at `0x00573430`).
pub const SE_GRAVESTONE_REMOVE_BOARD: u32 = 0x4290;
/// Sound effect ID: Gravestone board placed (`0x4291` at `0x00573073`).
pub const SE_GRAVESTONE_PLACE_BOARD: u32 = 0x4291;
/// Sound effect ID: Gravestone cavity unlocked (`0x4292` at `0x005736d1`).
pub const SE_GRAVESTONE_SOLVED: u32 = 0x4292;
/// Sound effect ID: Egg inserted into 1F Otherworld door (`0x44c7` at `0x00576a90`).
pub const SE_EGG_DOOR_INSERT: u32 = 0x44c7;
/// Sound effect ID: Egg door unlocked when both eggs are mounted (`0x4a3c` at `0x00576bf0`).
pub const SE_EGG_DOOR_UNLOCK: u32 = 0x4a3c;

/// Exact 17-entry teleport table for Otherworld Hotel 2F rooms `0x29..=0x39` (`sh2pc.exe`
/// `0x00575dd2..0x00575e04`): index bytes at `0x00575e10` are
/// `[0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0]`, where jump target `0` (`0x00575ded`)
/// executes the teleport (`call 0x0047b210`) and target `1` (`0x00575e01`) returns without it.
/// Only rooms `0x29`, `0x2d`, `0x31`, and `0x39` teleport.
pub const HOTEL_2F_OTHERWORLD_TELEPORT_MASK: [bool; 17] = [
    true, false, false, false, true, false, false, false, true, false, false, false, false, false,
    false, false, true,
];

/// Inserts Rust-Colored Egg (`72`) or Scarlet Egg (`73`) into the Otherworld Hotel 1F doors
/// (`EvProgEggHoleOnDoor` `sh2pc.exe` `0x005768c0`); returns `(both_placed, se_id)`.
pub fn apply_otherworld_hotel_egg(
    used_item_id: u8,
    right_door: bool,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<(bool, u32)> {
    let door_occupied = if right_door {
        flags.get(FLAG_EGG_RUST_PLACED_RIGHT) || flags.get(FLAG_EGG_SCARLET_PLACED_RIGHT)
    } else {
        flags.get(FLAG_EGG_RUST_PLACED_LEFT) || flags.get(FLAG_EGG_SCARLET_PLACED_LEFT)
    };
    if door_occupied
        || !matches!(used_item_id, ITEM_SCARLET_EGG | ITEM_RUST_COLORED_EGG)
        || !inventory.has_item(used_item_id)
    {
        return None;
    }
    inventory.item_lose(used_item_id);
    flags.set(FLAG_EGG_DOOR_LEFT_TARGET, !right_door);
    flags.set(FLAG_EGG_DOOR_RIGHT_TARGET, right_door);
    match (used_item_id, right_door) {
        (ITEM_SCARLET_EGG, false) => {
            flags.set(FLAG_EGG_SCARLET_PLACED_LEFT, true);
            flags.set(FLAG_SCARLET_EGG_INSERTED, true);
        }
        (ITEM_RUST_COLORED_EGG, false) => {
            flags.set(FLAG_EGG_RUST_PLACED_LEFT, true);
            flags.set(FLAG_RUST_EGG_INSERTED, true);
        }
        (ITEM_SCARLET_EGG, true) => {
            flags.set(FLAG_EGG_SCARLET_PLACED_RIGHT, true);
            flags.set(FLAG_SCARLET_EGG_INSERTED, true);
        }
        (ITEM_RUST_COLORED_EGG, true) => {
            flags.set(FLAG_EGG_RUST_PLACED_RIGHT, true);
            flags.set(FLAG_RUST_EGG_INSERTED, true);
        }
        _ => return None,
    }
    let both_placed = flags.get(FLAG_SCARLET_EGG_INSERTED) && flags.get(FLAG_RUST_EGG_INSERTED);
    let se_id = if both_placed {
        SE_EGG_DOOR_UNLOCK
    } else {
        SE_EGG_DOOR_INSERT
    };
    Some((both_placed, se_id))
}

/// Triggers the Otherworld Hotel 2F Reading Room (`room 0xb0`) Headphones tape recorder conversation
/// (`EvProgNotifyDeath` `sh2pc.exe` `0x005759b0`), gated so it plays once (`flag 500`).
pub fn trigger_reading_room_headphones(flags: &mut EventFlags) -> bool {
    if flags.get(FLAG_READING_ROOM_HEADPHONES_HEARD) {
        return false;
    }
    flags.set(FLAG_READING_ROOM_HEADPHONES_HEARD, true);
    true
}

/// Returns `true` if `room_low_byte` (`0x29..=0x39`) in Otherworld Hotel 2F triggers a non-Euclidean
/// corridor teleport (`sh2pc.exe` `0x00575dd2`, table `0x00575e10`).
pub const fn is_hotel_2f_teleport_corridor_room(room_low_byte: u8) -> bool {
    if room_low_byte < 0x29 || room_low_byte > 0x39 {
        return false;
    }
    HOTEL_2F_OTHERWORLD_TELEPORT_MASK[(room_low_byte - 0x29) as usize]
}

/// Uses the Dog Key (`item 25`) on the 3F Observation Room door (`Stage 46 ev[8]`, `EvProgUseDogKey` `0x00575910`).
pub fn apply_hotel_dog_key(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !inventory.has_item(ITEM_DOG_KEY) {
        return false;
    }
    inventory.item_lose(ITEM_DOG_KEY);
    flags.set(FLAG_DOG_KEY_DOOR_UNLOCKED, true);
    true
}

/// LIFO stack & orientation state of the *Born from a Wish* Baldwin Mansion gravestone puzzle (`0x00572d40`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WishGravestonePuzzle {
    stack: [Option<(GravestoneBoard, BoardRotation)>; 3],
    len: usize,
}

impl WishGravestonePuzzle {
    /// Creates an empty gravestone cavity.
    pub const fn new() -> Self {
        Self {
            stack: [None; 3],
            len: 0,
        }
    }

    /// Places `board` at `rotation` (`0x00572def..0x00573080`), consuming `board.item_id()`.
    /// When all 3 boards are inserted and satisfy any of the 4 rotationally-equivalent solutions
    /// (`Black == White` and `Red == Left90` relative to `Black`, `0x0057328e..0x005732d7`),
    /// sets `flag 704` and awards `Acacia Key` (`item 82`).
    pub fn place_board(
        &mut self,
        board: GravestoneBoard,
        rotation: BoardRotation,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> Option<(bool, u32)> {
        if self.len >= 3
            || self.stack[..self.len]
                .iter()
                .any(|s| matches!(s, Some((b, _)) if *b == board))
            || !inventory.has_item(board.item_id())
        {
            return None;
        }
        inventory.item_lose(board.item_id());
        self.stack[self.len] = Some((board, rotation));
        self.len += 1;
        self.sync_word_flags(flags);

        if self.len == 3 && self.is_winning_arrangement() {
            flags.set(FLAG_WISH_GRAVESTONE_SOLVED, true);
            inventory.give_raw(ITEM_ACACIA_KEY, 1);
            Some((true, SE_GRAVESTONE_SOLVED))
        } else {
            Some((false, SE_GRAVESTONE_PLACE_BOARD))
        }
    }

    /// Removes the top-most board in LIFO order (`3rd -> 2nd -> 1st`, `0x00573488..0x00573549`)
    /// and returns its item to `inventory` (`SE 0x4290`).
    pub fn pop_board(
        &mut self,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> Option<(GravestoneBoard, u32)> {
        if self.len == 0 || flags.get(FLAG_WISH_GRAVESTONE_SOLVED) {
            return None;
        }
        self.len -= 1;
        let (board, _) = self.stack[self.len].take()?;
        inventory.give_raw(board.item_id(), 1);
        self.sync_word_flags(flags);
        Some((board, SE_GRAVESTONE_REMOVE_BOARD))
    }

    fn rotation_of(&self, target: GravestoneBoard) -> Option<BoardRotation> {
        self.stack[..self.len]
            .iter()
            .find_map(|entry| entry.and_then(|(b, r)| (b == target).then_some(r)))
    }

    /// Exact 4-case check at `sh2pc.exe` `0x0057328e..0x005732d7`:
    /// `(Black, White, Red)` in `{(Deg0, Deg0, Left90), (Right90, Right90, Deg0), (Deg180, Deg180, Right90), (Left90, Left90, Deg180)}`.
    pub fn is_winning_arrangement(&self) -> bool {
        let (Some(b), Some(w), Some(r)) = (
            self.rotation_of(GravestoneBoard::Black),
            self.rotation_of(GravestoneBoard::White),
            self.rotation_of(GravestoneBoard::Red),
        ) else {
            return false;
        };
        matches!(
            (b, w, r),
            (
                BoardRotation::Deg0,
                BoardRotation::Deg0,
                BoardRotation::Left90
            ) | (
                BoardRotation::Right90,
                BoardRotation::Right90,
                BoardRotation::Deg0
            ) | (
                BoardRotation::Deg180,
                BoardRotation::Deg180,
                BoardRotation::Right90
            ) | (
                BoardRotation::Left90,
                BoardRotation::Left90,
                BoardRotation::Deg180
            )
        )
    }

    fn sync_word_flags(&self, flags: &mut EventFlags) {
        for bit in 0..32 {
            flags.set(FLAG_WISH_GRAVESTONE_WORD_BASE + bit, false);
        }
        for (layer, slot) in self.stack[..self.len].iter().enumerate() {
            if let Some((board, rot)) = *slot {
                let (layer_base, rot_base) = match board {
                    GravestoneBoard::White => (7, 10),
                    GravestoneBoard::Black => (16, 19),
                    GravestoneBoard::Red => (25, 28),
                };
                flags.set(FLAG_WISH_GRAVESTONE_WORD_BASE + layer_base + layer, true);
                flags.set(
                    FLAG_WISH_GRAVESTONE_WORD_BASE + rot_base + (rot as usize),
                    true,
                );
            }
        }
    }
}

/// Uses Acacia Key (`item 82`, `Stage 56 ev[33]`) to unlock the Baldwin Mansion 2F Attic stair door.
pub fn apply_wish_acacia_key(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !inventory.has_item(ITEM_ACACIA_KEY) {
        return false;
    }
    inventory.item_lose(ITEM_ACACIA_KEY);
    flags.set(FLAG_WISH_ACACIA_DOOR_UNLOCKED, true);
    true
}

/// Uses Matches (`item 83`, `Stage 56 ev[53]`) to light the Mansion 2F candlestick, revealing the Birthday Card (`84`).
pub fn apply_wish_matches_to_candle(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if !inventory.has_item(ITEM_MATCHES) {
        return false;
    }
    inventory.item_lose(ITEM_MATCHES);
    flags.set(FLAG_WISH_CANDLE_LIT, true);
    true
}
