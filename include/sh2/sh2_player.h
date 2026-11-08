/**
 * @file sh2_player.h
 * @brief Silent Hill 2 Player Workspace (`shPlayerWork`) & Action Status Enums
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28:
 * - `_JAMES_UPPER_STATUS` (DIE @0xeb2e)
 * - `_JAMES_LOWER_STATUS` (DIE @0xdeb8)
 * - `PlayerState` / `shPlayerWork` (1344B / 0x540, DIE @0x10b49)
 */

#ifndef SH2_PLAYER_H
#define SH2_PLAYER_H

#include "sh2_math.h"
#include "sh2_collision.h"
#include "sh2_pad.h"
#include "sh2_chara.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * James Upper-Body Animation / Action Status (`_JAMES_UPPER_STATUS`, DIE @0xeb2e)
 * Stored as a 1-byte value (`uint8_t`) in `PlayerState.upper_st`.
 */
typedef enum _JAMES_UPPER_STATUS {
    JMS_ST_U_STAND    = 0,  /* 0x00 */
    JMS_ST_U_RELAX    = 1,  /* 0x01 */
    JMS_ST_U_ALERT    = 2,  /* 0x02 */
    JMS_ST_U_TIRED    = 3,  /* 0x03 */
    JMS_ST_U_READY    = 4,  /* 0x04 */
    JMS_ST_U_READYOFF = 5,  /* 0x05 */
    JMS_ST_U_LROUND   = 6,  /* 0x06 */
    JMS_ST_U_RROUND   = 7,  /* 0x07 */
    JMS_ST_U_BACK     = 8,  /* 0x08 */
    JMS_ST_U_WALK     = 9,  /* 0x09 */
    JMS_ST_U_LSWALK   = 10, /* 0x0A */
    JMS_ST_U_RSWALK   = 11, /* 0x0B */
    JMS_ST_U_RUN1     = 12, /* 0x0C */
    JMS_ST_U_RUN2     = 13, /* 0x0D */
    JMS_ST_U_RUN3     = 14, /* 0x0E */
    JMS_ST_U_LSRUN    = 15, /* 0x0F */
    JMS_ST_U_RSRUN    = 16, /* 0x10 */
    JMS_ST_U_WALL_F   = 17, /* 0x11 */
    JMS_ST_U_LTURN    = 18, /* 0x12 */
    JMS_ST_U_RTURN    = 19, /* 0x13 */
    JMS_ST_U_JUMP     = 20, /* 0x14 */
    JMS_ST_U_GUARD    = 21, /* 0x15 */
    JMS_ST_U_LSTEP    = 22, /* 0x16 */
    JMS_ST_U_RSTEP    = 23, /* 0x17 */
    JMS_ST_U_FALL     = 24, /* 0x18 */
    JMS_ST_U_DAMAGE   = 25, /* 0x19 */
    JMS_ST_U_HOLD     = 26, /* 0x1A */
    JMS_ST_U_RELEASE  = 27, /* 0x1B */
    JMS_ST_U_ATTACK   = 28, /* 0x1C */
    JMS_ST_U_KICK     = 29, /* 0x1D */
    JMS_ST_U_TO_STAND = 30, /* 0x1E */
    JMS_ST_U_EVENT    = 31  /* 0x1F */
} _JAMES_UPPER_STATUS;

/**
 * James Lower-Body Locomotion Status (`_JAMES_LOWER_STATUS`, DIE @0xdeb8)
 * Stored as a 1-byte value (`uint8_t`) in `PlayerState.lower_st`.
 */
typedef enum _JAMES_LOWER_STATUS {
    JMS_ST_L_STAND    = 0,  /* 0x00 */
    JMS_ST_L_RELAX    = 1,  /* 0x01 */
    JMS_ST_L_ALERT    = 2,  /* 0x02 */
    JMS_ST_L_TIRED    = 3,  /* 0x03 */
    JMS_ST_L_READY    = 4,  /* 0x04 */
    JMS_ST_L_READYOFF = 5,  /* 0x05 */
    JMS_ST_L_LROUND   = 6,  /* 0x06 */
    JMS_ST_L_RROUND   = 7,  /* 0x07 */
    JMS_ST_L_BACK     = 8,  /* 0x08 */
    JMS_ST_L_WALK     = 9,  /* 0x09 */
    JMS_ST_L_LSWALK   = 10, /* 0x0A */
    JMS_ST_L_RSWALK   = 11, /* 0x0B */
    JMS_ST_L_RUN1     = 12, /* 0x0C */
    JMS_ST_L_RUN2     = 13, /* 0x0D */
    JMS_ST_L_RUN3     = 14, /* 0x0E */
    JMS_ST_L_LSRUN    = 15, /* 0x0F */
    JMS_ST_L_RSRUN    = 16, /* 0x10 */
    JMS_ST_L_WALL_F   = 17, /* 0x11 */
    JMS_ST_L_LTURN    = 18, /* 0x12 */
    JMS_ST_L_RTURN    = 19, /* 0x13 */
    JMS_ST_L_JUMP     = 20, /* 0x14 */
    JMS_ST_L_GUARD    = 21, /* 0x15 */
    JMS_ST_L_LSTEP    = 22, /* 0x16 */
    JMS_ST_L_RSTEP    = 23, /* 0x17 */
    JMS_ST_L_FALL     = 24, /* 0x18 */
    JMS_ST_L_DAMAGE   = 25, /* 0x19 */
    JMS_ST_L_HOLD     = 26, /* 0x1A */
    JMS_ST_L_RELEASE  = 27, /* 0x1B */
    JMS_ST_L_ATTACK   = 28, /* 0x1C */
    JMS_ST_L_KICK     = 29, /* 0x1D */
    JMS_ST_L_TO_STAND = 30, /* 0x1E */
    JMS_ST_L_EVENT    = 31  /* 0x1F */
} _JAMES_LOWER_STATUS;

/**
 * Global Player Workspace (`shPlayerWork`, 1344 bytes / 0x540, DIE @0x10b49)
 * Maps to `sh2gfw_Player_Work` in SLUS_202.28.
 */
typedef struct PlayerState {
    /* ====================================================================== *
     * 1. Kinematics & Wall Distances (+0x0000 .. +0x0064)                    *
     * ====================================================================== */
    SH2_PTR32(struct SubCharacter) player; /* +0x0000 (0)   Player SubCharacter pointer */
    float dist_rot;                        /* +0x0004 (4)   Target rotation delta */
    float dist_pos;                        /* +0x0008 (8)   Target position delta */
    float dir;                             /* +0x000C (12)  Movement direction angle */
    float to_wall;                         /* +0x0010 (16)  Forward distance to wall */
    float to_wall_r;                       /* +0x0014 (20)  Right-side distance to wall */
    float to_wall_l;                       /* +0x0018 (24)  Left-side distance to wall */
    float to_wall_b;                       /* +0x001C (28)  Backward distance to wall */
    float to_wall_f;                       /* +0x0020 (32)  Front ray distance to wall */
    float to_chara;                        /* +0x0024 (36)  Distance to nearest character */
    float r_wall;                          /* +0x0028 (40)  Right wall clearance */
    float l_wall;                          /* +0x002C (44)  Left wall clearance */
    Vector4 pos;                           /* +0x0030 (48)  World position */
    Vector4 rot;                           /* +0x0040 (64)  Euler rotation */
    Vector4 spd;                           /* +0x0050 (80)  Linear velocity */
    float rot_y;                           /* +0x0060 (96)  Yaw rotation angle */

    /* ====================================================================== *
     * 2. Action & Animation State Machine (+0x0064 .. +0x0098)               *
     * ====================================================================== */
    int32_t anime;                         /* +0x0064 (100) Active animation ID */
    uint8_t upper_st;                      /* +0x0068 (104) _JAMES_UPPER_STATUS */
    uint8_t lower_st;                      /* +0x0069 (105) _JAMES_LOWER_STATUS */
    uint8_t act_st;                        /* +0x006A (106) Action state */
    uint8_t sub_st;                        /* +0x006B (107) Sub-state */
    uint8_t up_st_old;                     /* +0x006C (108) Previous _JAMES_UPPER_STATUS */
    uint8_t low_st_old;                    /* +0x006D (109) Previous _JAMES_LOWER_STATUS */
    uint8_t act_st_old;                    /* +0x006E (110) Previous action state */
    uint8_t sub_st_old;                    /* +0x006F (111) Previous sub-state */
    uint8_t weapon;                        /* +0x0070 (112) Equipped weapon ID */
    uint8_t weapon_old;                    /* +0x0071 (113) Previously equipped weapon ID */
    uint8_t hold_st;                       /* +0x0072 (114) Weapon ready/hold state */
    uint8_t move_st;                       /* +0x0073 (115) Locomotion state */
    int32_t step;                          /* +0x0074 (116) State machine step */
    int32_t sub_step;                      /* +0x0078 (120) Sub-state step */
    float timer;                           /* +0x007C (124) General state timer */
    float timer2;                          /* +0x0080 (128) Secondary state timer */
    float tiredness;                       /* +0x0084 (132) Stamina / fatigue level */
    SH2_PTR32(struct SubCharacter) target; /* +0x0088 (136) Combat target SubCharacter */
    SH2_PTR32(struct SubCharacter) look_tgt; /* +0x008C (140) Head-tracking target SubCharacter */
    uint16_t flag;                         /* +0x0090 (144) Player state bitflags */
    uint8_t _pad_92[2];                    /* +0x0092 (146) Alignment padding */
    float light_dir_x;                     /* +0x0094 (148) Flashlight pitch angle */

    /* ====================================================================== *
     * 3. Flashlight & Neck Vectors (+0x0098 .. +0x0130)                      *
     * ====================================================================== */
    float light_dir_y;                     /* +0x0098 (152) Flashlight yaw angle */
    float neck_angle_y;                    /* +0x009C (156) Procedural neck yaw */
    float neck_angle_x;                    /* +0x00A0 (160) Procedural neck pitch */
    float neck_angle_z;                    /* +0x00A4 (164) Procedural neck roll */
    float light_angle_y;                   /* +0x00A8 (168) Flashlight world yaw */
    float light_angle_x;                   /* +0x00AC (172) Flashlight world pitch */
    Vector3 neck_point;                    /* +0x00B0 (176) Neck pivot world position */
    uint8_t _pad_bc[4];                    /* +0x00BC (188) 16-byte alignment padding */
    Vector4 light_pos;                     /* +0x00C0 (192) Flashlight origin position */
    Vector4 light_dir;                     /* +0x00D0 (208) Flashlight direction vector */
    Vector4 hand_pos;                      /* +0x00E0 (224) Right hand world position */
    Vector4 focus_pos;                     /* +0x00F0 (240) Camera/gaze focus position */
    Vector4 eye_pos;                       /* +0x0100 (256) Eye world position */
    Vector4 neck_dir;                      /* +0x0110 (272) Neck forward direction vector */
    Vector4 look_pos;                      /* +0x0120 (288) Look-at target world position */

    /* ====================================================================== *
     * 4. Room & Collision Sensors (+0x0130 .. +0x032C)                       *
     * ====================================================================== */
    int32_t room;                          /* +0x0130 (304) Current room ID */
    float floor_y;                         /* +0x0134 (308) Floor elevation at player */
    uint8_t _pad_138[8];                   /* +0x0138 (312) 16-byte alignment padding */
    _CL_VHIT_RESULT r_side;                /* +0x0140 (320) Right-side raycast sensor (64B) */
    _CL_VHIT_RESULT l_side;                /* +0x0180 (384) Left-side raycast sensor (64B) */
    _CL_VHIT_RESULT foot;                  /* +0x01C0 (448) Center foot raycast sensor (64B) */
    _CL_VHIT_RESULT ft_floor;              /* +0x0200 (512) Forward floor raycast sensor (64B) */
    _CL_VHIT_RESULT r_foot;                /* +0x0240 (576) Right foot raycast sensor (64B) */
    _CL_VHIT_RESULT l_foot;                /* +0x0280 (640) Left foot raycast sensor (64B) */
    _CL_HITPOLY_COLUMN column_mov;         /* +0x02C0 (704) Movement collision cylinder (48B) */
    _CL_HITPOLY_COLUMN column_atk;         /* +0x02F0 (752) Attack collision cylinder (48B) */
    float col_mov_z_hosei;                 /* +0x0320 (800) Movement cylinder Z correction */
    float col_atk_z_hosei;                 /* +0x0324 (804) Attack cylinder Z correction */
    uint8_t wep_no_hit;                    /* +0x0328 (808) Weapon wall-clipping flag */
    uint8_t push_wall;                     /* +0x0329 (809) Pushing against wall flag */
    uint8_t push_chara;                    /* +0x032A (810) Pushing against character flag */
    uint8_t _pad_32b[1];                   /* +0x032B (811) Alignment padding */

    /* ====================================================================== *
     * 5. Controller & Camera Input (+0x032C .. +0x048C)                      *
     * ====================================================================== */
    PAD_INFO pad[10];                      /* +0x032C (812)  Controller input ring buffer (320B) */
    int32_t pad_id;                        /* +0x046C (1132) Current ring buffer write index */
    float cam_ang_y;                       /* +0x0470 (1136) Camera yaw angle */
    float cam_ang_y_old;                   /* +0x0474 (1140) Previous camera yaw angle */
    float cam_ang_y_spd;                   /* +0x0478 (1144) Camera yaw angular velocity */
    float move_ang;                        /* +0x047C (1148) Input movement angle */
    float move_pow;                        /* +0x0480 (1152) Input movement magnitude */
    uint8_t cam_chg;                       /* +0x0484 (1156) Camera cut / angle change flag */
    uint8_t _pad_485[3];                   /* +0x0485 (1157) Alignment padding */
    float cam_chg_timer;                   /* +0x0488 (1160) Camera change grace timer */

    /* ====================================================================== *
     * 6. Audio, IK Angles, Vitals & Combat (+0x048C .. +0x0540)              *
     * ====================================================================== */
    uint8_t se_upper[4];                   /* +0x048C (1164) Upper-body sound effect slots */
    uint8_t se_foot[4];                    /* +0x0490 (1168) Footstep sound effect slots */
    float floor_rot_z;                     /* +0x0494 (1172) Floor slope roll angle */
    float floor_rot_x;                     /* +0x0498 (1176) Floor slope pitch angle */
    float body_rot_z;                      /* +0x049C (1180) Procedural body roll angle */
    float body_rot_x;                      /* +0x04A0 (1184) Procedural body pitch angle */
    float waist_rot_y;                     /* +0x04A4 (1188) Procedural waist yaw angle */
    float waist_rot_z;                     /* +0x04A8 (1192) Procedural waist roll angle */
    float waist_rot_x;                     /* +0x04AC (1196) Procedural waist pitch angle */
    float r_knee_angle;                    /* +0x04B0 (1200) Right knee IK bend angle */
    float l_knee_angle;                    /* +0x04B4 (1204) Left knee IK bend angle */
    float body_up_down;                    /* +0x04B8 (1208) Pelvis vertical offset */
    uint8_t _pad_4bc[4];                   /* +0x04BC (1212) 16-byte alignment padding */
    Vector4 r_foot_pos;                    /* +0x04C0 (1216) Right foot IK world position */
    uint8_t hold_chg[2];                   /* +0x04D0 (1232) Weapon hold transition state */
    uint8_t hold_loop[2];                  /* +0x04D2 (1234) Weapon hold loop state */
    float aim_h_angle;                     /* +0x04D4 (1236) Horizontal weapon aim angle */
    float aim_v_angle;                     /* +0x04D8 (1240) Vertical weapon aim angle */
    float tgt_neck_angle_y;                /* +0x04DC (1244) Target neck yaw angle */
    float tgt_neck_angle_x;                /* +0x04E0 (1248) Target neck pitch angle */
    float tgt_neck_angle_z;                /* +0x04E4 (1252) Target neck roll angle */
    int32_t gun_no_hit_timer;              /* +0x04E8 (1256) Firearm obstruction timer */
    uint8_t _pad_4ec[4];                   /* +0x04EC (1260) 16-byte alignment padding */
    Vector4 l_foot_pos;                    /* +0x04F0 (1264) Left foot IK world position */
    Vector4 gun_target_pos;                /* +0x0500 (1280) Firearm aim target position */
    SH2_PTR32(struct SubCharacter) enemy_liedown; /* +0x0510 (1296) Downed enemy stomp target */
    int32_t light_Sw;                      /* +0x0514 (1300) Flashlight switch state */
    int32_t no_input_time;                 /* +0x0518 (1304) Idle no-input timer */
    int32_t no_action_time;                /* +0x051C (1308) Idle no-action timer */
    float hp;                              /* +0x0520 (1312) Player current hit points */
    float hp_max;                          /* +0x0524 (1316) Player maximum hit points */
    float damage;                          /* +0x0528 (1320) Pending damage accumulator */
    uint8_t atk_reserve[2];                /* +0x052C (1324) Queued combo attack input */
    uint8_t _pad_52e[2];                   /* +0x052E (1326) Alignment padding */
    int32_t boat_ride;                     /* +0x0530 (1328) Lake boat rowboat mode flag */
    float boat_rot_y;                      /* +0x0534 (1332) Boat yaw angle */
    float boat_spd_rot;                    /* +0x0538 (1336) Boat rotational velocity */
    float boat_spd;                        /* +0x053C (1340) Boat forward velocity */
} PlayerState;

typedef PlayerState shPlayerWork;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(PlayerState) == 1344, "PlayerState must be 1344 bytes (0x540)");
_Static_assert(offsetof(PlayerState, player) == 0x0000, "PlayerState.player offset mismatch");
_Static_assert(offsetof(PlayerState, pos) == 0x0030, "PlayerState.pos offset mismatch");
_Static_assert(offsetof(PlayerState, rot) == 0x0040, "PlayerState.rot offset mismatch");
_Static_assert(offsetof(PlayerState, spd) == 0x0050, "PlayerState.spd offset mismatch");
_Static_assert(offsetof(PlayerState, anime) == 0x0064, "PlayerState.anime offset mismatch");
_Static_assert(offsetof(PlayerState, upper_st) == 0x0068, "PlayerState.upper_st offset mismatch");
_Static_assert(offsetof(PlayerState, lower_st) == 0x0069, "PlayerState.lower_st offset mismatch");
_Static_assert(offsetof(PlayerState, target) == 0x0088, "PlayerState.target offset mismatch");
_Static_assert(offsetof(PlayerState, look_tgt) == 0x008C, "PlayerState.look_tgt offset mismatch");
_Static_assert(offsetof(PlayerState, light_dir_y) == 0x0098, "PlayerState.light_dir_y offset mismatch");
_Static_assert(offsetof(PlayerState, light_pos) == 0x00C0, "PlayerState.light_pos offset mismatch");
_Static_assert(offsetof(PlayerState, room) == 0x0130, "PlayerState.room offset mismatch");
_Static_assert(offsetof(PlayerState, r_side) == 0x0140, "PlayerState.r_side offset mismatch");
_Static_assert(offsetof(PlayerState, l_side) == 0x0180, "PlayerState.l_side offset mismatch");
_Static_assert(offsetof(PlayerState, foot) == 0x01C0, "PlayerState.foot offset mismatch");
_Static_assert(offsetof(PlayerState, ft_floor) == 0x0200, "PlayerState.ft_floor offset mismatch");
_Static_assert(offsetof(PlayerState, r_foot) == 0x0240, "PlayerState.r_foot offset mismatch");
_Static_assert(offsetof(PlayerState, l_foot) == 0x0280, "PlayerState.l_foot offset mismatch");
_Static_assert(offsetof(PlayerState, column_mov) == 0x02C0, "PlayerState.column_mov offset mismatch");
_Static_assert(offsetof(PlayerState, column_atk) == 0x02F0, "PlayerState.column_atk offset mismatch");
_Static_assert(offsetof(PlayerState, pad) == 0x032C, "PlayerState.pad offset mismatch");
_Static_assert(offsetof(PlayerState, pad_id) == 0x046C, "PlayerState.pad_id offset mismatch");
_Static_assert(offsetof(PlayerState, se_upper) == 0x048C, "PlayerState.se_upper offset mismatch");
_Static_assert(offsetof(PlayerState, se_foot) == 0x0490, "PlayerState.se_foot offset mismatch");
_Static_assert(offsetof(PlayerState, r_foot_pos) == 0x04C0, "PlayerState.r_foot_pos offset mismatch");
_Static_assert(offsetof(PlayerState, hold_chg) == 0x04D0, "PlayerState.hold_chg offset mismatch");
_Static_assert(offsetof(PlayerState, hold_loop) == 0x04D2, "PlayerState.hold_loop offset mismatch");
_Static_assert(offsetof(PlayerState, l_foot_pos) == 0x04F0, "PlayerState.l_foot_pos offset mismatch");
_Static_assert(offsetof(PlayerState, gun_target_pos) == 0x0500, "PlayerState.gun_target_pos offset mismatch");
_Static_assert(offsetof(PlayerState, enemy_liedown) == 0x0510, "PlayerState.enemy_liedown offset mismatch");
_Static_assert(offsetof(PlayerState, hp) == 0x0520, "PlayerState.hp offset mismatch");
_Static_assert(offsetof(PlayerState, atk_reserve) == 0x052C, "PlayerState.atk_reserve offset mismatch");
_Static_assert(offsetof(PlayerState, boat_spd) == 0x053C, "PlayerState.boat_spd offset mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SH2_PLAYER_H */
