/**
 * @file sh2_pad.h
 * @brief Silent Hill 2 Controller Input Structures
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28:
 * - `PAD_3D` (6B, DIE @0xe843)
 * - `PAD_2D` (8B, DIE @0xeaa6)
 * - `PAD_INFO` (32B, DIE @0xed62)
 */

#ifndef SH2_PAD_H
#define SH2_PAD_H

#include "sh2_math.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * 3D Tank-Control Input State (6 bytes, DIE @0xe843)
 */
typedef struct PAD_3D {
    int8_t round_way;   /* +0x00 (0) Turning direction */
    uint8_t reserved1;  /* +0x01 (1) Reserved padding */
    uint8_t lslide;     /* +0x02 (2) Left strafe / slide pressure */
    uint8_t rslide;     /* +0x03 (3) Right strafe / slide pressure */
    uint8_t lturn180;   /* +0x04 (4) Left 180-degree quick-turn trigger */
    uint8_t rturn180;   /* +0x05 (5) Right 180-degree quick-turn trigger */
} PAD_3D;

/**
 * 2D Camera-Relative Control Input State (8 bytes, DIE @0xeaa6)
 */
typedef struct PAD_2D {
    uint8_t pow;          /* +0x00 (0) Movement magnitude */
    uint8_t lower_motion; /* +0x01 (1) Lower-body motion state */
    uint8_t _pad[2];      /* +0x02 (2) Alignment padding */
    float dir;            /* +0x04 (4) Movement direction angle (radians) */
} PAD_2D;

/**
 * Union of 3D and 2D Control Sub-Structures (8 bytes)
 */
typedef union PAD_MODE_DATA {
    PAD_3D pad3d;         /* +0x00 (0) 6 bytes */
    PAD_2D pad2d;         /* +0x00 (0) 8 bytes */
} PAD_MODE_DATA;

/**
 * Processed Controller Frame Snapshot (32 bytes, DIE @0xed62)
 * Stored in a 10-element history ring buffer (`pad[10]`, 320 bytes) inside `shPlayerWork`.
 */
typedef struct PAD_INFO {
    uint8_t action;       /* +0x00 (0)  Action / confirm button */
    uint8_t dash;         /* +0x01 (1)  Run / dash button */
    uint8_t attack0;      /* +0x02 (2)  Primary attack input */
    uint8_t attack1;      /* +0x03 (3)  Secondary attack input */
    uint8_t attack2;      /* +0x04 (4)  Tertiary attack input */
    uint8_t menu;         /* +0x05 (5)  Inventory / status screen button */
    uint8_t hold;         /* +0x06 (6)  Weapon ready / aim hold button */
    uint8_t search;       /* +0x07 (7)  Search / camera view button */
    uint8_t map;          /* +0x08 (8)  Map toggle button */
    union {
        struct {
            uint8_t light_ : 4; /* +0x09 (9) Low nibble */
            uint8_t light  : 4; /* +0x09 (9) High nibble */
        };
        uint8_t light_raw;      /* +0x09 (9) Full flashlight button byte */
    };
    uint8_t l_button;     /* +0x0A (10) Left shoulder button */
    uint8_t r_button;     /* +0x0B (11) Right shoulder button */
    int8_t lstickX;       /* +0x0C (12) Left analog stick X (-128..127) */
    int8_t lstickY;       /* +0x0D (13) Left analog stick Y (-128..127) */
    int8_t rstickX;       /* +0x0E (14) Right analog stick X (-128..127) */
    int8_t rstickY;       /* +0x0F (15) Right analog stick Y (-128..127) */
    uint8_t forward;      /* +0x10 (16) Move forward pressure */
    uint8_t backward;     /* +0x11 (17) Move backward pressure */
    uint8_t lround;       /* +0x12 (18) Turn left pressure */
    uint8_t rround;       /* +0x13 (19) Turn right pressure */
    uint8_t skip;         /* +0x14 (20) Cutscene skip button */
    uint8_t pause;        /* +0x15 (21) Pause button */
    uint8_t _pad[2];      /* +0x16 (22) Alignment padding before +0x18 union */
    union {
        PAD_3D pad3d;     /* +0x18 (24) 3D control mode state (6B) */
        PAD_2D pad2d;     /* +0x18 (24) 2D control mode state (8B) */
    };
} PAD_INFO;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(PAD_3D) == 6, "PAD_3D must be 6 bytes");
_Static_assert(sizeof(PAD_2D) == 8, "PAD_2D must be 8 bytes");
_Static_assert(offsetof(PAD_2D, dir) == 0x04, "PAD_2D.dir offset mismatch");

_Static_assert(sizeof(PAD_INFO) == 32, "PAD_INFO must be 32 bytes");
_Static_assert(offsetof(PAD_INFO, action) == 0x00, "PAD_INFO.action offset mismatch");
_Static_assert(offsetof(PAD_INFO, light_raw) == 0x09, "PAD_INFO.light_raw offset mismatch");
_Static_assert(offsetof(PAD_INFO, lstickX) == 0x0C, "PAD_INFO.lstickX offset mismatch");
_Static_assert(offsetof(PAD_INFO, forward) == 0x10, "PAD_INFO.forward offset mismatch");
_Static_assert(offsetof(PAD_INFO, pause) == 0x15, "PAD_INFO.pause offset mismatch");
_Static_assert(offsetof(PAD_INFO, pad3d) == 0x18, "PAD_INFO.pad3d offset mismatch");
_Static_assert(offsetof(PAD_INFO, pad2d) == 0x18, "PAD_INFO.pad2d offset mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SH2_PAD_H */
