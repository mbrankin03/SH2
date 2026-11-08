/**
 * @file sh2_math.h
 * @brief Silent Hill 2 Core Math & 32-Bit Pointer Primitives
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28.
 * Provides 32-bit pointer layout parity on 64-bit hosts via `sh2_ptr32_t`,
 * as well as Vector3, Vector4, Matrix4x4, and _VbWVSMATRIX.
 */

#ifndef SH2_MATH_H
#define SH2_MATH_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * 32-bit PS2 EE / x86 virtual memory address.
 * Ensures C structs on 64-bit host builds retain identical size and field
 * offsets to the original 32-bit PS2 EE DWARF-1 and Rust (`u32`) definitions.
 */
typedef uint32_t sh2_ptr32_t;

/**
 * Self-documenting macro for 32-bit pointers in DWARF-1 structures.
 */
#define SH2_PTR32(type) sh2_ptr32_t

/**
 * 3D Cartesian Vector (12 bytes)
 */
typedef struct Vector3 {
    float x; /* +0x00 (0) */
    float y; /* +0x04 (4) */
    float z; /* +0x08 (8) */
} Vector3;

/**
 * 4D Homogeneous / PS2 VU0 Quadword Vector (16 bytes, DIE @0xe3e9)
 */
typedef struct Vector4 {
    float x; /* +0x00 (0) */
    float y; /* +0x04 (4) */
    float z; /* +0x08 (8) */
    float w; /* +0x0C (12) */
} Vector4;

/**
 * 4x4 Floating-Point Transformation Matrix (64 bytes, DIE @0xe988)
 */
typedef struct Matrix4x4 {
    float m[4][4]; /* +0x00 (0) */
} Matrix4x4;

/**
 * World-View-Screen Matrix Bundle (192 bytes, DIE @0x2cf22)
 * Maps to `VbWvsMatrix` at 0x01C6E440 in SLUS_202.28.
 */
typedef struct _VbWVSMATRIX {
    Matrix4x4 wvm; /* +0x00 (0)   World-to-View Matrix */
    Matrix4x4 vsm; /* +0x40 (64)  View-to-Screen Matrix */
    Matrix4x4 wsm; /* +0x80 (128) World-to-Screen Matrix */
} _VbWVSMATRIX;

typedef _VbWVSMATRIX WorldViewScreenMatrices;

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(sh2_ptr32_t) == 4, "sh2_ptr32_t must be 4 bytes");
_Static_assert(sizeof(Vector3) == 12, "Vector3 must be 12 bytes");
_Static_assert(sizeof(Vector4) == 16, "Vector4 must be 16 bytes");
_Static_assert(sizeof(Matrix4x4) == 64, "Matrix4x4 must be 64 bytes");
_Static_assert(sizeof(_VbWVSMATRIX) == 192, "_VbWVSMATRIX must be 192 bytes");
_Static_assert(offsetof(_VbWVSMATRIX, wvm) == 0x00, "_VbWVSMATRIX.wvm offset mismatch");
_Static_assert(offsetof(_VbWVSMATRIX, vsm) == 0x40, "_VbWVSMATRIX.vsm offset mismatch");
_Static_assert(offsetof(_VbWVSMATRIX, wsm) == 0x80, "_VbWVSMATRIX.wsm offset mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SH2_MATH_H */
