//! Lakeview Hotel Normal 2F (`Stage 41`): Employee Elevator Weight-Limit Cupboard (`for_shelf`),
//! Cloakroom `Fish Key`, Room 202 `Thinner`, and Room 204 Briefcase 4-Letter Drum Lock (`PZ_DRUM`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28` (`stg_hotel_2f_f.c`) and retail PC
//! `sh2pc.exe` (`0x00579050..0x0057b150`, `trunk_drum` `0x008ca414`, `trunk_key` `0x008ca438`,
//! `for_shelf` `0x008ca488`, `0x008ca504`).

use super::ITEM_MUSIC_BOX_CINDERELLA;
use crate::events::{EventFlags, PuzzleState, Sh2Rng, PZ_DRUM};
use crate::inventory::InventoryManager;

/// Flag `400` (`0x01f7aa10` bit `16`): James's inventory is empty enough for the 1-person employee elevator (`0x0057a3b4`).
pub const FLAG_ELEVATOR_WEIGHT_OK: usize = 400;
/// Flag `401` (`0x01f7aa10` bit `17`): Employee elevator weight-overload buzzer active (`SE 0x44c4`, `0x0057a410`).
pub const FLAG_ELEVATOR_OVERWEIGHT_ALARM: usize = 401;
/// Flag `403`: Cloakroom briefcase unlocked with Fish Key (`item 47`, `EvProgUseFishKey` `0x0057aac0`).
pub const FLAG_CLOAKROOM_FISH_KEY_USED: usize = 403;
/// Flag `405`: Room 204 briefcase drum dials scrambled once (`0x0057a170`).
pub const FLAG_BRIEFCASE_DRUMS_INITIALIZED: usize = 405;
/// Base flag `406` (`406..=410`, 5 bits): Selected briefcase 4-letter word index (`0..=18`, `FlagInit` `0x0048aed0`).
pub const FLAG_BRIEFCASE_WORD_INDEX_BASE: usize = 406;
/// Flag `412`: Room 202 blacked-out photo cleaned with Thinner (`item 66`, `EvProgPictureWithMarker` `0x0057b020`).
pub const FLAG_PHOTO_CLEANED_WITH_THINNER: usize = 412;
/// Flag `413` (`0x01f7aa10` bit `29`): Briefcase 4-letter drum lock opened (`0x00579be0`, `SE 0x44c3`).
pub const FLAG_BRIEFCASE_UNLOCKED: usize = 413;
/// Flag `446` (`0x01f7aa14` bit `30`): At least one item is stored in the 2F Employee Elevator cupboard (`0x00579510`).
pub const FLAG_SHELF_HAS_ITEMS: usize = 446;

/// Inventory item ID: 204 key (`44`).
pub const ITEM_HOTEL_204_KEY: u8 = 44;
/// Inventory item ID: Fish key (`47`).
pub const ITEM_FISH_KEY: u8 = 47;
/// Inventory item ID: Thinner (`66`).
pub const ITEM_THINNER: u8 = 66;

/// Sound effect ID: Shelf item transfer (`0x2b21` at `0x00579330` / `0x00579510`).
pub const SE_SHELF_TRANSFER: u32 = 0x2b21;
/// Sound effect ID: Briefcase unlocked (`0x44c3` at `0x00579f30`).
pub const SE_BRIEFCASE_UNLOCK: u32 = 0x44c3;
/// Sound effect ID: Employee elevator overweight alarm (`0x44c4` at `0x0057a410`).
pub const SE_ELEVATOR_OVERWEIGHT: u32 = 0x44c4;
/// Sound effect ID: Briefcase dial click (`0x4a45` at `0x00579e10`).
pub const SE_BRIEFCASE_DIAL_TURN: u32 = 0x4a45;

/// Exact 4x8 briefcase alphabet wheels `trunk_drum[4][8]` (`sh2pc.exe` `0x008ca414`).
pub const BRIEFCASE_TRUNK_DRUMS: [[u8; 8]; 4] =
    [*b"dhklmnot", *b"aeiopuvx", *b"celmrsvw", *b"aeklnprt"];

/// Exact 19 randomized 4-letter briefcase passwords `trunk_key[19]` (`sh2pc.exe` `0x008ca438`).
pub const BRIEFCASE_TRUNK_WORDS: [&str; 19] = [
    "open", "damn", "hell", "town", "dark", "mama", "down", "love", "lock", "mist", "luck", "lose",
    "dose", "over", "dust", "time", "help", "kill", "null",
];

/// Exact 30 `(item_id, shelf_flag_id)` pairs in `for_shelf` (`sh2pc.exe` `0x008ca488`).
pub const ELEVATOR_SHELF_ITEM_FLAGS: [(u8, usize); 30] = [
    (21, 417),
    (6, 418),
    (8, 419),
    (12, 420),
    (14, 421),
    (15, 422),
    (16, 423),
    (33, 424),
    (43, 425),
    (66, 426),
    (67, 427),
    (68, 428),
    (74, 429),
    (76, 430),
    (77, 431),
    (25, 432),
    (1, 433),
    (2, 434),
    (3, 435),
    (4, 436),
    (5, 437),
    (7, 438),
    (9, 439),
    (13, 440),
    (18, 441),
    (19, 442),
    (20, 443),
    (22, 444),
    (24, 445),
    (78, 730),
];

/// Exact 6 stackable consumable/ammunition item IDs whose counts (`0..=999`) are preserved in
/// `0x01f7adc0..0x01f7adcc` (`sh2pc.exe` `0x008ca504`).
pub const ELEVATOR_SHELF_STACKABLE_IDS: [u8; 6] = [5, 7, 9, 1, 2, 3];

/// Saved stackable quantities (`0x01f7adc0..0x01f7adcc`) for the 2F Employee Elevator cupboard
/// (`EvProgPutBaggageForShelf` `0x00579510` & `EvProgGetBaggageInShelf` `0x00579330`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ElevatorCupboardState {
    saved_stack_counts: [u16; 6],
}

impl ElevatorCupboardState {
    /// Creates an empty cupboard state.
    pub const fn new() -> Self {
        Self {
            saved_stack_counts: [0; 6],
        }
    }

    /// Deposits all 30 shelf-eligible items (`for_shelf` at `0x008ca488`) from `inventory` into
    /// the cupboard (`0x00579510`), preserving stackable ammo/consumable counts (`0..=999`).
    pub fn deposit_all(
        &mut self,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> usize {
        let mut moved = 0usize;
        for &(item_id, flag_id) in &ELEVATOR_SHELF_ITEM_FLAGS {
            if inventory.has_item(item_id) {
                if let Some(slot) = ELEVATOR_SHELF_STACKABLE_IDS
                    .iter()
                    .position(|&id| id == item_id)
                {
                    let qty = inventory.count(item_id);
                    self.saved_stack_counts[slot] =
                        self.saved_stack_counts[slot].saturating_add(qty).min(999);
                    inventory.give_raw(item_id, -i32::from(qty));
                } else {
                    inventory.item_lose(item_id);
                }
                flags.set(flag_id, true);
                moved += 1;
            }
        }
        let any_on_shelf = ELEVATOR_SHELF_ITEM_FLAGS
            .iter()
            .any(|&(_, fid)| flags.get(fid));
        flags.set(FLAG_SHELF_HAS_ITEMS, any_on_shelf);
        sync_employee_elevator_weight(flags, inventory);
        moved
    }

    /// Retrieves all stored items from the cupboard back into `inventory` (`0x00579330`), clamping
    /// combined stackable counts to `999` (`0x0057941a`).
    pub fn withdraw_all(
        &mut self,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> usize {
        let mut restored = 0usize;
        for &(item_id, flag_id) in &ELEVATOR_SHELF_ITEM_FLAGS {
            if flags.get(flag_id) {
                flags.set(flag_id, false);
                if let Some(slot) = ELEVATOR_SHELF_STACKABLE_IDS
                    .iter()
                    .position(|&id| id == item_id)
                {
                    let saved = self.saved_stack_counts[slot];
                    self.saved_stack_counts[slot] = 0;
                    inventory.give_raw(item_id, i32::from(saved));
                } else {
                    inventory.give_raw(item_id, 1);
                }
                restored += 1;
            }
        }
        flags.set(FLAG_SHELF_HAS_ITEMS, false);
        sync_employee_elevator_weight(flags, inventory);
        restored
    }
}

/// Evaluates the 2F Employee Elevator 1-person weight sensor (`EvAllTimeFunc` `0x0057a3b4`): sets
/// `flag 400` (`FLAG_ELEVATOR_WEIGHT_OK`) iff no shelf-eligible items remain in `inventory`.
pub fn sync_employee_elevator_weight(flags: &mut EventFlags, inventory: &InventoryManager) -> bool {
    let carrying_weight = ELEVATOR_SHELF_ITEM_FLAGS
        .iter()
        .any(|&(item_id, _)| inventory.has_item(item_id));
    if carrying_weight {
        flags.set(FLAG_ELEVATOR_WEIGHT_OK, false);
        false
    } else {
        flags.set(FLAG_ELEVATOR_WEIGHT_OK, true);
        flags.set(FLAG_ELEVATOR_OVERWEIGHT_ALARM, false);
        true
    }
}

/// Attempts to board the 2F Employee Elevator (`0x0057a3b4` & `0x0057a410`): returns `Ok(())` when
/// `FLAG_ELEVATOR_WEIGHT_OK` holds, or sets `FLAG_ELEVATOR_OVERWEIGHT_ALARM` and returns `Err(SE_ELEVATOR_OVERWEIGHT)`.
pub fn try_board_employee_elevator(
    flags: &mut EventFlags,
    inventory: &InventoryManager,
) -> Result<(), u32> {
    if sync_employee_elevator_weight(flags, inventory) {
        Ok(())
    } else {
        flags.set(FLAG_ELEVATOR_OVERWEIGHT_ALARM, true);
        Err(SE_ELEVATOR_OVERWEIGHT)
    }
}

/// Uses Fish Key (`item 47`) on the 2F Cloakroom briefcase (`0x0057aac0`) -> awards `204 Key` (`44`).
pub fn apply_cloakroom_fish_key(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !inventory.has_item(ITEM_FISH_KEY) {
        return false;
    }
    inventory.item_lose(ITEM_FISH_KEY);
    flags.set(FLAG_CLOAKROOM_FISH_KEY_USED, true);
    inventory.give_raw(ITEM_HOTEL_204_KEY, 1);
    true
}

/// Uses Thinner (`item 66`) on the Room 202 marker-defaced photo (`0x0057b020`), revealing the
/// 4-letter briefcase password stored in flags `406..=410`.
pub fn apply_room202_thinner<'a>(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<&'a str> {
    if !inventory.has_item(ITEM_THINNER) {
        return None;
    }
    inventory.item_lose(ITEM_THINNER);
    flags.set(FLAG_PHOTO_CLEANED_WITH_THINNER, true);
    Some(selected_briefcase_word(flags))
}

/// Reads the 5-bit briefcase password index (`0..=18`) from flags `406..=410` (`0x0057a170`).
pub fn selected_briefcase_word(flags: &EventFlags) -> &'static str {
    let mut idx = 0usize;
    for b in 0..5 {
        if flags.get(FLAG_BRIEFCASE_WORD_INDEX_BASE + b) {
            idx |= 1 << b;
        }
    }
    BRIEFCASE_TRUNK_WORDS[idx % BRIEFCASE_TRUNK_WORDS.len()]
}

/// Scrambles `PZ_DRUM[0..4]` once (`flag 405`) so no dial starts on its target letter (`0x0057a170`).
pub fn init_briefcase_drums_if_needed(
    flags: &mut EventFlags,
    puzzle: &mut PuzzleState,
    rng: &mut Sh2Rng,
) {
    if flags.get(FLAG_BRIEFCASE_DRUMS_INITIALIZED) {
        return;
    }
    let word = selected_briefcase_word(flags).as_bytes();
    for i in 0..4 {
        let target_pos = BRIEFCASE_TRUNK_DRUMS[i]
            .iter()
            .position(|&c| c == word[i])
            .unwrap_or(0) as u8;
        let offset = ((rng.next_i32() % 7) as u8).wrapping_add(1);
        puzzle.raw_mut()[PZ_DRUM + i] = (target_pos + offset) & 7;
    }
    flags.set(FLAG_BRIEFCASE_DRUMS_INITIALIZED, true);
}

/// Rotates briefcase dial `wheel` (`0..=3`) by `delta` (`+1` or `-1`), unlocking (`flag 413`,
/// `SE 0x44c3`, awarding Cinderella music box `68`) when all 4 letters match (`0x00579be0`).
pub fn turn_briefcase_drum(
    wheel: usize,
    delta: i8,
    flags: &mut EventFlags,
    puzzle: &mut PuzzleState,
    inventory: &mut InventoryManager,
) -> Option<(bool, u32)> {
    if wheel >= 4 {
        return None;
    }
    let cur = puzzle.raw()[PZ_DRUM + wheel] as i8;
    puzzle.raw_mut()[PZ_DRUM + wheel] = (cur.wrapping_add(delta) & 7) as u8;
    let current_letters = [
        BRIEFCASE_TRUNK_DRUMS[0][(puzzle.raw()[PZ_DRUM] & 7) as usize],
        BRIEFCASE_TRUNK_DRUMS[1][(puzzle.raw()[PZ_DRUM + 1] & 7) as usize],
        BRIEFCASE_TRUNK_DRUMS[2][(puzzle.raw()[PZ_DRUM + 2] & 7) as usize],
        BRIEFCASE_TRUNK_DRUMS[3][(puzzle.raw()[PZ_DRUM + 3] & 7) as usize],
    ];
    if current_letters == selected_briefcase_word(flags).as_bytes() {
        flags.set(FLAG_BRIEFCASE_UNLOCKED, true);
        inventory.give_raw(ITEM_MUSIC_BOX_CINDERELLA, 1);
        Some((true, SE_BRIEFCASE_UNLOCK))
    } else {
        Some((false, SE_BRIEFCASE_DIAL_TURN))
    }
}
