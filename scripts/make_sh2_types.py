#!/usr/bin/env python3

with open('include/sh2/player_state_gen.h') as f:
    ps_text = f.read()

header = '''/*
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

''' + ps_text + '''

#ifdef __cplusplus
}
#endif

#endif /* SH2_TYPES_H */
'''

with open('include/sh2/sh2_types.h', 'w') as f:
    f.write(header)
