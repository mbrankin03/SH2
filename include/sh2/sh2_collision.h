/**
 * @file sh2_collision.h
 * @brief Silent Hill 2 Collision & Raycast Hit Structures
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28:
 * - `_CL_VHIT_WALL` (48B, DIE @0x10abd)
 * - `_CL_VHIT_CHARA` (32B, DIE @0xdbbf)
 * - `_CL_VHIT_RESULT` (64B, DIE @0xdcdb)
 * - `_CL_HITPOLY_COLUMN` (48B, DIE @0xe53b)
 */

#ifndef SH2_COLLISION_H
#define SH2_COLLISION_H

#include "sh2_math.h"

#ifdef __cplusplus
extern "C" {
#endif

struct SubCharacter;
struct _CL_HITPOLY_HEAD;

/**
 * Wall Raycast Hit Payload (48 bytes, DIE @0x10abd)
 */
typedef struct _CL_VHIT_WALL {
    Vector4 cp;                          /* +0x00 (0)  Contact point */
    Vector4 nl;                          /* +0x10 (16) Surface normal */
    SH2_PTR32(struct _CL_HITPOLY_HEAD) pd; /* +0x20 (32) Hit polygon header pointer */
    uint8_t _pad[12];                    /* +0x24 (36) PS2 quadword tail padding */
} _CL_VHIT_WALL;

/**
 * Character Raycast Hit Payload (32 bytes, DIE @0xdbbf)
 */
typedef struct _CL_VHIT_CHARA {
    SH2_PTR32(struct SubCharacter) sc;   /* +0x00 (0)  Hit character pointer */
    uint8_t _pad[12];                    /* +0x04 (4)  16-byte alignment padding for cp */
    Vector4 cp;                          /* +0x10 (16) Contact point */
} _CL_VHIT_CHARA;

/**
 * Union of Wall and Character Hit Objects (48 bytes, DIE @0xdd47)
 */
typedef union _CL_VHIT_HOBJ {
    _CL_VHIT_WALL wall;                  /* +0x00 (0)  48 bytes */
    _CL_VHIT_CHARA chara;                /* +0x00 (0)  32 bytes */
} _CL_VHIT_HOBJ;

/**
 * Raycast / Sensor Hit Result (64 bytes, DIE @0xdcdb)
 */
typedef struct _CL_VHIT_RESULT {
    int32_t kind;                        /* +0x00 (0)  Hit object classification */
    uint8_t _pad[12];                    /* +0x04 (4)  16-byte alignment padding for hobj */
    _CL_VHIT_HOBJ hobj;                  /* +0x10 (16) Hit object payload (wall or chara) */
} _CL_VHIT_RESULT;

/**
 * Cylindrical / Column Collision Volume (48 bytes, DIE @0xe53b)
 */
typedef struct _CL_HITPOLY_COLUMN {
    uint8_t kind;                        /* +0x00 (0)  Collision kind */
    uint8_t shape;                       /* +0x01 (1)  Collision shape */
    uint16_t pad;                        /* +0x02 (2)  Alignment padding */
    uint32_t weight;                     /* +0x04 (4)  Collision weight */
    uint32_t material;                   /* +0x08 (8)  Surface material ID */
    int32_t flg;                         /* +0x0C (12) Collision flags */
    float p[2][4];                       /* +0x10 (16) Top/bottom cylinder endpoints (32B) */
} _CL_HITPOLY_COLUMN;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(_CL_VHIT_WALL) == 48, "_CL_VHIT_WALL must be 48 bytes");
_Static_assert(offsetof(_CL_VHIT_WALL, cp) == 0x00, "_CL_VHIT_WALL.cp offset mismatch");
_Static_assert(offsetof(_CL_VHIT_WALL, nl) == 0x10, "_CL_VHIT_WALL.nl offset mismatch");
_Static_assert(offsetof(_CL_VHIT_WALL, pd) == 0x20, "_CL_VHIT_WALL.pd offset mismatch");

_Static_assert(sizeof(_CL_VHIT_CHARA) == 32, "_CL_VHIT_CHARA must be 32 bytes");
_Static_assert(offsetof(_CL_VHIT_CHARA, sc) == 0x00, "_CL_VHIT_CHARA.sc offset mismatch");
_Static_assert(offsetof(_CL_VHIT_CHARA, cp) == 0x10, "_CL_VHIT_CHARA.cp offset mismatch");

_Static_assert(sizeof(_CL_VHIT_RESULT) == 64, "_CL_VHIT_RESULT must be 64 bytes");
_Static_assert(offsetof(_CL_VHIT_RESULT, kind) == 0x00, "_CL_VHIT_RESULT.kind offset mismatch");
_Static_assert(offsetof(_CL_VHIT_RESULT, hobj) == 0x10, "_CL_VHIT_RESULT.hobj offset mismatch");

_Static_assert(sizeof(_CL_HITPOLY_COLUMN) == 48, "_CL_HITPOLY_COLUMN must be 48 bytes");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, kind) == 0x00, "_CL_HITPOLY_COLUMN.kind offset mismatch");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, shape) == 0x01, "_CL_HITPOLY_COLUMN.shape offset mismatch");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, pad) == 0x02, "_CL_HITPOLY_COLUMN.pad offset mismatch");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, weight) == 0x04, "_CL_HITPOLY_COLUMN.weight offset mismatch");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, material) == 0x08, "_CL_HITPOLY_COLUMN.material offset mismatch");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, flg) == 0x0C, "_CL_HITPOLY_COLUMN.flg offset mismatch");
_Static_assert(offsetof(_CL_HITPOLY_COLUMN, p) == 0x10, "_CL_HITPOLY_COLUMN.p offset mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SH2_COLLISION_H */
