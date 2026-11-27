//! `connectStageInit` (`SLUS_202.28` `0x0020bd60`, `Stage_Data + 0x18`) and
//! `connectRoomInit` (`SLUS_202.28` `0x0020be10`, `Stage_Data + 0x1c`) hook dispatchers.
//!
//! All flag mutations, room IDs, and RNG thresholds are verified 1:1 from `sh2pc.exe`
//! (`Stage_Data::stage_init_fn` and `Stage_Data::room_init_fn` tables).

use crate::events::horror_triggers::StageEventDispatcher;
use crate::events::stages_act1::{act1_eval_stage_init, Act1StageId};
use crate::events::{EventFlags, Sh2Rng};
use crate::StageId;

/// Executes `Stage_Data::stage_init_fn` (`SLUS_202.28` `connectStageInit` `0x0020bd60`:
/// `if (*(code **)(_stage + 0x18) != 0) (**(code **)(_stage + 0x18))();`).
///
/// Verified against `sh2pc.exe` `stage_init_fn` addresses:
/// - Stage 5 (`TownEast`, `0x0059dfc0`): syncs cross-town map flags `1534->1497`, `1533->1496`, `1494->1526`.
/// - Stage 9 (`ApartE3fe`, `0x005992f0`): resets Room `0x13` camera lock & footstep timer.
/// - Stage 13 (`ApartOut`, `0x00595e80`): clears transient flag `87` (`0x1f7a9e8 &= 0xff7fffff`).
/// - Stage 14 (`TownWest`, `0x00593af0`): syncs cross-town map flags `1497->1534`, `1496->1533`, `1526->1494`
///   and re-joins Maria (`flag 15`) when Rosewater Park met (`flag 152`) outside Bowling/post-Hospital.
/// - Stage 15 (`Bowling`, `0x00591bd0`): clears `flag 15` (`0x1f7a9e0 &= 0xffff7fff`, Maria waits outside).
/// - Stage 40 (`Hotel2fF`, `0x0057a170`): marks Hotel 2F luggage/briefcase init flag `405` (`0x1f7aa10 |= 0x200000`).
/// - Stage 49 (`EndMaria`, `0x00440ee0`): sets `flag 45` (`0x1f7a9e4 |= 0x2000`) and `flag 168` (`0x1f7a9f4 |= 0x100`).
pub fn eval_stage_init_hook(
    stage_id: u16,
    flags: &mut EventFlags,
    horror_triggers: &mut StageEventDispatcher,
) {
    if let Some(act1) = Act1StageId::from_raw(stage_id as u8) {
        act1_eval_stage_init(act1, flags);
    }
    if let Some(sid) = StageId::from_raw(stage_id as u8) {
        horror_triggers.on_stage_init(sid, flags);
    }

    match stage_id {
        // `sh2pc.exe` `0x0059dfc0` (`stg_town_east` `stage_init_fn`)
        5 => {
            // `0x59dfc5`: `test $0x40000000, 0x1f7aa9c` (`flag 1534`) -> `orl $0x2000000, 0x1f7aa98` (`flag 1497`)
            if flags.get(1534) {
                flags.set(1497, true);
            }
            // `0x59dfd6`: `test $0x20000000, 0x1f7aa9c` (`flag 1533`) -> `orl $0x1000000, 0x1f7aa98` (`flag 1496`)
            if flags.get(1533) {
                flags.set(1496, true);
            }
            // `0x59dfe7`: `testl $0x400000, 0x1f7aa98` (`flag 1494`) -> `or $0x400000, 0x1f7aa9c` (`flag 1526`)
            if flags.get(1494) {
                flags.set(1526, true);
            }
        }
        // `sh2pc.exe` `0x00593af0` (`stg_town_west` `stage_init_fn`)
        14 => {
            // `0x593afc`: `testl $0x2000000, 0x1f7aa98` (`flag 1497`) -> `orl $0x40000000, 0x1f7aa9c` (`flag 1534`)
            if flags.get(1497) {
                flags.set(1534, true);
            }
            // `0x593b12`: `0x1f7aa9b & 1` (`flag 1496`) -> `orl $0x20000000, 0x1f7aa9c` (`flag 1533`)
            if flags.get(1496) {
                flags.set(1533, true);
            }
            // `0x593b28`: `test $0x400000, 0x1f7aa9c` (`flag 1526`) -> `or $0x400000, 0x1f7aa98` (`flag 1494`)
            if flags.get(1526) {
                flags.set(1494, true);
            }
        }
        // `sh2pc.exe` `0x0057a170` (`stg_hotel_2f_f` `stage_init_fn`)
        40 if !flags.get(405) => {
            // `0x57a176` / `0x57a231`: initialize once gated by `0x1f7aa10 & 0x200000` (`flag 405`).
            flags.set(405, true);
        }
        // `sh2pc.exe` `0x00440ee0` (`stage_end_maria` `stage_init_fn`)
        49 => {
            // `0x440ee0`: `orl $0x2000, 0x1f7a9e4` (`flag 45`)
            flags.set(45, true);
            // `0x440eef`: `or $0x100, 0x1f7a9f4` (`flag 168`)
            flags.set(168, true);
        }
        _ => {}
    }
}

/// Executes `Stage_Data::room_init_fn` (`SLUS_202.28` `connectRoomInit` `0x0020be10`:
/// `if (*(code **)(_stage + 0x1c) != 0) (**(code **)(_stage + 0x1c))();`).
///
/// Verified against `sh2pc.exe` `room_init_fn` addresses:
/// - Horror-trigger rooms (`ApartW2f` `0x005972d0`, `ApartStair` `0x00596480`, `Society` `0x00587a50`,
///   `PrisonN` `0x005859a0`, `PrisonS` `0x005845f0`) via [`HorrorTriggerEngine::on_room_init`].
/// - Stage 15 (`Bowling`, `0x00591c00`): sets `flag 530` (`0x1f7aa20 |= 0x40000`) outside Room `0x09`
///   and in Room `0x0b` when `!flag(159)` or `flag(251)`.
/// - Stage 16 (`ToHeaven`, `0x00591170`): if `flag(163)` (`0x1f7a9f4 & 0x08`), sets `flag(164)` (`0x10`).
/// - Stage 20 (`Hospital3fF`, `0x0058edc0`): in Room `0x41`, clears `flag 15` (Maria), sets `flag 188`
///   (`0x1f7a9f4 |= 0x10000000`), and if `flag(186)` sets `flag 629` (`0x1f7aa2c |= 0x200000`).
/// - Stage 34 (`LabyrinthW`, `0x00582a00`): in Room `0x89`, if `flag(327) && !flag(368)` sets `flag 328`.
/// - Stage 40 (`Hotel2fF`, `0x0057a240`): in Room `0x9f`, 1-in-4 chance (`rng & 3 == 0`) sets `flag 541`.
/// - Stage 41 (`Hotel3fF`, `0x00578730`): in Room `0xa2`, clears `flag 527` (`0x1f7aa20 &= 0xffff7fff`).
/// - Stage 43 (`HotelBfB`, `0x00577430`): in Room `0xb8`, sets map bits `0x02f80400` in word `0x1f7aaf4`.
/// - Stage 44 (`Hotel1fB`, `0x00576790`): in Room `0xae` sets `0xfdfc0000` in `0x1f7aaf8` & bit 0 in
///   `0x1f7aafc`; in Room `0xad` sets `0x1c0` in `0x1f7aaf8`.
/// - Stage 56 (`Wish2f`, `0x00571860`): if `flag(711)` (`0x1f7aa38 & 0x80`), clears `flag 710` (`&= ~0x40`).
pub fn eval_room_init_hook(
    stage_id: u16,
    room_id: u16,
    flags: &mut EventFlags,
    horror_triggers: &mut StageEventDispatcher,
    rng: &mut Sh2Rng,
) {
    if let Some(sid) = StageId::from_raw(stage_id as u8) {
        horror_triggers.on_room_init(sid, room_id as u8, flags, rng);
    }

    match stage_id {
        // `sh2pc.exe` `0x00591c00` (`stg_bowling` `room_init_fn`)
        15 => {
            if room_id == 0x0b {
                if !flags.get(159) || flags.get(251) {
                    flags.set(530, true);
                }
            } else if room_id != 0x09 {
                flags.set(530, true);
            }
        }
        // `sh2pc.exe` `0x00591170` (`stg_to_heaven` `room_init_fn`)
        16 if flags.get(163) => {
            flags.set(164, true);
        }
        // `sh2pc.exe` `0x0058edc0` (`stg_hospital_3f_f` `room_init_fn`)
        20 if room_id == 0x41 => {
            if flags.get(15) {
                flags.set(15, false);
            }
            flags.set(188, true);
            if flags.get(186) {
                flags.set(629, true);
            }
        }
        // `sh2pc.exe` `0x00582a00` (`stage_labyrinth_w` `room_init_fn`)
        34 if room_id == 0x89 && flags.get(327) && !flags.get(368) => {
            flags.set(328, true);
        }
        // `sh2pc.exe` `0x0057a240` (`stage_hotel_2f_f` `room_init_fn`)
        40 if room_id == 0x9f && (rng.next_i32() & 3) == 0 => {
            flags.set(541, true);
        }
        // `sh2pc.exe` `0x00578730` (`stage_hotel_3f_f` `room_init_fn`)
        41 if room_id == 0xa2 => {
            flags.set(527, false);
        }
        // `sh2pc.exe` `0x00577430` (`stage_hotel_bf_b` `room_init_fn`)
        43 if room_id == 0xb8 => {
            set_flag_mask(flags, 2208, 0x02f8_0400);
        }
        // `sh2pc.exe` `0x00576790` (`stage_hotel_1f_b` `room_init_fn`)
        44 => {
            if room_id == 0xae {
                set_flag_mask(flags, 2240, 0xfdfc_0000);
                flags.set(2272, true);
            } else if room_id == 0xad {
                set_flag_mask(flags, 2240, 0x0000_01c0);
            }
        }
        // `sh2pc.exe` `0x00571860` (`stage_wish_2f` `room_init_fn`)
        56 if flags.get(711) => {
            flags.set(710, false);
        }
        _ => {}
    }
}

/// Sets every bit present in `mask` starting at `base_flag` (32-bit aligned `0x01f7a9e0` word).
fn set_flag_mask(flags: &mut EventFlags, base_flag: usize, mask: u32) {
    for bit in 0..32 {
        if ((mask >> bit) & 1) != 0 {
            flags.set(base_flag + bit, true);
        }
    }
}
