//! Labyrinth (West/East/North), Eddie Boss, and Toluca Lake Riddles & Lifecycle (`Stages 34..=38`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28` (`stg_labyrinth_w.c`, `stg_labyrinth_e.c`,
//! `stg_labyrinth_n.c`, `stg_eddie_boss.c`, `stg_lake.c`) and retail PC `sh2pc.exe`
//! (`0x0057ea50..0x00582c80`, `cube_check` at `0x008d14d4`).

use std::f32::consts::FRAC_PI_2;

use crate::config::RiddleDifficulty;
use crate::events::{EventFlags, PuzzleState, PZ_HANGMAN};
use crate::inventory::InventoryManager;
use crate::types::wrap_angle;

/// Flag `317` (`0x01f7aa04` bit `29`): Rotating Face Cube `plane` bit `0` (`0x00581ca2`).
pub const FLAG_CUBE_PLANE_BIT0: usize = 317;
/// Flag `318` (`0x01f7aa04` bit `30`): Rotating Face Cube `plane` bit `1` (`0x00581c9a`).
pub const FLAG_CUBE_PLANE_BIT1: usize = 318;
/// Flag `319` (`0x01f7aa04` bit `31`): Rotating Face Cube `roll` bit `0` (`0x00581cd0`).
pub const FLAG_CUBE_ROLL_BIT0: usize = 319;
/// Flag `320` (`0x01f7aa08` bit `0`): Rotating Face Cube `roll` bit `1` (`0x00581ccd`).
pub const FLAG_CUBE_ROLL_BIT1: usize = 320;
/// Flag `321` (`0x01f7aa08` bit `1`): Rotating Face Cube initial pitch offset `itr` bit `0` (`0x00581cbc`).
pub const FLAG_CUBE_ITR_BIT0: usize = 321;
/// Flag `322` (`0x01f7aa08` bit `2`): Rotating Face Cube initial pitch offset `itr` bit `1` (`0x00581cb7`).
pub const FLAG_CUBE_ITR_BIT1: usize = 322;
/// Flag `325` (`0x01f7aa08` bit `5`): Rotating room passage connected (`cube_check[plane][roll] > 0`, `0x00582520`).
pub const FLAG_CUBE_PASSAGE_OPEN: usize = 325;
/// Flag `326` (`0x01f7aa08` bit `6`): Maria cell passage connected (`cube_check[plane][roll] > 1`, `0x00582536`).
pub const FLAG_CUBE_MARIA_PATH_OPEN: usize = 326;

/// Flag `330` (`Stage 34 ev[9]`): Electrified wire fence cut with Wire Cutter (`item 65`, `0x00582810`).
pub const FLAG_LABYRINTH_WIRE_CUT: usize = 330;
/// Flag `332` (`0x01f7aa08` bit `12`): Labyrinth East spark hazard deactivated (`0x0058130c`).
pub const FLAG_LABYRINTH_SPARK_DEACTIVATED: usize = 332;

/// Base flag `337` (`337..=342`): 6 Hanged Men corpse placard inspection triggers (`0x0057f560`).
pub const FLAG_HANGMAN_CORPSE_INSPECT_BASE: usize = 337;
/// Base flag `343` (`343..=348`): Correct innocent noose pulled (`digit == 5` at `0x0057f9fa`), spawning Key of "Persecuted".
pub const FLAG_HANGMAN_CORRECT_ROPE_BASE: usize = 343;
/// Base flag `349` (`349..=354`): 6 Hanged Men noose pull trigger flags (`EvProgHangRope` `0x0057f780`).
pub const FLAG_HANGMAN_PULL_TRIGGER_BASE: usize = 349;
/// Base flag `355` (`355..=360`): Pulled noose state flags (`0x0057fa15`, cleared on wrong-pull room entry `0x00580250`).
pub const FLAG_HANGMAN_ROPE_PULLED_BASE: usize = 355;
/// Flag `361`: Hanged Men poem memo Easy (`0x0057fab0`, message `9`).
pub const FLAG_HANGMAN_POEM_EASY: usize = 361;
/// Flag `362`: Hanged Men poem memo Normal (`0x0057fab0`, message `10`).
pub const FLAG_HANGMAN_POEM_NORMAL: usize = 362;
/// Flag `363`: Hanged Men poem memo Hard (`0x0057fab0`, message `11`).
pub const FLAG_HANGMAN_POEM_HARD: usize = 363;
/// Flag `612`: Hanged Men poem memo Extra (`0x0057fab0`, message `12`).
pub const FLAG_HANGMAN_POEM_EXTRA: usize = 612;
/// Flag `365` (`0x01f7aa0c` bit `13`): Wrong noose penalty ambush triggered on entering corpse room `0x8b` (`0x00580250`).
pub const FLAG_HANGMAN_WRONG_ROPE_AMBUSH: usize = 365;
/// Flag `367` (`0x01f7aa0c` bit `15`): Handcuffs unlocked with Key of "Persecuted" (`item 42`, `0x0057fb80`).
pub const FLAG_HANGMAN_HANDCUFFS_UNLOCKED: usize = 367;

/// Inventory item ID: Key of "Persecuted" (`42`).
pub const ITEM_KEY_OF_PERSECUTED: u8 = 42;
/// Inventory item ID: Wire cutter (`65`).
pub const ITEM_WIRE_CUTTER: u8 = 65;

/// Sound effect ID: Rotating Face Cube vertical pitch (`Up`/`Down`, `0x4970` at `0x00581f63`).
pub const SE_CUBE_PITCH: u32 = 0x4970;
/// Sound effect ID: Rotating Face Cube horizontal yaw (`Left`/`Right`, `0x4971` at `0x00581e60`).
pub const SE_CUBE_YAW: u32 = 0x4971;
/// Sound effect ID: Rotating Face Cube stone rumble (`0x4972` at `0x00581e70`).
pub const SE_CUBE_RUMBLE: u32 = 0x4972;
/// Sound effect ID: Key of "Persecuted" unlocking valve handcuffs (`0x465c` at `0x0057fb80`).
pub const SE_FALSE_CHARGE_KEY_UNLOCK: u32 = 0x465c;

/// Exact `4 x 4` doorway connectivity lookup table `cube_check[plane][roll]` (`sh2pc.exe` `0x008d14d4`):
/// - `0`: Rotating room doorways blocked (`flag 325 = false, flag 326 = false`).
/// - `1`: Lower passage open (`flag 325 = true, flag 326 = false`).
/// - `2`: Maria cell upper passage open (`flag 325 = true, flag 326 = true`).
pub const LABYRINTH_CUBE_CHECK: [[u8; 4]; 4] =
    [[2, 1, 2, 0], [1, 0, 1, 0], [0, 1, 1, 0], [0, 0, 0, 0]];

/// Crime placard stage `.mes` indices (`digit 0..=5`, where `digit 5` is ALWAYS the innocent man,
/// `sh2pc.exe` `0x0057f720..0x0057f77f`).
pub const HANGMAN_CRIME_MESSAGE_TABLE: [[u8; 6]; 4] = [
    [7, 3, 6, 5, 1, 2], // Easy (0x0057f720): innocent digit 5 -> msg 2 (Kidnapper/Bodily Injury framing)
    [0, 2, 1, 6, 3, 7], // Normal (0x0057f738): innocent digit 5 -> msg 7 (Arsonist framing)
    [5, 3, 2, 4, 0, 6], // Hard (0x0057f750): innocent digit 5 -> msg 6 (Counterfeiter framing)
    [0, 5, 7, 2, 3, 1], // Extra (0x0057f768): innocent digit 5 -> msg 1 (Thief framing)
];

/// Input turn direction for the Labyrinth Rotating Face Cube (`EvProgControlCube` `0x00581c50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeTurnDirection {
    /// D-Pad Up (`0x00581f08`): `plane = (plane + 1) & 3`, pitch `+PI / 2`.
    Up,
    /// D-Pad Down (`0x00581f81`): `plane = (plane - 1) & 3`, pitch `-PI / 2`.
    Down,
    /// D-Pad Right (`0x00581df2`): `roll = (roll + 1) & 3`, yaw `+PI / 2`.
    Right,
    /// D-Pad Left (`0x00581e7e`): `roll = (roll - 1) & 3`, yaw `-PI / 2`.
    Left,
}

/// Synchronized 3D orientation and live room doorway state of the Labyrinth Rotating Face Cube.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceCubeState {
    /// Logical vertical plane index (`0..=3`, stored in flags `317..=318`).
    pub plane: u8,
    /// Logical horizontal roll/yaw index (`0..=3`, stored in flags `319..=320`).
    pub roll: u8,
    /// Randomized initial pitch offset (`0..=3`, stored in flags `321..=322`).
    pub itr: u8,
    /// Live 3D face-head pitch angle in radians (`(plane - itr) * PI / 2`, wrapped to `[-PI, PI)`).
    pub pitch_rad: f32,
    /// Live 3D face-head & rotating-room yaw angle in radians (`roll * PI / 2`, wrapped to `[-PI, PI)`).
    pub yaw_rad: f32,
    /// `cube_check[plane][roll]` (`0`, `1`, or `2`).
    pub connectivity_level: u8,
    /// Whether the standard passage doorway is aligned (`flag 325`).
    pub passage_open: bool,
    /// Whether the Maria cell upper doorway is aligned (`flag 326`).
    pub maria_path_open: bool,
    /// Sound effects triggered by the rotation (`[primary_se, SE_CUBE_RUMBLE]` when turned).
    pub sound_effects: Option<[u32; 2]>,
}

/// Reads the current [`FaceCubeState`] from [`EventFlags`] and synchronizes doorway flags `325`/`326`
/// (`EvProgControlCube` `sh2pc.exe` `0x00581c93..0x00582550`).
pub fn sync_labyrinth_face_cube(flags: &mut EventFlags) -> FaceCubeState {
    let plane = u8::from(flags.get(FLAG_CUBE_PLANE_BIT0))
        | (u8::from(flags.get(FLAG_CUBE_PLANE_BIT1)) << 1);
    let roll =
        u8::from(flags.get(FLAG_CUBE_ROLL_BIT0)) | (u8::from(flags.get(FLAG_CUBE_ROLL_BIT1)) << 1);
    let itr =
        u8::from(flags.get(FLAG_CUBE_ITR_BIT0)) | (u8::from(flags.get(FLAG_CUBE_ITR_BIT1)) << 1);

    let level = LABYRINTH_CUBE_CHECK[(plane & 3) as usize][(roll & 3) as usize];
    let passage_open = level > 0;
    let maria_path_open = level > 1;
    flags.set(FLAG_CUBE_PASSAGE_OPEN, passage_open);
    flags.set(FLAG_CUBE_MARIA_PATH_OPEN, maria_path_open);

    let pitch_rad = wrap_angle(((plane as i32) - (itr as i32)) as f32 * FRAC_PI_2);
    let yaw_rad = wrap_angle((roll as f32) * FRAC_PI_2);

    FaceCubeState {
        plane,
        roll,
        itr,
        pitch_rad,
        yaw_rad,
        connectivity_level: level,
        passage_open,
        maria_path_open,
        sound_effects: None,
    }
}

/// Rotates the Labyrinth Face Cube by 90 degrees in `direction`, writing updated `(plane, roll)`
/// flags `317..=320` and synchronizing room doorway flags `325`/`326` (`0x00581df1..0x00582550`).
pub fn rotate_labyrinth_face_cube(
    flags: &mut EventFlags,
    direction: CubeTurnDirection,
) -> FaceCubeState {
    let cur = sync_labyrinth_face_cube(flags);
    let (next_plane, next_roll, primary_se) = match direction {
        CubeTurnDirection::Up => ((cur.plane.wrapping_add(1)) & 3, cur.roll, SE_CUBE_PITCH),
        CubeTurnDirection::Down => ((cur.plane.wrapping_sub(1)) & 3, cur.roll, SE_CUBE_PITCH),
        CubeTurnDirection::Right => (cur.plane, (cur.roll.wrapping_add(1)) & 3, SE_CUBE_YAW),
        CubeTurnDirection::Left => (cur.plane, (cur.roll.wrapping_sub(1)) & 3, SE_CUBE_YAW),
    };
    flags.set(FLAG_CUBE_PLANE_BIT0, (next_plane & 1) != 0);
    flags.set(FLAG_CUBE_PLANE_BIT1, (next_plane & 2) != 0);
    flags.set(FLAG_CUBE_ROLL_BIT0, (next_roll & 1) != 0);
    flags.set(FLAG_CUBE_ROLL_BIT1, (next_roll & 2) != 0);

    let mut updated = sync_labyrinth_face_cube(flags);
    updated.sound_effects = Some([primary_se, SE_CUBE_RUMBLE]);
    updated
}

/// Uses the Wire Cutter (`item 65`) on the electrified spark gate (`Stage 34 ev[9]`, `0x00582810`),
/// consuming `65` and setting `flag 330` and `flag 332`.
pub fn apply_labyrinth_wire_cutter(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if !inventory.has_item(ITEM_WIRE_CUTTER) {
        return false;
    }
    inventory.item_lose(ITEM_WIRE_CUTTER);
    flags.set(FLAG_LABYRINTH_WIRE_CUT, true);
    flags.set(FLAG_LABYRINTH_SPARK_DEACTIVATED, true);
    true
}

/// Decodes the 6-element permutation `[d0, d1, d2, d3, d4, d5]` stored in `PZ_HANGMAN` (`+0x18`,
/// `sh2pc.exe` `0x0057f6e0` & `0x0057f9e0`: `digit[slot] = (pz / 6^slot) % 6`).
pub fn decode_hangman_digits(puzzle: &PuzzleState) -> [u8; 6] {
    let mut v = puzzle.read_u16(PZ_HANGMAN);
    let mut out = [0u8; 6];
    for slot in &mut out {
        *slot = (v % 6) as u8;
        v /= 6;
    }
    out
}

/// Returns the slot index (`0..=5`) of the innocent prisoner (`digit == 5` at `sh2pc.exe` `0x0057f9fa`).
pub fn hangman_innocent_slot(puzzle: &PuzzleState) -> Option<usize> {
    decode_hangman_digits(puzzle)
        .iter()
        .position(|&digit| digit == 5)
}

/// Inspects Hanged Man corpse `slot` (`0..=5`), returning its difficulty-remapped `.mes` crime ID
/// (`EvProgHangingCorpse` at `sh2pc.exe` `0x0057f560`).
pub fn inspect_hanged_corpse_message(
    slot: usize,
    riddle: RiddleDifficulty,
    puzzle: &PuzzleState,
    flags: &mut EventFlags,
) -> Option<u8> {
    if slot >= 6 {
        return None;
    }
    flags.set(FLAG_HANGMAN_CORPSE_INSPECT_BASE + slot, true);
    let digit = decode_hangman_digits(puzzle)[slot] as usize;
    if digit >= 6 {
        return None;
    }
    let level_idx = match riddle {
        RiddleDifficulty::Easy => 0,
        RiddleDifficulty::Normal => 1,
        RiddleDifficulty::Hard => 2,
        RiddleDifficulty::Extra => 3,
    };
    Some(HANGMAN_CRIME_MESSAGE_TABLE[level_idx][digit])
}

/// Reads the 6 Hanged Men gallows poem (`EvProgReadHangHint` at `sh2pc.exe` `0x0057fab0`), setting
/// the difficulty-specific memo flag (`361, 362, 363, 612`) and returning `(memo_flag, message_id)`.
pub fn read_hangman_poem_hint(riddle: RiddleDifficulty, flags: &mut EventFlags) -> (usize, u8) {
    let (flag_id, msg_id) = match riddle {
        RiddleDifficulty::Easy => (FLAG_HANGMAN_POEM_EASY, 9),
        RiddleDifficulty::Normal => (FLAG_HANGMAN_POEM_NORMAL, 10),
        RiddleDifficulty::Hard => (FLAG_HANGMAN_POEM_HARD, 11),
        RiddleDifficulty::Extra => (FLAG_HANGMAN_POEM_EXTRA, 12),
    };
    flags.set(flag_id, true);
    (flag_id, msg_id)
}

/// Outcome of pulling noose `slot` (`0..=5`) in the Labyrinth gallows room (`EvProgHangRope` `0x0057f780`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HangmanNooseOutcome {
    /// Correct innocent prisoner's noose pulled (`digit == 5`): sets `355 + slot` and `343 + slot`.
    CorrectInnocent {
        /// Slot (`0..=5`).
        slot: usize,
        /// Spawned key flag (`343 + slot`).
        reward_flag: usize,
    },
    /// Guilty prisoner's noose pulled (`digit != 5`): sets `355 + slot` (triggers ambush on entering `0x8b`).
    WrongGuilty {
        /// Slot (`0..=5`).
        slot: usize,
    },
    /// Invalid slot (`>= 6`) or rope already pulled.
    InvalidOrAlreadyPulled,
}

/// Pulls noose `slot` (`0..=5`) in the 6 Hanged Men puzzle (`EvProgHangRope` `sh2pc.exe` `0x0057f780`).
pub fn pull_hangman_noose(
    slot: usize,
    puzzle: &PuzzleState,
    flags: &mut EventFlags,
) -> HangmanNooseOutcome {
    if slot >= 6 || flags.get(FLAG_HANGMAN_ROPE_PULLED_BASE + slot) {
        return HangmanNooseOutcome::InvalidOrAlreadyPulled;
    }
    flags.set(FLAG_HANGMAN_PULL_TRIGGER_BASE + slot, false);
    flags.set(FLAG_HANGMAN_ROPE_PULLED_BASE + slot, true);
    let digit = decode_hangman_digits(puzzle)[slot];
    if digit == 5 {
        let reward_flag = FLAG_HANGMAN_CORRECT_ROPE_BASE + slot;
        flags.set(reward_flag, true);
        HangmanNooseOutcome::CorrectInnocent { slot, reward_flag }
    } else {
        HangmanNooseOutcome::WrongGuilty { slot }
    }
}

/// Executes the `stage_labyrinth_n` `EvRoomInit` check when entering the Hanged Men corpse room
/// (`room 0x8b`, `sh2pc.exe` `0x00580250`): if any rope (`355..=360`) was pulled without solving
/// (`343..=348` all false), resets pulled-rope flags `355..=360` and sets ambush `flag 365`.
pub fn on_enter_hangman_corpse_room(flags: &mut EventFlags) -> bool {
    let any_solved = (0..6).any(|i| flags.get(FLAG_HANGMAN_CORRECT_ROPE_BASE + i));
    if any_solved {
        return false;
    }
    let any_pulled = (0..6).any(|i| flags.get(FLAG_HANGMAN_ROPE_PULLED_BASE + i));
    if any_pulled {
        for i in 0..6 {
            flags.set(FLAG_HANGMAN_ROPE_PULLED_BASE + i, false);
        }
        flags.set(FLAG_HANGMAN_WRONG_ROPE_AMBUSH, true);
        true
    } else {
        false
    }
}

/// Uses Key of "Persecuted" (`item 42`) on the Labyrinth valve handcuffs (`EvProgUseFalseChargeKey`
/// `sh2pc.exe` `0x0057fb80`), consuming `42`, setting `flag 367`, and returning `SE 0x465c`.
pub fn apply_key_of_persecuted(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<u32> {
    if !inventory.has_item(ITEM_KEY_OF_PERSECUTED) {
        return None;
    }
    inventory.item_lose(ITEM_KEY_OF_PERSECUTED);
    flags.set(FLAG_HANGMAN_HANDCUFFS_UNLOCKED, true);
    Some(SE_FALSE_CHARGE_KEY_UNLOCK)
}
