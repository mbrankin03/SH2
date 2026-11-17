//! Stage & room lifecycle hooks (`EvStageInit`, `EvRoomInit`, `EvAllTimeFunc`, `EvBgmControl`,
//! and `EvSoundCallAfterLoad`) for Act 1 Stages 1–27.
//!
//! Disassembled 1:1 from `sh2pc.exe` (`0x00587ee0..0x005a1b50`) and `SLUS_202.28`
//! (`stg_toilet.c`..`stg_hospital_pass.c`), plus `ShowerRoomFlashlightFix.cpp`.

use std::f32::consts::TAU;

use super::{Act1StageId, Act1VanillaFixesConfig};
use crate::events::{EventFlags, PuzzleState, PZ_CLOCK};

/// Room ID for Wood Side Courtyard (`R_APT_E_COURTYARD` in `sh2pc.exe` `0x00595ea0`).
pub const ACT1_ROOM_APT_E_COURTYARD: u8 = 0x07;
/// Room ID for Pete's Bowl-O-Rama main alley (`cmp eax, 0x0b` at `sh2pc.exe` `0x00591c08`).
pub const ACT1_ROOM_BOWLING_MAIN: u8 = 0x0b;
/// Room ID for Wood Side SE Staircase (`cmp eax, 0x10` at `sh2pc.exe` `0x005964a0`).
pub const ACT1_ROOM_APT_E_STAIR_SW: u8 = 0x10;
/// Room ID for Wood Side Rm 307 (`cmp eax, 0x13` at `sh2pc.exe` `0x00599330`).
pub const ACT1_ROOM_APT_E_307: u8 = 0x13;
/// Room ID for Wood Side Rm 202 butterfly room (`cmp esi, 0x16` at `sh2pc.exe` `0x0059ba61`).
pub const ACT1_ROOM_APT_E_202: u8 = 0x16;
/// Room ID for Wood Side Rm 205 mannequin flashlight room (`cmp eax, 0x17` at `sh2pc.exe` `0x0059bae1`).
pub const ACT1_ROOM_APT_E_205: u8 = 0x17;
/// Room ID for Wood Side Rm 208 grandfather clock room (`cmp esi, 0x18` at `sh2pc.exe` `0x0059b9d8`).
pub const ACT1_ROOM_APT_E_208: u8 = 0x18;
/// Room ID for Wood Side 3F Hallway (`cmp eax, 0x1f` at `sh2pc.exe` `0x00599339`).
pub const ACT1_ROOM_APT_E_HALL_3F: u8 = 0x1f;
/// Room ID for Blue Creek S Staircase (`sub eax, 0x10` -> `0x20` at `sh2pc.exe` `0x005964a9`).
pub const ACT1_ROOM_APT_W_STAIR_S: u8 = 0x20;
/// Room ID for Blue Creek N Staircase (`0x21` Pyramid Head boss drain at `sh2pc.exe` `0x005964b2`).
pub const ACT1_ROOM_APT_W_STAIR_N: u8 = 0x21;
/// Room ID for Blue Creek Rm 105 3-Coin desk room (`R_APT_W_RM_105 = 0x22`, `sh2pc.exe` `0x005985b0`).
pub const ACT1_ROOM_APT_W_105: u8 = 0x22;
/// Room ID for Blue Creek Rm 109 Angela knife room (`0x24`, `sh2pc.exe` `0x0059853f`).
pub const ACT1_ROOM_APT_W_109_INNER: u8 = 0x24;
/// Room ID for Blue Creek Rm 203 Safe & Toilet room (`R_APT_W_RM_203 = 0x27`, `sh2pc.exe` `0x00597313`).
pub const ACT1_ROOM_APT_W_203: u8 = 0x27;
/// Room ID for Blue Creek Rm 208/209 Whispering room (`0x28`, `sh2pc.exe` `0x005972d5`).
pub const ACT1_ROOM_APT_W_208_209: u8 = 0x28;
/// Room ID for Brookhaven Normal Elevator (`R_HSP_ELEVATOR = 0x2a`, `sh2pc.exe` `0x0058d110`).
pub const ACT1_ROOM_HSP_ELEVATOR: u8 = 0x2a;
/// Room ID for Brookhaven Roof (`R_HSP_ROOF = 0x2b`, `sh2pc.exe` `0x0058d0a5`).
pub const ACT1_ROOM_HSP_ROOF: u8 = 0x2b;
/// Room ID for Brookhaven 1F Rm C2 Laura room (`R_HSP_RM_C2 = 0x31`, `sh2pc.exe` `0x00590ba6`).
pub const ACT1_ROOM_HSP_C2: u8 = 0x31;
/// Room ID for Brookhaven 1F Lobby (`R_HSP_LOBBY = 0x33`, `sh2pc.exe` `0x00590b48`).
pub const ACT1_ROOM_HSP_LOBBY: u8 = 0x33;
/// Room ID for Brookhaven 2F Women's Locker Room (`R_HSP_LOCKER_WOMENS = 0x35`, `sh2pc.exe` `0x005901e5`).
pub const ACT1_ROOM_HSP_LOCKER_WOMENS: u8 = 0x35;
/// Room ID for Brookhaven 3F Shower Room (`R_HSP_SHOWER = 0x40`, `ShowerRoomFlashlightFix.cpp`).
pub const ACT1_ROOM_HSP_SHOWER: u8 = 0x40;
/// Room ID for Brookhaven 3F Rm S3 Maria resting room (`R_HSP_RM_S3 = 0x41`, `sh2pc.exe` `0x0058edc8`).
pub const ACT1_ROOM_HSP_S3: u8 = 0x41;
/// Room ID for Brookhaven 3F Rm S14 Louise Box room (`R_HSP_RM_S14 = 0x43`).
pub const ACT1_ROOM_HSP_S14: u8 = 0x43;
/// Room ID for Brookhaven Otherworld Elevator (`R_HSP_ALT_ELEVATOR = 0x46`).
pub const ACT1_ROOM_HSP_ALT_ELEVATOR: u8 = 0x46;
/// Room ID for Brookhaven Otherworld 2F Day Room (`R_HSP_ALT_DAY_ROOM = 0x53`, `sh2pc.exe` `0x0058ad25`).
pub const ACT1_ROOM_HSP_ALT_DAY_ROOM: u8 = 0x53;
/// Room ID for Brookhaven Otherworld 3F Store Room (`R_HSP_ALT_STORE_ROOM = 0x54`, `sh2pc.exe` `0x00589de0`).
pub const ACT1_ROOM_HSP_ALT_STORE_ROOM: u8 = 0x54;
/// Room ID for Brookhaven Otherworld 3F Rm S3 (`R_HSP_ALT_RM_S3 = 0x55`, `sh2pc.exe` `0x00589de9`).
pub const ACT1_ROOM_HSP_ALT_S3: u8 = 0x55;
/// Room ID for Brookhaven Otherworld Basement (`R_HSP_ALT_BASEMENT = 0x59`).
pub const ACT1_ROOM_HSP_ALT_BASEMENT: u8 = 0x59;

/// Town outdoor streaming grid scale (`5.0e-5` = `1 / 20_000` mm, `sh2pc.exe` `0x006ac094`).
pub const ACT1_TOWN_GRID_INV_CELL_MM: f32 = 5e-5;
/// Town outdoor streaming grid origin bias (`16.0`, `sh2pc.exe` `0x006315f8`).
pub const ACT1_TOWN_GRID_BIAS: f32 = 16.0;
/// Town East linear block table base subtraction (`0x171`, `sh2pc.exe` `0x0059ed94`).
pub const ACT1_TOWN_EAST_BLOCK_OFFSET: i32 = 0x171;
/// Town West linear block table base subtraction (`0x1b4`, `sh2pc.exe` `0x00594ccc`).
pub const ACT1_TOWN_WEST_BLOCK_OFFSET: i32 = 0x1b4;

/// Flag `15` (`0x1f7a9e0 & 0x8000`): Maria is currently accompanying James (`sh2pc.exe` `0x00593b5e`).
pub const ACT1_FLAG_MARIA_ACCOMPANYING: usize = 15;

/// Deterministic snapshot produced by evaluating `EvAllTimeFunc`, `EvRoomInit`, and `EvBgmControl`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Act1LifecycleSnapshot {
    /// Active BGM track/channel bitmask returned by `EvBgmControl` (`0` = silence/default).
    pub bgm_control_mask: u32,
    /// Outdoor 32x32 streaming block index for `stg_town_east` (`0..=0xe1`) or `stg_town_west` (`0..=0xa2`).
    pub town_streaming_block: Option<u16>,
    /// Grandfather clock `(hour_hand_rad, minute_hand_rad)` in Wood Side Rm 208 (`stg_apart_e2f`).
    pub clock_hand_angles_rad: Option<[f32; 2]>,
    /// Dynamic door state overrides `(door_id, state)` emitted by `DoorStateChange` (`0x004a2f10`).
    pub door_overrides: Vec<(u8, i8)>,
    /// Sound effect IDs triggered on room load (`EvSoundCallAfterLoad`) or one-shot proximity.
    pub triggered_sound_effects: Vec<u32>,
    /// Whether Pyramid Head staircase water has drained (`stg_apart_stair` flag `149` -> `150`).
    pub stair_water_drained: bool,
    /// Whether the flashlight beam/glow renders in the current room (`ShowerRoomFlashlightFix`).
    pub render_flashlight_glow: bool,
}

/// Computes the 32x32 outdoor streaming block index from PS2 world `(x_mm, z_mm)` coordinates.
///
/// Matches `EvAllTimeFunc` in `stg_town_east.c` (`sh2pc.exe` `0x0059ed58..0x0059ed94`) and
/// `stg_town_west.c` (`0x00594c58..0x00594ccc`).
#[must_use]
pub fn act1_town_grid_block(stage: Act1StageId, x_mm: f32, z_mm: f32) -> Option<u16> {
    let gx = (x_mm * ACT1_TOWN_GRID_INV_CELL_MM + ACT1_TOWN_GRID_BIAS) as i32;
    let gz = (z_mm * ACT1_TOWN_GRID_INV_CELL_MM + ACT1_TOWN_GRID_BIAS) as i32;
    match stage {
        Act1StageId::TownEast => {
            let idx = (gx << 5) + gz - ACT1_TOWN_EAST_BLOCK_OFFSET;
            (0..=0xe1).contains(&idx).then_some(idx as u16)
        }
        Act1StageId::TownWest => {
            let idx = (gx << 5) + gz - ACT1_TOWN_WEST_BLOCK_OFFSET;
            (0..=0xa2).contains(&idx).then_some(idx as u16)
        }
        _ => None,
    }
}

/// Computes the Rm 208 grandfather clock hour & minute hand angles (in radians) from `PZ_CLOCK` (`0..43200`).
///
/// Ported from `stg_apart_e2f.c` `EvAllTimeFunc` (`sh2pc.exe` `0x0059bd45..0x0059be26`):
/// minute period is `0x0f00` (`3840` ticks = 60 minutes * 64 ticks/min) and hour period is `46080` (`12 * 3840`).
#[must_use]
pub fn act1_clock_hand_angles_rad(puzzle: &PuzzleState) -> [f32; 2] {
    let raw = puzzle.read_u16(PZ_CLOCK) as u32;
    let min_frac = ((raw % 0x0f00) as f32) / 3840.0;
    let hour_frac = ((raw % (12 * 0x0f00)) as f32) / 46_080.0;
    [hour_frac * TAU, min_frac * TAU]
}

/// Evaluates flashlight glow visibility, preserving both 1:1 vanilla PS2 behavior and SH2:EE's
/// `ShowerRoomFlashlightFix` (`Patches/ShowerRoomFlashlightFix.cpp`, `R_HSP_SHOWER = 0x40`).
///
/// On PC vanilla, `event_flashlight_on` turns `false` during the Shower Room drain crouch/freeze
/// frame, causing a 1-frame light flicker. When `fixes.shower_room_flashlight_fix` is `true` and
/// `room_id == 0x40`, the player entity's status flag (`player_status_flashlight_on`) is used instead.
#[must_use]
pub const fn act1_eval_shower_flashlight_glow(
    room_id: u8,
    event_flashlight_on: bool,
    player_status_flashlight_on: bool,
    fixes: Act1VanillaFixesConfig,
) -> bool {
    if fixes.shower_room_flashlight_fix && room_id == ACT1_ROOM_HSP_SHOWER {
        player_status_flashlight_on
    } else {
        event_flashlight_on
    }
}

/// Applies `EvStageInit` side effects when loading an Act 1 stage (`sh2pc.exe` `Stage_Data::stage_init`).
///
/// Ported from `stg_apart_e3fe` (`0x005992f0`), `stg_apart_out` (`0x00595e80`), `stg_town_west`
/// (`0x00593af0`), `stg_bowling` (`0x00591bd0`), and `stg_hospital_3f_b` (`0x00589db0`).
pub fn act1_eval_stage_init(stage: Act1StageId, flags: &mut EventFlags) {
    match stage {
        Act1StageId::ApartOut => {
            // `0x00595e80`: `0x1f7a9e8 &= 0xff7fffff` -> clears transient flag `87` (`64 + 23`).
            flags.set(87, false);
        }
        Act1StageId::Bowling => {
            // `0x00591bd0`: `0x1f7a9e0 &= 0xffff7fff` -> Maria waits outside Pete's Bowl-O-Rama (`!flag(15)`).
            flags.set(ACT1_FLAG_MARIA_ACCOMPANYING, false);
        }
        Act1StageId::TownWest => {
            // `0x00593b3d..0x00593b65`: Re-joins Maria (`flag(15) = true`) when Rosewater Park met (`flag(152)`)
            // unless inside Bowling/Heaven's Night exclusion or post-Hospital (`flag(251)`).
            if flags.get(152) && (!flags.get(162) || flags.get(163)) && !flags.get(251) {
                flags.set(ACT1_FLAG_MARIA_ACCOMPANYING, true);
            }
        }
        _ => {}
    }
}

/// Evaluates `EvBgmControl` (`Stage_Data::bgm_control`) returning the active BGM channel bitmask.
///
/// Ported 1:1 from `stg_toilet` (`0x005a1b30`), `stg_ovservation` (`0x005a1390`), `stg_apart_e2f`
/// (`0x0059bea0`), `stg_apart_e3fw` (`0x00599a40`), `stg_apart_stair` (`0x00596640`), `stg_town_west`
/// (`0x00593db0`), `stg_bowling` (`0x00591d30`), `stg_hospital_1f_f` (`0x00590cd0`),
/// `stg_hospital_1fw_b` (`0x0058c2b0`), and `stg_hospital_pass` (`0x00587f50`).
#[must_use]
pub fn act1_eval_bgm_control(stage: Act1StageId, flags: &EventFlags, active_bgm_id: u32) -> u32 {
    match stage {
        Act1StageId::Toilet => {
            // `0x005a1b30`: `0x1f7a9e4` bit 4 (`flag 36`) -> `1`, else `!bit 1` (`!flag 33`) -> `4`.
            if flags.get(36) {
                1
            } else if !flags.get(33) {
                4
            } else {
                0
            }
        }
        Act1StageId::Observation => {
            // `0x005a1390`: `flag(517)` -> `0x0e`, else `flag(36)` -> `1`.
            if flags.get(517) {
                0x0e
            } else {
                u32::from(flags.get(36))
            }
        }
        Act1StageId::ApartE2f | Act1StageId::ApartE3fw => {
            // `0x0059bea0` / `0x00599a40`: `flag(540)` (`0x1f7aa20 & 0x1000_0000`) -> `0x0e`.
            if flags.get(540) {
                0x0e
            } else {
                0
            }
        }
        Act1StageId::ApartStair => {
            // `0x00596640`: `flag(529)` && `!flag(526)` -> `1` (Pyramid Head boss siren/theme).
            u32::from(flags.get(529) && !flags.get(526))
        }
        Act1StageId::TownWest => {
            // `0x00593db0`: `flag(525)` -> `1`, else `flag(519)` -> `0x0e`.
            if flags.get(525) {
                1
            } else if flags.get(519) {
                0x0e
            } else {
                0
            }
        }
        Act1StageId::Bowling => {
            // `0x00591d30`: `flag(530)` (`0x1f7aa20 & 0x40000`) -> `0`, else `0x0e`.
            if flags.get(530) {
                0
            } else {
                0x0e
            }
        }
        Act1StageId::Hospital1fNormal => {
            // `0x00590cd0`: when `active_bgm_id == 0x27` (`39`), returns `4`.
            if active_bgm_id == 0x27 {
                4
            } else {
                0
            }
        }
        Act1StageId::Hospital1fwOtherworld => {
            // `0x0058c2b0`: `0x28` -> `4`, `0x29` -> `0x0e`.
            match active_bgm_id {
                0x28 => 4,
                0x29 => 0x0e,
                _ => 0,
            }
        }
        Act1StageId::HospitalPass => {
            // `0x00587f50`: `(0x1f7aa20 >> 8) & 4` -> `flag(522)` (`0x1f7aa20 & 0x400`) -> `4`.
            if flags.get(522) {
                4
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Evaluates `EvRoomInit`, `EvSoundCallAfterLoad`, and `EvAllTimeFunc` for an Act 1 stage & room.
#[allow(clippy::too_many_arguments)]
pub fn act1_eval_alltime_and_room(
    stage: Act1StageId,
    room_id: u8,
    player_pos_mm: [f32; 3],
    active_bgm_id: u32,
    event_flashlight_on: bool,
    player_status_flashlight_on: bool,
    fixes: Act1VanillaFixesConfig,
    flags: &mut EventFlags,
    puzzle: &PuzzleState,
) -> Act1LifecycleSnapshot {
    let mut snap = Act1LifecycleSnapshot {
        bgm_control_mask: act1_eval_bgm_control(stage, flags, active_bgm_id),
        town_streaming_block: act1_town_grid_block(stage, player_pos_mm[0], player_pos_mm[2]),
        render_flashlight_glow: act1_eval_shower_flashlight_glow(
            room_id,
            event_flashlight_on,
            player_status_flashlight_on,
            fixes,
        ),
        ..Act1LifecycleSnapshot::default()
    };

    match stage {
        Act1StageId::Observation => {
            // `0x005a1350`: door `0x03` state `-1` if `flag(24)` (`0x1f7a9e3 & 1`), else `0`.
            snap.door_overrides
                .push((0x03, if flags.get(24) { -1 } else { 0 }));
        }
        Act1StageId::ApartE2f => {
            if room_id == ACT1_ROOM_APT_E_208 {
                snap.clock_hand_angles_rad = Some(act1_clock_hand_angles_rad(puzzle));
                // `0x0059ba80` (`EvSoundCallAfterLoad`): ticking clock SE `0x3e80` (`16000`) while `!flag(69)`.
                if flags.get(69) {
                    flags.set(70, true);
                    snap.triggered_sound_effects.push(0x3e80);
                }
            }
        }
        Act1StageId::ApartW1f => {
            if room_id == ACT1_ROOM_APT_W_105 {
                // `0x00598645`: Rm 105 door `0x4e` locked (`1`) until 3-Coin puzzle solved (`flag(142)`).
                snap.door_overrides.push((0x4e, i8::from(!flags.get(142))));
            }
        }
        Act1StageId::ApartW2f => {
            if room_id == ACT1_ROOM_APT_W_203 {
                // `0x00597318..0x00597360`: Safe door `0x43` unlocked (`0`) when `flag(108)`, else `1`.
                snap.door_overrides.push((0x43, i8::from(!flags.get(108))));
            }
        }
        Act1StageId::ApartStair => {
            // `0x00596480`: clears transient flag `87`.
            flags.set(87, false);
            if room_id == ACT1_ROOM_APT_W_STAIR_N {
                // `0x005964b9..0x005964f8`: once PH retreats (`flag(148)`), triggers water drain SE `0x2716` (`10006`) & `flag(149)`.
                if flags.get(148) && !flags.get(149) {
                    flags.set(149, true);
                    snap.triggered_sound_effects.push(0x2716);
                }
                snap.stair_water_drained = flags.get(149);
            } else if room_id == ACT1_ROOM_APT_W_STAIR_S {
                snap.door_overrides
                    .push((0x6e, if flags.get(26) { -1 } else { 0 }));
            } else if room_id == ACT1_ROOM_APT_E_STAIR_SW {
                snap.door_overrides
                    .push((0x19, if flags.get(25) { -1 } else { 0 }));
            }
        }
        Act1StageId::ApartOut => {
            if room_id != ACT1_ROOM_APT_E_COURTYARD {
                if flags.get(95) {
                    snap.door_overrides.push((0x2e, 2));
                }
                snap.door_overrides.push((0x2e, 1));
            }
        }
        Act1StageId::TownWest => {
            // `0x00593ac0`: ambient foghorn/wind SE `0x4269` (`17001`) while `!flag(168)`.
            if !flags.get(168) {
                snap.triggered_sound_effects.push(0x4269);
            }
        }
        Act1StageId::Bowling => {
            // `0x00591d50`: jukebox music stream `0xeae8` (`60136`) in room `0x0a` setting `flag(161)`.
            if room_id == 0x0a && !flags.get(161) {
                flags.set(161, true);
                snap.triggered_sound_effects.push(0xeae8);
            }
        }
        Act1StageId::ToHeaven => {
            // `0x00591170`: propagates `flag(163)` -> `flag(164)`.
            if flags.get(163) {
                flags.set(164, true);
            }
        }
        Act1StageId::Hospital1fNormal => {
            if room_id == ACT1_ROOM_HSP_LOBBY {
                // `0x00590c35`: front hospital doors `0x04` locked (`-1` if `flag(27)` else `0`).
                snap.door_overrides
                    .push((0x04, if flags.get(27) { -1 } else { 0 }));
            }
        }
        Act1StageId::Hospital3fNormal => {
            if room_id == ACT1_ROOM_HSP_S3 {
                // `0x0058edd1..0x0058eea0`: Maria rests in S3 (`flag(15) = false`, sets `flag(188)`).
                if flags.get(ACT1_FLAG_MARIA_ACCOMPANYING) {
                    flags.set(ACT1_FLAG_MARIA_ACCOMPANYING, false);
                }
                flags.set(188, true);
                if flags.get(186) {
                    flags.set(629, true);
                }
            }
        }
        Act1StageId::HospitalRoofNormal => {
            if room_id == ACT1_ROOM_HSP_ELEVATOR {
                // `0x0058d119..0x0058d1a0`: syncs elevator doors `0x19` for floors 1F/2F/3F (`flags 21, 22, 23`).
                snap.door_overrides
                    .push((0x19, if flags.get(21) { 2 } else { 3 }));
                snap.door_overrides
                    .push((0x19, if flags.get(22) { 1 } else { 4 }));
                snap.door_overrides
                    .push((0x19, if flags.get(23) { 0 } else { 5 }));
            }
        }
        Act1StageId::Hospital3fOtherworld if room_id == ACT1_ROOM_HSP_ALT_STORE_ROOM => {
            // `0x00589ea6..0x00589f33`: Quiz chest (`0xd3`) hidden (`-1`) before radio show (`!flag(232)`),
            // open (`0`) if solved (`flag(240)`), closed (`1`) otherwise.
            let state = if !flags.get(232) {
                -1
            } else if flags.get(240) {
                0
            } else {
                1
            };
            snap.door_overrides.push((0xd3, state));
        }
        _ => {}
    }

    snap
}
