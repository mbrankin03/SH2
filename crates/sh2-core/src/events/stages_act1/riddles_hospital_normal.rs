//! Brookhaven Hospital (Normal, Stages 18–21) interactive riddles & horror sequences:
//! 3F Patient Wing Keypad, 2F Carbon Paper, Room S14 Louise Box (keys + 4-digit keypad + 4-wheel lock
//! with anti-bruteforce scramble), Teddy Bear Bent Needle (`TeddyBearLookFix`), Shower Room Drain,
//! and Roof Diary / Pyramid Head ambush (`stg_hospital_1f_f.c`..`stg_hospital_rf_f.c`).

use super::Act1VanillaFixesConfig;
use crate::config::RiddleDifficulty;
use crate::events::{EventFlags, PuzzleState, PZ_BOX_CODE, PZ_WHEEL, PZ_WHEEL_GOAL};
use crate::inventory::InventoryManager;

/// Item ID for Roof Key (`34`, `0x22`, found in Examining Room 3 / used on Roof door `0x0058cab0`).
pub const ACT1_ITEM_ROOF_KEY: u8 = 34;
/// Item ID for "Purple Bull" Key (`35`, `0x23`, `inventory/catalog.rs` `0x892640`).
pub const ACT1_ITEM_PURPLE_BULL_KEY: u8 = 35;
/// Item ID for "Lapis Eye" Key (`36`, `0x24`, `inventory/catalog.rs` `0x892640`).
pub const ACT1_ITEM_LAPIS_EYE_KEY: u8 = 36;
/// Item ID for Elevator Key (`37`, `0x25`, fished from 3F Shower Room Drain `0x0058df73`).
pub const ACT1_ITEM_ELEVATOR_KEY: u8 = 37;
/// Item ID for Piece of Hair (`53`, `0x35`, inside Room S14 Louise Box `0x0058f7b6`).
pub const ACT1_ITEM_PIECE_OF_HAIR: u8 = 53;
/// Item ID for Bent Needle (`54`, `0x36`, inside 2F Women's Locker Room Teddy Bear `0x0058ffb1`).
pub const ACT1_ITEM_BENT_NEEDLE: u8 = 54;
/// Combination pseudo-item ID `86`: `"Purple Bull" key (35) + "Lapis Eye" key (36)`.
pub const ACT1_COMBO_BULL_AND_LAPIS_KEYS: u8 = 86;
/// Combination pseudo-item ID `87`: `Piece of hair (53) + Bent needle (54)`.
pub const ACT1_COMBO_HAIR_AND_NEEDLE: u8 = 87;

/// Prop model kind for the 2F Women's Locker Room Teddy Bear (`i_bear = 0x0727`, `TeddyBearLookFix.cpp`).
pub const ACT1_PROP_KIND_TEDDY_BEAR: i16 = 0x0727;

/// Flag `170` (`0xaa`, `0x1f7a9f4 & 0x0400`): Bent Needle taken from Teddy Bear (`0x005901ea`, `TeddyBearLookFix.cpp`).
pub const ACT1_FLAG_TEDDY_BEAR_NEEDLE_TAKEN: usize = 170;
/// Flag `171` (`0xab`, `0x1f7a9f4 |= 0x0800`): Piece of Hair taken from Room S14 Louise Box (`0x0058f7d7`).
pub const ACT1_FLAG_LOUISE_HAIR_TAKEN: usize = 171;
/// Flag `172` (`0xac`, `0x1f7a9f4 & 0x1000`): Elevator Key fished out of Shower Room Drain (`0x0058dd69`).
pub const ACT1_FLAG_SHOWER_DRAIN_KEY_TAKEN: usize = 172;
/// Flag `181` (`0xb5`, `0x1f7a9f4 |= 0x200000`): 3F Patient Wing Corridor Keypad unlocked (`0x0058eb4e`).
pub const ACT1_FLAG_WING_KEYPAD_UNLOCKED: usize = 181;
/// Flag `183` (`0xb7`, `0x1f7a9f4 |= 0x800000`): 2F Carbon Paper examined on Hard/Extra (`0x0058fca9`).
pub const ACT1_FLAG_CARBON_PAPER_HARD_READ: usize = 183;
/// Flag `193` (`0xc1`, `0x1f7a9f8 |= 0x02`): Louise Box Key Lock 1 unlocked (`0x0058f29d`).
pub const ACT1_FLAG_LOUISE_KEY1_OPEN: usize = 193;
/// Flag `195` (`0xc3`, `0x1f7a9f8 |= 0x08`): Louise Box Key Lock 2 unlocked (`0x0058f319`).
pub const ACT1_FLAG_LOUISE_KEY2_OPEN: usize = 195;
/// Flag `197` (`0xc5`, `0x1f7a9f8 |= 0x20`): Louise Box 4-Wheel rotary lock unlocked (`0x0058f45a`).
pub const ACT1_FLAG_LOUISE_WHEEL_OPEN: usize = 197;
/// Flag `198` (`0xc6`, `0x1f7a9f8 |= 0x40`): Louise Box 4-Digit keypad lock unlocked (`0x0058f3e7`).
pub const ACT1_FLAG_LOUISE_KEYPAD_OPEN: usize = 198;
/// Flag `199` (`0xc7`): Hospital Roof door unlocked with `Roof key` (`stg_hospital_rf_f` `ev_list[4]`).
pub const ACT1_FLAG_ROOF_DOOR_UNLOCKED: usize = 199;
/// Flag `200` (`0xc8`): Hospital Roof door slammed/locked behind James (`stg_hospital_rf_f` `ev_list[8]`).
pub const ACT1_FLAG_ROOF_DOOR_LOCKED_BEHIND: usize = 200;
/// Flag `203` (`0xcb`, `0x1f7a9f8 & 0x0800`): 3F Wall Blood Code read (`0x0058f41d`) / Roof PH push (`ev_list[2]`).
pub const ACT1_FLAG_WALL_BLOOD_CODE_OR_ROOF_PUSH: usize = 203;
/// Flag `204` (`0xcc`): Hospital Roof rusty chain-link fence inspected (`stg_hospital_rf_f` `ev_list[11]`).
pub const ACT1_FLAG_ROOF_FENCE_INSPECTED: usize = 204;
/// Flag `613` (`0x265`, `0x1f7aa2c |= 0x20`): 2F Carbon Paper memo recorded on Easy/Normal (`0x0058fcd4`).
pub const ACT1_FLAG_CARBON_PAPER_MEMO_EASY: usize = 613;

/// Sound effect `19009` (`0x4a41`): 3F Patient Wing Keypad door unlock (`0x0058eb59`).
pub const ACT1_SE_WING_KEYPAD_UNLOCK: u32 = 0x4a41;
/// Sound effect `19013` (`0x4a45`): Louise Box rotary dial click (`0x0058e7ab`).
pub const ACT1_SE_LOUISE_WHEEL_CLICK: u32 = 0x4a45;
/// Sound effect `19014` (`0x4a46`): Louise Box rotary lock unlatch (`0x0058f462`).
pub const ACT1_SE_LOUISE_WHEEL_UNLOCK: u32 = 0x4a46;
/// Sound effect `19015` (`0x4a47`): Louise Box first key lock unlatch (`0x0058f27c`).
pub const ACT1_SE_LOUISE_KEY1_UNLOCK: u32 = 0x4a47;
/// Sound effect `19019` (`0x4a4b`): Hospital Roof rusty fence rattle (`EvProgMetalNetRusty` `0x0058d03a`).
pub const ACT1_SE_ROOF_RUSTY_FENCE: u32 = 0x4a4b;
/// Sound effect `19021` (`0x4a4d`): Louise Box second key lock unlatch (`0x0058f2f6`).
pub const ACT1_SE_LOUISE_KEY2_UNLOCK: u32 = 0x4a4d;
/// Sound effect `19029` (`0x4a55`): 3F Keypad / Louise push-button beep (`0x0058ead1`).
pub const ACT1_SE_KEYPAD_BEEP: u32 = 0x4a55;
/// Sound effect `19030` (`0x4a56`): Louise Box push-button lock unlatch (`0x0058f3ef`).
pub const ACT1_SE_LOUISE_KEYPAD_UNLOCK: u32 = 0x4a56;
/// Sound effect `19898` (`0x4dba`): Louise Box lid creaking open (`0x0058f63c`).
pub const ACT1_SE_LOUISE_BOX_OPEN: u32 = 0x4dba;

/// Returns the expected 4-digit code for the 3F Patient Wing Corridor Keypad (`0x0058eaef..0x0058eb44`).
///
/// - **Easy / Normal (`0 | 1`):** `[7, 3, 3, 5]` ("7335", written directly on Doctors' Lounge whiteboard).
/// - **Hard / Extra (`2 | 3`):** `[1, 3, 2, 8]` ("1328", traced as the letter `'T'` on the 1..9 keypad).
#[must_use]
pub const fn act1_hospital_wing_keypad_goal(difficulty: RiddleDifficulty) -> [u8; 4] {
    match difficulty {
        RiddleDifficulty::Easy | RiddleDifficulty::Normal => [7, 3, 3, 5],
        RiddleDifficulty::Hard | RiddleDifficulty::Extra => [1, 3, 2, 8],
    }
}

/// Interactive 4-digit shift-register state for `EvProgSetPassNumberT` (`sh2pc.exe` `0x0058ea40..0x0058fa10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Act1HospitalWingKeypad {
    /// Rolling 4-byte shift register (`0x01d838c8..0x01d838cb` in `sh2pc.exe`).
    pub digits: [u8; 4],
}

impl Act1HospitalWingKeypad {
    /// Creates a cleared keypad buffer (`[0, 0, 0, 0]`, `0x0058f945`).
    #[must_use]
    pub const fn new() -> Self {
        Self { digits: [0; 4] }
    }

    /// Presses button `digit` (`1..=9`), shifting `digits` left and unlocking `flag(181)` on match (`0x0058eab3`).
    pub fn press_digit(
        &mut self,
        digit: u8,
        difficulty: RiddleDifficulty,
        flags: &mut EventFlags,
    ) -> Option<bool> {
        if !(1..=9).contains(&digit) {
            return None;
        }
        self.digits = [self.digits[1], self.digits[2], self.digits[3], digit];
        let solved = self.digits == act1_hospital_wing_keypad_goal(difficulty);
        if solved {
            flags.set(ACT1_FLAG_WING_KEYPAD_UNLOCKED, true);
        }
        Some(solved)
    }
}

/// Formats the 4 BCD nibbles of `PZ_BOX_CODE` (`0x1f7ae72`) into `"dddd"` (`EvProgCarbonPaper` `0x0058fc47`).
#[must_use]
pub fn format_act1_carbon_paper_code(puzzle: &PuzzleState) -> String {
    let raw = puzzle.read_u16(PZ_BOX_CODE);
    let d0 = ((raw >> 12) & 0x0f) as u8;
    let d1 = ((raw >> 8) & 0x0f) as u8;
    let d2 = ((raw >> 4) & 0x0f) as u8;
    let d3 = (raw & 0x0f) as u8;
    format!("{d0}{d1}{d2}{d3}")
}

/// Executes `EvProgCarbonPaper` (`sh2pc.exe` `0x0058faf0`), returning `(message_id, code_string)`.
pub fn act1_read_carbon_paper_memo(
    difficulty: RiddleDifficulty,
    puzzle: &PuzzleState,
    flags: &mut EventFlags,
) -> (u8, String) {
    let text = format_act1_carbon_paper_code(puzzle);
    let msg = match difficulty {
        RiddleDifficulty::Easy | RiddleDifficulty::Normal => {
            flags.set(ACT1_FLAG_CARBON_PAPER_MEMO_EASY, true);
            23
        }
        RiddleDifficulty::Hard | RiddleDifficulty::Extra => {
            flags.set(ACT1_FLAG_CARBON_PAPER_HARD_READ, true);
            8
        }
    };
    (msg, text)
}

/// Marks the 3F Wall Blood Code (`PZ_WHEEL_GOAL`) as read (`EvProgGuruguruNumber` `0x0058eec0`),
/// setting `flag(203)` (`0x1f7a9f8 |= 0x0800`) required before Louise's rotary lock can unlatch.
pub fn act1_read_louise_wall_blood_code(flags: &mut EventFlags) {
    flags.set(ACT1_FLAG_WALL_BLOOD_CODE_OR_ROOF_PUSH, true);
}

/// Interactive state machine for Room S14's 4-lock Louise Box (`EvProgBoxWithKey` `sh2pc.exe` `0x0058f100`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act1LouiseBoxState {
    /// Zero-based push-button digits (`-1` = unset, `0..=8` = buttons `1..=9`, `0x01d838bc..bf`).
    pub keypad_slots: [i8; 4],
    /// Next slot index (`0..4`) written by push-button presses.
    pub keypad_cursor: usize,
}

impl Default for Act1LouiseBoxState {
    fn default() -> Self {
        Self::new()
    }
}

impl Act1LouiseBoxState {
    /// Initializes Louise Box inspection (`0x0058f138`: `0x1d838bc = -1`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keypad_slots: [-1; 4],
            keypad_cursor: 0,
        }
    }

    /// Uses `Purple Bull Key` (`35`), `Lapis Eye Key` (`36`), or combo `86` on Louise's key locks (`0x0058f275..0x0058f340`).
    pub fn use_key(
        &mut self,
        item_id: u8,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> bool {
        match item_id {
            ACT1_ITEM_PURPLE_BULL_KEY
                if !flags.get(ACT1_FLAG_LOUISE_KEY1_OPEN) && inventory.has_item(item_id) =>
            {
                inventory.item_lose(item_id);
                flags.set(ACT1_FLAG_LOUISE_KEY1_OPEN, true);
                true
            }
            ACT1_ITEM_LAPIS_EYE_KEY
                if !flags.get(ACT1_FLAG_LOUISE_KEY2_OPEN) && inventory.has_item(item_id) =>
            {
                inventory.item_lose(item_id);
                flags.set(ACT1_FLAG_LOUISE_KEY2_OPEN, true);
                true
            }
            ACT1_COMBO_BULL_AND_LAPIS_KEYS if inventory.has_item(item_id) => {
                inventory.item_lose(item_id);
                flags.set(ACT1_FLAG_LOUISE_KEY1_OPEN, true);
                flags.set(ACT1_FLAG_LOUISE_KEY2_OPEN, true);
                true
            }
            _ => false,
        }
    }

    /// Enters a digit (`1..=9`) on Louise's 4-digit push-button lock (`0x0058f387..0x0058f417`).
    pub fn press_keypad_digit(
        &mut self,
        digit: u8,
        puzzle: &PuzzleState,
        flags: &mut EventFlags,
    ) -> bool {
        if !(1..=9).contains(&digit) || flags.get(ACT1_FLAG_LOUISE_KEYPAD_OPEN) {
            return false;
        }
        self.keypad_slots[self.keypad_cursor % 4] = (digit - 1) as i8;
        self.keypad_cursor = (self.keypad_cursor + 1) % 4;

        let raw = puzzle.read_u16(PZ_BOX_CODE);
        let goal = [
            ((raw >> 12) & 0x0f) as i8,
            ((raw >> 8) & 0x0f) as i8,
            ((raw >> 4) & 0x0f) as i8,
            (raw & 0x0f) as i8,
        ];
        if (0..4).all(|i| self.keypad_slots[i] + 1 == goal[i]) {
            flags.set(ACT1_FLAG_LOUISE_KEYPAD_OPEN, true);
            return true;
        }
        false
    }

    /// Increments rotary wheel `wheel_idx` (`0..=3`, wrapping `0..=8` -> `1..=9` on dial, `0x0058e789..0x0058e7a4`),
    /// unlocking `flag(197)` if `flag(203)` is set and `PZ_WHEEL == PZ_WHEEL_GOAL` (`0x0058f418..0x0058f48a`).
    pub fn rotate_wheel(
        &mut self,
        wheel_idx: usize,
        puzzle: &mut PuzzleState,
        flags: &mut EventFlags,
    ) -> bool {
        if wheel_idx >= 4 || flags.get(ACT1_FLAG_LOUISE_WHEEL_OPEN) {
            return false;
        }
        let cur = puzzle.raw()[PZ_WHEEL + wheel_idx];
        puzzle.raw_mut()[PZ_WHEEL + wheel_idx] = if cur >= 8 { 0 } else { cur + 1 };

        let solved =
            puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4] == puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4];
        if flags.get(ACT1_FLAG_WALL_BLOOD_CODE_OR_ROOF_PUSH) && solved {
            flags.set(ACT1_FLAG_LOUISE_WHEEL_OPEN, true);
            return true;
        }
        false
    }

    /// Executes the exit anti-bruteforce check (`sh2pc.exe` `0x0058f81f..0x0058f88c`):
    /// if the player backs out before `flag(197)` is open (because `flag(203)` wasn't read yet)
    /// while `PZ_WHEEL == PZ_WHEEL_GOAL`, scrambles wheel `rng_slot & 3` away from `goal`!
    pub fn on_exit_box(&self, rng_seed: u32, puzzle: &mut PuzzleState, flags: &EventFlags) -> bool {
        let solved =
            puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4] == puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4];
        if !flags.get(ACT1_FLAG_LOUISE_WHEEL_OPEN) && solved {
            let slot = (rng_seed as usize) & 3;
            let mut candidate = ((rng_seed >> 8) % 7) as u8;
            let goal_val = puzzle.raw()[PZ_WHEEL_GOAL + slot];
            if candidate == goal_val {
                candidate += 1;
            }
            puzzle.raw_mut()[PZ_WHEEL + slot] = candidate;
            return true;
        }
        false
    }

    /// Returns `true` when all 4 locks (`flags 193, 195, 197, 198`) are open (`0x0058f5d1`).
    #[must_use]
    pub fn all_four_locks_open(&self, flags: &EventFlags) -> bool {
        flags.get(ACT1_FLAG_LOUISE_KEY1_OPEN)
            && flags.get(ACT1_FLAG_LOUISE_KEY2_OPEN)
            && flags.get(ACT1_FLAG_LOUISE_WHEEL_OPEN)
            && flags.get(ACT1_FLAG_LOUISE_KEYPAD_OPEN)
    }

    /// Opens the fully unlocked Louise Box (`0x0058f60a..0x0058f7f0`), awarding `Piece of hair` (`53`) and `flag(171)`.
    pub fn claim_piece_of_hair(
        &self,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> bool {
        if !self.all_four_locks_open(flags) || flags.get(ACT1_FLAG_LOUISE_HAIR_TAKEN) {
            return false;
        }
        inventory.item_get_now(ACT1_ITEM_PIECE_OF_HAIR);
        flags.set(ACT1_FLAG_LOUISE_HAIR_TAKEN, true);
        true
    }
}

/// Inspects the 2F Women's Locker Room Teddy Bear (`EvProgNeedleInAnimal` `sh2pc.exe` `0x0058fec0`),
/// awarding `Bent needle` (`54`) and setting `flag(170)` (`0xaa`).
pub fn act1_inspect_teddy_bear(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if flags.get(ACT1_FLAG_TEDDY_BEAR_NEEDLE_TAKEN) {
        return false;
    }
    inventory.item_get_now(ACT1_ITEM_BENT_NEEDLE);
    flags.set(ACT1_FLAG_TEDDY_BEAR_NEEDLE_TAKEN, true);
    true
}

/// Evaluates whether James's dynamic head-tracking should look at `prop_kind` (`TeddyBearLookFix.cpp`).
///
/// In vanilla PS2/PC (`fixes.teddy_bear_look_fix == false`), James keeps staring at `0x0727` (`i_bear`)
/// forever even after taking the `Bent needle`. When `true`, `0x0727` is ignored once `flag(170)` is set.
#[must_use]
pub fn act1_should_james_look_at_prop(
    prop_kind: i16,
    flags: &EventFlags,
    fixes: Act1VanillaFixesConfig,
) -> bool {
    !(fixes.teddy_bear_look_fix
        && prop_kind == ACT1_PROP_KIND_TEDDY_BEAR
        && flags.get(ACT1_FLAG_TEDDY_BEAR_NEEDLE_TAKEN))
}

/// Outcome of interacting with or using items on the 3F Shower Room Drain (`0x0058da90..0x0058df90`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act1ShowerDrainOutcome {
    /// `EvProgInShowerDrain` (`0x0058dcb0`): `msg 8` (glint visible) or `msg 9` (empty drain).
    Inspected { message_id: u8 },
    /// `EvProgOnlyNeedle` (`0x0058da90`): `msg 12` (needle alone is too short).
    NeedleOnlyTooShort,
    /// `EvProgOnlyHair` (`0x0058dba0`): `msg 11` (hair alone is too limp).
    HairOnlyUseless,
    /// `EvProgFishKey` (`0x0058dee0`): `msg 15`, consumes Hair + Needle, awards `Elevator key` (`37`) + `flag(172)`.
    FishedElevatorKey,
    /// Unrelated item or already retrieved.
    Invalid,
}

/// Evaluates item use or inspection on the 3F Shower Room Drain (`0x0058da90..0x0058df90`).
pub fn act1_interact_shower_drain(
    used_item_id: Option<u8>,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Act1ShowerDrainOutcome {
    match used_item_id {
        None => Act1ShowerDrainOutcome::Inspected {
            message_id: if flags.get(ACT1_FLAG_SHOWER_DRAIN_KEY_TAKEN) {
                9
            } else {
                8
            },
        },
        Some(ACT1_ITEM_BENT_NEEDLE) if !flags.get(ACT1_FLAG_SHOWER_DRAIN_KEY_TAKEN) => {
            Act1ShowerDrainOutcome::NeedleOnlyTooShort
        }
        Some(ACT1_ITEM_PIECE_OF_HAIR) if !flags.get(ACT1_FLAG_SHOWER_DRAIN_KEY_TAKEN) => {
            Act1ShowerDrainOutcome::HairOnlyUseless
        }
        Some(ACT1_COMBO_HAIR_AND_NEEDLE)
            if !flags.get(ACT1_FLAG_SHOWER_DRAIN_KEY_TAKEN)
                && inventory.has_item(ACT1_COMBO_HAIR_AND_NEEDLE) =>
        {
            inventory.item_lose(ACT1_COMBO_HAIR_AND_NEEDLE);
            inventory.item_get_now(ACT1_ITEM_ELEVATOR_KEY);
            flags.set(ACT1_FLAG_SHOWER_DRAIN_KEY_TAKEN, true);
            Act1ShowerDrainOutcome::FishedElevatorKey
        }
        _ => Act1ShowerDrainOutcome::Invalid,
    }
}

/// Applies the exact Hospital Roof Pyramid Head push HP reduction (`EvProgTriheadOnRoof` `sh2pc.exe` `0x0058caee..0x0058cb9a`),
/// clearing `flag(200)` and setting `flag(203)` before transitioning to `Stage 20` Special Treatment Room (`EvProgFromRoof`).
///
/// Exact binary formula (`0x0058cafa..0x0058cb3e`, `[0x69f390] = 0.7`, `[0x69f3d4] = 0.05`):
/// if `hp > max_hp * 0.7`, `hp -= max_hp * 0.7`; else `hp = hp.min(max_hp * 0.05)`.
pub fn act1_apply_roof_pyramid_head_push(
    current_hp: f32,
    max_hp: f32,
    flags: &mut EventFlags,
) -> f32 {
    flags.set(ACT1_FLAG_ROOF_DOOR_LOCKED_BEHIND, false);
    flags.set(ACT1_FLAG_WALL_BLOOD_CODE_OR_ROOF_PUSH, true);
    let drop = max_hp * 0.7;
    if current_hp > drop {
        current_hp - drop
    } else {
        current_hp.min(max_hp * 0.05)
    }
}
