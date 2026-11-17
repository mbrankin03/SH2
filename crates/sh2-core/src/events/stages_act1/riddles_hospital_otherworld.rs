//! Brookhaven Hospital (Otherworld, Stages 22–27) interactive riddles & horror sequences:
//! 2F Day Room Refrigerator (`reizouko`), B1 Storeroom Shelf & Copper Ring, 3F Angel Door Rings,
//! Trick-or-Treat Elevator Radio Quiz (`stg_hospital_bf_b.c`), 3F Store Room Quiz Chest & Poison
//! Gas Trap (`stg_hospital_3f_b.c`), and Basement Passage Pyramid Head Chase (`stg_hospital_pass.c`).

use super::ACT1_FLAG_MARIA_ACCOMPANYING;
use crate::events::EventFlags;
use crate::inventory::InventoryManager;

/// Item ID for Shotgun Shells (`7`, 5x inside 3F Store Room Quiz Chest `gi_list[2..=6]`).
pub const ACT1_ITEM_SHOTGUN_SHELLS: u8 = 7;
/// Item ID for Ampoule (`3`, 2x inside 3F Store Room Quiz Chest `gi_list[8..=9]`).
pub const ACT1_ITEM_AMPOULE: u8 = 3;
/// Item ID for Storeroom Key (`38`, `0x26`, `EvProgUseStoreroomKey` `0x00588400`).
pub const ACT1_ITEM_STOREROOM_KEY: u8 = 38;
/// Item ID for Dry Cell Battery (`55`, `0x37`, `EvProgGetDrycell` `0x0058ad00`).
pub const ACT1_ITEM_DRY_CELL_BATTERY: u8 = 55;
/// Item ID for Copper Ring (`56`, `0x38`, B1 Basement `EvProgGetCopperRing` `0x005883f0`).
pub const ACT1_ITEM_COPPER_RING: u8 = 56;
/// Item ID for Lead Ring (`57`, `0x39`, 2F Refrigerator `EvProgRefrigeratorOpen` `0x0058a88b`).
pub const ACT1_ITEM_LEAD_RING: u8 = 57;
/// Combination pseudo-item ID `88`: `Copper ring (56) + Lead ring (57)`.
pub const ACT1_COMBO_COPPER_AND_LEAD_RINGS: u8 = 88;

/// Flag `219` (`0xdb`, `0x1f7a9f8 |= 0x0800_0000`): 2F Refrigerator opened with Maria & Lead Ring taken (`0x0058a9d0`).
pub const ACT1_FLAG_REFRIGERATOR_OPENED: usize = 219;
/// Flag `223` (`0xdf`, `0x1f7a9f8 |= 0x8000_0000`): B1 Storeroom shelf pushed aside (`0x00588370`).
pub const ACT1_FLAG_BASEMENT_SHELF_MOVED: usize = 223;
/// Flag `227` (`0xe3`, `0x1f7a9fc |= 0x08`): Copper Ring placed on 3F Basement Angel Door (`0x0058a282`).
pub const ACT1_FLAG_ANGEL_COPPER_PLACED: usize = 227;
/// Flag `228` (`0xe4`, `0x1f7a9fc |= 0x10`): Lead Ring placed on 3F Basement Angel Door (`0x0058a314`).
pub const ACT1_FLAG_ANGEL_LEAD_PLACED: usize = 228;
/// Flag `232` (`0xe8`, `0x1f7a9fc |= 0x0100`): Trick-or-Treat Elevator Radio Quiz broadcast completed (`0x00589389`).
pub const ACT1_FLAG_ELEVATOR_QUIZ_HEARD: usize = 232;
/// Flag `233` (`0xe9`, `0x1f7a9fc |= 0x0200`): 3F Store Room Quiz Chest attempted/locked (`0x005897d7`).
pub const ACT1_FLAG_QUIZ_CHEST_ATTEMPTED: usize = 233;
/// Base flag `234` (`234..=239`, `0x1f7a9fc` bits `10..=15`): 3x2-bit Quiz Chest button selections (`0x005898c2..0x00589bec`).
pub const ACT1_FLAG_QUIZ_BTN_BIT0: usize = 234;
/// Flag `240` (`0xf0`, `0x1f7a9fc |= 0x1_0000`): Quiz Chest solved correctly `(3, 1, 3)` (`0x005897d7`).
pub const ACT1_FLAG_QUIZ_CHEST_SOLVED: usize = 240;
/// Flag `241` (`0xf1`, `0x1f7a9fc |= 0x2_0000`): Quiz Chest answered wrong -> poison spray (`0x00589804`).
pub const ACT1_FLAG_QUIZ_CHEST_WRONG: usize = 241;
/// Flag `1964` (`0x7ac`, `0x1f7aad4 |= 0x10`): Refrigerator solo-attempt / decline flag (`0x0058a790`, `0x0058aa74`).
pub const ACT1_FLAG_REFRIGERATOR_TRIED_SOLO: usize = 1964;

/// Sound effect `17007` (`0x426f`): Quiz Chest push-button click (`0x005898af`).
pub const ACT1_SE_QUIZ_BUTTON_CLICK: u32 = 0x426f;
/// Sound effect `19011` (`0x4a43`): Hospital Elevator travel hum (`0x00589155`).
pub const ACT1_SE_ELEVATOR_TRAVEL: u32 = 0x4a43;
/// Sound effect `19012` (`0x4a44`): Hospital Elevator arrival chime (`0x005893a5`).
pub const ACT1_SE_ELEVATOR_ARRIVE: u32 = 0x4a44;
/// Sound effect `19025` (`0x4a51`): 3F Basement Angel Door unlock (`0x0058a36e`).
pub const ACT1_SE_ANGEL_DOOR_UNLOCK: u32 = 0x4a51;
/// Sound effect `19894` (`0x4db6`): Quiz Chest latch unlock (`0x005897dd`).
pub const ACT1_SE_QUIZ_CHEST_UNLOCK: u32 = 0x4db6;
/// Sound effect `19895` (`0x4db7`): Quiz Chest poison gas spray trap (`0x0058980a`).
pub const ACT1_SE_QUIZ_GAS_TRAP: u32 = 0x4db7;
/// Streamed audio ID `60107` (`0xeacb`): Trick-or-Treat Elevator Radio Quiz host voice (`0x005890c3`).
pub const ACT1_STREAM_TRICK_OR_TREAT_RADIO: u32 = 0xeacb;

/// Total duration of the Trick-or-Treat Elevator Radio Quiz in 30 Hz ticks (`5175.0`, `sh2pc.exe` `0x0072a944`).
pub const ACT1_ELEVATOR_QUIZ_TOTAL_TICKS: f32 = 5175.0;

/// Exact 42 `[show_tick, hide_tick]` subtitle windows for messages `32..=73` during the Trick-or-Treat
/// Elevator Radio Quiz, extracted 1:1 from `sh2pc.exe` `0x008d5618` (`EvProgElevatorButton` `0x00589055`).
#[rustfmt::skip]
pub const ACT1_ELEVATOR_RADIO_SUBTITLES: [(u16, u16); 42] = [
    (218, 330),   (330, 523),   (523, 681),   (681, 829),   (829, 909),   (909, 1082),
    (1082, 1178), (1178, 1306), (1306, 1384), (1384, 1474), (1474, 1598), (1598, 1713),
    (1713, 1785), (1785, 1927), (1927, 1998), (1998, 2084), (2084, 2184), (2184, 2317),
    (2317, 2441), (2441, 2675), (2675, 2930), (2930, 3015), (3015, 3176), (3176, 3329),
    (3329, 3405), (3405, 3484), (3484, 3584), (3584, 3670), (3670, 3794), (3794, 3926),
    (3926, 3995), (3995, 4077), (4077, 4209), (4209, 4272), (4272, 4334), (4334, 4411),
    (4411, 4473), (4473, 4550), (4550, 4695), (4695, 4836), (4836, 5001), (5001, 5100),
];

/// Outcome of interacting with the 2F Day Room Refrigerator (`EvProgRefrigeratorOpen` `0x0058a6d0` /
/// `EvProgRefrigeratorNotOpen` `0x0058aa50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act1RefrigeratorOutcome {
    /// Player declined the Yes/No prompt (`msg 0`); sets `flag(1964)` when Maria is present.
    Declined,
    /// Maria is absent (`!flag(15)`): plays `"reizouko_ng"`, displays `msg 2` ("I can't open it by myself").
    TooHeavyAlone,
    /// Maria helps open the door (`flag(15)`): plays `"reizouko_ok"`, awards `Lead ring` (`57`, `msg 5`), sets `flag(219)`.
    OpenedWithMaria,
    /// Refrigerator was already opened (`flag(219)`).
    AlreadyOpened,
}

/// Evaluates the 2F Day Room Refrigerator interaction (`sh2pc.exe` `0x0058a6d0` & `0x0058aa50`).
pub fn act1_interact_hospital_refrigerator(
    chose_yes: bool,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Act1RefrigeratorOutcome {
    if flags.get(ACT1_FLAG_REFRIGERATOR_OPENED) {
        return Act1RefrigeratorOutcome::AlreadyOpened;
    }
    let maria_present = flags.get(ACT1_FLAG_MARIA_ACCOMPANYING);
    if !chose_yes {
        if maria_present {
            flags.set(ACT1_FLAG_REFRIGERATOR_TRIED_SOLO, true);
        }
        return Act1RefrigeratorOutcome::Declined;
    }
    if !maria_present {
        flags.set(ACT1_FLAG_REFRIGERATOR_TRIED_SOLO, true);
        return Act1RefrigeratorOutcome::TooHeavyAlone;
    }
    inventory.item_get_now(ACT1_ITEM_LEAD_RING);
    flags.set(ACT1_FLAG_REFRIGERATOR_OPENED, true);
    Act1RefrigeratorOutcome::OpenedWithMaria
}

/// Returns the refrigerator prop (`0x040b`) animation ID in `R_HSP_ALT_DAY_ROOM` (`0x53`, `sh2pc.exe` `0x0058ad36`):
/// `0x282f` once `flag(219)` is set, else `0x282e`.
#[must_use]
pub fn act1_hospital_refrigerator_anim_id(flags: &EventFlags) -> u16 {
    if flags.get(ACT1_FLAG_REFRIGERATOR_OPENED) {
        0x282f
    } else {
        0x282e
    }
}

/// Pushes the B1 Basement shelf (`EvProgMoveEmptyShelf` `0x00588220`), setting `flag(223)` to expose B1B.
pub fn act1_push_basement_shelf(chose_yes: bool, flags: &mut EventFlags) -> bool {
    if !chose_yes || flags.get(ACT1_FLAG_BASEMENT_SHELF_MOVED) {
        return false;
    }
    flags.set(ACT1_FLAG_BASEMENT_SHELF_MOVED, true);
    true
}

/// Places `Copper ring` (`56`), `Lead ring` (`57`), or combo `88` onto the 3F Basement Stairwell Angel Door
/// (`EvProgPictureOfAngel` `sh2pc.exe` `0x0058a090..0x0058a390`).
///
/// Returns `(placed, door_unlocked_now)`.
pub fn act1_place_ring_on_angel_door(
    item_id: u8,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> (bool, bool) {
    let was_unlocked =
        flags.get(ACT1_FLAG_ANGEL_COPPER_PLACED) && flags.get(ACT1_FLAG_ANGEL_LEAD_PLACED);
    let placed = match item_id {
        ACT1_ITEM_COPPER_RING
            if !flags.get(ACT1_FLAG_ANGEL_COPPER_PLACED) && inventory.has_item(item_id) =>
        {
            inventory.item_lose(item_id);
            flags.set(ACT1_FLAG_ANGEL_COPPER_PLACED, true);
            true
        }
        ACT1_ITEM_LEAD_RING
            if !flags.get(ACT1_FLAG_ANGEL_LEAD_PLACED) && inventory.has_item(item_id) =>
        {
            inventory.item_lose(item_id);
            flags.set(ACT1_FLAG_ANGEL_LEAD_PLACED, true);
            true
        }
        ACT1_COMBO_COPPER_AND_LEAD_RINGS if inventory.has_item(item_id) => {
            inventory.item_lose(item_id);
            flags.set(ACT1_FLAG_ANGEL_COPPER_PLACED, true);
            flags.set(ACT1_FLAG_ANGEL_LEAD_PLACED, true);
            true
        }
        _ => false,
    };
    let now_unlocked =
        flags.get(ACT1_FLAG_ANGEL_COPPER_PLACED) && flags.get(ACT1_FLAG_ANGEL_LEAD_PLACED);
    (placed, placed && !was_unlocked && now_unlocked)
}

/// Returns `true` when riding the Otherworld Elevator from B1 (`from_basement`) triggers the
/// Trick-or-Treat Radio Quiz (`sh2pc.exe` `0x00588ff0`: `from_basement && flag(15) && !flag(232)`).
#[must_use]
pub fn act1_should_trigger_elevator_radio_quiz(from_basement: bool, flags: &EventFlags) -> bool {
    from_basement
        && flags.get(ACT1_FLAG_MARIA_ACCOMPANYING)
        && !flags.get(ACT1_FLAG_ELEVATOR_QUIZ_HEARD)
}

/// Steps the Trick-or-Treat Elevator Radio Quiz timeline (`sh2pc.exe` `0x00589162..0x005893ba`).
///
/// `elapsed_30hz_ticks` advances at `dt_secs * 30.0` (`[0x6388a8] = 30.0`). Returns `(active_subtitle_msg_id, finished)`.
pub fn act1_step_elevator_radio_quiz(
    elapsed_30hz_ticks: f32,
    skip_requested: bool,
    flags: &mut EventFlags,
) -> (Option<u8>, bool) {
    if skip_requested || elapsed_30hz_ticks >= ACT1_ELEVATOR_QUIZ_TOTAL_TICKS {
        flags.set(ACT1_FLAG_ELEVATOR_QUIZ_HEARD, true);
        return (None, true);
    }
    let tick = elapsed_30hz_ticks as u16;
    let active_msg = ACT1_ELEVATOR_RADIO_SUBTITLES
        .iter()
        .position(|&(show, hide)| tick >= show && tick < hide)
        .map(|idx| (32 + idx) as u8);
    (active_msg, false)
}

/// Outcome of pressing/submitting the 3x3 push-button grid on the 3F Store Room Quiz Chest
/// (`EvProgSubAnswer` `sh2pc.exe` `0x00589796..0x00589834`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act1QuizChestOutcome {
    /// Fewer than all 3 questions have a button selected yet.
    Incomplete,
    /// Exact answers `(Q1 = 3, Q2 = 1, Q3 = 3)` selected: unlocks chest (`flag(233)` + `flag(240)`, SE `0x4db6`).
    SolvedCorrectly,
    /// All 3 questions answered with at least one wrong choice: triggers poison gas trap (`flag(233)` + `flag(241)`, SE `0x4db7`).
    WrongPoisonSpray,
}

/// Selects choice `choice` (`1..=3`) for `question` (`1..=3`) on the 3F Store Room Quiz Chest (`0x00589869..0x00589bec`),
/// immediately evaluating whether all 3 rows are selected (`0x00589796..0x00589815`).
pub fn act1_select_quiz_chest_button(
    question: u8,
    choice: u8,
    flags: &mut EventFlags,
) -> Option<Act1QuizChestOutcome> {
    if !(1..=3).contains(&question)
        || !(1..=3).contains(&choice)
        || flags.get(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED)
    {
        return None;
    }
    let base = ACT1_FLAG_QUIZ_BTN_BIT0 + ((question - 1) as usize) * 2;
    flags.set(base, (choice & 1) != 0);
    flags.set(base + 1, (choice & 2) != 0);

    let c1 = u8::from(flags.get(234)) | (u8::from(flags.get(235)) << 1);
    let c2 = u8::from(flags.get(236)) | (u8::from(flags.get(237)) << 1);
    let c3 = u8::from(flags.get(238)) | (u8::from(flags.get(239)) << 1);
    if c1 == 0 || c2 == 0 || c3 == 0 {
        return Some(Act1QuizChestOutcome::Incomplete);
    }

    flags.set(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, true);
    if (c1, c2, c3) == (3, 1, 3) {
        flags.set(ACT1_FLAG_QUIZ_CHEST_SOLVED, true);
        Some(Act1QuizChestOutcome::SolvedCorrectly)
    } else {
        flags.set(ACT1_FLAG_QUIZ_CHEST_WRONG, true);
        Some(Act1QuizChestOutcome::WrongPoisonSpray)
    }
}

/// Applies the Quiz Chest wrong-answer poison gas trap (`EvAllTimeFunc` `sh2pc.exe` `0x00589f40..0x0058a07d`):
/// across a `1.4s` (`[0x69f3b8]`) spray window, when `doku_timer` crosses `1.0s` (`[0x62edf4]`),
/// deals `(max_hp * 0.6)` (`[0x69f38c]`) damage clamped so James's HP never drops below `min_survival_hp` (`0x00545310`).
#[must_use]
pub fn act1_apply_quiz_poison_gas_damage(
    prev_timer_secs: f32,
    dt_secs: f32,
    current_hp: f32,
    max_hp: f32,
    min_survival_hp: f32,
) -> (f32, f32, bool) {
    let next_timer = prev_timer_secs + dt_secs;
    if prev_timer_secs >= 1.4 {
        return (next_timer, current_hp, false);
    }
    if prev_timer_secs <= 1.0 && next_timer > 1.0 {
        let raw_dmg = max_hp * 0.6;
        let target_hp = (current_hp - raw_dmg).max(min_survival_hp.min(current_hp));
        return (next_timer, target_hp, true);
    }
    (next_timer, current_hp, true)
}

/// Evaluates the 4 Pyramid Head chase speed zones in the Hospital Basement Passage (`stg_hospital_pass.c`
/// `sh2pc.exe` `0x00587c10..0x00587d30`: `z = -258800, -257600, -256000, -255200, -252400`, `x = 19200`).
#[must_use]
pub fn act1_eval_hospital_pass_ph_speed_tier(ph_x_mm: f32, ph_z_mm: f32) -> Option<u8> {
    if ph_z_mm > -252_400.0 {
        Some(3)
    } else if ph_z_mm > -256_000.0 || (ph_x_mm >= 19_200.0 && ph_z_mm > -255_200.0) {
        Some(2)
    } else if ph_x_mm > 19_200.0 && ph_z_mm > -257_600.0 {
        Some(1)
    } else if ph_z_mm > -258_800.0 {
        Some(0)
    } else {
        None
    }
}

/// Completes the Hospital Basement Passage escape cutscene (`sh2pc.exe` `0x00587e3a..0x00587e98`),
/// separating Maria (`flag(15) = false`) and setting `flag(522)` (`0x1f7aa20 |= 0x400`).
pub fn act1_complete_hospital_pass_escape(flags: &mut EventFlags) {
    flags.set(ACT1_FLAG_MARIA_ACCOMPANYING, false);
    flags.set(522, true);
}
