/**
 * @file sh2_camera.h
 * @brief Silent Hill 2 Virtual Camera System Structures
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28:
 * - `_VC_CAM_MV_PARAM` (16B)
 * - `_VC_WATCH_MV_PARAM` (12B)
 * - `_VC_THROUGH_DOOR_CAM_PARAM` (48B)
 * - `_VC_FIX_CAM_MANAGER` (16B)
 * - `_VC_CIR_CAM_MANAGER` (32B)
 * - `Camera` / `VC_WORK` (`vcWork`, 416B / 0x1A0)
 */

#ifndef SH2_CAMERA_H
#define SH2_CAMERA_H

#include "sh2_math.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Camera Movement Acceleration / Speed Parameters (16 bytes)
 */
typedef struct _VC_CAM_MV_PARAM {
    float accel_xz; /* +0x00 (0) Horizontal acceleration */
    float accel_y;  /* +0x04 (4) Vertical acceleration */
    float max_spd_xz; /* +0x08 (8) Maximum horizontal speed */
    float max_spd_y;  /* +0x0C (12) Maximum vertical speed */
} _VC_CAM_MV_PARAM;

/**
 * Camera Watch / Look-At Angular Parameters (12 bytes)
 */
typedef struct _VC_WATCH_MV_PARAM {
    float ang_accel_x; /* +0x00 (0) Pitch angular acceleration */
    float ang_accel_y; /* +0x04 (4) Yaw angular acceleration */
    float max_ang_spd_x; /* +0x08 (8) Maximum pitch speed */
    float max_ang_spd_y; /* +0x0C (12) Note: 12B in DWARF if 3 floats, see layout below */
} _VC_WATCH_MV_PARAM_FULL;

typedef struct _VC_WATCH_MV_PARAM_12 {
    float ang_accel_x; /* +0x00 (0) */
    float ang_accel_y; /* +0x04 (4) */
    float max_ang_spd; /* +0x08 (8) */
} _VC_WATCH_MV_PARAM;

/**
 * Through-Door Camera Transition Parameters (48 bytes)
 */
typedef struct _VC_THROUGH_DOOR_CAM_PARAM {
    int32_t active_f;       /* +0x00 (0)  Transition active flag */
    float timer;            /* +0x04 (4)  Transition elapsed timer */
    float rail_ang_y;       /* +0x08 (8)  Doorway rail yaw angle */
    uint8_t _pad0[4];       /* +0x0C (12) 16-byte alignment padding */
    Vector4 rail_sta_pos;   /* +0x10 (16) Rail start position */
    float rail_sta_to_chara_dist; /* +0x20 (32) Distance from rail start to character */
    uint8_t _pad1[12];      /* +0x24 (36) PS2 quadword tail padding */
} _VC_THROUGH_DOOR_CAM_PARAM;

/**
 * Fixed Camera Manager (16 bytes)
 */
typedef struct _VC_FIX_CAM_MANAGER {
    int32_t exception_f;    /* +0x00 (0) Exception state flag */
    float add_ang_y;        /* +0x04 (4) Additional yaw angle offset */
    float add_rd_x;         /* +0x08 (8) Additional road X offset */
    float add_rd_z;         /* +0x0C (12) Additional road Z offset */
} _VC_FIX_CAM_MANAGER;

/**
 * Circular Camera Manager (32 bytes)
 */
typedef struct _VC_CIR_CAM_MANAGER {
    Vector4 origin;         /* +0x00 (0)  Orbit center position */
    Vector4 sw_p;           /* +0x10 (16) Switch / target point */
} _VC_CIR_CAM_MANAGER;

/**
 * Main Virtual Camera Workspace (`vcWork`, 416 bytes / 0x1A0)
 * Uses `SH2_PTR32(void)` for `vc_road_ary_list` at `+0x018` so 64-bit host builds
 * preserve exact 32-bit PS2 EE / x86 field offsets.
 */
typedef struct Camera {
    int32_t view_cam_active_f;      /* +0x000 (0)   Active camera flag */
    SH2_PTR32(void) vc_road_ary_list; /* +0x004 (4)   32-bit pointer to camera road array */
    uint32_t flags;                 /* +0x008 (8)   Camera state bitflags */
    int32_t through_door_activate_init_f; /* +0x00C (12) Door transition init flag */
    _VC_THROUGH_DOOR_CAM_PARAM through_door; /* +0x010 (16) Through-door state (48B) */
    float scr_half_ang_wy;          /* +0x040 (64)  Vertical half-FOV angle */
    float scr_half_ang_wx;          /* +0x044 (68)  Horizontal half-FOV angle */
    _VC_CAM_MV_PARAM user_cam_mv_prm; /* +0x048 (72) User camera movement params (16B) */
    Vector4 cam_tgt_pos;            /* +0x058 (88)  Camera target position (16B) */
    Vector4 cam_pos;                /* +0x068 (104) Current camera world position (16B) */
    Vector4 cam_mv_ang;             /* +0x078 (120) Camera movement velocity/angles (16B) */
    Vector4 cam_velo;               /* +0x088 (136) Camera velocity vector (16B) */
    float old_cam_excl_area_r;      /* +0x098 (152) Previous exclusion radius */
    Vector4 ideal_cam_pos_h;        /* +0x09C (156) Horizontal ideal camera position (16B) */
    _VC_WATCH_MV_PARAM user_watch_mv_prm; /* +0x0AC (172) Watch angular parameters (12B) */
    Vector4 watch_tgt_pos;          /* +0x0B8 (184) Look-at target world position (16B) */
    float watch_tgt_max_y;          /* +0x0C8 (200) Maximum look-at target Y */
    float watch_tgt_ang_z;          /* +0x0CC (204) Look-at target roll angle */
    Vector3 cam_mat_ang;            /* +0x0D0 (208) Camera matrix Euler angles (12B) */
    uint8_t _pad0[4];               /* +0x0DC (220) Alignment padding */
    Matrix4x4 cam_mat;              /* +0x0E0 (224) Camera transformation matrix (64B) */
    Vector3 ofs_cam_ang;            /* +0x120 (288) Offset camera Euler angles (12B) */
    Vector3 ofs_cam_ang_spd;        /* +0x12C (300) Offset camera angular speed (12B) */
    Vector3 base_cam_ang;           /* +0x138 (312) Base camera Euler angles (12B) */
    uint8_t old_base_cam_ang[12];   /* +0x144 (324) Previous base camera angles (12B) */
    float cam_chara2ideal_ang_y;    /* +0x150 (336) Character-to-ideal camera yaw */
    Vector4 cam_tgt_velo;           /* +0x154 (340) Camera target velocity (16B) */
    float cam_tgt_mv_ang_y;         /* +0x164 (356) Camera target movement yaw */
    float cam_tgt_spd;              /* +0x168 (360) Camera target scalar speed */
    float chara_pos[4];             /* +0x16C (364) Tracked character position (16B) */
    float chara_bottom_y;           /* +0x17C (380) Character feet Y coordinate */
    float chara_top_y;              /* +0x180 (384) Character head Y coordinate */
    float chara_center_y;           /* +0x184 (388) Character center Y coordinate */
    float chara_grnd_y;             /* +0x188 (392) Character ground Y coordinate */
    Vector3 chara_head_pos;         /* +0x18C (396) Character head 3D position (12B) */
    float chara_mv_spd;             /* +0x198 (408) Character movement scalar speed */
    float chara_mv_ang_y;           /* +0x19C (412) Character movement yaw angle */
} Camera;

typedef Camera CameraState;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(_VC_CAM_MV_PARAM) == 16, "_VC_CAM_MV_PARAM must be 16 bytes");
_Static_assert(sizeof(_VC_WATCH_MV_PARAM) == 12, "_VC_WATCH_MV_PARAM must be 12 bytes");
_Static_assert(sizeof(_VC_THROUGH_DOOR_CAM_PARAM) == 48, "_VC_THROUGH_DOOR_CAM_PARAM must be 48 bytes");
_Static_assert(sizeof(_VC_FIX_CAM_MANAGER) == 16, "_VC_FIX_CAM_MANAGER must be 16 bytes");
_Static_assert(sizeof(_VC_CIR_CAM_MANAGER) == 32, "_VC_CIR_CAM_MANAGER must be 32 bytes");
_Static_assert(sizeof(Camera) == 416, "Camera must be 416 bytes (0x1A0)");
_Static_assert(offsetof(Camera, vc_road_ary_list) == 0x004, "Camera.vc_road_ary_list offset mismatch");
_Static_assert(offsetof(Camera, through_door) == 0x010, "Camera.through_door offset mismatch");
_Static_assert(offsetof(Camera, cam_pos) == 0x068, "Camera.cam_pos offset mismatch");
_Static_assert(offsetof(Camera, watch_tgt_pos) == 0x0B8, "Camera.watch_tgt_pos offset mismatch");
_Static_assert(offsetof(Camera, cam_mat) == 0x0E0, "Camera.cam_mat offset mismatch");
_Static_assert(offsetof(Camera, chara_mv_ang_y) == 0x19C, "Camera.chara_mv_ang_y offset mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SH2_CAMERA_H */
