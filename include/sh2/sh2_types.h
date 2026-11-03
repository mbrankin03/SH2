/*
 * sh2_types.h - Core engine structs and types extracted from Silent Hill 2
 * unstripped prototype ELFs (July 13, 2001 SLUS_202.28 and E3 Trial SLPM_123.45).
 *
 * Byte-exact layouts matching PS2 EE MIPS and PC x86 data alignments.
 */

#ifndef SH2_TYPES_H
#define SH2_TYPES_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===================================================================== */
/* 1. Vector and Matrix Mathematics                                       */
/* ===================================================================== */

/* 4D Vector / Homogeneous Coordinate (16 bytes) */
typedef struct Vector4 {
    float x;
    float y;
    float z;
    float w;
} Vector4;

/* 3D Vector (12 bytes) */
typedef struct Vector3 {
    float x;
    float y;
    float z;
} Vector3;

/* 4x4 Matrix (64 bytes, row-major / PS2 vu0 layout) */
typedef union Matrix4x4 {
    float m[4][4];
    float f[16];
    struct {
        Vector4 r0;
        Vector4 r1;
        Vector4 r2;
        Vector4 r3;
    };
} Matrix4x4;

/* World-View-Screen Matrix Triple (_VbWVSMATRIX, 192 bytes from DWARF) */
typedef struct _VbWVSMATRIX {
    Matrix4x4 wvm;  /* World-to-View matrix (64 bytes) */
    Matrix4x4 vsm;  /* View-to-Screen matrix (64 bytes) */
    Matrix4x4 wsm;  /* World-to-Screen matrix (64 bytes) */
} _VbWVSMATRIX;

/* ===================================================================== */
/* 2. Camera Subsystem Types (_VC_WORK & Camera)                         */
/* ===================================================================== */

typedef struct _VC_CAM_MV_PARAM {
    float accel_xz;
    float accel_y;
    float max_spd_xz;
    float max_spd_y;
} _VC_CAM_MV_PARAM;

typedef struct _VC_WATCH_MV_PARAM {
    float ang_accel_x;
    float ang_accel_y;
    float max_ang_spd_x;
    float max_ang_spd_y;
} _VC_WATCH_MV_PARAM;

typedef struct _VC_THROUGH_DOOR_CAM_PARAM {
    int32_t active_f;
    float   timer;
    float   rail_ang_y;
    float   rail_sta_pos[4];
    float   rail_sta_to_chara_dist;
} _VC_THROUGH_DOOR_CAM_PARAM;

typedef struct _VC_FIX_CAM_MANAGER {
    int32_t exception_f;
    float   add_ang_y;
    float   add_rd_x;
    float   add_rd_z;
} _VC_FIX_CAM_MANAGER;

typedef struct _VC_CIR_CAM_MANAGER {
    float origin[4];
    float sw_l[4];
} _VC_CIR_CAM_MANAGER;

/* Camera structure matching _VC_WORK runtime camera state */
typedef struct Camera {
    int32_t                     view_cam_active_f;            /* +0x000 */
    void*                       vc_road_ary_list;             /* +0x004 */
    uint32_t                    flags;                        /* +0x008 */
    int32_t                     through_door_activate_init_f; /* +0x00C */
    _VC_THROUGH_DOOR_CAM_PARAM  through_door;                 /* +0x010 */
    int32_t                     cam_noise;                    /* +0x030 */
    float                       scr_half_ang_wy;              /* +0x034 */
    float                       scr_half_ang_wx;              /* +0x038 */
    _VC_CAM_MV_PARAM            user_cam_mv_prm;              /* +0x03C */
    float                       cam_tgt_pos[4];               /* +0x04C */
    float                       cam_pos[4];                   /* +0x05C (cam_pos[1] = Y) */
    float                       cam_mv_ang_y;                 /* +0x06C */
    float                       cam_velo[4];                  /* +0x070 */
    float                       old_cam_excl_area_r;          /* +0x080 */
    float                       ideal_cam_pos_h;              /* +0x084 */
    _VC_WATCH_MV_PARAM          user_watch_mv_prm;            /* +0x088 */
    float                       watch_tgt_pos[4];             /* +0x098 */
    float                       watch_tgt_max_y;              /* +0x0A8 */
    float                       watch_tgt_ang_z;              /* +0x0AC */
    float                       cam_mat_ang[4];               /* +0x0B0 */
    Matrix4x4                   cam_mat;                      /* +0x0C0 */
    float                       ofs_cam_ang[4];               /* +0x100 */
    float                       ofs_cam_ang_spd[4];           /* +0x110 */
    float                       base_cam_ang[4];              /* +0x120 */
    float                       old_base_cam_ang[4];          /* +0x130 */
    float                       cam_chara2ideal_ang_y;        /* +0x140 */
    float                       cam_tgt_velo[4];              /* +0x144 */
    float                       cam_tgt_mv_ang_y;             /* +0x154 */
    float                       cam_tgt_spd;                  /* +0x158 */
    float                       chara_pos[4];                 /* +0x15C */
    float                       chara_bottom_y;               /* +0x16C */
    float                       chara_top_y;                  /* +0x170 */
    float                       chara_center_y;               /* +0x174 */
    float                       chara_grnd_y;                 /* +0x178 */
    float                       chara_head_pos[4];            /* +0x17C */
    float                       chara_mv_spd;                 /* +0x18C */
    float                       chara_mv_ang_y;               /* +0x190 */
    float                       chara_ang_spd_y;              /* +0x194 */
    float                       chara_eye_ang_y;              /* +0x198 */
    float                       chara_eye_ang_wy;             /* +0x19C */
} Camera;

/* ===================================================================== */
/* 3. Player State Structure (shPlayerWork, 1344 bytes)                  */
/* ===================================================================== */

/* Automatically generated exact C layout for shPlayerWork (PlayerState) */
typedef struct PlayerState {
    void*     player; uint8_t _pad_player[12]; /* +0x0000 */
    float     dist_rot[4];                     /* +0x0010 */
    float     dist_pos[4];                     /* +0x0020 */
    float     pos[4];                          /* +0x0030 */
    float     rot[4];                          /* +0x0040 */
    float     rot_y;                           /* +0x0050 */
    float     inner_to_wall;                   /* +0x0054 */
    float     dist_to_wall;                    /* +0x0058 */
    float     dir_tgt_2d;                      /* +0x005c */
    float     dir_now_2d;                      /* +0x0060 */
    uint8_t   map_mode;                        /* +0x0064 */
    uint8_t   reserve0;                        /* +0x0065 */
    uint8_t   step;                            /* +0x0066 */
    uint8_t   sub_step;                        /* +0x0067 */
    uint16_t  timer;                           /* +0x0068 */
    uint16_t  timer2;                          /* +0x006a */
    uint32_t  allbody_now;                     /* +0x006c */
    uint8_t   upper_now;                       /* +0x0070 */
    uint8_t   lower_now; uint8_t _pad_lower_now[2]; /* +0x0071 */
    uint32_t  allbody_prev;                    /* +0x0074 */
    uint8_t   upper_prev;                      /* +0x0078 */
    uint8_t   lower_prev;                      /* +0x0079 */
    uint8_t   event_status_now;                /* +0x007a */
    uint8_t   event_status_prev;               /* +0x007b */
    uint8_t   event_move_mode;                 /* +0x007c */
    uint8_t   reserve1; uint8_t _pad_reserve1[2]; /* +0x007d */
    uint32_t  upper_st_flg;                    /* +0x0080 */
    uint32_t  lower_st_flg;                    /* +0x0084 */
    uint32_t  u_anime_st_flg;                  /* +0x0088 */
    uint32_t  l_anime_st_flg;                  /* +0x008c */
    uint32_t  anime_pause;                     /* +0x0090 */
    uint32_t  act_with_wep;                    /* +0x0094 */
    float     light_vec_inner_rate; float _pad_light_vec_inner_rate; /* +0x0098 */
    float     light_pos_revise[4];             /* +0x00a0 */
    float     light_vec_revise[4];             /* +0x00b0 */
    float     light_pos[4];                    /* +0x00c0 */
    float     light_vec[4];                    /* +0x00d0 */
    float     light_pos_revise_reverse[4];     /* +0x00e0 */
    float     light_vec_revise_reverse[4];     /* +0x00f0 */
    float     light_pos_reverse[4];            /* +0x0100 */
    float     light_vec_reverse[4];            /* +0x0110 */
    float     light_vec_neck[4];               /* +0x0120 */
    int32_t   room_name_prev;                  /* +0x0130 */
    uint8_t   room_name_now[12];               /* +0x0134 */
    uint8_t   r_side[64];                      /* +0x0140 */
    uint8_t   l_side[64];                      /* +0x0180 */
    uint8_t   foot[64];                        /* +0x01c0 */
    uint8_t   ft_floor[64];                    /* +0x0200 */
    uint8_t   r_foot[64];                      /* +0x0240 */
    uint8_t   l_foot[64];                      /* +0x0280 */
    uint8_t   column_mov[48];                  /* +0x02c0 */
    uint8_t   column_atk[48];                  /* +0x02f0 */
    float     col_mov_z_hosei;                 /* +0x0320 */
    float     col_atk_z_hosei;                 /* +0x0324 */
    uint8_t   wep_no_hit_floor;                /* +0x0328 */
    uint8_t   efct_upper;                      /* +0x0329 */
    uint8_t   light_reverse;                   /* +0x032a */
    uint8_t   water_road;                      /* +0x032b */
    uint8_t   pad[320];                        /* +0x032c */
    float     rstick_x;                        /* +0x046c */
    float     rstick_y;                        /* +0x0470 */
    float     lstick_x;                        /* +0x0474 */
    float     lstick_y;                        /* +0x0478 */
    float     lstick_p;                        /* +0x047c */
    float     cam_rot_y[2];                    /* +0x0480 */
    uint8_t   now_cam_no;                      /* +0x0488 */
    uint8_t   cam_chg_flg;                     /* +0x0489 */
    uint8_t   ctrl_unit;                       /* +0x048a */
    uint8_t   ctrl_inputing;                   /* +0x048b */
    uint32_t  se_upper;                        /* +0x048c */
    uint32_t  se_foot;                         /* +0x0490 */
    float     csaw_se_vol;                     /* +0x0494 */
    uint8_t   csaw_se_flg;                     /* +0x0498 */
    uint8_t   d_shock;                         /* +0x0499 */
    uint8_t   reserve2;                        /* +0x049a */
    uint8_t   strike_splash_flg;               /* +0x049b */
    int32_t   event_anime;                     /* +0x049c */
    float     tgt_body_angle[4];               /* +0x04a0 */
    float     tgt_neck_angle[4];               /* +0x04b0 */
    float     tgt_arms_angle[4];               /* +0x04c0 */
    uint16_t  hold_chg;                        /* +0x04d0 */
    uint16_t  hold_loop;                       /* +0x04d2 */
    uint8_t   motion_no;                       /* +0x04d4 */
    uint8_t   shotgun_dir;                     /* +0x04d5 */
    uint8_t   shotgun_prev;                    /* +0x04d6 */
    uint8_t   guard_check;                     /* +0x04d7 */
    int32_t   parts_light;                     /* +0x04d8 */
    int32_t   parts_rhand;                     /* +0x04dc */
    int32_t   parts_lhand;                     /* +0x04e0 */
    void*     target;                          /* +0x04e4 */
    void*     look_tgt;                        /* +0x04e8 */
    float     non_input;                       /* +0x04ec */
    float     hugging_gauge;                   /* +0x04f0 */
    float     running_time;                    /* +0x04f4 */
    float     muteki_time;                     /* +0x04f8 */
    float     hp;                              /* +0x04fc */
    float     hp_max;                          /* +0x0500 */
    float     hp_recover;                      /* +0x0504 */
    float     shock;                           /* +0x0508 */
    float     spray_time;                      /* +0x050c */
    int32_t   tired;                           /* +0x0510 */
    int32_t   tired_max;                       /* +0x0514 */
    int32_t   spirit;                          /* +0x0518 */
    void*     enemy_liedown;                   /* +0x051c */
    uint8_t   enemy_around;                    /* +0x0520 */
    uint8_t   enemy_atk_area;                  /* +0x0521 */
    uint8_t   enemy_near;                      /* +0x0522 */
    uint8_t   lock_on;                         /* +0x0523 */
    uint8_t   running;                         /* +0x0524 */
    uint8_t   cannot_run;                      /* +0x0525 */
    uint8_t   fall_type;                       /* +0x0526 */
    uint8_t   reload;                          /* +0x0527 */
    uint8_t   weapon;                          /* +0x0528 */
    uint8_t   shoot_val;                       /* +0x0529 */
    uint8_t   reload_val;                      /* +0x052a */
    uint8_t   atk_type;                        /* +0x052b */
    uint16_t  atk_reserve;                     /* +0x052c */
    uint8_t   attack_ok;                       /* +0x052e */
    int8_t    hold_type;                       /* +0x052f */
    uint8_t   dead;                            /* +0x0530 */
    uint8_t   no_damage;                       /* +0x0531 */
    uint8_t   atk_count;                       /* +0x0532 */
    uint8_t   hug_status;                      /* +0x0533 */
    uint8_t   hug_dir;                         /* +0x0534 */
    uint8_t   se_on;                           /* +0x0535 */
    uint8_t   spray_set;                       /* +0x0536 */
    int8_t    spray_pow;                       /* +0x0537 */
    uint16_t  damage_no;                       /* +0x0538 */
    uint16_t  attack_no; uint8_t _pad_attack_no[4]; /* +0x053a */
} PlayerState;


#ifdef __cplusplus
}
#endif

#endif /* SH2_TYPES_H */
