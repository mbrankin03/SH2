/**
 * @file sh2_chara.h
 * @brief Silent Hill 2 Character, Skeleton, and Battle Structures
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28:
 * - `shSkelton` (240B / 0xF0, DIE @0xe0ec)
 * - `shBattleArea` (8B, DIE @0xf2d8)
 * - `shBattleInfo` (128B / 0x80, DIE @0xf3d3)
 * - `SubCharacter` (496B / 0x1F0, DIE @0xd4e2)
 */

#ifndef SH2_CHARA_H
#define SH2_CHARA_H

#include "sh2_math.h"
#include "sh2_collision.h"

#ifdef __cplusplus
extern "C" {
#endif

struct SubCharacter;

/**
 * Character Skeleton Bone Node (240 bytes / 0xF0, DIE @0xe0ec)
 */
typedef struct shSkelton {
    SH2_PTR32(struct shSkelton) next;   /* +0x00 (0)   Next sibling/chain bone pointer */
    SH2_PTR32(struct shSkelton) parent; /* +0x04 (4)   Parent bone pointer */
    uint8_t _pad0[8];                   /* +0x08 (8)   16-byte alignment padding */
    Matrix4x4 src_m;                    /* +0x10 (16)  Source rotation/scale matrix (64B) */
    Vector4 src_t;                      /* +0x50 (80)  Source translation vector (16B) */
    Matrix4x4 des_m;                    /* +0x60 (96)  Destination matrix (64B) */
    Vector4 des_t;                      /* +0xA0 (160) Destination translation vector (16B) */
    Vector4 axis;                       /* +0xB0 (176) Rotation axis vector (16B) */
    float theta;                        /* +0xC0 (192) Rotation angle */
    float xx;                           /* +0xC4 (196) */
    float yy;                           /* +0xC8 (200) */
    float zz;                           /* +0xCC (204) */
    float xy;                           /* +0xD0 (208) */
    float yz;                           /* +0xD4 (212) */
    float zx;                           /* +0xD8 (216) */
    uint16_t c_count;                   /* +0xDC (220) Interpolation counter */
    uint16_t c_speed;                   /* +0xDE (222) Interpolation speed */
    int8_t change;                      /* +0xE0 (224) State change flag */
    int8_t reserved;                    /* +0xE1 (225) */
    int8_t is_key;                      /* +0xE2 (226) Keyframe flag */
    int8_t pad;                         /* +0xE3 (227) */
    SH2_PTR32(void) untouchable;        /* +0xE4 (228) */
    uint8_t _pad1[8];                   /* +0xE8 (232) PS2 quadword tail padding to 0xF0 */
} shSkelton;

/**
 * Character Sensory Detection Cone / Radius (8 bytes, DIE @0xf2d8)
 */
typedef struct shBattleArea {
    float center;                       /* +0x00 (0) Detection center distance */
    float radius;                       /* +0x04 (4) Detection radius */
} shBattleArea;

/**
 * Character Combat & Health State (128 bytes / 0x80, DIE @0xf3d3)
 */
typedef struct shBattleInfo {
    Vector4 pos;                        /* +0x00 (0)   Attack origin / hit position */
    Vector4 vec;                        /* +0x10 (16)  Attack direction vector */
    uint16_t id;                        /* +0x20 (32)  Weapon / attack ID */
    uint16_t kind;                      /* +0x22 (34)  Attack kind */
    float dead_timer;                   /* +0x24 (36)  Death state timer */
    float damage;                       /* +0x28 (40)  Last damage amount */
    float shock;                        /* +0x2C (44)  Stagger / shock amount */
    int32_t atk_result;                 /* +0x30 (48)  Attack hit result code */
    uint8_t _pad0[12];                  /* +0x34 (52)  16-byte alignment padding */
    Vector4 prev_atk_pos;               /* +0x40 (64)  Previous frame attack position */
    SH2_PTR32(struct SubCharacter) target; /* +0x50 (80) Combat target character pointer */
    float hp;                           /* +0x54 (84)  Current hit points */
    float hp_max;                       /* +0x58 (88)  Maximum hit points */
    float hp_rate;                      /* +0x5C (92)  Normalized HP ratio (hp / hp_max) */
    uint32_t status;                    /* +0x60 (96)  Combat status flags */
    shBattleArea look;                  /* +0x64 (100) Visual awareness area */
    shBattleArea feel;                  /* +0x6C (108) Proximity awareness area */
    uint8_t se;                         /* +0x74 (116) Sound effect trigger ID */
    uint8_t _pad1[11];                  /* +0x75 (117) PS2 quadword tail padding to 0x80 */
} shBattleInfo;

/**
 * Runtime Character / Actor Entity (496 bytes / 0x1F0, DIE @0xd4e2)
 * Pointed to by `shPlayerWork.player`, `target`, `look_tgt`, and `enemy_liedown`.
 */
typedef struct SubCharacter {
    int32_t index;                      /* +0x000 (0)   Actor slot index */
    uint32_t status;                    /* +0x004 (4)   Primary actor status */
    uint32_t sub_status;                /* +0x008 (8)   Secondary actor status */
    uint32_t sub_st;                    /* +0x00C (12)  Sub-state flags */
    int16_t kind;                       /* +0x010 (16)  Character kind / archetype ID */
    int16_t id;                         /* +0x012 (18)  Instance ID */
    uint32_t step;                      /* +0x014 (20)  State machine step counter */
    uint32_t model_type;                /* +0x018 (24)  3D model type identifier */
    uint8_t _pad0[4];                   /* +0x01C (28)  16-byte alignment padding */
    Vector4 pos;                        /* +0x020 (32)  World position */
    Vector4 rot;                        /* +0x030 (48)  Euler rotation angles */
    Vector4 pos_spd;                    /* +0x040 (64)  Linear velocity vector */
    Vector4 rot_spd;                    /* +0x050 (80)  Angular velocity vector */
    Matrix4x4 mat;                      /* +0x060 (96)  World transformation matrix (64B) */
    SH2_PTR32(struct shSkelton) sk_top; /* +0x0A0 (160) Root skeleton bone pointer */
    float eye_y;                        /* +0x0A4 (164) Eye height offset */
    float center_y;                     /* +0x0A8 (168) Center-of-mass height offset */
    float spd;                          /* +0x0AC (172) Current scalar horizontal speed */
    float spd_org;                      /* +0x0B0 (176) Base scalar horizontal speed */
    float spd_y;                        /* +0x0B4 (180) Vertical velocity */
    float spd_roty;                     /* +0x0B8 (184) Yaw rotation speed */
    uint8_t _pad1[4];                   /* +0x0BC (188) 16-byte alignment padding */
    Vector4 grnd_normal;                /* +0x0C0 (192) Ground surface normal vector */
    float grnd_height;                  /* +0x0D0 (208) Ground elevation Y */
    uint8_t _pad2[12];                  /* +0x0D4 (212) 16-byte alignment padding */
    Vector4 b_pos;                      /* +0x0E0 (224) Backup / previous position */
    Vector4 b_rot;                      /* +0x0F0 (240) Backup / previous rotation */
    int32_t en_first_status;            /* +0x100 (256) Initial enemy spawn status */
    int32_t colis_fall_timer;           /* +0x104 (260) Collision fall timer */
    uint8_t _pad3[8];                   /* +0x108 (264) 16-byte alignment padding */
    shBattleInfo battle;                /* +0x110 (272) Combat & health state (128B) */
    _CL_VHIT_RESULT eye;                /* +0x190 (400) Line-of-sight raycast result (64B) */
    SH2_PTR32(void) function;           /* +0x1D0 (464) Main update callback pointer */
    SH2_PTR32(void) effecter_function;  /* +0x1D4 (468) Effector callback pointer */
    SH2_PTR32(struct SubCharacter) pre; /* +0x1D8 (472) Previous linked-list character */
    SH2_PTR32(struct SubCharacter) next;/* +0x1DC (476) Next linked-list character */
    uint8_t work[4];                    /* +0x1E0 (480) Scratchpad work bytes */
    SH2_PTR32(void) enemy_p;            /* +0x1E4 (484) Enemy-specific data pointer */
    uint8_t _pad4[8];                   /* +0x1E8 (488) PS2 quadword tail padding to 0x1F0 */
} SubCharacter;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(shSkelton) == 240, "shSkelton must be 240 bytes");
_Static_assert(offsetof(shSkelton, src_m) == 0x10, "shSkelton.src_m offset mismatch");
_Static_assert(offsetof(shSkelton, des_m) == 0x60, "shSkelton.des_m offset mismatch");
_Static_assert(offsetof(shSkelton, untouchable) == 0xE4, "shSkelton.untouchable offset mismatch");

_Static_assert(sizeof(shBattleArea) == 8, "shBattleArea must be 8 bytes");
_Static_assert(sizeof(shBattleInfo) == 128, "shBattleInfo must be 128 bytes");
_Static_assert(offsetof(shBattleInfo, prev_atk_pos) == 0x40, "shBattleInfo.prev_atk_pos offset mismatch");
_Static_assert(offsetof(shBattleInfo, target) == 0x50, "shBattleInfo.target offset mismatch");
_Static_assert(offsetof(shBattleInfo, look) == 0x64, "shBattleInfo.look offset mismatch");
_Static_assert(offsetof(shBattleInfo, se) == 0x74, "shBattleInfo.se offset mismatch");

_Static_assert(sizeof(SubCharacter) == 496, "SubCharacter must be 496 bytes (0x1F0)");
_Static_assert(offsetof(SubCharacter, pos) == 0x020, "SubCharacter.pos offset mismatch");
_Static_assert(offsetof(SubCharacter, mat) == 0x060, "SubCharacter.mat offset mismatch");
_Static_assert(offsetof(SubCharacter, sk_top) == 0x0A0, "SubCharacter.sk_top offset mismatch");
_Static_assert(offsetof(SubCharacter, grnd_normal) == 0x0C0, "SubCharacter.grnd_normal offset mismatch");
_Static_assert(offsetof(SubCharacter, b_pos) == 0x0E0, "SubCharacter.b_pos offset mismatch");
_Static_assert(offsetof(SubCharacter, battle) == 0x110, "SubCharacter.battle offset mismatch");
_Static_assert(offsetof(SubCharacter, eye) == 0x190, "SubCharacter.eye offset mismatch");
_Static_assert(offsetof(SubCharacter, function) == 0x1D0, "SubCharacter.function offset mismatch");
_Static_assert(offsetof(SubCharacter, enemy_p) == 0x1E4, "SubCharacter.enemy_p offset mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SH2_CHARA_H */
