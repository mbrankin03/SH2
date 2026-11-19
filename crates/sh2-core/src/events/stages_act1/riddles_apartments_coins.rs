//! Wood Side & Blue Creek Apartments coin, trash-chute, and fire-escape riddles
//! (`stg_apart_e2f.c`, `stg_apart_w1f.c`, `stg_apart_out.c`, `OldManCoinFix.cpp`, `PuzzleAlignmentFixes.cpp`).

use super::{
    Act1VanillaFixesConfig, ACT1_FLAG_CANNED_JUICE_DROPPED, ACT1_FLAG_FIRE_ESCAPE_UNLOCKED,
    ACT1_FLAG_OLD_MAN_COIN_SUB, ACT1_FLAG_OLD_MAN_COIN_TAKEN, ACT1_ITEM_CANNED_JUICE,
    ACT1_ITEM_COIN_OLD_MAN, ACT1_ITEM_FIRE_ESCAPE_KEY, ACT1_SE_TRASH_CHUTE_DROP,
};
use crate::config::RiddleDifficulty;
use crate::events::EventFlags;
use crate::inventory::InventoryManager;

/// Inspects the Wood Side 2F Trash Chute (`EvProgLookDustChute` at `sh2pc.exe` `0x0059ade0`).
///
/// Displays message `15`, plus hint message `16` exclusively on `RiddleDifficulty::Easy` (`riddle == 0`).
#[must_use]
pub fn act1_look_trash_chute(difficulty: RiddleDifficulty) -> &'static [u8] {
    if difficulty == RiddleDifficulty::Easy {
        &[15, 16]
    } else {
        &[15]
    }
}

/// Uses `Canned juice` (`49`) on the Wood Side 2F Trash Chute (`EvProgUseCannedJuice` `0x0059b010`).
pub fn act1_use_canned_juice_on_chute(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<u32> {
    if flags.get(ACT1_FLAG_CANNED_JUICE_DROPPED) || !inventory.has_item(ACT1_ITEM_CANNED_JUICE) {
        return None;
    }
    inventory.item_lose(ACT1_ITEM_CANNED_JUICE);
    flags.set(ACT1_FLAG_CANNED_JUICE_DROPPED, true);
    Some(ACT1_SE_TRASH_CHUTE_DROP)
}

/// Evaluates whether the 3D `Coin [Old Man]` model (`kind = 0x071f`, `Stage 13` `mdl_list[1]` at `0x008de900`)
/// is visible in the Courtyard trash pile.
///
/// - **Vanilla PS2 (`fixes.old_man_coin_fix == false`):** `flag_on = 0, flag_off = 118` (`0x76`) -> visible even before dropping Canned Juice!
/// - **SH2:EE (`fixes.old_man_coin_fix == true`):** `flag_on = 95` (`0x5f`), `flag_off = 118` (`0x76`, `OldManCoinFix.cpp:30`).
#[must_use]
pub fn act1_is_old_man_coin_prop_visible(
    flags: &EventFlags,
    fixes: Act1VanillaFixesConfig,
) -> bool {
    if flags.get(ACT1_FLAG_OLD_MAN_COIN_SUB) || flags.get(ACT1_FLAG_OLD_MAN_COIN_TAKEN) {
        return false;
    }
    if fixes.old_man_coin_fix {
        flags.get(ACT1_FLAG_CANNED_JUICE_DROPPED)
    } else {
        true
    }
}

/// Picks up `Coin [Old Man]` (`51`) from the Wood Side Courtyard chute (`EvProgGetCoinOfOldman` `0x00595810`).
pub fn act1_pickup_old_man_coin_courtyard(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if !flags.get(ACT1_FLAG_CANNED_JUICE_DROPPED) || flags.get(ACT1_FLAG_OLD_MAN_COIN_TAKEN) {
        return false;
    }
    inventory.item_get_now(ACT1_ITEM_COIN_OLD_MAN);
    flags.set(ACT1_FLAG_OLD_MAN_COIN_TAKEN, true);
    flags.set(ACT1_FLAG_OLD_MAN_COIN_SUB, true);
    true
}

/// Unlocks the Wood Side 2F West Fire-Escape door into Blue Creek (`ACT1_APART_E2F_EVENT_LIST[31]`:
/// `cond = 0xa12831ec` -> requires & consumes `ACT1_ITEM_FIRE_ESCAPE_KEY` `30`, sets `flag(93)`).
pub fn act1_use_fire_escape_key(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
    if flags.get(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED) || !inventory.has_item(ACT1_ITEM_FIRE_ESCAPE_KEY) {
        return false;
    }
    inventory.item_lose(ACT1_ITEM_FIRE_ESCAPE_KEY);
    flags.set(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED, true);
    true
}

/// Which of the three Apartment Coins is being rendered in the Rm 105 5-slot desk cutaway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act1DeskCoinKind {
    /// `Coin [Old Man]` (`51`, `coin_idx = 1` in `EvProgSubDrawDesk`).
    OldMan,
    /// `Coin [Snake]` (`50`, `coin_idx = 0` in `EvProgSubDrawDesk`).
    Snake,
    /// `Coin [Prisoner]` (`52`, `coin_idx = 2` in `EvProgSubDrawDesk`).
    Prisoner,
}

/// Retail `3 × 5` `[x0, y0]` table (`[Snake, OldMan, Prisoner]` × `slot 0..=4`) from `sh2pc.exe` `0x008e0538`
/// (`EvProgSubDrawDesk` `0x00597b10..0x00597cc7`, paired with `width = 0x30 << 4 = 768` at `0x008e057a`
/// and `height = 0x40 << 4 = 1024` at `0x008e057b`).
pub const ACT1_VANILLA_COIN_DESK_POS: [[[i16; 2]; 5]; 3] = [
    [
        [-3159, -730],
        [-1787, -703],
        [-410, -667],
        [972, -642],
        [2358, -609],
    ],
    [
        [-3167, -697],
        [-1794, -668],
        [-420, -641],
        [968, -614],
        [2352, -581],
    ],
    [
        [-3169, -729],
        [-1796, -704],
        [-416, -671],
        [964, -636],
        [2345, -610],
    ],
];

/// 2D cutaway quad UV (`[u1, v1, u2, v2]`) and screen vertex bounds (`[x0, y0, x1, y1]`) for a coin
/// in `R_APT_W_RM_105` (`0x22`), preserving both 1:1 vanilla PS2 (`0x008e0538`) and SH2:EE `CoinPuzzleFix`
/// (`Patches/PuzzleAlignmentFixes.cpp` lines 98–181).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Act1CoinQuadLayout {
    /// Texture coordinates `[u1, v1, u2, v2]` in Q4 texel space.
    pub uv: [f32; 4],
    /// Screen-space quad corners `[x0, y0, x1, y1]` in Q4 pixel units.
    pub bounds: [i16; 4],
}

/// Computes the exact coin sprite UVs and slot bounds for `slot` (`0..=4`), gated by `fixes.coin_puzzle_alignment_fix`.
#[must_use]
pub const fn act1_coin_puzzle_quad_layout(
    coin: Act1DeskCoinKind,
    slot: usize,
    fixes: Act1VanillaFixesConfig,
) -> Option<Act1CoinQuadLayout> {
    if slot >= 5 {
        return None;
    }
    let (coin_row, uv) = if fixes.coin_puzzle_alignment_fix {
        match coin {
            Act1DeskCoinKind::Snake => (0, [0.0, 1055.0, 1024.0, 2075.0]),
            Act1DeskCoinKind::OldMan => (1, [0.0, 0.0, 1024.0, 1024.0]),
            Act1DeskCoinKind::Prisoner => (2, [-4.0, 2082.0, 1028.0, 3108.0]),
        }
    } else {
        match coin {
            Act1DeskCoinKind::Snake => (0, [0.0, 1024.0, 752.0, 2032.0]),
            Act1DeskCoinKind::OldMan => (1, [0.0, 0.0, 752.0, 1008.0]),
            Act1DeskCoinKind::Prisoner => (2, [0.0, 2048.0, 752.0, 3056.0]),
        }
    };
    let bounds = if fixes.coin_puzzle_alignment_fix {
        match slot {
            0 => [-3160, -708, -2090, 362],
            1 => [-1790, -682, -720, 388],
            2 => [-414, -650, 656, 420],
            3 => [962, -612, 2032, 458],
            _ => [2348, -580, 3418, 490],
        }
    } else {
        let [x0, y0] = ACT1_VANILLA_COIN_DESK_POS[coin_row][slot];
        [x0, y0, x0 + 768, y0 + 1024]
    };
    Some(Act1CoinQuadLayout { uv, bounds })
}
