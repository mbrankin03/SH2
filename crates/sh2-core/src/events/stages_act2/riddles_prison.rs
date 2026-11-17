//! Historical Society, Delusion 2/3, and Toluca Prison Riddles & Stage/Room Lifecycle (`Stages 28..=33`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28` (`stg_society.c`, `stg_delusion_2.c`,
//! `stg_delusion_3.c`, `stg_prison_n.c`, `stg_prison_s.c`, `stg_prison_bf.c`), retail PC
//! `sh2pc.exe` (`0x00583f50..0x00588210`), and SH2:EE `PrisonerTimer.cpp` (`0x004ae5f8`).

use crate::events::{EventFlags, PuzzleState, PZ_PANEL};
use crate::inventory::InventoryManager;

/// Flag `269` (`0x01f7aa00` bit `13`): Bug/Roach Room keypad door locked (`EvProgCocRoomLock` `0x005872a0`).
pub const FLAG_DELUSION3_BUG_DOOR_LOCKED: usize = 269;
/// Flag `271` (`0x01f7aa00` bit `15`): Spiral-writing key (`item 41`) used (`Stage 30 ev[21..23]`).
pub const FLAG_DELUSION3_SPIRAL_KEY_USED: usize = 271;
/// Flag `272` (`0x01f7aa02` bit `0`): Flashlight dry-cell battery depleted (`0x0048e141` & `0x00587320`).
pub const FLAG_FLASHLIGHT_BATTERY_DEAD: usize = 272;

/// Flag `280` (`0x01f7aa02` bit `8`): Prison N trapdoor inspected with Horseshoe/Lighter (`Stage 31 ev[10..15]`).
pub const FLAG_PRISON_TRAPDOOR_HORSESHOE_OR_LIGHTER: usize = 280;
/// Flag `281` (`0x01f7aa02` bit `9`): Prison N trapdoor inspected with Wax Doll (`Stage 31 ev[16..19]`).
pub const FLAG_PRISON_TRAPDOOR_WAX_DOLL: usize = 281;
/// Flag `282` (`0x01f7aa02` bit `10`): Prison N trapdoor inspected with Lighter + Wax Doll (`Stage 31 ev[20..21]`).
pub const FLAG_PRISON_TRAPDOOR_MELTED_WAX_ONLY: usize = 282;
/// Flag `283` (`0x01f7aa02` bit `11`): Prison N floor trapdoor opened via Wax Doll + Horseshoe + Lighter (`0x00585b30`).
pub const FLAG_PRISON_TRAPDOOR_OPENED: usize = 283;

/// Flag `294`: Pending placement of Tablet of "The Oppressor" (`item 59`, `Stage 32 ev[3]`).
pub const FLAG_SCAFFOLD_PENDING_OPPRESSOR: usize = 294;
/// Flag `295`: Pending placement of Tablet of "Gluttonous Pig" (`item 60`, `Stage 32 ev[4]`).
pub const FLAG_SCAFFOLD_PENDING_GLUTTONOUS_PIG: usize = 295;
/// Flag `296`: Pending placement of Tablet of "The Seductress" (`item 61`, `Stage 32 ev[5]`).
pub const FLAG_SCAFFOLD_PENDING_SEDUCTRESS: usize = 296;
/// Flag `297`: Combined tablets `59 + 60` (`item 89`) used (`Stage 32 ev[6]`).
pub const FLAG_SCAFFOLD_COMBO_89: usize = 297;
/// Flag `298`: Combined tablets `59 + 61` (`item 90`) used (`Stage 32 ev[7]`).
pub const FLAG_SCAFFOLD_COMBO_90: usize = 298;
/// Flag `299`: Combined tablets `60 + 61` (`item 91`) used (`Stage 32 ev[8]`).
pub const FLAG_SCAFFOLD_COMBO_91: usize = 299;
/// Flag `300`: All three combined tablets `59 + 60 + 61` (`item 92`) used (`Stage 32 ev[9]`).
pub const FLAG_SCAFFOLD_COMBO_92: usize = 300;
/// Flag `301`: Tablet of "The Oppressor" mounted in gallows scaffold (`alpha_set_flag[0]` at `0x008d27fc`).
pub const FLAG_SCAFFOLD_SET_OPPRESSOR: usize = 301;
/// Flag `302`: Tablet of "Gluttonous Pig" mounted in gallows scaffold (`alpha_set_flag[1]` at `0x008d27fe`).
pub const FLAG_SCAFFOLD_SET_GLUTTONOUS_PIG: usize = 302;
/// Flag `303`: Tablet of "The Seductress" mounted in gallows scaffold (`alpha_set_flag[2]` at `0x008d2800`).
pub const FLAG_SCAFFOLD_SET_SEDUCTRESS: usize = 303;
/// Flag `304`: All three Prison Tablets mounted; scream echoes & Horseshoe spawns on door (`0x00584b10`).
pub const FLAG_SCAFFOLD_ALL_SET: usize = 304;

/// Inventory item ID: Spiral-writing key (`41`).
pub const ITEM_SPIRAL_WRITING_KEY: u8 = 41;
/// Inventory item ID: Dry cell battery (`55`).
pub const ITEM_DRY_CELL_BATTERY: u8 = 55;
/// Inventory item ID: Tablet of "The Oppressor" (`59`).
pub const ITEM_TABLET_OPPRESSOR: u8 = 59;
/// Inventory item ID: Tablet of "Gluttonous Pig" (`60`).
pub const ITEM_TABLET_GLUTTONOUS_PIG: u8 = 60;
/// Inventory item ID: Tablet of "The Seductress" (`61`).
pub const ITEM_TABLET_SEDUCTRESS: u8 = 61;
/// Inventory item ID: Horseshoe (`62`).
pub const ITEM_HORSESHOE: u8 = 62;
/// Inventory item ID: Lighter (`63`).
pub const ITEM_LIGHTER: u8 = 63;
/// Inventory item ID: Wax doll (`64`).
pub const ITEM_WAX_DOLL: u8 = 64;
/// Combined item ID: Wax doll + Horseshoe + Lighter (`96` at `sh2pc.exe` `0x00891858`).
pub const ITEM_COMBO_TRAPDOOR_HANDLE: u8 = 96;

/// Sound effect ID: Bug Room keypad button press (`0x2710` at `sh2pc.exe` `0x00586a20`).
pub const SE_ROACH_KEYPAD_BEEP: u32 = 0x2710;
/// Sound effect ID: Bug Room keypad correct 3-digit code unlock (`0x4650` at `sh2pc.exe` `0x00586b90`).
pub const SE_ROACH_KEYPAD_UNLOCK: u32 = 0x4650;
/// Sound effect ID: Bug Room keypad wrong 3-digit code buzzer (`0x4651` at `sh2pc.exe` `0x00586be0`).
pub const SE_ROACH_KEYPAD_ERROR: u32 = 0x4651;
/// Sound effect ID: Prison N floor trapdoor opened (`0x4658` at `sh2pc.exe` `0x00585b70`).
pub const SE_PRISON_TRAPDOOR_OPEN: u32 = 0x4658;
/// Sound effect ID: Prison tablet inserted into gallows slot (`0x4659` at `sh2pc.exe` `0x00584a10`).
pub const SE_SCAFFOLD_INSERT_PLATE: u32 = 0x4659;
/// Sound effect ID: Distant gallows scream upon inserting all 3 tablets (`0x426e` at `sh2pc.exe` `0x00584b40`).
pub const SE_SCAFFOLD_GALLOWS_SCREAM: u32 = 0x426e;

/// Exact 4-row `[trigger_flag, set0, set1, set2]` table `comb_use_flag` (`sh2pc.exe` `0x008d2804..0x008d2824`).
pub const SCAFFOLD_COMB_USE_FLAG_TABLE: [[u16; 4]; 4] = [
    [300, 294, 295, 296], // item 92: 59 + 60 + 61
    [297, 294, 295, 0],   // item 89: 59 + 60
    [298, 294, 296, 0],   // item 90: 59 + 61
    [299, 295, 296, 0],   // item 91: 60 + 61
];

/// Outcome of pressing one button (`0..=8`, representing keys `1..=9`) on the Delusion 3 Roach Room keypad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoachKeypadOutcome {
    /// Button `0..=8` accepted into slot `1` or `2` (`SE 0x2710`).
    DigitAccepted {
        /// Number of digits entered so far (`1` or `2`).
        digits_entered: u8,
        /// Sound effect played (`0x2710`).
        se_id: u32,
    },
    /// Third digit completed an exact match against `PZ_PANEL[0..3]`; clears flag `269` (`SE 0x4650`).
    Unlocked {
        /// Sound effect played (`0x4650`).
        se_id: u32,
    },
    /// Third digit did not match `PZ_PANEL[0..3]`; clears entered buffer (`SE 0x4651`).
    Rejected {
        /// Sound effect played (`0x4651`).
        se_id: u32,
    },
    /// Button index out of `0..=8` bounds.
    InvalidButton,
}

/// Interactive 3-digit input buffer for the Delusion 3 Roach Room keypad (`EvProgSubLockCheck`
/// `SLUS_202.28` `stg_delusion_3.c` / `sh2pc.exe` `0x00586910`).
///
/// Note on randomization parity: `FlagInit` (`0x0048aed0`) picks 3 distinct highlighted keys
/// `PZ_PANEL[0..3]` in `0..=8`, and `EvProgSubLockCheck` (`0x00586b40`) compares the 3 entered
/// buttons in exact order against `[PZ_PANEL[0], PZ_PANEL[1], PZ_PANEL[2]]` (one of the `3! = 6`
/// permutations of the 3 lit keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoachKeypadState {
    entered: [u8; 3],
    len: u8,
}

impl RoachKeypadState {
    /// Creates an empty 3-digit keypad input buffer.
    pub const fn new() -> Self {
        Self {
            entered: [0; 3],
            len: 0,
        }
    }

    /// Resets any partially entered digits.
    pub fn reset(&mut self) {
        self.entered = [0; 3];
        self.len = 0;
    }

    /// Returns the 3 highlighted button indices (`0..=8` for keys `1..=9`) from `PZ_PANEL` (`+0x14`).
    pub fn highlighted_buttons(puzzle: &PuzzleState) -> [u8; 3] {
        let raw = puzzle.raw();
        [raw[PZ_PANEL], raw[PZ_PANEL + 1], raw[PZ_PANEL + 2]]
    }

    /// Enters `button` (`0..=8` corresponding to keypad digits `1..=9`, `0x00586910`).
    pub fn press_button(
        &mut self,
        button: u8,
        puzzle: &PuzzleState,
        flags: &mut EventFlags,
    ) -> RoachKeypadOutcome {
        if button > 8 {
            return RoachKeypadOutcome::InvalidButton;
        }
        let idx = self.len as usize;
        self.entered[idx] = button;
        self.len += 1;
        if self.len < 3 {
            return RoachKeypadOutcome::DigitAccepted {
                digits_entered: self.len,
                se_id: SE_ROACH_KEYPAD_BEEP,
            };
        }
        let target = Self::highlighted_buttons(puzzle);
        let matched = self.entered == target;
        self.reset();
        if matched {
            flags.set(FLAG_DELUSION3_BUG_DOOR_LOCKED, false);
            RoachKeypadOutcome::Unlocked {
                se_id: SE_ROACH_KEYPAD_UNLOCK,
            }
        } else {
            RoachKeypadOutcome::Rejected {
                se_id: SE_ROACH_KEYPAD_ERROR,
            }
        }
    }
}

/// Applies the Delusion 3 Roach Room trap when unlocking the door with the Spiral-writing key
/// (`item 41`, `EvProgUseSpiralKey` / `EvRoomInit` at `sh2pc.exe` `0x005872a0`): consumes `41`,
/// locks the exit keypad (`flag 269`), marks key used (`flag 271`), and kills the flashlight
/// battery (`flag 272`).
pub fn apply_delusion3_spiral_key_trap(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if !inventory.has_item(ITEM_SPIRAL_WRITING_KEY) {
        return false;
    }
    inventory.item_lose(ITEM_SPIRAL_WRITING_KEY);
    flags.set(FLAG_DELUSION3_SPIRAL_KEY_USED, true);
    flags.set(FLAG_DELUSION3_BUG_DOOR_LOCKED, true);
    flags.set(FLAG_FLASHLIGHT_BATTERY_DEAD, true);
    true
}

/// Replaces the dead flashlight battery using the Dry Cell Battery (`item 55`,
/// `EvProgUseBattery` at `sh2pc.exe` `0x00587320`): consumes `55` and clears `flag 272`.
pub fn apply_dry_cell_battery(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if !flags.get(FLAG_FLASHLIGHT_BATTERY_DEAD) || !inventory.has_item(ITEM_DRY_CELL_BATTERY) {
        return false;
    }
    inventory.item_lose(ITEM_DRY_CELL_BATTERY);
    flags.set(FLAG_FLASHLIGHT_BATTERY_DEAD, false);
    true
}

/// Result of attempting to pry open the Prison North floor trapdoor (`0x005853f0` / `0x00585b30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapdoorUseOutcome {
    /// Single or 2-part combination (`62, 63, 64, 93, 94, 95`): sets hint flag without consuming parts.
    IncompleteHint {
        /// Inspection flag set (`280`, `281`, or `282`).
        hint_flag: usize,
    },
    /// Complete 3-part combination (`item 96` = Wax Doll + Horseshoe + Lighter): consumes parts,
    /// sets `flag 283`, and plays `SE 0x4658`.
    Opened {
        /// Opened flag (`283`).
        opened_flag: usize,
        /// Sound effect ID (`0x4658`).
        se_id: u32,
    },
    /// Item is unrelated to the Prison North trapdoor.
    NotApplicable,
}

/// Evaluates using `used_item_id` on the Toluca Prison North floor trapdoor (`EvProgTryOpenFloor`
/// `0x005853f0` & `EvProgSuccessOpenFloor` `0x00585b30` in `stg_prison_n.c`).
pub fn try_open_prison_trapdoor(
    used_item_id: u8,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> TrapdoorUseOutcome {
    match used_item_id {
        ITEM_HORSESHOE | ITEM_LIGHTER | 93 => {
            flags.set(FLAG_PRISON_TRAPDOOR_HORSESHOE_OR_LIGHTER, true);
            TrapdoorUseOutcome::IncompleteHint {
                hint_flag: FLAG_PRISON_TRAPDOOR_HORSESHOE_OR_LIGHTER,
            }
        }
        ITEM_WAX_DOLL | 94 => {
            flags.set(FLAG_PRISON_TRAPDOOR_WAX_DOLL, true);
            TrapdoorUseOutcome::IncompleteHint {
                hint_flag: FLAG_PRISON_TRAPDOOR_WAX_DOLL,
            }
        }
        95 => {
            flags.set(FLAG_PRISON_TRAPDOOR_MELTED_WAX_ONLY, true);
            TrapdoorUseOutcome::IncompleteHint {
                hint_flag: FLAG_PRISON_TRAPDOOR_MELTED_WAX_ONLY,
            }
        }
        ITEM_COMBO_TRAPDOOR_HANDLE => {
            if !inventory.has_item(ITEM_COMBO_TRAPDOOR_HANDLE) {
                return TrapdoorUseOutcome::NotApplicable;
            }
            inventory.item_lose(ITEM_COMBO_TRAPDOOR_HANDLE);
            flags.set(FLAG_PRISON_TRAPDOOR_OPENED, true);
            TrapdoorUseOutcome::Opened {
                opened_flag: FLAG_PRISON_TRAPDOOR_OPENED,
                se_id: SE_PRISON_TRAPDOOR_OPEN,
            }
        }
        _ => TrapdoorUseOutcome::NotApplicable,
    }
}

/// Result of inserting one or more Prison Tablets (`59..=61` or combos `89..=92`) into the
/// Toluca Prison South gallows scaffold (`EvProgSetScaffoldPlate` at `sh2pc.exe` `0x005847b0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaffoldPlateResult {
    /// Number of newly mounted tablets (`1..=3`).
    pub newly_mounted_count: u8,
    /// `true` if all three tablets (`301, 302, 303`) are now mounted (`flag 304` set, Horseshoe spawned).
    pub puzzle_completed: bool,
    /// Primary sound effect (`SE_SCAFFOLD_INSERT_PLATE`).
    pub insert_se_id: u32,
    /// Completion scream sound effect (`Some(SE_SCAFFOLD_GALLOWS_SCREAM)` when `puzzle_completed`).
    pub completion_se_id: Option<u32>,
}

/// Inserts single tablet (`59, 60, 61`) or combined tablets (`89..=92`) into the Prison Yard
/// gallows scaffold (`EvProgSetScaffoldPlate` `SLUS_202.28` `stg_prison_s.c` / `sh2pc.exe` `0x005847b0`).
pub fn apply_prison_scaffold_tablet(
    used_item_id: u8,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<ScaffoldPlateResult> {
    if !inventory.has_item(used_item_id) {
        return None;
    }
    match used_item_id {
        ITEM_TABLET_OPPRESSOR => flags.set(FLAG_SCAFFOLD_PENDING_OPPRESSOR, true),
        ITEM_TABLET_GLUTTONOUS_PIG => flags.set(FLAG_SCAFFOLD_PENDING_GLUTTONOUS_PIG, true),
        ITEM_TABLET_SEDUCTRESS => flags.set(FLAG_SCAFFOLD_PENDING_SEDUCTRESS, true),
        89 => flags.set(FLAG_SCAFFOLD_COMBO_89, true),
        90 => flags.set(FLAG_SCAFFOLD_COMBO_90, true),
        91 => flags.set(FLAG_SCAFFOLD_COMBO_91, true),
        92 => flags.set(FLAG_SCAFFOLD_COMBO_92, true),
        _ => return None,
    }
    inventory.item_lose(used_item_id);

    // Expand `comb_use_flag` (`0x008d2804..0x008d2824`) exactly as `0x00584840..0x0058490c`.
    for &[trig, s0, s1, s2] in &SCAFFOLD_COMB_USE_FLAG_TABLE {
        if flags.get(trig as usize) {
            for target in [s0, s1, s2] {
                if target != 0 {
                    flags.set(target as usize, true);
                }
            }
            flags.set(trig as usize, false);
        }
    }

    let mut newly_mounted = 0u8;
    for (pending, mounted) in [
        (FLAG_SCAFFOLD_PENDING_OPPRESSOR, FLAG_SCAFFOLD_SET_OPPRESSOR),
        (
            FLAG_SCAFFOLD_PENDING_GLUTTONOUS_PIG,
            FLAG_SCAFFOLD_SET_GLUTTONOUS_PIG,
        ),
        (
            FLAG_SCAFFOLD_PENDING_SEDUCTRESS,
            FLAG_SCAFFOLD_SET_SEDUCTRESS,
        ),
    ] {
        if flags.get(pending) {
            if !flags.get(mounted) {
                newly_mounted += 1;
            }
            flags.set(mounted, true);
            flags.set(pending, false);
        }
    }

    let all_mounted = flags.get(FLAG_SCAFFOLD_SET_OPPRESSOR)
        && flags.get(FLAG_SCAFFOLD_SET_GLUTTONOUS_PIG)
        && flags.get(FLAG_SCAFFOLD_SET_SEDUCTRESS);
    let completion_se_id = if all_mounted && !flags.get(FLAG_SCAFFOLD_ALL_SET) {
        flags.set(FLAG_SCAFFOLD_ALL_SET, true);
        Some(SE_SCAFFOLD_GALLOWS_SCREAM)
    } else {
        None
    };

    Some(ScaffoldPlateResult {
        newly_mounted_count: newly_mounted,
        puzzle_completed: all_mounted,
        insert_se_id: SE_SCAFFOLD_INSERT_PLATE,
        completion_se_id,
    })
}

/// Computes the effective frame-counter threshold for the Toluca Prison invisible chanting
/// prisoner (`Ritual` / `0x004ae5f8` in `sh2pc.exe`, SH2:EE `PrisonerTimer.cpp`).
///
/// - When `prisoner_timer_60fps_fix == false` (1:1 vanilla PS2/PC): returns `base_30hz_ticks`
///   unchanged (advancing twice as fast if ticked at 60 Hz).
/// - When `prisoner_timer_60fps_fix == true` and `sim_hz >= 60`: returns `base_30hz_ticks * 2`
///   so footstep pacing matches the original 30 Hz PS2 cadence.
#[inline]
pub const fn effective_prisoner_step_threshold(
    base_30hz_ticks: u32,
    sim_hz: u32,
    prisoner_timer_60fps_fix: bool,
) -> u32 {
    if prisoner_timer_60fps_fix && sim_hz >= 60 {
        base_30hz_ticks.saturating_mul(2)
    } else {
        base_30hz_ticks
    }
}
