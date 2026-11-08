use bytemuck::{Pod, Zeroable};

use super::collision::{ClHitpolyColumn, ClVhitResult};
use super::matrix::Sh2Ptr32;
use super::pad::PadInfo;

/// James Upper-Body Animation / Action Status (`_JAMES_UPPER_STATUS`, DIE `@0xeb2e`)
/// Stored as a 1-byte value (`u8`) in `PlayerState.upper_st` and `PlayerState.up_st_old`.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum JamesUpperStatus {
    #[default]
    Stand = 0, // JMS_ST_U_STAND
    Relax = 1,    // JMS_ST_U_RELAX
    Alert = 2,    // JMS_ST_U_ALERT
    Tired = 3,    // JMS_ST_U_TIRED
    Ready = 4,    // JMS_ST_U_READY
    ReadyOff = 5, // JMS_ST_U_READYOFF
    LRound = 6,   // JMS_ST_U_LROUND
    RRound = 7,   // JMS_ST_U_RROUND
    Back = 8,     // JMS_ST_U_BACK
    Walk = 9,     // JMS_ST_U_WALK
    LSWalk = 10,  // JMS_ST_U_LSWALK
    RSWalk = 11,  // JMS_ST_U_RSWALK
    Run1 = 12,    // JMS_ST_U_RUN1
    Run2 = 13,    // JMS_ST_U_RUN2
    Run3 = 14,    // JMS_ST_U_RUN3
    LSRun = 15,   // JMS_ST_U_LSRUN
    RSRun = 16,   // JMS_ST_U_RSRUN
    WallF = 17,   // JMS_ST_U_WALL_F
    LTurn = 18,   // JMS_ST_U_LTURN
    RTurn = 19,   // JMS_ST_U_RTURN
    Jump = 20,    // JMS_ST_U_JUMP
    Guard = 21,   // JMS_ST_U_GUARD
    LStep = 22,   // JMS_ST_U_LSTEP
    RStep = 23,   // JMS_ST_U_RSTEP
    Fall = 24,    // JMS_ST_U_FALL
    Damage = 25,  // JMS_ST_U_DAMAGE
    Hold = 26,    // JMS_ST_U_HOLD
    Release = 27, // JMS_ST_U_RELEASE
    Attack = 28,  // JMS_ST_U_ATTACK
    Kick = 29,    // JMS_ST_U_KICK
    ToStand = 30, // JMS_ST_U_TO_STAND
    Event = 31,   // JMS_ST_U_EVENT
}

impl JamesUpperStatus {
    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Stand),
            1 => Some(Self::Relax),
            2 => Some(Self::Alert),
            3 => Some(Self::Tired),
            4 => Some(Self::Ready),
            5 => Some(Self::ReadyOff),
            6 => Some(Self::LRound),
            7 => Some(Self::RRound),
            8 => Some(Self::Back),
            9 => Some(Self::Walk),
            10 => Some(Self::LSWalk),
            11 => Some(Self::RSWalk),
            12 => Some(Self::Run1),
            13 => Some(Self::Run2),
            14 => Some(Self::Run3),
            15 => Some(Self::LSRun),
            16 => Some(Self::RSRun),
            17 => Some(Self::WallF),
            18 => Some(Self::LTurn),
            19 => Some(Self::RTurn),
            20 => Some(Self::Jump),
            21 => Some(Self::Guard),
            22 => Some(Self::LStep),
            23 => Some(Self::RStep),
            24 => Some(Self::Fall),
            25 => Some(Self::Damage),
            26 => Some(Self::Hold),
            27 => Some(Self::Release),
            28 => Some(Self::Attack),
            29 => Some(Self::Kick),
            30 => Some(Self::ToStand),
            31 => Some(Self::Event),
            _ => None,
        }
    }
}

#[allow(non_camel_case_types)]
pub type _JAMES_UPPER_STATUS = JamesUpperStatus;

/// James Lower-Body Locomotion Status (`_JAMES_LOWER_STATUS`, DIE `@0xdeb8`)
/// Stored as a 1-byte value (`u8`) in `PlayerState.lower_st` and `PlayerState.low_st_old`.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum JamesLowerStatus {
    #[default]
    Stand = 0, // JMS_ST_L_STAND
    Relax = 1,    // JMS_ST_L_RELAX
    Alert = 2,    // JMS_ST_L_ALERT
    Tired = 3,    // JMS_ST_L_TIRED
    Ready = 4,    // JMS_ST_L_READY
    ReadyOff = 5, // JMS_ST_L_READYOFF
    LRound = 6,   // JMS_ST_L_LROUND
    RRound = 7,   // JMS_ST_L_RROUND
    Back = 8,     // JMS_ST_L_BACK
    Walk = 9,     // JMS_ST_L_WALK
    LSWalk = 10,  // JMS_ST_L_LSWALK
    RSWalk = 11,  // JMS_ST_L_RSWALK
    Run1 = 12,    // JMS_ST_L_RUN1
    Run2 = 13,    // JMS_ST_L_RUN2
    Run3 = 14,    // JMS_ST_L_RUN3
    LSRun = 15,   // JMS_ST_L_LSRUN
    RSRun = 16,   // JMS_ST_L_RSRUN
    WallF = 17,   // JMS_ST_L_WALL_F
    LTurn = 18,   // JMS_ST_L_LTURN
    RTurn = 19,   // JMS_ST_L_RTURN
    Jump = 20,    // JMS_ST_L_JUMP
    Guard = 21,   // JMS_ST_L_GUARD
    LStep = 22,   // JMS_ST_L_LSTEP
    RStep = 23,   // JMS_ST_L_RSTEP
    Fall = 24,    // JMS_ST_L_FALL
    Damage = 25,  // JMS_ST_L_DAMAGE
    Hold = 26,    // JMS_ST_L_HOLD
    Release = 27, // JMS_ST_L_RELEASE
    Attack = 28,  // JMS_ST_L_ATTACK
    Kick = 29,    // JMS_ST_L_KICK
    ToStand = 30, // JMS_ST_L_TO_STAND
    Event = 31,   // JMS_ST_L_EVENT
}

impl JamesLowerStatus {
    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Stand),
            1 => Some(Self::Relax),
            2 => Some(Self::Alert),
            3 => Some(Self::Tired),
            4 => Some(Self::Ready),
            5 => Some(Self::ReadyOff),
            6 => Some(Self::LRound),
            7 => Some(Self::RRound),
            8 => Some(Self::Back),
            9 => Some(Self::Walk),
            10 => Some(Self::LSWalk),
            11 => Some(Self::RSWalk),
            12 => Some(Self::Run1),
            13 => Some(Self::Run2),
            14 => Some(Self::Run3),
            15 => Some(Self::LSRun),
            16 => Some(Self::RSRun),
            17 => Some(Self::WallF),
            18 => Some(Self::LTurn),
            19 => Some(Self::RTurn),
            20 => Some(Self::Jump),
            21 => Some(Self::Guard),
            22 => Some(Self::LStep),
            23 => Some(Self::RStep),
            24 => Some(Self::Fall),
            25 => Some(Self::Damage),
            26 => Some(Self::Hold),
            27 => Some(Self::Release),
            28 => Some(Self::Attack),
            29 => Some(Self::Kick),
            30 => Some(Self::ToStand),
            31 => Some(Self::Event),
            _ => None,
        }
    }
}

#[allow(non_camel_case_types)]
pub type _JAMES_LOWER_STATUS = JamesLowerStatus;

/// Global Player Workspace (`shPlayerWork`, 1344 bytes / `0x540`, DIE `@0x10b49`)
/// Maps to `sh2gfw_Player_Work` in `SLUS_202.28`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct PlayerState {
    // =========================================================================
    // 1. Kinematics & Wall Distances (+0x0000 .. +0x0064)
    // =========================================================================
    pub player: Sh2Ptr32, // +0x0000 (0)   `SubCharacter*`
    pub dist_rot: f32,    // +0x0004 (4)
    pub dist_pos: f32,    // +0x0008 (8)
    pub dir: f32,         // +0x000C (12)
    pub to_wall: f32,     // +0x0010 (16)
    pub to_wall_r: f32,   // +0x0014 (20)
    pub to_wall_l: f32,   // +0x0018 (24)
    pub to_wall_b: f32,   // +0x001C (28)
    pub to_wall_f: f32,   // +0x0020 (32)
    pub to_chara: f32,    // +0x0024 (36)
    pub r_wall: f32,      // +0x0028 (40)
    pub l_wall: f32,      // +0x002C (44)
    pub pos: [f32; 4],    // +0x0030 (48)
    pub rot: [f32; 4],    // +0x0040 (64)
    pub spd: [f32; 4],    // +0x0050 (80)
    pub rot_y: f32,       // +0x0060 (96)

    // =========================================================================
    // 2. Action & Animation State Machine (+0x0064 .. +0x0098)
    // =========================================================================
    pub anime: i32,         // +0x0064 (100)
    pub upper_st: u8,       // +0x0068 (104) `_JAMES_UPPER_STATUS`
    pub lower_st: u8,       // +0x0069 (105) `_JAMES_LOWER_STATUS`
    pub act_st: u8,         // +0x006A (106)
    pub sub_st: u8,         // +0x006B (107)
    pub up_st_old: u8,      // +0x006C (108)
    pub low_st_old: u8,     // +0x006D (109)
    pub act_st_old: u8,     // +0x006E (110)
    pub sub_st_old: u8,     // +0x006F (111)
    pub weapon: u8,         // +0x0070 (112)
    pub weapon_old: u8,     // +0x0071 (113)
    pub hold_st: u8,        // +0x0072 (114)
    pub move_st: u8,        // +0x0073 (115)
    pub step: i32,          // +0x0074 (116)
    pub sub_step: i32,      // +0x0078 (120)
    pub timer: f32,         // +0x007C (124)
    pub timer2: f32,        // +0x0080 (128)
    pub tiredness: f32,     // +0x0084 (132)
    pub target: Sh2Ptr32,   // +0x0088 (136) `SubCharacter*`
    pub look_tgt: Sh2Ptr32, // +0x008C (140) `SubCharacter*`
    pub flag: u16,          // +0x0090 (144)
    pub _pad_92: [u8; 2],   // +0x0092 (146)
    pub light_dir_x: f32,   // +0x0094 (148)

    // =========================================================================
    // 3. Flashlight & Neck Vectors (+0x0098 .. +0x0130)
    // =========================================================================
    pub light_dir_y: f32,     // +0x0098 (152)
    pub neck_angle_y: f32,    // +0x009C (156)
    pub neck_angle_x: f32,    // +0x00A0 (160)
    pub neck_angle_z: f32,    // +0x00A4 (164)
    pub light_angle_y: f32,   // +0x00A8 (168)
    pub light_angle_x: f32,   // +0x00AC (172)
    pub neck_point: [f32; 3], // +0x00B0 (176)
    pub _pad_bc: [u8; 4],     // +0x00BC (188)
    pub light_pos: [f32; 4],  // +0x00C0 (192)
    pub light_dir: [f32; 4],  // +0x00D0 (208)
    pub hand_pos: [f32; 4],   // +0x00E0 (224)
    pub focus_pos: [f32; 4],  // +0x00F0 (240)
    pub eye_pos: [f32; 4],    // +0x0100 (256)
    pub neck_dir: [f32; 4],   // +0x0110 (272)
    pub look_pos: [f32; 4],   // +0x0120 (288)

    // =========================================================================
    // 4. Room & Collision Sensors (+0x0130 .. +0x032C)
    // =========================================================================
    pub room: i32,                   // +0x0130 (304)
    pub floor_y: f32,                // +0x0134 (308)
    pub _pad_138: [u8; 8],           // +0x0138 (312)
    pub r_side: ClVhitResult,        // +0x0140 (320) `_CL_VHIT_RESULT` (64B)
    pub l_side: ClVhitResult,        // +0x0180 (384) `_CL_VHIT_RESULT` (64B)
    pub foot: ClVhitResult,          // +0x01C0 (448) `_CL_VHIT_RESULT` (64B)
    pub ft_floor: ClVhitResult,      // +0x0200 (512) `_CL_VHIT_RESULT` (64B)
    pub r_foot: ClVhitResult,        // +0x0240 (576) `_CL_VHIT_RESULT` (64B)
    pub l_foot: ClVhitResult,        // +0x0280 (640) `_CL_VHIT_RESULT` (64B)
    pub column_mov: ClHitpolyColumn, // +0x02C0 (704) `_CL_HITPOLY_COLUMN` (48B)
    pub column_atk: ClHitpolyColumn, // +0x02F0 (752) `_CL_HITPOLY_COLUMN` (48B)
    pub col_mov_z_hosei: f32,        // +0x0320 (800)
    pub col_atk_z_hosei: f32,        // +0x0324 (804)
    pub wep_no_hit: u8,              // +0x0328 (808)
    pub push_wall: u8,               // +0x0329 (809)
    pub push_chara: u8,              // +0x032A (810)
    pub _pad_32b: [u8; 1],           // +0x032B (811)

    // =========================================================================
    // 5. Controller & Camera Input (+0x032C .. +0x048C)
    // =========================================================================
    pub pad: [PadInfo; 10], // +0x032C (812)  `PAD_INFO[10]` (320B)
    pub pad_id: i32,        // +0x046C (1132)
    pub cam_ang_y: f32,     // +0x0470 (1136)
    pub cam_ang_y_old: f32, // +0x0474 (1140)
    pub cam_ang_y_spd: f32, // +0x0478 (1144)
    pub move_ang: f32,      // +0x047C (1148)
    pub move_pow: f32,      // +0x0480 (1152)
    pub cam_chg: u8,        // +0x0484 (1156)
    pub _pad_485: [u8; 3],  // +0x0485 (1157)
    pub cam_chg_timer: f32, // +0x0488 (1160)

    // =========================================================================
    // 6. Audio, IK Angles, Vitals & Combat (+0x048C .. +0x0540)
    // =========================================================================
    pub se_upper: [u8; 4],        // +0x048C (1164)
    pub se_foot: [u8; 4],         // +0x0490 (1168)
    pub floor_rot_z: f32,         // +0x0494 (1172)
    pub floor_rot_x: f32,         // +0x0498 (1176)
    pub body_rot_z: f32,          // +0x049C (1180)
    pub body_rot_x: f32,          // +0x04A0 (1184)
    pub waist_rot_y: f32,         // +0x04A4 (1188)
    pub waist_rot_z: f32,         // +0x04A8 (1192)
    pub waist_rot_x: f32,         // +0x04AC (1196)
    pub r_knee_angle: f32,        // +0x04B0 (1200)
    pub l_knee_angle: f32,        // +0x04B4 (1204)
    pub body_up_down: f32,        // +0x04B8 (1208)
    pub _pad_4bc: [u8; 4],        // +0x04BC (1212)
    pub r_foot_pos: [f32; 4],     // +0x04C0 (1216)
    pub hold_chg: [u8; 2],        // +0x04D0 (1232)
    pub hold_loop: [u8; 2],       // +0x04D2 (1234)
    pub aim_h_angle: f32,         // +0x04D4 (1236)
    pub aim_v_angle: f32,         // +0x04D8 (1240)
    pub tgt_neck_angle_y: f32,    // +0x04DC (1244)
    pub tgt_neck_angle_x: f32,    // +0x04E0 (1248)
    pub tgt_neck_angle_z: f32,    // +0x04E4 (1252)
    pub gun_no_hit_timer: i32,    // +0x04E8 (1256)
    pub _pad_4ec: [u8; 4],        // +0x04EC (1260)
    pub l_foot_pos: [f32; 4],     // +0x04F0 (1264)
    pub gun_target_pos: [f32; 4], // +0x0500 (1280)
    pub enemy_liedown: Sh2Ptr32,  // +0x0510 (1296) `SubCharacter*`
    pub light_sw: i32,            // +0x0514 (1300)
    pub no_input_time: i32,       // +0x0518 (1304)
    pub no_action_time: i32,      // +0x051C (1308)
    pub hp: f32,                  // +0x0520 (1312)
    pub hp_max: f32,              // +0x0524 (1316)
    pub damage: f32,              // +0x0528 (1320)
    pub atk_reserve: [u8; 2],     // +0x052C (1324)
    pub _pad_52e: [u8; 2],        // +0x052E (1326)
    pub boat_ride: i32,           // +0x0530 (1328)
    pub boat_rot_y: f32,          // +0x0534 (1332)
    pub boat_spd_rot: f32,        // +0x0538 (1336)
    pub boat_spd: f32,            // +0x053C (1340)
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl PlayerState {
    /// Typed helper to decode `upper_st` into `JamesUpperStatus`.
    #[inline]
    pub const fn upper_status(&self) -> Option<JamesUpperStatus> {
        JamesUpperStatus::from_u8(self.upper_st)
    }

    /// Typed helper to decode `lower_st` into `JamesLowerStatus`.
    #[inline]
    pub const fn lower_status(&self) -> Option<JamesLowerStatus> {
        JamesLowerStatus::from_u8(self.lower_st)
    }
}

#[allow(non_camel_case_types)]
pub type shPlayerWork = PlayerState;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_player_state_size_and_offsets() {
        assert_eq!(size_of::<PlayerState>(), 1344);
        assert_eq!(offset_of!(PlayerState, player), 0x0000);
        assert_eq!(offset_of!(PlayerState, pos), 0x0030);
        assert_eq!(offset_of!(PlayerState, rot), 0x0040);
        assert_eq!(offset_of!(PlayerState, spd), 0x0050);
        assert_eq!(offset_of!(PlayerState, anime), 0x0064);
        assert_eq!(offset_of!(PlayerState, upper_st), 0x0068);
        assert_eq!(offset_of!(PlayerState, lower_st), 0x0069);
        assert_eq!(offset_of!(PlayerState, target), 0x0088);
        assert_eq!(offset_of!(PlayerState, look_tgt), 0x008C);
        assert_eq!(offset_of!(PlayerState, light_dir_y), 0x0098);
        assert_eq!(offset_of!(PlayerState, light_pos), 0x00C0);
        assert_eq!(offset_of!(PlayerState, room), 0x0130);
        assert_eq!(offset_of!(PlayerState, r_side), 0x0140);
        assert_eq!(offset_of!(PlayerState, l_side), 0x0180);
        assert_eq!(offset_of!(PlayerState, foot), 0x01C0);
        assert_eq!(offset_of!(PlayerState, ft_floor), 0x0200);
        assert_eq!(offset_of!(PlayerState, r_foot), 0x0240);
        assert_eq!(offset_of!(PlayerState, l_foot), 0x0280);
        assert_eq!(offset_of!(PlayerState, column_mov), 0x02C0);
        assert_eq!(offset_of!(PlayerState, column_atk), 0x02F0);
        assert_eq!(offset_of!(PlayerState, pad), 0x032C);
        assert_eq!(offset_of!(PlayerState, pad_id), 0x046C);
        assert_eq!(offset_of!(PlayerState, se_upper), 0x048C);
        assert_eq!(offset_of!(PlayerState, se_foot), 0x0490);
        assert_eq!(offset_of!(PlayerState, r_foot_pos), 0x04C0);
        assert_eq!(offset_of!(PlayerState, hold_chg), 0x04D0);
        assert_eq!(offset_of!(PlayerState, hold_loop), 0x04D2);
        assert_eq!(offset_of!(PlayerState, l_foot_pos), 0x04F0);
        assert_eq!(offset_of!(PlayerState, gun_target_pos), 0x0500);
        assert_eq!(offset_of!(PlayerState, enemy_liedown), 0x0510);
        assert_eq!(offset_of!(PlayerState, hp), 0x0520);
        assert_eq!(offset_of!(PlayerState, atk_reserve), 0x052C);
        assert_eq!(offset_of!(PlayerState, boat_spd), 0x053C);
    }

    #[test]
    fn test_james_status_enums() {
        assert_eq!(JamesUpperStatus::from_u8(0), Some(JamesUpperStatus::Stand));
        assert_eq!(
            JamesUpperStatus::from_u8(28),
            Some(JamesUpperStatus::Attack)
        );
        assert_eq!(JamesUpperStatus::from_u8(31), Some(JamesUpperStatus::Event));
        assert_eq!(JamesUpperStatus::from_u8(32), None);

        assert_eq!(JamesLowerStatus::from_u8(0), Some(JamesLowerStatus::Stand));
        assert_eq!(JamesLowerStatus::from_u8(9), Some(JamesLowerStatus::Walk));
        assert_eq!(JamesLowerStatus::from_u8(31), Some(JamesLowerStatus::Event));
        assert_eq!(JamesLowerStatus::from_u8(32), None);
    }
}
